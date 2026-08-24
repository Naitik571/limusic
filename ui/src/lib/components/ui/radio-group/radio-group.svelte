<script lang="ts">
	import { cn } from "$lib/utils.js";

	// Self-contained radio group root (the fork pins bits-ui 2.x, which has no RadioGroup
	// primitive). The playlist page owns `sortMode` and toggles it on each item's button, so the
	// root just needs to be a `role="radiogroup"` container — no binding machinery required.
	let {
		class: className,
		children,
		...restProps
	}: { class?: string; children?: import("svelte").Snippet; [k: string]: unknown } = $props();
</script>

<div data-slot="radio-group" role="radiogroup" class={cn("grid gap-0", className)} {...restProps}>
	{#if children}{@render children()}{/if}
</div>
