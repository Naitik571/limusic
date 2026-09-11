// Dyslexia-friendly typography extras: letter/word spacing + line height, applied as
// inheritable CSS vars on <html> (components with explicit tracking/leading override).
// localStorage-persisted; defaults are neutral so untouched installs render identically.
export interface DysPrefs {
	ls: number; // em, 0–0.3
	ws: number; // em, 0–0.5
	lh: number; // unitless, 1.2–2.0
}

const KEY = 'limusic_dys';

export const DYS_DEFAULTS: DysPrefs = { ls: 0, ws: 0, lh: 1.5 };

export function loadDys(): DysPrefs {
	try {
		const raw = localStorage.getItem(KEY);
		if (!raw) return { ...DYS_DEFAULTS };
		const p = JSON.parse(raw) as Partial<DysPrefs>;
		return {
			ls: Math.min(0.3, Math.max(0, Number(p.ls) || 0)),
			ws: Math.min(0.5, Math.max(0, Number(p.ws) || 0)),
			lh: Math.min(2.0, Math.max(1.2, Number(p.lh) || 1.5))
		};
	} catch {
		return { ...DYS_DEFAULTS };
	}
}

export function applyDys(p: DysPrefs): void {
	try {
		localStorage.setItem(KEY, JSON.stringify(p));
	} catch {
		/* quota */
	}
	const root = document.documentElement;
	root.style.setProperty('--dys-ls', `${p.ls}em`);
	root.style.setProperty('--dys-ws', `${p.ws}em`);
	root.style.setProperty('--dys-lh', String(p.lh));
}
