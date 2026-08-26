<script lang="ts">
	// Poolside Library: ALBUMS / SONGS / ARTISTS / FOLDERS over real data. Albums merge the
	// signed-in library with local folders; songs come from Liked Music + local files.
	import { onMount } from 'svelte';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import { Search01Icon, PlusSignIcon, Folder01Icon } from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { BrowseItem, SongItem } from '$lib/api';
	import { auth, local, playSong, playFrom, scanLocal, addLocalFolder, toast } from '$lib/player.svelte';

	let {
		onOpenNow,
		onOpenAlbum
	}: {
		onOpenNow: () => void;
		onOpenAlbum: (item: BrowseItem) => void;
	} = $props();

	type Tab = 'albums' | 'songs' | 'artists' | 'folders';
	let tab = $state<Tab>('albums');
	let search = $state('');

	let ytmAlbums = $state<BrowseItem[]>([]);
	let ytmArtists = $state<BrowseItem[]>([]);
	let likedSongs = $state<SongItem[]>([]);
	let loaded = $state(false);

	// Local album tiles: group local songs by their album name.
	const localAlbumTiles = $derived.by(() => {
		const byName = new Map<string, SongItem>();
		for (const s of local.songs) {
			const key = s.album || 'Unknown Album';
			if (!byName.has(key)) byName.set(key, s);
		}
		return [...byName.entries()].map(([album, first]) => ({
			kind: 'album' as const,
			id: `LOCALALBUM:${album}`,
			title: album,
			subtitle: first.artists || 'Local',
			thumbnail: first.thumbnail,
			local: true
		}));
	});

	const albums = $derived([...localAlbumTiles, ...ytmAlbums]);
	const allSongs = $derived([...local.songs, ...likedSongs]);

	const q = $derived(search.trim().toLowerCase());
	const filteredAlbums = $derived(
		albums.filter(
			(a) => !q || a.title.toLowerCase().includes(q) || (a.subtitle ?? '').toLowerCase().includes(q)
		)
	);
	const filteredSongs = $derived(
		allSongs.filter(
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

	const tilt = (i: number) => (i % 3 === 2 ? 'tr' : i % 4 === 1 ? 'tl' : '');

	function playAlbumTile(item: (typeof localAlbumTiles)[number] | BrowseItem) {
		if (item.id.startsWith('LOCALALBUM:')) {
			const albumName = item.title;
			const tracks = local.songs.filter((s) => (s.album || 'Unknown Album') === albumName);
			if (!tracks.length) {
				toast.error('No tracks found for this album');
				return;
			}
			playFrom(item, tracks, 0);
			return;
		}
		onOpenAlbum(item);
	}

	function playSongRow(s: SongItem, i: number, list: SongItem[]) {
		if (s.video_id.startsWith('LOCAL:')) {
			playSong(s);
			return;
		}
		// play in the context of the visible list, like a queue
		playFrom({ kind: 'playlist', id: 'ps-songs', title: 'Poolside' }, list, i);
	}

	async function importFolder() {
		try {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const picked = await open({ directory: true, multiple: false, title: 'Add a music folder' });
			const path = Array.isArray(picked) ? picked[0] : picked;
			if (!path) return;
			toast.info('Scanning folder…');
			await addLocalFolder(path);
			scanLocal();
			toast.success('Folder added — local songs are in FOLDERS + SONGS');
			tab = 'folders';
		} catch (e) {
			toast.error(String(e));
		}
	}

	function load() {
		if (!auth.account?.signedIn) return;
		api
			.getLibraryAlbums()
			.then((a) => (ytmAlbums = a))
			.catch(() => {});
		api
			.getLibraryArtists()
			.then((a) => (ytmArtists = a))
			.catch(() => {});
		api
			.getPlaylist(api.LIKED_MUSIC_ID)
			.then((p) => (likedSongs = p.items))
			.catch(() => {});
	}

	$effect(() => {
		if (auth.account?.signedIn && !loaded) {
			loaded = true;
			load();
		}
	});

	onMount(() => {
		scanLocal();
	});
</script>

<div class="ps-view on ps-lib">
	<div class="ps-lib-left">
		<button class="ps-logo" onclick={onOpenNow} title="Back to now playing">
			<span
				class="inline-block h-6 w-6 rounded-full"
				style="background:conic-gradient(from 210deg,#e8e8e8,#9fb6bc,#ffffff,#7fa6ae,#e8e8e8);box-shadow:inset 0 0 0 2px rgba(255,255,255,.7),0 1px 3px rgba(0,40,50,.4)"
			></span>
			<span class="word">Limusic</span>
			<span class="badge" style="background:#E02020">BETA</span>
		</button>

		<!-- hero sleeve: first album slides its disc out -->
		{#if filteredAlbums.length}
			<div
				class="ps-hero-sleeve"
				role="button"
				tabindex="0"
				onclick={() => playAlbumTile(filteredAlbums[0])}
				onkeydown={(e) => e.key === 'Enter' && playAlbumTile(filteredAlbums[0])}
				title="Play {filteredAlbums[0].title}"
			>
				<div class="ps-sleeve"><div class="mouth"></div></div>
				{#if filteredAlbums[0].thumbnail}
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<img
						class="ps-vinyl"
						style="width:86%;top:7%;left:-24%;position:absolute;transition:transform .7s var(--ps-ease)"
						src={filteredAlbums[0].thumbnail}
						alt={filteredAlbums[0].title}
					/>
				{/if}
				<svg class="ps-sticker" style="left:-12px;bottom:-10px;transform:rotate(-10deg)" width="64" height="40" viewBox="0 0 64 40">
					<rect x="3" y="3" width="58" height="34" rx="17" fill="#fff" stroke="#111" stroke-width="3" />
					<text x="32" y="25" text-anchor="middle" font-family="monospace" font-size="11" font-weight="bold" fill="#111">33⅓ RPM</text>
				</svg>
			</div>
			<div class="ps-hero-cap">
				{filteredAlbums[0].title.toUpperCase()}<br />{filteredAlbums[0].subtitle ?? ''}
			</div>
		{/if}
	</div>

	<div class="ps-lib-right ps-glass">
		<div class="ps-lib-top">
			<label class="ps-search">
				<HugeiconsIcon icon={Search01Icon} class="w-3.5 h-3.5 opacity-65" />
				<input bind:value={search} type="search" placeholder="Search library…" aria-label="Search library" />
			</label>
			<button class="ps-aqua px-4 py-2.5 text-[9px] flex items-center gap-1.5" onclick={importFolder} title="Add a local music folder">
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
							class="ps-tile {tilt(i)} {i === 0 ? 'feat' : ''}"
							onclick={() => playAlbumTile(a)}
							role="button"
							tabindex="0"
							onkeydown={(e) => e.key === 'Enter' && playAlbumTile(a)}
						>
							<div class="cov">
								{#if a.thumbnail}
									<img src={a.thumbnail} alt={a.title} />
								{:else}
									<div class="aspect-square grid place-items-center bg-muted text-muted-foreground"><HugeiconsIcon icon={Folder01Icon} class="w-8 h-8" /></div>
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
				<div class="flex flex-col gap-0.5">
					{#each filteredSongs as s, i (s.video_id + i)}
						<div class="ps-songrow" role="button" tabindex="0" onclick={() => playSongRow(s, i, filteredSongs)} onkeydown={(e) => e.key === 'Enter' && playSongRow(s, i, filteredSongs)}>
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
				<div class="ps-artistgrid">
					{#each artistGroups as [artist, songs] (artist)}
						<div class="ps-artist" role="button" tabindex="0" onclick={() => (tab = 'songs')} onkeydown={(e) => e.key === 'Enter' && (tab = 'songs')}>
							<div
								class="av"
								style="background:linear-gradient(140deg, rgba(255,255,255,.5), rgba(6,48,58,.6))"
							>
								{artist.slice(0, 2).toUpperCase()}
							</div>
							<span>{artist}</span>
							<span class="opacity-60">{songs.length} songs</span>
						</div>
					{:else}
						<div class="ps-empty">No artists yet.</div>
					{/each}
				</div>
			{:else}
				<div class="flex flex-col gap-2">
					{#each local.folders as folder (folder)}
						<div class="ps-folderrow" role="button" tabindex="0" onclick={() => (tab = 'songs')} onkeydown={(e) => e.key === 'Enter' && (tab = 'songs')}>
							<HugeiconsIcon icon={Folder01Icon} class="w-6 h-6" />
							<span class="text-[9.5px] tracking-wide uppercase flex-1">{folder}</span>
							<span class="text-[9px] opacity-65 uppercase">{local.songs.length} local songs</span>
						</div>
					{:else}
						<div class="ps-empty">No folders yet — use IMPORT MUSIC above.</div>
					{/each}
					{#if local.songs.length}
						<div class="ps-songrow" role="button" tabindex="0" onclick={() => (tab = 'songs')} onkeydown={(e) => e.key === 'Enter' && (tab = 'songs')}>
							<span class="n">♪</span>
							<span class="st">All local songs</span>
							<span class="sa">{local.songs.length} tracks</span>
							<span class="sd"></span>
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>
