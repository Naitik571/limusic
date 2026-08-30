<!--
  Poolside water background — deep blue vinyl-pool with a real waterline.

  Stack (back -> front, all under .ps-root):
    1. base gradient (surface teal -> deep navy) gives the pool its depth
    2. tile-grid floor, perspective-tilted so the back rows look further away
    3. waterline highlight at the top (the bright rim where sky reflects on the surface)
    4. two caustic layers drifting in opposite directions, large & slow (not busy)
    5. four light shafts from above, gently swaying
    6. two ambient color blobs (coral + cyan) for warmth vs cool
    7. floating motes drifting up, soft
    8. surface glare cap (optional .no-caustics hides 2-7)
-->
<div class="ps-water" aria-hidden="true">
	<!-- SVG turbulence — feeds the .ps-rip layers so the caustics distort, not just slide.
	     Kept as a single global filter; the layers swap the displacement on/off via .no-caustics. -->
	<svg class="ps-turb-svg" width="0" height="0" aria-hidden="true">
		<defs>
			<filter id="ps-turbulence" x="0%" y="0%" width="100%" height="100%">
				<feTurbulence
					id="ps-sea-filter"
					type="fractalNoise"
					numOctaves="3"
					seed="2"
					baseFrequency="0.015 0.04"
				/>
				<feDisplacementMap scale="12" in="SourceGraphic" />
			</filter>
		</defs>
	</svg>

	<!-- caustic ripple layers — two sizes, opposite drift directions -->
	<div class="ps-rip a"></div>
	<div class="ps-rip b"></div>

	<!-- glow blobs — luminous underwater light pools, warm + cool so the pool isn't monochrome -->
	<div class="ps-blob coral"></div>
	<div class="ps-blob blue"></div>
	<div class="ps-blob sun-ray"></div>

	<!-- light shafts from the surface — long blurred beams, alternating tilt so they feel natural -->
	<div class="ps-shaft" style="left:8vw;transform:rotate(8deg);width:22vw;opacity:.85"></div>
	<div class="ps-shaft" style="left:36vw;transform:rotate(-5deg);width:18vw;opacity:.65"></div>
	<div class="ps-shaft" style="left:62vw;transform:rotate(10deg);width:24vw;opacity:.75"></div>
	<div class="ps-shaft" style="left:85vw;transform:rotate(-3deg);width:16vw;opacity:.55"></div>

	<!-- waterline highlight band — the bright reflective rim that reads as "water surface" -->
	<div class="ps-waterline"></div>

	<!-- surface glare at the top — the broader, softer hot spots from sky reflection -->
	<div class="ps-glare"></div>

	<!-- floating light motes — handful is plenty; more starts to feel noisy -->
	<div class="ps-mote" style="left:15%;top:20%;animation-delay:0s;width:4px;height:4px"></div>
	<div class="ps-mote" style="left:42%;top:55%;animation-delay:3s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:68%;top:35%;animation-delay:6s;width:5px;height:5px"></div>
	<div class="ps-mote" style="left:28%;top:70%;animation-delay:9s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:78%;top:15%;animation-delay:12s;width:4px;height:4px"></div>
	<div class="ps-mote" style="left:55%;top:80%;animation-delay:4.5s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:8%;top:60%;animation-delay:7.5s;width:4px;height:4px"></div>
</div>
