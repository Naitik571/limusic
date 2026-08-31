<!--
  Poolside Vinyl shell — sidebar-first beta layout.

  Structure (z-order, back -> front):
    .ps-water             contextual pool background (now reflects currently-playing album)
    .ps-shell             grid: [sidebar | main | (drawer)]
    .ps-sidebar           persistent left rail, hover-expand (icon-only at rest, labeled on hover)
    .ps-views             main column, scrollable; one view at a time, camera-dolly on switch
    .ps-mini              bottom-bar mini player, sits over main column
    .ps-lyrics-drawer     right-side lyrics, slides in/out, displaces main content
    .ps-notifications     fixed top-right toast region
    .ps-settings-panel    right-side settings popover (separate from lyrics)
    .ps-overlay           centered modals (custom cover, etc.)

  vs. the previous version:
    - was: bottom tab bar floating above content; mini player in corner; lyrics as floating
      panel; settings as floating panel; everything piled on the same z-stack
    - now: each surface has a defined slot in the grid, no overlap, no in-place modals
-->
<script lang="ts">
	import '@fontsource/space-mono/400.css';
	import '@fontsource/space-mono/700.css';
	import '@fontsource/space-mono/400-italic.css';
	import '@fontsource/silkscreen/400.css';
	import '@fontsource/silkscreen/700.css';
	import '@fontsource/audiowide/400.css';
	import './poolside.css';

	import { onDestroy, onMount, untrack } from 'svelte';
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
		Queue01Icon,
		Radio01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { BrowseItem, SongItem } from '$lib/api';
	import { auth, local, np, playback, playFrom, ui, toast } from '$lib/player.svelte';
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
	import LyricsView from '../LyricsView.svelte';
	import CoverFlowCarousel from './CoverFlowCarousel.svelte';
	import RadioBrowse from './RadioBrowse.svelte';
	import RadioNowPlaying from './RadioNowPlaying.svelte';
	import EdgeVinyl from './EdgeVinyl.svelte';
	import CustomCoverPicker from './CustomCoverPicker.svelte';
	import FeatureCallout from './FeatureCallout.svelte';
	import MiniPlayer from './MiniPlayer.svelte';
	import MiniPlayerPill from './MiniPlayerPill.svelte';

	type View =
		| 'home'
		| 'search'
		| 'library'
		| 'library-carousel'
		| 'history'
		| 'now'
		| 'queue'
		| 'album'
		| 'radio'
		| 'radio-now';
	let view = $state<View>('home');
	let album = $state<BrowseItem | null>(null);
	let dusk = $state(localStorage.getItem('ps-dusk') === 'true');
	let lyricsOpen = $state(false);
	// Fullscreen lyrics takeover — poolside's own sing mode. Esc or the ✕ exits.
	let sing = $state(false);
	$effect(() => {
		np.sing = sing;
	});
	// Unmount safety: switching layouts away from poolside while sing is on must not leave
	// the shared flag set — the root layout would keep the titlebar hidden forever.
	onDestroy(() => {
		np.sing = false;
	});
	let ccOpen = $state(false);
	let ccAlbum = $state<BrowseItem | null>(null);
	let ccFileInput = $state<HTMLInputElement>();
	let settingsOpen = $state(false);
	let covers = $state<Record<string, string>>({});

	// Sidebar hover-expand. Default = collapsed (icons only), expand on hover.
	let sidebarHover = $state(false);

	// Feature callout — fades in over the current view when the view first activates.
	let callout = $state<{ text: string; sectionId: string; key: number } | null>(null);
	let calloutTimer: ReturnType<typeof setTimeout> | null = null;
	const SEEN_KEY = 'ps-callouts-seen';
	let seenSet = $state<Set<string>>(new Set());
	$effect(() => {
		// Map view -> callout text. The first time a user lands on each view, show
		// the callout. Once they've seen it, don't show again. We read seenSet via
		// untrack() so the effect only re-runs on view changes, not when we
		// update the seen set ourselves below.
		const v = view;
		const map: Record<string, string> = {
			radio: 'You can now use the radio!!!',
			'library-carousel': 'Browse your library as a coverflow!',
			'radio-now': 'Live radio stations, all in one place.',
			lyrics: 'Lyrics auto-scroll and highlight the current line.',
			album: 'Tap any track to play it instantly.'
		};
		const text = map[v];
		if (!text) return;
		if (untrack(() => seenSet.has(v))) return;
		untrack(() => {
			seenSet = new Set([...seenSet, v]);
			try { localStorage.setItem(SEEN_KEY, JSON.stringify([...seenSet])); } catch { /* quota */ }
		});
		// re-mount the component on each new view so the entrance animation plays
		callout = { text, sectionId: v, key: Date.now() };
		if (calloutTimer) clearTimeout(calloutTimer);
		calloutTimer = setTimeout(() => (callout = null), 3600);
	});
	onMount(() => {
		try {
			const raw = localStorage.getItem(SEEN_KEY);
			if (raw) seenSet = new Set(JSON.parse(raw));
		} catch { /* quota */ }
	});

	// radio: the currently-selected station (for the radio-now view)
	const currentStation = $derived(
		playback.now
			? { id: 'live', name: playback.now.title || 'Live Mix', genre: 'Live', location: 'Now Playing', isLive: true, listeners: 0, isFavorite: false }
			: { id: 'lush-fm', name: 'Lush FM', genre: 'Lo-Fi', location: 'Tokyo', isLive: true, listeners: 1284, isFavorite: true }
	);

	// poolside visual prefs
	let caustics = $state(localStorage.getItem('ps-caustics') !== 'false');
	let koi = $state(localStorage.getItem('ps-koi') !== 'false');
	let reduce = $state(localStorage.getItem('ps-reduce') === 'true');
	let spin = $state(localStorage.getItem('ps-spin') ?? '3s');

	// Contextual background — the gradient derives from the currently-playing album art.
	// We pre-blur + boost saturation via CSS so it's cheap. When nothing's playing we fall
	// back to the static pool gradient.
	let albumHue = $state<number | null>(null);
	let albumAccent = $state<string | null>(null);
	$effect(() => {
		const url = playback.now?.thumbnail;
		if (!url) {
			albumHue = null;
			albumAccent = null;
			return;
		}
		// Sample the dominant hue via a tiny offscreen canvas. Cheap enough to do on track
		// change (not per-frame).
		const img = new Image();
		img.crossOrigin = 'anonymous';
		img.src = url;
		img.onload = () => {
			try {
				const c = document.createElement('canvas');
				c.width = 16; c.height = 16;
				const ctx = c.getContext('2d', { willReadFrequently: true })!;
				ctx.drawImage(img, 0, 0, 16, 16);
				const d = ctx.getImageData(0, 0, 16, 16).data;
				let r = 0, g = 0, b = 0, n = 0;
				for (let i = 0; i < d.length; i += 4) {
					// skip near-white and near-black pixels (text / pure shadows bias the average)
					if (d[i] > 230 && d[i+1] > 230 && d[i+2] > 230) continue;
					if (d[i] < 25 && d[i+1] < 25 && d[i+2] < 25) continue;
					r += d[i]; g += d[i+1]; b += d[i+2]; n++;
				}
				if (n === 0) return;
				r = Math.round(r / n); g = Math.round(g / n); b = Math.round(b / n);
				// HSL hue (we only need H for the gradient stops)
				const max = Math.max(r, g, b), min = Math.min(r, g, b);
				let h = 0; const dlt = max - min;
				if (dlt !== 0) {
					if (max === r) h = ((g - b) / dlt) % 6;
					else if (max === g) h = (b - r) / dlt + 2;
					else h = (r - g) / dlt + 4;
					h = Math.round(h * 60); if (h < 0) h += 360;
				}
				const s = max === 0 ? 0 : Math.round((dlt / max) * 100);
				const l = Math.round((max + min) / 2 / 255 * 100);
				albumHue = h;
				// push to CSS as the accent variable so existing .ps-aqua etc. follow
				document.documentElement.style.setProperty(
					'--ps-album-accent',
					`hsl(${h} ${Math.min(80, s)}% ${Math.max(40, Math.min(60, l + 5))}%)`
				);
			} catch { /* CORS-tainted canvas — leave default */ }
		};
	});

	// library data
	let ytmAlbums = $state<BrowseItem[]>([]);
	let likedSongs = $state<SongItem[]>([]);
	let albumsLoaded = $state(false);

	const localAlbumTiles = $derived.by(() => {
		// Group local songs by album name. A real album has 2+ songs sharing the same
		// album string; single songs with no siblings go to the "Singles" bucket, not
		// their own one-song "album". (This is the "1 tracks" bug the user flagged.)
		const byName = new Map<string, SongItem[]>();
		for (const s of local.songs) {
			const key = (s.album || '').trim() || '__singles__';
			if (!byName.has(key)) byName.set(key, []);
			byName.get(key)!.push(s);
		}
		const tiles: BrowseItem[] = [];
		for (const [albumName, songs] of byName) {
			if (albumName === '__singles__' || songs.length === 0) continue;
			tiles.push({
				kind: 'album',
				id: `LOCALALBUM:${albumName}`,
				title: albumName,
				subtitle: songs[0].artists || 'Local',
				thumbnail: songs[0].thumbnail
			});
		}
		return tiles;
	});
	const localSingles = $derived(local.songs.filter((s) => !(s.album || '').trim()));
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
		// Closing the lyrics drawer when navigating away so the content doesn't get
		// stranded behind the next view's mount.
		if (v !== 'now' && lyricsOpen) lyricsOpen = false;
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
			const albumName = item.id.slice('LOCALALBUM:'.length);
			const tracks = local.songs.filter((s) => (s.album || '').trim() === albumName);
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
		// Wire the NowView's "Lyrics" hint to actually open the lyrics drawer.
		const onOpenLyrics = () => {
			if (playback.now) lyricsOpen = true;
		};
		window.addEventListener('ps:open-lyrics', onOpenLyrics);
		// Esc leaves the fullscreen lyrics takeover (the ✕ works too).
		const onEsc = (e: KeyboardEvent) => {
			if (e.key === 'Escape') sing = false;
		};
		window.addEventListener('keydown', onEsc);
		return () => {
			window.removeEventListener('ps:open-lyrics', onOpenLyrics);
			window.removeEventListener('keydown', onEsc);
		};
	});

	const navItems: { id: View; icon: typeof Home02Icon; label: string }[] = [
		{ id: 'home', icon: Home02Icon, label: 'Home' },
		{ id: 'search', icon: Search01Icon, label: 'Search' },
		{ id: 'library', icon: LibraryIcon, label: 'Library' },
		{ id: 'library-carousel', icon: LibraryIcon, label: 'Cover Flow' },
		{ id: 'radio', icon: Radio01Icon, label: 'Radio' },
		{ id: 'history', icon: HistoryIcon, label: 'History' },
		{ id: 'queue', icon: Queue01Icon, label: 'Queue' }
	];
