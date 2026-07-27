<script lang="ts">
	import { goto } from '$app/navigation';
	import type { ArtistRun } from '$lib/api';

	let {
		runs,
		text,
		class: cls = ''
	}: {
		/** Per-run artist links; when empty the line renders as plain `text`. */
		runs?: ArtistRun[];
		/** The flattened artist line, used as fallback and as the truncation source. */
		text: string;
		class?: string;
	} = $props();

	const linked = $derived(runs?.some((r) => r.id) ? runs! : undefined);
</script>

{#if linked}
	<span class="min-w-0 truncate {cls}">
		{#each linked as run}
			{#if run.id}
				<button
					class="cursor-pointer text-left hover:text-foreground hover:underline"
					onclick={(e) => {
						e.stopPropagation();
						goto(`/artist/${encodeURIComponent(run.id!)}`);
					}}
				>
					{run.text}
				</button>
			{:else}
				{run.text}
			{/if}
		{/each}
	</span>
{:else}
	<span class="min-w-0 truncate {cls}">{text}</span>
{/if}
