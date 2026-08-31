<!--
  CoverFlowCarousel — 3D auto-scrolling album carousel for the Library.

  Visual model:
    - N album covers arranged along a shallow horizontal arc
    - The "focused" cover (centred) is largest, faces forward (rotateY 0), and sharp
    - Covers further to the left/right rotate away from the viewer, shrink, and dim
    - The arc auto-scrolls: covers slide in from the right, pass through the focus
      position, and slide out the left
    - Clicking a cover pauses the auto-scroll and focuses that cover
    - A floating MiniPlayerPill sits over the bottom-center, never moves while the
      carousel underneath scrolls

  Implementation: one big .ps-cf-track that translates horizontally via a CSS
  animation (or a JS spring on click). Each cover is a 3D-transformed child.
  Per-frame math: position from -N/2 to +N/2 covers a unit; rotateY is
  sin(distance) * MAX_ANGLE; scale is 1 - |distance|/DEPTH_FALLOFF; z-index
  bumps the centered cover.
-->
<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon, PauseIcon, StarIcon } from '@hugeicons/core-free-icons';
	import { Spring } from 'svelte/motion';
	import { onMount, untrack } from 'svelte';
	import type { BrowseItem } from '$lib/api';
	import { playback, toast } from '$lib/player.svelte';
	import MiniPlayerPill from './MiniPlayerPill.svelte';

	let {
		albums,
		artFor,
		onOpenAlbum,
		onPlayAlbum
	}: {
		albums: BrowseItem[];
		artFor: (item: BrowseItem) => string;
		onOpenAlbum: (item: BrowseItem) => void;
		onPlayAlbum: (item: BrowseItem) => void;
	} = $props();

	// Arc geometry. CARD_W and CARD_H are the actual rendered size of each cover;
	// the visual gap is half of CARD_W so covers overlap slightly and the rotation
	// reads as a smooth arc instead of a row.
	const CARD_W = 240;
	const CARD_H = 240;
	const GAP = 70; // px between adjacent cover centres
	const MAX_ANGLE = 38; // deg, at the far edges
	const DEPTH_FALLOFF = 5; // covers within this distance of centre are "in focus"
	const ARC_RADIUS = 600; // px, for the 3D perspective

	// We expose a single shared Spring target (`scrollIndex`) that the cover array
	// reads. Auto-scroll ticks it forward every AUTO_MS when the user hasn't touched
	// the carousel. Click/drag pauses auto-scroll, releases it after IDLE_MS of silence.
	const scrollIndex = new Spring(0, { stiffness: 60, damping: 18 });
	let pausedAuto = $state(false);
	let lastTouch = Date.now();
	const IDLE_MS = 4500;
	const AUTO_MS = 1800;
	let lastTick = Date.now();

	function bumpTouch() {
		lastTouch = Date.now();
		pausedAuto = true;
	}

	$effect(() => {
		// Track-driven: also keep scrolling if a new album joins the library.
		// We untrack() the wall-clock so this effect only re-runs when the user
		// interacts (click/drag) or the album list changes meaningfully.
		untrack(() => {
			// subscribe to lastTouch so the effect re-runs on user input
			void lastTouch;
			const now = Date.now();
			const dt = now - lastTick;
			lastTick = now;
			if (pausedAuto || albums.length === 0) return;
			if (now - lastTouch < IDLE_MS) return;
			// one tick per AUTO_MS
			if (dt < AUTO_MS) return;
			const i = Math.round(scrollIndex.current);
			scrollIndex.target = (i + 1) % Math.max(1, albums.length);
		});
	});

	// rAF loop for the auto-scroll timer (the Spring does the actual easing).
	let raf = 0;
	onMount(() => {
		const tick = () => {
			if (!pausedAuto && Date.now() - lastTouch > IDLE_MS) {
				// nudge the target so the spring smoothly advances
				const i = Math.round(scrollIndex.current);
				const n = Math.max(1, albums.length);
				scrollIndex.target = (i + 1) % n;
			}
			raf = requestAnimationFrame(tick);
		};
		raf = requestAnimationFrame(tick);
		return () => cancelAnimationFrame(raf);
	});

	function clickAt(i: number) {
		bumpTouch();
		scrollIndex.target = i;
		// resume auto-scroll after a longer pause (so single-click is a deliberate pause)
		setTimeout(() => {
			lastTouch = Date.now() - IDLE_MS + 1500; // 1.5s of "still" before resume
		}, 200);
	}

	function playAt(i: number, e: MouseEvent) {
		e.stopPropagation();
		const a = albums[i];
		if (!a) return;
		toast.info(`Playing ${a.title}`);
		onPlayAlbum(a);
	}

	function togglePause() {
		if (!playback.now) return;
		import('$lib/api').then((api) => api.togglePause().catch(() => {}));
	}

	function openNow() {
		// The pill's onclick bubbles here too; this is a no-op unless the pill is unmounted.
	}
