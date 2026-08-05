// Two kinds of theme live here, selected from one picker and persisted to localStorage (a pure UI
// preference, no backend round-trip):
//   - 'accent'  — overrides only --primary/--accent as inline styles on <html>, layered over the
//                 app's default palette. Wins over both :root and .dark.
//   - 'palette' — a full token set (background, card, sidebar, radius, …) for light AND dark, defined
//                 as a `.theme-<id>` class in layout.css. Applied by toggling that class on <html>.
//
// On top of whichever preset is selected sits the *custom* layer (accent colour, background tint,
// roundness, fonts). It's inline styles too, applied after the preset, so it wins over both kinds
// and survives switching presets. Anything the user hasn't touched stays null and the preset shows
// through — the customization is a set of overrides, not a rival theme to maintain.

import { isLight } from './color';

export type ThemeId = 'rose' | 'blue' | 'lime' | 'purple' | 'teal' | 'catppuccin' | 'caffeine' | 'neon' | 'breeze';

// `fg` (accent themes only) is the text/icon colour that sits ON the accent: light accents (lime,
// teal) need a dark foreground; dark accents keep the light one. `color` is just the picker swatch.
type Theme =
	| { id: ThemeId; label: string; kind: 'accent'; color: string; fg: string }
	| { id: ThemeId; label: string; kind: 'palette'; color: string };

export const THEMES: Theme[] = [
	{ id: 'rose', label: 'Rose', kind: 'accent', color: 'oklch(0.455 0.188 13.697)', fg: 'oklch(0.985 0 0)' },
	{ id: 'blue', label: 'Blue', kind: 'accent', color: 'oklch(0.49 0.22 264)', fg: 'oklch(0.985 0 0)' },
	{ id: 'lime', label: 'Lime', kind: 'accent', color: 'oklch(0.77 0.2 131)', fg: 'oklch(0.205 0 0)' },
	{ id: 'purple', label: 'Purple', kind: 'accent', color: 'oklch(0.56 0.25 302)', fg: 'oklch(0.985 0 0)' },
	{ id: 'teal', label: 'Teal', kind: 'accent', color: 'oklch(0.85 0.13 181)', fg: 'oklch(0.205 0 0)' },
	{ id: 'catppuccin', label: 'Catppuccin', kind: 'palette', color: 'oklch(0.5547 0.2503 297.0156)' },
	{ id: 'caffeine', label: 'Caffeine', kind: 'palette', color: 'oklch(0.4341 0.0392 41.9938)' },
	{ id: 'neon', label: 'Neon', kind: 'palette', color: 'oklch(0.6726 0.2904 341.4084)' },
	{ id: 'breeze', label: 'Breeze', kind: 'palette', color: 'oklch(0.7227 0.1920 149.5793)' }
];

/** Font stacks bundled with the app (imported in layout.css). "System" needs no download. */
export const FONTS: { label: string; value: string }[] = [
	{ label: 'Oxanium', value: "'Oxanium Variable', sans-serif" },
	{ label: 'IBM Plex Sans', value: "'IBM Plex Sans Variable', sans-serif" },
	{ label: 'Montserrat', value: "'Montserrat Variable', sans-serif" },
	{ label: 'Outfit', value: "'Outfit Variable', sans-serif" },
	{ label: 'DM Sans', value: "'DM Sans Variable', sans-serif" },
	{ label: 'System', value: 'ui-sans-serif, system-ui, sans-serif' }
];

/** The custom layer. `null` = untouched, so the selected preset decides. */
export type Custom = {
	accent: string | null; // hex
	hue: number | null; // 0–360, tints the default palette's neutrals
	radius: number | null; // rem
	fontSans: string | null; // a CSS font-family value
	fontHeading: string | null;
};

const KEY = 'primary-theme';
const CUSTOM_KEY = 'custom-theme';
const PALETTE_CLASSES = THEMES.filter((t) => t.kind === 'palette').map((t) => `theme-${t.id}`);
const ACCENT_VARS = ['--primary', '--primary-foreground', '--accent', '--accent-foreground'];
const CUSTOM_VARS = ['--hue', '--radius', '--font-sans', '--font-heading'];
// Same two neutrals the preset accent themes pick between.
const ON_DARK = 'oklch(0.985 0 0)';
const ON_LIGHT = 'oklch(0.205 0 0)';

/** Reactive current selection, so the picker reflects it. */
export const theme = $state<{ id: ThemeId }>({ id: 'rose' });
export const custom = $state<Custom>({
	accent: null,
	hue: null,
	radius: null,
	fontSans: null,
	fontHeading: null
});

/**
 * What the tokens resolve to *after* the preset and the overrides are applied. The controls read
 * this so their starting position is wherever the current theme actually sits, instead of a
 * hardcoded default that goes stale the moment a preset moves it.
 */
export const effective = $state({
	hue: 326,
	radius: 0.45,
	accent: '#000000',
	fontSans: '',
	fontHeading: ''
});

