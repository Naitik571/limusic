<script lang="ts">
	// Downloads library: everything saved for offline playback, plus live in-flight rows.
	// Catalogue rows come from `list_downloads`; the monitor's live items overlay them while
	// a batch runs (progress + cancel). Deleting drops file + row; playing uses the disk file.
	import { onMount } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Download01Icon,
		PlayIcon,
		Cancel01Icon,
		Delete01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { DownloadedTrack } from '$lib/api';
	import {
		downloads,
		downloadedIds,
		loadDownloadedIds,
		markNotDownloaded,
		playSong,
		startDownloadMonitor,
		cancelDownload,
		cancelAllDownloads,
		toast
	} from '$lib/player.svelte';

	let catalogue = $state<DownloadedTrack[]>([]);
	let totalBytes = $state(0);
	let loading = $state(true);

	async function refresh() {
		try {
			const r = await api.listDownloads();
			catalogue = r.items;
			totalBytes = r.total_bytes;
		} catch {
			/* catalogue read failed — live rows still render */
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		startDownloadMonitor();
		void refresh();
	});

	// Live monitor rows first (newest activity on top), then catalogue rows not already live.
	const liveIds = $derived(new Set(downloads.items.map((i) => i.id)));
	const stored = $derived(catalogue.filter((t) => !liveIds.has(t.video_id)));

	function fmtSize(bytes: number): string {
		if (!bytes) return '—';
		const mb = bytes / (1024 * 1024);
		return mb >= 1024 ? `${(mb / 1024).toFixed(2)} GB` : `${mb.toFixed(1)} MB`;
	}
	function fmtDur(secs: number): string {
		if (!secs || Number.isNaN(secs)) return '';
		const t = Math.max(0, Math.floor(secs));
		return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, '0')}`;
	}
	function fmtDate(ts: number): string {
		return new Date(ts * 1000).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' });
	}
	function playStored(t: DownloadedTrack) {
		playSong({
			video_id: t.video_id,
			title: t.title,
			artists: t.artists,
			thumbnail: t.thumb ?? undefined
		});
	}
	async function removeStored(t: DownloadedTrack) {
		try {
			await api.deleteDownload(t.video_id);
		} catch (e) {
			toast.error(String(e));
			return;
		}
		markNotDownloaded(t.video_id);
		catalogue = catalogue.filter((x) => x.video_id !== t.video_id);
	}
</script>

<div class="mx-auto w-full max-w-3xl px-4 py-6">
	<div class="mb-4 flex items-center justify-between gap-3">
		<div>
			<h1 class="font-heading text-xl font-bold">Downloads</h1>
			<p class="text-xs text-muted-foreground">
				{catalogue.length} track{catalogue.length === 1 ? '' : 's'} · {fmtSize(totalBytes)} offline
			</p>
		</div>
		{#if downloads.active > 0}
			<button
				class="cursor-pointer rounded-lg border px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
				onclick={() => cancelAllDownloads()}
			>
				Cancel all ({downloads.active})
			</button>
		{/if}
	</div>

	{#if loading}
		<div class="space-y-3">
			{#each { length: 6 } as _, i (i)}
				<div class="h-12 animate-pulse rounded-lg bg-muted" style="width:{92 - ((i * 13) % 18)}%"></div>
			{/each}
		</div>
	{:else}
		{#if downloads.items.length === 0 && stored.length === 0}
			<div class="flex flex-col items-center gap-3 py-16 text-center">
				<HugeiconsIcon icon={Download01Icon} class="h-8 w-8 text-muted-foreground/50" />
				<p class="text-sm text-muted-foreground">Nothing downloaded yet.</p>
				<p class="max-w-xs text-xs text-muted-foreground/70">
					Use the ⋯ menu on any track, album or playlist — or pin a playlist to keep it offline.
				</p>
			</div>
		{/if}
		<div class="flex flex-col gap-1.5">
			{#each downloads.items as it (it.id)}
				<div class="flex items-center gap-3 rounded-xl border border-border bg-card px-3 py-2">
					{#if it.thumb}
						<img decoding="async" src={it.thumb} alt="" class="h-10 w-10 shrink-0 rounded-md object-cover" />
					{:else}
						<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground/50">
							<HugeiconsIcon icon={Download01Icon} class="h-4 w-4" />
						</div>
					{/if}
					<div class="min-w-0 flex-1">
						<p class="truncate text-sm font-medium">{it.title}</p>
						<p class="truncate text-xs text-muted-foreground">{it.artists ?? ''}</p>
						{#if it.state === 'downloading'}
							<div class="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-muted">
								<div class="h-full rounded-full bg-yellow-400 transition-[width] duration-200" style="width:{it.percent}%"></div>
							</div>
						{:else if it.state === 'error'}
							<p class="mt-1 truncate text-xs text-red-500">{it.message ?? 'Failed'}</p>
						{:else if it.state === 'cancelled'}
							<p class="mt-1 text-xs text-muted-foreground">Cancelled</p>
						{:else}
							<p class="mt-1 text-xs text-green-500">Done</p>
						{/if}
					</div>
					{#if it.state === 'downloading'}
						<span class="shrink-0 text-xs tabular-nums text-muted-foreground">{it.percent}%</span>
						<button
							class="flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded text-muted-foreground hover:bg-accent/10 hover:text-foreground"
							onclick={() => cancelDownload(it.id)}
							aria-label="Cancel download"
							title="Cancel"
						>
							<HugeiconsIcon icon={Cancel01Icon} class="h-3.5 w-3.5" />
						</button>
					{/if}
				</div>
			{/each}
			{#each stored as t (t.video_id)}
				<div class="group flex items-center gap-3 rounded-xl px-3 py-2 transition-colors hover:bg-accent/10">
					{#if t.thumb}
						<img decoding="async" src={t.thumb} alt="" loading="lazy" class="h-10 w-10 shrink-0 rounded-md object-cover" />
					{:else}
						<div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground/50">
							<HugeiconsIcon icon={Download01Icon} class="h-4 w-4" />
						</div>
					{/if}
					<div class="min-w-0 flex-1">
						<p class="truncate text-sm font-medium">{t.title}</p>
						<p class="truncate text-xs text-muted-foreground">
							{t.artists}{t.duration ? ` · ${fmtDur(t.duration)}` : ''} · {fmtSize(t.size_bytes)} · {fmtDate(t.added_at)}
						</p>
					</div>
					{#if downloadedIds.has(t.video_id)}
						<span class="shrink-0 rounded-full bg-green-500/15 px-2 py-0.5 text-[10px] font-semibold text-green-500">OFFLINE</span>
					{/if}
					<button
						class="flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded text-muted-foreground opacity-0 transition group-hover:opacity-100 hover:bg-accent/10 hover:text-foreground focus-visible:opacity-100"
						onclick={() => playStored(t)}
						aria-label="Play {t.title}"
						title="Play"
					>
						<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" />
					</button>
					<button
						class="flex h-7 w-7 shrink-0 cursor-pointer items-center justify-center rounded text-muted-foreground opacity-0 transition group-hover:opacity-100 hover:bg-destructive/10 hover:text-destructive focus-visible:opacity-100"
						onclick={() => removeStored(t)}
						aria-label="Delete {t.title}"
						title="Delete file and entry"
					>
						<HugeiconsIcon icon={Delete01Icon} class="h-4 w-4" />
					</button>
				</div>
			{/each}
		</div>
	{/if}
</div>
