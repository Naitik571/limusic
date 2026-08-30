<script lang="ts">
	// Poolside album detail — coverflow ported from ashishgogula/coverflow (continuous spring
	// scrollX: cards swing through center with rotateY, stack beyond ±1, brightness dim,
	// click-to-snap, drag + wheel) plus addyosmani/threejs-coverflow (reflections under each
	// cover, expo-out settle). All CSS 3D — no WebGL dependency.
	import { Spring } from 'svelte/motion';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ArrowLeft01Icon, PlayIcon } from '@hugeicons/core-free-icons';
	import type { BrowseItem } from '$lib/api';
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

	const CENTER_GAP = 150;
	const STACK_SPACING = 92;
	const ROTATION = 34;

	const n = $derived(albums.length);
	// continuous scroll position — the heart of the coverflow feel
	const scroll = new Spring(0, { stiffness: 150, damping: 30 });

	// keep the spring synced to the selected album (back-navigation, shell-driven changes)
	$effect(() => {
		const i = albums.findIndex((a) => a.id === album.id);
		if (i >= 0) scroll.target = i;
	});

	const activeIdx = $derived(Math.round(scroll.current));

	const cards = $derived.by(() => {
		void scroll.current; // subscribe
		return albums.map((a, i) => {
			const pos = i - scroll.current;
			const abs = Math.abs(pos);
			// swing through center: near |pos|<0.5 the card rotates proportionally
			const rotateY = abs < 0.5 ? -pos * (ROTATION * 2) : pos < 0 ? ROTATION : -ROTATION;
			// continuous spread near center, stacked beyond ±1
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
	}
	function onPointerMove(e: PointerEvent) {
		if (!dragging) {
			return;
		}
		const dx = e.clientX - dragStartX;
		if (Math.abs(dx) > 6) {
			moved = true;
		}
		scroll.target = clamp(dragStartScroll - dx / 140);
	}
	function onPointerUp() {
		if (!dragging) {
			return;
		}
		dragging = false;
		// snap to the nearest card, expo-out via the spring
		scroll.target = clamp(Math.round(scroll.current));
	}
	function onWheel(e: WheelEvent) {
		e.preventDefault();
		scroll.target = clamp(scroll.target + e.deltaY * 0.0022);
	}
	function onCardClick(i: number) {
		if (moved) {
			return;
		}
		if (i === activeIdx) {
			onPlayAlbum(albums[i]);
		} else {
			scroll.target = i;
			onSelect(albums[i]);
		}
	}
	function onKeyDown(e: KeyboardEvent) {
		// Left/Right arrow keys navigate the coverflow. Space/Enter plays the active album.
		if (e.key === 'ArrowLeft') {
			e.preventDefault();
			const i = clamp(activeIdx - 1);
			scroll.target = i;
			onSelect(albums[i]);
		} else if (e.key === 'ArrowRight') {
			e.preventDefault();
			const i = clamp(activeIdx + 1);
			scroll.target = i;
			onSelect(albums[i]);
		} else if (e.key === 'Enter' || e.key === ' ') {
			e.preventDefault();
			onPlayAlbum(albums[activeIdx]);
		}
	}
</script>

<div class="ps-albumview">
	<button class="ps-back ps-glass" onclick={onBack} title="Back to library" aria-label="Back to library">
		<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" width="18"><path d="M15 5l-7 7 7 7" /></svg>
	</button>

	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
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
					tipX = e.clientX + 16;
					tipY = e.clientY - 10;
				}}
				onmouseleave={() => (tip = '')}
			>
				<div class="cov"><img src={artFor(c.a)} alt={c.a.title} draggable="false" /></div>
				{#if c.abs < 0.5}
					<div class="play-chip" title="Play this album">
						<HugeiconsIcon icon={PlayIcon} class="w-3.5 h-3.5" />
					</div>
				{/if}
				<!-- reflection: flipped copy under the cover, like the threejs reference -->
				<div class="reflection" aria-hidden="true">
					<img src={artFor(c.a)} alt="" draggable="false" />
				</div>
			</div>
		{/each}
	</div>

	{#if tip}
		<div class="ps-fan-tip" style="left:{tipX}px;top:{tipY}px;opacity:1">{tip}</div>
	{/if}

	<div class="ps-alb-meta">
		<h3>{album.title.toUpperCase()}</h3>
		<p>{album.subtitle ?? ''}</p>
	</div>

	<div class="ps-alb-actions">
		<button class="ps-aqua px-4.5 py-2.5 text-[9px] flex items-center gap-1.5" onclick={() => onPlayAlbum(album)}>
			<HugeiconsIcon icon={PlayIcon} class="w-3.5 h-3.5" />
			Play album
		</button>
		<button class="ps-ghost" onclick={onOpenCustom}>Add custom CD cover</button>
	</div>

	<div class="ps-alb-deck relative" style="width:min(20vh,180px);margin-top:-6px">
		<div class="ps-sleeve" style="aspect-ratio:1;width:100%"><div class="mouth"></div></div>
		<Vinyl src={artFor(album)} playing={true} style="width:88%;position:absolute;top:6%;left:6%;transform:translateX(24%)" />
	</div>
</div>

<style>
	.ps-fan {
		touch-action: pan-y;
	}
	.ps-fan.dragging {
		cursor: grabbing;
	}
	.ps-fcard {
		will-change: transform, filter;
		/* no overflow clip here — the reflection lives below the cover */
	}
	/* cover: rounded, clipped, top half of the card root */
	.ps-fcard .cov {
		position: absolute;
		inset: 0 0 auto 0;
		aspect-ratio: 1;
		border-radius: 16px;
		overflow: hidden;
		border: 1.5px solid rgba(255, 255, 255, 0.9);
	}
	.ps-fcard .cov img {
		width: 100%; height: 100%;
		object-fit: cover; display: block;
	}
	.ps-fcard .play-chip {
		position: absolute;
		right: 10px; bottom: 10px;
		width: 38px; height: 38px; border-radius: 50%;
		display: grid; place-items: center; color: #fff;
		background: linear-gradient(180deg, #8fdcf2, var(--ps-accent));
		border: 1px solid rgba(255, 255, 255, 0.8);
		box-shadow: 0 4px 12px rgba(8, 60, 70, 0.4);
	}
	/* reflection: flipped copy under the cover, perspective tilt, faded + masked out
	   (threejs-coverflow's reflection plane, as CSS) */
	.ps-fcard .reflection {
		position: absolute;
		left: 0; top: calc(100% + 1px);
		width: 100%; height: 42%;
		pointer-events: none;
		transform-origin: top center;
		transform: rotateX(12deg);
		will-change: transform;
		-webkit-mask-image: linear-gradient(180deg, rgba(0, 0, 0, 0.4), transparent 85%);
		mask-image: linear-gradient(180deg, rgba(0, 0, 0, 0.4), transparent 85%);
	}
	.ps-fcard .reflection img {
		width: 100%; aspect-ratio: 1;
		object-fit: cover; display: block;
		transform: scaleY(-1);
		opacity: 0.4;
	}
</style>
