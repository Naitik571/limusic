// Horizontal swipe-to-remove predicates, shared by every queue list (QueueList for the
// classic layouts, QueueView for poolside). Vertical movement belongs to reorder/scroll
// and must never trigger removal: a press becomes a swipe only while |dx| dominates.

/** Minimum horizontal travel before a press counts as a swipe (px). */
export const SWIPE_PX = 12;

/** True once the gesture is unambiguously horizontal. */
export function isSwipe(dx: number, dy: number): boolean {
	const ax = Math.abs(dx);
	return ax > SWIPE_PX && ax > Math.abs(dy) * 1.6;
}

/** Release past 32% of the row width (min 80px) removes; anything less snaps back. */
export function shouldRemove(dx: number, rowWidth: number): boolean {
	return Math.abs(dx) > Math.max(80, rowWidth * 0.32);
}
