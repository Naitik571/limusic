<script lang="ts">
	// Skeuomorphic picture-disc vinyl, ported from the reference mockups:
	// art + dark tint stacked into the background, static ::before grooves,
	// conic sheen that spins with the record, layered spindle.
	// `playing` drives the rotation via the .playing class.
	let {
		src,
		playing = false,
		style = '',
		title,
		onclick,
		flightTarget = false
	}: {
		src: string;
		playing?: boolean;
		style?: string;
		title?: string;
		onclick?: (e: MouseEvent) => void;
		flightTarget?: boolean;
	} = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
	class="ps-vinyl {playing ? 'playing' : ''}"
	style="--art:url('{src}');{style}"
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
	<div class="sheen"></div>
	<div class="spindle"></div>
</div>
