<script lang="ts">
	// Poolside Vinyl shell (beta): a full-app reskin — animated pool water, skeuomorphic
	// picture-discs, coverflow library. Routes internally between NOW / LIBRARY / ALBUM views
	// with camera-dolly transitions; all playback goes through the same stores/commands as the
	// classic shell. Settings (gear) and the Ctrl+K palette remain the escape hatches; the
	// EXIT BETA chip returns to the Default layout.
	import { onMount } from 'svelte';
	import { fade, scale } from 'svelte/transition';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		Settings01Icon,
		Sun01Icon,
		Moon02Icon,
		Mic01Icon,
		Logout01Icon,
		Search01Icon
	} from '@hugeicons/core-free-icons';
	import * as api from '$lib/api';
	import type { BrowseItem, SongItem } from '$lib/api';
	import {
		auth,
		local,
		playback,
		playFrom,
		scanLocal,
		ui,
		toast
	} from '$lib/player.svelte';
	import { applyLayout } from '$lib/theme.svelte';
	import Water from './Water.svelte';
	import Vinyl from './Vinyl.svelte';
	import NowView from './NowView.svelte';
	import LibraryView from './LibraryView.svelte';
	import AlbumView from './AlbumView.svelte';
	import MiniPlayer from './MiniPlayer.svelte';
	import LyricsView from '../LyricsView.svelte';

	type View = 'now' | 'library' | 'album';
	let view = $state<View>('now');
	let dusk = $state(localStorage.getItem('ps-dusk') === 'true');
	let lyricsOpen = $state(false);
	let ccOpen = $state(false);
	let ccAlbum = $state<BrowseItem | null>(null);
	// album id -> data URL printed onto the disc (localStorage, poolside-local)
	let covers = $state<Record<string, string>>({});

	let albums = $state<BrowseItem[]>([]);
	let album = $state<BrowseItem | null>(null);
	let albumsLoaded = $state(false);

	let ccFileInput: HTMLInputElement | undefined = $state();

	function artFor(item: BrowseItem): string {
		return covers[item.id] ?? item.thumbnail ?? '';
	}

	function saveCovers() {
		try {
			localStorage.setItem('ps-covers', JSON.stringify(covers));
		} catch {
			/* quota — covers are decorative */
		}
	}

	function go(v: View) {
		view = v;
	}

	// Load the library albums once signed in; local album tiles are merged in LibraryView.
	$effect(() => {
		if (auth.account?.signedIn && !albumsLoaded) {
			albumsLoaded = true;
			api
				.getLibraryAlbums()
				.then((a) => (albums = a))
				.catch(() => {});
		}
	});

	// Album detail: pick a real album from the grid; the fan shows the rest of the collection.
	function openAlbum(item: BrowseItem) {
		if (item.id.startsWith('LOCALALBUM:')) {
			// local albums play directly from the grid — no detail view in beta
			return;
		}
		album = item;
		go('album');
	}

	function playAlbum(item: BrowseItem) {
		api
			.getAlbum(item.id)
			.then((alb) => {
				if (!alb.items.length) {
					toast.error('This album has no playable tracks');
					return;
				}
				playFrom(item, alb.items, 0, alb.playlistId ?? undefined, undefined, alb.continuation);
				go('now');
			})
			.catch((e) => toast.error(String(e)));
	}

	function openCustomCover() {
		ccAlbum = album ?? albums[0] ?? null;
		if (!ccAlbum) {
			toast.error('Open an album first');
			return;
		}
		ccOpen = true;
	}

	function onCcFile(e: Event) {
		const input = e.currentTarget as HTMLInputElement;
		const f = input.files?.[0];
		if (!f || !ccAlbum) return;
		const rd = new FileReader();
		rd.onload = () => {
			covers = { ...covers, [ccAlbum!.id]: String(rd.result) };
			saveCovers();
			toast.success('Cover printed onto the disc');
			ccOpen = false;
		};
		rd.readAsDataURL(f);
		input.value = '';
	}

	function resetCover() {
		if (!ccAlbum) return;
		const next = { ...covers };
		delete next[ccAlbum.id];
		covers = next;
		saveCovers();
		toast.success('Reset to printed art');
	}

	function toggleDusk() {
		dusk = !dusk;
		localStorage.setItem('ps-dusk', String(dusk));
	}

	onMount(() => {
		// local folders feed the FOLDERS tab + local album tiles
		scanLocalSafe();
	});
	async function scanLocalSafe() {
		try {
			await scanLocal();
		} catch {
			/* beta: local tab just stays empty */
		}
	}

	// keep a SongItem shape for the local album tiles the LibraryView emits
	function isLocalAlbum(item: BrowseItem): boolean {
		return item.id.startsWith('LOCALALBUM:');
	}
	function localTracksFor(item: BrowseItem): SongItem[] {
		return local.songs.filter((s) => (s.album || 'Unknown Album') === item.title);
	}
