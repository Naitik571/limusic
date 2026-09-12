<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PreviousIcon,
		NextIcon,
		PlayIcon,
		PauseIcon,
		ShuffleIcon,
		RepeatIcon,
		RepeatOne01Icon,
		Queue01Icon,
		Mic01Icon,
		VolumeHighIcon,
		VolumeMute02Icon,
		FavouriteIcon,
		Add01Icon,
		InfinityIcon,
		MinimizeScreenIcon,
		MusicNote01Icon,
		ArrowUp01Icon,
		ArrowDown01Icon,
		Moon01Icon
	} from '@hugeicons/core-free-icons';
	import { fade } from 'svelte/transition';
	import { goto } from '$app/navigation';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import {
		np,
		playback,
		commitVolume,
		cycleRepeat,
		dragVolume,
		likeBursts,
		openAddToPlaylist,
		openMiniPlayer,
		setSleepTimer,
		sleepTimer,
		toggleMute,
		toggleNowPlayingLike,
		wheelVolume,
		volumeHud,
		type SleepTimerMode
	} from '$lib/player.svelte';
	import { anchorMenu, claimMenu, fitMenu, nextMenuId, NO_ANCHOR, onOtherMenuClaimed, toBody } from '$lib/menu';
	import { thumb } from '$lib/thumb';
	import { t } from '$lib/i18n.svelte';
	import { appearance, setAppearance } from '$lib/theme.svelte';
	import ArtistLine from './ArtistLine.svelte';
	import TrackMenu from './TrackMenu.svelte';

	let {
		onToggleQueue,
		queueOpen,
		onToggleLyrics,
		lyricsOpen
	}: {
		onToggleQueue: () => void;
		queueOpen: boolean;
		onToggleLyrics: () => void;
		lyricsOpen: boolean;
	} = $props();

	// Heart burst, same as TrackRow's: ~600ms of heart-pop + six sparks (layout.css) when the
	// current track gets liked ON. Driven by the shared `likeBursts` stamp rather than only the
	// bar's own click, so liking this track anywhere (row heart, ⋯ menu, another surface) pops the
	// bar's heart too. `shownBurstAt` keeps a stale entry from replaying on remount or on an
	// unlucky track change.
	let burst = $state(false);
	let burstTimer: ReturnType<typeof setTimeout> | undefined;
	let shownBurstAt = 0;

	$effect(() => {
		const id = playback.now?.videoId;
		const at = id ? (likeBursts[id] ?? 0) : 0;
		if (at === 0 || at <= shownBurstAt || Date.now() - at > 1000) return;
		shownBurstAt = at;
		burst = true;
		clearTimeout(burstTimer);
		burstTimer = setTimeout(() => (burst = false), 600);
	});

	function toggleLike() {
		toggleNowPlayingLike();
	}

	const fmt = (secs: number) => {
		if (!secs || secs < 0) return '0:00';
		const t = Math.floor(secs);
		const h = Math.floor(t / 3600);
		const m = Math.floor((t % 3600) / 60);
		const s = t % 60;
		const mm = h ? m.toString().padStart(2, '0') : `${m}`;
		return `${h ? `${h}:` : ''}${mm}:${s.toString().padStart(2, '0')}`;
	};

	const shuffleOn = $derived(playback.queue.shuffle ?? false);
	const repeat = $derived(playback.queue.repeat ?? 'off');

	// Sleep timer chip menu (same anchored-popup pattern as TrackMenu).
	let sleepMenuOpen = $state(false);
	let sleepAnchor = $state(NO_ANCHOR);

	// One menu at a time (see TrackMenu).
	const sleepMenuId = nextMenuId();
	$effect(() => onOtherMenuClaimed(sleepMenuId, () => (sleepMenuOpen = false)));

	function openSleepMenu(e: MouseEvent) {
		e.stopPropagation();
		sleepAnchor = anchorMenu(e, { align: 'right' });
		sleepMenuOpen = true;
		claimMenu(sleepMenuId);
	}
	function closeSleepMenu(e: MouseEvent) {
		e.stopPropagation();
		sleepMenuOpen = false;
	}
	function pickSleep(e: MouseEvent, mode: SleepTimerMode, minutes = 30) {
		e.stopPropagation();
		sleepMenuOpen = false;
		setSleepTimer(mode, minutes);
	}

	// The current track was appended by autoplay â†’ show the subtle âˆž badge next to the title.
	// Matched against the now-playing videoId so a transient queue/now-playing mismatch (mid
	// gapless advance) can't flash the badge on the wrong song.
	const autoplayTrack = $derived.by(() => {
		const cur = playback.queue.items[playback.queue.currentIndex];
		return !!cur?.autoplay && cur.video_id === playback.now?.videoId;
	});

	// The ⋮ menu needs the full SongItem — NowPlaying carries no album_id. Take it from the queue
	// row, matched on videoId so a mid-advance mismatch can't point the menu at the wrong song.
	const currentSong = $derived.by(() => {
		const cur = playback.queue.items[playback.queue.currentIndex];
		return cur?.video_id === playback.now?.videoId ? cur : null;
	});

	// The title links to the song's album (there is no per-song page). Local files carry no
	// album_id, so their title stays plain text.
	const albumId = $derived(
		currentSong && !api.isLocalId(currentSong.video_id) ? currentSong.album_id : undefined
	);

	// Seek: while dragging, hold a local value so incoming mpv position ticks can't yank the thumb
	// back under the pointer; only invoke the (expensive) seek on release.
	let seekDrag = $state<number | null>(null);
	const shownPosition = $derived(seekDrag ?? playback.position);

	// No loading/resolving flag exists on `playback` (now/queue/paused/position/duration/volume/liked
	// only): while a new stream is resolving we know the track but have no duration yet, so that
	// window stands in for "resolving" and drives the .seek-preparing shimmer (layout.css).
	const seekResolving = $derived(playback.now != null && !(playback.duration > 0));

	function onSeekInput(e: Event) {
		seekDrag = Number((e.target as HTMLInputElement).value);
	}
	function onSeekCommit(e: Event) {
		const v = Number((e.target as HTMLInputElement).value);
		playback.position = v;
		seekDrag = null;
		api.seek(v);
	}

	const onVolume = (e: Event) => dragVolume(Number((e.target as HTMLInputElement).value));
	const onVolumeCommit = (e: Event) => commitVolume(Number((e.target as HTMLInputElement).value));

	// Scroll-wheel volume: a wheel anywhere on the bar (sliders included) steps the volume, 5% per
	// notch, matching the shortcut keys. wheelVolume reuses nudgeVolume, so a held scroll is one
	// IPC per frame and persists once the gesture stops. A % badge over the cover art shows the
	// level and clears ~1s after the last notch.
	let volBadge = $state<number | null>(null);
	let volBadgeTimer: ReturnType<typeof setTimeout> | undefined;

	function onBarWheel(e: WheelEvent) {
		wheelVolume(e);
		volBadge = playback.volume;
		clearTimeout(volBadgeTimer);
		volBadgeTimer = setTimeout(() => (volBadge = null), 900);
	}

	// Anywhere on the bar that isn't a control opens (or closes) the now-playing view: the bar is
	// what's left of it once it's minimised, so it's the way back in. Deliberately no pointer
	// cursor, because this is the whole bar, not a button, and every real button keeps its own click.
	function onBarClick(e: MouseEvent) {
		if ((e.target as HTMLElement).closest('button, a, input, [role="button"]')) return;
		np.open = !np.open;
	}
