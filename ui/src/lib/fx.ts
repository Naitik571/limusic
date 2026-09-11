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

/**
 * Particle dissolve at a point: ~26 canvas dots in accent/white tones with gravity + fade
 * over ~600ms. Used on queue/playlist row removal. Skipped under reduced motion (nothing
 * spawns — the row's own exit takes over).
 */
export function burst(x: number, y: number): void {
	if (typeof document === 'undefined') return;
	if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return;
	const canvas = document.createElement('canvas');
	const W = 220;
	const H = 220;
	const dpr = Math.min(2, window.devicePixelRatio || 1);
	canvas.width = W * dpr;
	canvas.height = H * dpr;
	canvas.setAttribute('aria-hidden', 'true');
	canvas.style.cssText = [
		'position:fixed',
		`left:${x - W / 2}px`,
		`top:${y - H / 2}px`,
		`width:${W}px`,
		`height:${H}px`,
		'z-index:9999',
		'pointer-events:none'
	].join(';');
	document.body.appendChild(canvas);
	const ctx = canvas.getContext('2d');
	if (!ctx) {
		canvas.remove();
		return;
	}
	ctx.scale(dpr, dpr);
	const cs = getComputedStyle(document.documentElement);
	const accent = cs.getPropertyValue('--primary').trim() || '#e0402a';
	const colors = [accent, '#ffffff', accent, '#ffffff', accent];
	type P = { x: number; y: number; vx: number; vy: number; r: number; c: string; life: number };
	const parts: P[] = [];
	for (let i = 0; i < 26; i++) {
		const a = Math.random() * Math.PI * 2;
		const sp = 60 + Math.random() * 160;
		parts.push({
			x: W / 2,
			y: H / 2,
			vx: Math.cos(a) * sp,
			vy: Math.sin(a) * sp - 60,
			r: 1.5 + Math.random() * 2.5,
			c: colors[i % colors.length],
			life: 1
		});
	}
	const t0 = performance.now();
	const dur = 600;
	(function frame(now: number) {
		const k = Math.min(1, (now - t0) / dur);
		ctx.clearRect(0, 0, W, H);
		const dt = 1 / 60;
		for (const p of parts) {
			p.vy += 420 * dt;
			p.x += p.vx * dt;
			p.y += p.vy * dt;
			p.life = 1 - k;
			ctx.globalAlpha = Math.max(0, p.life);
			ctx.fillStyle = p.c;
			ctx.beginPath();
			ctx.arc(p.x, p.y, p.r * (0.5 + p.life * 0.5), 0, Math.PI * 2);
			ctx.fill();
		}
		ctx.globalAlpha = 1;
		if (k < 1) requestAnimationFrame(frame);
		else canvas.remove();
	})(t0);
	setTimeout(() => canvas.remove(), 800); // safety net
}
