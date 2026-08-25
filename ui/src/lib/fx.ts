// Tiny one-shot UI effects that outlive any single component: floating "+"/heart chips spawned at
// a screen position, animated up and faded, then dropped from the DOM. Pure DOM + Web Animations
// API — no Svelte state, so they can be called from anywhere (even outside a component's effects).

/** Shared implementation: a glass chip carrying `char`, rising 48px and fading over 700ms. */
function fly(char: string, x: number, y: number): void {
	if (typeof document === 'undefined') return; // SSR guard
	const el = document.createElement('span');
	el.textContent = char;
	el.setAttribute('aria-hidden', 'true');
	el.style.cssText = [
		'position:fixed',
		`left:${x}px`,
		`top:${y}px`,
		'z-index:9999',
		'pointer-events:none',
		'transform:translate(-50%,-50%)',
		'padding:2px 8px',
		'border-radius:9999px',
		'background:var(--glass-strong)',
		'border:1px solid var(--glass-border)',
		'box-shadow:inset 0 1px 0 0 var(--glass-highlight)',
		'-webkit-backdrop-filter:blur(8px)',
		'backdrop-filter:blur(8px)',
		'color:var(--primary)',
		'font-weight:700',
		'font-size:14px',
		'line-height:1.4'
	].join(';');
	document.body.appendChild(el);
	const anim = el.animate(
		[
			{ transform: 'translate(-50%,-50%) translateY(0)', opacity: 1 },
			{ transform: 'translate(-50%,-50%) translateY(-48px)', opacity: 0 }
		],
		{ duration: 700, easing: 'ease-out', fill: 'forwards' }
	);
	anim.onfinish = () => el.remove();
	setTimeout(() => el.remove(), 800); // safety net if onfinish never lands
}

/** A "+" chip flying up from where the user clicked "add to playlist". */
export function flyPlus(x: number, y: number): void {
	fly('+', x, y);
}

/** Same treatment with a heart — for like gestures outside TrackRow's own CSS burst. */
export function flyHeart(x: number, y: number): void {
	fly('♥', x, y);
}
