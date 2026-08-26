<script lang="ts">
	// Poolside mini-player: spinning disc, tiny mono meta, hairline seek bar, aqua play button.
	// The glass pill gets the liquid-glass lens (SDF displacement refraction) on mount.
	import { onMount } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon, PauseIcon, PreviousIcon, NextIcon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { playback } from '$lib/player.svelte';
	import { liquidLens } from './liquidLens';
	import Vinyl from './Vinyl.svelte';

	let { onOpenNow }: { onOpenNow: () => void } = $props();

	let pill: HTMLDivElement | undefined = $state();

	const cur = $derived(playback.now);
	const paused = $derived(playback.paused);
	const dur = $derived(playback.duration || 0);
	const pos = $derived(Math.min(playback.position, dur || playback.position));
	const pct = $derived(dur > 0 ? (pos / dur) * 100 : 0);

	onMount(() => {
		if (!pill) return;
		// wait a frame so the pill has its final size
		requestAnimationFrame(() => {
			if (!pill) return;
			const lens = liquidLens(pill, { id: 'ps-mini-lens', strength: 42, radius: 22 });
			pill.style.filter = lens;
		});
	});

	function seek(e: MouseEvent) {
		if (!dur) return;
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		const ratio = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
		api.seek(ratio * dur).catch(() => {});
	}
</script>

<div
	class="ps-mini ps-glass"
	bind:this={pill}
	role="button"
	tabindex="0"
	onclick={(e) => {
		if ((e.target as HTMLElement).closest('button')) return;
		onOpenNow();
	}}
	onkeydown={(e) => e.key === 'Enter' && onOpenNow()}
	title="Open now playing"
>
	<div class="w-[46px] flex-none">
		<Vinyl src={cur?.thumbnail ?? ''} playing={!paused} style="width:100%" />
	</div>
	<div class="meta">
		<div class="mt">{cur ? cur.title.toUpperCase() : 'NOTHING PLAYING'}</div>
		<div class="ma">{cur ? cur.artists : ''}</div>
		<div
			class="bar"
			role="slider"
			tabindex="0"
			aria-label="Seek"
			aria-valuenow={Math.round(pos)}
			aria-valuemin={0}
			aria-valuemax={Math.round(dur)}
			onclick={seek}
			onkeydown={(e) => e.key === 'Enter' && seek(e as unknown as MouseEvent)}
		>
			<div class="track"><div class="fill" style="width:{pct}%"></div><div class="dot" style="left:{pct}%"></div></div>
		</div>
	</div>
	<button
		class="mbtn"
		onclick={(e) => {
			e.stopPropagation();
			api.prevTrack().catch(() => {});
		}}
		aria-label="Previous"
	>
		<HugeiconsIcon icon={PreviousIcon} />
	</button>
	<button
		class="aqua-play ps-aqua"
		onclick={(e) => {
			e.stopPropagation();
			api.togglePause().catch(() => {});
		}}
		aria-label="Play or pause"
	>
		{#if paused}<HugeiconsIcon icon={PlayIcon} />{:else}<HugeiconsIcon icon={PauseIcon} />{/if}
	</button>
	<button
		class="mbtn"
		onclick={(e) => {
			e.stopPropagation();
			api.nextTrack().catch(() => {});
		}}
		aria-label="Next"
	>
		<HugeiconsIcon icon={NextIcon} />
	</button>
</div>
