// Two kinds of theme live here, selected from one picker and persisted to localStorage (a pure UI
// preference, no backend round-trip):
//   - 'accent'  — overrides only --primary/--accent as inline styles on <html>, layered over the
//                 app's default palette. Wins over both :root and .dark.
//   - 'palette' — a full token set (background, card, sidebar, radius, …) for light AND dark, defined
//                 as a `.theme-<id>` class in layout.css. Applied by toggling that class on <html>.
//
// On top of whichever preset is selected sits the *custom* layer (accent colour, background tint,
// roundness, fonts). It's inline styles too, applied after the preset, so it wins over both kinds.
// Anything the user hasn't touched stays null and the preset shows through — the customization is
// a set of overrides, not a rival theme to maintain. Choosing a preset rewrites the bundle
// fields (accent, wash, follow strength, true-black, geometry) into this layer and sets the
// dark/light mode, so the preset lands as a whole look; tweaks after that are ordinary overrides.

import { convertFileSrc } from '@tauri-apps/api/core';
import { setMode } from 'mode-watcher';
import { hexToHsv, hsvToHex, isLight, lerpHue, type Hsv } from './color';
import { artworkAccent } from './artcolor';
import { solveVeil } from './veil';
import { allowFontFile } from './api';

export type ThemeId = 'midnight' | 'canopy-light' | 'aurora' | 'mono' | 'catppuccin';

export type GeometryId = 'sharp' | 'soft';

export type LayoutId = 'grove' | 'canopy' | 'poolside';

export const LAYOUTS: { id: LayoutId; label: string; description: string }[] = [
	{ id: 'grove', label: 'Grove', description: 'Balanced sidebar + feed - the classic Limusic look' },
	{ id: 'canopy', label: 'Canopy', description: 'Transport in the top bar - no bottom bar at all' },
	{ id: 'poolside', label: 'Poolside (Beta)', description: 'Y2K aqua-pool vinyl deck — full-app reskin' }
];

// `fg` (accent themes only) is the text/icon colour that sits ON the accent: light accents (mono)
// need a dark foreground; dark accents keep the light one. `color` is just the picker swatch.
// `description` is the one-liner the Settings preset picker shows under the name (layouts already
// have them; themes had none). `className` (accent only) is an optional surface hook toggled on
// <html> alongside the inline accent vars — how midnight/aurora/mono get their own surfaces while
// staying accent-kind (see layout.css `.theme-midnight` etc, mirroring the catppuccin pattern).
// A preset is a whole look, not just an accent: choosing one sets the dark/light mode plus a
// bundle of custom-layer defaults (accent + wash + follow strength + true-black + geometry).
// `accent` is oklch like the preset itself (converted to hex on write — custom.accent is hex);
// null means the preset owns its accent per mode, so no override is written and a palette's own
// light/dark primaries survive (canopy-light, catppuccin).
export type PresetLook = {
	accent: string | null;
	mode: 'dark' | 'light';
	wash: number;
	followStrength: number;
	trueBlack: boolean;
	geometry: GeometryId;
};

type Theme =
	| { id: ThemeId; label: string; description: string; look: PresetLook; kind: 'accent'; color: string; fg: string; className?: string }
	| { id: ThemeId; label: string; description: string; look: PresetLook; kind: 'palette'; color: string };

export const THEMES: Theme[] = [
	{ id: 'midnight', label: 'Midnight', description: 'True-black OLED + electric violet — the default', kind: 'accent', color: 'oklch(0.65 0.27 297)', fg: 'oklch(0.985 0 0)', className: 'theme-midnight', look: { accent: 'oklch(0.65 0.27 297)', mode: 'dark', wash: 30, followStrength: 60, trueBlack: true, geometry: 'soft' } },
	{ id: 'aurora', label: 'Aurora', description: 'Artwork-follow showcase — surfaces chase the cover', kind: 'accent', color: 'oklch(0.68 0.24 320)', fg: 'oklch(0.985 0 0)', className: 'theme-aurora', look: { accent: 'oklch(0.68 0.24 320)', mode: 'dark', wash: 55, followStrength: 80, trueBlack: false, geometry: 'soft' } },
	{ id: 'mono', label: 'Mono', description: 'Near-white accent, zero-chroma grayscale surfaces', kind: 'accent', color: 'oklch(0.93 0.005 0)', fg: 'oklch(0.205 0 0)', className: 'theme-mono', look: { accent: 'oklch(0.93 0.005 0)', mode: 'dark', wash: 10, followStrength: 30, trueBlack: false, geometry: 'sharp' } },
	{ id: 'canopy-light', label: 'Canopy Light', description: 'Warm-paper light theme — ink text, deep accent', kind: 'palette', color: 'oklch(0.48 0.16 55)', look: { accent: null, mode: 'light', wash: 25, followStrength: 60, trueBlack: false, geometry: 'soft' } },
	{ id: 'catppuccin', label: 'Catppuccin', description: 'Pastel community palette — light + dark', kind: 'palette', color: 'oklch(0.5547 0.2503 297.0156)', look: { accent: null, mode: 'dark', wash: 30, followStrength: 60, trueBlack: false, geometry: 'soft' } }
];

