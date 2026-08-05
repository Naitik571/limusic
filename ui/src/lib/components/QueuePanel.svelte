<script lang="ts">
	import { fade, fly } from 'svelte/transition';
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { InfinityIcon } from '@hugeicons/core-free-icons';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import * as api from '$lib/api';
	import { queueBlocks, type QueueRow } from '$lib/queue';
	import { playback, openAddToPlaylist } from '$lib/player.svelte';
	import { lt } from '$lib/lt.svelte';

	let { onClose }: { onClose: () => void } = $props();

	// Guests are add-only in a session — no removing (theirs or anyone's). The playing row can't
	// be removed either (backend guards it too).
	const canRemove = $derived(lt.role !== 'guest');

	// Blocks in play order, cut wherever the upcoming tracks change origin (`queue.ts`).
	const view = $derived(queueBlocks(playback.queue));
</script>

{#snippet rows(list: QueueRow[])}
	{#each list as { item, key, i, n } (key)}
		<div animate:flip={{ duration: 200, easing: cubicOut }}>
			<TrackRow
				song={item}
				index={n - 1}
				active={i === playback.queue.currentIndex}
				onplay={() => api.playIndex(i)}
				onAdd={() => openAddToPlaylist(item)}
				onRemove={canRemove && i !== playback.queue.currentIndex
					? () => api.removeFromQueue(i)
					: undefined}
				removeLabel="Remove from queue"
			/>
		</div>
	{/each}
{/snippet}

<!-- The panel always floats over the content (see the `relative` wrapper in +layout) rather than
     squeezing it into a column: two docked panels left the page too narrow to read, and a page you
     can't use behind a panel you opened on purpose is the better trade. Below lg a scrim dismisses
     it; at lg+ the content stays visible underneath and the player bar's button closes it. -->
<button
	class="absolute inset-0 z-20 cursor-default bg-black/40 lg:hidden"
	onclick={onClose}
	aria-label="Close queue"
	transition:fade={{ duration: 150 }}
></button>
<aside
	transition:fly={{ x: 32, duration: 220, easing: cubicOut }}
	class="absolute inset-y-0 right-0 z-30 flex h-full w-80 max-w-[80vw] flex-col border-l bg-card shadow-2xl"
>
	<h2 class="border-b px-4 py-3 font-heading text-sm font-semibold">Queue</h2>
	<div class="min-h-0 flex-1 overflow-y-auto p-2">
		{#if view.now}
			<h3 class="px-2 pt-2 pb-1.5 text-sm font-semibold">Now playing</h3>
			{@render rows([view.now])}

			{#each view.blocks as block (block.key)}
				{#if block.autoplay}
					<div
						class="mt-3 flex items-center gap-2 border-t px-2 pt-2.5 pb-1.5 text-muted-foreground"
						title="Autoplay keeps the music going with similar songs. Turn it off in Settings ▸ Playback."
					>
						<HugeiconsIcon icon={InfinityIcon} class="h-3.5 w-3.5" />
						<span class="text-xs font-medium">Autoplay</span>
						<span class="truncate text-xs">· similar music</span>
					</div>
				{:else}
					<div class="mt-3 flex items-center justify-between gap-2 px-2 pb-1.5">
						<h3 class="truncate text-sm font-semibold">{block.heading}</h3>
						{#if block.clearable && canRemove}
							<button
								class="shrink-0 cursor-pointer text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
								onclick={() => api.clearQueued()}
							>
								Clear queue
							</button>
						{/if}
					</div>
				{/if}
				{@render rows(block.rows)}
			{/each}
		{:else}
			<p class="p-4 text-sm text-muted-foreground">The queue is empty.</p>
		{/if}
	</div>
</aside>
