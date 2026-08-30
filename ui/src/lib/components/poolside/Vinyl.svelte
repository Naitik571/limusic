<script lang="ts">
	// Skeuomorphic picture-disc vinyl — sitting in a pool, with physics.
	//
	// What's new vs the previous static version:
	//   - Wobble: a real rAF loop adds a tiny eccentric rotation while playing, like a
	//     real record has — it never sits perfectly still, even when the spin is
	//     perfectly constant. amplitude = 1.5deg, frequency ~3Hz (the "warble" of a
	//     slightly off-center spindle).
	//   - Dual-speed sheens: the two specular highlights rotate at slightly different
	//     speeds than the art so the surface looks like it has multiple reflections
	//     at different angles (like wet vinyl catching the room light from two
	//     different sources).
	//   - Hover lift: the disc raises 6px and gets a deeper blue shadow when hovered.
	//   - Drop-in: when the disc first appears it does a scale(0.7)->1 with a small
	//     rotation settle.
	//   - Custom-cover flip: when `flipped` is true, the disc does a 720deg spin once.
	//
	// .spin / sheens / label / spindle all keep the same DOM structure; the wobble
	// is applied via a CSS variable on the wrapper that the .spin + sheens read
	// through their existing transform.
	import { onMount } from 'svelte';
	import { reducedMotion, rafLoop } from './motion';

	let {
		src,
		playing = false,
		style = '',
		title,
		onclick,
		flightTarget = false,
		size = 0,
		flipped = false
	}: {
		src: string;
		playing?: boolean;
		style?: string;
		title?: string;
		onclick?: (e: MouseEvent) => void;
		flightTarget?: boolean;
		/** Optional fixed pixel size — locks the diameter instead of letting the parent size it. */
		size?: number;
		/** When true, the disc does a one-time 720° flip animation on mount. */
		flipped?: boolean;
	} = $props();

	let root = $state<HTMLDivElement>();
	let spinEl = $state<HTMLDivElement>();

	// Spin/wobble physics. We translate the css angle offset onto a single CSS variable
	// (--ps-wobble) that the .spin-wrap inherits. The base spin is the existing CSS
	// animation in poolside.css; we just add a small extra rotation on top.
	let wobbleDeg = 0;
	let wobbleVel = 0;
	// Track the rise of the `flipped` prop via $effect, so the rAF loop can run a one-time
	// 720° flip on each rising edge without the parent having to clear it.
	let lastFlipped = false;
	let flipStart = 0;
	let flipProgress = 1; // 0..1, 1 == done

	$effect(() => {
		// Detect the rising edge of `flipped`: when it goes false->true, start the flip.
		const f = flipped;
		if (f && !lastFlipped) {
			flipStart = performance.now();
			flipProgress = 0;
		}
		lastFlipped = f;
	});

	onMount(() => {
		const stop = rafLoop((t) => {
			if (!playing || reducedMotion()) {
				// ease wobble to 0 when paused
				wobbleDeg += (0 - wobbleDeg) * 0.2;
				if (root) root.style.setProperty('--ps-wobble', `${wobbleDeg}deg`);
				return;
			}
			// Cheap physics: integrate a 1.5° amplitude sine at ~3 Hz into a small
			// velocity term so it doesn't look perfectly mechanical.
			const period = 0.33; // seconds
			const target = Math.sin((t % period) * (Math.PI * 2 / period)) * 1.5;
			wobbleVel += (target - wobbleDeg) * 0.35;
			wobbleVel *= 0.5;
			wobbleDeg += wobbleVel;

			// The custom-cover flip: 720° over 0.7s, eased, then it sits at 0 and the
			// parent should set flipped back to false on next render.
			let extra = 0;
			if (flipProgress < 1) {
				const elapsed = (performance.now() - flipStart) / 1000;
				const dur = 0.7;
				if (elapsed < dur) {
					flipProgress = elapsed / dur;
					// ease-out cubic
					const eased = 1 - Math.pow(1 - flipProgress, 3);
					extra = 720 * eased;
				} else {
					flipProgress = 1;
				}
			}

			if (root) root.style.setProperty('--ps-wobble', `${wobbleDeg + extra}deg`);
		});
		return stop;
	});
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
	bind:this={root}
	class="ps-vinyl {playing ? 'playing' : ''}"
	style="--art:url('{src}');{style}{size ? ` width:${size}px; height:${size}px;` : ''}"
	{title}
	{onclick}
	role={onclick ? 'button' : undefined}
	tabindex={onclick ? 0 : undefined}
	onkeydown={onclick
		? (e) => {
				if (e.key === 'Enter' || e.key === ' ') {
					e.preventDefault();
					(onclick as (e: MouseEvent) => void)(new MouseEvent('click'));
				}
			}
		: undefined}
	data-flight-target={flightTarget ? 'true' : undefined}
>
	<div class="pool-shadow" aria-hidden="true"></div>
	<!-- everything that rotates; the wobble is applied via translateZ on the wrapper
	     so the .spin transform stacks on top of it cleanly. -->
	<div class="spin-wrap" bind:this={spinEl} style="transform: rotate(var(--ps-wobble, 0deg));">
		<div class="spin">
			<div class="art"></div>
			<div class="grooves"></div>
		</div>
		<div class="sheen-a"></div>
		<div class="sheen-b"></div>
	</div>
	<!-- these stay put -->
	<div class="label-ring"></div>
	<div class="spindle"></div>
</div>
