<script lang="ts">
	import { fade, fly, scale } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { beforeNavigate } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Maximize01Icon,
		Minimize01Icon,
		Mic01Icon,
		MusicNote01Icon,
		PauseIcon,
		PlayIcon,
		Queue01Icon,
		PreviousIcon,
		NextIcon,
		ShuffleIcon,
		RepeatIcon,
		RepeatOne01Icon,
		VolumeHighIcon,
		VolumeMute02Icon,
		FavouriteIcon
	} from '@hugeicons/core-free-icons';
	import * as Tabs from '$lib/components/ui/tabs';
	import {
		dragVolume,
		commitVolume,
		np,
		playback,
		wheelVolume,
		toggleNowPlayingLike,
		cycleRepeat
	} from '$lib/player.svelte';
	import { appearance, layout } from '$lib/theme.svelte';
	import { thumb, thumbHQ } from '$lib/thumb';
	import * as api from '$lib/api';
	import QueueList from './QueueList.svelte';
	import TrackMenu from './TrackMenu.svelte';
	import LyricsView from './LyricsView.svelte';
	import { setAppearance } from '$lib/theme.svelte';

	// Off in settings, this view drops its tabs and the queue/lyrics panels stay in charge of both
	// (see +layout): they paint above this (z-30 over z-20), so all this needs is to hand back the
	// width they take at lg+ instead of letting them cover a third of the artwork. Below lg they're
	// a scrimmed overlay and there's nothing to shrink into. In tabbed mode both are always closed.
	let { queueOpen, lyricsOpen }: { queueOpen: boolean; lyricsOpen: boolean } = $props();

	/** Seconds → "m:ss" for the canopy transport readout. */
	function fmt(s: number): string {
		if (!s || Number.isNaN(s)) return '0:00';
		const total = Math.max(0, Math.floor(s));
		return `${Math.floor(total / 60)}:${String(total % 60).padStart(2, '0')}`;
	}
	const tabbed = $derived(appearance.tabbedPlayer);
	// ponytail: mirrors QueuePanel / LyricsPanel's w-80, keep in sync if those change.
	const panels = $derived(Number(queueOpen) + Number(lyricsOpen));
	const inset = $derived(['', 'lg:right-80', 'lg:right-[40rem]'][panels]);

	// Going somewhere means the user wants that page, not this one: minimise. The player bar brings
	// it back. beforeNavigate (not a pathname effect) so clicking the tab you're already on counts.
	beforeNavigate(() => (np.open = false));

	// The queue item matching what's actually playing — the track menu's song. Null when
	// the queue and the player disagree about what is playing (mid-skip), same guard as PlayerBar.
	const currentSong = $derived.by(() => {
		const cur = playback.queue.items[playback.queue.currentIndex];
		return cur?.video_id === playback.now?.videoId ? cur : null;
	});

	// Enlarged lyrics take the whole view, artwork column and tab strip included. A class swap
	// rather than unmounting the tabs: LyricsView must survive it or it refetches and loses its
	// scroll position.
	let big = $state(false);
	$effect(() => {
		if (np.tab !== 'lyrics') big = false; // nothing to enlarge on the queue tab
	});

	// Google's CDN doesn't serve every rewritten size for every image (see MediaCard), and at this
	// size a broken-image glyph *is* the page. So step down until one loads: crisp, then the size
	// proven everywhere else in the app, then the 120 the player bar is already showing for this
	// very track, and only then a music note.
	let attempt = $state(0);
	let bgFailed = $state(false);
	$effect(() => {
		playback.now?.thumbnail; // re-arm on every track change
		attempt = 0;
		bgFailed = false;
	});
	// Aurora round: the hero art now tries full-res first â€” the 1080 token, `maxresdefault`
	// on i.ytimg variants â€” then steps down through the sizes proven everywhere else (720 â†’
	// 400 â†’ the 120 the player bar already has loaded). Consecutive duplicates are dropped so
	// an onerror can't land on the same URL twice and stall the step-down.
	const srcs = $derived(
		[thumbHQ(playback.now?.thumbnail), ...[720, 400, 120].map((px) => thumb(playback.now?.thumbnail, px))].filter(
			(u, i, a) => u && u !== a[i - 1]
		) as string[]
	);
	const src = $derived(srcs[attempt]);

	// iTunes full-res artwork (Rust art.rs): a genuine 100000x100000-999 cover for the hero,
	// swapped in over the cascade when the lookup lands. Guarded against track-change races;
	// if it fails to load it simply drops back to the cascade source.
	let itunesArt = $state<string | null>(null);
	$effect(() => {
		const now = playback.now;
		itunesArt = null;
		if (!now || !now.artists || api.isLocalId(now.videoId)) return;
		const vid = now.videoId;
		api
			.getHighresArt(now.artists, now.title)
			.then((url) => {
				if (url && vid === playback.now?.videoId) itunesArt = url;
			})
			.catch(() => {});
	});
	const heroSrc = $derived(itunesArt ?? src);
	const imgFailed = () => attempt++;
	function handleHeroError() {
		if (itunesArt) {
			itunesArt = null; // fall back to the cascade; a failure there steps down via imgFailed
		} else {
			imgFailed();
		}
	}

	// Spotify Canvas (#8): looping video when available, palette gradient fallback.
	let canvasUrl = $state<string | null>(null);
	$effect(() => {
		const now = playback.now;
		canvasUrl = null;
		if (!now || !now.artists || api.isLocalId(now.videoId)) return;
		const vid = now.videoId;
		api
			.getCanvas(now.artists, now.title)
			.then((url) => {
				if (url && vid === playback.now?.videoId) canvasUrl = url;
			})
			.catch(() => {});
	});

	// Clicking the artwork toggles playback, and flashes the action just taken over it so the click
	// visibly did something. Read `paused` before the toggle: the backend event that flips it is a
	// round trip away, and the icon has to be right on the frame the user clicked.
	let flash: 'play' | 'pause' | null = $state(null);
	let flashTimer: ReturnType<typeof setTimeout>;
	function toggle() {
		flash = playback.paused ? 'play' : 'pause';
		clearTimeout(flashTimer);
		flashTimer = setTimeout(() => (flash = null), 220);
		api.togglePause();
	}

	// Wheel over the cover art in the maximized view steps the volume (same 5%/notch as the
	// player bar) and pops a % badge over the artwork. wheelVolume reuses nudgeVolume, so the
	// level persists once the gesture stops. The badge clears ~1s after the last notch.
	// Hold Shift for precise 1% steps.
	let volBadge = $state<number | null>(null);
	let volBadgeTimer: ReturnType<typeof setTimeout> | undefined;
	function onMaxWheel(e: WheelEvent) {
		wheelVolume(e);
		volBadge = playback.volume;
		clearTimeout(volBadgeTimer);
		volBadgeTimer = setTimeout(() => (volBadge = null), 900);
	}
