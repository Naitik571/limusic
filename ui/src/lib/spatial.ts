// Spatial navigation: Alt+Arrow moves focus to the nearest element in that direction,
// measured by rect geometry. Opt-in (Settings → System, off by default): arrow keys otherwise
// keep their seek/volume jobs. Skips hidden, disabled and tabindex=-1 elements.
const STORE_KEY = 'limusic_spatial';

export function spatialEnabled(): boolean {
	try {
		return localStorage.getItem(STORE_KEY) === 'true';
	} catch {
		return false;
	}
}

export function setSpatialEnabled(on: boolean): void {
	try {
		localStorage.setItem(STORE_KEY, on ? 'true' : 'false');
	} catch {
		/* quota */
	}
}

function focusables(): HTMLElement[] {
	const out: HTMLElement[] = [];
	for (const el of document.querySelectorAll<HTMLElement>(
		'button, a[href], input, select, textarea, [role="button"], [tabindex]:not([tabindex="-1"])'
	)) {
		if (el.closest('[hidden], [aria-hidden="true"]')) continue;
		const style = getComputedStyle(el);
		if (style.display === 'none' || style.visibility === 'hidden') continue;
		const r = el.getBoundingClientRect();
		if (r.width < 2 || r.height < 2) continue;
		if ((el as HTMLButtonElement).disabled) continue;
		out.push(el);
	}
	return out;
}

/** Focus the nearest focusable in `dir` from the current anchor. Returns whether it moved. */
export function spatialMove(dir: 'up' | 'down' | 'left' | 'right'): boolean {
	const active = document.activeElement as HTMLElement | null;
	const ax = window.innerWidth / 2;
	const ay = window.innerHeight / 2;
	const ar = active?.getBoundingClientRect();
	const cx = ar ? ar.left + ar.width / 2 : ax;
	const cy = ar ? ar.top + ar.height / 2 : ay;
	let best: HTMLElement | null = null;
	let bestScore = Infinity;
	for (const el of focusables()) {
		if (el === active) continue;
		const r = el.getBoundingClientRect();
		const ex = r.left + r.width / 2;
		const ey = r.top + r.height / 2;
		const dx = ex - cx;
		const dy = ey - cy;
		let primary: number;
		let secondary: number;
		if (dir === 'left' || dir === 'right') {
			primary = dir === 'left' ? -dx : dx;
			secondary = Math.abs(dy);
		} else {
			primary = dir === 'up' ? -dy : dy;
			secondary = Math.abs(dx);
		}
		if (primary <= 4) continue; // behind or level — not in this direction
		// Mostly forward distance, some lateral penalty (cone, not ray).
		const score = primary + secondary * 2.2;
		if (score < bestScore) {
			bestScore = score;
			best = el;
		}
	}
	if (best) {
		best.focus({ preventScroll: false });
		try {
			best.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
		} catch {
			/* older webview */
		}
		return true;
	}
	return false;
}
