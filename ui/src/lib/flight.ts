// Shared-element transition: the artwork on the card you clicked "flies" into the player.
//
// A capture-phase click listener remembers the cover art of whatever card was last clicked
// (any surface — cards, rows, shelves — works for free). When the player view opens, a clone
// of that image animates from the card's rect to the player's big artwork rect (FLIP), floating
// over the already-visible view. Nothing is hidden beforehand, so a missed flight degrades to
// the ordinary slide-up with no flicker.

type Flight = { src: string; rect: DOMRect; at: number };

const MAX_AGE = 700;
let flight: Flight | null = null;

/** Install the capture listener. Returns the teardown fn (call from initApp). */
export function initFlightCapture(): () => void {
	const onClick = (e: MouseEvent) => {
		const t = e.target as HTMLElement | null;
		if (!t) return;
		const img = (t.closest('img') as HTMLImageElement | null) ?? t.querySelector('img');
		if (!img) return;
		const rect = img.getBoundingClientRect();
		// Empty or huge images are the player's own art, not a card cover.
		if (!rect.width || rect.width > 420) return;
		const src = img.currentSrc || img.src;
		if (!src) return;
		flight = { src, rect, at: Date.now() };
	};
	document.addEventListener('click', onClick, { capture: true });
	return () => document.removeEventListener('click', onClick, { capture: true });
}

/**
 * Animate the last captured artwork into `targetSel` (the player's big artwork container).
 * Returns false when there's nothing recent to fly or the target isn't measurable — the caller
 * (and the user) never notices; the view just opened the ordinary way.
 */
export function playFlight(targetSel: string): boolean {
	const f = flight;
	flight = null;
	if (!f || Date.now() - f.at > MAX_AGE) return false;
	const target = document.querySelector(targetSel);
	if (!target) return false;
	const to = target.getBoundingClientRect();
	if (!to.width || !to.height) return false;

	const img = document.createElement('img');
	img.src = f.src; // the card's own URL — already decoded in memory
	img.alt = '';
	img.style.cssText =
		`position:fixed;z-index:95;pointer-events:none;object-fit:cover;max-width:none;` +
		`left:${f.rect.left}px;top:${f.rect.top}px;width:${f.rect.width}px;height:${f.rect.height}px;` +
		`border-radius:12px;box-shadow:0 18px 60px rgba(0,0,0,.45);`;
	document.body.appendChild(img);

	const anim = img.animate(
		[
			{
				left: `${f.rect.left}px`,
				top: `${f.rect.top}px`,
				width: `${f.rect.width}px`,
				height: `${f.rect.height}px`,
				borderRadius: '12px'
			},
			{
				left: `${to.left}px`,
				top: `${to.top}px`,
				width: `${to.width}px`,
				height: `${to.height}px`,
				borderRadius: '24px'
			}
		],
		{ duration: 380, easing: 'cubic-bezier(0.32, 0.72, 0, 1)', fill: 'forwards' }
	);
	anim.finished
		.finally(() => img.remove())
		.catch(() => img.remove());
	return true;
}
