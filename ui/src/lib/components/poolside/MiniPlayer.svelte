<script lang="ts">
	// Poolside mini-player: spinning disc, tiny mono meta, hairline seek bar, aqua play button.
	// Clicking the pill opens the Now Playing view.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { PlayIcon, PauseIcon, PreviousIcon, NextIcon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { playback } from '$lib/player.svelte';
	import Vinyl from './Vinyl.svelte';

	let { onOpenNow }: { onOpenNow: () => void } = $props();

	const cur = $derived(playback.now);
	const paused = $derived(playback.paused);
	const dur = $derived(playback.duration || 0);
	const pos = $derived(Math.min(playback.position, dur || playback.position));
	const pct = $derived(dur > 0 ? (pos / dur) * 100 : 0);

	function seek(e: MouseEvent) {
		if (!dur) return;
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		const ratio = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
		api.seek(ratio * dur).catch(() => {});
	}
</script>

<div
	class="ps-mini ps-glass"
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
		<div class="mt">{cur ? cur.title.toUpperCase() : 'Nothing playing'}</div>
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
		class="flex items-center justify-center text-[#06303a] opacity-80 hover:opacity-100 w-7 h-7"
		onclick={(e) => {
			e.stopPropagation();
			api.prevTrack().catch(() => {});
		}}
		aria-label="Previous"
	>
		<HugeiconsIcon icon={PreviousIcon} class="w-3.5 h-3.5" />
	</button>
	<button
		class="ps-aqua"
		onclick={(e) => {
			e.stopPropagation();
			api.togglePause().catch(() => {});
		}}
		aria-label="Play or pause"
	>
		{#if paused}<HugeiconsIcon icon={PlayIcon} class="w-3.5 h-3.5" />{:else}<HugeiconsIcon icon={PauseIcon} class="w-3.5 h-3.5" />{/if}
	</button>
	<button
		class="flex items-center justify-center text-[#06303a] opacity-80 hover:opacity-100 w-7 h-7"
		onclick={(e) => {
			e.stopPropagation();
			api.nextTrack().catch(() => {});
		}}
		aria-label="Next"
	>
		<HugeiconsIcon icon={NextIcon} class="w-3.5 h-3.5" />
	</button>
</div>
