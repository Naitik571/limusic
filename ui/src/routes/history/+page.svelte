<script lang="ts">
	// The local play diary — everything the app recorded, newest first, grouped by day. This is
	// the chronological view of the same table On Repeat ranks; it never leaves the machine
	// (works signed out), and Clear wipes it for good.
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Delete02Icon,
		HistoryIcon,
		PlayIcon,
		ShuffleIcon,
		MusicNote01Icon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import TrackRowSkeleton from '$lib/components/TrackRowSkeleton.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import * as api from '$lib/api';
	import type { HistoryEntry } from '$lib/api';
	import { playFrom, playSong, toast } from '$lib/player.svelte';

	let entries = $state<HistoryEntry[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let confirmingClear = $state(false);

	// Day buckets, newest first. Keys render as "Today" / "Yesterday" / a real date; the rows
	// inside keep the diary order (a replay of the same song later in the day sits below).
	const days = $derived.by(() => {
		const out: { label: string; entries: HistoryEntry[] }[] = [];
		const key = (e: HistoryEntry) => new Date(e.playedAt * 1000).toDateString();
		let current: string | null = null;
		for (const e of entries) {
			const k = key(e);
			if (k !== current) {
				current = k;
				out.push({ label: dayLabel(e.playedAt), entries: [e] });
			} else {
				out.at(-1)!.entries.push(e);
			}
		}
		return out;
	});

	function dayLabel(unixSecs: number): string {
		const d = new Date(unixSecs * 1000);
		const today = new Date();
		const yesterday = new Date(today);
		yesterday.setDate(today.getDate() - 1);
		if (d.toDateString() === today.toDateString()) return 'Today';
		if (d.toDateString() === yesterday.toDateString()) return 'Yesterday';
		return d.toLocaleDateString(undefined, { month: 'long', day: 'numeric' });
	}

	// The diary as one queue: clicking play on the header shuffles everything loaded.
	function shuffleAll() {
		if (!entries.length) return;
		playFrom(
			{ kind: 'playlist', id: 'history', title: 'History' },
			entries.map((e) => e.song),
			null,
			undefined,
			true
		);
	}

	async function clear() {
		try {
			await api.clearHistory();
			entries = [];
			confirmingClear = false;
			toast.success('History cleared');
		} catch (e) {
			toast.error(String(e));
		}
	}

	onMount(async () => {
		try {
			entries = await api.getHistory(500);
		} catch (e) {
			error = String(e);
		} finally {
			loading = false;
		}
	});
</script>

<div class="flex flex-col">
	<div class="flex items-end justify-between gap-4 border-b p-6">
		<div class="flex items-center gap-3">
			<span class="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10 text-primary">
				<HugeiconsIcon icon={HistoryIcon} class="h-5 w-5" />
			</span>
			<div>
				<h1 class="font-heading text-2xl font-bold tracking-tight">History</h1>
				<p class="text-sm text-muted-foreground">
					{entries.length
						? `${entries.length} play${entries.length === 1 ? '' : 's'} on this machine — newest first.`
						: 'Everything you play, on this machine only.'}
				</p>
			</div>
		</div>
		{#if entries.length}
			<div class="flex items-center gap-2">
				<Button variant="outline" size="sm" class="gap-2" onclick={shuffleAll}>
					<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" /> Shuffle all
				</Button>
				{#if confirmingClear}
					<Button variant="destructive" size="sm" onclick={clear}>Really clear</Button>
					<Button variant="ghost" size="sm" onclick={() => (confirmingClear = false)}>Cancel</Button>
				{:else}
					<Button
						variant="ghost"
						size="sm"
						class="gap-2 text-muted-foreground"
						onclick={() => (confirmingClear = true)}
					>
						<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" /> Clear
					</Button>
				{/if}
			</div>
		{/if}
	</div>

	<div class="p-4">
		{#if loading}
			{#each Array(8) as _, i (i)}
				<TrackRowSkeleton />
			{/each}
		{:else if error}
			<ErrorState message={error} onRetry={() => location.reload()} />
		{:else if !entries.length}
			<div class="flex flex-col items-center gap-3 py-20 text-center">
				<span
					class="flex h-14 w-14 items-center justify-center rounded-full bg-muted text-muted-foreground/50"
				>
					<HugeiconsIcon icon={MusicNote01Icon} class="h-7 w-7" />
				</span>
				<p class="max-w-sm text-sm text-muted-foreground">
					Nothing here yet — plays land once they've passed the halfway point (or four minutes),
					same rule On Repeat uses.
				</p>
			</div>
		{:else}
			{#each days as day (day.label)}
				<section class="mb-2">
					<h2
						class="sticky top-0 z-10 -mx-4 bg-background/95 px-5 py-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground backdrop-blur"
					>
						{day.label}
					</h2>
					{#each day.entries as e, i (e.playedAt * 1000 + i)}
						<div in:fade={{ duration: 120 }}>
							<TrackRow
								song={e.song}
								index={i}
								active={false}
								onplay={() => playSong(e.song)}
							/>
						</div>
					{/each}
				</section>
			{/each}
			<p class="px-2 pt-2 text-xs text-muted-foreground">
				Only about the last month is kept — the diary prunes itself as it grows.
			</p>
		{/if}
	</div>
</div>
