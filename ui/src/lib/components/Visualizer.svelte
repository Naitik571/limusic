<script lang="ts">
	// Real-time spectrum (WASAPI app-loopback + FFT in Rust, `visualizer-frame` events).
	// Canvas bars or circular ring, colored from the accent prop. The component decays its
	// own displayed values every frame, so pause/disable flatlines smoothly with no help
	// from the backend (which simply stops sending).
	import { onDestroy, onMount } from 'svelte';
	import * as api from '$lib/api';

	let {
		accent = '#8ce1f0',
		style = 'bars',
		barCount = 24
	}: { accent?: string; style?: 'bars' | 'circular'; barCount?: number } = $props();

	let canvas = $state<HTMLCanvasElement | null>(null);
	let target = $state<number[]>([]);
	let shown = $state<number[]>([]);
	// (Re)seed when the band count changes; plain init would capture the prop once.
	$effect(() => {
		target = new Array<number>(barCount).fill(0);
		shown = new Array<number>(barCount).fill(0);
	});
	let unsub: (() => void) | null = null;
	let raf = 0;

	function resize(c: HTMLCanvasElement): void {
		const r = c.getBoundingClientRect();
		const dpr = Math.min(2, window.devicePixelRatio || 1);
		c.width = Math.max(1, Math.round(r.width * dpr));
		c.height = Math.max(1, Math.round(r.height * dpr));
	}

	function draw() {
		const c = canvas;
		if (!c) return;
		const ctx = c.getContext('2d');
		if (!ctx) return;
		const W = c.width;
		const H = c.height;
		ctx.clearRect(0, 0, W, H);
		// Ease displayed toward target, then decay — flatlines when events stop.
		let alive = false;
		for (let i = 0; i < barCount; i++) {
			const t = target[i] ?? 0;
			shown[i] = Math.max(t, shown[i] * 0.9);
			if (shown[i] > 0.004) alive = true;
		}
		if (!alive) {
			raf = requestAnimationFrame(draw);
			return;
		}
		if (style === 'circular') {
			const cx = W / 2;
			const cy = H / 2;
			const base = Math.min(W, H) * 0.28;
			const maxR = Math.min(W, H) * 0.22;
			for (let i = 0; i < barCount; i++) {
				const a0 = (i / barCount) * Math.PI * 2 - Math.PI / 2;
				const a1 = ((i + 0.72) / barCount) * Math.PI * 2 - Math.PI / 2;
				const r = base + shown[i] * maxR;
				ctx.beginPath();
				ctx.arc(cx, cy, r, a0, a1);
				ctx.strokeStyle = accent;
				ctx.globalAlpha = 0.35 + shown[i] * 0.65;
				ctx.lineWidth = Math.max(2, (Math.PI * 2 * base) / barCount - 3);
				ctx.stroke();
			}
			ctx.globalAlpha = 1;
		} else {
			const gap = W / barCount;
			const bw = Math.max(2, gap * 0.62);
			for (let i = 0; i < barCount; i++) {
				const h = Math.max(2, shown[i] * H);
				const x = i * gap + (gap - bw) / 2;
				const g = ctx.createLinearGradient(0, H, 0, H - h);
				g.addColorStop(0, accent);
				g.addColorStop(1, '#ffffff');
				ctx.globalAlpha = 0.35 + shown[i] * 0.65;
				ctx.fillStyle = g;
				const rad = Math.min(bw / 2, 4);
				ctx.beginPath();
				ctx.roundRect(x, H - h, bw, h, [rad, rad, 0, 0]);
				ctx.fill();
			}
			ctx.globalAlpha = 1;
		}
		raf = requestAnimationFrame(draw);
	}

	onMount(() => {
		if (!canvas) return;
		let mounted = true;
		resize(canvas);
		const ro = new ResizeObserver(() => canvas && resize(canvas));
		ro.observe(canvas);
		// The subscribe promise can resolve after unmount: only keep the unsubscriber while
		// still mounted, otherwise unsubscribe immediately so the event can't write into a
		// dead component.
		api.onVisualizerFrame((bands) => {
			for (let i = 0; i < barCount; i++) target[i] = bands[i] ?? 0;
		}).then((u) => {
			if (mounted) unsub = u;
			else u();
		}).catch(() => {});
		raf = requestAnimationFrame(draw);
		return () => {
			mounted = false;
			cancelAnimationFrame(raf);
			raf = 0;
			ro.disconnect();
			unsub?.();
			unsub = null;
		};
	});
	onDestroy(() => {
		cancelAnimationFrame(raf);
		raf = 0;
		unsub?.();
		unsub = null;
	});
</script>

<canvas
	bind:this={canvas}
	class="block h-20 w-full"
	aria-hidden="true"
	style="filter: drop-shadow(0 2px 10px color-mix(in srgb, {accent} 35%, transparent));"
></canvas>
