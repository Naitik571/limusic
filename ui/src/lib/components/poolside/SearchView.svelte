<script lang="ts">
	// Poolside Search — YouTube search with autocomplete, categorized results.
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Search01Icon, PlayIcon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import { searchPreview } from '$lib/browse';
	import type { BrowseItem, SongItem } from '$lib/api';
	import { playback, playFrom, toast } from '$lib/player.svelte';

	let {
		onOpenAlbum
	}: {
		onOpenAlbum: (item: BrowseItem) => void;
	} = $props();

	let query = $state('');
	let suggestions = $state<BrowseItem[]>([]);
	let results = $state<{ songs: BrowseItem[]; albums: BrowseItem[]; artists: BrowseItem[]; playlists: BrowseItem[] }>({ songs: [], albums: [], artists: [], playlists: [] });
	let hasSearched = $state(false);
	let searching = $state(false);
	let debounceTimer: ReturnType<typeof setTimeout> | undefined;

	function onInput() {
		clearTimeout(debounceTimer);
		if (!query.trim()) { suggestions = []; return; }
		debounceTimer = setTimeout(async () => {
			try { suggestions = await searchPreview(query.trim()); } catch { suggestions = []; }
		}, 200);
	}

	async function doSearch() {
		const q = query.trim();
		if (!q) return;
		suggestions = [];
		searching = true;
		hasSearched = true;
		try {
			const r = await api.searchAll(q);
			results = {
				songs: r.songs ?? [],
				albums: r.albums ?? [],
				artists: r.artists ?? [],
				playlists: r.playlists ?? []
			};
		} catch { results = { songs: [], albums: [], artists: [], playlists: [] }; }
		searching = false;
	}

	function onKeyDown(e: KeyboardEvent) {
		if (e.key === 'Enter') { doSearch(); return; }
	}

	function pickSuggestion(item: BrowseItem) {
		query = item.title;
		doSearch();
	}

	function playSong(s: BrowseItem, i: number) {
		const song: SongItem = { video_id: s.id, title: s.title, artists: s.subtitle ?? '', thumbnail: s.thumbnail };
		playFrom({ kind: 'playlist', id: 'ps-search', title: 'Search' }, [song], 0);
	}
</script>

<div class="ps-search-view">
	<!-- search bar -->
	<div class="ps-search-bar ps-anim-fade-up">
		<label class="ps-search-input-wrap">
			<HugeiconsIcon icon={Search01Icon} />
			<input
				bind:value={query}
				type="search"
				placeholder="SEARCH YOUTUBE MUSIC…"
				aria-label="Search YouTube Music"
				oninput={onInput}
				onkeydown={onKeyDown}
			/>
		</label>
		<button class="ps-aqua" onclick={doSearch}>Search</button>
	</div>

	<!-- suggestions dropdown -->
	{#if suggestions.length}
		<div class="ps-suggestions ps-glass ps-anim-fade-down">
			{#each suggestions.slice(0, 8) as s (s.id)}
				<button class="ps-suggestion-row" onclick={() => pickSuggestion(s)}>
					{#if s.thumbnail}
						<img src={s.thumbnail} alt="" class="ps-sug-thumb" />
					{/if}
					<span class="ps-sug-title">{s.title}</span>
					<span class="ps-sug-sub">{s.subtitle ?? ''}</span>
				</button>
			{/each}
		</div>
	{/if}

	{#if searching}
		<div class="ps-loading-dots"><span></span><span></span><span></span></div>
	{/if}

	<!-- results -->
	{#if hasSearched && !searching}
		<!-- songs -->
		{#if results.songs.length}
			<section class="ps-section ps-anim-fade-up" style="animation-delay:.05s">
				<h3 class="ps-section-title">SONGS</h3>
				<div class="ps-songlist">
					{#each results.songs as s, i (s.id + i)}
						<div class="ps-songrow ps-anim-slide-in" style="animation-delay:{i * 0.03}s" onclick={() => playSong(s, i)} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && playSong(s, i)}>
							<span class="n">{String(i + 1).padStart(2, '0')}</span>
							<span class="st">{s.title.toUpperCase()}</span>
							<span class="sa">{s.subtitle ?? ''}</span>
						</div>
					{/each}
				</div>
			</section>
		{/if}

		<!-- albums -->
		{#if results.albums.length}
			<section class="ps-section ps-anim-fade-up" style="animation-delay:.1s">
				<h3 class="ps-section-title">ALBUMS</h3>
				<div class="ps-rail">
					{#each results.albums as a (a.id)}
						<button class="ps-rail-card" onclick={() => onOpenAlbum(a)}>
							{#if a.thumbnail}<img src={a.thumbnail} alt={a.title} />{:else}<div class="ps-rail-placeholder"></div>{/if}
							<span class="ps-rail-label">{a.title}</span>
							<span class="ps-rail-sub">{a.subtitle ?? ''}</span>
						</button>
					{/each}
				</div>
			</section>
		{/if}

		<!-- artists -->
		{#if results.artists.length}
			<section class="ps-section ps-anim-fade-up" style="animation-delay:.15s">
				<h3 class="ps-section-title">ARTISTS</h3>
				<div class="ps-rail ps-rail-artists">
					{#each results.artists as a (a.id)}
						<button class="ps-rail-card ps-rail-artist" onclick={() => onOpenAlbum(a)}>
							{#if a.thumbnail}<img src={a.thumbnail} alt={a.title} class="ps-artist-circle" />{:else}<div class="ps-rail-placeholder ps-artist-circle"></div>{/if}
							<span class="ps-rail-label">{a.title}</span>
						</button>
					{/each}
				</div>
			</section>
		{/if}

		<!-- playlists -->
		{#if results.playlists.length}
			<section class="ps-section ps-anim-fade-up" style="animation-delay:.2s">
				<h3 class="ps-section-title">PLAYLISTS</h3>
				<div class="ps-rail">
					{#each results.playlists as p (p.id)}
						<button class="ps-rail-card" onclick={() => onOpenAlbum(p)}>
							{#if p.thumbnail}<img src={p.thumbnail} alt={p.title} />{:else}<div class="ps-rail-placeholder"></div>{/if}
							<span class="ps-rail-label">{p.title}</span>
							<span class="ps-rail-sub">{p.subtitle ?? ''}</span>
						</button>
					{/each}
				</div>
			</section>
		{/if}

		{#if !results.songs.length && !results.albums.length && !results.artists.length && !results.playlists.length}
			<div class="ps-empty ps-anim-fade-up">No results found.</div>
		{/if}
	{/if}

	{#if !hasSearched}
		<div class="ps-search-empty ps-anim-fade-up">
			<HugeiconsIcon icon={Search01Icon} />
			<p>Search for songs, albums, artists, and playlists on YouTube Music.</p>
		</div>
	{/if}
</div>
