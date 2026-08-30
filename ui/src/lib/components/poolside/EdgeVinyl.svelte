<!--
  EdgeVinyl — a large decorative vinyl record anchored to the left or right edge of the
  screen, ~60-70% cropped off-screen so only a partial circle is visible. Continuously
  rotates in place. Behind the main UI but above the background.

  Renders a real-looking picture-disc (grooves, label, blurred album art in the centre)
  but is purely decorative — no pointer events.
-->
<script lang="ts">
	let {
		side = 'left',
		size = 700,
		art = '',
		speed = 24 // seconds per full rotation
	}: { side?: 'left' | 'right'; size?: number; art?: string; speed?: number } = $props();
</script>

<div
	class="ps-edge-vinyl ps-edge-vinyl--{side}"
	style="width: {size}px; height: {size}px; --spin-dur: {speed}s;"
	aria-hidden="true"
>
	<svg viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
		<defs>
			<radialGradient id="edge-vinyl-grooves" cx="50%" cy="50%" r="50%">
				<stop offset="0%" stop-color="#0a0a0a" />
				<stop offset="60%" stop-color="#101010" />
				<stop offset="100%" stop-color="#0a0a0a" />
			</radialGradient>
			<linearGradient id="edge-vinyl-sheen" x1="0" y1="0" x2="1" y2="1">
				<stop offset="0%" stop-color="rgba(255,255,255,0.18)" />
				<stop offset="50%" stop-color="rgba(255,255,255,0)" />
				<stop offset="100%" stop-color="rgba(255,255,255,0.12)" />
			</linearGradient>
			<clipPath id="edge-vinyl-clip">
				<circle cx="100" cy="100" r="100" />
			</clipPath>
		</defs>

		<g clip-path="url(#edge-vinyl-clip)">
			<g class="ps-edge-vinyl-spin">
				<!-- record body -->
				<circle cx="100" cy="100" r="100" fill="url(#edge-vinyl-grooves)" />
				<!-- grooves: a tight set of concentric rings -->
				{#each Array(28) as _, i}
					<circle
						cx="100"
						cy="100"
						r={28 + i * 2.4}
						fill="none"
						stroke="rgba(255,255,255,0.05)"
						stroke-width="0.5"
					/>
				{/each}
				<!-- centre label with the (blurred) album art -->
				<circle cx="100" cy="100" r="32" fill="#0a0a0a" />
				{#if art}
					<image
						href={art}
						x="68"
						y="68"
						width="64"
						height="64"
						preserveAspectRatio="xMidYMid slice"
						filter="blur(2px)"
					/>
				{/if}
				<circle cx="100" cy="100" r="32" fill="none" stroke="rgba(255,255,255,0.18)" stroke-width="1" />
				<!-- spindle hole -->
				<circle cx="100" cy="100" r="3" fill="#000" />
				<!-- moving specular sheen -->
				<circle cx="100" cy="100" r="100" fill="url(#edge-vinyl-sheen)" />
			</g>
		</g>
	</svg>
</div>

<style>
	.ps-edge-vinyl {
		position: fixed;
		top: 50%;
		transform: translateY(-50%);
		z-index: 1; /* above background water, below main UI */
		pointer-events: none;
		filter: drop-shadow(0 24px 50px rgba(0, 0, 0, 0.5));
		/* Slight blur to push it further back (depth-of-field) */
	}
	.ps-edge-vinyl--left {
		left: calc(-1 * 0.65 * var(--vinyl-size, 700px));
	}
	.ps-edge-vinyl--right {
		right: calc(-1 * 0.65 * var(--vinyl-size, 700px));
	}
	.ps-edge-vinyl svg {
		width: 100%;
		height: 100%;
		display: block;
		overflow: visible;
	}
	/* The spinning group is what the CSS animation rotates; the SVG is static. */
	.ps-edge-vinyl-spin {
		transform-origin: 100px 100px;
		transform-box: view-box;
		animation: ps-edge-vinyl-spin var(--spin-dur, 24s) linear infinite;
	}
	@keyframes ps-edge-vinyl-spin {
		to {
			transform: rotate(360deg);
		}
	}
	@media (prefers-reduced-motion: reduce) {
		.ps-edge-vinyl-spin { animation: none; }
	}
</style>
