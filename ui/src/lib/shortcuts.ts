// App-wide keyboard shortcuts. One window listener, gated on Ctrl/Cmd before anything else, so a
// key typed into a field costs a single boolean check and falls straight through. Zoom keeps its
// own listener (zoom.ts) because it also owns the ctrl+wheel gesture.
import { browser } from '$app/environment';
import { np, nudgeVolume, playback, ui } from './player.svelte';
import { bindingsFor, matchBinding } from './keys';

/** How this machine writes the modifier these shortcuts hang off, for anything that shows a key
 *  hint. Mac takes the bare glyph; everywhere else the `+` is part of the spelling. */
export const MOD = browser && navigator.platform.startsWith('Mac') ? '⌘' : 'Ctrl+';

/** Percent per press, matching a step of the volume slider's arrow keys. */
const VOLUME_STEP = 5;
const VOLUME_STEP_PRECISE = 1;

export function initShortcuts() {
	const onKey = (e: KeyboardEvent) => {
		if (!e.ctrlKey && !e.metaKey) return;
		const run = (action: 'palette' | 'shortcuts' | 'nowplaying'): boolean =>
			bindingsFor(action).some((b) => matchBinding(e, b));
		// Toggles, so the key that opened the palette also dismisses it.
		if (run('palette')) {
			e.preventDefault();
			ui.paletteOpen = !ui.paletteOpen;
			return;
		}
		if (run('shortcuts')) {
			e.preventDefault();
			ui.shortcutsOpen = !ui.shortcutsOpen;
			return;
		}
		if (run('nowplaying')) {
			// With nothing playing there is no view to open (the layout renders it behind
			// `playback.now`), and flipping the flag anyway would ambush the next play.
			if (!playback.now) return;
			e.preventDefault();
			np.open = !np.open;
			return;
		}
		// Legacy precise-volume chords (pear parity): Ctrl with . /> and , / <.
		const precise = e.shiftKey; // Shift for precise 1% (pear parity)
		const step = precise ? VOLUME_STEP_PRECISE : VOLUME_STEP;
		switch (e.key) {
			// Shift+. and Shift+, on a US layout. The unshifted keys are accepted too, so the
			// shortcut still works on layouts that put > and < somewhere else.
			case '>':
			case '.':
				e.preventDefault();
				nudgeVolume(step, precise);
				break;
			case '<':
			case ',':
				e.preventDefault();
				nudgeVolume(-step, precise);
				break;
			default:
				return;
		}
	};
	window.addEventListener('keydown', onKey);
	return () => window.removeEventListener('keydown', onKey);
}
