// Colour maths for the theme picker. Pure, no DOM, no dependency: the picker only needs hex <->
// HSV and a "is this light?" decision, and that is all this file.
//
// HSV (not HSL) because the picker's square IS the HSV plane — x is saturation, y is value. Driving
// it from HSL needs a fudge factor at the edges that washes the top-left corner out.

export type Hsv = { h: number; s: number; v: number }; // h 0–360, s/v 0–1

const clamp01 = (n: number) => Math.min(1, Math.max(0, n));

function hexToRgb(hex: string): [number, number, number] | null {
	const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim());
	if (!m) return null;
	const h = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
	return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as [number, number, number];
}

export function hexToHsv(hex: string): Hsv | null {
	const rgb = hexToRgb(hex);
	if (!rgb) return null;
	const [r, g, b] = rgb;
	const max = Math.max(r, g, b);
	const d = max - Math.min(r, g, b);
	let h = 0;
	if (d) {
		if (max === r) h = ((g - b) / d) % 6;
		else if (max === g) h = (b - r) / d + 2;
		else h = (r - g) / d + 4;
		h = (h * 60 + 360) % 360;
	}
	return { h, s: max ? d / max : 0, v: max };
}

export function hsvToHex({ h, s, v }: Hsv): string {
	s = clamp01(s);
	v = clamp01(v);
	// CSS Color 4's HSV->RGB, written as one channel function.
	const f = (n: number) => {
		const k = (n + h / 60) % 6;
		return Math.round(255 * v * (1 - s * Math.max(0, Math.min(k, 4 - k, 1))));
	};
	return '#' + [f(5), f(3), f(1)].map((c) => c.toString(16).padStart(2, '0')).join('');
}

/**
 * WCAG relative luminance of a hex colour (0–1). Replaces the old HSP "perceived brightness"
 * gate: foreground choice is now derived from the same luminance the contrast ratios use, so
 * the picker's dark/light decision agrees with the 4.5:1 checks in veil.ts/artcolor.ts.
 */
export function relativeLuminance(hex: string): number | null {
	const rgb = hexToRgb(hex);
	if (!rgb) return null;
	const lin = (c: number) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
	const [r, g, b] = rgb;
	return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/** WCAG contrast ratio between two hex colours (1–21). Null when either won't parse. */
export function contrastRatio(a: string, b: string): number | null {
	const la = relativeLuminance(a);
	const lb = relativeLuminance(b);
	if (la === null || lb === null) return null;
	return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/**
 * Foreground for `hex` that keeps WCAG AA (4.5:1) when one exists: the higher-contrast of
 * black/white, preferring whichever clears the target. Falls back to the higher-contrast side
 * when neither clears it (very mid tones) rather than inventing a third colour.
 */
export function bestForeground(hex: string): '#ffffff' | '#000000' {
	const onWhite = contrastRatio(hex, '#ffffff') ?? 0;
	const onBlack = contrastRatio(hex, '#000000') ?? 0;
	if (onWhite >= 4.5 && onWhite >= onBlack) return '#ffffff';
	if (onBlack >= 4.5 && onBlack >= onWhite) return '#000000';
	return onWhite >= onBlack ? '#ffffff' : '#000000';
}

/**
 * Should text on this colour be dark? WCAG relative luminance against a 0.35 threshold —
 * the crossover the preset themes already use by hand (lime/teal dark, indigo/rose light).
 */
export function isLight(hex: string): boolean {
	const l = relativeLuminance(hex);
	if (l === null) return false;
	return l > 0.35;
}

/** Lerp a hue, shortest way round the wheel: 350 -> 10 goes forward through 0, not back through 180. */
export function lerpHue(a: number, b: number, t: number): number {
	return (a + (((b - a + 540) % 360) - 180) * clamp01(t) + 360) % 360;
}
