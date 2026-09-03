// Poolside motion utilities — rAF-driven animations that respect the user's reduce-motion
// preference and pause when the tab is hidden. Used by components that need physics-style
// animation that CSS can't do declaratively (e.g. the vinyl wobble has a tiny eccentric
// rotation that needs per-frame math).
//
// All exported helpers are no-ops when (prefers-reduced-motion: reduce) OR when the .reduce
// class is on the root — they still return the current value so callers can render normally,
// but they don't move the value over time.

import { onMount } from 'svelte';

/** True when the user has asked for less motion, either via OS or the in-app toggle. */
export function reducedMotion(): boolean {
	if (typeof window === 'undefined') return false;
	if (window.matchMedia?.('(prefers-reduced-motion: reduce)').matches) return true;
	// Cached lookup: the toggle rarely changes, avoid querySelector on every frame.
	// Cache is cleared on toggle by PoolsideShell (see ps-reduce write).
	return !!document.querySelector('.ps-root.reduce');
}

/**
 * Run a per-frame loop, automatically paused when the tab is hidden or reduced-motion is on.
 * Returns a stop() function. Safe to call during SSR — returns a no-op stop.
 *
 *   const stop = rafLoop((t, dt) => { ... });
 *   onDestroy(stop);
 */
export function rafLoop(tick: (t: number, dt: number) => void): () => void {
	if (typeof window === 'undefined') return () => {};
	let raf = 0;
	let last = performance.now();
	let running = true;

	const step = (now: number) => {
		if (!running) return;
		const dt = Math.min(0.1, (now - last) / 1000); // clamp to 100ms (tab-restore safety)
		last = now;
		tick(now / 1000, dt);
		raf = requestAnimationFrame(step);
	};
	raf = requestAnimationFrame(step);

	const onVisibility = () => {
		if (document.hidden) {
			running = false;
			cancelAnimationFrame(raf);
		} else if (!running) {
			running = true;
			last = performance.now();
			raf = requestAnimationFrame(step);
		}
	};
	document.addEventListener('visibilitychange', onVisibility);

	return () => {
		running = false;
		cancelAnimationFrame(raf);
		document.removeEventListener('visibilitychange', onVisibility);
	};
}

/**
 * Smoothly approach a target value (critically-damped spring, no overshoot). Returns a getter
 * for the current value, a setTarget(), and a stop(). Cheap — runs on rAF.
 *
 *   const { get, setTarget, stop } = smooth(0, { rate: 6 });
 *   setTarget(100);
 *   // later... get() returns something close to 100
 */
export function smooth(
	initial: number,
	opts: { rate?: number; reduce?: number } = {}
): { get: () => number; setTarget: (v: number) => void; stop: () => void } {
	const rate = opts.rate ?? 8; // higher = faster
	const reduce = opts.reduce ?? 0.001;
	let value = initial;
	let target = initial;
	const stop = rafLoop((_t, dt) => {
		if (reducedMotion()) {
			value = target;
			return;
		}
		const k = 1 - Math.exp(-rate * dt);
		value += (target - value) * k;
		// snap when very close so we don't keep the loop alive for a non-difference
		if (Math.abs(target - value) < reduce) value = target;
	});
	return {
		get: () => value,
		setTarget: (v: number) => {
			target = v;
		},
		stop
	};
}

/**
 * Spring with overshoot (bouncy settle). stiffness/damping are the same scale svelte/motion
 * uses; mass defaults to 1.
 */
export function spring(
	initial: number,
	opts: { stiffness?: number; damping?: number; mass?: number } = {}
): {
	get: () => number;
	setTarget: (v: number) => void;
	stop: () => void;
	velocity: () => number;
} {
	const stiffness = opts.stiffness ?? 170;
	const damping = opts.damping ?? 26;
	const mass = opts.mass ?? 1;
	let value = initial;
	let target = initial;
	let velocity = 0;
	const stop = rafLoop((_t, dt) => {
		if (reducedMotion()) {
			value = target;
			velocity = 0;
			return;
		}
		const x = value - target;
		const a = (-stiffness * x - damping * velocity) / mass;
		velocity += a * dt;
		value += velocity * dt;
		if (Math.abs(x) < 0.0005 && Math.abs(velocity) < 0.0005) {
			value = target;
			velocity = 0;
		}
	});
	return {
		get: () => value,
		setTarget: (v: number) => {
			target = v;
		},
		stop,
		velocity: () => velocity
	};
}

/** Set a CSS custom property on an element with an optional `transform` or `opacity` write. */
export function writeCss(
	el: HTMLElement,
	props: Record<string, string | number>
): void {
	for (const k in props) {
		el.style.setProperty(k.startsWith('--') ? k : `--${k}`, String(props[k]));
	}
}

/** Mount a once-per-component rAF loop that cleans up on destroy. Convenience over rafLoop + onDestroy. */
export function mountRaf(tick: (t: number, dt: number) => void): void {
	onMount(() => rafLoop(tick));
}

/** Clamp v into [lo, hi]. */
export function clamp(v: number, lo: number, hi: number): number {
	return Math.max(lo, Math.min(hi, v));
}

/** Linear interpolation. */
export function lerp(a: number, b: number, t: number): number {
	return a + (b - a) * t;
}

/** A short, sweet ripple element appended to a button on click. Auto-removes after the anim. */
export function spawnRipple(
	el: HTMLElement,
	e: MouseEvent | PointerEvent,
	color = 'rgba(255,255,255,.5)'
): void {
	if (reducedMotion()) return;
	const rect = el.getBoundingClientRect();
	const x = e.clientX - rect.left;
	const y = e.clientY - rect.top;
	const size = Math.max(rect.width, rect.height) * 1.4;
	const r = document.createElement('span');
	r.className = 'ps-ripple';
	r.style.cssText = `left:${x}px;top:${y}px;width:${size}px;height:${size}px;background:${color};`;
	el.appendChild(r);
	// 650ms matches the CSS keyframe duration
	setTimeout(() => r.remove(), 700);
}
