<script lang="ts">
	import { flip } from 'svelte/animate';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { InfinityIcon } from '@hugeicons/core-free-icons';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import * as api from '$lib/api';
	import { queueBlocks, type QueueRow } from '$lib/queue';
	import { playback, openAddToPlaylist } from '$lib/player.svelte';
	import { lt } from '$lib/lt.svelte';

	// Guests are add-only in a session — no removing or reordering (theirs or anyone's). The
	// playing row can't be removed either (backend guards it too).
	const canRemove = $derived(lt.role !== 'guest');
	const canReorder = $derived(lt.role !== 'guest');

	// Pointer-based drag-to-reorder (absolute queue indices). Upcoming rows only: the playing
	// row is neither draggable nor a drop target (the backend enforces the same rule).
	//
	// Deliberately NOT HTML5 drag-and-drop: WebView2 reliably fires `dragstart` (you can pick
	// a row up) but the `dragover`/`drop` chain frequently never lands, so nothing could ever
	// be dropped anywhere. A pointer drag has no browser drag session at all — press, move
	// past a small threshold, release on a row. The source row captures the pointer, the row
	// under the cursor is found via elementFromPoint, and a click is swallowed after a real
	// drag so releasing doesn't also play the song.
	let dragFrom: number | null = $state(null);
	let dragOver: number | null = $state(null);
	let dragging = $state(false);
	let pressIndex: number | null = null;
	let pressX = 0;
	let pressY = 0;
	let swallowClick = false;

	const DRAG_THRESHOLD_PX = 6;

	function onRowPointerDown(e: PointerEvent, i: number) {
		if (e.button !== 0 || i === playback.queue.currentIndex) return;
		pressIndex = i;
		pressX = e.clientX;
		pressY = e.clientY;
		(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
	}

	function onRowPointerMove(e: PointerEvent, i: number) {
		if (pressIndex === null) return;
		if (!dragging) {
			const dx = e.clientX - pressX;
			const dy = e.clientY - pressY;
			if (dx * dx + dy * dy < DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX) return; // still a click
			dragging = true;
			dragFrom = pressIndex;
			e.preventDefault();
		}
		// The source row captured the pointer, so locate the hovered row manually.
		const row = (document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null)?.closest(
			'[data-queue-row]'
		) as HTMLElement | null;
		const idx = row ? Number(row.dataset.queueRow) : null;
		dragOver =
			idx !== null && idx !== dragFrom && idx !== playback.queue.currentIndex ? idx : null;
	}

	function onRowPointerUp(e: PointerEvent, i: number) {
		if (pressIndex === null) return;
		const wasDrag = dragging;
		const from = dragFrom;
		resetDrag();
		if (!wasDrag) return; // plain click — TrackRow's onclick plays
		e.preventDefault();
		swallowClick = true; // kill the click that follows a real drag
		setTimeout(() => (swallowClick = false), 300);
		const row = (document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null)?.closest(
			'[data-queue-row]'
		) as HTMLElement | null;
		const idx = row ? Number(row.dataset.queueRow) : null;
		if (from === null || idx === null || idx === from || idx === playback.queue.currentIndex)
			return;
		// Backend semantics are remove(from) + insert(to) — "take the target row's slot" in
		// both directions (adjacent drops read as a swap, which is what users expect).
		api.moveQueueItem(from, idx);
	}

	function resetDrag() {
		dragging = false;
		dragFrom = null;
		dragOver = null;
		pressIndex = null;
	}

	// Runs in the capture phase (before TrackRow's bubble handler), so a release that ended a
	// drag doesn't also play the song.
	function swallowPostDragClick(e: MouseEvent) {
		if (!swallowClick) return;
		swallowClick = false;
		e.preventDefault();
		e.stopPropagation();
	}

	// Blocks in play order, cut wherever the upcoming tracks change origin (`queue.ts`).
	const view = $derived(queueBlocks(playback.queue));
</script>

{#snippet rows(list: QueueRow[])}
	{#each list as { item, key, i, n } (key)}
		<div
			animate:flip={{ duration: 200, easing: cubicOut }}
			role="listitem"
			data-queue-row={i}
			onpointerdown={(e) => onRowPointerDown(e, i)}
			onpointermove={(e) => onRowPointerMove(e, i)}
			onpointerup={(e) => onRowPointerUp(e, i)}
			onpointercancel={resetDrag}
			onclickcapture={swallowPostDragClick}
			class={[
				'select-none touch-pan-y',
				canReorder && i !== playback.queue.currentIndex ? 'cursor-grab' : '',
				dragging && dragFrom === i ? 'cursor-grabbing opacity-60' : '',
				dragOver === i && dragFrom !== i ? 'rounded-md bg-muted/40 ring-1 ring-primary/60' : ''
			].join(' ')}
		>
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

<!-- The list on its own, so the side panel and the now-playing view's Queue tab render the same
     one instead of drifting apart. -->
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
