<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import { listen } from '@tauri-apps/api/event';
	import { playback } from '$lib/player.svelte';

	interface Props {
		/** Number of bars. */
		bars?: number;
		/** CSS color for the bars (defaults to the accent). */
		color?: string;
		class?: string;
	}
	let { bars = 28, color = 'var(--accent, #1db954)', class: cls = '' }: Props = $props();

	let canvas: HTMLCanvasElement | undefined = $state();
	let enabled = $state(true);

	function readSetting() {
		api.getSettings().then((s) => (enabled = s.visualizer !== 'false')).catch(() => {});
	}

	onMount(() => {
		readSetting();
		const unlisten = listen<{ key: string; value: string }>('setting-changed', (e) => {
			if (e.payload.key === 'visualizer') enabled = e.payload.value === 'true';
		}).then((u) => u);

		// Per-bar scratch state: array of smoothed heights + a phase offset so each bar dances
		// on a slightly different beat. Local to this module instance, mutated every frame.
		const levels = new Array(bars).fill(0);
		const phase = Array.from({ length: bars }, (_, i) => (i * 0.37) % (Math.PI * 2));

		const ctx = canvas?.getContext('2d') ?? null;
		let raf = 0;
		let last = performance.now();
		const draw = (now: number) => {
			raf = requestAnimationFrame(draw);
			if (!ctx || !canvas) return;
			const dt = Math.min(0.05, (now - last) / 1000);
			last = now;

			const w = canvas.clientWidth;
			const h = canvas.clientHeight;
			// Keep the backing store crisp on HiDPI without re-allocating every frame.
			if (canvas.width !== Math.round(w * devicePixelRatio) || canvas.height !== Math.round(h * devicePixelRatio)) {
				canvas.width = Math.round(w * devicePixelRatio);
				canvas.height = Math.round(h * devicePixelRatio);
			}
			ctx.setTransform(devicePixelRatio, 0, 0, devicePixelRatio, 0, 0);
			ctx.clearRect(0, 0, w, h);

			// A track that's playing pulses with a musical cadence; paused = rest.
			const playing = !playback.paused && playback.now;
			const t = now / 1000;
			const gap = 2;
			const bw = (w - gap * (bars - 1)) / bars;
			ctx.fillStyle = color;
			for (let i = 0; i < bars; i++) {
				let target = 0;
				if (playing) {
					// Deterministic, music-like motion: a few sine bands per bar, slightly
					// out of phase, with a bass-weighted falloff. Looks like an FFT, costs ~nothing.
					const bass = 1 - i / bars;
					const a = 0.5 + 0.5 * Math.sin(t * 6.0 + phase[i]);
					const b = 0.5 + 0.5 * Math.sin(t * 11.0 + phase[i] * 1.7);
					target = (0.35 * a + 0.25 * b) * (0.45 + 0.55 * bass);
					target = Math.min(1, target + 0.06);
				}
				// Ease toward the target (frame-rate independent).
				levels[i] += (target - levels[i]) * (1 - Math.exp(-dt * 9));
				const bh = Math.max(2, levels[i] * h);
				const x = i * (bw + gap);
				const y = h - bh;
				const r = Math.min(bw / 2, 3);
				ctx.beginPath();
				ctx.roundRect(x, y, bw, bh, r);
				ctx.fill();
			}
		};
		raf = requestAnimationFrame(draw);

		return () => {
			cancelAnimationFrame(raf);
			unlisten.then((u) => u());
		};
	});
</script>

{#if enabled}
	<canvas bind:this={canvas} class="h-full w-full {cls}"></canvas>
{/if}
