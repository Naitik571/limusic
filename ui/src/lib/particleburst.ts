/**
 * MIUI-style disintegrate burst: dependency-free particle explosion over a shared
 * fixed canvas. Visual only — callers remove/splice their own rows.
 *
 * - Shared canvas (one per page), rAF loop self-stops when particles die.
 * - Capped particles (drops oldest when over budget).
 * - pointer-events: none, transform/opacity only on the canvas element itself
 *   (particles are canvas pixels, so no layout cost).
 * - Palette: sample the element's cover <img> (first found) for accent pixels,
 *   else fall back to the app accent / primary.
 */
export function particleBurst(el: Element): void {
	try {
		if (typeof document === 'undefined') return;
		if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return;
		const rect = el.getBoundingClientRect();
		if (rect.width <= 0 || rect.height <= 0) return;

		const canvas = getBurstCanvas();
		const ctx = canvas.getContext('2d');
		if (!ctx) return;

		const dpr = Math.min(2, window.devicePixelRatio || 1);
		const vw = window.innerWidth;
		const vh = window.innerHeight;
		if (canvas.width !== Math.round(vw * dpr) || canvas.height !== Math.round(vh * dpr)) {
			canvas.width = Math.round(vw * dpr);
			canvas.height = Math.round(vh * dpr);
		}
		ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

		const colors = paletteFrom(el);
		const cx = rect.left + rect.width / 2;
		const cy = rect.top + rect.height / 2;
		const count = Math.min(46, Math.max(18, Math.round((rect.width * rect.height) / 900)));
		const now = performance.now();
		for (let i = 0; i < count; i++) {
			const a = Math.random() * Math.PI * 2;
			const speed = 60 + Math.random() * 260;
			const life = 450 + Math.random() * 450;
			particles.push({
				x: cx + (Math.random() - 0.5) * rect.width * 0.6,
				y: cy + (Math.random() - 0.5) * rect.height * 0.6,
				vx: Math.cos(a) * speed,
				vy: Math.sin(a) * speed - 60,
				size: 1.5 + Math.random() * 3,
				color: colors[(Math.random() * colors.length) | 0],
				born: now,
				life
			});
		}
		// Cap: drop oldest first so a burst spam can't grow the array.
		while (particles.length > MAX_PARTICLES) particles.splice(0, particles.length - MAX_PARTICLES);
		kick();
	} catch {
		/* visual only — never break the removal path */
	}
}

type P = {
	x: number;
	y: number;
	vx: number;
	vy: number;
	size: number;
	color: string;
	born: number;
	life: number;
};

const MAX_PARTICLES = 320;
const particles: P[] = [];
let raf = 0;
let last = 0;

function getBurstCanvas(): HTMLCanvasElement {
	let c = document.getElementById('limusic-burst-layer') as HTMLCanvasElement | null;
	if (!c) {
		c = document.createElement('canvas');
		c.id = 'limusic-burst-layer';
		c.setAttribute('aria-hidden', 'true');
		c.style.cssText =
			'position:fixed;inset:0;width:100vw;height:100vh;z-index:300;pointer-events:none;opacity:1;';
		document.body.appendChild(c);
	}
	return c;
}

function paletteFrom(el: Element): string[] {
	const fallback = [accentCss(), '#ffffff', '#8b8b8b'];
	try {
		const img = el.querySelector?.('img') as HTMLImageElement | null;
		const rootImg =
			img ?? (el.closest?.('[data-burst-cover]')?.querySelector('img') as HTMLImageElement | null);
		if (rootImg && rootImg.complete && rootImg.naturalWidth > 0) {
			const sample = document.createElement('canvas');
			const n = 8;
			sample.width = n;
			sample.height = n;
			const sctx = sample.getContext('2d');
			if (sctx) {
				try {
					sctx.drawImage(rootImg, 0, 0, n, n);
					const d = sctx.getImageData(0, 0, n, n).data;
					const out: string[] = [];
					for (let i = 0; i < d.length; i += 4) {
						if (d[i + 3] < 128) continue;
						out.push(`rgb(${d[i]} ${d[i + 1]} ${d[i + 2]})`);
						if (out.length >= 6) break;
					}
					if (out.length) return [...out, accentCss()];
				} catch {
					/* tainted canvas (remote cover) — fall through to accent */
				}
			}
		}
	} catch {
		/* ignore */
	}
	return fallback;
}

function accentCss(): string {
	try {
		const v = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim();
		if (v) return v.startsWith('oklch') || v.startsWith('rgb') || v.startsWith('#') ? v : `var(--accent)`;
	} catch {
		/* ignore */
	}
	return '#e0402a';
}

function kick(): void {
	if (raf) return;
	last = performance.now();
	raf = requestAnimationFrame(tick);
}

function tick(t: number): void {
	const canvas = document.getElementById('limusic-burst-layer') as HTMLCanvasElement | null;
	const ctx = canvas?.getContext('2d');
	if (!canvas || !ctx) {
		raf = 0;
		particles.length = 0;
		return;
	}
	const dt = Math.min(0.05, (t - last) / 1000);
	last = t;
	const dpr = Math.min(2, window.devicePixelRatio || 1);
	ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
	ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
	for (let i = particles.length - 1; i >= 0; i--) {
		const p = particles[i];
		const age = t - p.born;
		if (age >= p.life) {
			particles.splice(i, 1);
			continue;
		}
		const k = age / p.life;
		p.vy += 420 * dt; // gravity
		p.vx *= 1 - 1.6 * dt; // drag
		p.x += p.vx * dt;
		p.y += p.vy * dt;
		ctx.globalAlpha = 1 - k;
		ctx.fillStyle = p.color;
		const s = p.size * (1 - k * 0.6);
		ctx.fillRect(p.x, p.y, s, s);
	}
	ctx.globalAlpha = 1;
	if (particles.length) {
		raf = requestAnimationFrame(tick);
	} else {
		raf = 0;
		ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);
	}
}
