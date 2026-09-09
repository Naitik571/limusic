<!--
  AlbumStackView — vertical 3D album stack for the Library (BlazePod-style alternative
  to the horizontal coverflow and the diagonal fan).

  Model: the albums sit in a receding vertical pile (top card front-facing, the rest
  stepping down and back with scale + dim). Drag the pile up to send the top card to
  the back, drag down to pull one forward; wheel and arrow keys do the same. Clicking
  the top card opens the album; its play button plays it straight from the stack.
-->
<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon } from '@hugeicons/core-free-icons';
	import type { BrowseItem } from '$lib/api';
	import MiniPlayerPill from './MiniPlayerPill.svelte';

	let {
		albums,
		artFor,
		onOpenAlbum,
		onPlayAlbum,
		onBack
	}: {
		albums: BrowseItem[];
		artFor: (item: BrowseItem) => string;
		onOpenAlbum: (item: BrowseItem) => void;
		onPlayAlbum: (item: BrowseItem) => void;
		onBack?: () => void;
	} = $props();

	function back() {
		onBack?.();
	}

	// Rotation order: order[0] is the top card. Advance sends it to the back.
	let order = $state<number[]>([]);
	$effect(() => {
		// Re-seed when the library itself changes (length/identity), not on every render.
		const n = albums.length;
		if (order.length !== n || order.some((o) => o < 0 || o >= n)) {
			order = Array.from({ length: n }, (_, i) => i);
		}
	});

	function advance() {
		if (order.length < 2) return;
		order = [...order.slice(1), order[0]];
	}
	function recede() {
		if (order.length < 2) return;
		order = [order[order.length - 1], ...order.slice(0, -1)];
	}

	// Drag the pile: live-follow the top card, release past 70px to cycle.
	let dragY = $state(0);
	let dragging = $state(false);
	let startY = 0;
	let lastDragDist = 0;
	function onDown(e: PointerEvent) {
		if (e.button !== 0) return;
		dragging = true;
		startY = e.clientY;
		dragY = 0;
		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
	}
	function onMove(e: PointerEvent) {
		if (!dragging) return;
		dragY = e.clientY - startY;
	}
	function onUp() {
		if (!dragging) return;
		dragging = false;
		// Stash the distance BEFORE resetting: the click that follows a drag tests this, and
		// testing dragY after the reset below is always true (every flip opened the album).
		lastDragDist = Math.abs(dragY);
		if (dragY < -70) advance();
		else if (dragY > 70) recede();
		dragY = 0;
	}
	// Cancellation is not a release: reset without cycling, so a touchcancel past the
	// threshold doesn't flip the stack the user never let go of.
	function onCancel() {
		if (!dragging) return;
		dragging = false;
		lastDragDist = Math.abs(dragY);
		dragY = 0;
	}
	let wheelAt = 0;
	function onWheel(e: WheelEvent) {
		const now = performance.now();
		if (now - wheelAt < 220) return;
		wheelAt = now;
		if (e.deltaY > 0) advance();
		else if (e.deltaY < 0) recede();
	}

	function openNow() {
		// Pill passthrough; no-op unless the pill is unmounted.
	}
</script>

