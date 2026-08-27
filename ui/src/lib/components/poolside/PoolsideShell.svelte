<script lang="ts">
	// Poolside Vinyl shell — full-layout beta. Fonts + poolside stylesheet imported HERE.
	import '@fontsource/space-mono/400.css';
	import '@fontsource/space-mono/700.css';
	import '@fontsource/space-mono/400-italic.css';
	import '@fontsource/silkscreen/400.css';
	import '@fontsource/silkscreen/700.css';
	import '@fontsource/audiowide/400.css';
	import './poolside.css';

	import { onMount } from 'svelte';
	import { fade, scale, fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Settings01Icon,
		Sun01Icon,
		Moon02Icon,
		Mic01Icon,
		Home02Icon,
		Search01Icon,
		LibraryIcon,
		HistoryIcon,
		Playlist02Icon,
		Queue01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { BrowseItem, SongItem } from '$lib/api';
	import { auth, local, playback, playFrom, ui, toast } from '$lib/player.svelte';
	import { applyLayout } from '$lib/theme.svelte';
	import Water from './Water.svelte';
	import Vinyl from './Vinyl.svelte';
	import NowView from './NowView.svelte';
	import LibraryView from './LibraryView.svelte';
	import AlbumView from './AlbumView.svelte';
	import HomeView from './HomeView.svelte';
	import SearchView from './SearchView.svelte';
	import HistoryView from './HistoryView.svelte';
	import QueueView from './QueueView.svelte';
	import MiniPlayer from './MiniPlayer.svelte';
	import LyricsView from '../LyricsView.svelte';

	type View = 'home' | 'search' | 'library' | 'history' | 'now' | 'queue' | 'album';
	let view = $state<View>('home');
	let dusk = $state(localStorage.getItem('ps-dusk') === 'true');
	let lyricsOpen = $state(false);
	let ccOpen = $state(false);
	let ccAlbum = $state<BrowseItem | null>(null);
	let ccFileInput: HTMLInputElement | undefined = $state();
	let settingsOpen = $state(false);
	let covers = $state<Record<string, string>>({});

	// poolside visual prefs
	let caustics = $state(localStorage.getItem('ps-caustics') !== 'false');
	let koi = $state(localStorage.getItem('ps-koi') !== 'false');
	let reduce = $state(localStorage.getItem('ps-reduce') === 'true');
	let spin = $state(localStorage.getItem('ps-spin') ?? '3s');

	// library data
	let ytmAlbums = $state<BrowseItem[]>([]);
	let likedSongs = $state<SongItem[]>([]);
	let albumsLoaded = $state(false);
	let album = $state<BrowseItem | null>(null);

	const localAlbumTiles = $derived.by(() => {
		const byName = new Map<string, SongItem>();
		for (const s of local.songs) {
			const key = s.album || 'Unknown Album';
			if (!byName.has(key)) byName.set(key, s);
		}
		return [...byName.entries()].map(([albumName, first]) => ({
			kind: 'album' as const,
			id: `LOCALALBUM:${albumName}`,
			title: albumName,
			subtitle: first.artists || 'Local',
			thumbnail: first.thumbnail
		}));
	});
	const mergedAlbums = $derived([...localAlbumTiles, ...ytmAlbums]);
	const songs = $derived([...likedSongs, ...local.songs]);

	function artFor(item: BrowseItem): string {
		return covers[item.id] ?? item.thumbnail ?? '';
	}
	function saveCovers() {
		try { localStorage.setItem('ps-covers', JSON.stringify(covers)); } catch { /* quota */ }
	}

	function go(v: View) {
		view = v;
	}

	$effect(() => {
		if (auth.account?.signedIn && !albumsLoaded) {
			albumsLoaded = true;
			api.getLibraryAlbums().then((a) => (ytmAlbums = a)).catch(() => {});
			api.getPlaylist(api.LIKED_MUSIC_ID).then((p) => (likedSongs = p.items)).catch(() => {});
		}
	});

	function openAlbum(item: BrowseItem) {
		album = item;
		go('album');
	}
	function playAlbum(item: BrowseItem) {
		if (item.id.startsWith('LOCALALBUM:')) {
			const tracks = local.songs.filter((s) => (s.album || 'Unknown Album') === item.title);
			if (!tracks.length) { toast.error('No tracks found for this album'); return; }
			playFrom(item, tracks, 0);
			go('now');
			return;
		}
		api.getAlbum(item.id).then((alb) => {
			if (!alb.items.length) { toast.error('This album has no playable tracks'); return; }
			playFrom(item, alb.items, 0, alb.playlistId ?? undefined, undefined, alb.continuation);
			go('now');
		}).catch((e) => toast.error(String(e)));
	}
	function playSongInList(s: SongItem, i: number, list: SongItem[]) {
		if (s.video_id.startsWith('LOCAL:')) {
			import('$lib/player.svelte').then((m) => m.playSong(s));
			return;
		}
		playFrom({ kind: 'playlist', id: 'ps-songs', title: 'Poolside' }, list, i);
	}

	function openCustomCover() {
		ccAlbum = album ?? mergedAlbums[0] ?? null;
		if (!ccAlbum) { toast.error('Open an album first'); return; }
		ccOpen = true;
	}
	function onCcFile(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const f = input.files?.[0];
		if (!f || !ccAlbum) return;
		const rd = new FileReader();
		rd.onload = () => { covers = { ...covers, [ccAlbum!.id]: String(rd.result) }; saveCovers(); toast.success('Cover printed onto the disc'); ccOpen = false; };
		rd.readAsDataURL(f);
		input.value = '';
	}
	function resetCover() {
		if (!ccAlbum) return;
		const next = { ...covers }; delete next[ccAlbum.id]; covers = next; saveCovers(); toast.success('Reset to printed art');
	}

	function toggleDusk() { dusk = !dusk; localStorage.setItem('ps-dusk', String(dusk)); }
	function setSpin(v: string) { spin = v; localStorage.setItem('ps-spin', v); }
	function setPref(key: 'caustics' | 'koi' | 'reduce', v: boolean) {
		if (key === 'caustics') { caustics = v; localStorage.setItem('ps-caustics', String(v)); }
		else if (key === 'koi') { koi = v; localStorage.setItem('ps-koi', String(v)); }
		else { reduce = v; localStorage.setItem('ps-reduce', String(v)); }
	}

	async function importFolder() {
		try {
			const { open } = await import('@tauri-apps/plugin-dialog');
			const picked = await open({ directory: true, multiple: false, title: 'Add a music folder' });
			const path = Array.isArray(picked) ? picked[0] : picked;
			if (!path) return;
			toast.info('Scanning folder…');
			await api.addLocalFolder(path);
			toast.success('Folder added — local songs are in FOLDERS + SONGS');
		} catch (e) { toast.error(String(e)); }
	}

	onMount(() => {
		import('$lib/player.svelte').then((m) => m.scanLocal().catch(() => {}));
	});

	// nav items
	const navItems: { id: View; icon: typeof Home02Icon; label: string }[] = [
		{ id: 'home', icon: Home02Icon, label: 'Home' },
		{ id: 'search', icon: Search01Icon, label: 'Search' },
		{ id: 'library', icon: LibraryIcon, label: 'Library' },
		{ id: 'history', icon: HistoryIcon, label: 'History' },
		{ id: 'queue', icon: Queue01Icon, label: 'Queue' },
	];
</script>

<div
	class="ps-root {dusk ? 'dusk' : ''} {caustics ? '' : 'no-caustics'} {koi ? '' : 'no-koi'} {reduce ? 'reduce' : ''} {playback.paused ? 'paused' : ''}"
	style="--ps-spin:{spin}"
	data-view={view}
>
	<Water />

	<!-- all views stay mounted — .on toggle drives the camera-dolly transition -->
	<div class="ps-views">
		<div class="ps-view" class:on={view === 'home'}>
			<div class="ps-scroll-area">
				<HomeView onOpenAlbum={openAlbum} />
			</div>
		</div>
		<div class="ps-view" class:on={view === 'search'}>
			<div class="ps-scroll-area">
				<SearchView onOpenAlbum={openAlbum} />
			</div>
		</div>
		<div class="ps-view" class:on={view === 'library'}>
			<LibraryView
				albums={mergedAlbums}
				{songs}
				onOpenNow={() => go('now')}
				onOpenAlbum={openAlbum}
				onPlayLocalAlbum={playAlbum}
				onPlaySong={playSongInList}
				onImport={importFolder}
			/>
		</div>
		<div class="ps-view" class:on={view === 'history'}>
			<div class="ps-scroll-area">
				<HistoryView />
			</div>
		</div>
		<div class="ps-view" class:on={view === 'now'}>
			<NowView onOpenLibrary={() => go('library')} />
		</div>
		<div class="ps-view" class:on={view === 'queue'}>
			<div class="ps-scroll-area">
				<QueueView />
			</div>
		</div>
		<div class="ps-view" class:on={view === 'album'}>
			{#if album}
				<AlbumView
					albums={mergedAlbums}
					{album}
					{artFor}
					onBack={() => go('library')}
					onSelect={(a) => (album = a)}
					onPlayAlbum={playAlbum}
					onOpenCustom={openCustomCover}
				/>
			{/if}
		</div>
	</div>

	<!-- bottom navigation bar -->
	<nav class="ps-nav" aria-label="Poolside navigation">
		{#each navItems as item (item.id)}
			<button
				class="ps-nav-btn {view === item.id ? 'on' : ''}"
				onclick={() => go(item.id)}
				aria-label={item.label}
				title={item.label}
			>
				<HugeiconsIcon icon={item.icon} />
				<span class="ps-nav-label">{item.label}</span>
			</button>
		{/each}
		{#if playback.now}
			<button class="ps-nav-btn {view === 'now' ? 'on' : ''}" onclick={() => go('now')} aria-label="Now Playing" title="Now Playing">
				<div class="ps-nav-disc">
					{#if playback.now.thumbnail}
						<img src={playback.now.thumbnail} alt="" />
					{:else}
						<div class="ps-nav-disc-fallback"></div>
					{/if}
				</div>
				<span class="ps-nav-label">Playing</span>
			</button>
		{/if}
	</nav>

	<!-- mini player -->
	<MiniPlayer onOpenNow={() => go('now')} />

	<!-- edge buttons -->
	<div class="ps-edge mid">
		<button class="ps-edge-btn gear" onclick={(e) => { e.stopPropagation(); settingsOpen = !settingsOpen; }} title="Pool settings" aria-label="Pool settings">
			<HugeiconsIcon icon={Settings01Icon} />
		</button>
		<button class="ps-edge-btn" onclick={toggleDusk} title={dusk ? 'Day pool' : 'Dusk pool'} aria-label="Toggle dusk">
			<HugeiconsIcon icon={dusk ? Moon02Icon : Sun01Icon} />
		</button>
		{#if playback.now}
			<button class="ps-edge-btn" onclick={() => (lyricsOpen = !lyricsOpen)} title="Lyrics" aria-label="Lyrics">
				<HugeiconsIcon icon={Mic01Icon} />
			</button>
		{/if}
	</div>

	<!-- pool settings popover -->
	{#if settingsOpen}
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions, a11y_no_static_element_interactions -->
		<div class="ps-settings ps-glass open" role="dialog" aria-label="Pool settings">
			<h3>POOL SETTINGS</h3>
			<div class="ps-setrow">
				<span>CAUSTICS</span>
				<button class="ps-sw {caustics ? 'on' : ''}" role="switch" aria-checked={caustics} onclick={() => setPref('caustics', !caustics)} aria-label="Toggle caustics"></button>
			</div>
			<div class="ps-setrow">
				<span>REDUCE MOTION</span>
				<button class="ps-sw {reduce ? 'on' : ''}" role="switch" aria-checked={reduce} onclick={() => setPref('reduce', !reduce)} aria-label="Toggle reduce motion"></button>
			</div>
			<div class="ps-setrow">
				<span>SPIN SPEED</span>
				<select value={spin} onchange={(e) => setSpin(e.currentTarget.value)} aria-label="Record spin speed">
					<option value="2s">FAST · 2S</option>
					<option value="3s">NORMAL · 3S</option>
					<option value="4s">SLOW · 4S</option>
				</select>
			</div>
			<div class="ps-setrow" style="border-top: 1px solid rgba(255,255,255,.12); padding-top: 14px;">
				<span>APP SETTINGS</span>
				<button class="ps-exit-btn" style="background:var(--accent)" onclick={() => { ui.settingsOpen = true; settingsOpen = false; }} aria-label="Open full settings">
					OPEN
				</button>
			</div>
			<div class="ps-setrow" style="border-top: 1px solid rgba(255,255,255,.12); padding-top: 14px;">
				<span>EXIT BETA</span>
				<button class="ps-exit-btn" onclick={() => { applyLayout('default'); toast.info('Exited Poolside — back to Default layout'); }} aria-label="Exit Poolside beta">
					EXIT
				</button>
			</div>
			<div class="foot">LIMUSIC · POOLSIDE VINYL BETA</div>
		</div>
	{/if}

	<!-- lyrics overlay -->
	{#if lyricsOpen && playback.now}
		<div class="ps-lyrics-frame" role="dialog" aria-label="Lyrics">
			<button class="ps-lyrics-close" onclick={() => (lyricsOpen = false)} aria-label="Close lyrics">✕</button>
			<LyricsView expanded />
		</div>
	{/if}

	<!-- custom CD cover overlay -->
	{#if ccOpen && ccAlbum}
		{@const ca = ccAlbum}
		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
		<div class="ps-overlay open" role="dialog" aria-modal="true" aria-label="Add custom CD covers" tabindex="-1"
			onclick={(e) => { if (e.target === e.currentTarget) ccOpen = false; }}
			onkeydown={(e) => e.key === 'Escape' && (ccOpen = false)}
			transition:fade={{ duration: 200 }}
		>
			<div class="ps-card text-center" in:scale={{ start: 0.96, duration: 260 }}>
				<button class="absolute right-4 top-3 text-lg opacity-80 hover:opacity-100" onclick={() => (ccOpen = false)} aria-label="Close">✕</button>
				<h2 class="serif-big">Add Custom CD Covers!</h2>
				<p class="sub">Print your own art onto the picture disc for "{ca.title}"</p>
				<div class="ps-cc-opts justify-center">
					<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
					<div class="ps-cc-opt {covers[ca.id] === 'css:spider' ? 'sel' : ''}" role="button" tabindex="0"
						onclick={() => { covers = { ...covers, [ca.id]: 'css:spider' }; saveCovers(); }}
						onkeydown={(e) => e.key === 'Enter' && (covers = { ...covers, [ca.id]: 'css:spider' })}
					>
						<div class="ps-vinyl" style="--art:none"><div class="ps-cc-art ps-art-spider"></div><div class="spindle" style="z-index:3"></div></div>
						<span>Spider Disc</span>
					</div>
					<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
					<div class="ps-cc-opt {covers[ca.id] === 'css:smiley' ? 'sel' : ''}" role="button" tabindex="0"
						onclick={() => { covers = { ...covers, [ca.id]: 'css:smiley' }; saveCovers(); }}
						onkeydown={(e) => e.key === 'Enter' && (covers = { ...covers, [ca.id]: 'css:smiley' })}
					>
						<div class="ps-vinyl" style="--art:none"><div class="ps-cc-art ps-art-smiley"></div><div class="spindle" style="z-index:3"></div></div>
						<span>Smiley Disc</span>
					</div>
				</div>
				<div class="ps-cc-body">
					<div style="width:120px"><Vinyl src={artFor(ca)} playing={false} style="width:100%" /></div>
					<div class="flex flex-col items-start gap-3">
						<button class="ps-aqua px-4 py-2.5 text-[9px]" onclick={() => ccFileInput?.click()}>Choose image…</button>
						<button class="ps-ghost" onclick={resetCover}>Reset to printed art</button>
					</div>
				</div>
				<div class="ps-card-actions justify-center">
					<button class="ps-ghost" onclick={() => (ccOpen = false)}>Done</button>
				</div>
				<input bind:this={ccFileInput} type="file" accept="image/*" hidden onchange={onCcFile} />
			</div>
		</div>
	{/if}
</div>

<!-- mount existing dialogs via the ui store (they self-toggle) -->
{#await import('../CommandPalette.svelte') then { default: CommandPalette }}
	<CommandPalette />
{/await}
{#await import('../SettingsDialog.svelte') then { default: SettingsDialog }}
	<SettingsDialog />
{/await}
