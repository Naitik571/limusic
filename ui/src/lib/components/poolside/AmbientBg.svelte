<!--
  Ambient video background — the BlazePod water videos (aqua/verdant/goldfish), vendored
  into $lib/assets/ambient. Replaces the procedural Water for those three themes; `clear`
  and `night` keep the pool.

  Mirrors their behavior: muted loop playsinline, lazy src (assigned on first activation,
  never fetched for themes you don't visit), a still poster underneath for first paint,
  theme overlays for text legibility, and poster-only when `still` (reduce-motion).
-->
<script lang="ts">
	import aquaMp4 from '$lib/assets/ambient/aqua.mp4';
	import verdantMp4 from '$lib/assets/ambient/verdant.mp4';
	import goldfishMp4 from '$lib/assets/ambient/goldfish.mp4';
	import aquaJpg from '$lib/assets/ambient/aqua.jpg';
	import verdantJpg from '$lib/assets/ambient/verdant.jpg';
	import goldfishJpg from '$lib/assets/ambient/goldfish.jpg';

	let {
		theme,
		still = false
	}: { theme: 'aqua' | 'verdant' | 'goldfish'; still?: boolean } = $props();

	const VID = { aqua: aquaMp4, verdant: verdantMp4, goldfish: goldfishMp4 };
	const POSTER = { aqua: aquaJpg, verdant: verdantJpg, goldfish: goldfishJpg };

	let video = $state<HTMLVideoElement | undefined>();

	$effect(() => {
		const v = video;
		const want = VID[theme];
		if (!v) return;
		if (still) {
			v.pause();
			v.removeAttribute('src');
			v.load();
			return;
		}
		if (v.getAttribute('src') !== want) {
			v.src = want;
			v.load();
		}
		v.play().catch(() => {
			/* codec/autoplay block — the poster underneath still paints */
		});
	});
</script>

<div class="ps-ambient ps-ambient--{theme}" aria-hidden="true">
	<img class="ps-ambient-poster" src={POSTER[theme]} alt="" draggable="false" decoding="async" />
	{#if !still}
		<video
			bind:this={video}
			class="ps-ambient-video"
			muted
			loop
			playsinline
			preload="auto"
			tabindex="-1"
			disablepictureinpicture
			controlslist="nodownload noremoteplayback nofullscreen noplaybackrate"
			oncontextmenu={(e) => e.preventDefault()}
		></video>
	{/if}
	<div class="ps-ambient-overlay"></div>
	{#if theme === 'goldfish'}
		<div class="ps-ambient-grain"></div>
	{/if}
</div>