/** oklch/rgb/anything CSS -> hex, via canvas's own normalization. '#000000' if it won't parse. */
function toHex(color: string): string {
	const ctx = document.createElement('canvas').getContext('2d');
	if (!ctx) return '#000000';
	ctx.fillStyle = '#000000';
	ctx.fillStyle = color; // ignored (leaving black) if this engine can't parse the colour space
	return typeof ctx.fillStyle === 'string' && ctx.fillStyle.startsWith('#')
		? ctx.fillStyle
		: '#000000';
}

/**
 * Re-read the tokens. Called after every apply, and by the settings modal on open: toggling
 * light/dark doesn't route through here, and a palette's --primary differs between the two.
 */
export function readBack(): void {
	const cs = getComputedStyle(document.documentElement);
	const g = (n: string) => cs.getPropertyValue(n).trim();
	effective.hue = parseFloat(g('--hue')) || 0;
	effective.radius = parseFloat(g('--radius')) || 0;
	effective.accent = toHex(g('--primary'));
	effective.fontSans = g('--font-sans');
	effective.fontHeading = g('--font-heading');
}

function apply(): void {
	const t = THEMES.find((x) => x.id === theme.id) ?? THEMES[0];
	const root = document.documentElement;
	// Reset every mechanism first, so switching between an accent and a palette (or clearing a
	// custom override) never leaves the previous choice's inline vars or class behind.
	[...ACCENT_VARS, ...CUSTOM_VARS].forEach((v) => root.style.removeProperty(v));
	root.classList.remove(...PALETTE_CLASSES);

	if (t.kind === 'accent') {
		root.style.setProperty('--primary', t.color);
		root.style.setProperty('--primary-foreground', t.fg);
		root.style.setProperty('--accent', t.color);
		root.style.setProperty('--accent-foreground', t.fg);
	} else {
		root.classList.add(`theme-${t.id}`);
	}

	if (custom.accent) {
		const fg = isLight(custom.accent) ? ON_LIGHT : ON_DARK;
		root.style.setProperty('--primary', custom.accent);
		root.style.setProperty('--primary-foreground', fg);
		root.style.setProperty('--accent', custom.accent);
		root.style.setProperty('--accent-foreground', fg);
	}
	if (custom.hue !== null) root.style.setProperty('--hue', String(custom.hue));
	if (custom.radius !== null) root.style.setProperty('--radius', `${custom.radius}rem`);
	if (custom.fontSans) root.style.setProperty('--font-sans', custom.fontSans);
	if (custom.fontHeading) root.style.setProperty('--font-heading', custom.fontHeading);

	readBack();
}

export function applyTheme(id: ThemeId): void {
	theme.id = THEMES.some((t) => t.id === id) ? id : THEMES[0].id;
	apply();
	localStorage.setItem(KEY, theme.id);
}

export function setCustom(patch: Partial<Custom>): void {
	Object.assign(custom, patch);
	apply();
	localStorage.setItem(CUSTOM_KEY, JSON.stringify(custom));
}

export function resetCustom(): void {
	setCustom({ accent: null, hue: null, radius: null, fontSans: null, fontHeading: null });
}

/** True when nothing is overridden, so the UI can hide the reset. */
export function isDefaultCustom(): boolean {
	return Object.values(custom).every((v) => v === null);
}

/** First family in a font stack, unquoted — what the UI shows and matches on. */
export function familyName(stack: string): string {
	return (stack.split(',')[0] ?? '').replace(/["']/g, '').trim();
}

/**
 * Is this font family installed? Renders a string in it and compares the width against a fallback.
 * ponytail: a custom font that happens to measure exactly like monospace reads as missing. It's a
 * hint next to the input, not a gate — the font is applied either way.
 */
export function fontAvailable(name: string): boolean {
	const ctx = document.createElement('canvas').getContext('2d');
	if (!ctx || !name.trim()) return true;
	const probe = 'mmmmmmmmmmlli';
	ctx.font = '72px monospace';
	const base = ctx.measureText(probe).width;
	ctx.font = `72px "${name}", monospace`;
	return ctx.measureText(probe).width !== base;
}

/** Apply the stored theme + customization on startup (defaults to rose, no overrides). */
export function initTheme(): void {
	const stored = localStorage.getItem(KEY) as ThemeId | null;
	theme.id = stored && THEMES.some((t) => t.id === stored) ? stored : 'rose';
	try {
		const saved = JSON.parse(localStorage.getItem(CUSTOM_KEY) ?? '{}');
		// Only keys we know about, only the shape we expect: a hand-edited or older localStorage
		// entry must not be able to write arbitrary properties into the inline style.
		for (const k of Object.keys(custom) as (keyof Custom)[]) {
			const v = saved?.[k];
			if (typeof v === (k === 'hue' || k === 'radius' ? 'number' : 'string')) {
				(custom[k] as string | number) = v;
			}
		}
	} catch {
		// unparseable — start clean
	}
	apply();
}
