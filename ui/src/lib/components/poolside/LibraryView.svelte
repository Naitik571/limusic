<script lang="ts">
	// Poolside Library — left column (hero sleeve with the disc sliding out, caption),
	// right glass panel (search + IMPORT MUSIC, pill tabs, panels).
	// The BETA logo is in the sidebar now (PoolsideShell) — the library is just content.
	// Local songs come from the shell: `albums` are real albums (>= 2 songs with shared
	// album string), `singles` are songs with no album tag (the "1 tracks" bug fix).
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Search01Icon, PlusSignIcon, Folder01Icon } from '@hugeicons/core-free-icons';
	import type { BrowseItem, SongItem } from '$lib/api';
	import { local } from '$lib/player.svelte';
	import Vinyl from './Vinyl.svelte';

	let {
		albums,
		songs,
		singles = [],
		onOpenNow,
		onOpenAlbum,
		onPlayLocalAlbum,
		onPlaySong,
		onImport,
		onOpenFlow
	}: {
		albums: BrowseItem[];
		songs: SongItem[];
		singles?: SongItem[];
		onOpenNow: () => void;
		onOpenAlbum: (item: BrowseItem) => void;
		onPlayLocalAlbum: (item: BrowseItem) => void;
		onPlaySong: (s: SongItem, i: number, list: SongItem[]) => void;
		onImport: () => void;
		onOpenFlow?: (view: 'library-coverflow' | 'library-fan') => void;
	} = $props();

	type Tab = 'albums' | 'songs' | 'artists' | 'folders' | 'singles';
	let tab = $state<Tab>('albums');
	let search = $state('');
	const tabs: { id: Tab; label: string }[] = $derived([
		{ id: 'albums', label: 'Albums' },
		{ id: 'songs', label: 'Songs' },
		{ id: 'artists', label: 'Artists' },
		{ id: 'singles', label: 'Singles' },
		{ id: 'folders', label: 'Folders' }
	]);

	const q = $derived(search.trim().toLowerCase());
	const filteredAlbums = $derived(
		albums.filter(
			(a) => !q || a.title.toLowerCase().includes(q) || (a.subtitle ?? '').toLowerCase().includes(q)
		)
	);
	const filteredSongs = $derived(
		songs.filter(
			(s) =>
				!q ||
				s.title.toLowerCase().includes(q) ||
				(s.artists ?? '').toLowerCase().includes(q) ||
				(s.album ?? '').toLowerCase().includes(q)
		)
	);
	const filteredSingles = $derived(
		singles.filter(
			(s) => !q || s.title.toLowerCase().includes(q) || (s.artists ?? '').toLowerCase().includes(q)
		)
	);
	// Artist groups — only include artists that have >= 2 tracks, otherwise they'd
	// show as a "1 tracks" group which is a metadata bug, not real artist grouping.
	const artistGroups = $derived.by(() => {
		const by = new Map<string, SongItem[]>();
		for (const s of filteredSongs) {
			const a = (s.artists || '').trim() || 'Unknown Artist';
			if (!by.has(a)) by.set(a, []);
			by.get(a)!.push(s);
		}
		return [...by.entries()]
			.filter(([, list]) => list.length >= 2)
			.sort((x, y) => x[0].localeCompare(y[0]));
	});
	const isFeatured = (i: number) => i === 0;

	function openOrPlay(item: BrowseItem) {
		if (item.id.startsWith('LOCALALBUM:')) onPlayLocalAlbum(item);
		else onOpenAlbum(item);
	}
	function playSongRow(s: SongItem, i: number, list: SongItem[]) {
		onPlaySong(s, i, list);
	}
</script>