/** Retired single-hue presets (rose/blue/lime/purple/teal). Kept as one-click swatches that write
 *  the custom accent override — and as the migration source, so a stored retired id keeps its hue.
 *  `oklch` is the original preset color (for migration via toHex); `hex` is the click target. */
export const RETIRED_ACCENTS: { id: string; label: string; oklch: string; hex: string }[] = [
	{ id: 'rose', label: 'Rose', oklch: 'oklch(0.455 0.188 13.697)', hex: '#f43f5e' },
	{ id: 'blue', label: 'Blue', oklch: 'oklch(0.49 0.22 264)', hex: '#3b82f6' },
	{ id: 'lime', label: 'Lime', oklch: 'oklch(0.77 0.2 131)', hex: '#a3e635' },
	{ id: 'purple', label: 'Purple', oklch: 'oklch(0.56 0.25 302)', hex: '#a855f7' },
	{ id: 'teal', label: 'Teal', oklch: 'oklch(0.85 0.13 181)', hex: '#2dd4bf' }
];

/** Font stacks bundled with the app (imported in layout.css). "System" needs no download.
    One UI stack (Inter) + mono (JetBrains Mono): the novelty display faces left with the
    deleted palettes. Lyrics keep their own picker (content typography, untouched). */
export const FONTS: { label: string; value: string }[] = [
	{ label: 'Inter', value: "'Inter Variable', sans-serif" },
	{ label: 'JetBrains Mono', value: "'JetBrains Mono Variable', sans-serif" },
	{ label: 'OpenDyslexic', value: "'OpenDyslexic', sans-serif" },
	{ label: 'System', value: 'ui-sans-serif, system-ui, sans-serif' }
];

/** The custom layer. `null` = untouched, so the selected preset decides. */
export type Custom = {
	accent: string | null; // hex
	hue: number | null; // 0–360, tints the default palette's neutrals
	radius: number | null; // rem
	fontSans: string | null; // a CSS font-family value
	fontHeading: string | null;
	// Premium color mechanics (appearance/custom layer, persisted):
	//   wash 0–100 — how far the accent bleeds into surfaces (CSS var --wash).
	//   followStrength 0–100 — artwork-follow chroma cap (replaces the fixed ART_TINT cap).
	//   trueBlack — pure #000 backgrounds, elevation via borders (class `true-black`).
	//   geometry — corner sharpness scale (classes geo-sharp/geo-soft, var --geo).
	wash: number | null;
	followStrength: number | null;
	trueBlack: boolean | null;
	geometry: GeometryId | null;
	// Font files the user loaded from disk, by absolute path. Not an override — a small library that
	// both font rows can then choose from, which is why `resetCustom` leaves it alone.
	fontFiles: string[];
};

export const DEFAULT_WASH = 30;
export const DEFAULT_FOLLOW_STRENGTH = 60;
export const DEFAULT_GEOMETRY: GeometryId = 'soft';

const KEY = 'primary-theme';
const CUSTOM_KEY = 'custom-theme';
const APPEARANCE_KEY = 'appearance';
const LAYOUT_KEY = 'layout';
const GLASS_KEY = 'glass-intensity';
/** Every theme surface hook toggled on <html> (palette classes + accent-with-class hooks like
 *  theme-midnight/theme-aurora/theme-mono). Removed together in apply() before the new one goes on. */
