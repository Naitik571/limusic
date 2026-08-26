<script lang="ts">
	// Skeuomorphic picture-disc vinyl — reference-material pass.
	// .spin wraps what rotates (art + grooves); sheen spins too; label-ring + spindle stay put.
	// Spin/pause animation lives in poolside.css (global scope).
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
	<!-- everything that rotates -->
	<div class="spin">
		<div class="art"></div>
		<div class="grooves"></div>
	</div>
	<!-- sheens rotate too -->
	<div class="sheen-a"></div>
	<div class="sheen-b"></div>
	<!-- these stay put -->
	<div class="label-ring"></div>
	<div class="spindle"></div>
</div>
