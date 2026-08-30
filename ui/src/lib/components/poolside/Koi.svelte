<!--
  Koi SVG — a single fish that swims across the pool. The outer .ps-koi element handles
  the path (keyframed in poolside.css) and the inner .body bobs vertically. The fish
  itself faces right by default; CSS flips it via scaleX(-1) at the path turn-around.
-->
<script lang="ts">
	let { color = '#F4A078', size = 56 }: { color?: string; size?: number } = $props();
</script>

<div class="ps-koi-svg" style="width: {size}px; height: {size * 0.45}px;">
	<svg viewBox="0 0 80 36" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
		<defs>
			<radialGradient id="koi-body" cx="55%" cy="40%" r="65%">
				<stop offset="0%" stop-color="#fff" stop-opacity=".95" />
				<stop offset="35%" stop-color={color} stop-opacity=".9" />
				<stop offset="100%" stop-color="#7a2410" stop-opacity=".85" />
			</radialGradient>
			<linearGradient id="koi-fin" x1="0" y1="0" x2="1" y2="0">
				<stop offset="0%" stop-color="#fff" stop-opacity=".5" />
				<stop offset="100%" stop-color={color} stop-opacity=".3" />
			</linearGradient>
		</defs>
		<!-- tail (trailing) -->
		<path d="M2 18 Q8 6 14 14 Q12 18 14 22 Q8 30 2 18 Z" fill="url(#koi-fin)" />
		<!-- body -->
		<ellipse cx="40" cy="18" rx="28" ry="11" fill="url(#koi-body)" />
		<!-- top fin -->
		<path d="M30 8 Q40 2 50 8 Q45 12 35 12 Z" fill="url(#koi-fin)" opacity=".7" />
		<!-- bottom fin -->
		<path d="M30 28 Q40 34 50 28 Q45 24 35 24 Z" fill="url(#koi-fin)" opacity=".5" />
		<!-- side fin (front) -->
		<path d="M48 20 Q56 24 60 18 Q56 14 50 16 Z" fill="url(#koi-fin)" opacity=".6" />
		<!-- white belly highlight -->
		<ellipse cx="42" cy="22" rx="18" ry="3" fill="#fff" opacity=".25" />
		<!-- spot pattern (each koi gets a couple of orange patches) -->
		<circle cx="32" cy="14" r="3" fill="#fff" opacity=".5" />
		<circle cx="48" cy="20" r="2" fill="#fff" opacity=".4" />
		<circle cx="38" cy="16" r="1.5" fill={color} opacity=".6" />
		<!-- eye -->
		<circle cx="62" cy="17" r="1.6" fill="#0c1a1f" />
		<circle cx="62.5" cy="16.5" r=".5" fill="#fff" />
	</svg>
</div>

<style>
	.ps-koi-svg { display: block; overflow: visible; }
	.ps-koi-svg svg { width: 100%; height: 100%; display: block; overflow: visible; }
</style>
