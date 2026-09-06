<script lang="ts">
	// Song Info: title, artists, album, duration and the YouTube view count for one track.
	// Opened from any ⋯ menu (`openSongInfo`). Views come from a cheap /player call and load
	// after the dialog opens; a dash means YouTube didn't report one.
	import * as Dialog from '$lib/components/ui/dialog';
	import * as api from '$lib/api';
	import { ui } from '$lib/player.svelte';
	import { thumb } from '$lib/thumb';

	let info = $state<{ view_count: string | null; author: string | null; title: string | null } | null>(null);
	let loading = $state(false);
	let forId = '';

	function fmtViews(raw: string | null): string {
		if (!raw) return '—';
		const n = Number(raw);
		if (!Number.isFinite(n)) return raw;
		if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B views`;
		if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M views`;
		if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K views`;
		return `${n} views`;
	}

	$effect(() => {
		const song = ui.songInfo;
		if (!song) {
			info = null;
			return;
		}
		if (song.video_id === forId) return;
		forId = song.video_id;
		info = null;
		loading = true;
		api
			.videoViews(song.video_id)
			.then((r) => {
				if (ui.songInfo?.video_id !== song.video_id) return;
				info = r;
			})
			.catch(() => {})
			.finally(() => {
				if (ui.songInfo?.video_id === song.video_id) loading = false;
			});
	});
</script>

<Dialog.Root bind:open={() => ui.songInfo !== null, (v) => { if (!v) ui.songInfo = null; }}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Song info</Dialog.Title>
		</Dialog.Header>
		{#if ui.songInfo}
			{@const song = ui.songInfo}
			<div class="flex items-center gap-3">
				{#if song.thumbnail}
					<img decoding="async" src={thumb(song.thumbnail, 400)} alt="" class="h-14 w-14 shrink-0 rounded-lg object-cover" />
				{/if}
				<div class="min-w-0">
					<p class="truncate text-sm font-semibold">{song.title}</p>
					<p class="truncate text-xs text-muted-foreground">{song.artists}</p>
				</div>
			</div>
			<dl class="mt-3 space-y-1.5 text-sm">
				{#if song.album}
					<div class="flex justify-between gap-4">
						<dt class="text-muted-foreground">Album</dt>
						<dd class="truncate">{song.album}</dd>
					</div>
				{/if}
				{#if song.duration}
					<div class="flex justify-between gap-4">
						<dt class="text-muted-foreground">Duration</dt>
						<dd class="tabular-nums">{song.duration}</dd>
					</div>
				{/if}
				<div class="flex justify-between gap-4">
					<dt class="text-muted-foreground">Views</dt>
					<dd class="tabular-nums">{loading ? '…' : fmtViews(info?.view_count ?? null)}</dd>
				</div>
			</dl>
		{/if}
	</Dialog.Content>
</Dialog.Root>
