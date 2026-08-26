/**
 * Liquid-glass lens for poolside glass panels — compact port of the SDF
 * displacement-map refraction technique (samasante/liquid-glass).
 *
 * Each pixel of a rounded-rect map encodes:
 *   R — X displacement (128 = neutral)
 *   G — Y displacement (128 = neutral)
 *   B — specular mask (edges lift toward white)
 * An injected SVG filter (feImage + feDisplacementMap) refracts the element's
 * backdrop through the map. Combine with `backdrop-filter: blur()` on the same
 * element: Chromium applies backdrop first, then the filter refracts the result.
 */

const injected = new Map<string, string>();

/** Build (once per size+strength) and return the `filter: url(#id)` value. */
export function liquidLens(
	el: HTMLElement,
	opts: { id?: string; strength?: number; edge?: number; radius?: number } = {}
): string {
	const rect = el.getBoundingClientRect();
	const w = Math.max(2, Math.round(rect.width));
	const h = Math.max(2, Math.round(rect.height));
	const strength = opts.strength ?? 46;
	const edgeStart = opts.edge ?? 0.55;
	const radius = opts.radius ?? Math.min(24, h / 2);
	const id = opts.id ?? `ps-lens-${w}x${h}-${strength}`;
	if (injected.has(id)) return `url(#${id})`;

	const map = document.createElement('canvas');
	map.width = w;
	map.height = h;
	const ctx = map.getContext('2d');
	if (!ctx) return '';
	const img = ctx.createImageData(w, h);
	const hw = w / 2;
	const hh = h / 2;
	// rounded-rect half-size for the SDF
	const rw = hw - radius;
	const rh = hh - radius;
	for (let y = 0; y < h; y++) {
		for (let x = 0; x < w; x++) {
			const i = (y * w + x) * 4;
			// rounded-rect SDF (approx): distance from the inner rect
			const dx = Math.abs(x - hw) - rw;
			const dy = Math.abs(y - hh) - rh;
			const outside = Math.min(Math.max(dx, dy), 0) + Math.hypot(Math.max(dx, 0), Math.max(dy, 0));
			// 0 at center → 1 at the edge
			const t = Math.min(1, Math.max(0, outside / radius));
			const edge = Math.pow(t, 1.4);
			// displacement points inward (toward center)
			const dirX = x === hw ? 0 : (hw - x) / Math.abs(hw - x);
			const dirY = y === hh ? 0 : (hh - y) / Math.abs(hh - y);
			img.data[i] = 128 + dirX * edge * 127 * (t > 0 ? 1 : 0);
			img.data[i + 1] = 128 + dirY * edge * 127 * (t > 0 ? 1 : 0);
			img.data[i + 2] = 128 + edge * 127;
			img.data[i + 3] = 255;
		}
	}
	ctx.putImageData(img, 0, 0);
	const href = map.toDataURL('image/png');

	const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
	svg.setAttribute('width', '0');
	svg.setAttribute('height', '0');
	svg.style.position = 'absolute';
	svg.innerHTML =
		`<filter id="${id}" x="0" y="0" width="100%" height="100%" color-interpolation-filters="sRGB">` +
		`<feImage href="${href}" x="0" y="0" width="${w}" height="${h}" result="map" preserveAspectRatio="none" />` +
		`<feDisplacementMap in="SourceGraphic" in2="map" scale="${strength}" xChannelSelector="R" yChannelSelector="G" />` +
		`</filter>`;
	document.body.appendChild(svg);
	injected.set(id, href);
	return `url(#${id})`;
}
