// Orchard-style solved veil for ambient mode.
//
// The ambient backdrop is the playing artwork composited over the app background at a layer
// opacity, then dimmed by a black "veil" layer. This solves for the smallest veil alpha in
// [0.34, 0.82] that still lets the UI foreground reach WCAG 4.5:1 contrast against the
// composited result — bisection, since contrast moves monotonically with the veil alpha.

type Rgb = [number, number, number];

const VEIL_MIN = 0.34;
const VEIL_MAX = 0.82;
const CONTRAST_TARGET = 4.5;

let normCtx: CanvasRenderingContext2D | null | undefined;

/** Any CSS colour (oklch, rgb(), names…) -> sRGB, via canvas's own fillStyle normalization. */
function cssToRgb(color: string): Rgb | null {
	if (normCtx === undefined) normCtx = document.createElement('canvas').getContext('2d');
	if (!normCtx) return null;
	normCtx.fillStyle = '#000000';
	normCtx.fillStyle = color;
	const hex = typeof normCtx.fillStyle === 'string' ? normCtx.fillStyle : '';
	if (hex.length === 7) {
		return [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255) as Rgb;
	}
	if (hex.length === 4) {
		return [1, 2, 3].map((i) => parseInt(hex[i] + hex[i], 16) / 255) as Rgb;
	}
	return null;
}

/** WCAG relative luminance. */
function luminance([r, g, b]: Rgb): number {
	const lin = (c: number) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
	return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function contrast(a: Rgb, b: Rgb): number {
	const la = luminance(a);
	const lb = luminance(b);
	return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/**
 * Smallest veil alpha so `foregroundCss` keeps 4.5:1 against the artwork composited over
 * `backgroundCss` at `layerOpacity` and dimmed by a black veil at that alpha. Clamped to
 * [0.34, 0.82]; when the target is unreachable (dark foreground under a dark veil), whichever
 * bound contrasts better.
 */
export function solveVeil(
	artworkHex: string,
	foregroundCss: string,
	backgroundCss: string,
	layerOpacity: number
): number {
	const art = cssToRgb(artworkHex);
	const fg = cssToRgb(foregroundCss);
	const bg = cssToRgb(backgroundCss);
	if (!art || !fg || !bg) return VEIL_MIN;

	const k = Math.min(1, Math.max(0, layerOpacity));
	const base: Rgb = [0, 1, 2].map((i) => bg[i] * (1 - k) + art[i] * k) as Rgb;
	const scoreAt = (alpha: number) =>
		contrast(fg, base.map((c) => c * (1 - alpha)) as Rgb);

	if (scoreAt(VEIL_MIN) >= CONTRAST_TARGET) return VEIL_MIN;
	if (scoreAt(VEIL_MAX) < CONTRAST_TARGET) {
		return scoreAt(VEIL_MAX) >= scoreAt(VEIL_MIN) ? VEIL_MAX : VEIL_MIN;
	}
	let lo = VEIL_MIN;
	let hi = VEIL_MAX;
	for (let i = 0; i < 20; i++) {
		const mid = (lo + hi) / 2;
		if (scoreAt(mid) >= CONTRAST_TARGET) hi = mid;
		else lo = mid;
	}
	return hi;
}