const THEME_CLASSES = ['theme-midnight', 'theme-aurora', 'theme-mono', 'theme-canopy-light', 'theme-catppuccin'];
const LAYOUT_CLASSES = LAYOUTS.map((l) => `layout-${l.id}`);
const ACCENT_VARS = ['--primary', '--primary-foreground', '--accent', '--accent-foreground'];
/** Set on <html> while the artwork tint is live; the surface rules in layout.css hang off it. */
const TINT_CLASS = 'art-tint';
/** True-black toggle hook: backgrounds to #000, elevation via borders (layout.css). */
const TRUE_BLACK_CLASS = 'true-black';
const GEO_CLASSES = ['geo-sharp', 'geo-soft'];
const CUSTOM_VARS = ['--hue', '--radius', '--font-sans', '--font-heading', '--wash', '--geo'];
// Same two neutrals the preset accent themes pick between.
const ON_DARK = 'oklch(0.985 0 0)';
const ON_LIGHT = 'oklch(0.205 0 0)';

/** Reactive current selection, so the picker reflects it. */
export const theme = $state<{ id: ThemeId }>({ id: 'midnight' });
export const layout = $state<{ id: LayoutId }>({ id: 'grove' });
export const custom = $state<Custom>({
	accent: null,
	hue: null,
	radius: null,
	fontSans: null,
	fontHeading: null,
	wash: null,
	followStrength: null,
	trueBlack: null,
	geometry: null,
	fontFiles: []
});

/**
 * Looks the UI reads directly rather than through a CSS token. Same store and same reasoning as
 * the theme above (a pure UI preference, no backend round-trip), which also means a component can
 * read it during its first render instead of flashing the default while a command round-trips.
 */
export type AmbientIntensity = 'subtle' | 'balanced' | 'vivid';
export type LyricFontId =
	| 'system'
	| 'plus-jakarta'
	| 'outfit'
	| 'dm-sans'
	| 'space-grotesk'
	| 'inter';

export const LYRIC_FONTS: { label: string; id: LyricFontId; className: string }[] = [
	{ label: 'System', id: 'system', className: '' },
	{ label: 'Plus Jakarta Sans', id: 'plus-jakarta', className: 'lyrics-font-plus-jakarta' },
	{ label: 'Outfit', id: 'outfit', className: 'lyrics-font-outfit' },
	{ label: 'DM Sans', id: 'dm-sans', className: 'lyrics-font-dm-sans' },
	{ label: 'Space Grotesk', id: 'space-grotesk', className: 'lyrics-font-space-grotesk' },
	{ label: 'Inter', id: 'inter', className: 'lyrics-font-inter' }
];
const LYRIC_FONT_CLASSES = LYRIC_FONTS.map((f) => `lyrics-font-${f.id}`);

export const appearance = $state({
	artworkBackground: true,
	tabbedPlayer: true,
	artworkAccent: false,
	ambientMode: false,
	ambientIntensity: 'balanced' as AmbientIntensity,
	immersiveBackgroundIntensity: 0.12,
	lyricsFont: 'system' as LyricFontId
});

// --- Glass intensity -------------------------------------------------------------------------
// Scales every frosted surface (`.glass`/`.glass-strong`/`.glass-edge`, layout.css) through two
// CSS vars on <html>: --glass-blur (the backdrop blur radius) and --glass-alpha (a multiplier on
// each palette's translucent fill). 100 = today's defaults; 0 = a subtle 4px, near-clear glass.

export const glass = $state<{ intensity: number }>({ intensity: 100 });

export function setGlassIntensity(v: number): void {
	const clamped = Math.min(100, Math.max(0, Math.round(v)));
	glass.intensity = clamped;
	localStorage.setItem(GLASS_KEY, String(clamped));
	const k = clamped / 100;
	const root = document.documentElement;
	root.style.setProperty('--glass-blur', `${(4 + k * 28).toFixed(1)}px`);
	root.style.setProperty('--glass-alpha', k.toFixed(2));
}

/** Reads the persisted intensity at startup; anything unreadable falls back to the default. */
function initGlass(): void {
	let v = NaN;
	try {
		const raw = localStorage.getItem(GLASS_KEY);
		if (raw !== null) v = Number(raw);
	} catch {
		// storage unavailable — keep the default
	}
	setGlassIntensity(Number.isFinite(v) ? v : 100);
}

