// Dominant-color sampling for the auto vinyl skin: tint the pressed disc from the
// cover's own palette instead of a fixed treatment. Results are cached per URL (cap 64,
// oldest evicted) so re-renders and track revisits don't re-decode.
//
// CORS: YouTube thumbs serve `Access-Control-Allow-Origin: *`, so the canvas stays clean;
// anything that taints (custom file:// covers, offline blobs) resolves null and the
// caller falls back to the photo skin.
const cache = new Map<string, string | null>();

export function sampleDominant(url: string): Promise<string | null> {
	const hit = cache.get(url);
	if (hit !== undefined) return Promise.resolve(hit);
	return new Promise((resolve) => {
		const img = new Image();
		img.crossOrigin = 'anonymous';
		img.onload = () => {
			let out: string | null = null;
			try {
				const c = document.createElement('canvas');
				c.width = 48;
				c.height = 48;
				const ctx = c.getContext('2d', { willReadFrequently: true });
				if (ctx) {
					ctx.drawImage(img, 0, 0, 48, 48);
					const d = ctx.getImageData(0, 0, 48, 48).data;
					let r = 0, g = 0, b = 0, w = 0;
					let fr = 0, fg = 0, fb = 0, fw = 0;
					for (let i = 0; i < d.length; i += 4) {
						if (d[i + 3] < 128) continue;
						const pr = d[i], pg = d[i + 1], pb = d[i + 2];
						const mx = Math.max(pr, pg, pb), mn = Math.min(pr, pg, pb);
						const sat = mx === 0 ? 0 : (mx - mn) / mx;
						const lum = (pr + pg + pb) / 765;
						// Vivid mid-tones decide; near-black/white/gray pixels barely vote.
						const vote = 0.15 + sat * (1 - Math.abs(lum - 0.5) * 1.6);
						r += pr * vote; g += pg * vote; b += pb * vote; w += vote;
						fr += pr; fg += pg; fb += pb; fw += 1;
					}
					if (w > 0) {
						const n = fw > 0 ? fw : 1;
						const avg = [(fr / n) | 0, (fg / n) | 0, (fb / n) | 0];
						const sat = [(r / w) | 0, (g / w) | 0, (b / w) | 0];
						// Blend vivid pick with the plain average so monochrome art (all
						// gray) still lands on its own tone instead of mud.
						const m = [
							((sat[0] + avg[0]) / 2) | 0,
							((sat[1] + avg[1]) / 2) | 0,
							((sat[2] + avg[2]) / 2) | 0
						];
						out = `rgb(${m[0]},${m[1]},${m[2]})`;
					}
				}
			} catch {
				out = null; // tainted canvas — caller falls back
			}
			if (cache.size >= 64) {
				const oldest = cache.keys().next();
				if (!oldest.done) cache.delete(oldest.value);
			}
			cache.set(url, out);
			resolve(out);
		};
		img.onerror = () => {
			if (cache.size >= 64) {
				const oldest = cache.keys().next();
				if (!oldest.done) cache.delete(oldest.value);
			}
			cache.set(url, null);
			resolve(null);
		};
		img.src = url;
	});
}
