<!--
   MiniPlayerPill — floating island mini player (mooziac-style Dynamic Island) that sits at
   the bottom-center of the screen, on top of every other UI. Near-black glass shell with a
   live waveform seekbar: peaks are decoded once per track by Rust (waveform.rs, cached in
   SQLite) and glow white behind the playhead. Click or drag-scrub the wave to seek; the
   thin fill shows until peaks land.
-->
<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon, PauseIcon, PreviousIcon, NextIcon } from '@hugeicons/core-free-icons';
	import { playback, dragVolume, commitVolume } from '$lib/player.svelte';
	import * as api from '$lib/api';

	let { onOpenNow }: { onOpenNow?: () => void } = $props();

	const cur = $derived(playback.now);
	const paused = $derived(playback.paused);
	const pos = $derived(playback.position);
	const dur = $derived(playback.duration || 0);
	const pct = $derived(dur > 0 ? Math.min(100, (pos / dur) * 100) : 0);

	let pill = $state<HTMLDivElement>();

	// Waveform peaks for the island seekbar (mooziac-style): decoded once per track by Rust,
	// then cached in SQLite. Module-level map so remounts don't refetch; the plain thin fill
	// shows until peaks land (decode needs the audio bytes first).
	const WAVE_BARS = 96;
	const peakCache = new Map<string, number[]>();
	let peaks = $state<number[] | null>(null);
	$effect(() => {
		const id = cur?.videoId;
		if (!id) {
			peaks = null;
			return;
		}
		const hit = peakCache.get(id);
		if (hit) {
			peaks = hit;
			return;
		}
		peaks = null;
		let live = true;
		api
			.waveformPeaks(id, WAVE_BARS)
			.then((bars) => {
				if (!live || !bars.length) return;
				peakCache.set(id, bars);
				peaks = bars;
			})
			.catch(() => {});
		return () => {
			live = false;
		};
	});

	function fmt(s: number): string {
		if (!s || Number.isNaN(s)) return '0:00';
		const t = Math.max(0, Math.floor(s));
		return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
	}
	function seekRatio(e: MouseEvent): number | null {
		if (!dur) return null;
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		if (!r.width) return null;
		return Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
	}
	function seek(e: MouseEvent) {
		const ratio = seekRatio(e);
		if (ratio !== null) api.seek(ratio * dur).catch(() => {});
	}
	// Drag-scrub across the waveform: press seeks, moving with the button held keeps seeking.
	function scrub(e: PointerEvent) {
		if (e.buttons !== 1) return;
		const ratio = seekRatio(e);
		if (ratio !== null) api.seek(ratio * dur).catch(() => {});
	}
	function openNow() {
		onOpenNow?.();
	}
</script>