export function setAppearance(patch: Partial<typeof appearance>): void {
	Object.assign(appearance, patch);
	if (patch.ambientIntensity) {
		const map: Record<AmbientIntensity, number> = { subtle: 0.06, balanced: 0.12, vivid: 0.22 };
		appearance.immersiveBackgroundIntensity = map[patch.ambientIntensity] ?? 0.12;
	}
	applyAmbient();
	applyLyricsFont();
	localStorage.setItem(APPEARANCE_KEY, JSON.stringify(appearance));
}

/** Ambient intensity -> artwork backdrop layer opacity (subtle/balanced/vivid). */
const AMBIENT_LAYER_OPACITY: Record<AmbientIntensity, number> = {
	subtle: 0.5,
	balanced: 0.7,
	vivid: 0.85
};

function applyAmbient(): void {
	const root = document.documentElement;
	const enabled = appearance.ambientMode;
	const layerOpacity = AMBIENT_LAYER_OPACITY[appearance.ambientIntensity] ?? 0.7;
	root.style.setProperty('--ambient-opacity', enabled ? String(layerOpacity) : '0');
	let veil = '0';
	if (enabled) {
		const cs = getComputedStyle(root);
		veil = solveVeil(
			artAccentHex ?? effective.accent,
			cs.getPropertyValue('--foreground'),
			cs.getPropertyValue('--background'),
			layerOpacity
		).toFixed(3);
	}
	root.style.setProperty('--ambient-veil', veil);
	root.classList.toggle('ambient-on', enabled);
	root.setAttribute('data-ambient-intensity', appearance.ambientIntensity);
}

export function applyLyricsFont(): void {
	const root = document.documentElement;
	root.classList.remove(...LYRIC_FONT_CLASSES);
	const match = LYRIC_FONTS.find((f) => f.id === appearance.lyricsFont);
	if (match?.className) root.classList.add(match.className);
}

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
	fontHeading: '',
	wash: DEFAULT_WASH,
	followStrength: DEFAULT_FOLLOW_STRENGTH,
	geometry: DEFAULT_GEOMETRY as GeometryId,
	trueBlack: false
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
	const washRaw = parseFloat(g('--wash'));
	effective.wash = Number.isFinite(washRaw) ? Math.round(washRaw * 100) : (custom.wash ?? DEFAULT_WASH);
	effective.followStrength = custom.followStrength ?? DEFAULT_FOLLOW_STRENGTH;
	effective.geometry = custom.geometry ?? DEFAULT_GEOMETRY;
	effective.trueBlack = custom.trueBlack ?? false;
}

/** Write the accent quartet as inline vars on <html>, foreground picked for legibility on it. */
function setAccentVars(color: string): void {
	const fg = isLight(color) ? ON_LIGHT : ON_DARK;
	const root = document.documentElement;
	root.style.setProperty('--primary', color);
	root.style.setProperty('--primary-foreground', fg);
	root.style.setProperty('--accent', color);
	root.style.setProperty('--accent-foreground', fg);
}

function apply(): void {
	const t = THEMES.find((x) => x.id === theme.id) ?? THEMES[0];
	const root = document.documentElement;
	[...ACCENT_VARS, ...CUSTOM_VARS, '--art-h', '--lyrics-font-family'].forEach((v) => root.style.removeProperty(v));
	root.classList.remove(...THEME_CLASSES, ...GEO_CLASSES, TRUE_BLACK_CLASS, TINT_CLASS, ...LYRIC_FONT_CLASSES);

	if (t.kind === 'accent') {
		root.style.setProperty('--primary', t.color);
		root.style.setProperty('--primary-foreground', t.fg);
		root.style.setProperty('--accent', t.color);
		root.style.setProperty('--accent-foreground', t.fg);
		if (t.className) root.classList.add(t.className);
	} else {
		root.classList.add(`theme-${t.id}`);
	}

	if (custom.accent) setAccentVars(custom.accent);
	if (art) setArtVars(art);
	if (custom.hue !== null) root.style.setProperty('--hue', String(custom.hue));
	if (custom.radius !== null) root.style.setProperty('--radius', `${custom.radius}rem`);
	if (custom.fontSans) root.style.setProperty('--font-sans', custom.fontSans);
	if (custom.fontHeading) root.style.setProperty('--font-heading', custom.fontHeading);

	// Wash intensity 0–100 -> --wash 0–1: how far the accent bleeds into surfaces
	// (layout.css dark/light tokens color-mix toward --accent by it).
	const wash = custom.wash ?? DEFAULT_WASH;
	root.style.setProperty('--wash', (Math.min(100, Math.max(0, wash)) / 100).toFixed(2));
	// Geometry Sharp/Soft -> --geo factor + hook class (layout.css scales --r-* by it).
	const geo = custom.geometry ?? DEFAULT_GEOMETRY;
	root.classList.add(geo === 'sharp' ? 'geo-sharp' : 'geo-soft');
	root.style.setProperty('--geo', geo === 'sharp' ? '0.55' : '1');
	// True-black OLED toggle.
	if (custom.trueBlack) root.classList.add(TRUE_BLACK_CLASS);

	readBack();
	applyAmbient();
	applyLyricsFont();
}

