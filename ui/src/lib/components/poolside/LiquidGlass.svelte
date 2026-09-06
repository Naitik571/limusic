<!--
  Liquid-glass SVG filters (BlazePod technique): a fractal-noise displacement wobbles the
  edge of whatever the backdrop blur samples, so glass sheets read as liquid instead of
  frosted. `sheet` (scale 16) for large panels, `bar` (scale 8) for the pill. Zero-size,
  mounted once in the shell; surfaces opt in via CSS. Skipped under reduce-motion.
-->
<svg class="ps-liquid-defs" width="0" height="0" aria-hidden="true" focusable="false">
	<defs>
		<filter
			id="ps-liquid-sheet"
			x="-20%"
			y="-20%"
			width="140%"
			height="140%"
			color-interpolation-filters="sRGB"
		>
			<feTurbulence
				type="fractalNoise"
				baseFrequency="0.012 0.02"
				numOctaves="2"
				seed="7"
				result="warp"
			/>
			<feDisplacementMap in="SourceGraphic" in2="warp" scale="16" xChannelSelector="R" yChannelSelector="G" />
		</filter>
		<filter
			id="ps-liquid-bar"
			x="-20%"
			y="-20%"
			width="140%"
			height="140%"
			color-interpolation-filters="sRGB"
		>
			<feTurbulence
				type="fractalNoise"
				baseFrequency="0.015 0.025"
				numOctaves="2"
				seed="3"
				result="warp"
			/>
			<feDisplacementMap in="SourceGraphic" in2="warp" scale="8" xChannelSelector="R" yChannelSelector="G" />
		</filter>
	</defs>
</svg>

<style>
	.ps-liquid-defs {
		position: absolute;
		width: 0;
		height: 0;
		overflow: hidden;
	}
</style>
