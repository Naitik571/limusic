<!--
  RadioNowPlaying — the "ON AIR" screen for a live radio station.

  Visual:
    - Header: ON AIR label with pulsing red dot, station name + live status
    - 8-bar spectrum visualizer with frequency-band labels (60, 125, 250, 500,
      1K, 2K, 4K, 8K). Each bar animates up and down on a deterministic but
      audio-feeling pattern (we don't capture real audio, but the pattern is
      tuned so the bars look like a real spectrogram).
    - Track title with a red LIVE badge
    - Horizontal row of other live station cards (quick-switch)
    - LIVE listener count, station description
-->
<script lang="ts">
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon, PauseIcon, StarIcon } from '@hugeicons/core-free-icons';
	import { onMount } from 'svelte';
	import { reducedMotion, rafLoop } from './motion';

	type Station = {
		id: string;
		name: string;
		genre: string;
		location: string;
		isLive: boolean;
		listeners: number;
		isFavorite: boolean;
	};

	let {
		station,
		stations = [],
		onSwitchStation
	}: {
		station: Station;
		stations?: Station[];
		onSwitchStation?: (s: Station) => void;
	} = $props();

	const FREQ_BANDS = ['60', '125', '250', '500', '1K', '2K', '4K', '8K'] as const;
	// Each band has a per-frame target height (0-100). The spectrum updater
	// advances these on a rAF loop, tuned to look like a real audio mix.
	let bandHeights = $state<number[]>([20, 30, 45, 60, 75, 50, 35, 22]);

	onMount(() => {
		const stop = rafLoop((t, dt) => {
			if (reducedMotion()) return;
			// Each band has a base amplitude + a phase offset. The motion is a
			// combination of slow + fast terms so adjacent bars don't pulse in sync.
			const next = FREQ_BANDS.map((_band, i) => {
				const baseAmp = 30 + i * 4; // higher bands quieter
				const slow = Math.sin(t * 0.6 + i * 0.7) * 18;
				const fast = Math.sin(t * 4.2 + i * 1.1) * 10;
				const kick = Math.max(0, Math.sin(t * 0.4 + i * 0.3)) * 30;
				const v = baseAmp + slow + fast + kick;
				return Math.max(8, Math.min(100, v));
			});
			bandHeights = next;
		});
		return stop;
	});
</script>

