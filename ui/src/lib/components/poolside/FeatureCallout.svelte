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
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';

	// Durations mirror the shell: visible window matches the shell's 3600ms
	// callout timer, entrance matches its 260ms scale-in.
	let { text, sectionId, duration = 3600 }: { text: string; sectionId: string; duration?: number } =
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
		data-section={sectionId}
		transition:fly={{ y: -10, duration: 260, easing: cubicOut }}
	>
		<span class="ps-callout-halo" aria-hidden="true"></span>
		<span class="ps-callout-text">{text}</span>
	</div>
{/if}

<style>
	.ps-callout {
		position: absolute;
		top: 84px;
		left: 0;
		right: 0;
		display: flex;
		justify-content: center;
		z-index: 30;
		pointer-events: none;
		padding: 0 16px;
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
		max-width: min(520px, 90vw);
		text-align: center;
		text-wrap: balance;
		font-family: var(--serif, Georgia, serif);
		font-style: italic;
		font-size: var(--ps-fs-lg, 18px);
		font-weight: 600;
		letter-spacing: 0.01em;
		color: var(--red, #ff5050);
		text-shadow: 0 0 18px rgba(255, 70, 70, 0.7), 0 0 32px rgba(255, 70, 70, 0.4),
			0 2px 0 rgba(0, 0, 0, 0.4);
	}
</style>
