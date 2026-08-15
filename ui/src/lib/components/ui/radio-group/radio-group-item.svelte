<script lang="ts">
	import { cn } from "$lib/utils.js";

	// Visual radio indicator. The playlist page owns `sortMode` and toggles it; this only draws
	// the dot. (The fork pins bits-ui 2.x, which has no RadioGroup primitive, so we keep it tiny.)
	let {
		checked = false,
		class: className,
		...restProps
	}: { checked?: boolean; class?: string; [k: string]: unknown } = $props();
</script>

<span
	data-slot="radio-group-item"
	role="radio"
	aria-checked={checked}
	class={cn(
		"border-input aspect-square size-4 shrink-0 rounded-full border shadow-xs",
		checked && "border-primary",
		className
	)}
	{...restProps}
>
	{#if checked}
		<span class="flex h-full w-full items-center justify-center">
			<span class="size-2 rounded-full bg-primary"></span>
		</span>
	{/if}
</span>