{#if cur}
	<div
		bind:this={pill}
		class="ps-mini-pill"
		role="group"
		aria-label="Mini player"
	>
		<button class="ps-mini-pill-art" onclick={openNow} title="Open now playing" aria-label="Open now playing">
			{#if cur.thumbnail}
				<img decoding="async" src={cur.thumbnail} alt="" />
			{:else}
				<div class="ps-mini-pill-art-fallback"></div>
			{/if}
		</button>
		<div class="ps-mini-pill-meta" onclick={openNow} role="button" tabindex="0" onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && openNow()}>
			<span class="ps-mini-pill-title">{cur.title}</span>
			<span class="ps-mini-pill-artist">{cur.artists}</span>
		</div>
		<button class="ps-mini-pill-btn" onclick={() => api.prevTrack().catch(() => {})} aria-label="Previous">
			<HugeiconsIcon icon={PreviousIcon} />
		</button>
		<button class="ps-mini-pill-btn ps-mini-pill-btn--play" onclick={() => api.togglePause().catch(() => {})} aria-label={paused ? 'Play' : 'Pause'}>
			<HugeiconsIcon icon={paused ? PlayIcon : PauseIcon} />
		</button>
		<button class="ps-mini-pill-btn" onclick={() => api.nextTrack().catch(() => {})} aria-label="Next">
			<HugeiconsIcon icon={NextIcon} />
		</button>
		<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
		<!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
		<div
			class="ps-mini-pill-progress"
			onclick={seek}
			onpointerdown={scrub}
			onpointermove={scrub}
			onkeydown={(e) => {
				if (e.key === 'ArrowLeft') { e.preventDefault(); api.seek(Math.max(0, pos - 5)).catch(() => {}); }
				if (e.key === 'ArrowRight') { e.preventDefault(); api.seek(Math.min(dur, pos + 5)).catch(() => {}); }
				if (e.key === 'Home') { e.preventDefault(); api.seek(0).catch(() => {}); }
				if (e.key === 'End') { e.preventDefault(); api.seek(dur).catch(() => {}); }
			}}
			role="slider"
			aria-label="Seek"
			aria-valuemin="0"
			aria-valuemax={Math.round(dur)}
			aria-valuenow={Math.round(pos)}
			tabindex="0"
		>
			{#if peaks}
				<!-- Waveform seekbar: played bars glow, the rest sit dim. Bar count is fixed so
				     layout never shifts when peaks land; heights come straight from the decoder. -->
				<div class="ps-mini-pill-wave" aria-hidden="true">
					{#each peaks as p, i (i)}
						{@const played = (i / peaks.length) * 100 <= pct}
						<span
							class="ps-mini-pill-bar {played ? 'on' : ''}"
							style="height: {Math.max(12, Math.round((p / 255) * 100))}%"
						></span>
					{/each}
				</div>
			{:else}
				<div class="ps-mini-pill-progress-track">
					<div class="ps-mini-pill-progress-fill" style="width: {pct}%"></div>
				</div>
			{/if}
			<div class="ps-mini-pill-progress-times">
				<span>{fmt(pos)}</span>
				<span>{fmt(dur)}</span>
			</div>
		</div>
		<div class="ps-mini-pill-volume" aria-label="Volume">
			<input
				type="range"
				min="0"
				max="100"
				value={playback.volume}
				oninput={(e) => dragVolume(Number(e.currentTarget.value))}
				onchange={(e) => commitVolume(Number(e.currentTarget.value))}
				aria-label="Volume"
			/>
		</div>
	</div>
{/if}

<style>
	.ps-mini-pill {
		position: fixed;
		left: 50%;
		bottom: 18px;
		transform: translateX(-50%);
		z-index: 50;
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 8px 14px;
		border-radius: 999px;
		/* Dynamic-island look: near-black glass, white-on-dark chrome. The pool tint comes
		   from the glow accents, not the shell, so it reads as hardware on any backdrop. */
		background: linear-gradient(180deg, rgba(12, 12, 14, 0.84) 0%, rgba(12, 12, 14, 0.68) 100%);
		backdrop-filter: blur(24px) saturate(1.8);
		-webkit-backdrop-filter: blur(24px) saturate(1.8);
		border: 1px solid rgba(255, 255, 255, 0.12);
		box-shadow: 0 18px 44px rgba(0, 0, 0, 0.5), 0 4px 12px rgba(0, 0, 0, 0.3), inset 0 1px 0 rgba(255, 255, 255, 0.08);
		color: #fff;
		min-width: 520px;
		max-width: calc(100vw - 36px);
	}
	.ps-mini-pill-art {
		all: unset;
		cursor: pointer;
		width: 44px;
		height: 44px;
		border-radius: 50%;
		overflow: hidden;
		border: 1.5px solid rgba(255, 255, 255, 0.5);
		flex: none;
		background: conic-gradient(from 210deg, #e8e8e8, #9fb6bc, #fff, #7fa6ae, #e8e8e8);
	}
	.ps-mini-pill-art img { width: 100%; height: 100%; object-fit: cover; display: block; }
	.ps-mini-pill-art-fallback { width: 100%; height: 100%; background: inherit; }
	.ps-mini-pill-meta {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		max-width: 220px;
		padding: 0 4px;
		cursor: pointer;
	}
	.ps-mini-pill-title {
		font-size: 12px;
		font-weight: 700;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-mini-pill-artist {
		font-size: 10px;
		letter-spacing: 0.08em;
		text-transform: uppercase;
		opacity: 0.7;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-mini-pill-btn {
		all: unset;
		cursor: pointer;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		color: #fff;
		opacity: 0.85;
		transition: opacity 0.15s, transform 0.15s;
	}
	.ps-mini-pill-btn:hover { opacity: 1; transform: scale(1.08); }
	.ps-mini-pill-btn svg { width: 14px; height: 14px; }
	.ps-mini-pill-btn--play {
		background: linear-gradient(180deg, #8fdef6, var(--accent) 55%, #2e9ecb);
		color: #111;
		opacity: 1;
		box-shadow: 0 4px 12px rgba(14, 110, 140, 0.5);
	}
	.ps-mini-pill-btn--play svg { width: 16px; height: 16px; }
	.ps-mini-pill-progress {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 100px;
		flex: 1;
		cursor: pointer;
	}
	.ps-mini-pill-progress-track {
		height: 4px;
		border-radius: 4px;
		background: rgba(255, 255, 255, 0.18);
		overflow: hidden;
	}
	.ps-mini-pill-progress-fill {
		height: 100%;
		background: linear-gradient(90deg, rgba(255, 255, 255, 0.95), rgba(140, 225, 240, 0.95));
		transition: width 0.3s linear;
	}
	/* Waveform seekbar: fixed bar count, decoder-driven heights. Played bars glow white,
	   the rest sit dim; the whole strip is the slider hit area (click + drag-scrub). */
	.ps-mini-pill-wave {
		display: flex;
		align-items: center;
		gap: 1px;
		height: 28px;
		cursor: pointer;
	}
	.ps-mini-pill-bar {
		flex: 1 1 0;
		min-width: 1px;
		border-radius: 1px;
		background: rgba(255, 255, 255, 0.22);
		transition: background 0.2s;
	}
	.ps-mini-pill-bar.on {
		background: linear-gradient(180deg, #fff, rgba(140, 225, 240, 0.9));
		box-shadow: 0 0 6px rgba(140, 225, 240, 0.45);
	}
	.ps-mini-pill-progress-times {
		display: flex;
		justify-content: space-between;
		font-size: 9px;
		letter-spacing: 0.08em;
		opacity: 0.65;
		font-variant-numeric: tabular-nums;
	}
	.ps-mini-pill-volume {
		width: 60px;
		flex: none;
	}
	.ps-mini-pill-volume input {
		width: 100%;
		accent-color: var(--accent);
	}
	@media (max-width: 720px) {
		.ps-mini-pill {
			min-width: 0;
			padding: 6px 8px;
			gap: 6px;
		}
		.ps-mini-pill-meta { display: none; }
		.ps-mini-pill-volume { display: none; }
		.ps-mini-pill-progress { min-width: 80px; }
	}
</style>
