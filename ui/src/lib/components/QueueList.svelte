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
	// past a small threshold, release on a row.
	//
	// No setPointerCapture: capture would redirect the browser-synthesized click to this
	// wrapper and silently kill TrackRow's own onclick (clicking a song would stop playing
	// it). Instead a press arms window-level move/up/cancel listeners, the hovered row is
	// found via elementFromPoint, and a click is swallowed only when a real drag ends where
	// it started (the browser still synthesizes a click there).
	let dragFrom: number | null = $state(null);
	let dragOver: number | null = $state(null);
	let dragging = $state(false);
	let pressIndex: number | null = null;
	let pressX = 0;
	let pressY = 0;
	let swallowClick = false;
	let detachPress: (() => void) | null = null;

	const DRAG_THRESHOLD_PX = 6;

	function onRowPointerDown(e: PointerEvent, i: number) {
		if (e.button !== 0 || i <= playback.queue.currentIndex) return; // past + playing rows don't drag
		pressIndex = i;
		pressX = e.clientX;
		pressY = e.clientY;
		attachPressListeners();
	}

	function attachPressListeners() {
		if (detachPress) return;
		const move = (e: PointerEvent) => {
			if (pressIndex === null) return;
			if (!dragging) {
				const dx = e.clientX - pressX;
				const dy = e.clientY - pressY;
				if (dx * dx + dy * dy < DRAG_THRESHOLD_PX * DRAG_THRESHOLD_PX) return; // still a click
				dragging = true;
				dragFrom = pressIndex;
				e.preventDefault();
			}
			const row = (document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null)?.closest(
				'[data-queue-row]'
			) as HTMLElement | null;
			const idx = row ? Number(row.dataset.queueRow) : null;
			dragOver =
			idx !== null && idx !== dragFrom && idx > playback.queue.currentIndex ? idx : null;
		};
		const up = (e: PointerEvent) => {
			if (pressIndex === null) return;
			const wasDrag = dragging;
			const from = dragFrom;
			detachPressListeners();
			resetDrag();
			if (!wasDrag) return; // plain click — TrackRow's own onclick plays
			e.preventDefault();
			swallowClick = true; // kill the click a same-row release synthesizes
			setTimeout(() => (swallowClick = false), 300);
			const row = (document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null)?.closest(
				'[data-queue-row]'
			) as HTMLElement | null;
			const idx = row ? Number(row.dataset.queueRow) : null;
			if (from === null || idx === null || idx === from || idx <= playback.queue.currentIndex)
			return;
			// Backend semantics are remove(from) + insert(to) — "take the target row's slot" in
			// both directions (adjacent drops read as a swap, which is what users expect).
			api.moveQueueItem(from, idx);
		};
		const cancel = () => {
			detachPressListeners();
			resetDrag();
		};
		// Capture phase: run ahead of anything in the app. Also cancel on leaving the window
		// or losing focus mid-press, so a release outside the app can't leave a stray drag.
		window.addEventListener('pointermove', move, true);
		window.addEventListener('pointerup', up, true);
		window.addEventListener('pointercancel', cancel, true);
		window.addEventListener('pointerleave', cancel, true);
		window.addEventListener('blur', cancel, true);
		detachPress = () => {
			window.removeEventListener('pointermove', move, true);
			window.removeEventListener('pointerup', up, true);
			window.removeEventListener('pointercancel', cancel, true);
			window.removeEventListener('pointerleave', cancel, true);
			window.removeEventListener('blur', cancel, true);
			detachPress = null;
		};
	}

	function detachPressListeners() {
		detachPress?.();
	}

	function resetDrag() {
			dragging = false;
			dragFrom = null;
			dragOver = null;
			pressIndex = null;
		}

		// ——— Past-song peek —————————————————————————————————————————
		// The queue starts at "Now playing"; songs you've already heard sit hidden above it.
		// Scrolling up from the top reveals them, the most recent four first; a further flick
		// while at the top reveals four more, all the way back to the start of the queue.
		// Nothing below the current row changes — that's the "what's coming next" view, kept
		// exactly as it was.
		let pastShown = $state(0);
		const PAST_CHUNK = 4;
		let scroller: HTMLElement | undefined = $state();

		// Most-recent-first: the row that plays next (currentIndex - 1) is the deepest in the
		// block, adjacent to "Now playing"; older ones stack towards the top.
		const pastRows = $derived.by(() => {
			const count = Math.min(pastShown, playback.queue.currentIndex);
			const out: QueueRow[] = [];
			for (let j = 0; j < count; j++) {
				const idx = playback.queue.currentIndex - 1 - j;
				out.push({ key: `past-${idx}`, item: playback.queue.items[idx], i: idx, n: idx + 1 });
			}
			return out.toReversed(); // oldest first — new chunks grow above, none must ever move
		});

		// Any queue reshuffle (new queue, clear, reorder) drops the peek: back to "what's next".
		$effect(() => {
			playback.queue.items; // tracked by identity only — index advances don't re-run this
			if (pastShown > 0) pastShown = 0;
		});

		function onQueueWheel(e: WheelEvent) {
			if (e.deltaY >= 0) return; // only scrolling up reaches into the past
			if (!scroller || scroller.scrollTop > 0) return; // only from the top of the list
			const total = playback.queue.currentIndex;
			if (total <= 0 || pastShown >= total) return;
			pastShown = Math.min(pastShown + PAST_CHUNK, total);
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

{#snippet rows(list: QueueRow[], past = false)}
	{#each list as { item, key, i, n } (key)}
		<div
			animate:flip={{ duration: 200, easing: cubicOut }}
			role="listitem"
			data-queue-row={i}
			onpointerdown={past ? undefined : (e) => onRowPointerDown(e, i)}
			onclickcapture={past ? undefined : swallowPostDragClick}
			class={[
				'select-none touch-pan-y',
				!past && canReorder && i !== playback.queue.currentIndex ? 'cursor-grab' : '',
				!past && dragging && dragFrom === i ? 'cursor-grabbing opacity-60' : '',
				!past && dragOver === i && dragFrom !== i ? 'rounded-md bg-muted/40 ring-1 ring-primary/60' : ''
			].join(' ')}
		>
			<TrackRow
				song={item}
				index={n - 1}
				active={i === playback.queue.currentIndex}
				onplay={() => api.playIndex(i)}
				onAdd={() => openAddToPlaylist(item)}
				onRemove={!past && canRemove && i !== playback.queue.currentIndex
					? () => api.removeFromQueue(i)
					: undefined}
				removeLabel={past ? undefined : 'Remove from queue'}
			/>
		</div>
	{/each}
{/snippet}

<!-- The list on its own, so the side panel and the now-playing view's Queue tab render the same
     one instead of drifting apart. -->
<div bind:this={scroller} onwheel={onQueueWheel} class="min-h-0 flex-1 overflow-y-auto p-2">
	{#if pastRows.length > 0}
		<h3 class="px-2 pt-2 pb-1.5 text-sm font-semibold">Previously played</h3>
		{@render rows(pastRows, true)}
	{/if}
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
