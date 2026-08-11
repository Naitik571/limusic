<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
	Search01Icon,
	MusicNote01Icon,
	UserIcon,
	Album01Icon,
	Playlist01Icon
} from '@hugeicons/core-free-icons';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import MediaCardSkeleton from '$lib/components/MediaCardSkeleton.svelte';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import TrackRowSkeleton from '$lib/components/TrackRowSkeleton.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import Shelf from '$lib/components/Shelf.svelte';
	import * as api from '$lib/api';
	import type { BrowseItem, SearchResults, SongItem } from '$lib/api';
	import { getCached, putCached } from '$lib/pagecache';
	import { openAddToPlaylist, playSong } from '$lib/player.svelte';
	import { asSong } from '$lib/browse';

	let query = $state('');
	let res = $state<SearchResults | null>(null);
	let searched = $state('');
	let searching = $state(false);
	let error = $state<string | null>(null);

	// The query of the most recent runSearch call, so an older in-flight one can't clobber it.
	let latest = '';

	// ——— Live suggestions while typing ———————————————————————————————
	// Debounced search as you type; the top hits render in a dropdown under the field, so the
	// song is usually one click (or one Enter on a highlighted row) away — no full search needed.
	// ↑/↓ move the highlight, Enter picks it, Esc closes, Enter elsewhere runs the full search.
	let suggestions = $state<SearchResults | null>(null);
	let suggOpen = $state(false);
	let suggIdx = $state(-1);
	let debounce: ReturnType<typeof setTimeout> | undefined;
	let box: HTMLElement | undefined = $state();

	function onSearchInput() {
		const q = query.trim();
		clearTimeout(debounce);
		suggIdx = -1;
		if (q.length < 2) {
			suggOpen = false;
			suggestions = null;
			return;
		}
		debounce = setTimeout(() => {
			const key = `search:${q}`;
			const hit = getCached<SearchResults>(key);
			const fetch = hit
				? Promise.resolve(hit)
				: api.searchAll(q).then((r) => (putCached(key, r), r)); // feeds the full search too
			fetch
				.then((r) => {
					if (query.trim() !== q) return; // the field moved on — stale
					suggestions = r;
					suggOpen = true;
				})
				.catch(() => {}); // typing is best-effort; the full search reports errors
		}, 250);
	}

	type Sug =
		| { kind: 'song'; label: string; sub: string; song: SongItem }
		| { kind: 'artist' | 'album' | 'playlist'; label: string; sub: string; id: string };

	const artistLine = (i: BrowseItem) =>
		(i.artistRuns ?? []).length ? i.artistRuns!.map((r) => r.text).join(', ') : i.subtitle ?? '';

	const entries = $derived.by((): Sug[] => {
		if (!suggestions) return [];
		const out: Sug[] = [];
		for (const i of suggestions.top.slice(0, 1))
			out.push({ kind: 'song', label: i.title, sub: `Top result · ${artistLine(i)}`, song: asSong(i) });
		for (const i of suggestions.songs.slice(0, 4))
			out.push({ kind: 'song', label: i.title, sub: artistLine(i), song: asSong(i) });
		for (const i of suggestions.artists.slice(0, 2)) out.push({ kind: 'artist', label: i.title, sub: 'Artist', id: i.id });
		for (const i of suggestions.albums.slice(0, 2)) out.push({ kind: 'album', label: i.title, sub: 'Album', id: i.id });
		for (const i of suggestions.playlists.slice(0, 2)) out.push({ kind: 'playlist', label: i.title, sub: 'Playlist', id: i.id });
		return out;
	});

	function activateSug(s: Sug) {
		suggOpen = false;
		if (s.kind === 'song') {
			playSong(s.song);
		} else if (s.kind === 'artist') goto(`/artist/${s.id}`);
		else if (s.kind === 'album') goto(`/album/${s.id}`);
		else goto(`/playlist/${s.id}`);
	}

	function onSugKeydown(e: KeyboardEvent) {
		if (!suggOpen || !entries.length) return;
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			suggIdx = (suggIdx + 1) % entries.length;
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			suggIdx = (suggIdx - 1 + entries.length) % entries.length;
		} else if (e.key === 'Enter' && suggIdx >= 0) {
			e.preventDefault();
			activateSug(entries[suggIdx]);
		} else if (e.key === 'Escape') {
			suggOpen = false;
		}
	}

	// Outside click closes the dropdown; clicks inside stay so item clicks land.
	$effect(() => {
		if (!suggOpen) return;
		const close = (e: PointerEvent) => {
			if (!box?.contains(e.target as Node)) suggOpen = false;
		};
		window.addEventListener('pointerdown', close);
		return () => window.removeEventListener('pointerdown', close);
	});

	async function runSearch() {
		if (!query.trim()) return;
		suggOpen = false; // the full results take over
		const q = query;
		latest = q;
		const key = `search:${q}`;
		const hit = getCached<SearchResults>(key);
		if (hit) {
			res = hit;
			searched = q;
			searching = false;
		} else {
			searching = true;
		}
		error = null;
		try {
			const fresh = await api.searchAll(q);
			if (latest !== q) return; // a newer search superseded this one
			res = fresh;
			searched = q;
			putCached(key, fresh);
		} catch (e) {
			if (latest !== q) return;
			if (!hit) error = String(e);
		} finally {
			if (latest === q) searching = false;
		}
	}

	function showMore(cat: 'songs' | 'albums' | 'artists' | 'playlists') {
		goto(`/search-more?${new URLSearchParams({ q: searched, cat }).toString()}`);
	}

	// Run the search when arriving with a ?q= (e.g. from the Home search box). Keyed on the URL
	// alone: typing a new query in the field must not look like a URL change and bounce us back.
	const urlQuery = $derived(page.url.searchParams.get('q') ?? '');
	let lastUrlQuery = '';
	$effect(() => {
		if (urlQuery && urlQuery !== lastUrlQuery) {
			lastUrlQuery = urlQuery;
			query = urlQuery;
			runSearch();
		}
	});

	// Sections are horizontal card rows, except Songs which is a vertical list. `top` has no "show more".
	const sections = $derived(
		res
			? [
					{ key: 'top', label: 'Top results', items: res.top, max: 4, more: false, list: false },
					{ key: 'songs', label: 'Songs', items: res.songs, max: 6, more: true, list: true },
					{ key: 'albums', label: 'Albums', items: res.albums, max: 5, more: true, list: false },
					{ key: 'artists', label: 'Artists', items: res.artists, max: 3, more: true, list: false },
					{ key: 'playlists', label: 'Playlists', items: res.playlists, max: 5, more: true, list: false }
				].filter((s) => s.items.length)
			: []
	);

