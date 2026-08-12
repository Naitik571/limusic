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
		Moon01Icon,
		MaximizeScreenIcon
	} from '@hugeicons/core-free-icons';
	import { fade } from 'svelte/transition';
	import { Button } from '$lib/components/ui/button';
	import * as api from '$lib/api';
	import {
		np,
		playback,
		commitVolume,
		cycleRepeat,
		dragVolume,
		openAddToPlaylist,
		openMiniPlayer,
		setSleepTimer,
		sleepTimer,
		toggleMute,
		toggleNowPlayingLike,
		type SleepTimerMode
	} from '$lib/player.svelte';
	import { anchorMenu, toBody } from '$lib/menu';
	import { thumb } from '$lib/thumb';
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

	// Pop the heart once when the user favourites (not when un-favouriting). Reset on animation end
	// so the next like can replay it.
	let justLiked = $state(false);

	function toggleLike() {
		if (!playback.liked) justLiked = true;
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
	let mx = $state(0);
	let my = $state(0);
	let openUp = $state(false);

	function openSleepMenu(e: MouseEvent) {
		e.stopPropagation();
		({ right: mx, y: my, openUp } = anchorMenu(e.currentTarget as HTMLElement));
		sleepMenuOpen = true;
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

	// The current track was appended by autoplay → show the subtle ∞ badge next to the title.
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

	// Seek: while dragging, hold a local value so incoming mpv position ticks can't yank the thumb
	// back under the pointer; only invoke the (expensive) seek on release.
	let seekDrag = $state<number | null>(null);
	const shownPosition = $derived(seekDrag ?? playback.position);

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

	// Scroll-wheel volume: a wheel over the bar steps the volume (5% per notch, matching the
	// shortcut arrows) and pops a % badge over the cover art while you scroll. The badge
	// clears ~1s after the last notch.
	const VOLUME_STEP = 5;
	let volBadge = $state<number | null>(null);
	let volBadgeTimer: ReturnType<typeof setTimeout> | undefined;

	function onBarWheel(e: WheelEvent) {
		const v = Math.min(
			100,
			Math.max(0, playback.volume + (e.deltaY < 0 ? VOLUME_STEP : -VOLUME_STEP))
		);
		dragVolume(v); // same throttled path as the slider's oninput
		volBadge = v;
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

<!-- The chevron button below is the keyboard equivalent of clicking the bar, so the bar itself
     stays a plain region rather than becoming a focusable control wrapping every other control. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
<footer
	onclick={onBarClick}
	onwheel={onBarWheel}
	class="flex items-center gap-2 border-x-0 border-b-0 glass px-2 py-2.5 sm:gap-4 sm:px-4 sm:py-3"
>
	<!-- Now playing -->
		<div class="flex min-w-0 flex-1 items-center gap-3">
			<button
				type="button"
				class="group relative block shrink-0 cursor-pointer rounded-lg bg-transparent p-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
				onclick={(e) => {
					e.stopPropagation();
					api.togglePause();
				}}
				aria-label={playback.paused ? 'Play' : 'Pause'}
				title={playback.paused ? 'Play' : 'Pause'}
			>
				{#key playback.now?.videoId}
					{#if playback.now?.thumbnail}
						<img
							src={thumb(playback.now.thumbnail, 120)}
							alt=""
							style="max-width:none"
							class="h-12 w-12 rounded-lg object-cover"
							in:fade={{ duration: 250 }}
						/>
					{:else}
						<div
							class="flex h-12 w-12 items-center justify-center rounded-lg bg-muted text-muted-foreground/50"
						>
							<HugeiconsIcon icon={MusicNote01Icon} class="h-5 w-5" />
						</div>
					{/if}
				{/key}
				{#if volBadge !== null}
					<span
						class="pointer-events-none absolute -top-2 -left-2 z-10 rounded-md bg-black/85 px-1.5 py-0.5 text-[11px] font-bold text-white shadow-lg tabular-nums"
						transition:fade={{ duration: 150 }}
						aria-hidden="true"
					>
						{volBadge}%
					</span>
				{/if}
			</button>
		<div class="min-w-0">
			<div class="flex items-center gap-1.5">
				<div class="truncate text-sm font-medium">{playback.now?.title ?? 'Nothing playing'}</div>
				{#if autoplayTrack}
					<span
						class="shrink-0 text-muted-foreground"
						title="Playing similar music (Autoplay)"
						in:fade={{ duration: 200 }}
					>
						<HugeiconsIcon icon={InfinityIcon} class="h-3.5 w-3.5" />
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
					<Button variant="ghost" size="icon-sm" onclick={toggleLike} aria-label="Like">
						<span
							class="inline-flex"
							class:animate-heart-pop={justLiked}
							onanimationend={() => (justLiked = false)}
						>
							<HugeiconsIcon
								icon={FavouriteIcon}
								class="h-4 w-4 {playback.liked ? 'fill-current text-primary' : 'text-muted-foreground'}"
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
						aria-label="Add to playlist"
					>
						<HugeiconsIcon icon={Add01Icon} class="h-4 w-4 text-muted-foreground" />
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
				aria-label="Shuffle"
				aria-pressed={shuffleOn}
			>
				<HugeiconsIcon
					icon={ShuffleIcon}
					class="h-4 w-4 {shuffleOn ? 'text-primary' : 'text-muted-foreground'}"
				/>
			</Button>
			<Button variant="ghost" size="icon-sm" onclick={() => api.prevTrack()} aria-label="Previous">
				<HugeiconsIcon icon={PreviousIcon} class="h-5 w-5" />
			</Button>
			<Button
				variant="default"
				size="icon"
				class="rounded-full"
				onclick={() => api.togglePause()}
				aria-label="Play/pause"
			>
				<!-- HugeiconsIcon only re-renders `altIcon`/`showAlt`, not `icon` (frozen at mount) —
			     so toggle via showAlt, not a ternary on `icon`. -->
			<HugeiconsIcon
				icon={PauseIcon}
				altIcon={PlayIcon}
				showAlt={playback.paused}
				class="h-5 w-5"
			/>
			</Button>
			<Button variant="ghost" size="icon-sm" onclick={() => api.nextTrack()} aria-label="Next">
				<HugeiconsIcon icon={NextIcon} class="h-5 w-5" />
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={cycleRepeat}
				aria-label="Repeat: {repeat}"
				aria-pressed={repeat !== 'off'}
			>
				<!-- icon swap via altIcon/showAlt — `icon` is frozen at mount (see play/pause above) -->
				<HugeiconsIcon
					icon={RepeatIcon}
					altIcon={RepeatOne01Icon}
					showAlt={repeat === 'one'}
					class="h-4 w-4 {repeat !== 'off' ? 'text-primary' : 'text-muted-foreground'}"
				/>
			</Button>
		</div>
		<div class="flex w-full max-w-md items-center gap-2 text-xs text-muted-foreground">
			<span class="tabular-nums">{fmt(shownPosition)}</span>
			<input
				type="range"
				class="range flex-1"
				style="--pct:{playback.duration ? (shownPosition / playback.duration) * 100 : 0}%"
				min="0"
				max={playback.duration || 0}
				value={shownPosition}
				oninput={onSeekInput}
				onchange={onSeekCommit}
				aria-label="Seek"
			/>
			<span class="tabular-nums">{fmt(playback.duration)}</span>
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
				aria-label={playback.volume === 0 ? 'Unmute' : 'Mute'}
			>
				<!-- icon swap via altIcon/showAlt — `icon` is frozen at mount (see play/pause above) -->
				<HugeiconsIcon
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
				aria-label="Volume"
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
				<HugeiconsIcon icon={Moon01Icon} class="h-5 w-5" />
				{#if sleepTimer.mode !== 'off'}
					<span class="ml-0.5 text-[10px] font-medium tabular-nums">
						{sleepTimer.mode === 'minutes' ? fmt(sleepTimer.remaining) : '♪'}
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
					class="fixed z-50 min-w-44 animate-in rounded-xl border-transparent glass-strong p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95 {openUp
						? 'origin-bottom-right'
						: 'origin-top-right'}"
					style="right:{mx}px; {openUp ? 'bottom' : 'top'}:{my}px;"
					{@attach toBody}
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
				<HugeiconsIcon icon={MinimizeScreenIcon} class="h-5 w-5" />
			</Button>
			<!-- Pop the always-on-top floating player out (Rust floating.rs): the same playback
			     stream, as a portable glass card that stays on top of other apps. -->
			<Button variant="ghost" size="icon-sm" onclick={() => api.toggleFloating()} aria-label="Floating player">
				<HugeiconsIcon icon={MaximizeScreenIcon} class="h-5 w-5" />
			</Button>
			<Button
				variant={lyricsOpen ? 'secondary' : 'ghost'}
				size="icon-sm"
				onclick={onToggleLyrics}
				aria-label="Toggle lyrics"
			>
				<HugeiconsIcon icon={Mic01Icon} class="h-5 w-5" />
			</Button>
			<Button
				variant={queueOpen ? 'secondary' : 'ghost'}
				size="icon-sm"
				onclick={onToggleQueue}
				aria-label="Toggle queue"
			>
				<HugeiconsIcon icon={Queue01Icon} class="h-5 w-5" />
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
				<!-- icon swap via altIcon/showAlt — `icon` is frozen at mount (see play/pause above) -->
				<HugeiconsIcon
					icon={ArrowUp01Icon}
					altIcon={ArrowDown01Icon}
					showAlt={np.open}
					class="h-5 w-5"
				/>
			</Button>
		</div>
	</div>
</footer>
