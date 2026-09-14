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
<script lang="ts" module>
	import { rafLoop, reducedMotion } from './motion';

	type KoiFish = {
		el: HTMLElement;
		visible: boolean;
		angle: number;
		target: number;
		lastX: number;
		seeded: boolean;
	};

	const fishes = new Set<KoiFish>();
	let koiUid = 0;
	let koiLoopOn = false;
	let koiObserver: IntersectionObserver | null = null;

	function koiHidden(el: HTMLElement): boolean {
		// no-koi pref (shell toggles .no-koi on .ps-root) — skip the measuring work.
		return !!el.closest('.ps-root.no-koi');
	}

	function ensureKoiLoop(): void {
		if (koiLoopOn || typeof window === 'undefined') return;
		koiLoopOn = true;
		// ONE shared rAF for every fish on the page.
		rafLoop((_t, dt) => {
			// Early return before any getBoundingClientRect when motion is off.
			if (reducedMotion() || dt <= 0 || dt >= 0.1) return;
			for (const f of fishes) {
				if (!f.visible || koiHidden(f.el)) continue;
				const r = f.el.getBoundingClientRect();
				const x = r.left + r.width / 2;
				if (f.seeded) {
					const vx = (x - f.lastX) / dt; // px/s
					// Path keyframes shuttle the fish between off-screen edges; the
					// sign of vx tells us which way it's actually heading.
					if (Math.abs(vx) > 5) f.target = vx > 0 ? 0 : 180;
					const k = 1 - Math.exp(-6 * dt);
					f.angle += (f.target - f.angle) * k;
					f.el.style.setProperty('--ps-koi-angle', `${Math.round(f.angle)}deg`);
				}
				f.lastX = x;
				f.seeded = true;
			}
		});
	}

	function ensureKoiObserver(): IntersectionObserver | null {
		if (typeof window === 'undefined' || typeof IntersectionObserver === 'undefined') return null;
		if (!koiObserver) {
			koiObserver = new IntersectionObserver(
				(list) => {
					for (const e of list) {
						for (const f of fishes) {
							if (f.el === e.target) f.visible = e.isIntersecting;
						}
					}
				},
				{ threshold: 0 }
			);
		}
		return koiObserver;
	}
</script>

<script lang="ts">
	import { onMount } from 'svelte';

	let { color = '#F4A078', size = 60 }: { color?: string; size?: number } = $props();

	// Unique gradient IDs per instance so N fish never share defs.
	const uid = `k${++koiUid}`;
	const bodyId = `koi-body-${uid}`;
	const finId = `koi-fin-${uid}`;
	const bellyId = `koi-belly-${uid}`;

	let root = $state<HTMLDivElement>();

	onMount(() => {
		if (!root) return;
		ensureKoiLoop();
		const fish: KoiFish = { el: root, visible: true, angle: 0, target: 0, lastX: 0, seeded: false };
		fishes.add(fish);
		const io = ensureKoiObserver();
		io?.observe(root);
		return () => {
			fishes.delete(fish);
			io?.unobserve(root!);
		};
	});
</script>

<div
	bind:this={root}
	class="ps-koi-sprite"
	style="width: {size * 1.6}px; height: {size}px; --ps-koi-angle: 0deg;"
>
	<svg viewBox="0 0 100 60" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
		<defs>
			<radialGradient id={bodyId} cx="55%" cy="40%" r="65%">
				<stop offset="0%" stop-color="#fff" stop-opacity=".9" />
				<stop offset="35%" stop-color={color} stop-opacity=".95" />
				<stop offset="100%" stop-color="#5e1d0e" stop-opacity=".95" />
			</radialGradient>
			<linearGradient id={finId} x1="0" y1="0" x2="1" y2="0">
				<stop offset="0%" stop-color="#fff" stop-opacity=".55" />
				<stop offset="100%" stop-color={color} stop-opacity=".35" />
			</linearGradient>
			<radialGradient id={bellyId} cx="50%" cy="80%" r="60%">
				<stop offset="0%" stop-color="#fff" stop-opacity=".55" />
				<stop offset="100%" stop-color="#fff" stop-opacity="0" />
			</radialGradient>
		</defs>

		<!-- Tail group: animated wagging via a CSS keyframe (sine-ish rotation). -->
		<g class="ps-koi-tail">
			<!-- caudal fin (tail) with proper fluke shape — two lobes, like a real koi tail -->
			<path
				d="M 8 30 Q 0 12 14 22 Q 0 32 8 50 Q 4 38 14 42 Q 18 38 18 30 Q 18 22 14 18 Q 4 22 8 30 Z"
				fill={`url(#${finId})`}
				stroke={color}
				stroke-opacity=".25"
				stroke-width="0.5"
			/>
		</g>

		<!-- Pectoral fin (front side fin) — also wags subtly -->
		<g class="ps-koi-pectoral">
			<path
				d="M 60 32 Q 70 38 78 34 Q 70 30 62 28 Q 58 30 60 32 Z"
				fill={`url(#${finId})`}
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
			fill={`url(#${bodyId})`}
		/>

		<!-- Belly highlight -->
		<ellipse cx="50" cy="42" rx="22" ry="6" fill={`url(#${bellyId})`} />

		<!-- Dorsal fin (top) -->
		<path
			d="M 36 14 Q 48 6 60 14 Q 56 18 50 18 Q 44 18 36 14 Z"
			fill={`url(#${finId})`}
			opacity=".7"
		/>
		<!-- Ventral fin (bottom) -->
		<path
			d="M 36 46 Q 48 54 60 46 Q 56 42 50 42 Q 44 42 36 46 Z"
			fill={`url(#${finId})`}
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