</script>

<div
	class="ps-root {dusk ? 'dusk' : ''} {caustics ? '' : 'no-caustics'} {koi ? '' : 'no-koi'} {reduce ? 'reduce' : ''} {playback.paused ? 'paused' : ''} {sidebarHover ? 'sidebar-hover' : ''} {lyricsOpen ? 'lyrics-open' : ''} {settingsOpen ? 'settings-open' : ''}"
	style="--ps-spin:{spin}"
	data-view={view}
	data-album-hue={albumHue ?? ''}
>
	<!-- Contextual water: the gradient stops now derive from the current album's hue
	     (set by the $effect above via --ps-album-accent). When nothing's playing it
	     stays on the static pool blue. -->
	<Water accent={albumAccent} />

	<!-- ============================================================
	     EDGE VINYLS — ambient depth decoration. Two large vinyl records anchored
	     off the left and right edges, ~65% cropped. They sit BEHIND the shell
	     grid (z-index between Water and .ps-shell) so the UI reads on top of them.
	     Each rotates slowly and uses the current album art as a blurred label. -->
	<EdgeVinyl side="left" art={playback.now?.thumbnail ?? ''} />
	<EdgeVinyl side="right" art={playback.now?.thumbnail ?? ''} />

	<div class="ps-shell">
		<!-- ============================================================
		     SIDEBAR — persistent left rail, hover-expand
		     ============================================================ -->
		<aside
			class="ps-sidebar"
			aria-label="Main navigation"
			onmouseenter={() => (sidebarHover = true)}
			onmouseleave={() => (sidebarHover = false)}
			role="navigation"
		>
			<button class="ps-sidebar-logo" onclick={() => go('home')} title="Limusic · Poolside Vinyl" aria-label="Limusic · Poolside Vinyl — go home">
				<span class="ps-sidebar-mark" aria-hidden="true"></span>
				<span class="ps-sidebar-word">Limusic</span>
				<span class="ps-sidebar-badge">BETA</span>
			</button>

			<nav class="ps-sidebar-nav">
				{#each navItems as item (item.id)}
					<button
						class="ps-sidebar-btn {view === item.id ? 'on' : ''}"
						onclick={() => go(item.id)}
						aria-label={item.label}
						aria-current={view === item.id ? 'page' : undefined}
						title={item.label}
					>
						<HugeiconsIcon icon={item.icon} />
						<span class="ps-sidebar-label">{item.label}</span>
					</button>
				{/each}
			</nav>

			<div class="ps-sidebar-foot">
				<button
					class="ps-sidebar-btn {view === 'now' ? 'on' : ''} {playback.now ? 'has-track' : ''}"
					onclick={() => go('now')}
					aria-label="Now Playing"
					title="Now Playing"
				>
					<div class="ps-sidebar-disc" class:spin={!playback.paused && !!playback.now}>
						{#if playback.now?.thumbnail}
							<img src={playback.now.thumbnail} alt="" />
						{:else}
							<div class="ps-sidebar-disc-fallback"></div>
						{/if}
					</div>
					<span class="ps-sidebar-label">Playing</span>
				</button>
				<button
					class="ps-sidebar-btn"
					onclick={(e) => { e.stopPropagation(); settingsOpen = !settingsOpen; }}
					aria-label="Pool settings"
					title="Pool settings"
				>
					<HugeiconsIcon icon={Settings01Icon} />
					<span class="ps-sidebar-label">Settings</span>
				</button>
			</div>
		</aside>

		<!-- ============================================================
		     MAIN COLUMN — one view at a time, scrollable
		     ============================================================ -->
		<main class="ps-main">
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
					<div class="ps-scroll-area">
						<LibraryView
							albums={mergedAlbums}
							{songs}
							singles={localSingles}
							onOpenNow={() => go('now')}
							onOpenAlbum={openAlbum}
							onPlayLocalAlbum={playAlbum}
							onPlaySong={playSongInList}
							onImport={importFolder}
						/>
					</div>
				</div>
				<div class="ps-view" class:on={view === 'library-carousel'}>
					<CoverFlowCarousel
						albums={mergedAlbums}
						{artFor}
						onOpenAlbum={openAlbum}
						onPlayAlbum={playAlbum}
					/>
				</div>
				<div class="ps-view" class:on={view === 'radio'}>
					<div class="ps-scroll-area">
						<RadioBrowse />
					</div>
				</div>
				<div class="ps-view" class:on={view === 'radio-now'}>
					<RadioNowPlaying station={currentStation} />
				</div>
				<div class="ps-view" class:on={view === 'history'}>
					<div class="ps-scroll-area">
						<HistoryView />
					</div>
				</div>
				<div class="ps-view" class:on={view === 'queue'}>
					<div class="ps-scroll-area">
						<QueueView />
					</div>
				</div>
				<div class="ps-view" class:on={view === 'now'}>
					<NowView onOpenLibrary={() => go('library')} />
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

			<!-- Mini player sits at the bottom of the main column, not floating in a
			     corner. It auto-hides on the Now view to avoid double-decking. -->
			{#if view !== 'now'}
				<div class="ps-mini-wrap">
					<MiniPlayer onOpenNow={() => go('now')} />
				</div>
			{/if}
		</main>

		<!-- ============================================================
			     LYRICS DRAWER — right side, displaces main content. Closes via
			     the X button or by toggling the sidebar's "Playing" button.
			     The ⤢ button expands into a fullscreen sing takeover (ps-sing),
			     which hides the sidebar too — pure lyrics, whole window.
			     ============================================================ -->
				{#if lyricsOpen && playback.now && !sing}
					<aside class="ps-lyrics-drawer" aria-label="Lyrics">
						<button class="ps-drawer-close" onclick={() => (lyricsOpen = false)} aria-label="Close lyrics">✕</button>
						<button
							class="ps-drawer-close"
							style="right: 52px"
							onclick={() => (sing = true)}
							aria-label="Fullscreen lyrics"
							title="Fullscreen lyrics"
						>⤢</button>
						<LyricsView expanded />
					</aside>
				{/if}

				<!-- Fullscreen sing takeover: over everything poolside paints, Esc or ✕ exits. -->
				{#if sing && playback.now}
					<div
						class="ps-sing ps-glass"
						role="dialog"
						aria-label="Fullscreen lyrics"
						onkeydown={(e) => e.key === 'Escape' && (sing = false)}
					>
						<button class="ps-drawer-close" onclick={() => (sing = false)} aria-label="Exit fullscreen lyrics">✕</button>
						<LyricsView expanded sing />
					</div>
				{/if}

		<!-- ============================================================
		     SETTINGS PANEL — right side, separate slot from lyrics. Both
		     can theoretically be open but they're in different columns.
		     ============================================================ -->
		{#if settingsOpen}
			<div class="ps-settings-panel ps-glass" role="dialog" aria-label="Pool settings" aria-modal="false">
				<header class="ps-settings-head">
					<h3>POOL SETTINGS</h3>
					<button class="ps-drawer-close" onclick={() => (settingsOpen = false)} aria-label="Close settings">✕</button>
				</header>
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
				<div class="ps-setrow">
					<span>APP SETTINGS</span>
					<button class="ps-setbtn" onclick={() => { ui.settingsOpen = true; settingsOpen = false; }} aria-label="Open full settings">
						OPEN
					</button>
				</div>
				<div class="ps-setrow ps-setrow--exit">
					<span>EXIT BETA</span>
					<button class="ps-setbtn ps-setbtn--exit" onclick={() => { applyLayout('default'); toast.info('Exited Poolside — back to Default layout'); }} aria-label="Exit Poolside beta">
						EXIT
					</button>
				</div>
				<div class="ps-setfoot">LIMUSIC · POOLSIDE VINYL BETA</div>
			</div>
		{/if}
	</div>

	<!-- ============================================================
	     CENTERED MODALS — custom CD cover (and anything else later).
	     ============================================================ -->
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

				<!-- ============================================================
				MINI PLAYER PILL — floating transport chip fixed to the bottom-center.
				Always visible (even on the Now view) so the user has transport controls
				anywhere in the app, but doesn't sit on top of the Now deck's own transport.
				Hides on the coverflow view so the user can see the covers unobstructed. -->
				{#if playback.now && view !== 'library-carousel'}
				<MiniPlayerPill onOpenNow={() => go('now')} />
				{/if}

				<!-- ============================================================
				     FEATURE CALLOUTS — first-time-visit announcements for the new sections.
				     A glowing red serif-font banner on a soft halo, fades in on view change
				     and out after 4s. The seen-set is persisted by the effect that creates
				     the callout, so we just pass `seen` to tell the child not to re-fire
				     if the user has already seen this view's callout.
				     The child takes `text` and `sectionId` (not `message`/`view`).
				-->
				{#if callout}
					<FeatureCallout
						text={callout.text}
						sectionId={callout.sectionId}
					/>
				{/if}