</script>

<div class="flex h-full flex-col">
	<div class="border-b p-6">
		<h1 class="mb-4 font-heading text-2xl font-bold">Search</h1>
		<form
			class="flex max-w-xl gap-2"
			onsubmit={(e) => {
				e.preventDefault();
				runSearch();
			}}
		>
			<div class="relative min-w-0 flex-1" bind:this={box}>
				<Input
					bind:value={query}
					oninput={onSearchInput}
					onkeydown={onSugKeydown}
					placeholder="Search songs, albums, artists, playlists…"
				/>
				{#if suggOpen && entries.length}
					<div class="absolute top-full right-0 left-0 z-50 mt-1.5 overflow-hidden rounded-lg border bg-popover shadow-xl">
						<div class="max-h-80 overflow-y-auto p-1">
							{#each entries as s, j (s.kind + s.label)}
								<button
									type="button"
									onclick={() => activateSug(s)}
									onpointerenter={() => (suggIdx = j)}
									class="flex w-full cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 text-left transition-colors {j ===
									suggIdx
										? 'bg-muted'
										: 'hover:bg-muted/50'}"
								>
									{#if s.kind === 'song'}
										<HugeiconsIcon icon={MusicNote01Icon} class="h-4 w-4 shrink-0 text-muted-foreground" />
									{:else if s.kind === 'artist'}
										<HugeiconsIcon icon={UserIcon} class="h-4 w-4 shrink-0 text-muted-foreground" />
									{:else if s.kind === 'album'}
										<HugeiconsIcon icon={Album01Icon} class="h-4 w-4 shrink-0 text-muted-foreground" />
									{:else}
										<HugeiconsIcon icon={Playlist01Icon} class="h-4 w-4 shrink-0 text-muted-foreground" />
									{/if}
									<span class="min-w-0 flex-1">
										<span class="block truncate text-sm font-medium">{s.label}</span>
										<span class="block truncate text-xs text-muted-foreground">{s.sub}</span>
									</span>
								</button>
							{/each}
						</div>
					</div>
				{/if}
			</div>
			<Button type="submit" class="gap-2" disabled={searching}>
				<HugeiconsIcon icon={Search01Icon} class="h-4 w-4" />
				{searching ? 'Searching…' : 'Search'}
			</Button>
		</form>
		{#if error}<div class="mt-2"><ErrorState message={error} onRetry={runSearch} /></div>{/if}
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto p-6">
		{#if searching}
			<div class="flex flex-col gap-10">
				<section>
					<Skeleton class="mb-3 h-6 w-40 rounded" />
					{#each Array(5) as _, i (i)}
						<TrackRowSkeleton />
					{/each}
				</section>
				<section>
					<Skeleton class="mb-3 h-6 w-32 rounded" />
					<div class="flex gap-2 overflow-hidden pb-2">
						{#each Array(5) as _, i (i)}
							<div class="w-40 shrink-0"><MediaCardSkeleton /></div>
						{/each}
					</div>
				</section>
			</div>
		{:else if !res}
			<p class="text-sm text-muted-foreground">Search for a song, album, artist, or playlist.</p>
		{:else if !sections.length}
			<p class="text-sm text-muted-foreground">No results for “{searched}”.</p>
		{:else}
			<div class="content-in flex flex-col gap-10">
				{#each sections as sec (sec.key)}
					<section>
						<div class="mb-3 flex items-center justify-between">
							<h2 class="font-heading text-xl font-bold">{sec.label}</h2>
							{#if sec.more}
								<button
									class="cursor-pointer text-xs font-semibold uppercase text-muted-foreground hover:text-foreground"
									onclick={() => showMore(sec.key as 'songs' | 'albums' | 'artists' | 'playlists')}
								>
									Show more
								</button>
							{/if}
						</div>
						{#if sec.list}
							{#each sec.items.slice(0, sec.max) as item (item.id)}
								{@const song = asSong(item)}
								<TrackRow {song} onplay={() => playSong(song)} onAdd={() => openAddToPlaylist(song)} />
							{/each}
						{:else}
							<Shelf items={sec.items.slice(0, sec.max)} />
						{/if}
					</section>
				{/each}
			</div>
		{/if}
	</div>
</div>
