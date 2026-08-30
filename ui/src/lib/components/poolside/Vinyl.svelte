<script lang="ts">
	// Skeuomorphic picture-disc vinyl — sitting in a pool.
	// .spin wraps what rotates (art + grooves + label); sheens rotate too; spindle stays put.
	// Spin/pause animation lives in poolside.css (global scope).
	let {
		src,
		playing = false,
		style = '',
		title,
		onclick,
		flightTarget = false,
		size = 0
	}: {
		src: string;
		playing?: boolean;
		style?: string;
		title?: string;
		onclick?: (e: MouseEvent) => void;
		flightTarget?: boolean;
		/** Optional fixed pixel size — locks the diameter instead of letting the parent size it. */
		size?: number;
	} = $props();
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
	class="ps-vinyl {playing ? 'playing' : ''}"
	style="--art:url('{src}');{style}{size ? ` width:${size}px; height:${size}px;` : ''}"
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
	<!-- pool reflection under the disc — a soft blue bloom, like the disc is sitting on water -->
	<div class="pool-shadow" aria-hidden="true"></div>
	<!-- everything that rotates -->
	<div class="spin">
		<div class="art"></div>
		<div class="grooves"></div>
	</div>
	<!-- sheens rotate too — sells "the disc is reflecting the water surface" -->
	<div class="sheen-a"></div>
	<div class="sheen-b"></div>
	<!-- these stay put -->
	<div class="label-ring"></div>
	<div class="spindle"></div>
</div>
