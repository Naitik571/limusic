// Rebindable keyboard map. Every driving key in the app resolves through here (the plain
// playback chain in player.svelte + the Ctrl chords in shortcuts.ts), so the shortcuts
// dialog can offer click-to-rebind with conflict handling instead of a static list.
//
// Model: each action owns a list of binding slots (usually one; play/pause ships two).
// Overrides persist as a partial map in localStorage; anything untouched reads defaults.
// A rebind that collides refuses with the holder's name — capture again within 3 s to force
// it (the loser's slot clears; Reset restores everything).
export type ActionId =
	| 'playpause'
	| 'next'
	| 'prev'
	| 'mute'
	| 'volup'
	| 'voldown'
	| 'seekback'
	| 'seekfwd'
	| 'seekback10'
	| 'seekfwd10'
	| 'palette'
	| 'shortcuts'
	| 'nowplaying';

export interface Binding {
	key: string;
	ctrl?: boolean;
	shift?: boolean;
	alt?: boolean;
}

type Slot = { action: ActionId; label: string };

const DEFAULTS: Record<ActionId, Binding[]> = {
	playpause: [
		{ key: ' ' },
		{ key: 'k' }
	],
	next: [{ key: 'n', shift: true }],
	prev: [{ key: 'p', shift: true }],
	mute: [{ key: 'm' }],
	volup: [{ key: 'ArrowUp' }],
	voldown: [{ key: 'ArrowDown' }],
	seekback: [{ key: 'ArrowLeft' }],
	seekfwd: [{ key: 'ArrowRight' }],
	seekback10: [{ key: 'j' }],
	seekfwd10: [{ key: 'l' }],
	palette: [{ key: 'k', ctrl: true }],
	shortcuts: [{ key: 'h', ctrl: true }],
	nowplaying: [{ key: 'e', ctrl: true }]
};

export const ACTION_LABELS: Record<ActionId, string> = {
	playpause: 'Play / pause',
	next: 'Next track',
	prev: 'Previous track',
	mute: 'Mute',
	volup: 'Volume up',
	voldown: 'Volume down',
	seekback: 'Seek back 5s',
	seekfwd: 'Seek forward 5s',
	seekback10: 'Seek back 10s',
	seekfwd10: 'Seek forward 10s',
	palette: 'Quick search',
	shortcuts: 'This list',
	nowplaying: 'Now-playing view'
};

const STORE_KEY = 'limusic_keys';

function loadOverrides(): Partial<Record<ActionId, Binding[]>> {
	try {
		const raw = localStorage.getItem(STORE_KEY);
		if (!raw) return {};
		const parsed = JSON.parse(raw) as unknown;
		if (!parsed || typeof parsed !== 'object') return {};
		const out: Partial<Record<ActionId, Binding[]>> = {};
		for (const [k, v] of Object.entries(parsed as Record<string, unknown>)) {
			if (!(k in DEFAULTS) || !Array.isArray(v)) continue;
			const slots = (v as unknown[]).filter(
				(b): b is Binding =>
					!!b && typeof b === 'object' && typeof (b as Binding).key === 'string'
			);
			if (slots.length) out[k as ActionId] = slots;
		}
		return out;
	} catch {
		return {};
	}
}

let overrides: Partial<Record<ActionId, Binding[]>> = loadOverrides();

function persist(): void {
	try {
		localStorage.setItem(STORE_KEY, JSON.stringify(overrides));
	} catch {
		/* quota */
	}
}

export function bindingsFor(action: ActionId): Binding[] {
	return overrides[action] ?? DEFAULTS[action];
}

/** Human spelling: "Space", "Ctrl + K", "Shift + N", "↑". */
export function describe(b: Binding): string {
	const key =
		b.key === ' '
			? 'Space'
			: b.key === 'ArrowUp'
				? '↑'
				: b.key === 'ArrowDown'
					? '↓'
					: b.key === 'ArrowLeft'
						? '←'
						: b.key === 'ArrowRight'
							? '→'
							: b.key.length === 1
								? b.key.toUpperCase()
								: b.key;
	const mods: string[] = [];
	if (b.ctrl) mods.push('Ctrl');
	if (b.alt) mods.push('Alt');
	if (b.shift) mods.push('Shift');
	return [...mods, key].join(' + ');
}

function same(a: Binding, b: Binding): boolean {
	return (
		a.key.toLowerCase() === b.key.toLowerCase() &&
		!!a.ctrl === !!b.ctrl &&
		!!a.shift === !!b.shift &&
		!!a.alt === !!b.alt
	);
}

export function matchBinding(e: KeyboardEvent, b: Binding): boolean {
	// Meta counts as Ctrl (macOS ⌘ chords). Shift/Alt are compared by flag, not by
	// character: Shift+N must not fire a plain-n binding and vice versa, and an
	// explicitly Alt-bound Arrow must match so it can win over spatial navigation.
	const ctrl = e.ctrlKey || e.metaKey;
	if (!!ctrl !== !!b.ctrl) return false;
	if (!!e.shiftKey !== !!b.shift) return false;
	if (!!e.altKey !== !!b.alt) return false;
	return e.key.toLowerCase() === b.key.toLowerCase();
}

/** Which action (if any) currently owns this keystroke, excluding one action. */
export function holderOf(b: Binding, except?: ActionId): { action: ActionId; slot: number } | null {
	for (const action of Object.keys(DEFAULTS) as ActionId[]) {
		if (action === except) continue;
		const slots = bindingsFor(action);
		for (let i = 0; i < slots.length; i++) {
			if (same(slots[i], b)) return { action, slot: i };
		}
	}
	return null;
}

export function setBinding(
	action: ActionId,
	slot: number,
	b: Binding,
	force = false
): { ok: true } | { ok: false; holder: ActionId } {
	const clash = holderOf(b, action);
	if (clash && !force) return { ok: false, holder: clash.action };
	if (clash && force) {
		// Evict the loser's slot (an action left slot-less just loses that chord).
		const loser = bindingsFor(clash.action).filter((_, i) => i !== clash.slot);
		if (loser.length) overrides[clash.action] = loser;
		else delete overrides[clash.action];
	}
	const slots = [...bindingsFor(action)];
	slots[slot] = b;
	overrides[action] = slots;
	persist();
	return { ok: true };
}

export function resetBindings(): void {
	overrides = {};
	try {
		localStorage.removeItem(STORE_KEY);
	} catch {
		/* quota */
	}
}

export type { Slot };
