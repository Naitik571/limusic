<!--
  Koi — a real-looking fish, not a flat blob.

  The svg faces RIGHT by default. The outer wrapper (.ps-koi) is positioned by
  CSS keyframes that translate it across the pool. The fish needs to:
    1. Wag its tail (a sine-driven rotate on the tail group, at ~3 Hz)
    2. Face the direction it's actually moving (the orientation problem you flagged:
       a CSS keyframe that just translates the element never changes which way it
       points, so a fish swimming from left -> right looks the same as one swimming
       right -> left).
    3. Have a proper silhouette: rounded body curve, dorsal + ventral fins, a tail
       with flukes (not a single triangle), an eye, and a belly highlight.

  Approach: the outer wrapper is what CSS keyframes move. We track the wrapper's
  x-translation per-frame in a rAF loop and set a CSS variable --ps-koi-angle on it.
  The fish's body (inside the wrapper) is then rotated by --ps-koi-angle, so it
  always faces its direction of travel. On direction reversal (the path flips
  scaleX), the angle smoothly tracks the new heading instead of staying pinned.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { reducedMotion, rafLoop } from './motion';

	let { color = '#F4A078', size = 60 }: { color?: string; size?: number } = $props();

	let root = $state<HTMLDivElement>();
	let lastX = 0;
	let lastT = 0;
	let angle = 0; // current heading in radians, smoothed
	let targetAngle = 0;

	onMount(() => {
		// Read the wrapper's current screen position on each frame, derive the velocity
		// vector, and update the heading angle so the fish faces its actual motion.
		const stop = rafLoop((_t, dt) => {
			if (!root) return;
			const r = root.getBoundingClientRect();
			const x = r.left + r.width / 2;
			const t = performance.now();
			if (lastT > 0 && dt > 0 && dt < 0.1) {
				const vx = (x - lastX) / dt; // px/s
				// Path keyframes move the fish between -15vw and 115vw and back. When the
				// path is going right, vx > 0. When it's going left (after the 50% turn),
				// vx < 0. The 90° / 270° rotation we apply below handles both directions.
				if (Math.abs(vx) > 5 && !reducedMotion()) {
					// Smooth toward the new heading. The fish rotates in CSS space: 0deg = facing
					// right (the default SVG pose), 180deg = facing left. So:
					targetAngle = vx > 0 ? 0 : 180;
				}
				// Exponential ease toward target (fast but not snappy)
				const k = 1 - Math.exp(-6 * dt);
				angle += (targetAngle - angle) * k;
				// Convert degrees to radians for CSS rotate (and round to integer deg)
				const deg = Math.round(angle);
				root.style.setProperty('--ps-koi-angle', `${deg}deg`);
			}
			lastX = x;
			lastT = t;
		});
		return stop;
	});
</script>

<div
	bind:this={root}
	class="ps-koi-sprite"
	style="width: {size * 1.6}px; height: {size}px; --ps-koi-angle: 0deg;"