</script>

<div class="ps-cf-stage">
	<!-- back button in the top-left -->
	<button class="ps-cf-back" onclick={() => history.length > 1 ? history.back() : null} aria-label="Back">
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" width="22">
			<path d="M15 5l-7 7 7 7" />
		</svg>
	</button>

	<!-- header label -->
	<div class="ps-cf-header">
		<span class="ps-cf-eyebrow">YOUR COLLECTION</span>
		<h1 class="ps-cf-title">Library</h1>
		<span class="ps-cf-sub">{albums.length} ALBUMS</span>
	</div>

	<!-- the 3D arc. .ps-cf-perspective is the camera; .ps-cf-track is what translates -->
	<div class="ps-cf-perspective">
		<div
			class="ps-cf-track"
			style="transform: translate3d(calc(50% - {CARD_W / 2}px - {scrollIndex.current * GAP}px), 0, 0);"
		>
			{#each albums as a, i (a.id)}
				{@const d = i - scrollIndex.current}
				{@const abs = Math.abs(d)}
				{@const angle = abs < 0.001 ? 0 : (d < 0 ? MAX_ANGLE : -MAX_ANGLE) * Math.min(1, abs / DEPTH_FALLOFF)}
				{@const z = -abs * 60}
				{@const scale = 1 - Math.min(0.35, abs * 0.06)}
				{@const opacity = 1 - Math.min(0.6, abs * 0.12)}
				<!--
				  The card itself is a role="button" <div>, not a real <button>, because
				  it contains a real <button> (the play overlay). Nested <button> is
				  invalid HTML and breaks Svelte's structure assumptions.
				-->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
				<div
					class="ps-cf-card {abs < 0.5 ? 'is-focused' : ''}"
					style="transform: translate(-50%, -50%) translateX({i * GAP}px) translateZ({z}px) rotateY({angle}deg) scale({scale}); opacity: {opacity}; z-index: {1000 - Math.round(abs * 10)};"
					role="button"
					tabindex="0"
					onclick={() => clickAt(i)}
					onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), clickAt(i))}
					title={`${a.title} — ${a.subtitle ?? ''}`}
				>
					<div class="ps-cf-card-frame">
						<div class="ps-cf-card-sleeve">
							<div class="ps-cf-card-mouth"></div>
						</div>
						<div class="ps-cf-card-disc">
							<div class="ps-cf-card-art" style="background-image: url('{artFor(a)}');"></div>
						</div>
					</div>
					<button class="ps-cf-card-play" onclick={(e) => playAt(i, e)} aria-label={`Play ${a.title}`} title="Play album">
						<HugeiconsIcon icon={PlayIcon} />
					</button>
					</div>
					{/each}
		</div>
	</div>

	<!-- focused album caption (the .is-focused card's info) -->
	{#key scrollIndex.current}
		{@const i = Math.round(scrollIndex.current)}
		{@const a = albums[((i % albums.length) + albums.length) % albums.length]}
		{#if a}
			<div class="ps-cf-caption">
				<span class="ps-cf-caption-kind">{a.kind === 'artist' ? 'ARTIST' : 'ALBUM'}</span>
				<h2 class="ps-cf-caption-title">{a.title}</h2>
				<span class="ps-cf-caption-artist">{a.subtitle ?? ''}</span>
			</div>
		{/if}
	{/key}

	<!-- always-on-top mini player pill (sits over the carousel, never moves) -->
	<MiniPlayerPill onOpenNow={openNow} />
</div>

<style>
	.ps-cf-stage {
		position: absolute;
		inset: 0;
		overflow: hidden;
	}
	.ps-cf-back {
		all: unset;
		cursor: pointer;
		position: absolute;
		left: 24px;
		top: 24px;
		width: 44px;
		height: 44px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: rgba(255, 255, 255, 0.18);
		backdrop-filter: blur(14px);
		border: 1px solid rgba(255, 255, 255, 0.3);
		color: #fff;
		z-index: 4;
		transition: background 0.15s, transform 0.15s;
	}
	.ps-cf-back:hover { background: rgba(255, 255, 255, 0.3); transform: scale(1.06); }
	.ps-cf-header {
		position: absolute;
		left: 50%;
		top: 28px;
		transform: translateX(-50%);
		text-align: center;
		z-index: 2;
		pointer-events: none;
	}
	.ps-cf-eyebrow {
		display: block;
		font-size: 10px;
		letter-spacing: 0.4em;
		text-transform: uppercase;
		opacity: 0.65;
	}
	.ps-cf-title {
		font-family: var(--display);
		font-size: 38px;
		letter-spacing: 0.08em;
		margin: 4px 0 0;
		text-shadow: 0 2px 12px rgba(8, 60, 70, 0.6);
	}
	.ps-cf-sub {
		display: block;
		font-size: 10px;
		letter-spacing: 0.32em;
		text-transform: uppercase;
		opacity: 0.6;
		margin-top: 4px;
	}
	.ps-cf-perspective {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
		perspective: 900px;
		perspective-origin: 50% 60%;
	}
	.ps-cf-track {
		position: relative;
		width: 0;
		height: 0;
		transform-style: preserve-3d;
		/* No animation: the rAF loop and Spring drive translateX. */
	}
	.ps-cf-card {
		all: unset;
		cursor: pointer;
		position: absolute;
		left: 0;
		top: 50%;
		width: 240px;
		height: 240px;
		transform-style: preserve-3d;
		transition: transform 0.6s cubic-bezier(0.22, 1, 0.36, 1), opacity 0.5s;
		will-change: transform, opacity;
	}
	.ps-cf-card-frame {
		position: relative;
		width: 100%;
		height: 100%;
	}
	.ps-cf-card-sleeve {
		position: absolute;
		inset: 0;
		border-radius: 16px;
		background: linear-gradient(160deg, #c39a76, #b08968 55%, #8f6b4e);
		box-shadow: 0 18px 44px rgba(8, 60, 70, 0.45), 0 6px 16px rgba(8, 60, 70, 0.3),
			inset 0 2px 3px rgba(255, 255, 255, 0.35), inset 0 -6px 14px rgba(90, 60, 35, 0.45);
		overflow: hidden;
	}
	.ps-cf-card-mouth {
		position: absolute;
		left: 24%;
		top: 3%;
		bottom: 3%;
		width: 9%;
		border-radius: 12px;
		background: linear-gradient(90deg, rgba(60, 38, 20, 0.5), transparent);
	}
	.ps-cf-card-disc {
		position: absolute;
		left: -10%;
		top: 50%;
		width: 100%;
		height: 100%;
		transform: translateY(-50%);
		filter: drop-shadow(0 12px 24px rgba(8, 60, 70, 0.45));
	}
	.ps-cf-card.is-focused .ps-cf-card-disc {
		left: -20%;
	}
	.ps-cf-card-art {
		width: 100%;
		height: 100%;
		border-radius: 50%;
		background-color: #0a0a0a;
		background-size: cover;
		background-position: center;
		box-shadow: inset 0 0 0 1.5px rgba(255, 255, 255, 0.3);
	}
	.ps-cf-card-play {
		all: unset;
		cursor: pointer;
		position: absolute;
		right: 10px;
		bottom: 10px;
		width: 40px;
		height: 40px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: linear-gradient(180deg, #8fdef6, var(--accent) 55%, #2e9ecb);
		color: #111;
		opacity: 0;
		transform: scale(0.85);
		transition: opacity 0.2s, transform 0.2s;
		box-shadow: 0 4px 14px rgba(14, 110, 140, 0.5);
	}
	.ps-cf-card-play:hover { transform: scale(1.06); }
	.ps-cf-card.is-focused .ps-cf-card-play {
	  opacity: 1;
	  transform: scale(1);
	}
	.ps-cf-caption {
	  position: absolute;
	  left: 50%;
	  bottom: 90px;
	  transform: translateX(-50%);
	  text-align: center;
	  z-index: 2;
	  pointer-events: none;
	  max-width: 60vw;
	}
	.ps-cf-caption-kind {
		display: block;
		font-size: 10px;
		letter-spacing: 0.4em;
		text-transform: uppercase;
		opacity: 0.6;
	}
	.ps-cf-caption-title {
		font-family: var(--display);
		font-size: 22px;
		letter-spacing: 0.04em;
		margin: 4px 0 0;
		text-shadow: 0 2px 12px rgba(8, 60, 70, 0.6);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-cf-caption-artist {
		display: block;
		font-size: 11px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		opacity: 0.75;
		margin-top: 2px;
	}
</style>