/**
 * Choosing a preset applies its whole look: the dark/light mode plus the bundle written into
 * the custom layer (accent included for accent-kind; palette-kind writes null so the palette's
 * own per-mode primaries show through). The wash/follow/true-black/geometry controls then sit
 * where the look sits, and any tweak after is an ordinary custom override. Reset clears custom
 * back to the pure preset — visually identical, because bundle == preset.
 */
export function applyTheme(id: ThemeId): void {
	const next = THEMES.find((t) => t.id === id) ?? THEMES[0];
	theme.id = next.id;
	const look = next.look;
	// The bundle speaks oklch like the preset itself; custom.accent is hex. A '#000000' result
	// means this engine can't parse oklch (canvas fallback) — store null so the preset's own
	// inline vars show through instead of flashing black.
	let accentHex: string | null = null;
	if (look.accent) {
		const hex = toHex(look.accent);
		accentHex = hex === '#000000' ? null : hex;
	}
	custom.accent = accentHex;
	custom.wash = look.wash;
	custom.followStrength = look.followStrength;
	custom.trueBlack = look.trueBlack;
	custom.geometry = look.geometry;
	// Same path as the sidebar toggle (mode-watcher's own storage key, `.dark` on <html>) —
	// picking a preset persists the mode exactly like flipping it by hand.
	setMode(look.mode);
	apply();
	persist();
	localStorage.setItem(KEY, theme.id);
	// ModeWatcher flips `.dark` in its own reaction, a tick after the state change, so re-read
	// once it lands — palette primaries differ per mode and `effective` drives the swatches.
	requestAnimationFrame(() => readBack());
}

export function applyLayout(id: LayoutId): void {
	const valid = (LAYOUTS.some((l) => l.id === id) ? id : 'grove') as LayoutId;
	// Remember where we came from when entering poolside so its exit restores it
	// (never hardcoded). try/catch: storage may be denied.
	try {
		if (valid === 'poolside' && layout.id !== 'poolside') {
			localStorage.setItem('ps-prev-layout', layout.id);
		}
	} catch {
		/* exit falls back to grove */
	}
	layout.id = valid;
	const root = document.documentElement;
	root.classList.remove(...LAYOUT_CLASSES);
	root.classList.add(`layout-${valid}`);
	// Orchard scopes its per-layout stylesheet to this attribute (component-carried CSS hangs off
	// it); the class above stays for the rules in layout.css.
	root.dataset.layoutPreset = valid;
	localStorage.setItem(LAYOUT_KEY, valid);
}

export function initLayout(): void {
	let stored: LayoutId | null = null;
	try {
		stored = localStorage.getItem(LAYOUT_KEY) as LayoutId | null;
	} catch {
		// storage denied/unavailable — fall through to the default layout
	}
	const id = (stored && LAYOUTS.some((l) => l.id === stored) ? stored : 'grove') as LayoutId;
	layout.id = id;
	const root = document.documentElement;
	root.classList.remove(...LAYOUT_CLASSES);
	root.classList.add(`layout-${id}`);
	root.dataset.layoutPreset = id;
}

const persist = () => localStorage.setItem(CUSTOM_KEY, JSON.stringify(custom));