>
	<svg viewBox="0 0 100 60" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
		<defs>
			<radialGradient id="koi-body" cx="55%" cy="40%" r="65%">
				<stop offset="0%" stop-color="#fff" stop-opacity=".9" />
				<stop offset="35%" stop-color={color} stop-opacity=".95" />
				<stop offset="100%" stop-color="#5e1d0e" stop-opacity=".95" />
			</radialGradient>
			<linearGradient id="koi-fin" x1="0" y1="0" x2="1" y2="0">
				<stop offset="0%" stop-color="#fff" stop-opacity=".55" />
				<stop offset="100%" stop-color={color} stop-opacity=".35" />
			</linearGradient>
			<radialGradient id="koi-belly" cx="50%" cy="80%" r="60%">
				<stop offset="0%" stop-color="#fff" stop-opacity=".55" />
				<stop offset="100%" stop-color="#fff" stop-opacity="0" />
			</radialGradient>
		</defs>

		<!-- Tail group: animated wagging via a CSS keyframe (sine-ish rotation). -->
		<g class="ps-koi-tail">
			<!-- caudal fin (tail) with proper fluke shape — two lobes, like a real koi tail -->
			<path
				d="M 8 30 Q 0 12 14 22 Q 0 32 8 50 Q 4 38 14 42 Q 18 38 18 30 Q 18 22 14 18 Q 4 22 8 30 Z"
				fill="url(#koi-fin)"
				stroke={color}
				stroke-opacity=".25"
				stroke-width="0.5"
			/>
		</g>

		<!-- Pectoral fin (front side fin) — also wags subtly -->
		<g class="ps-koi-pectoral">
			<path
				d="M 60 32 Q 70 38 78 34 Q 70 30 62 28 Q 58 30 60 32 Z"
				fill="url(#koi-fin)"
				opacity=".75"
			/>
		</g>

		<!-- Body: a proper fish silhouette, not a flat ellipse. Head is wider, body
		     tapers to the caudal peduncle (the narrow bit just before the tail). -->
		<path
			d="M 80 30
			   Q 78 18 60 14
			   Q 38 10 22 18
			   Q 14 24 18 30
			   Q 14 36 22 42
			   Q 38 50 60 46
			   Q 78 42 80 30 Z"
			fill="url(#koi-body)"
		/>

		<!-- Belly highlight -->
		<ellipse cx="50" cy="42" rx="22" ry="6" fill="url(#koi-belly)" />

		<!-- Dorsal fin (top) -->
		<path
			d="M 36 14 Q 48 6 60 14 Q 56 18 50 18 Q 44 18 36 14 Z"
			fill="url(#koi-fin)"
			opacity=".7"
		/>
		<!-- Ventral fin (bottom) -->
		<path
			d="M 36 46 Q 48 54 60 46 Q 56 42 50 42 Q 44 42 36 46 Z"
			fill="url(#koi-fin)"
			opacity=".55"
		/>

		<!-- Koi-style orange-and-white spot pattern (kohaku variety) -->
		<ellipse cx="40" cy="22" rx="6" ry="4" fill="#fff" opacity=".7" />
		<ellipse cx="56" cy="36" rx="5" ry="3" fill="#fff" opacity=".65" />
		<ellipse cx="30" cy="32" rx="3" ry="2" fill={color} opacity=".6" />

		<!-- Eye: positioned on the head end (right side, which is the "forward" direction) -->
		<g class="ps-koi-eye">
			<circle cx="74" cy="27" r="2" fill="#0c1a1f" />
			<circle cx="74.6" cy="26.4" r=".6" fill="#fff" />
		</g>

		<!-- Mouth: a small mark at the very front -->
		<path d="M 80 32 Q 78 34 76 33" stroke="#5e1d0e" stroke-width=".6" fill="none" />
	</svg>
</div>

<style>
	.ps-koi-sprite {
		display: block;
		overflow: visible;
		/* The angle is updated per-frame by the rAF loop via --ps-koi-angle. We
		   add a small constant tilt so the fish doesn't look perfectly rigid. */
		transform: rotate(var(--ps-koi-angle, 0deg));
		will-change: transform;
	}
	.ps-koi-sprite svg { width: 100%; height: 100%; display: block; overflow: visible; }

	/* Tail wag: the tail group rotates around its base (x≈18, y≈30) at ~3 Hz.
	   Amplitude is small (5°) — real koi tails don't whip dramatically. */
	.ps-koi-tail {
		transform-origin: 18px 30px;
		transform-box: fill-box;
		animation: ps-koi-tail-wag 0.4s ease-in-out infinite alternate;
	}
	@keyframes ps-koi-tail-wag {
		from { transform: rotate(-5deg); }
		to   { transform: rotate(5deg); }
	}

	/* Pectoral fin: tiny, slower — like a fish "sculling" with its side fin. */
	.ps-koi-pectoral {
		transform-origin: 60px 30px;
		transform-box: fill-box;
		animation: ps-koi-pectoral-flutter 1.2s ease-in-out infinite alternate;
	}
	@keyframes ps-koi-pectoral-flutter {
		from { transform: rotate(-3deg); }
		to   { transform: rotate(4deg); }
	}

	/* Eye blink — every few seconds, briefly scaleY(0.1) to suggest a blink. */
	.ps-koi-eye {
		transform-origin: 74px 27px;
		transform-box: fill-box;
		animation: ps-koi-blink 5.2s steps(1) infinite;
	}
	@keyframes ps-koi-blink {
		0%, 96% { transform: scaleY(1); }
		97%    { transform: scaleY(0.1); }
		98%    { transform: scaleY(1); }
		100%   { transform: scaleY(1); }
	}

	/* Reduce motion: kill the wag and the flutter, keep the orientation lock. */
	@media (prefers-reduced-motion: reduce) {
		.ps-koi-tail, .ps-koi-pectoral, .ps-koi-eye { animation: none !important; }
	}
</style>
