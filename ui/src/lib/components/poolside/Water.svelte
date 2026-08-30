<!--
  Poolside water background — deep blue vinyl-pool with koi swimming through it.

  Stack (back -> front, all under .ps-root):
    1. base gradient (surface teal -> deep navy) gives the pool its depth
    2. tile-grid floor, perspective-tilted so the back rows look further away
    3. waterline highlight at the top (drifts horizontally — the bright rim that
       reads as "water surface" in motion, like sky reflection moving)
    4. two caustic layers drifting in opposite directions, large & slow
    5. four light shafts from above, gently swaying
    6. two ambient color blobs (coral + cyan)
    7. four koi fish swimming across the pool at different depths
    8. floating motes drifting up, soft
    9. surface glare cap
    10. SVG turbulence filter — displacement for the caustics
-->
<script lang="ts">
	import Koi from './Koi.svelte';
</script>

<div class="ps-water" aria-hidden="true">
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

	<div class="ps-rip a"></div>
	<div class="ps-rip b"></div>

	<div class="ps-blob coral"></div>
	<div class="ps-blob blue"></div>
	<div class="ps-blob sun-ray"></div>

	<div class="ps-shaft" style="left:8vw;transform:rotate(8deg);width:22vw;opacity:.85"></div>
	<div class="ps-shaft" style="left:36vw;transform:rotate(-5deg);width:18vw;opacity:.65"></div>
	<div class="ps-shaft" style="left:62vw;transform:rotate(10deg);width:24vw;opacity:.75"></div>
	<div class="ps-shaft" style="left:85vw;transform:rotate(-3deg);width:16vw;opacity:.55"></div>

	<div class="ps-waterline"></div>
	<div class="ps-glare"></div>

	<!-- Koi — four fish at different depths, speeds, colors. Paths are keyframed in CSS;
	     the path itself does a long S-curve and flips at the midpoint so the fish turns
	     around at the edge of the pool. -->
	<div class="ps-koi a"><div class="body"><Koi color="#F4A078" size={68} /></div></div>
	<div class="ps-koi b"><div class="body"><Koi color="#F8C9A4" size={56} /></div></div>
	<div class="ps-koi c"><div class="body"><Koi color="#E07856" size={48} /></div></div>
	<div class="ps-koi d"><div class="body"><Koi color="#FFD8B8" size={42} /></div></div>

	<div class="ps-mote" style="left:15%;top:20%;animation-delay:0s;width:4px;height:4px"></div>
	<div class="ps-mote" style="left:42%;top:55%;animation-delay:3s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:68%;top:35%;animation-delay:6s;width:5px;height:5px"></div>
	<div class="ps-mote" style="left:28%;top:70%;animation-delay:9s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:78%;top:15%;animation-delay:12s;width:4px;height:4px"></div>
	<div class="ps-mote" style="left:55%;top:80%;animation-delay:4.5s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:8%;top:60%;animation-delay:7.5s;width:4px;height:4px"></div>
</div>
