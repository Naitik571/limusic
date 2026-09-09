<script lang="ts">
import { flip } from 'svelte/animate';
import { cubicOut } from 'svelte/easing';
import { onDestroy } from 'svelte';
import { HugeiconsIcon } from '@hugeicons/svelte';
import { InfinityIcon } from '@hugeicons/core-free-icons';
import TrackRow from '$lib/components/TrackRow.svelte';
import RadioMoods from '$lib/components/RadioMoods.svelte';
import * as api from '$lib/api';
import { queueBlocks, type QueueRow } from '$lib/queue';
import { isSwipe, shouldRemove } from '$lib/swipe';
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
	let pressWidth = 0;
	let swallowClick = false;
	let detachPress: (() => void) | null = null;

	// Horizontal swipe-to-remove shares the press with reorder: whichever axis dominates
	// first owns the gesture (swipe = remove, vertical = reorder). Same guards as the
	// remove button — guests and the playing row can't swipe anything away.
	let swipeIdx: number | null = $state(null);
	let swipedX = $state(0);
	let swiping = $state(false);
	let removingIdx: number | null = $state(null);
	let removingDir = $state(1);

	const DRAG_THRESHOLD_PX = 6;

	function onRowPointerDown(e: PointerEvent, i: number) {
		if (e.button !== 0 || i <= playback.queue.currentIndex) return; // past + playing rows don't drag
		pressIndex = i;
		pressX = e.clientX;
		pressY = e.clientY;
		pressWidth =
			(e.currentTarget as HTMLElement | null)?.clientWidth ??
			(e.target as HTMLElement)?.closest?.('[data-queue-row]')?.clientWidth ??
			300;
		attachPressListeners();
	}

	function attachPressListeners() {
		if (detachPress) return;
		const move = (e: PointerEvent) => {
			if (pressIndex === null) return;
			const dx = e.clientX - pressX;
			const dy = e.clientY - pressY;
			// Swipe arbitration runs before reorder: a dominant horizontal press becomes a
			// removal swipe and reorder never arms for it. Non-removable rows (guests) fall
			// through to the old reorder path untouched.
			if (!dragging && !swiping && canRemove && isSwipe(dx, dy)) {
				swiping = true;
				swipeIdx = pressIndex;
				swipedX = dx;
				e.preventDefault();
				return;
			}
			if (swiping) {
				swipedX = dx;
				e.preventDefault();
				return;
			}
			// A reorder already in flight converts to a swipe the moment the gesture turns
			// unambiguously horizontal: reorder arms at 6px, swipe needs 12, so without this
			// every horizontal drag would read as a reorder first and swiping could never win.
			if (dragging && canRemove && isSwipe(dx, dy)) {
				dragging = false;
				dragFrom = null;
				dragOver = null;
				swiping = true;
				swipeIdx = pressIndex;
				swipedX = dx;
				e.preventDefault();
				return;
			}
			if (!dragging) {
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
			// A swipe ends here: past the distance it flies off and removes; short of it
			// the row snaps back. Either way the release never plays the song.
			if (swiping) {
				const idx = swipeIdx;
				const dx = e.clientX - pressX;
				const w = pressWidth;
				detachPressListeners();
				resetDrag();
				swallowClick = true; // kill the click a same-row release synthesizes
				setTimeout(() => (swallowClick = false), 300);
				if (idx !== null && shouldRemove(dx, w)) {
					// Resolve by videoId at fire time, not release time: a skip/reorder
					// inside the 200ms fly-off would otherwise delete the wrong row.
					const vid = playback.queue.items[idx]?.video_id;
					removingIdx = idx;
					removingDir = dx < 0 ? -1 : 1;
					setTimeout(() => {
						removingIdx = null;
						const live = vid
							? playback.queue.items.findIndex(
									(it, j) => j > playback.queue.currentIndex && it.video_id === vid
								)
							: -1;
						if (live >= 0) api.removeFromQueue(live);
					}, 200);
				}
				return;
			}
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

	// Unmount mid-press (navigation while dragging): drop the window listeners so a later
	// pointerup can't fire a reorder/remove against a dead press.
	onDestroy(() => {
		detachPressListeners();
	});

	function resetDrag() {
			dragging = false;
			dragFrom = null;
			dragOver = null;
			pressIndex = null;
			swiping = false;
			swipeIdx = null;
			swipedX = 0;
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

		// A reshuffle (new queue, clear, reorder — anything that changes the items themselves)
		// drops the peek: back to "what's next". Position advances must NOT reset it, so the
		// fingerprint keys on the tracks' video_ids, not on `queue` identity — the backend
		// swaps the whole queue object on every event, which would otherwise wipe the peek
		// every time the song changes.
		let lastPeekFp = '';
		$effect(() => {
			const fp = playback.queue.items.map((i) => i.video_id).join('\u0000');
			if (fp !== lastPeekFp) pastShown = 0;
			lastPeekFp = fp;
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
		{@const isSwiping = !past && swiping && swipeIdx === i}
		{@const isRemoving = !past && removingIdx === i}
		<div
			animate:flip={{ duration: 200, easing: cubicOut }}
			role="listitem"
			data-queue-row={i}
			style="content-visibility: auto; contain-intrinsic-size: auto 3.5rem;"
			onpointerdown={past ? undefined : (e) => onRowPointerDown(e, i)}
			onclickcapture={past ? undefined : swallowPostDragClick}
			class={[
				'relative select-none touch-pan-y overflow-hidden rounded-md',
				!past && canReorder && i !== playback.queue.currentIndex ? 'cursor-grab' : '',
				!past && dragging && dragFrom === i ? 'cursor-grabbing opacity-60' : '',
				!past && dragOver === i && dragFrom !== i ? 'rounded-md bg-muted/40 ring-1 ring-primary/60' : ''
			].join(' ')}
		>
			{#if isSwiping}
				<!-- Red underlay revealed by the swipe; icon sits on the vacated side. -->
				<div
					class="absolute inset-0 flex items-center bg-red-500/90 px-4 {swipedX < 0
						? 'justify-end'
						: 'justify-start'}"
					aria-hidden="true"
				>
					<svg viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="2" class="h-4 w-4"><path d="M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2m-9 0 1 13h8l1-13" stroke-linecap="round" stroke-linejoin="round"/></svg>
				</div>
			{/if}
			<div
				class="relative rounded-md {isSwiping ? 'bg-background' : ''}"
				style={isRemoving
					? `transform: translateX(${removingDir * 120}%); transition: transform .2s ease-in, opacity .2s; opacity: 0;`
					: isSwiping
						? `transform: translateX(${swipedX}px); transition: none;`
						: 'transition: transform .25s ease-out;'}
			>
			<TrackRow
				song={item}
				index={n - 1}
				active={i === playback.queue.currentIndex}
				draggable={false}
				onplay={() => api.playIndex(i)}
				onAdd={() => openAddToPlaylist(item)}
				onRemove={!past && canRemove && i !== playback.queue.currentIndex
					? () => api.removeFromQueue(i)
					: undefined}
				removeLabel={past ? undefined : 'Remove from queue'}
			/>
			</div>
		</div>
	{/each}
{/snippet}

<!-- The list on its own, so the side panel and the now-playing view's Queue tab render the same
     one instead of drifting apart. -->
<div bind:this={scroller} onwheel={onQueueWheel} class="min-h-0 flex-1 overflow-y-auto p-2">
	<!-- Filter + bulk actions. Filtering is display-only (absolute indices kept), Shuffle rest
	     randomizes the upcoming tracks in place, Clear played drops the played prefix. -->
	{#if playback.queue.currentIndex > 0 && canRemove}
		<div class="flex items-center gap-1.5 px-2 pt-1 pb-2">
			<button
				class="h-7 shrink-0 cursor-pointer rounded-md px-2 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent/10 hover:text-foreground"
				title="Drop every played track"
				onclick={() => api.clearPlayed()}
			>
				Clear played
			</button>
		</div>
	{/if}
	{#if pastRows.length > 0}
		<h3 class="px-2 pt-2 pb-1.5 text-sm font-semibold">Previously played</h3>
		{@render rows(pastRows, true)}
	{/if}
	<RadioMoods moods={playback.queue.radioMoods} />
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
