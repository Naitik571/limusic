<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';

	// One empty state for every surface (interior #14): icon in a soft disc, one line of copy,
	// an optional hint, an optional action. Surfaces pass their own icon + words; layout never
	// drifts between library, queue, history and downloads again.
	let {
		icon,
		line,
		hint = '',
		actionLabel = '',
		onAction
	}: {
		icon: any;
		line: string;
		hint?: string;
		actionLabel?: string;
		onAction?: () => void;
	} = $props();
</script>

<div class="flex flex-col items-center gap-3 py-16 text-center">
	<span
		class="flex h-14 w-14 items-center justify-center rounded-full"
		style="background:var(--surface-2);color:var(--text-3)"
	>
		<HugeiconsIcon {icon} strokeWidth={2} class="h-7 w-7" />
	</span>
	<p class="max-w-sm text-sm" style="color:var(--text-2)">{line}</p>
	{#if hint}
		<p class="max-w-xs text-xs" style="color:var(--text-3)">{hint}</p>
	{/if}
	{#if actionLabel && onAction}
		<button
			onclick={onAction}
			class="mt-1 cursor-pointer rounded-full px-4 py-1.5 text-sm font-medium transition-transform"
			style="background:var(--primary);color:var(--primary-foreground);border-radius:var(--r-full);transition-duration:var(--dur-2);transition-timing-function:var(--ease-out)"
		>
			{actionLabel}
		</button>
	{/if}
</div>
