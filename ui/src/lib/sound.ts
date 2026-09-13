// UI sounds — tiny WebAudio blips, no audio assets.
//
// Three helpers: playClick (transport buttons), playSuccess (toast.success-worthy
// events), playError (failures). Each is a sub-100ms oscillator envelope through a
// master gain of ~0.08, so they stay subtle. The AudioContext is created lazily on
// the first play call (i.e. on a user gesture), never at module load.
//
// Enabled flag defaults ON. SettingsDialog syncs it from the persisted
// `sounds_enabled` backend setting on open; a localStorage mirror keeps the first
// paint correct before that sync runs.

let enabled = true;

const STORE_KEY = 'limusic_sounds';

try {
	if (typeof localStorage !== 'undefined') {
		const raw = localStorage.getItem(STORE_KEY);
		if (raw !== null) enabled = raw !== 'false';
	}
} catch {
	/* quota or no storage — stay on default */
}

export function soundsEnabled(): boolean {
	return enabled;
}

export function setSoundsEnabled(on: boolean): void {
	enabled = on;
	try {
		localStorage.setItem(STORE_KEY, on ? 'true' : 'false');
	} catch {
		/* quota */
	}
}

let ctx: AudioContext | null = null;
let master: GainNode | null = null;

/** Lazily create (or reuse) the context + master gain. Null when unavailable. */
function ensure(): AudioContext | null {
	try {
		if (ctx && master) {
			if (ctx.state === 'suspended') void ctx.resume();
			return ctx;
		}
		if (typeof window === 'undefined') return null;
		const AC = window.AudioContext;
		if (!AC) return null;
		ctx = new AC();
		master = ctx.createGain();
		master.gain.value = 0.08;
		master.connect(ctx.destination);
		if (ctx.state === 'suspended') void ctx.resume();
		return ctx;
	} catch {
		return null;
	}
}

/** One enveloped oscillator blip. `dur` in seconds — keep every call < 0.1. */
function tone(
	freqFrom: number,
	freqTo: number,
	dur: number,
	type: OscillatorType,
	delay = 0,
	vol = 1
): void {
	const ac = ensure();
	if (!ac || !master) return;
	try {
		const t0 = ac.currentTime + delay;
		const osc = ac.createOscillator();
		const gain = ac.createGain();
		osc.type = type;
		osc.frequency.setValueAtTime(freqFrom, t0);
		osc.frequency.exponentialRampToValueAtTime(Math.max(1, freqTo), t0 + dur);
		// Fast attack, exponential decay to silence — no clicks at the edges.
		gain.gain.setValueAtTime(0.0001, t0);
		gain.gain.exponentialRampToValueAtTime(vol, t0 + 0.008);
		gain.gain.exponentialRampToValueAtTime(0.0001, t0 + dur);
		osc.connect(gain);
		gain.connect(master);
		osc.start(t0);
		osc.stop(t0 + dur + 0.02);
	} catch {
		/* never break UI for a blip */
	}
}

/** Short transport click. */
export function playClick(): void {
	if (!enabled) return;
	tone(660, 440, 0.06, 'sine');
}

/** Bright two-note chime for successes. */
export function playSuccess(): void {
	if (!enabled) return;
	tone(523, 523, 0.07, 'sine', 0, 0.9);
	tone(784, 784, 0.09, 'sine', 0.07, 0.9);
}

/** Low soft buzz for failures. */
export function playError(): void {
	if (!enabled) return;
	tone(220, 160, 0.09, 'triangle', 0, 0.9);
}