<div class="ps-stack-stage">
	<button class="ps-cf-back" onclick={back} aria-label="Back">
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" width="22">
			<path d="M15 5l-7 7 7 7" />
		</svg>
	</button>

	<div class="ps-cf-header">
		<span class="ps-cf-eyebrow">YOUR COLLECTION</span>
		<h1 class="ps-cf-title">Stack</h1>
		<span class="ps-cf-sub">{albums.length} ALBUMS</span>
	</div>

	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div
		class="ps-stack-pile"
		role="listbox"
		aria-label="Album stack — drag up or down to flip through"
		tabindex="0"
		onpointerdown={onDown}
		onpointermove={onMove}
		onpointerup={onUp}
		onpointercancel={onCancel}
		onwheel={onWheel}
		onkeydown={(e) => {
			if (e.key === 'ArrowDown' || e.key === 'ArrowRight') { e.preventDefault(); advance(); }
			if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') { e.preventDefault(); recede(); }
			if ((e.key === 'Enter' || e.key === ' ') && order.length) {
				e.preventDefault();
				const a = albums[order[0]];
				if (a) onOpenAlbum(a);
			}
		}}
	>
		{#each order as ai, depth (albums[ai]?.id ?? ai)}
			{@const a = albums[ai]}
			{#if a}
				{@const lift = depth === 0 && dragging ? dragY : 0}
				<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions: Enter/Space on the pile
				     itself opens the top card; the play button is a real button. -->
				<div
					class="ps-stack-card {depth === 0 ? 'is-top' : ''}"
					style="transform: translateY({depth * 44 + lift}px) translateZ({-depth * 110}px) scale({Math.max(0.6, 1 - depth * 0.07)}); opacity: {Math.max(0, 1 - depth * 0.16)}; z-index: {1000 - depth}; {dragging && depth === 0 ? 'transition: none;' : ''}"
					onclick={() => {
						if (depth === 0 && lastDragDist < 6) onOpenAlbum(a);
					}}
				>
					<img decoding="async" src={artFor(a)} alt="" draggable="false" />
					{#if depth === 0}
						<div class="ps-stack-frame">
							<span class="ps-stack-title">{a.title}</span>
							<span class="ps-stack-artist">{a.subtitle ?? ''}</span>
						</div>
						<button
							class="ps-stack-play"
							onclick={(e) => {
								e.stopPropagation();
								onPlayAlbum(a);
							}}
							aria-label={`Play ${a.title}`}
							title="Play album"
						>
							<HugeiconsIcon icon={PlayIcon} />
						</button>
					{/if}
				</div>
			{/if}
		{/each}
		{#if !order.length}
			<div class="ps-empty">No albums yet — sign in or import a folder.</div>
		{/if}
	</div>

	<div class="ps-stack-hint" aria-hidden="true">DRAG TO FLIP · CLICK TOP TO OPEN</div>

	<MiniPlayerPill onOpenNow={openNow} />
</div>

<style>
	.ps-stack-stage {
		position: absolute;
		inset: 0;
		overflow: hidden;
	}
	.ps-stack-pile {
		position: absolute;
		inset: 0;
		perspective: 1100px;
		perspective-origin: 50% 38%;
		touch-action: none;
		cursor: grab;
		outline: none;
	}
	.ps-stack-pile:active { cursor: grabbing; }
	.ps-stack-card {
		position: absolute;
		left: 50%;
		top: 30%;
		width: min(46vmin, 300px);
		aspect-ratio: 1;
		margin-left: calc(min(46vmin, 300px) / -2);
		margin-top: calc(min(46vmin, 300px) / -2);
		border-radius: 14px;
		overflow: hidden;
		background: #0a0a0a;
		border: 1px solid rgba(255, 255, 255, 0.14);
		box-shadow: 0 26px 54px -16px rgba(0, 0, 0, 0.6), 0 8px 20px -6px rgba(0, 0, 0, 0.4), inset 0 0 0 1px rgba(255, 255, 255, 0.08);
		transform-style: preserve-3d;
		transition: transform 0.45s var(--ease-spring), opacity 0.35s ease-out;
		will-change: transform, opacity;
	}
	.ps-stack-card img {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
		pointer-events: none;
	}
	.ps-stack-frame {
		position: absolute;
		left: 0; right: 0; bottom: 0;
		padding: 14px 16px 12px;
		background: linear-gradient(transparent 0%, rgba(0, 0, 0, 0.75) 100%);
		pointer-events: none;
	}
	.ps-stack-title {
		display: block;
		color: #fff;
		font-size: 15px;
		font-weight: 700;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-stack-artist {
		display: block;
		color: rgba(255, 255, 255, 0.75);
		font-size: 10.5px;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		margin-top: 3px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-stack-play {
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
		box-shadow: 0 4px 14px rgba(14, 110, 140, 0.5);
	}
	.ps-stack-play svg { width: 16px; height: 16px; }
	.ps-stack-hint {
		position: absolute;
		left: 50%;
		bottom: 92px;
		transform: translateX(-50%);
		font-size: 9px;
		letter-spacing: 0.3em;
		text-transform: uppercase;
		opacity: 0.5;
		pointer-events: none;
		white-space: nowrap;
	}
	.ps-stack-stage .ps-cf-back {
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
	}
	.ps-stack-stage .ps-cf-header {
		position: absolute;
		left: 50%;
		top: 28px;
		transform: translateX(-50%);
		text-align: center;
		z-index: 2;
		pointer-events: none;
	}
	.ps-stack-stage .ps-cf-eyebrow {
		display: block;
		font-size: 10px;
		letter-spacing: 0.4em;
		text-transform: uppercase;
		opacity: 0.65;
	}
	.ps-stack-stage .ps-cf-title {
		font-family: var(--display);
		font-size: 38px;
		letter-spacing: 0.08em;
		margin: 4px 0 0;
	}
	.ps-stack-stage .ps-cf-sub {
		display: block;
		font-size: 10px;
		letter-spacing: 0.32em;
		text-transform: uppercase;
		opacity: 0.6;
		margin-top: 4px;
	}
</style>