<div class="ps-rnp">
	<header class="ps-rnp-head">
		<div class="ps-rnp-onair">
			<span class="ps-rnp-onair-dot" aria-hidden="true"></span>
			<span class="ps-rnp-onair-text">ON AIR</span>
		</div>
		<h1 class="ps-rnp-station">{station.name}</h1>
		<div class="ps-rnp-sub">
			<span class="ps-rnp-genre">{station.genre}</span>
			<span class="ps-rnp-sep">·</span>
			<span class="ps-rnp-loc">{station.location}</span>
			<span class="ps-rnp-sep">·</span>
			<span class="ps-rnp-listeners">{station.listeners.toLocaleString()} listening</span>
		</div>
	</header>

	<div class="ps-rnp-spectrum" aria-hidden="true">
		<div class="ps-rnp-bars">
			{#each FREQ_BANDS as band, i}
				<div class="ps-rnp-bar-col">
					<div class="ps-rnp-bar" style="height: {bandHeights[i]}%"></div>
					<span class="ps-rnp-bar-label">{band}</span>
				</div>
			{/each}
		</div>
	</div>

	<div class="ps-rnp-track">
		<div class="ps-rnp-track-art">
			<div class="ps-rnp-track-art-bg" style="background: linear-gradient(135deg, hsl({(station.id.length * 47) % 360} 60% 45%), hsl({(station.id.length * 89) % 360} 70% 35%));"></div>
		</div>
		<div class="ps-rnp-track-info">
			<div class="ps-rnp-track-title-row">
				<span class="ps-rnp-track-title">{station.name} · Live Mix</span>
				<span class="ps-rnp-track-live">
					<span class="ps-rnp-track-live-dot"></span>LIVE
				</span>
			</div>
			<span class="ps-rnp-track-artist">Various Artists</span>
		</div>
		<button class="ps-rnp-play" aria-label="Play station">
			<HugeiconsIcon icon={PlayIcon} />
		</button>
		<button class="ps-rnp-fav {station.isFavorite ? 'is-fav' : ''}" aria-label="Favorite">
			<HugeiconsIcon icon={StarIcon} />
		</button>
	</div>

	{#if stations.length > 0}
		<div class="ps-rnp-switch">
			<h3 class="ps-rnp-section">QUICK SWITCH</h3>
			<div class="ps-rnp-switch-row">
				{#each stations.slice(0, 8) as s (s.id)}
					{#if s.id !== station.id}
						<button class="ps-rnp-switch-card" onclick={() => onSwitchStation?.(s)}>
							<div
								class="ps-rnp-switch-art"
								style="background: linear-gradient(135deg, hsl({(s.id.length * 47) % 360} 60% 45%), hsl({(s.id.length * 89) % 360} 70% 35%));"
							></div>
							<div class="ps-rnp-switch-info">
								<span class="ps-rnp-switch-name">{s.name}</span>
								<span class="ps-rnp-switch-meta">{s.genre}</span>
								{#if s.isLive}
									<span class="ps-rnp-switch-badge-live">LIVE</span>
								{:else}
									<span class="ps-rnp-switch-badge-online">ONLINE</span>
								{/if}
							</div>
						</button>
					{/if}
				{/each}
			</div>
		</div>
	{/if}
</div>

<style>
	.ps-rnp {
		padding: 80px 32px 110px;
		height: 100%;
		overflow-y: auto;
		scrollbar-width: thin;
	}
	.ps-rnp-head {
		text-align: center;
		margin-bottom: 24px;
	}
	.ps-rnp-onair {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		padding: 4px 12px;
		background: rgba(255, 60, 60, 0.18);
		border: 1px solid rgba(255, 60, 60, 0.4);
		border-radius: 999px;
	}
	.ps-rnp-onair-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: #ff3030;
		box-shadow: 0 0 8px rgba(255, 60, 60, 0.7);
		animation: ps-rnp-live-pulse 1.2s ease-in-out infinite;
	}
	@keyframes ps-rnp-live-pulse {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.4; transform: scale(0.65); }
	}
	.ps-rnp-onair-text {
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.32em;
		color: #ff5050;
	}
	.ps-rnp-station {
		font-family: var(--display);
		font-size: 36px;
		letter-spacing: 0.06em;
		margin: 12px 0 6px;
		text-shadow: 0 2px 12px rgba(8, 60, 70, 0.6);
	}
	.ps-rnp-sub {
		display: flex;
		justify-content: center;
		gap: 8px;
		font-size: 10px;
		letter-spacing: 0.18em;
		text-transform: uppercase;
		opacity: 0.7;
	}
	.ps-rnp-sep { opacity: 0.4; }
	.ps-rnp-spectrum {
		max-width: 800px;
		margin: 24px auto 28px;
		padding: 18px 24px;
		background: rgba(0, 0, 0, 0.35);
		border: 1px solid rgba(255, 255, 255, 0.12);
		border-radius: 18px;
		backdrop-filter: blur(8px);
	}
	.ps-rnp-bars {
		display: flex;
		align-items: flex-end;
		justify-content: space-between;
		gap: 8px;
		height: 160px;
	}
	.ps-rnp-bar-col {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 6px;
		height: 100%;
	}
	.ps-rnp-bar {
		width: 100%;
		min-height: 6px;
		border-radius: 4px 4px 0 0;
		background: linear-gradient(180deg, #ff5050 0%, #c01818 50%, #6a0c0c 100%);
		box-shadow: 0 0 12px rgba(255, 80, 80, 0.5);
		transition: height 0.08s linear;
	}
	.ps-rnp-bar-label {
		font-size: 9px;
		letter-spacing: 0.1em;
		opacity: 0.6;
		font-variant-numeric: tabular-nums;
	}
	.ps-rnp-track {
		max-width: 700px;
		margin: 0 auto 32px;
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 14px 18px;
		background: rgba(255, 255, 255, 0.1);
		border: 1px solid rgba(255, 255, 255, 0.2);
		border-radius: 16px;
		backdrop-filter: blur(14px);
	}
	.ps-rnp-track-art {
		width: 64px;
		height: 64px;
		border-radius: 50%;
		overflow: hidden;
		flex: none;
		border: 1.5px solid rgba(255, 255, 255, 0.4);
	}
	.ps-rnp-track-art-bg {
		width: 100%;
		height: 100%;
	}
	.ps-rnp-track-info {
		display: flex;
		flex-direction: column;
		gap: 4px;
		min-width: 0;
		flex: 1;
	}
	.ps-rnp-track-title-row {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.ps-rnp-track-title {
		font-size: 13px;
		font-weight: 700;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-rnp-track-live {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px 6px;
		background: #e02020;
		color: #fff;
		font-size: 8px;
		font-weight: 700;
		letter-spacing: 0.18em;
		border-radius: 4px;
	}
	.ps-rnp-track-live-dot {
		width: 5px;
		height: 5px;
		border-radius: 50%;
		background: #fff;
		animation: ps-rnp-live-pulse 1.2s ease-in-out infinite;
	}
	.ps-rnp-track-artist {
		font-size: 10px;
		letter-spacing: 0.14em;
		text-transform: uppercase;
		opacity: 0.7;
	}
	.ps-rnp-play {
		all: unset;
		cursor: pointer;
		width: 44px;
		height: 44px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		background: linear-gradient(180deg, #8fdef6, var(--accent) 55%, #2e9ecb);
		color: #111;
		box-shadow: 0 5px 14px rgba(14, 110, 140, 0.5);
		transition: transform 0.15s;
	}
	.ps-rnp-play:hover { transform: scale(1.06); }
	.ps-rnp-play svg { width: 18px; height: 18px; }
	.ps-rnp-fav {
		all: unset;
		cursor: pointer;
		width: 36px;
		height: 36px;
		border-radius: 50%;
		display: grid;
		place-items: center;
		color: rgba(255, 255, 255, 0.7);
		transition: color 0.2s;
	}
	.ps-rnp-fav.is-fav { color: #ffd54a; }
	.ps-rnp-fav svg { width: 16px; height: 16px; }
	.ps-rnp-section {
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.32em;
		text-transform: uppercase;
		opacity: 0.65;
		text-align: center;
		margin-bottom: 14px;
	}
	.ps-rnp-switch-row {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
		gap: 12px;
		max-width: 1100px;
		margin: 0 auto;
	}
	.ps-rnp-switch-card {
		all: unset;
		cursor: pointer;
		display: flex;
		gap: 10px;
		padding: 8px;
		border-radius: 12px;
		background: rgba(255, 255, 255, 0.08);
		border: 1px solid rgba(255, 255, 255, 0.18);
		transition: background 0.2s, transform 0.2s;
	}
	.ps-rnp-switch-card:hover {
		background: rgba(255, 255, 255, 0.15);
		transform: translateY(-2px);
	}
	.ps-rnp-switch-art {
		width: 44px;
		height: 44px;
		border-radius: 8px;
		flex: none;
	}
	.ps-rnp-switch-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
		position: relative;
	}
	.ps-rnp-switch-name {
		font-size: 10px;
		font-weight: 700;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.ps-rnp-switch-meta {
		font-size: 9px;
		letter-spacing: 0.1em;
		text-transform: uppercase;
		opacity: 0.6;
	}
	.ps-rnp-switch-badge-live,
	.ps-rnp-switch-badge-online {
		position: absolute;
		top: 0;
		right: 0;
		padding: 1px 5px;
		font-size: 7px;
		font-weight: 700;
		letter-spacing: 0.16em;
		border-radius: 3px;
	}
	.ps-rnp-switch-badge-live {
		background: #e02020;
		color: #fff;
	}
	.ps-rnp-switch-badge-online {
		background: #4ade80;
		color: #062c1a;
	}
</style>
