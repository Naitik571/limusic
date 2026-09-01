<!--
  StackedFanView — diagonal fan/stack of album cards, receding from bottom-left
  to upper-right. Static until interaction. Hover shows pill tooltip. Back arrow
  in top-left exits to Library.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import type { BrowseItem } from '$lib/api';

	let { albums, artFor, onOpenAlbum, onPlayAlbum }: {
		albums: BrowseItem[];
		artFor: (item: BrowseItem) => string;
		onOpenAlbum: (item: BrowseItem) => void;
		onPlayAlbum: (item: BrowseItem) => void;
	} = $props();

	let hovered = $state<{ title: string; subtitle?: string; x: number; y: number } | null>(null);

	function onCardHover(e: MouseEvent, a: BrowseItem) {
		hovered = { title: a.title, subtitle: a.subtitle, x: e.clientX, y: e.clientY };
	}
	function onCardLeave() {
		hovered = null;
	}
	function playAt(a: BrowseItem) {
		onPlayAlbum(a);
	}
</script>

<div class="ps-fan-stage">
	<button class="ps-fan-back" onclick={() => history.length > 1 ? history.back() : null} aria-label="Back">
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" width="22">
			<path d="M15 5l-7 7 7 7" />
		</svg>
	</button>

	<div class="ps-fan-header">
		<span class="ps-fan-eyebrow">YOUR COLLECTION</span>
		<h1 class="ps-fan-title">Library</h1>
		<span class="ps-fan-sub">{albums.length} ALBUMS</span>
	</div>

	<div class="ps-fan-stack">
		{#each albums as a, i (a.id)}
			{@const depth = i}
			{@const scale = 1 - depth * 0.04}
			{@const opacity = 1 - depth * 0.08}
			{@const rotate = -6 + depth * 1.8}
			{@const tx = 12 + depth * 18}
			{@const ty = -8 - depth * 14}
			<div
				class="ps-fan-card"
				style="transform: translate({tx}px, {ty}px) rotate({rotate}deg) scale({Math.max(0.7, scale)}); opacity: {Math.max(0.4, opacity)}; z-index: {1000 - depth};"
				onmouseenter={(e) => onCardHover(e, a)}
				onmouseleave={onCardLeave}
				onclick={() => playAt(a)}
				role="button"
				tabindex="0"
				onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (e.preventDefault(), playAt(a))}
				title={`${a.title} — ${a.subtitle ?? ''}`}
			>
				<div class="ps-fan-card-sleeve">
					<div class="ps-fan-card-art" style="background-image: url('{artFor(a)}');"></div>
				</div>
				<div class="ps-fan-card-meta">
					<span class="ps-fan-card-title">{a.title}</span>
					<span class="ps-fan-card-artist">{a.subtitle ?? ''}</span>
				</div>
			</div>
		{/each}
	</div>

	{#if hovered}
		<div class="ps-fan-tooltip" style="left: {hovered.x + 12}px; top: {hovered.y - 12}px;">
			<span class="ps-fan-tooltip-title">{hovered.title}</span>
			{#if hovered.subtitle}<span class="ps-fan-tooltip-sub">{hovered.subtitle}</span>{/if}
		</div>
	{/if}
</div>

<style>
	.ps-fan-stage {
		position: absolute;
		inset: 0;
		overflow: hidden;
		background: transparent;
	}
	.ps-fan-back {
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
	.ps-fan-back:hover {
		background: rgba(255, 255, 255, 0.3);
		transform: scale(1.06);
	}
	.ps-fan-header {
		position: absolute;
		left: 50%;
		top: 28px;
		transform: translateX(-50%);
		text-align: center;
		z-index: 2;
		pointer-events: none;
	}
	.ps-fan-eyebrow {
		display: block;
		font-size: 10px;
		letter-spacing: 0.4em;
		text-transform: uppercase;
		opacity: 0.65;
	}
	.ps-fan-title {
		font-family: var(--display);
		font-size: 38px;
		letter-spacing: 0.08em;
		margin: 4px 0 0;
		text-shadow: 0 2px 12px rgba(8, 60, 70, 0.6);
	}
	.ps-fan-sub {
		display: block;
		font-size: 10px;
		letter-spacing: 0.32em;
		text-transform: uppercase;
		opacity: 0.6;
		margin-top: 4px;
	}
	.ps-fan-stack {
		position: absolute;
		left: 8%;
		bottom: 8%;
		width: 84%;
		height: 78%;
		z-index: 1;
	}
	.ps-fan-card {
		position: absolute;
		left: 0;
		bottom: 0;
		width: 260px;
		cursor: pointer;
		transition: filter 0.3s ease;
		transform-origin: left bottom;
	}
	.ps-fan-card:hover {
		filter: brightness(1.08);
	}
	.ps-fan-card-sleeve {
		position: relative;
		width: 100%;
		aspect-ratio: 1 / 1.05;
		border-radius: 18px;
		background: linear-gradient(160deg, #c39a76, #b08968 55%, #8f6b4e);
		box-shadow: 0 18px 44px rgba(8, 60, 70, 0.45), 0 6px 16px rgba(8, 60, 70, 0.3),
			inset 0 2px 3px rgba(255, 255, 255, 0.35), inset 0 -6px 14px rgba(90, 60, 35, 0.45);
		overflow: hidden;
	}
	.ps-fan-card-art {
		position: absolute;
		inset: 0;
		border-radius: 18px;
		background-color: #0a0a0a;
		background-size: cover;
		background-position: center;
		box-shadow: inset 0 0 0 1.5px rgba(255, 255, 255, 0.3);
	}
	.ps-fan-card-meta {
		margin-top: 10px;
		display: flex;
		flex-direction: column;
		gap: 3px;
		padding-right: 20px;
	}
	.ps-fan-card-title {
		font-size: 12px;
		font-weight: 700;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		color: rgba(255, 255, 255, 0.92);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-fan-card-artist {
		font-size: 10px;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		opacity: 0.65;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-fan-tooltip {
		position: fixed;
		z-index: 9999;
		transform: translate(12px, -12px);
		background: rgba(8, 50, 60, 0.85);
		backdrop-filter: blur(14px);
		border: 1px solid rgba(255, 255, 255, 0.25);
		border-radius: 999px;
		padding: 8px 14px;
		display: flex;
		flex-direction: column;
		gap: 2px;
		pointer-events: none;
		box-shadow: 0 10px 30px rgba(0, 0, 0, 0.35);
	}
	.ps-fan-tooltip-title {
		font-size: 11px;
		font-weight: 700;
		letter-spacing: 0.08em;
		color: #fff;
		white-space: nowrap;
	}
	.ps-fan-tooltip-sub {
		font-size: 9px;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		opacity: 0.75;
		color: rgba(255, 255, 255, 0.9);
		white-space: nowrap;
	}
</style>