</script>

<div class="ps-root {dusk ? 'dusk' : ''}">
	<Water />

	<!-- NOW PLAYING -->
	{#if view === 'now'}
		<div in:fade={{ duration: 300 }} class="absolute inset-0">
			<NowView onOpenLibrary={() => go('library')} />
		</div>
	<!-- LIBRARY -->
	{:else if view === 'library'}
		<div in:fade={{ duration: 300 }} class="absolute inset-0">
			<LibraryView
				onOpenNow={() => go('now')}
				onOpenAlbum={(item) => {
					if (isLocalAlbum(item)) {
						const tracks = localTracksFor(item);
						if (tracks.length) playFrom(item, tracks, 0);
						else toast.error('No tracks found for this album');
						return;
					}
					openAlbum(item);
				}}
			/>
		</div>
	<!-- ALBUM DETAIL -->
	{:else if view === 'album' && album}
		<div in:fade={{ duration: 300 }} class="absolute inset-0">
			<AlbumView
				{albums}
				album={album}
				{artFor}
				onBack={() => go('library')}
				onPlayAlbum={playAlbum}
				onOpenCustom={openCustomCover}
			/>
		</div>
	{/if}

	<!-- mini player -->
	<MiniPlayer onOpenNow={() => go('now')} />

	<!-- edge buttons -->
	<div class="ps-edge mid">
		<button class="ps-edge-btn" onclick={() => (ui.settingsOpen = true)} title="Settings" aria-label="Settings">
			<HugeiconsIcon icon={Settings01Icon} />
		</button>
		<button
			class="ps-edge-btn"
			onclick={toggleDusk}
			title={dusk ? 'Switch to day pool' : 'Switch to dusk pool'}
			aria-label="Toggle dusk"
		>
			<HugeiconsIcon icon={dusk ? Moon02Icon : Sun01Icon} />
		</button>
		{#if playback.now}
			<button
				class="ps-edge-btn"
				onclick={() => (lyricsOpen = !lyricsOpen)}
				title="Lyrics"
				aria-label="Lyrics"
			>
				<HugeiconsIcon icon={Mic01Icon} />
			</button>
		{/if}
		<button
			class="ps-edge-btn ps-exit"
			onclick={() => {
				applyLayout('default');
				toast.info('Exited Poolside — back to Default layout');
			}}
			title="Exit Poolside beta"
		>
			<HugeiconsIcon icon={Logout01Icon} class="w-3.5 h-3.5" />
			Exit beta
		</button>
	</div>

	<!-- search shortcut: Ctrl+K palette still searches YouTube; add a visible hint button top-left -->
	<button
		class="ps-edge-btn absolute left-6 top-6 flex items-center gap-2 w-auto px-4"
		onclick={() => (ui.paletteOpen = true)}
		title="Search (Ctrl+K)"
	>
		<HugeiconsIcon icon={Search01Icon} />
		<span class="text-[8px] font-bold tracking-[0.2em] uppercase">Search</span>
	</button>

	<!-- lyrics overlay -->
	{#if lyricsOpen && playback.now}
		<div
			class="ps-lyrics-frame"
			transition:fade={{ duration: 220 }}
			role="dialog"
			aria-label="Lyrics"
		>
			<button class="ps-lyrics-close" onclick={() => (lyricsOpen = false)} aria-label="Close lyrics">✕</button>
			<LyricsView expanded />
		</div>
	{/if}

	<!-- custom CD cover overlay -->
	{#if ccOpen}
		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
		<div
			class="ps-overlay open"
			role="dialog"
			aria-modal="true"
			aria-label="Add custom CD covers"
			tabindex="-1"
			onclick={(e) => {
				if (e.target === e.currentTarget) ccOpen = false;
			}}
			onkeydown={(e) => e.key === 'Escape' && (ccOpen = false)}
			transition:fade={{ duration: 200 }}
		>
			<div class="ps-card text-center" in:scale={{ start: 0.96, duration: 260 }}>
				<button
					class="absolute right-4 top-3 text-lg opacity-80 hover:opacity-100"
					onclick={() => (ccOpen = false)}
					aria-label="Close"
				>
					✕
				</button>
				<h2 class="ps-serif-big">Add Custom CD Covers!</h2>
				<p class="sub">Print your own art onto the picture disc for<br />“{ccAlbum?.title}”</p>
				<div class="my-6 flex items-center justify-center gap-6">
					<div style="width:140px">
						<Vinyl src={ccAlbum ? artFor(ccAlbum) : ''} playing={false} style="width:100%" />
					</div>
					<div class="flex flex-col items-start gap-3">
						<button class="ps-aqua px-4 py-2.5 text-[9px]" onclick={() => ccFileInput?.click()}>
							Choose image…
						</button>
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