</script>

	<!-- Precise volume HUD: shows on any nudgeVolume/dragVolume, auto-hides after 1.5s -->
	{#if volumeHud.visible}
		<div
			class="pointer-events-none fixed bottom-20 left-1/2 z-50 flex -translate-x-1/2 items-center gap-3 rounded-[var(--r-xl)] glass-strong px-4 py-2 shadow-[var(--elevation-3)]"
			transition:fade={{ duration: 150 }}
			aria-live="polite"
		>
			<HugeiconsIcon strokeWidth={2} icon={volumeHud.value === 0 ? VolumeMute02Icon : VolumeHighIcon} class="h-4 w-4 text-primary" />
			<div class="h-1.5 w-24 overflow-hidden rounded-full bg-muted">
				<div class="h-full bg-primary transition-all" style="width:{volumeHud.value}%"></div>
			</div>
			<span class="min-w-8 text-right text-xs font-bold tabular-nums">{volumeHud.value}%</span>
		</div>
	{/if}

<!-- The chevron button below is the keyboard equivalent of clicking the bar, so the bar itself
     stays a plain region rather than becoming a focusable control wrapping every other control. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
<footer
	onclick={onBarClick}
	onwheel={onBarWheel}
	class="flex items-center gap-2 border-x-0 border-b-0 glass px-2 py-2.5 sm:gap-4 sm:px-4 sm:py-3"
>
	<!-- Now playing. data-ctx: right-clicking the cover, the title or the space around them opens
	     the â‹¯ menu for the track that's playing. -->
	<div class="flex min-w-0 flex-1 items-center gap-3" data-ctx>
		<button
			type="button"
			class="group relative block shrink-0 cursor-pointer rounded-[var(--r-lg)] bg-transparent p-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
			onclick={(e) => {
				e.stopPropagation();
				api.togglePause();
			}}
			aria-label={playback.paused ? t('player.play') : t('player.pause')}
			title={playback.paused ? t('player.play') : t('player.pause')}
		>
			{#key playback.now?.videoId}
				{#if playback.now?.thumbnail}
					<img decoding="async"
						src={thumb(playback.now.thumbnail, 120)}
						alt={playback.now?.title ? `${playback.now.title}${playback.now.artists ? ` by ${playback.now.artists}` : ''}` : ''}
						style="max-width:none"
						class="h-12 w-12 rounded-[var(--r-lg)] object-cover"
						in:fade={{ duration: 250 }}
					/>
				{:else}
					<div
						class="flex h-12 w-12 items-center justify-center rounded-[var(--r-lg)] bg-muted text-muted-foreground/50"
					>
						<HugeiconsIcon strokeWidth={2} icon={MusicNote01Icon} class="h-5 w-5" />
					</div>
				{/if}
			{/key}
			{#if volBadge !== null}
				<span
					class="timer pointer-events-none absolute -top-2 -left-2 z-10 rounded-[var(--r-md)] bg-[var(--scrim)] px-1.5 py-0.5 text-[var(--text-caption)] font-bold text-[var(--on-art)] shadow-[var(--elevation-2)]"
					data-timer
					transition:fade={{ duration: 150 }}
					aria-hidden="true"
				>
					{volBadge}%
				</span>
			{/if}
		</button>
		<div class="min-w-0">
			<div class="flex items-center gap-1.5">
				{#if albumId}
					<button
						class="min-w-0 cursor-pointer truncate text-left text-sm font-medium hover:underline"
						onclick={() => goto(`/album/${encodeURIComponent(albumId)}`)}
						title="Go to album"
					>
						{playback.now?.title ?? 'Nothing playing'}
					</button>
				{:else}
					<div class="truncate text-sm font-medium">{playback.now?.title ?? 'Nothing playing'}</div>
				{/if}
				{#if autoplayTrack}
					<span
						class="shrink-0 text-muted-foreground"
						title="Playing similar music (Autoplay)"
						in:fade={{ duration: 200 }}
					>
						<HugeiconsIcon strokeWidth={2} icon={InfinityIcon} class="h-4 w-4" />
					</span>
				{/if}
			</div>
			<ArtistLine
				runs={playback.now?.artistRuns}
				text={playback.now?.artists ?? ''}
				class="block max-w-full text-xs text-muted-foreground"
			/>
		</div>
		{#if playback.now}
			<div class="flex items-center">
				<!-- A local file has no YouTube identity (see api.isLocalId): nothing to like, and no
				     YTM playlist to add it to. -->
				{#if !api.isLocalId(playback.now.videoId)}
					<Button variant="ghost" size="icon-sm" onclick={toggleLike} aria-label={playback.liked ? t('player.remove_from_liked') : t('player.save_to_liked')}>
					<span class="relative inline-flex">
						{#if burst}
							<!-- 6 sparks flying outward; angles are spread by --a in the keyframes (layout.css). -->
							{#each Array(6) as _, i}
								<span class="heart-spark" style="--a:{i * 60}deg" aria-hidden="true"></span>
							{/each}
						{/if}
						<HugeiconsIcon strokeWidth={2}
							icon={FavouriteIcon}
							class="h-4 w-4 {playback.liked ? 'fill-current text-primary' : 'text-muted-foreground'} {burst
								? 'heart-pop'
								: ''}"
						/>
					</span>
				</Button>
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={() => {
							const now = playback.now!;
							openAddToPlaylist({
								video_id: now.videoId,
								title: now.title,
								artists: now.artists,
								artist_id: now.artistId,
								thumbnail: now.thumbnail,
								duration: now.duration
							});
						}}
						aria-label={t('player.add_to_playlist')}
					>
						<HugeiconsIcon strokeWidth={2} icon={Add01Icon} class="h-4 w-4 text-muted-foreground" />
					</Button>
				{/if}
				{#if currentSong}
					<TrackMenu
						song={currentSong}
						linksOnly
						triggerClass="inline-flex size-8 cursor-pointer items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground"
					/>
				{/if}
			</div>
		{/if}
	</div>

	<!-- Transport -->
	<div class="flex flex-[1.5] flex-col items-center gap-1">
		<div class="flex items-center gap-1">
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={() => api.toggleShuffle()}
				aria-label={t('player.shuffle')}
				aria-pressed={shuffleOn}
			>
				<HugeiconsIcon strokeWidth={2}
					icon={ShuffleIcon}
					class="h-4 w-4 {shuffleOn ? 'text-primary' : 'text-muted-foreground'}"
				/>
			</Button>
			<Button variant="ghost" size="icon-sm" class="pressable" onclick={() => api.prevTrack()} aria-label={t('player.previous')}>
				<HugeiconsIcon strokeWidth={2} icon={PreviousIcon} class="h-5 w-5" />
			</Button>
			<Button
				variant="default"
				size="icon"
				class="rounded-full pressable"
				onclick={() => api.togglePause()}
				aria-label={playback.paused ? t('player.play') : t('player.pause')}
			>
				<!-- HugeiconsIcon only re-renders `altIcon`/`showAlt`, not `icon` (frozen at mount) â€”
			     so toggle via showAlt, not a ternary on `icon`. -->
			<HugeiconsIcon strokeWidth={2}
				icon={PauseIcon}
				altIcon={PlayIcon}
				showAlt={playback.paused}
				class="h-5 w-5"
			/>
			</Button>
			<Button variant="ghost" size="icon-sm" class="pressable" onclick={() => api.nextTrack()} aria-label={t('player.next')}>
				<HugeiconsIcon strokeWidth={2} icon={NextIcon} class="h-5 w-5" />
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={cycleRepeat}
				aria-label="Repeat: {repeat}"
				aria-pressed={repeat !== 'off'}
			>
				<!-- icon swap via altIcon/showAlt â€” `icon` is frozen at mount (see play/pause above) -->
				<HugeiconsIcon strokeWidth={2}
					icon={RepeatIcon}
					altIcon={RepeatOne01Icon}
					showAlt={repeat === 'one'}
					class="h-4 w-4 {repeat !== 'off' ? 'text-primary' : 'text-muted-foreground'}"
				/>
			</Button>
		</div>
		<div class="flex w-full max-w-md items-center gap-2 text-xs text-muted-foreground" class:seek-preparing={seekResolving}>
			<span class="timer" data-timer>{fmt(shownPosition)}</span>
			<input
				type="range"
				class="range range-seek flex-1"
				style="--pct:{playback.duration ? (shownPosition / playback.duration) * 100 : 0}%"
				min="0"
				max={playback.duration || 0}
				value={shownPosition}
				oninput={onSeekInput}
				onchange={onSeekCommit}
				aria-label={t('player.seek')}
			/>
			<span class="timer" data-timer>{fmt(playback.duration)}</span>
		</div>
	</div>

	<!-- Volume + queue -->
	<div class="flex flex-1 items-center justify-end gap-2">
		<!-- Volume is the first control to drop on a narrow window (OS volume still works). -->
		<div class="hidden items-center gap-1 md:flex">
			<Button
				variant="ghost"
				size="icon-sm"
				class="text-muted-foreground"
				onclick={toggleMute}
				aria-label={playback.volume === 0 ? t('player.unmute') : t('player.mute')}
			>
				<!-- icon swap via altIcon/showAlt â€” `icon` is frozen at mount (see play/pause above) -->
				<HugeiconsIcon strokeWidth={2}
					icon={VolumeHighIcon}
					altIcon={VolumeMute02Icon}
					showAlt={playback.volume === 0}
					class="h-4 w-4"
				/>
			</Button>
			<input
				type="range"
				class="range w-24"
				style="--pct:{playback.volume}%"
				min="0"
				max="100"
				value={playback.volume}
				oninput={onVolume}
				onchange={onVolumeCommit}
				onwheel={wheelVolume}
				aria-label={t('player.volume')}
			/>
		</div>
		<!-- One cluster, so they sit tighter to each other than to the volume slider. -->
		<div class="flex items-center gap-0.5">
			<!-- Sleep timer chip: moon icon + countdown while armed; the menu offers presets,
			     end-of-song and cancel. Rust enforces the pause even if this window closes. -->
			<Button
				variant={sleepTimer.mode !== 'off' ? 'secondary' : 'ghost'}
				size="icon-sm"
				onclick={openSleepMenu}
				aria-label="Sleep timer"
				aria-expanded={sleepMenuOpen}
			>
				<HugeiconsIcon strokeWidth={2} icon={Moon01Icon} class="h-4 w-4" />
				{#if sleepTimer.mode !== 'off'}
					<span class="timer ml-0.5 text-[var(--text-caption)] font-medium" data-timer>
						{sleepTimer.mode === 'minutes' ? fmt(sleepTimer.remaining) : 'â™ª'}
					</span>
				{/if}
			</Button>
			{#if sleepMenuOpen}
				<button
					class="fixed inset-0 z-40 cursor-default"
					onclick={closeSleepMenu}
					aria-label="Close sleep timer menu"
					{@attach toBody}
				></button>
				<div
					class="fixed z-50 min-w-44 animate-in rounded-[var(--r-xl)] border-transparent glass-strong p-1 text-popover-foreground shadow-[var(--elevation-3)] duration-150 fade-in-0 zoom-in-95"
					style={sleepAnchor.style}
					{@attach toBody}
					{@attach fitMenu(sleepAnchor)}
				>
					<button
						class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
						onclick={(e) => pickSleep(e, 'minutes', 15)}
					>
						15 minutes
					</button>
					<button
						class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
						onclick={(e) => pickSleep(e, 'minutes', 30)}
					>
						30 minutes
					</button>
					<button
						class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
						onclick={(e) => pickSleep(e, 'minutes', 60)}
					>
						60 minutes
					</button>
					<button
						class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
						onclick={(e) => pickSleep(e, 'end_of_song')}
					>
						End of song
					</button>
					{#if sleepTimer.mode !== 'off'}
						<div class="my-1 h-px bg-border"></div>
						<button
							class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
							onclick={(e) => pickSleep(e, 'off')}
						>
							Cancel timer
						</button>
					{/if}
				</div>
			{/if}
			<Button variant="ghost" size="icon-sm" onclick={openMiniPlayer} aria-label="Mini player">
						<HugeiconsIcon strokeWidth={2} icon={MinimizeScreenIcon} class="h-4 w-4" />
					</Button>
					<Button
						variant={lyricsOpen ? 'secondary' : 'ghost'}
						size="icon-sm"
						onclick={onToggleLyrics}
						aria-label="Toggle lyrics"
					>
						<HugeiconsIcon strokeWidth={2} icon={Mic01Icon} class="h-4 w-4" />
					</Button>
			<Button
				variant={queueOpen ? 'secondary' : 'ghost'}
				size="icon-sm"
				onclick={onToggleQueue}
				aria-label="Toggle queue"
			>
				<HugeiconsIcon strokeWidth={2} icon={Queue01Icon} class="h-4 w-4" />
			</Button>
			<!-- The keyboard (and discoverable) way in and out of the now-playing view; clicking the
			     bar's empty space does the same thing. -->
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={() => (np.open = !np.open)}
				aria-label={np.open ? 'Minimise player' : 'Open player'}
				aria-expanded={np.open}
			>
				<!-- icon swap via altIcon/showAlt â€” `icon` is frozen at mount (see play/pause above) -->
				<HugeiconsIcon strokeWidth={2}
					icon={ArrowUp01Icon}
					altIcon={ArrowDown01Icon}
					showAlt={np.open}
					class="h-4 w-4"
				/>
			</Button>
		</div>
	</div>
</footer>
