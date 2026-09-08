<script lang="ts">
	// Poolside History — day-grouped play history with shuffle-all and clear.
	import { onMount } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { ShuffleIcon, Delete02Icon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { HistoryEntry } from '$lib/api';
	import { playFrom, toast } from '$lib/player.svelte';

	let entries = $state<HistoryEntry[]>([]);
	let loading = $state(true);
	let listenedSecs = $state(0);

	onMount(async () => {
		try { entries = await api.getHistory(200); } catch { /* ignore */ }
		try { listenedSecs = await api.listenSecondsTotal(); } catch { /* ignore */ }
		loading = false;
	});

	// "12 TRACKS · 3H 20M LISTENED" — hours only when nonzero.
	const listenedLabel = $derived.by(() => {
		const h = Math.floor(listenedSecs / 3600);
		const m = Math.floor((listenedSecs % 3600) / 60);
		return h > 0 ? `${h}H ${m}M LISTENED` : `${m}M LISTENED`;
	});

	const grouped = $derived.by(() => {
		const groups = new Map<string, HistoryEntry[]>();
		const now = new Date();
		const today = now.toDateString();
		const yesterday = new Date(now.getTime() - 86400000).toDateString();
		for (const e of entries) {
			const d = new Date(e.playedAt * 1000).toDateString();
			const label = d === today ? 'Today' : d === yesterday ? 'Yesterday' : new Date(e.playedAt * 1000).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
			if (!groups.has(label)) groups.set(label, []);
			groups.get(label)!.push(e);
		}
		return [...groups.entries()];
	});

	function shuffleAll() {
		const songs = entries.map((e) => e.song);
		if (!songs.length) return;
		playFrom({ kind: 'playlist', id: 'ps-history', title: 'History' }, songs, 0);
		toast.info('Shuffling history…');
	}
	async function clearAll() {
		try {
			await api.clearHistory();
			entries = [];
			toast.success('History cleared');
		} catch { toast.error('Failed to clear history'); }
	}
	function playEntry(e: HistoryEntry, i: number) {
		const songs = entries.map((x) => x.song);
		playFrom({ kind: 'playlist', id: 'ps-history', title: 'History' }, songs, i);
	}

	function fmtTime(ts: number) {
		return new Date(ts * 1000).toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
	}
</script>

<div class="ps-history">
	<div class="ps-history-head ps-anim-fade-up">
		<h2 class="ps-page-title">HISTORY</h2>
		<div class="ps-history-actions">
			{#if listenedSecs > 0}<span class="ps-history-listened">{listenedLabel}</span>{/if}
			<button class="ps-ghost" onclick={shuffleAll} disabled={!entries.length}>
				<HugeiconsIcon icon={ShuffleIcon} /> Shuffle All
			</button>
			<button class="ps-ghost" onclick={clearAll} disabled={!entries.length} style="color:var(--red)">
				<HugeiconsIcon icon={Delete02Icon} /> Clear
			</button>
		</div>
	</div>

	{#if loading}
		<div class="ps-loading-dots"><span></span><span></span><span></span></div>
	{:else if grouped.length === 0}
		<div class="ps-empty ps-anim-fade-up">No play history yet.</div>
	{:else}
		{#each grouped as [day, items], gi (day)}
			<div class="ps-day-group ps-anim-fade-up" style="animation-delay:{gi * 0.05}s">
				<h4 class="ps-day-label">{day}</h4>
				<div class="ps-songlist">
					{#each items as e, i (e.song.video_id + e.playedAt)}
						<div class="ps-songrow ps-anim-slide-in" style="animation-delay:{i * 0.02}s" onclick={() => playEntry(e, entries.indexOf(e))} role="button" tabindex="0" onkeydown={(ev) => ev.key === 'Enter' && playEntry(e, entries.indexOf(e))}>
							{#if e.song.thumbnail}
								<img decoding="async" loading="lazy" src={e.song.thumbnail} alt="" style="width:32px;height:32px;border-radius:8px;object-fit:cover;flex:none" />
							{/if}
							<span class="st">{e.song.title.toUpperCase()}</span>
							<span class="sa">{e.song.artists}</span>
							<span class="sd">{fmtTime(e.playedAt)}</span>
						</div>
					{/each}
				</div>
			</div>
		{/each}
	{/if}
</div>
