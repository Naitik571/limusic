<script lang="ts">
	// Live search-as-you-type for the home hero and the search page. Debounced, cached, and
	// keyboard-navigable (↑/↓ + Enter to pick, Esc to close). Enter with nothing highlighted
	// falls through to `onsubmit` — the parent's full search. The input is controlled from
	// `query` and reports keystrokes via `onchange`, so the same box works anywhere without
	// fighting a parent form (and there's no bind-vs-event ordering race).
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { MusicNote01Icon, Search01Icon, UserIcon, Album01Icon, Playlist01Icon } from '@hugeicons/core-free-icons';
	import { Input } from '$lib/components/ui/input';
	import * as api from '$lib/api';
	import type { BrowseItem, SearchResults, SongItem } from '$lib/api';
	import { getCached, putCached } from '$lib/pagecache';
	import { playSong } from '$lib/player.svelte';
	import { asSong } from '$lib/browse';
	import { thumb } from '$lib/thumb';

	let {
		query,
		onchange,
		onsubmit,
		placeholder = 'Search',
		icon = false,
		class: cls = '',
		inputClass = ''
	}: {
		query: string;
		onchange?: (q: string) => void;
		onsubmit?: () => void;
		placeholder?: string;
		/** Renders the search glyph inside the field (hero style). */
		icon?: boolean;
		class?: string;
		inputClass?: string;
	} = $props();

	let suggestions = $state<SearchResults | null>(null);
	let suggOpen = $state(false);
	let suggIdx = $state(-1);
	let debounce: ReturnType<typeof setTimeout> | undefined;
	let box: HTMLElement | undefined = $state();

	// Debounce on the query prop: a reactive read, so this fires on every keystroke with the
	// freshest value and cleans itself up before the next one.
	$effect(() => {
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
			const fetch = hit ? Promise.resolve(hit) : api.searchAll(q).then((r) => (putCached(key, r), r));
			fetch
				.then((r) => {
					if (query.trim() !== q) return; // the field moved on — stale
					suggestions = r;
					suggOpen = true;
				})
				.catch(() => {}); // typing is best-effort; the full search reports errors
		}, 250);
	});

	type Sug =
		| { kind: 'song'; label: string; sub: string; song: SongItem }
		| {
				kind: 'artist' | 'album' | 'playlist';
				label: string;
				sub: string;
				id: string;
				thumb?: string;
		  };

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
		for (const i of suggestions.albums.slice(0, 2))
			out.push({ kind: 'album', label: i.title, sub: 'Album', id: i.id, thumb: i.thumbnail });
		for (const i of suggestions.playlists.slice(0, 2))
			out.push({ kind: 'playlist', label: i.title, sub: 'Playlist', id: i.id, thumb: i.thumbnail });
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

	function onKeydown(e: KeyboardEvent) {
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
			e.preventDefault();
			suggOpen = false;
		}
		// Enter with nothing highlighted bubbles to the parent form → full search.
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
</script>

<div class="relative w-full {cls}" bind:this={box}>
	{#if icon}
		<HugeiconsIcon
			icon={Search01Icon}
			class="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-muted-foreground"
		/>
	{/if}
	<Input
		value={query}
		oninput={(e) => onchange?.((e.currentTarget as HTMLInputElement).value)}
		onkeydown={onKeydown}
		{placeholder}
		class="{inputClass}{icon ? ' pl-9' : ''}"
	/>
	{#if suggOpen && entries.length}
		<div class="absolute top-full right-0 left-0 z-50 mt-1.5 overflow-hidden rounded-xl glass-strong shadow-2xl">
			<div class="max-h-80 overflow-y-auto p-1">
				{#each entries as s, j (s.kind + s.label)}
					<button
						type="button"
						onclick={() => activateSug(s)}
						onpointerenter={() => (suggIdx = j)}
						class="flex w-full cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 text-left transition-colors {j === suggIdx ? 'bg-muted' : 'hover:bg-muted/50'}"
					>
						{#if (s.kind === 'song' ? s.song.thumbnail : s.kind === 'artist' ? undefined : s.thumb)}
							<img
								src={thumb(s.kind === 'song' ? s.song.thumbnail : s.kind === 'artist' ? undefined : s.thumb, 64)}
								alt=""
								loading="lazy"
								class="h-9 w-9 shrink-0 rounded object-cover"
							/>
						{:else if s.kind === 'artist'}
							<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded bg-muted">
								<HugeiconsIcon icon={UserIcon} class="h-4 w-4 text-muted-foreground" />
							</span>
						{:else}
							<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded bg-muted">
								<HugeiconsIcon
									icon={s.kind === 'album'
										? Album01Icon
										: s.kind === 'playlist'
											? Playlist01Icon
											: MusicNote01Icon}
									class="h-4 w-4 text-muted-foreground"
								/>
							</span>
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
