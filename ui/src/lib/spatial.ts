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

const FOCUS_SEL =
	'button, a[href], input, select, textarea, [role="button"], [tabindex]:not([tabindex="-1"])';

function visible(el: HTMLElement): boolean {
	if (el.closest('[hidden], [aria-hidden="true"]')) return false;
	const style = getComputedStyle(el);
	if (style.display === 'none' || style.visibility === 'hidden') return false;
	const r = el.getBoundingClientRect();
	if (r.width < 2 || r.height < 2) return false;
	if ((el as HTMLButtonElement).disabled) return false;
	return true;
}

function focusables(): HTMLElement[] {
	// An open dialog owns the focus: trap movement inside it so Alt+Arrows can't wander
	// the dimmed page behind the modal. Last dialog wins (stacked menus).
	const dialogs = [...document.querySelectorAll<HTMLElement>('[role="dialog"]')].filter(visible);
	const scope = dialogs.length ? dialogs[dialogs.length - 1] : document;
	const out: HTMLElement[] = [];
	for (const el of scope.querySelectorAll<HTMLElement>(FOCUS_SEL)) {
		if (visible(el)) out.push(el);
	}
	return out;
}

function focusEl(el: HTMLElement): void {
	el.focus({ preventScroll: false });
	try {
		el.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
	} catch {
		/* older webview */
	}
}

/** Focus the nearest focusable in `dir` from the current anchor. Returns whether it moved. */
export function spatialMove(dir: 'up' | 'down' | 'left' | 'right'): boolean {
	const active = document.activeElement as HTMLElement | null;
	const els = focusables();
	if (!els.length) return false;
	const ax = window.innerWidth / 2;
	const ay = window.innerHeight / 2;
	const ar = active?.getBoundingClientRect();
	const cx = ar ? ar.left + ar.width / 2 : ax;
	const cy = ar ? ar.top + ar.height / 2 : ay;
	let best: HTMLElement | null = null;
	let bestScore = Infinity;
	for (const el of els) {
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
		focusEl(best);
		return true;
	}
	// Dead end wraps around instead of stranding focus: forward (down/right) lands on the
	// first element, back (up/left) on the last — the same cycle Tab/Shift+Tab give.
	const others = els.filter((el) => el !== active);
	if (!others.length) return false;
	focusEl(dir === 'down' || dir === 'right' ? others[0] : others[others.length - 1]);
	return true;
}