export function setCustom(patch: Partial<Custom>): void {
	Object.assign(custom, patch);
	apply();
	persist();
}

/** Drops the overrides. Loaded font *files* stay: they're assets, not a setting. */
export function resetCustom(): void {
	setCustom({ accent: null, hue: null, radius: null, fontSans: null, fontHeading: null, wash: null, followStrength: null, trueBlack: null, geometry: null });
}

/** True when nothing is overridden, so the UI can disable the reset. */
export function isDefaultCustom(): boolean {
	return !custom.accent && custom.hue === null && custom.radius === null && !custom.fontSans && !custom.fontHeading && custom.wash === null && custom.followStrength === null && custom.trueBlack === null && custom.geometry === null;
}

/** Follow-strength cap 0–1 for the artwork tint (slider 0–100, default ~60%).
 *  Aurora gets +0.2 headroom (clamped): same slider, more aggressive surfaces. */
export function followCap(): number {
	const base = Math.min(100, Math.max(0, custom.followStrength ?? DEFAULT_FOLLOW_STRENGTH)) / 100;
	if (theme.id === 'aurora') return Math.min(1, base + 0.2);
	return base;
}

// --- Font files loaded from disk -------------------------------------------------------------
// Each file becomes an @font-face keyed on its filename, so it shows up in both font dropdowns
// like a bundled family. Only the path is stored: re-reading the file each launch keeps a 4 MB
// variable font out of localStorage, at the cost of the entry going dead if the file moves.

const FONT_STYLE_ID = 'custom-font-files';

/** Family name for a loaded file: its base name, minus extension and anything CSS-unsafe. */
export function fileFamily(path: string): string {
	const base = path.split(/[\\/]/).pop() ?? path;
	// ponytail: two files with the same name collide on one family — last one registered wins.
	return base.replace(/\.[^.]+$/, '').replace(/[^\w \-]/g, '').trim() || 'Custom font';
}

/** Loaded files as font-dropdown entries, so they sit alongside the bundled ones. */
export function fileFonts(): { label: string; value: string }[] {
	return custom.fontFiles.map((p) => ({
		label: fileFamily(p),
		value: `'${fileFamily(p)}', sans-serif`
	}));
}

/** Drop a path from the library, and clear any font row still pointing at its family. */
function forget(path: string): void {
	custom.fontFiles = custom.fontFiles.filter((p) => p !== path);
	const gone = `'${fileFamily(path)}', sans-serif`;
	if (custom.fontSans === gone) custom.fontSans = null;
	if (custom.fontHeading === gone) custom.fontHeading = null;
}

/**
 * (Re)build the `@font-face` rules, and forget any file that has since been deleted or moved —
 * the grant is what tells us, since it checks the file exists. Without the pruning, a font that is
 * no longer on disk keeps its dropdown entry and its row keeps *claiming* to use it while the app
 * silently renders the fallback. Runs at startup and whenever the settings modal opens.
 */
export async function registerFontFiles(): Promise<void> {
	const rules: string[] = [];
	const missing: string[] = [];
	for (const path of custom.fontFiles) {
		try {
			// Grant the URL before the rule exists: a font that 403s once is never retried.
			await allowFontFile(path);
		} catch {
			missing.push(path);
			continue;
		}
		rules.push(
			`@font-face { font-family: '${fileFamily(path)}'; src: url('${convertFileSrc(path)}'); }`
		);
	}
	if (missing.length) {
		missing.forEach(forget);
		persist();
		apply(); // a row that pointed at a missing font falls back to the preset's, visibly
	}
	let el = document.getElementById(FONT_STYLE_ID);
	if (!el) {
		el = document.createElement('style');
		el.id = FONT_STYLE_ID;
		document.head.append(el);
	}
	el.textContent = rules.join('\n');
}

/** Load a font file the user picked. Throws (for the caller to report) if it can't be granted. */
export async function addFontFile(path: string): Promise<string> {
	await allowFontFile(path);
	if (!custom.fontFiles.includes(path)) custom.fontFiles.push(path);
	persist();
	await registerFontFiles();
	return fileFamily(path);
}

