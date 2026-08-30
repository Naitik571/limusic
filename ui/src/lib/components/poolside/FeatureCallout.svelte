<!--
  FeatureCallout — a glowing red serif-font announcement banner that fades in when
  a relevant section comes into view, sits over a soft blurred halo for readability,
  then fades out after a few seconds so it never blocks interaction.

  Usage:
    <FeatureCallout text="You can now use the radio!!!" sectionId="radio" />

  The component subscribes to a global "active callout" event. The shell calls
  `showCallout({ text, sectionId })` whenever a relevant view becomes active.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { fade, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';

	let { text, sectionId, duration = 3200 }: { text: string; sectionId: string; duration?: number } =
		$props();

	let visible = $state(true);
	let timer: ReturnType<typeof setTimeout> | null = null;

	onMount(() => {
		timer = setTimeout(() => (visible = false), duration);
		return () => {
			if (timer) clearTimeout(timer);
		};
	});
</script>

{#if visible}
	<div
		class="ps-callout"
		role="status"
		aria-live="polite"
		transition:fly={{ y: -10, duration: 360, easing: cubicOut }}
	>
		<span class="ps-callout-halo" aria-hidden="true"></span>
		<span class="ps-callout-text">{text}</span>
	</div>
{/if}

<style>
	.ps-callout {
		position: absolute;
		top: 84px;
		left: 50%;
		transform: translateX(-50%);
		z-index: 30;
		pointer-events: none;
	}
	.ps-callout-halo {
		position: absolute;
		inset: -22px -42px;
		background: radial-gradient(ellipse 50% 70% at 50% 50%, rgba(255, 60, 60, 0.32) 0%, transparent 70%);
		filter: blur(14px);
		pointer-events: none;
	}
	.ps-callout-text {
		position: relative;
		font-family: Georgia, serif;
		font-style: italic;
		font-size: 22px;
		font-weight: 600;
		letter-spacing: 0.01em;
		color: #ff5050;
		text-shadow: 0 0 18px rgba(255, 70, 70, 0.7), 0 0 32px rgba(255, 70, 70, 0.4),
			0 2px 0 rgba(0, 0, 0, 0.4);
		white-space: nowrap;
	}
</style>
