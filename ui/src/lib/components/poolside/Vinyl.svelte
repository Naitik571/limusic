<script lang="ts">
	// Skeuomorphic picture-disc vinyl. The sheen spins with the record; grooves + spindle sit
	// outside the spin so they stay crisp. `playing` drives the rotation. `flightTarget` marks
	// the disc as the shared-element landing zone (PoolsideShell flies the clicked cover in).
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
	{style}
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
	<div
		class="absolute inset-0"
		style="background-image:url('{src}');background-size:cover;background-position:center"
	></div>
	<!-- grooves over the art -->
	<div class="grooves" style="z-index:1"></div>
	<div class="sheen" style="z-index:2"></div>
	<div class="spindle" style="z-index:3"></div>
</div>