export function removeFontFile(path: string): void {
	forget(path);
	persist();
	apply();
	registerFontFiles();
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

// --- Artwork colours --------------------------------------------------------------------------
// The playing cover's colour, layered on top of the preset and the custom accent, so switching it
// off puts the user's own theme straight back. Not persisted: it's derived from whatever is
// playing, and the next track overwrites it.
//
// Two things come out of one colour: the accent quartet (inline vars, as everywhere else) and
// --art-h, the hue every surface in the `.art-tint` rules is derived from (layout.css). That's why
// the crossfade runs in HSV rather than mixing two hex values: the hue has to travel the short way
// round the wheel, and sRGB mixing would take the whole palette through grey on the way.

let art: Hsv | null = null;
let frame = 0;
let wanted = '';
/** Raw hex from the last successful artworkAccent() fetch, for the ambient veil solver. */
let artAccentHex: string | null = null;

/** Push the current artwork colour into the accent vars and the tint hue. */
function setArtVars(c: Hsv): void {
	setAccentVars(hsvToHex(c));
	document.documentElement.style.setProperty('--art-h', c.h.toFixed(1));
	document.documentElement.classList.add(TINT_CLASS);
}

/** Crossfade to `to` over ~450 ms. A snap between two palettes is the thing this setting would
 *  otherwise be judged on. */
function fadeTo(to: Hsv): void {
	const from = art ?? hexToHsv(effective.accent) ?? to;
	cancelAnimationFrame(frame);
	const t0 = performance.now();
	const step = (now: number) => {
		const k = Math.min(1, (now - t0) / 450);
		const e = k * k * (3 - 2 * k); // smoothstep
		art = {
			h: lerpHue(from.h, to.h, e),
			s: from.s + (to.s - from.s) * e,
			v: from.v + (to.v - from.v) * e
		};
		if (k < 1) {
			setArtVars(art);
			frame = requestAnimationFrame(step);
		} else {
			art = to;
			apply(); // land through the normal path so `effective` (and the pickers) agree
		}
	};
	frame = requestAnimationFrame(step);
}

/**
 * Point the theme at a cover URL. `null`/undefined (setting off, nothing playing) drops the layer
 * immediately; an unreadable or colourless image leaves the current colours alone rather than
 * flashing to grey.
 */
let artReq = 0;
export function applyArtworkAccent(url: string | undefined | null): void {
	wanted = url ?? '';
	const my = ++artReq;
	if (!url) {
		artAccentHex = null;
		cancelAnimationFrame(frame);
		if (art) {
			art = null;
			apply();
		}
		return;
	}
	artworkAccent(url).then((hex) => {
		// Only the latest decode applies: a slow fetch for a skipped track must not repaint
		// over the current one. A null/unparseable result leaves the theme alone rather than
		// flashing black (toHex's '#000000' fallback is for reading tokens, never for fading).
		if (my !== artReq || wanted !== url) return;
		if (!hex) return;
		const raw = hexToHsv(hex);
		if (!raw) return;
		// Follow-strength chroma cap: a neon cover must not drag surfaces with it. The capped
		// hex is what both the crossfade and the ambient veil solver read, so they never disagree.
		// (Was a fixed ART_TINT_MAX_SATURATION 0.6; now the Follow slider, default ~60%.)
		const cap = followCap();
		const capped: Hsv = raw.s > cap ? { ...raw, s: cap } : raw;
		artAccentHex = hsvToHex(capped);
		fadeTo(capped);
	});
}

/** Apply the stored theme + customization on startup (defaults to midnight, no overrides).
 *  Migration: retired ids (rose/blue/lime/purple/teal) all map to midnight, preserving the user's
 *  hue by writing the old preset color into the custom accent override (when untouched), so
 *  nothing visually changes for them. Unknown ids fall through to midnight. Catppuccin stays.
 *  Upgrade guarantee: existing custom.* overrides are never rewritten here — preset bundles apply
 *  only on explicit choice (applyTheme), so every upgrader keeps their current effective look. */
export function initTheme(): void {
	try {
		const saved = JSON.parse(localStorage.getItem(CUSTOM_KEY) ?? '{}');
		// Only keys we know about, only the shape we expect: a hand-edited or older localStorage
		// entry must not be able to write arbitrary properties into the inline style.
		for (const k of ['accent', 'fontSans', 'fontHeading'] as const) {
			if (typeof saved?.[k] === 'string') custom[k] = saved[k];
		}
		for (const k of ['hue', 'radius', 'wash', 'followStrength'] as const) {
			if (typeof saved?.[k] === 'number') (custom as any)[k] = saved[k];
		}
		if (typeof saved?.trueBlack === 'boolean') custom.trueBlack = saved.trueBlack;
		if (saved?.geometry === 'sharp' || saved?.geometry === 'soft') custom.geometry = saved.geometry;
		if (Array.isArray(saved?.fontFiles)) {
			custom.fontFiles = saved.fontFiles.filter((p: unknown) => typeof p === 'string');
		}
		// Deleted font stacks (Oxanium, Bungee, …) fall back to the preset default instead of
		// rendering an unloaded family name. Loaded files always stay valid.
		{
			const allowed = new Set([
				...FONTS.map((f) => f.value),
				...fileFonts().map((f) => f.value)
			]);
			if (custom.fontSans && !allowed.has(custom.fontSans)) custom.fontSans = null;
			if (custom.fontHeading && !allowed.has(custom.fontHeading)) custom.fontHeading = null;
		}
		// Clamp the new mechanics into range; out-of-range hand-edits fall back to untouched.
		if (typeof custom.wash === 'number' && !(custom.wash >= 0 && custom.wash <= 100)) custom.wash = null;
		if (typeof custom.followStrength === 'number' && !(custom.followStrength >= 0 && custom.followStrength <= 100)) custom.followStrength = null;
	} catch {
		// unparseable — start clean
	}
	let stored: string | null = null;
	try {
		stored = localStorage.getItem(KEY);
	} catch {
		// storage denied/unavailable — fall through to the default theme
	}
	const retired = new Map(RETIRED_ACCENTS.map((r) => [r.id, r]));
	if (stored && retired.has(stored)) {
		theme.id = 'midnight';
		// Preserve the hue: a user who never touched the custom accent keeps seeing it.
		if (!custom.accent) {
			const r = retired.get(stored)!;
			let hex = r.hex;
			try {
				const converted = toHex(r.oklch);
				if (converted && converted !== '#000000') hex = converted;
			} catch {
				// canvas unavailable — keep the static hex fallback
			}
			custom.accent = hex;
			try {
				localStorage.setItem(CUSTOM_KEY, JSON.stringify(custom));
			} catch {
				// storage unavailable — theme still migrates for this session
			}
		}
		try {
			localStorage.setItem(KEY, theme.id);
		} catch {
			// ignore
		}
	} else {
		theme.id = stored && THEMES.some((t) => t.id === stored) ? (stored as ThemeId) : 'midnight';
		if (theme.id !== stored) {
			try {
				localStorage.setItem(KEY, theme.id);
			} catch {
				// ignore
			}
		}
	}
	try {
		const saved = JSON.parse(localStorage.getItem(APPEARANCE_KEY) ?? '{}');
		for (const k of ['artworkBackground', 'tabbedPlayer', 'artworkAccent', 'ambientMode'] as const) {
			if (typeof saved?.[k] === 'boolean') (appearance as any)[k] = saved[k];
		}
		if (typeof saved?.ambientIntensity === 'string' && ['subtle', 'balanced', 'vivid'].includes(saved.ambientIntensity)) {
			(appearance as any).ambientIntensity = saved.ambientIntensity;
		}
		if (typeof saved?.immersiveBackgroundIntensity === 'number') {
			(appearance as any).immersiveBackgroundIntensity = Math.min(1, Math.max(0, saved.immersiveBackgroundIntensity));
		} else if (typeof saved?.ambientIntensity === 'string') {
			const map: Record<string, number> = { subtle: 0.06, balanced: 0.12, vivid: 0.22 };
			(appearance as any).immersiveBackgroundIntensity = map[saved.ambientIntensity] ?? 0.12;
		}
		if (typeof saved?.lyricsFont === 'string' && LYRIC_FONTS.some((f) => f.id === saved.lyricsFont)) {
			(appearance as any).lyricsFont = saved.lyricsFont;
		}
	} catch {
		// unparseable — keep the defaults
	}
	apply();
	initLayout();
	initGlass();
	// Async (each file needs its URL granted first), so the app paints in the fallback font for a
	// frame or two before a loaded font swaps in.
	if (custom.fontFiles.length) registerFontFiles();
}
