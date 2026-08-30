<script lang="ts">
	// Poolside Library — reference layout: left column (logo pill, hero sleeve with the disc
	// sliding out, caption), right glass panel (search + IMPORT MUSIC, pill tabs, panels).
	// Presentational: albums/songs come from the shell (YTM library + Liked Music + local).
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Search01Icon, PlusSignIcon, Folder01Icon } from '@hugeicons/core-free-icons';
	import type { BrowseItem, SongItem } from '$lib/api';
	import { local } from '$lib/player.svelte';
	import Vinyl from './Vinyl.svelte';

	let {
		albums,
		songs,
		onOpenNow,
		onOpenAlbum,
		onPlayLocalAlbum,
		onPlaySong,
		onImport
	}: {
		albums: BrowseItem[];
		songs: SongItem[];
		onOpenNow: () => void;
		onOpenAlbum: (item: BrowseItem) => void;
		onPlayLocalAlbum: (item: BrowseItem) => void;
		onPlaySong: (s: SongItem, i: number, list: SongItem[]) => void;
		onImport: () => void;
	} = $props();

	type Tab = 'albums' | 'songs' | 'artists' | 'folders';
	let tab = $state<Tab>('albums');
	let search = $state('');

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
	const artistGroups = $derived.by(() => {
		const by = new Map<string, SongItem[]>();
		for (const s of filteredSongs) {
			const a = s.artists || 'Unknown';
			if (!by.has(a)) by.set(a, []);
			by.get(a)!.push(s);
		}
		return [...by.entries()].sort((x, y) => x[0].localeCompare(y[0]));
	});
	// Tile tilt is driven by CSS :nth-child(3n) and :nth-child(4n+1) selectors — no per-tile
	// class is needed (and adding one would just be dead weight in the markup).
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
		<button class="ps-logo" onclick={onOpenNow} title="Back to now playing">
			<span
				class="inline-block h-6 w-6 rounded-full"
				style="background:conic-gradient(from 210deg,#e8e8e8,#9fb6bc,#ffffff,#7fa6ae,#e8e8e8);box-shadow:inset 0 0 0 2px rgba(255,255,255,.7),0 1px 3px rgba(0,40,50,.4)"
			></span>
			<span class="word">Limusic</span>
			<span class="badge">BETA</span>
		</button>

		{#if filteredAlbums.length}
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
				<div style="position:absolute;width:86%;top:7%;left:-24%">
					<Vinyl src={filteredAlbums[0].thumbnail ?? ''} playing={false} style="width:100%" />
				</div>
			</div>
			<div class="ps-hero-cap">
				{filteredAlbums[0].title.toUpperCase()}<br />{filteredAlbums[0].subtitle ?? ''}
			</div>
		{:else}
			<div class="ps-empty" style="padding:60px 10px">Your albums land here —<br />sign in or import a folder.</div>
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

		<div class="ps-tabs" role="tablist">
			<button class:on={tab === 'albums'} onclick={() => (tab = 'albums')} role="tab">Albums</button>
			<button class:on={tab === 'songs'} onclick={() => (tab = 'songs')} role="tab">Songs</button>
			<button class:on={tab === 'artists'} onclick={() => (tab = 'artists')} role="tab">Artists</button>
			<button class:on={tab === 'folders'} onclick={() => (tab = 'folders')} role="tab">Folders</button>
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
									<img src={a.thumbnail} alt={a.title} />
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
