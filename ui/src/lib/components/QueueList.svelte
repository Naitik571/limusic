<script lang="ts">
import { flip } from 'svelte/animate';
import { cubicOut } from 'svelte/easing';
import { onDestroy } from 'svelte';
import { goto } from '$app/navigation';
import { HugeiconsIcon } from '@hugeicons/svelte';
import { InfinityIcon, Queue01Icon } from '@hugeicons/core-free-icons';
import TrackRow from '$lib/components/TrackRow.svelte';
import RadioMoods from '$lib/components/RadioMoods.svelte';
import EmptyState from '$lib/components/EmptyState.svelte';
import * as api from '$lib/api';
import { openItem } from '$lib/browse';
import { queueBlocks, type QueueRow } from '$lib/queue';
import { isSwipe, shouldRemove } from '$lib/swipe';
import { burst } from '$lib/fx';
import { personal, playback, openAddToPlaylist, toast } from '$lib/player.svelte';
import { recentItems } from '$lib/personal';
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
					// Remove by absolute index, not by first videoId match: duplicates of the
					// same song would otherwise delete the wrong copy. The videoId is still
					// captured to verify the row didn't shift under the 200ms fly-off — only
					// then fall back to a videoId lookup.
					const vid = playback.queue.items[idx]?.video_id;
					removingIdx = idx;
					removingDir = dx < 0 ? -1 : 1;
					burst(e.clientX, e.clientY);
					setTimeout(() => {
						removingIdx = null;
						if (!vid) return; // idx is non-null here (outer guard); vid may be missing
						if (
							idx > playback.queue.currentIndex &&
							playback.queue.items[idx]?.video_id === vid
						) {
							api.removeFromQueue(idx);
							return;
						}
						const live = playback.queue.items.findIndex(
							(it, j) => j > playback.queue.currentIndex && it.video_id === vid
						);
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

		// A reshuffle (new queue, clear — anything that changes the items themselves)
		// drops the peek: back to "what's next". Position advances must NOT reset it, so the
		// fingerprint keys on the tracks' video_ids, not on `queue` identity — the backend
		// swaps the whole queue object on every event, which would otherwise wipe the peek
		// every time the song changes. Order-insensitive (sorted) plus length: our own
		// reorder keeps the same set and must not collapse the peek we just opened.
		let lastPeekFp = '';
		$effect(() => {
			const items = playback.queue.items;
			const fp = `${items.length}|${[...items].map((i) => i.video_id).sort().join(' ')}`;
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

	// Visible previously-played control (interior #5): the wheel-up peek stays, but
	// discoverability needs a button driving the same pastShown state. Toggles between
	// hidden and one chunk; deeper history is still a wheel-up away.
	function togglePast() {
		const total = playback.queue.currentIndex;
		if (total <= 0) return;
		pastShown = pastShown > 0 ? 0 : Math.min(PAST_CHUNK, total);
	}

	// Empty-state "play something" (interior #5): the most recent listen. Songs play in
	// place, anything else opens its page; with no history yet, head for search.
	function playRecentPick() {
		const [recent] = recentItems(personal, 1);
		if (!recent) {
			goto('/search');
			return;
		}
		openItem(recent);
	}

	// Clear-queue undo (interior #19): snapshot the manual rows clearQueued drops, then offer
	// them back for 6s. Undo re-adds ahead of the autoplay filler (addToQueue's contract),
	// which is where "Next in queue" rows live. Failures toast; an empty clear stays quiet.
	async function clearQueueUndoable() {
		const cleared = playback.queue.items
			.slice(playback.queue.currentIndex + 1)
			.filter((t) => t.queued || t.queued_end);
		try {
			await api.clearQueued();
		} catch (e) {
			toast.error(String(e));
			return;
		}
		if (!cleared.length) return;
		toast.action(`Cleared ${cleared.length} track${cleared.length === 1 ? '' : 's'}`, 'Undo', () => {
			api.addToQueue(cleared).catch((e) => toast.error(String(e)));
		});
	}

	// Runs in the capture phase (before TrackRow's bubble handler), so a release that ended a
	// drag doesn't also play the song.
	function swallowPostDragClick(e: MouseEvent) {
		if (!swallowClick) return;
		swallowClick = false;
		e.preventDefault();
		e.stopPropagation();
	}

	// Queue header "Refresh radio": re-seed the mix from the current track (dead radio rescue).
	let refreshingRadio = $state(false);
	async function refreshRadio() {
		if (refreshingRadio) return;
		refreshingRadio = true;
		try {
			const n = await api.refreshRadio();
			toast.success(`Radio refreshed — ${n} new track${n === 1 ? '' : 's'}`);
		} catch (e) {
			toast.error(String(e));
		} finally {
			refreshingRadio = false;
		}
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
					? () => {
							const r = document
								.querySelector(`[data-queue-row="${i}"]`)
								?.getBoundingClientRect();
							if (r) burst(r.left + r.width / 2, r.top + r.height / 2);
							api.removeFromQueue(i);
						}
					: undefined}
				removeLabel={past ? undefined : 'Remove from queue'}
				fadeNext={!past && i > playback.queue.currentIndex
					? (playback.queue.items[i + 1]?.video_id ?? null)
					: null}
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
	{#if pastRows.length > 0 || playback.queue.currentIndex > 0}
		<!-- Always visible while anything has played: the count says how deep the past goes, the
		     button drives pastShown (same state the wheel-up peek feeds). -->
		<div class="flex items-center justify-between gap-2 px-2 pt-2 pb-1.5">
			<h3 class="text-sm font-semibold">Previously played ({playback.queue.currentIndex})</h3>
			<button
				class="shrink-0 cursor-pointer rounded-md px-2 py-0.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-accent/10 hover:text-foreground"
				style="border-radius:var(--r-sm);transition-duration:var(--dur-1);transition-timing-function:var(--ease-out)"
				onclick={togglePast}
				aria-expanded={pastShown > 0}
			>
				{pastShown > 0 ? 'Hide' : 'Show'}
			</button>
		</div>
		{#if pastRows.length > 0}
			{@render rows(pastRows, true)}
		{/if}
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
					<button
						class="ml-auto shrink-0 cursor-pointer text-xs font-medium transition-colors hover:text-foreground disabled:opacity-50"
						disabled={refreshingRadio}
						title="Fetch a fresh mix from the current track"
						onclick={() => refreshRadio()}
					>
						{refreshingRadio ? 'Refreshing…' : 'Refresh radio'}
					</button>
				</div>
			{:else}
				<div class="mt-3 flex items-center justify-between gap-2 px-2 pb-1.5">
					<h3 class="truncate text-sm font-semibold">{block.heading}</h3>
					{#if block.clearable && canRemove}
						<button
							class="shrink-0 cursor-pointer text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
							onclick={() => clearQueueUndoable()}
						>
							Clear queue
						</button>
					{/if}
				</div>
			{/if}
			{@render rows(block.rows)}
		{/each}
	{:else}
		<!-- One empty state (interior #5 + #14): icon, a way back in via the recent pick, and the
		     pick itself named so it reads as a continuation, not a dead end. -->
		{@const recentPick = recentItems(personal, 1)[0]}
		<EmptyState
			icon={Queue01Icon}
			line="The queue is empty."
			hint={recentPick
				? `Pick up where you left off: ${recentPick.title}`
				: 'Search for something to get the music going.'}
			actionLabel={recentPick ? 'Play something' : 'Search music'}
			onAction={playRecentPick}
		/>
	{/if}
</div>
