/**
 * Reparent a `fixed` popup to <body>.
 *
 * `position: fixed` is only viewport-relative while no ancestor is a containing block for it, and
 * `contain` / `content-visibility` / `transform` / `filter` on any ancestor all make one. Shelf uses
 * content-visibility, so an in-place popup anchored by `anchorMenu` lands at the shelf's offset
 * instead of the viewport's and is clipped away by paint containment: the further down the feed, the
 * further off it goes. At <body> there is nothing above it to contain it.
 */
export function toBody(el: HTMLElement) {
	document.body.appendChild(el);
	return () => el.remove();
}

// Anchor a `fixed` dropdown at its trigger, flipping above it when the viewport bottom is too
// close for the menu to fit. `y` pairs with `top` normally, `bottom` when flipped.
// ponytail: estHeight over measuring — menus here are small and a few px of early flip is fine.
export function anchorMenu(trigger: HTMLElement, estHeight = 280) {
	const r = trigger.getBoundingClientRect();
	const openUp = r.bottom + estHeight > window.innerHeight;
	return {
		right: window.innerWidth - r.right,
		y: openUp ? window.innerHeight - r.top + 4 : r.bottom + 4,
		openUp
	};
}