</script>

<!-- Covers the page but not the sidebar (you navigate away to minimise) and not the player bar,
     which stays in charge of transport and paints above this on the way in and out.
     z-20 matches the highest a page uses for its own chrome (home's sticky mood chips) and wins the
     tie on DOM order, since <main> is static and its z-indexes land in the same stacking context.
     The player bar and the queue/lyrics panels come later/higher, so they still paint above.
     ponytail: left offsets mirror Sidebar's w-16/lg:w-60 â€” keep in sync if those change. -->
<div
	transition:fly={{ y: '100%', duration: 320, easing: cubicOut }}
	class="np-view absolute inset-0 z-20 flex flex-col overflow-hidden bg-background {inset}"
>
	<!-- Orchard takeover: the view owns the whole row in every layout. Canopy has no bottom bar
	     (its transport lives in the top bar), so this view carries its own footer there; in the
	     other layouts the player bar below the row stays the transport and no footer renders. -->
	<div class="relative flex min-h-0 flex-1 justify-center px-4 py-4 sm:px-6 sm:py-6">
	<!-- The artwork itself, blurred to a wash, is the background: same trick as HomeHero, and it
	     needs no colour extraction (which a remote image would taint the canvas for anyway). The
	     120px variant is the one the player bar has already loaded for this track, so this costs
	     no request and nothing new to decode. Ambient mode (#7) renders no layer here — the
	     app-wide backdrop in +layout.svelte owns that; only this view's own wash is local.
	     Two opacities because the wash sits on opposite grounds: over white it has to stay pale
	     enough for dark text, over near-black it can carry more colour before muted-foreground
	     stops reading. Turn them up together if it's too subtle. -->
	{#if appearance.artworkBackground && srcs[2] && !bgFailed}
		<img
			src={srcs[2]}
			alt=""
			onerror={() => (bgFailed = true)}
			class="pointer-events-none absolute inset-0 h-full w-full scale-110 object-cover opacity-30 blur-2xl dark:opacity-40"
		/>
	{/if}

	<!-- Capped and centred, so a wide window doesn't park the artwork in the middle of an empty half
	     with the tabs glued to the right edge. --art is the artwork's side: whichever is smaller of
	     the column's width and the height left over once the titlebar, the player bar and this
	     padding have had theirs, at 75% so the square doesn't dominate the view.
	     ponytail: 11rem is those three measured, not computed. The 0.75 leaves it plenty of slack
	     now, so only a much taller player bar would need it raised. -->
	<div
		class="relative flex w-full max-w-[80rem] gap-6 xl:gap-10"
		style="--art:calc(min(100%,100vh - 11rem) * 0.75)"
	>
		{#if !big}
			<!-- Centred against the full height of the column on the right. Below md there isn't room
			     for both columns, and the queue wins. Untabbed there is no second column, so the
			     artwork is the whole view at every width. -->
			<div
				class="min-w-0 flex-1 items-center justify-center {tabbed ? 'hidden md:flex' : 'flex'}"
			>
		<!-- A div, not a button: it is the [data-ctx] host, so right-clicking anywhere on the
		     artwork opens the track menu at the pointer (ctxHost on the hidden TrackMenu). -->
			<div class="relative w-full max-w-[var(--art)]" data-ctx>
				{#if canvasUrl}
					<!-- Spotify Canvas (#8): looping video, muted autoplay, palette gradient fallback -->
					<div class="relative aspect-square w-full overflow-hidden rounded-3xl shadow-2xl glass">
						<video
							src={canvasUrl}
							autoplay
							muted
							loop
							playsinline
							onerror={() => (canvasUrl = null)}
							class="absolute inset-0 h-full w-full object-cover"
						></video>
						{#if heroSrc}
							<img src={heroSrc} alt="" class="absolute inset-0 h-full w-full object-cover opacity-30 mix-blend-overlay" aria-hidden="true" />
						{/if}
						<div class="pointer-events-none absolute inset-0 bg-gradient-to-br from-primary/10 via-transparent to-accent/10"></div>
						<button
							type="button"
							class="absolute inset-0 cursor-pointer bg-transparent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
							onclick={(e) => { e.stopPropagation(); toggle(); }}
							onwheel={onMaxWheel}
							aria-label={playback.paused ? 'Play' : 'Pause'}
						></button>
						<div class="pointer-events-none absolute bottom-2 left-2 rounded-full glass-strong px-2 py-0.5 text-[10px] font-bold tracking-wide text-white">CANVAS</div>
						{#if flash}
							<div in:scale={{ start: 0.7, duration: 150, easing: cubicOut }} out:scale={{ start: 1.3, duration: 320, easing: cubicOut }} class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center">
								<div class="rounded-full bg-black/55 p-3.5 text-white">
									<HugeiconsIcon icon={PauseIcon} altIcon={PlayIcon} showAlt={flash === 'play'} class="h-7 w-7" />
								</div>
							</div>
						{/if}
					</div>
				{:else}
				<button
					type="button"
					class="relative block w-full max-w-[var(--art)] cursor-pointer rounded-2xl bg-transparent p-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary"
					onclick={(e) => {
						e.stopPropagation();
						toggle();
					}}
					onwheel={onMaxWheel}
					aria-label={playback.paused ? 'Play' : 'Pause'}
					title={playback.paused ? 'Play' : 'Pause'}
				>
					{#if flash}
						<!-- No backdrop-blur: re-blurring the plate on every frame of the scale is what made
						     this stutter on WebKitGTK. Transform and opacity only. -->
						<div
							in:scale={{ start: 0.7, duration: 150, easing: cubicOut }}
							out:scale={{ start: 1.3, duration: 320, easing: cubicOut }}
							class="pointer-events-none absolute inset-0 z-10 flex items-center justify-center"
						>
							<div class="rounded-full bg-black/55 p-3.5 text-white">
								<!-- icon is frozen at mount, so swap via showAlt, not a ternary. -->
								<HugeiconsIcon
									icon={PauseIcon}
									altIcon={PlayIcon}
									showAlt={flash === 'play'}
									class="h-7 w-7"
								/>
							</div>
						</div>
					{/if}
					{#if heroSrc && attempt < srcs.length}
						<img
							src={heroSrc}
							alt=""
							onerror={handleHeroError}
							class="aspect-square w-full rounded-3xl object-cover shadow-2xl"
						/>
					{:else}
						<div
							class="flex aspect-square w-full items-center justify-center rounded-2xl bg-muted text-muted-foreground/40"
						>
							<HugeiconsIcon icon={MusicNote01Icon} class="h-16 w-16" />
						</div>
					{/if}
				</button>
				{/if}
				{#if volBadge !== null}
					<span
						class="pointer-events-none absolute top-2 left-2 z-10 rounded-lg glass-strong px-1.5 py-0.5 text-[11px] font-bold text-white shadow-lg tabular-nums"
						transition:fade={{ duration: 150 }}
						aria-hidden="true"
					>
						{volBadge}%
					</span>
				{/if}
				{#if currentSong}
					<!-- Menu with no ⋯ of its own: the cover already carries play/pause, and this view has
					     nowhere sensible to put another button. It exists for the right-click.
					     ponytail: `hidden` on the trigger rather than a no-trigger prop — display:none
					     keeps it out of the layout and the a11y tree, and ctxHost still finds the host. -->
					<TrackMenu song={currentSong} triggerClass="hidden" />
				{/if}
			</div>
			</div>
		{/if}

		{#if tabbed}
			<div class="flex min-h-0 flex-col {big ? 'flex-1' : 'w-full md:w-[22rem] xl:w-[26rem]'}">
				<Tabs.Root
				value={np.tab}
				onValueChange={(v) => (np.tab = v as typeof np.tab)}
				class="min-h-0 flex-1"
			>
				<div class="flex items-center gap-2 {big ? 'justify-end' : ''}">
					<!-- Same two glyphs the player bar uses for the queue and lyrics buttons. -->
					<Tabs.List class={big ? 'hidden' : 'flex-1'}>
						<Tabs.Trigger value="queue" class="gap-2.5">
							<HugeiconsIcon icon={Queue01Icon} class="h-4 w-4" /> Queue
						</Tabs.Trigger>
						<Tabs.Trigger value="lyrics" class="gap-2.5">
							<HugeiconsIcon icon={Mic01Icon} class="h-4 w-4" /> Lyrics
						</Tabs.Trigger>
					</Tabs.List>
					{#if np.tab === 'lyrics'}
						<button
							onclick={() => (big = !big)}
							class="cursor-pointer rounded-md p-1.5 text-muted-foreground transition-colors hover:text-foreground"
							aria-label={big ? 'Shrink lyrics' : 'Enlarge lyrics'}
						>
							<!-- icon swap via altIcon/showAlt: `icon` is frozen at mount -->
							<HugeiconsIcon
								icon={Maximize01Icon}
								altIcon={Minimize01Icon}
								showAlt={big}
								class="h-4 w-4"
							/>
						</button>
					{/if}
				</div>
				<!-- Only the open tab is mounted: bits-ui keeps inactive content in the DOM, which would
				     leave LyricsView fetching lyrics for every track you never asked to see. -->
				{#if np.tab === 'queue'}
					<Tabs.Content value="queue" class="flex min-h-0 flex-col">
						<QueueList />
					</Tabs.Content>
				{:else}
					<Tabs.Content value="lyrics" class="flex min-h-0 flex-col">
						<LyricsView expanded={big} />
					</Tabs.Content>
				{/if}
			</Tabs.Root>
			</div>
		{/if}
	</div>
	</div>

	<!-- Canopy-only transport: this layout unmounts the bottom player bar, so the takeover view
	     carries its own. Every other layout keeps the bar below the row and renders no footer. -->
	{#if layout.id === 'canopy'}
		<footer class="relative shrink-0 border-t px-6 pb-4 pt-3">
			<div class="mx-auto flex max-w-[80rem] flex-col gap-2">
				<!-- Seek line -->
				<div class="flex items-center gap-3">
					<span class="w-12 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
						{fmt(playback.position)}
					</span>
					<input
						type="range"
						min="0"
						max={Math.max(1, Math.floor(playback.duration))}
						value={Math.min(playback.position, playback.duration)}
						oninput={(e) => api.seek(Number(e.currentTarget.value)).catch(() => {})}
						class="h-1 flex-1 accent-primary"
						aria-label="Seek"
					/>
					<span class="w-12 shrink-0 text-xs tabular-nums text-muted-foreground">
						{fmt(playback.duration)}
					</span>
				</div>
				<!-- Controls -->
				<div class="flex items-center justify-center gap-2">
					<button
						class="flex h-9 w-9 items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground {playback.liked
							? 'text-primary'
							: ''}"
						onclick={() => toggleNowPlayingLike()}
						aria-label="Like"
					>
						<HugeiconsIcon icon={FavouriteIcon} class="h-4 w-4" />
					</button>
					<button
						class="flex h-9 w-9 items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground {playback.queue.shuffle
							? 'text-primary'
							: ''}"
						onclick={() => api.toggleShuffle().catch(() => {})}
						aria-label="Shuffle"
					>
						<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" />
					</button>
					<button
						class="flex h-10 w-10 items-center justify-center rounded-md text-foreground/90 transition-colors hover:bg-accent/10"
						onclick={() => api.prevTrack().catch(() => {})}
						aria-label="Previous"
					>
						<HugeiconsIcon icon={PreviousIcon} class="h-5 w-5" />
					</button>
					<button
						class="flex h-12 w-12 items-center justify-center rounded-full bg-primary text-primary-foreground shadow transition-transform hover:scale-105"
						onclick={() => api.togglePause().catch(() => {})}
						aria-label={playback.paused ? 'Play' : 'Pause'}
					>
						{#if playback.paused}
							<HugeiconsIcon icon={PlayIcon} class="h-6 w-6" />
						{:else}
							<HugeiconsIcon icon={PauseIcon} class="h-6 w-6" />
						{/if}
					</button>
					<button
						class="flex h-10 w-10 items-center justify-center rounded-md text-foreground/90 transition-colors hover:bg-accent/10"
						onclick={() => api.nextTrack().catch(() => {})}
						aria-label="Next"
					>
						<HugeiconsIcon icon={NextIcon} class="h-5 w-5" />
					</button>
					<button
						class="flex h-9 w-9 items-center justify-center rounded-md text-muted-foreground transition-colors hover:text-foreground {playback.queue.repeat !==
						'off'
							? 'text-primary'
							: ''}"
						onclick={() => cycleRepeat().catch(() => {})}
						aria-label="Repeat"
					>
						<HugeiconsIcon
							icon={playback.queue.repeat === 'one' ? RepeatOne01Icon : RepeatIcon}
							class="h-4 w-4"
						/>
					</button>
					<div class="ml-6 hidden items-center gap-2 md:flex">
						<button
							class="p-1 text-muted-foreground transition-colors hover:text-foreground"
							onclick={() => commitVolume(playback.volume === 0 ? 100 : 0)}
							aria-label="Mute"
						>
							<HugeiconsIcon
								icon={playback.volume === 0 ? VolumeMute02Icon : VolumeHighIcon}
								class="h-4 w-4"
							/>
						</button>
						<input
							type="range"
							min="0"
							max="100"
							value={playback.volume}
							oninput={(e) => dragVolume(Number(e.currentTarget.value))}
							onchange={(e) => commitVolume(Number(e.currentTarget.value))}
							class="w-28 accent-primary"
							aria-label="Volume"
						/>
					</div>
				</div>
			</div>
		</footer>
	{/if}
</div>
