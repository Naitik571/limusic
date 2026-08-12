<script lang="ts">
	// Aurora signature: the whole app sits on a soft ambient glow that takes its colours from
	// the current track's cover art — Apple Music's dynamic-backdrop trick. Three large blurred
	// radial blobs, painted as CSS vars so the blobs transition smoothly between tracks.
	//
	// Sampling is deliberately cheap: the art is downscaled to 8×8 and the readback bucketed by
	// quantized RGB, luminance-weighted, top-3 distinct picks. That runs once per track change —
	// the gradients themselves are compositor-only, no per-frame work.
	//
	// Local-library artwork (asset-protocol paths) can't be sampled cross-origin, so those
	// tracks keep the default accent glow (the CSS var fallbacks below).
	import { playback } from '$lib/player.svelte';

	const DIST_THRESHOLD = 90; // squared-ish RGB distance a new pick must beat to join the set

	function setGlow(vars: [string, string, string]) {
		document.documentElement.style.setProperty('--glow-1', vars[0]);
		document.documentElement.style.setProperty('--glow-2', vars[1]);
		document.documentElement.style.setProperty('--glow-3', vars[2]);
	}

	function clearGlow() {
		document.documentElement.style.removeProperty('--glow-1');
		document.documentElement.style.removeProperty('--glow-2');
		document.documentElement.style.removeProperty('--glow-3');
	}

	function sample(url: string | null | undefined) {
		if (!url || url.startsWith('/') || /^[A-Za-z]:[\\/]/.test(url)) {
			clearGlow();
			return;
		}
		const img = new Image();
		img.crossOrigin = 'anonymous';
		img.onload = () => {
			try {
				const canvas = document.createElement('canvas');
				canvas.width = 8;
				canvas.height = 8;
				const ctx = canvas.getContext('2d', { willReadFrequently: true });
				if (!ctx) return;
				ctx.drawImage(img, 0, 0, 8, 8);
				const data = ctx.getImageData(0, 0, 8, 8).data;
				// Bucket by 3-bit-per-channel quantization; weight by luminance so bright areas
				// (typically the art's real colours) win over black corners.
				const buckets = new Map<number, { r: number; g: number; b: number; w: number; n: number }>();
				for (let i = 0; i < data.length; i += 4) {
					const a = data[i + 3];
					if (a < 128) continue;
					const r = data[i];
					const g = data[i + 1];
					const b = data[i + 2];
					const key = ((r >> 5) << 6) | ((g >> 5) << 3) | (b >> 5);
					const w = 0.299 * r + 0.587 * g + 0.114 * b;
					const e = buckets.get(key);
					if (e) {
						e.r += r;
						e.g += g;
						e.b += b;
						e.w += w;
						e.n++;
					} else {
						buckets.set(key, { r, g, b, w, n: 1 });
					}
				}
				const sorted = [...buckets.values()].sort((a, b2) => b2.w - a.w);
				const picked: Array<[number, number, number]> = [];
				for (const b of sorted) {
					if (picked.length === 3) break;
					const avg: [number, number, number] = [b.r / b.n, b.g / b.n, b.b / b.n];
					const farEnough = picked.every(
						(p) =>
							(avg[0] - p[0]) ** 2 + (avg[1] - p[1]) ** 2 + (avg[2] - p[2]) ** 2 > DIST_THRESHOLD
					);
					if (farEnough) picked.push(avg);
				}
				if (picked.length === 0) {
					clearGlow();
					return;
				}
				// Darken a touch so the glow reads as atmosphere, not a screenshot.
				const css = picked.map(([r, g, b]) => `rgb(${(r * 0.72) | 0} ${(g * 0.72) | 0} ${(b * 0.72) | 0})`);
				while (css.length < 3) css.push(css[0]);
				setGlow([css[0], css[1], css[2]]);
			} catch {
				clearGlow();
			}
		};
		img.onerror = clearGlow;
		img.src = url;
	}

	$effect(() => {
		sample(playback.now?.thumbnail);
	});
</script>

<!-- Layered as the first child of the rounded root, beneath every surface. -->
<div class="ambient pointer-events-none absolute inset-0 z-0 overflow-hidden" aria-hidden="true">
	<div class="ambient-blob ambient-blob-1"></div>
	<div class="ambient-blob ambient-blob-2"></div>
	<div class="ambient-blob ambient-blob-3"></div>
</div>

<style>
	.ambient-blob {
		position: absolute;
		border-radius: 50%;
		filter: blur(90px);
		transition: background 600ms ease, opacity 600ms ease;
	}
	.ambient-blob-1 {
		width: 58vmax;
		height: 58vmax;
		top: -20vmax;
		left: -14vmax;
		background: var(--glow-1, oklch(0.62 0.22 15.458));
		opacity: 0.16;
	}
	.ambient-blob-2 {
		width: 50vmax;
		height: 50vmax;
		bottom: -18vmax;
		right: -12vmax;
		background: var(--glow-2, oklch(0.58 0.2 285));
		opacity: 0.12;
	}
	.ambient-blob-3 {
		width: 36vmax;
		height: 36vmax;
		top: 38%;
		left: 52%;
		background: var(--glow-3, oklch(0.55 0.17 45));
		opacity: 0.09;
	}
	:global(.dark) .ambient-blob {
		opacity: 0.13;
	}
	:global(.dark) .ambient-blob-2 {
		opacity: 0.1;
	}
	:global(.dark) .ambient-blob-3 {
		opacity: 0.08;
	}
</style>
