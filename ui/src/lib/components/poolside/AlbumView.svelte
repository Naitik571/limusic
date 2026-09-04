<script lang="ts">
	// Poolside album detail — top: cover + title + play button. Below: full track list,
	// scrollable, each row tappable to play. At the very top: the coverflow of the
	// library's other albums (so you can flip through without going back), with the
	// current album pinned in the centre.
	//
	// Z-stack (top -> bottom in document order):
	//   .ps-albumview
	//     .ps-alb-back  (back button, top-left, fixed)
	//     .ps-alb-cover (cover + title + meta + play button, in a row)
	//     .ps-alb-tracks (the actual track list — this is what was missing before)
	//     .ps-alb-coverflow (horizontal carousel of other albums, bottom)
	import { Spring } from 'svelte/motion';
	import { untrack } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ArrowLeft01Icon, PlayIcon, PauseIcon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { BrowseItem, SongItem } from '$lib/api';
	import { playback, toast } from '$lib/player.svelte';
	import Vinyl from './Vinyl.svelte';

	let {
		albums,
		album,
		artFor,
		onBack,
		onSelect,
		onPlayAlbum,
		onOpenCustom
	}: {
		albums: BrowseItem[];
		album: BrowseItem;
		artFor: (item: BrowseItem) => string;
		onBack: () => void;
		onSelect: (item: BrowseItem) => void;
		onPlayAlbum: (item: BrowseItem) => void;
		onOpenCustom: () => void;
	} = $props();

	// Track list state. Loading is keyed by album.id so navigating between albums
	// always cancels the in-flight load and starts fresh (no stale skeletons, no
	// bleeding in of the previous album's tracks).
	let tracks = $state<SongItem[]>([]);
	let tracksLoading = $state(false);
	let tracksError = $state<string | null>(null);
	let loadedFor = $state<string | null>(null);

	async function loadTracks(albumId: string) {
		loadedFor = albumId;
		tracksLoading = true;
		tracksError = null;
		tracks = [];
		try {
			if (albumId.startsWith('LOCALALBUM:')) {
				const albumName = albumId.slice('LOCALALBUM:'.length);
				const m = await import('$lib/player.svelte');
				const list = m.local.songs.filter((s) => (s.album || '').trim() === albumName);
				// Guard: only commit if the user is still on this album when the await resolves.
				if (loadedFor === albumId) {
					tracks = list;
				}
			} else {
				// Liked Music, playlists, and remote albums all funnel through getPlaylist,
				// which returns a PlaylistPage with `items`. getAlbum also works for
				// MPRE… albums but is a separate codepath; for the library's "click an
				// item" use-case getPlaylist is the universal loader.
				const a = await api.getPlaylist(albumId);
				if (loadedFor === albumId) {
					tracks = a.items ?? [];
				}
			}
		} catch (e) {
			if (loadedFor === albumId || loadedFor === null) {
				tracksError = e instanceof Error ? e.message : String(e);
				loadedFor = albumId;
			}
		} finally {
			if (loadedFor === albumId || loadedFor === null) {
				tracksLoading = false;
			}
		}
	}

	// One effect, no state writes inside it. Keyed on album.id + the loadedFor guard
	// so the user can navigate A -> B -> A and each transition kicks off its own load.
	$effect(() => {
		const id = album.id;
		// Mark "this is the album we want" before the async work, but write through
		// untrack so Svelte doesn't re-run this effect when loadedFor flips.
		untrack(() => { loadedFor = id; });
		loadTracks(id);
	});

	// Reactive: is this album currently playing? Used to flip the play button to
	// PAUSE and to highlight the playing track in the list.
	const isPlayingThis = $derived(
		!!playback.now?.videoId &&
		tracks.some((t) => t.video_id === playback.now?.videoId)
	);
	const playingIndex = $derived.by(() => {
		if (!isPlayingThis) return -1;
		const vid = playback.now?.videoId;
		return tracks.findIndex((t) => t.video_id === vid);
	});

	async function playTrack(track: SongItem, i: number) {
		try {
			// Play from this album as the playlist context. The Rust backend will build
			// the queue from this playlist starting at index i.
			const m = await import('$lib/player.svelte');
			m.playFrom(album, tracks, i);
		} catch (e) {
			toast.error(String(e));
		}
	}

	// ====================================================================
	// Coverflow (kept from before — the album carousel at the bottom)
	// ====================================================================
	const CENTER_GAP = 150;
	const STACK_SPACING = 92;
	const ROTATION = 34;
	const n = $derived(albums.length);
	const scroll = new Spring(0, { stiffness: 150, damping: 30 });
	$effect(() => {
		const i = albums.findIndex((a) => a.id === album.id);
		if (i >= 0) scroll.target = i;
	});

	// idle drift
	let lastTouch = Date.now();
	const IDLE_MS = 5000;
	const IDLE_STEP_MS = 2200;
	let idleTimer: ReturnType<typeof setTimeout> | null = null;
	function bumpTouch() { lastTouch = Date.now(); }
	$effect(() => {
		void scroll.current;
		if (idleTimer) clearTimeout(idleTimer);
		idleTimer = setTimeout(function tick() {
			if (Date.now() - lastTouch > IDLE_MS && n > 1) {
				scroll.target = (Math.round(scroll.current) + 1) % n;
			}
			idleTimer = setTimeout(tick, IDLE_STEP_MS);
		}, IDLE_STEP_MS);
		return () => { if (idleTimer) { clearTimeout(idleTimer); idleTimer = null; } };
	});

	const activeIdx = $derived(Math.round(scroll.current));

	const cards = $derived.by(() => {
		void scroll.current;
		return albums.map((a, i) => {
			const pos = i - scroll.current;
			const abs = Math.abs(pos);
			const rotateY = abs < 0.5 ? -pos * (ROTATION * 2) : pos < 0 ? ROTATION : -ROTATION;
			const x =
				abs < 1 ? pos * CENTER_GAP : (pos < 0 ? -1 : 1) * (CENTER_GAP + (abs - 1) * STACK_SPACING);
			const z = abs > 0.5 ? -200 : -abs * 400;
			const zi = 1000 - Math.round(abs * 10);
			const brightness = abs < 0.5 ? 1 : 0.5;
			return { a, i, x, rotateY, z, zi, brightness, abs };
		});
	});

	let dragging = $state(false);
	let dragStartX = 0;
	let dragStartScroll = 0;
	let moved = $state(false);
	let tip = $state('');
	let tipX = $state(0);
	let tipY = $state(0);
	const clamp = (v: number) => Math.max(0, Math.min(n - 1, v));

	function onPointerDown(e: PointerEvent) {
		dragging = true;
		moved = false;
		dragStartX = e.clientX;
		dragStartScroll = scroll.target;
		bumpTouch();
	}
	function onPointerMove(e: PointerEvent) {
		if (!dragging) return;
		const dx = e.clientX - dragStartX;
		if (Math.abs(dx) > 6) moved = true;
		scroll.target = clamp(dragStartScroll - dx / 140);
		if (moved) bumpTouch();
	}
	function onPointerUp() {
		if (!dragging) return;
		dragging = false;
		scroll.target = clamp(Math.round(scroll.current));
	}
	function onWheel(e: WheelEvent) {
		e.preventDefault();
		scroll.target = clamp(scroll.target + e.deltaY * 0.0022);
		bumpTouch();
	}
	function onCardClick(i: number) {
		if (moved) return;
		if (i === activeIdx) onPlayAlbum(albums[i]);
		else {
			scroll.target = i;
			onSelect(albums[i]);
		}
	}
	function onKeyDown(e: KeyboardEvent) {
		if (e.key === 'ArrowLeft') {
			e.preventDefault();
			const i = clamp(activeIdx - 1);
			scroll.target = i; onSelect(albums[i]); bumpTouch();
		} else if (e.key === 'ArrowRight') {
			e.preventDefault();
			const i = clamp(activeIdx + 1);
			scroll.target = i; onSelect(albums[i]); bumpTouch();
		} else if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onPlayAlbum(albums[activeIdx]);
		}
	}

	function fmtDur(s: string | undefined): string {
		return s || '';
	}
