<!-- Poolside water background. Three parts:
     1. Base gradient + drifting caustic layers wrapped in an SVG displacement filter
        (CSS-Realistic-Water technique: SMIL-animated feTurbulence + feDisplacementMap)
     2. Mesh-gradient color field — a slow-rotating conic layer (transform-only, GPU cheap)
        in the style of gradients.juangarcia.ch
     3. Pre-blurred glow blobs + koi -->
<div class="ps-water" aria-hidden="true">
	<div class="ps-mesh" aria-hidden="true"></div>
	<div class="ps-caustic-wrap">
		<div class="ps-caustic c1"></div>
		<div class="ps-caustic c2"></div>
	</div>
	<div class="ps-blob coral"></div>
	<div class="ps-blob sky"></div>
	<svg class="ps-koi k1" viewBox="0 0 120 54">
		<path d="M8 27c14-16 34-22 56-20 18 2 34 9 42 20-8 11-24 18-42 20-22 2-42-4-56-20z" fill="#F28C3B" />
		<path d="M8 27c10 4 20 6 30 6-6 6-14 9-24 9-4-4-6-9-6-15z" fill="#E86A1F" />
		<path d="M104 27l14-12c-2 8-2 16 0 24l-14-12z" fill="#E86A1F" />
		<circle cx="24" cy="24" r="2.6" fill="#1a1a1a" />
	</svg>
	<svg class="ps-koi k2" viewBox="0 0 120 54">
		<path d="M8 27c14-16 34-22 56-20 18 2 34 9 42 20-8 11-24 18-42 20-22 2-42-4-56-20z" fill="#F7F1E6" />
		<path d="M44 9c12-2 26 1 38 8-6 8-18 13-30 13-5-6-8-13-8-21z" fill="#F28C3B" />
		<path d="M104 27l14-12c-2 8-2 16 0 24l-14-12z" fill="#D9CFBA" />
		<circle cx="24" cy="24" r="2.6" fill="#1a1a1a" />
	</svg>
</div>

<!-- the displacement filter lives at root scope so `filter: url(#ps-sea)` can reach it -->
<svg width="0" height="0" style="position:absolute" aria-hidden="true">
	<filter id="ps-sea" x="0" y="0" width="100%" height="100%">
		<feTurbulence id="ps-sea-turb" type="turbulence" numOctaves="3" seed="2" baseFrequency="0.02 0.05" result="n" />
		<feDisplacementMap in="SourceGraphic" in2="n" scale="18" />
		<animate
			xlink:href="#ps-sea-turb"
			attributeName="baseFrequency"
			dur="60s"
			keyTimes="0;0.5;1"
			values="0.02 0.05;0.04 0.08;0.02 0.05"
			repeatCount="indefinite"
		/>
	</filter>
</svg>
