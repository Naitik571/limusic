<!-- Poolside water background: realistic swimming pool with SVG turbulence,
     light shafts, surface glare, drifting ripples, glow blobs, and floating motes. -->
<div class="ps-water" aria-hidden="true">
	<!-- SVG displacement filter for water distortion -->
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

	<!-- caustic ripple layers -->
	<div class="ps-rip a"></div>
	<div class="ps-rip b"></div>

	<!-- glow blobs -->
	<div class="ps-blob coral"></div>
	<div class="ps-blob blue"></div>
	<div class="ps-blob sun-ray"></div>

	<!-- light shafts from the surface -->
	<div class="ps-shaft" style="left:8vw;transform:rotate(8deg);width:22vw;opacity:.85"></div>
	<div class="ps-shaft" style="left:36vw;transform:rotate(-5deg);width:18vw;opacity:.65"></div>
	<div class="ps-shaft" style="left:62vw;transform:rotate(10deg);width:24vw;opacity:.75"></div>
	<div class="ps-shaft" style="left:85vw;transform:rotate(-3deg);width:16vw;opacity:.55"></div>

	<!-- surface glare at the top -->
	<div class="ps-glare"></div>

	<!-- floating light motes -->
	<div class="ps-mote" style="left:15%;top:20%;animation-delay:0s;width:4px;height:4px"></div>
	<div class="ps-mote" style="left:42%;top:55%;animation-delay:3s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:68%;top:35%;animation-delay:6s;width:5px;height:5px"></div>
	<div class="ps-mote" style="left:28%;top:70%;animation-delay:9s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:78%;top:15%;animation-delay:12s;width:4px;height:4px"></div>
	<div class="ps-mote" style="left:55%;top:80%;animation-delay:4.5s;width:3px;height:3px"></div>
	<div class="ps-mote" style="left:8%;top:60%;animation-delay:7.5s;width:4px;height:4px"></div>
</div>