<div class="ps-lib">
	<div class="ps-lib-left">
		{#if tab === 'albums' && filteredAlbums.length}
			<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
			<div
				class="ps-hero-sleeve"
				role="button"
				tabindex="0"
				onclick={() => openOrPlay(filteredAlbums[0])}
				onkeydown={(e) => e.key === 'Enter' && openOrPlay(filteredAlbums[0])}
				title="Play {filteredAlbums[0].title}"
			>
				<div class="ps-sleeve">
					<div class="mouth"></div>
					<svg class="ps-sticker" style="left:-12px;bottom:-10px;transform:rotate(-10deg)" width="64" height="40" viewBox="0 0 64 40">
						<rect x="3" y="3" width="58" height="34" rx="17" fill="#fff" stroke="#111" stroke-width="3" />
						<text x="32" y="25" text-anchor="middle" font-family="monospace" font-size="11" font-weight="bold" fill="#111">33⅓ RPM</text>
					</svg>
				</div>
				<div class="ps-hero-disc">
					<Vinyl src={filteredAlbums[0].thumbnail ?? ''} playing={false} style="width:100%" />
				</div>
			</div>
			<div class="ps-hero-cap">
				{filteredAlbums[0].title.toUpperCase()}<br />{filteredAlbums[0].subtitle ?? ''}
			</div>
		{:else if tab === 'songs' && filteredSongs.length}
			<!-- Songs tab: show a single disc for the first track + the total count -->
			<div class="ps-hero-track">
				<Vinyl src={filteredSongs[0].thumbnail ?? ''} playing={false} size={200} />
				<div class="ps-hero-cap">
					{filteredSongs[0].title.toUpperCase()}<br />{filteredSongs[0].artists}
				</div>
				<div class="ps-hero-meta">{filteredSongs.length} SONGS</div>
			</div>
		{:else if tab === 'singles' && filteredSingles.length}
			<div class="ps-hero-track">
				<Vinyl src={filteredSingles[0].thumbnail ?? ''} playing={false} size={200} />
				<div class="ps-hero-cap">
					{filteredSingles[0].title.toUpperCase()}<br />{filteredSingles[0].artists}
				</div>
				<div class="ps-hero-meta">{filteredSingles.length} SINGLES</div>
			</div>
		{:else if tab === 'artists' && artistGroups.length}
			<div class="ps-hero-artists">
				<div class="ps-hero-meta">{artistGroups.length} ARTISTS</div>
				<div class="ps-hero-artists-list">
					{#each artistGroups.slice(0, 5) as [artist, songsOf]}
						<div class="ps-hero-artist-row">
							<span class="ps-hero-artist-n">{songsOf.length}</span>
							<span class="ps-hero-artist-name">{artist}</span>
						</div>
					{/each}
				</div>
			</div>
		{:else if tab === 'folders' && local.folders.length}
			<div class="ps-hero-folders">
				<div class="ps-hero-meta">{local.folders.length} FOLDERS</div>
				{#each local.folders.slice(0, 4) as folder}
					<div class="ps-hero-folder-row">{folder.toUpperCase()}</div>
				{/each}
			</div>
		{:else}
			<div class="ps-empty" style="padding:60px 10px">
				{tab === 'albums' ? 'Your albums land here — sign in or import a folder.' : ''}
				{tab === 'songs' ? 'No songs in your library yet.' : ''}
				{tab === 'singles' ? 'No standalone tracks — every local song is in an album.' : ''}
				{tab === 'artists' ? 'No artists yet.' : ''}
				{tab === 'folders' ? 'No folders yet — use IMPORT MUSIC above.' : ''}
			</div>
		{/if}
	</div>

	<div class="ps-lib-right ps-glass">
		<div class="ps-lib-top">
			<label class="ps-search">
				<HugeiconsIcon icon={Search01Icon} />
				<input bind:value={search} type="search" placeholder="SEARCH LIBRARY…" aria-label="Search library" />
			</label>
			<button class="ps-aqua ps-import flex items-center gap-1.5" onclick={onImport}>
				<HugeiconsIcon icon={PlusSignIcon} class="w-3.5 h-3.5" />
				Import Music
			</button>
		</div>

		<div class="ps-tabs ps-tab-track" role="tablist">
			<div class="ps-tab-pill" style="left: {4 + tabs.findIndex(t => t.id === tab) * 84}px; width: 80px;"></div>
			{#each tabs as t (t.id)}
				<button class:on={tab === t.id} onclick={() => (tab = t.id)} role="tab">{t.label}</button>
			{/each}
		</div>

		<div class="ps-panels">
			{#if tab === 'albums'}
				<div class="ps-grid">
					{#each filteredAlbums as a, i (a.id)}
						<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
						<div
							class="ps-tile {isFeatured(i) ? 'feat' : ''}"
							onclick={() => openOrPlay(a)}
							role="button"
							tabindex="0"
							onkeydown={(e) => e.key === 'Enter' && openOrPlay(a)}
						>
							<div class="cov">
								{#if a.thumbnail}
									<img decoding="async" loading="lazy" src={a.thumbnail} alt={a.title} />
								{:else}
									<div class="aspect-square grid place-items-center" style="color:rgba(6,48,58,.5)"><HugeiconsIcon icon={Folder01Icon} class="w-8 h-8" /></div>
								{/if}
							</div>
							<div class="cap">{a.title}</div>
							<div class="sub">{a.subtitle ?? ''}</div>
						</div>
					{:else}
						<div class="ps-empty">No albums yet — sign in or import a folder.</div>
					{/each}
				</div>
				{#if filteredAlbums.length}
					<div class="ps-expand-row">
						<button class="ps-expand-btn" onclick={() => onOpenFlow?.('library-coverflow')} aria-label="Open coverflow">
							<HugeiconsIcon icon={PlusSignIcon} class="w-4 h-4" />
							<span>CoverFlow</span>
						</button>
					</div>
				{/if}
			{:else if tab === 'songs'}
				<div class="ps-songlist">
					{#each filteredSongs as s, i (s.video_id + i)}
						<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
						<div
							class="ps-songrow"
							onclick={() => playSongRow(s, i, filteredSongs)}
							onkeydown={(e) => e.key === 'Enter' && playSongRow(s, i, filteredSongs)}
							role="button"
							tabindex="0"
						>
							<span class="n">{String(i + 1).padStart(2, '0')}</span>
							<span class="st">{s.title.toUpperCase()}</span>
							<span class="sa">{s.artists}</span>
							<span class="sd">{s.duration ?? ''}</span>
						</div>
					{:else}
						<div class="ps-empty">No songs match.</div>
					{/each}
				</div>
				{#if filteredSongs.length}
					<div class="ps-expand-row">
						<button class="ps-expand-btn" onclick={() => onOpenFlow?.('library-fan')} aria-label="Open stacked queue">
							<HugeiconsIcon icon={PlusSignIcon} class="w-4 h-4" />
							<span>Stacked Queue</span>
						</button>
					</div>
				{/if}
			{:else if tab === 'artists'}
				{#each artistGroups as [artist, songsOf] (artist)}
					<div class="ps-artistgroup">
						<h4>{artist} · {songsOf.length} tracks</h4>
						<div class="ps-songlist">
							{#each songsOf as s, i (s.video_id + i)}
								<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
								<div
									class="ps-songrow"
									onclick={() => playSongRow(s, i, songsOf)}
									onkeydown={(e) => e.key === 'Enter' && playSongRow(s, i, songsOf)}
									role="button"
									tabindex="0"
								>
									<span class="n">{String(i + 1).padStart(2, '0')}</span>
									<span class="st">{s.title.toUpperCase()}</span>
									<span class="sa">{s.album ?? ''}</span>
									<span class="sd">{s.duration ?? ''}</span>
								</div>
							{/each}
						</div>
					</div>
				{:else}
						<div class="ps-empty">No artists yet.</div>
					{/each}
				{:else if tab === 'singles'}
					<div class="ps-songlist">
						{#each filteredSingles as s, i (s.video_id + i)}
							<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
							<div
								class="ps-songrow"
								onclick={() => playSongRow(s, i, filteredSingles)}
								onkeydown={(e) => e.key === 'Enter' && playSongRow(s, i, filteredSingles)}
								role="button"
								tabindex="0"
							>
								<span class="n">{String(i + 1).padStart(2, '0')}</span>
								<span class="st">{s.title.toUpperCase()}</span>
								<span class="sa">{s.artists}</span>
								<span class="sd">{s.duration ?? ''}</span>
							</div>
						{:else}
							<div class="ps-empty">No standalone tracks — every local song is in an album.</div>
						{/each}
					</div>
				{:else}
				{#each local.folders as folder (folder)}
					<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_noninteractive_tabindex -->
					<div
						class="ps-folderrow"
						onclick={() => (tab = 'songs')}
						onkeydown={(e) => e.key === 'Enter' && (tab = 'songs')}
						role="button"
						tabindex="0"
					>
						<HugeiconsIcon icon={Folder01Icon} />
						<span class="fn">{folder.toUpperCase()}</span>
						<span class="fc">{local.songs.length} local songs</span>
					</div>
				{:else}
					<div class="ps-empty">No folders yet — use IMPORT MUSIC above.</div>
				{/each}
			{/if}
		</div>
	</div>
</div>