</script>

<div class="ps-albumview">
	<button class="ps-back ps-glass" onclick={onBack} title="Back to library" aria-label="Back to library">
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" width="18"><path d="M15 5l-7 7 7 7" /></svg>
	</button>

	<!-- Hero: cover + meta + play button, all in a clean vertical flow -->
	<div class="ps-alb-hero">
		<div class="ps-alb-cover">
			<img decoding="async" src={artFor(album)} alt={album.title} />
			{#if album.id.startsWith('LOCALALBUM:')}
				<div class="ps-alb-cover-badge">LOCAL</div>
			{/if}
		</div>
		<div class="ps-alb-meta">
			<span class="ps-alb-kind">{album.kind === 'artist' ? 'ARTIST' : 'ALBUM'}</span>
			<h2 class="ps-alb-title">{album.title}</h2>
			{#if album.subtitle}
				<p class="ps-alb-artist">{album.subtitle}</p>
			{/if}
			<div class="ps-alb-actions">
				<button class="ps-aqua ps-alb-play" onclick={() => onPlayAlbum(album)} aria-label="Play album">
					<HugeiconsIcon icon={isPlayingThis && !playback.paused ? PauseIcon : PlayIcon} class="w-3.5 h-3.5" />
					{isPlayingThis && !playback.paused ? 'PAUSE' : 'PLAY ALBUM'}
				</button>
				<button class="ps-ghost" onclick={onOpenCustom}>Add custom CD cover</button>
			</div>
		</div>
	</div>

	<!-- Track list — the missing piece. Scrollable, rows are 56px tall with a hover
	     aqua bar (see .ps-songrow::before). Currently-playing track gets a glow. -->
	<div class="ps-alb-tracks">
		<h3 class="ps-section-title">TRACKS</h3>
		{#if tracksLoading}
			<div class="ps-alb-track ps-alb-track--skeleton">
				<span class="ps-alb-track-n"><span class="ps-skeleton" style="height: 10px; width: 18px; display: block;"></span></span>
				<span class="ps-alb-track-title"><span class="ps-skeleton" style="height: 12px; width: 50%; display: block;"></span></span>
				<span class="ps-alb-track-artist"><span class="ps-skeleton" style="height: 10px; width: 30%; display: block;"></span></span>
				<span class="ps-alb-track-dur"><span class="ps-skeleton" style="height: 10px; width: 24px; display: block;"></span></span>
			</div>
			<div class="ps-alb-track ps-alb-track--skeleton">
				<span class="ps-alb-track-n"><span class="ps-skeleton" style="height: 10px; width: 18px; display: block;"></span></span>
				<span class="ps-alb-track-title"><span class="ps-skeleton" style="height: 12px; width: 65%; display: block;"></span></span>
				<span class="ps-alb-track-artist"><span class="ps-skeleton" style="height: 10px; width: 22%; display: block;"></span></span>
				<span class="ps-alb-track-dur"><span class="ps-skeleton" style="height: 10px; width: 24px; display: block;"></span></span>
			</div>
			<div class="ps-alb-track ps-alb-track--skeleton">
				<span class="ps-alb-track-n"><span class="ps-skeleton" style="height: 10px; width: 18px; display: block;"></span></span>
				<span class="ps-alb-track-title"><span class="ps-skeleton" style="height: 12px; width: 40%; display: block;"></span></span>
				<span class="ps-alb-track-artist"><span class="ps-skeleton" style="height: 10px; width: 28%; display: block;"></span></span>
				<span class="ps-alb-track-dur"><span class="ps-skeleton" style="height: 10px; width: 24px; display: block;"></span></span>
			</div>
		{:else if tracksError}
			<div class="ps-alb-error">
				<strong>Couldn't load tracks.</strong>
				<span>{tracksError}</span>
				<button class="ps-ghost" onclick={() => loadTracks(album.id)}>Retry</button>
			</div>
		{:else if tracks.length === 0}
			<div class="ps-empty">This album has no tracks.</div>
		{:else}
			{#each tracks as t, i (t.video_id + i)}
				<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
				<div
					class="ps-alb-track {playingIndex === i ? 'is-playing' : ''}"
					role="button"
					tabindex="0"
					onclick={() => playTrack(t, i)}
					onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && playTrack(t, i)}
				>
					<span class="ps-alb-track-n">
						{#if playingIndex === i && !playback.paused}
							<span class="ps-alb-track-eq" aria-hidden="true">
								<i></i><i></i><i></i><i></i>
							</span>
						{:else}
							{String(i + 1).padStart(2, '0')}
						{/if}
					</span>
					<span class="ps-alb-track-title">{t.title}</span>
					<span class="ps-alb-track-artist">{t.artists ?? ''}</span>
					<span class="ps-alb-track-dur">{fmtDur(t.duration)}</span>
				</div>
			{/each}
		{/if}
	</div>

	<!-- Coverflow: horizontal carousel of OTHER albums from the library, with
	     reflections. Lives at the bottom so it doesn't compete with the hero. -->
	{#if albums.length > 1}
		<div class="ps-alb-more">
			<h3 class="ps-section-title">MORE IN LIBRARY</h3>
			<!-- svelte-ignore a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
			<div
				class="ps-fan"
				class:dragging
				onpointerdown={onPointerDown}
				onpointermove={onPointerMove}
				onpointerup={onPointerUp}
				onpointercancel={onPointerUp}
				onwheel={onWheel}
				onkeydown={onKeyDown}
				role="region"
				aria-label="Album coverflow — use left and right arrow keys to navigate"
				tabindex="0"
			>
				{#each cards as c (c.a.id)}
					<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
					<div
						class="ps-fcard {c.abs >= 0.5 ? 'dim' : ''}"
						style="
							transform: translate(-50%, -50%) translateX({c.x}px) translateZ({c.z}px) rotateY({c.rotateY}deg);
							z-index: {c.zi};
							filter: brightness({c.brightness});
							transition-delay: 0ms;
						"
						onclick={() => onCardClick(c.i)}
						onmousemove={(e) => {
							tip = c.abs < 0.5 ? `${c.a.title} / ${c.a.subtitle ?? ''}` : c.a.title;
							tipX = e.clientX + 16; tipY = e.clientY - 10;
						}}
						onmouseleave={() => (tip = '')}
					>
						<div class="cov"><img decoding="async" loading="lazy" src={artFor(c.a)} alt={c.a.title} draggable="false" /></div>
						{#if c.abs < 0.5}
							<div class="play-chip" title="Play this album">
								<HugeiconsIcon icon={PlayIcon} class="w-3.5 h-3.5" />
							</div>
						{/if}
						<div class="reflection" aria-hidden="true">
							<img decoding="async" loading="lazy" src={artFor(c.a)} alt="" draggable="false" />
						</div>
					</div>
				{/each}
			</div>
			{#if tip}<div class="ps-fan-tip" style="left:{tipX}px;top:{tipY}px;opacity:1">{tip}</div>{/if}
		</div>
	{/if}
</div>

<style>
	.ps-fan { touch-action: pan-y; }
	.ps-fan.dragging { cursor: grabbing; }
	.ps-fcard { will-change: transform, filter; }
	.ps-fcard .cov {
		position: absolute; inset: 0 0 auto 0;
		aspect-ratio: 1; border-radius: 16px; overflow: hidden;
		border: 1.5px solid rgba(255, 255, 255, 0.9);
	}
	.ps-fcard .cov img { width: 100%; height: 100%; object-fit: cover; display: block; }
	.ps-fcard .play-chip {
		position: absolute; right: 10px; bottom: 10px;
		width: 38px; height: 38px; border-radius: 50%;
		display: grid; place-items: center; color: #fff;
		background: linear-gradient(180deg, #8fdcf2, var(--ps-accent));
		border: 1px solid rgba(255, 255, 255, 0.8);
		box-shadow: 0 4px 12px rgba(8, 60, 70, 0.4);
	}
	.ps-fcard .reflection {
		position: absolute; left: 0; top: calc(100% + 1px);
		width: 100%; height: 42%;
		pointer-events: none; transform-origin: top center;
		transform: rotateX(12deg);
		will-change: transform;
		-webkit-mask-image: linear-gradient(180deg, rgba(0, 0, 0, 0.4), transparent 85%);
		mask-image: linear-gradient(180deg, rgba(0, 0, 0, 0.4), transparent 85%);
	}
	.ps-fcard .reflection img {
		width: 100%; aspect-ratio: 1; object-fit: cover; display: block;
		transform: scaleY(-1); opacity: 0.4;
	}
</style>
