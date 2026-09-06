// Deterministic generated artwork for tracks/albums with no cover image.
//
// Same idea as BlazePod's fallback art: a seeded two-tone SVG (palette + geometric
// motif + initials) so a missing thumbnail reads as deliberate cover art, not a blank
// box. Pure function of the seed — stable across renders, no storage, no network.
const PALETTES: Array<[string, string]> = [
	['#f59e0b', '#1c1917'],
	['#22c55e', '#052e16'],
	['#06b6d4', '#083344'],
	['#818cf8', '#1e1b4b'],
	['#f43f5e', '#4c0519'],
	['#a3e635', '#1a2e05'],
	['#1e1b4b', '#c7d2fe'],
	['#3b0764', '#fae8ff'],
	['#052e16', '#bbf7d0'],
	['#0c4a6e', '#e0f2fe'],
	['#431407', '#fed7aa'],
	['#3f3f46', '#fafafa']
];

function hash(seed: string): number {
	let h = 2166136261;
	for (let i = 0; i < seed.length; i++) {
		h ^= seed.charCodeAt(i);
		h = Math.imul(h, 16777619);
	}
	return h >>> 0;
}

function initials(label: string): string {
	const words = label.trim().split(/\s+/).filter(Boolean);
	if (!words.length) return '♪';
	const first = words[0][0] ?? '';
	const last = words.length > 1 ? (words[words.length - 1][0] ?? '') : '';
	return (first + last).toUpperCase();
}

/** data-uri SVG placeholder, seeded by id/title. `label` feeds the initials. */
export function fallbackArt(seed: string, label = ''): string {
	const h = hash(seed || label || 'limusic');
	const [bg, fg] = PALETTES[h % PALETTES.length];
	const motif = (h >>> 4) % 4;
	const cx = 34 + ((h >>> 8) % 60);
	const cy = 34 + ((h >>> 14) % 60);
	const r = 26 + ((h >>> 20) % 30);
	let shapes = '';
	if (motif === 0) {
		shapes = `<circle cx="${cx}" cy="${cy}" r="${r}" fill="${fg}" opacity="0.85"/><circle cx="${128 - cx}" cy="${128 - cy}" r="${r / 2}" fill="${fg}" opacity="0.45"/>`;
	} else if (motif === 1) {
		shapes = `<circle cx="64" cy="64" r="${r + 8}" fill="none" stroke="${fg}" stroke-width="10" opacity="0.8"/><circle cx="64" cy="64" r="${Math.max(6, r - 22)}" fill="${fg}" opacity="0.8"/>`;
	} else if (motif === 2) {
		const y = 30 + ((h >>> 8) % 68);
		shapes = `<rect x="0" y="${y}" width="128" height="14" fill="${fg}" opacity="0.75"/><rect x="0" y="${y + 24}" width="128" height="7" fill="${fg}" opacity="0.45"/>`;
	} else {
		shapes = `<circle cx="64" cy="64" r="52" fill="none" stroke="${fg}" stroke-width="6" opacity="0.7"/><circle cx="64" cy="64" r="52" fill="none" stroke="${bg}" stroke-width="2" stroke-dasharray="4 7" opacity="0.9"/>`;
	}
	const svg =
		`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">` +
		`<rect width="128" height="128" fill="${bg}"/>${shapes}` +
		`<text x="64" y="76" font-family="system-ui,sans-serif" font-size="40" font-weight="700" text-anchor="middle" fill="${bg}">${initials(label)}</text></svg>`;
	return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}
