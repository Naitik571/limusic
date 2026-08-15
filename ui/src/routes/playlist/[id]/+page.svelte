<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { HugeiconsIcon } from '@hugeicons/svelte';
	import {
		PlayIcon,
		ShuffleIcon,
		PencilEdit02Icon,
		Delete02Icon,
		MoreVerticalIcon,
		Tick02Icon,
		Cancel01Icon,
		Radio02Icon,
		ArrowUpNarrowWideIcon,
		ArrowDownWideNarrowIcon,
		DashboardSquare02Icon,
		ListRestartIcon,
		Playlist02Icon,
		Move01Icon,
		Download01Icon,
		PlusSignIcon,
		RefreshIcon,
		ArrowDownAZIcon
	} from '@hugeicons/core-free-icons';
	import { Button } from '$lib/components/ui/button';
	import { Skeleton } from '$lib/components/ui/skeleton';
	import * as RadioGroup from '$lib/components/ui/radio-group';
	import TrackRow from '$lib/components/TrackRow.svelte';
	import TrackRowSkeleton from '$lib/components/TrackRowSkeleton.svelte';
	import ErrorState from '$lib/components/ErrorState.svelte';
	import * as api from '$lib/api';
	import { ON_REPEAT_ID } from '$lib/api';
	import { SORTS, sortSongs, type SortKey } from '$lib/sort';
	import type { BrowseItem, PlaylistPage, SongItem } from '$lib/api';
	import { getCached, putCached, invalidateCached } from '$lib/pagecache';
	import { anchorMenu } from '$lib/menu';
	import {
		addPick,
		enqueue,
		playback,
		openAddToPlaylist,
		playFrom,
		startRadio,
		toast,
		bumpLibraryTrackCount,
		lastPlaylistAdd,
		lastPlaylistMove,
		openAddManyToPlaylist,
		openMoveToPlaylist,
		playSong
	} from '$lib/player.svelte';

	let pl = $state<PlaylistPage | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let loadingMore = $state(false);
	let moreError = $state(false);
	let inflight: Promise<void> | null = null;
	let confirmingDelete = $state(false);
	// A random song's cover, used as a blurred hero backdrop (like the artist/album pages).
	let bgImage = $state<string | null>(null);

	// ⋯ options menu, positioned `fixed` at the button so it isn't clipped (matches TrackRow).
	let menuOpen = $state(false);
	let mx = $state(0);
	let my = $state(0);

	// Inline rename state.
	let editingName = $state(false);
	let nameDraft = $state('');

	const id = $derived(page.params.id ?? '');
	const nowId = $derived(playback.now?.videoId);
	// The liked-music auto-playlist isn't a user playlist — no rename/delete, but shuffle is fine.
	const isLiked = $derived(id === 'VLLM');
	// On Repeat is built locally from play counts: no artwork, and no radio to seed autoplay from.
	const isOnRepeat = $derived(id === ON_REPEAT_ID);
	// Only offer rename/delete on playlists the signed-in user actually owns (backend `owned` flag).
	// Liked Music reports owned but can't be renamed/deleted, so exclude it explicitly.
	const editable = $derived((pl?.owned ?? false) && !isLiked);

	// ——— Sorting (the original repo's sorter: a view over `pl.items`) ————————————————
	// `pl.items` stays in YouTube's order; every optimistic mutation (add, remove, setVideoId
	// backfill, loadMore) works against the real list, and switching back to Default costs nothing.
	// The sort is a derived view on top, so it is never written back and never needs undoing.
	let sort = $state<SortKey>('default');
	let desc = $state(false);
	let sortOpen = $state(false);
	let sx = $state(0);
	let sy = $state(0);
	let sortUp = $state(false);
	const sortLabel = $derived(
		sort === 'default' ? 'Sort' : (SORTS.find((s) => s.key === sort)?.label ?? 'Sort')
	);

	// "Most played" sorts against local listening history. It's a SQLite read the other six sorts
	// don't need, so fetch it only when it's first asked for.
	let plays = $state<Record<string, number>>({});
	let playsInflight: Promise<void> | null = null;
	function loadPlays(): Promise<void> {
		playsInflight ??= api
			.getPlayCounts()
			.then((c) => void (plays = c))
			// An empty map just sorts everything as unplayed, which beats blocking the sort.
			.catch(() => {});
		return playsInflight;
	}

	// Liked Music is the one playlist YouTube hands back newest-addition-first.
	const sortedItems = $derived(sortSongs(pl?.items ?? [], sort, isLiked, desc, plays));

	// The rows actually on screen: the sorted view of whatever pages are loaded.
	const shown = $derived(sortedItems);
	const sorting = $derived(sort !== 'default' || desc);

	// A sort has to cover the whole playlist, not the pages scrolled so far, so pull the rest in
	// before play/queue hand a short list to the queue. Stops on a failed page (`moreError`), on
	// navigation, and on any walk that made no progress. Answers whether it got the lot.
	async function loadAll(): Promise<boolean> {
		const pid = id;
		moreError = false; // an explicit action gets another go at a page the scroll gave up on
		while (pl?.continuation && !moreError) {
			const token = pl.continuation;
			await loadMore();
			if (pid !== id) return false;
			if (pl?.continuation === token) break; // no progress, nothing left to walk
		}
		return !pl?.continuation;
	}
	async function ready(): Promise<boolean> {
		if (!sorting) return true;
		if (sort === 'plays') await loadPlays(); // queueing before the counts land sorts by nothing
		if (!pl?.continuation) return true;
		return loadAll();
	}
	// A sorted queue is built from whatever loaded; a page that failed is missing, and can't be
	// fixed later — say so instead of silently handing over half a playlist.
	function warnPartial(what: string) {
		toast.error(`Couldn't load all of this playlist, so only what loaded was ${what}.`);
	}
	function chooseSort(key: SortKey) {
		sortOpen = false;
		if (key === sort) return;
		sort = key;
		desc = false;
		if (key === 'plays') loadPlays(); // the list re-sorts itself when the counts land
		if (sorting) loadAll(); // start the walk now so Play rarely has to wait
	}
	function toggleDesc() {
		desc = !desc;
		if (sorting) loadAll();
	}
	// Right-anchored like the other header menus: the trigger sits at the far end of the header, so
	// a menu wider than it would run off the page opening leftwards from its left edge.
	function openSort(e: MouseEvent) {
		({ right: sx, y: sy, openUp: sortUp } = anchorMenu(e.currentTarget as HTMLElement, 300));
		sortOpen = true;
	}

	async function downloadPlaylistHere() {
		if (!pl) return;
		toast('Downloading playlist…');
		api.downloadPlaylist(id)
			.then((r) => toast.success(`Queued ${r.total} track${r.total === 1 ? '' : 's'} for download`))
			.catch((e) => toast.error(`Download failed: ${e}`));
	}

	async function load(pid: string) {
		const key = `playlist:${pid}`;
		const hit = getCached<PlaylistPage>(key);
		confirmingDelete = false;
		editingName = false;
		if (hit) {
			pl = hit;
			bgImage = pickCover(hit.items);
			loading = false;
		} else {
			loading = true;
			pl = null;
			bgImage = null;
		}
		error = null;
		try {
			const fresh = await api.getPlaylist(pid);
			if (pid !== id) return; // superseded by navigation — drop the stale response
			pl = fresh;
			bgImage = pickCover(fresh.items);
			putCached(key, fresh);
		} catch (e) {
			if (pid !== id) return;
			if (!hit) error = String(e);
		} finally {
			if (pid === id) loading = false;
		}
	}

	// Reload whenever the route param changes (playlist → playlist navigation).
	$effect(() => {
		if (id) load(id);
	});

	// ——— "More like this" ——————————————————————————————————————————
	// Once a playlist has a couple of songs, seed a radio on the first one (the same read-only
	// watch/radio endpoint behind autoplay) and offer its tracks as one-tap additions. The shelf
	// only makes sense where adding works: playlists the user owns, plus Liked Music. Cached per
	// (playlist, seed) so revisits don't refetch.
	let recs = $state<SongItem[] | null>(null);
	let recPool = $state<SongItem[]>([]); // fetched backlog — tops the shelf up after an add
	let recsCache = new Map<string, SongItem[]>();
	let recSeedIdx = $state(0); // which playlist track seeded the current batch
	let recRefreshKey = $state(0); // bump to fetch a fresh batch (the shelf's refresh button)
	let recRefreshing = $state(false);
	let recsLoadedFor = ''; // `${id}:${recRefreshKey}` — set once a batch has been applied

	// Dedupe against what's already in the playlist at apply time, so a song that just landed
	// (optimistic add, backfill, picker) never shows up here as a suggestion.
	function applyRecs(batch: SongItem[]) {
		const known = new Set((pl?.items ?? []).map((t) => t.video_id));
		const fresh = batch.filter((s) => !known.has(s.video_id));
		recs = fresh.slice(0, 8); // 8 on the shelf, the rest wait in the pool for top-ups
		recPool = fresh.slice(8);
	}

	// Loads on playlist change or refresh. The marker guards against the effect re-firing when
	// the playlist items grow (every add re-runs it) — loaded once per (playlist, refresh).
	$effect(() => {
		id; // navigation to another playlist resets the shelf before its fetch effect runs
		recRefreshKey;
		const marker = `${id}:${recRefreshKey}`;
		if (recsLoadedFor === marker) return;
		const seed = pl?.items[recSeedIdx] ?? pl?.items[0];
		if (!pl || isOnRepeat || !(editable || isLiked)) {
			recs = null;
			return;
		}
		if (!seed?.video_id || pl.items.length < 2) return; // wait for "a couple of songs"
		const key = `${id}:${seed.video_id}`;
		const hit = recsCache.get(key);
		if (hit) {
			recsLoadedFor = marker;
			applyRecs(hit);
			return;
		}
		recRefreshing = true;
		api
			.getSimilarSongs(seed.video_id, 16) // 16: 8 on the shelf, 8 in the pool for top-ups
			.then((r) => {
				recsCache.set(key, r);
				recsLoadedFor = marker;
				applyRecs(r);
			})
			.catch(() => (recs = [])) // quiet: the shelf just doesn't show
			.finally(() => (recRefreshing = false));
	});

	// Fresh batch seeded from the next playlist track, so "Find more like this" actually finds
	// different music (and never the same batch twice in a row).
	function refreshRecs() {
		const n = pl?.items.length ?? 1;
		if (!pl || n < 2) return;
		recSeedIdx = (recSeedIdx + 1) % n;
		recRefreshKey++;
	}

	// One-tap add. Optimistic append, reverted on failure — same contract as the picker, except
	// rows land without set_video_id and the 0/2/4s backfill in `load` patches them in.
	// One-tap add. Optimistic append, reverted on failure — same contract as the picker, except
	// rows land without set_video_id and the 0/2/4s backfill in `load` patches them in. On
	// success the added song leaves the shelf and a fresh one takes its place (from the pool,
	// or a radio seeded on the song just added), so the shelf never repeats and stays full.
	async function addRec(rec: SongItem) {
		if (!pl) return;
		if (isLiked) {
			pl = { ...pl, items: [...pl.items, rec] };
			try {
				await api.like(rec.video_id, true);
				cacheCurrent();
				toast.success('Added to Liked Music');
			} catch (e) {
				pl = { ...pl, items: pl.items.filter((t) => t.video_id !== rec.video_id) };
				cacheCurrent();
				toast.error(String(e));
			}
			return;
		}
		pl = { ...pl, items: [...pl.items, rec] };
		bumpLibraryTrackCount(id, 1);
		try {
			await api.addToPlaylist(id, rec.video_id);
			cacheCurrent();
			toast.success('Added to playlist');
		} catch (e) {
			pl = { ...pl, items: pl.items.filter((t) => t.video_id !== rec.video_id) };
			bumpLibraryTrackCount(id, -1);
			cacheCurrent();
			toast.error(String(e));
		}
		topUpRecs(rec);
	}

	async function topUpRecs(added: SongItem) {
		if (!recs) return;
		const known = new Set((pl?.items ?? []).map((t) => t.video_id));
		const rest = recs.filter((s) => s.video_id !== added.video_id);
		const next = recPool.find((s) => !known.has(s.video_id));
		if (next) {
			recs = [...rest, next];
			recPool = recPool.filter((s) => s.video_id !== next.video_id);
			return;
		}
		try {
			// Pool exhausted — seed a radio on the song that was just added: "more like the one
			// you liked", which is what a shelf should lean toward.
			const batch = await api.getSimilarSongs(added.video_id, 16);
			recsCache.set(`${id}:${added.video_id}`, batch);
			const fresh = batch.filter((s) => !known.has(s.video_id));
			recs = fresh.length ? [...rest, fresh[0]] : rest;
			recPool = fresh.slice(1);
		} catch {
			recs = rest; // keep the shelf as it is, minus the added song
		}
	}

	// Songs MOVED out of this playlist via the picker vanish immediately (the picker did the
	// server-side removal; the row drop is the mirror of the add-appends below).
	let seenMoveEpoch = lastPlaylistMove.epoch;
	$effect(() => {
		if (lastPlaylistMove.epoch === seenMoveEpoch) return;
		seenMoveEpoch = lastPlaylistMove.epoch;
		if (!pl || lastPlaylistMove.fromId !== id) return;
		const gone = new Set(lastPlaylistMove.songs.map(selKey));
		pl = { ...pl, items: pl.items.filter((t) => !gone.has(selKey(t))) };
		cacheCurrent();
	});

	// Songs added to THIS playlist via the picker (e.g. from the queue) appear immediately.
	// Epoch-guarded so an add is applied once; adds to other playlists are just marked seen.
	let seenAddEpoch = lastPlaylistAdd.epoch;
	$effect(() => {
		if (lastPlaylistAdd.epoch === seenAddEpoch) return;
		seenAddEpoch = lastPlaylistAdd.epoch;
		if (!pl || lastPlaylistAdd.playlistId !== id) return;
		pl = { ...pl, items: [...pl.items, ...lastPlaylistAdd.songs] };
		cacheCurrent();
		fillSetVideoIds();
	});

	// Optimistic rows lack set_video_id, so "Remove from playlist" is hidden on them. Refetch and
	// patch the real ids into place (merge, not replace — keeps loadMore pages and any row YouTube
	// hasn't reflected yet). Retries because the add is eventually-consistent on YouTube's side.
	async function fillSetVideoIds() {
		if (isLiked) return;
		const pid = id;
		for (const delay of [0, 2000, 4000]) {
			if (delay) await new Promise((r) => setTimeout(r, delay));
			if (pid !== id || !pl) return;
			try {
				const fresh = await api.getPlaylist(pid);
				if (pid !== id || !pl) return;
				const used = new Set(pl.items.map((t) => t.set_video_id).filter(Boolean));
				pl = {
					...pl,
					subtitle: fresh.subtitle, // header track count catches up too
					items: pl.items.map((t) => {
						if (t.set_video_id) return t;
						const match = fresh.items.find(
							(f) => f.video_id === t.video_id && f.set_video_id && !used.has(f.set_video_id)
						);
						if (!match) return t;
						used.add(match.set_video_id);
						return { ...t, set_video_id: match.set_video_id };
					})
				};
				cacheCurrent();
				if (pl.items.every((t) => t.set_video_id)) return;
			} catch {
				/* retry on the next pass */
			}
		}
	}

	// Keep the page cache in step with optimistic mutations so a revisit within the TTL never
	// resurrects pre-mutation data (the optimistic-UI contract). context: plans/007.
	function cacheCurrent() {
		if (pl) putCached(`playlist:${id}`, pl);
	}

	// One page at a time, shared: the scroll sentinel and the "load the rest before playing" walk
	// both go through here, so they can never fire overlapping requests for the same token.
	function loadMore(): Promise<void> {
		inflight ??= fetchPage().finally(() => (inflight = null));
		return inflight;
	}

	async function fetchPage() {
		const token = pl?.continuation;
		if (!token) return;
		loadingMore = true;
		moreError = false;
		try {
			const more = await api.getPlaylistMore(token);
			if (pl?.continuation !== token) return; // stale (navigated or mutated mid-flight)
			pl = {
				...pl,
				items: [...pl.items, ...more.items],
				// An empty page would leave the sentinel in view with nothing to show — that's the end.
				continuation: more.items.length ? more.continuation : undefined
			};
			cacheCurrent();
		} catch {
			// Stop auto-loading and offer a retry — auto-retrying a visible sentinel would spin.
			moreError = true;
		} finally {
			loadingMore = false;
		}
	}

	// One page per approach to the bottom: the observer only fires when the sentinel *enters* view,
	// so an appended page that pushes it back out is required before the next fetch. rootMargin
	// starts the fetch early enough that the rows are usually there by the time you reach them.
	function sentinel(node: HTMLElement) {
		const io = new IntersectionObserver(([e]) => e.isIntersecting && loadMore(), {
			rootMargin: '600px 0px'
		});
		io.observe(node);
		return () => io.disconnect();
	}

	// This playlist as a card, for the sidebar's last-played sort and the Shortcuts grid.
	const asItem = (): BrowseItem => ({
		kind: 'playlist',
		id,
		title: pl?.title ?? 'Playlist',
		subtitle: pl?.subtitle,
		// On Repeat stays artwork-free wherever it's rendered (shortcuts, recents) so it always
		// draws its icon rather than one of its songs' covers.
		thumbnail: isOnRepeat ? undefined : (pl?.thumbnail ?? bgImage ?? undefined)
	});

	// `sourceId` points autoplay at that playlist's radio. On Repeat has no YouTube id, so pass
	// none and let autoplay seed off the last video instead. The queue is the whole playlist, not
	// the pages scrolled so far, but waiting for it here is what made long playlists take forever
	// to start: YouTube hands out tracks 100 at a time and the tokens are chained, so the backend
	// takes the token and walks the rest into the queue while page 1 is already playing.
	async function playAll(start: number | null) {
		if (!pl) return;
		const pid = id;
		const whole = await ready();
		if (!pl || pid !== id) return;
		const pick = start === null ? null : shown[start];
		const at = pick ? sortedItems.indexOf(pick) : -1;
		if (!whole) warnPartial('played');
		playFrom(
			asItem(),
			sortedItems,
			at >= 0 ? at : null,
			isOnRepeat ? undefined : id,
			undefined,
			sorting ? undefined : pl.continuation
		);
	}

	// Random cover from the songs, picked once per load so it stays stable while browsing
	// (loadMore appends tracks without changing it).
	function pickCover(items: SongItem[]): string | null {
		const withThumb = items.filter((t) => t.thumbnail);
		if (!withThumb.length) return null;
		const url = withThumb[Math.floor(Math.random() * withThumb.length)].thumbnail!;
		return hiRes(url);
	}

	// List thumbnails come at a small size; YouTube/Google encode the size in the URL, so bump it
	// for a crisp full-width backdrop.
	function hiRes(url: string): string {
		return url.replace(/=w\d+-h\d+/, '=w1200-h1200').replace(/=s\d+/, '=s1200');
	}

	// Same deal as `playAll` for a long playlist: the loaded pages go in now and the token hands
	// the rest to the backend to walk in behind them.
	async function queue(next: boolean) {
		if (!pl?.items.length) return;
		const pid = id;
		const whole = await ready();
		if (!pl || pid !== id) return;
		if (!whole) warnPartial('queued');
		enqueue(sortedItems, next, pl.title, sorting ? undefined : pl.continuation);
	}

	function shufflePlay() {
		if (!pl?.items.length) return;
		// Real order + shuffle flag — the backend owns shuffling, so the shuffle toggle can
		// restore the true playlist order and every re-shuffle is fresh. It also mixes each page
		// it walks into the unplayed tail, so this stays a shuffle of the whole playlist rather
		// than of the pages that happen to be loaded.
		playFrom(asItem(), pl.items, null, isOnRepeat ? undefined : id, true, pl.continuation);
	}

	function openMenu(e: MouseEvent) {
		const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
		mx = r.left;
		my = r.bottom + 4;
		menuOpen = true;
	}
	function run(action: () => void) {
		menuOpen = false;
		action();
	}

	function startRename() {
		nameDraft = pl?.title ?? '';
		editingName = true;
	}

	async function saveRename() {
		const name = nameDraft.trim();
		if (!pl || !name || name === pl.title) {
			editingName = false;
			return;
		}
		const prev = pl.title;
		pl = { ...pl, title: name }; // optimistic
		editingName = false;
		try {
			await api.renamePlaylist(id, name);
			cacheCurrent();
			toast.success('Playlist renamed');
		} catch (e) {
			pl = { ...pl, title: prev }; // revert
			cacheCurrent();
			toast.error(String(e));
		}
	}

	// The liked-music auto-playlist can't be edited like a normal one — removing = un-liking.
	async function removeTrack(track: SongItem) {
		if (!pl) return;
		if (!isLiked && !track.set_video_id) return;
		const prev = pl.items;
		// Reassign `pl` (not mutate `pl.items`) so the list re-renders immediately. Match by the
		// per-instance setVideoId on normal playlists (duplicates), by videoId on liked music.
		const kept = pl.items.filter((t) =>
			isLiked ? t.video_id !== track.video_id : t.set_video_id !== track.set_video_id
		);
		pl = { ...pl, items: kept };
		try {
			if (isLiked) {
				await api.like(track.video_id, false);
				toast.success('Removed from Liked Music');
			} else {
				await api.removeFromPlaylist(id, track.video_id, track.set_video_id!);
				bumpLibraryTrackCount(id, -1);
				toast.success('Removed from playlist');
			}
			cacheCurrent();
		} catch (e) {
			pl = { ...pl, items: prev }; // revert
			cacheCurrent();
			toast.error(String(e));
		}
	}

	// --- Multi-select + right-click actions -------------------------------
	// Ctrl/Cmd-click toggles a row, Shift-click ranges from the anchor, a plain click plays and
	// clears the selection, right-click selects exactly the row under the cursor and opens the
	// action menu. Rows are keyed by set_video_id (unique per playlist row) so duplicates and
	// just-added optimistic rows don't collide; liked music falls back to video_id.
	const selKey = (t: SongItem) => t.set_video_id ?? t.video_id;
	let selected = $state<Set<string>>(new Set());
	let selAnchor = $state(-1);
	let selMenuOpen = $state(false);
	let selX = $state(0);
	let selY = $state(0);

	const selectedItems = $derived(pl?.items.filter((t) => selected.has(selKey(t))) ?? []);
	const canRemoveSel = $derived(
		selectedItems.length > 0 && selectedItems.every((t) => isLiked || (editable && t.set_video_id))
	);
	const canMoveSel = $derived(selectedItems.length > 0 && editable);

	function clearSelection() {
		selected = new Set();
		selAnchor = -1;
		selMenuOpen = false;
	}

	// Capture phase: modifier clicks select instead of playing; anything else plays (and, once
	// a selection exists, a plain click means "done with that" — clear it).
	function onRowClickCapture(e: MouseEvent, i: number) {
		if (!pl) return;
		const key = selKey(pl.items[i]);
		if (e.ctrlKey || e.metaKey || e.shiftKey) {
			e.preventDefault();
			e.stopPropagation();
			const next = new Set(selected);
			if (e.shiftKey && selAnchor >= 0) {
				const a = Math.min(selAnchor, i);
				const b = Math.max(selAnchor, i);
				if (!e.ctrlKey && !e.metaKey) next.clear(); // plain Shift replaces the selection
				for (let j = a; j <= b; j++) next.add(selKey(pl.items[j]));
			} else {
				if (next.has(key)) next.delete(key);
				else next.add(key);
				selAnchor = i;
			}
			selected = next;
			return;
		}
		if (selected.size) clearSelection();
	}

	function onRowContextMenu(e: MouseEvent, i: number) {
		if (!pl) return;
		e.preventDefault();
		const key = selKey(pl.items[i]);
		if (!selected.has(key)) {
			selected = new Set([key]); // right-click selects exactly this row
			selAnchor = i;
		}
		selX = e.clientX;
		selY = e.clientY;
		selMenuOpen = true;
	}

	function playSelected(items: SongItem[]) {
		if (!pl || !items.length) return;
		playFrom(asItem(), items, 0, isOnRepeat ? undefined : id);
	}
	function queueSelected(items: SongItem[]) {
		if (items.length) enqueue(items, false);
	}
	async function removeSelected(items: SongItem[]) {
		if (!pl || !items.length) return;
		const removable = items.filter((t) => isLiked || t.set_video_id);
		const keys = new Set(items.map(selKey));
		const prev = pl.items;
		pl = { ...pl, items: pl.items.filter((t) => !keys.has(selKey(t))) }; // optimistic
		clearSelection();
		try {
			for (const t of removable) {
				if (isLiked) await api.like(t.video_id, false);
				else await api.removeFromPlaylist(id, t.video_id, t.set_video_id!);
			}
			if (!isLiked) bumpLibraryTrackCount(id, -removable.length);
			toast.success(
				removable.length === 1
					? 'Removed from ' + (isLiked ? 'Liked Music' : 'playlist')
					: `Removed ${removable.length} songs`
			);
			cacheCurrent();
		} catch (e) {
			pl = { ...pl, items: prev }; // revert
			cacheCurrent();
			toast.error(String(e));
		}
	}

	// Toolbar/menu entry points. Play/Queue snap the selection first (they consume it); Add/Move/
	// Remove keep rows selected so a follow-up action is one click away.
	function doPlaySel() {
		const items = [...selectedItems];
		clearSelection();
		playSelected(items);
	}
	function doQueueSel() {
		const items = [...selectedItems];
		clearSelection();
		queueSelected(items);
	}
	function doAddSel() {
		openAddManyToPlaylist([...selectedItems]);
		selMenuOpen = false;
	}
	function doMoveSel() {
		openMoveToPlaylist([...selectedItems], id);
		selMenuOpen = false;
	}
	function doRemoveSel() {
		removeSelected([...selectedItems]); // clears internally
	}

	async function deleteThisPlaylist() {
		try {
			await api.deletePlaylist(id);
			invalidateCached(`playlist:${id}`);
			toast.success('Playlist deleted');
			goto('/library');
		} catch (e) {
			toast.error(String(e));
			confirmingDelete = false;
		}
	}

	function autofocus(node: HTMLInputElement) {
		node.focus();
		node.select();
	}
</script>

<svelte:window
	onkeydown={(e) => {
		if (e.key === 'Escape') {
			clearSelection();
			menuOpen = false;
		}
	}}
/>

<div class="flex h-full flex-col">
	{#if loading}
		<div class="flex items-end gap-6 border-b p-6">
			<Skeleton class="h-40 w-40 shrink-0 rounded-xl" />
			<div class="flex-1 space-y-3">
				<Skeleton class="h-3 w-16 rounded" />
				<Skeleton class="h-10 w-2/3 rounded-lg" />
				<Skeleton class="h-4 w-40 rounded" />
				<Skeleton class="h-9 w-24 rounded-4xl" />
			</div>
		</div>
		<div class="p-4">
			{#each Array(8) as _, i (i)}
				<TrackRowSkeleton />
			{/each}
		</div>
	{:else if error}
		<div class="p-6"><ErrorState message={error} onRetry={() => load(id)} /></div>
	{:else if pl}
		<div class="content-in relative flex min-h-[38vh] items-end gap-6 overflow-hidden border-b p-6">
			{#if bgImage}
				<img
					src={bgImage}
					alt=""
					class="pointer-events-none absolute inset-0 h-full w-full object-cover object-center"
				/>
			{/if}
			<!-- Fade the cover into the page so the text stays readable: solid at the bottom and on the
			     left (behind the title), the image itself visible toward the top-right. -->
			<div
				class="absolute inset-0 bg-gradient-to-t from-background via-background/60 to-background/20"
			></div>
			<div class="absolute inset-0 bg-gradient-to-r from-background via-background/50 to-transparent"></div>
			{#if isOnRepeat}
				<div
					class="relative flex h-40 w-40 items-center justify-center rounded-xl bg-primary/10 text-primary shadow-lg"
				>
					<HugeiconsIcon icon={ListRestartIcon} class="h-20 w-20" />
				</div>
			{:else if pl.thumbnail}
				<img
					src={pl.thumbnail}
					alt=""
					class="relative h-40 w-40 rounded-xl object-cover shadow-lg"
				/>
			{:else}
				<div class="relative h-40 w-40 rounded-xl bg-muted"></div>
			{/if}
			<div class="relative min-w-0 flex-1">
				<div class="text-xs font-medium uppercase text-muted-foreground">Playlist</div>
				{#if editingName}
					<div class="mt-1 flex items-center gap-2">
						<input
							use:autofocus
							bind:value={nameDraft}
							onkeydown={(e) => {
								if (e.key === 'Enter') saveRename();
								else if (e.key === 'Escape') (editingName = false);
							}}
							class="min-w-0 flex-1 rounded-md border bg-background px-2 py-1 font-heading text-3xl font-bold outline-none focus:border-accent"
							aria-label="Playlist name"
						/>
						<Button size="icon" aria-label="Save name" onclick={saveRename}>
							<HugeiconsIcon icon={Tick02Icon} class="h-5 w-5" />
						</Button>
						<Button
							variant="ghost"
							size="icon"
							aria-label="Cancel rename"
							onclick={() => (editingName = false)}
						>
							<HugeiconsIcon icon={Cancel01Icon} class="h-5 w-5 text-muted-foreground" />
						</Button>
					</div>
				{:else}
					<h1 class="text-gradient mt-1 font-heading text-4xl font-bold tracking-tight">
					{pl.title ?? 'Playlist'}
				</h1>
				{/if}
				{#if pl.subtitle}<p class="mt-2 text-sm text-muted-foreground">{pl.subtitle}</p>{/if}
				<div class="mt-4 flex items-center gap-2">
					<Button class="gap-2" onclick={() => playAll(null)} disabled={!pl.items.length}>
						<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" />
						Play
					</Button>
					{#if !isOnRepeat}
						<Button
							variant="outline"
							class="gap-2"
							onclick={downloadPlaylistHere}
							title="Download playlist for offline"
						>
							<HugeiconsIcon icon={Download01Icon} class="h-4 w-4" /> Download
						</Button>
					{/if}
					<Button
						variant="ghost"
						aria-label="Sort playlist"
						title={`Sort playlist${sort !== 'default' ? ` (${sortLabel})` : ''}`}
						onclick={openSort}
						class="gap-2"
					>
						<HugeiconsIcon icon={ArrowDownAZIcon} class="h-5 w-5 text-muted-foreground" />
						<span class="hidden text-sm md:inline">{sortLabel}</span>
					</Button>
					{#if sort !== 'default'}
						<Button
							variant="ghost"
							size="icon"
							aria-label="Reverse sort order"
							title={desc ? 'Sort ascending' : 'Sort descending'}
							onclick={toggleDesc}
						>
							<HugeiconsIcon
								icon={desc ? ArrowUpNarrowWideIcon : ArrowDownWideNarrowIcon}
								class="h-5 w-5 text-muted-foreground"
							/>
						</Button>
					{/if}
					{#if confirmingDelete}
						<div class="flex items-center gap-2 rounded-lg border border-destructive/40 px-2 py-1">
							<span class="text-xs text-muted-foreground">Delete this playlist?</span>
							<Button variant="destructive" size="sm" onclick={deleteThisPlaylist}>Delete</Button>
							<Button variant="ghost" size="sm" onclick={() => (confirmingDelete = false)}>
								Cancel
							</Button>
						</div>
					{:else}
						<Button
							variant="ghost"
							size="icon"
							aria-label="Playlist options"
							onclick={openMenu}
						>
							<HugeiconsIcon icon={MoreVerticalIcon} class="h-5 w-5 text-muted-foreground" />
						</Button>
					{/if}
				</div>
			</div>
		</div>
		<div class="content-in min-h-0 flex-1 overflow-y-auto p-4">
			{#if selected.size > 0}
				<div
					class="sticky top-0 z-20 -mx-4 mb-1 flex items-center gap-2 border-b bg-background/95 px-4 py-2 backdrop-blur"
				>
					<span class="px-1 text-sm font-medium">{selected.size} selected</span>
					<Button size="sm" class="gap-1.5" onclick={doPlaySel} disabled={!selectedItems.length}>
						<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" /> Play
					</Button>
					<Button
						variant="outline"
						size="sm"
						class="gap-1.5"
						onclick={doQueueSel}
						disabled={!selectedItems.length}
					>
						<HugeiconsIcon icon={ArrowDownWideNarrowIcon} class="h-4 w-4" /> Add to queue
					</Button>
					<Button
						variant="outline"
						size="sm"
						class="gap-1.5"
						onclick={doAddSel}
						disabled={!selectedItems.length}
					>
						<HugeiconsIcon icon={Playlist02Icon} class="h-4 w-4" /> Add to playlist…
					</Button>
					{#if canMoveSel}
						<Button variant="outline" size="sm" class="gap-1.5" onclick={doMoveSel}>
							<HugeiconsIcon icon={Move01Icon} class="h-4 w-4" /> Move to playlist…
						</Button>
					{/if}
					{#if canRemoveSel}
						<Button variant="destructive" size="sm" class="gap-1.5" onclick={doRemoveSel}>
							<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" /> Remove
						</Button>
					{/if}
					<div class="flex-1"></div>
					<Button variant="ghost" size="icon" aria-label="Clear selection" onclick={clearSelection}>
						<HugeiconsIcon icon={Cancel01Icon} class="h-4 w-4" />
					</Button>
				</div>
			{/if}
			{#each shown as item, i (item.video_id + i)}
				<!-- The row is interactive by design (select/play/right-click); TrackRow inside
				     provides the keyboard-accessible controls. -->
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class="rounded-lg {selected.has(selKey(item)) ? 'bg-accent/20 ring-1 ring-primary/40' : ''}"
					onclickcapture={(e) => onRowClickCapture(e, i)}
					oncontextmenu={(e) => onRowContextMenu(e, i)}
				>
					<TrackRow
						song={item}
						index={i}
						active={item.video_id === nowId}
						onplay={() => playAll(i)}
						onAdd={() => openAddToPlaylist(item)}
						onRemove={isLiked || (editable && item.set_video_id) ? () => removeTrack(item) : undefined}
					/>
				</div>
			{:else}
				<p class="p-4 text-sm text-muted-foreground">This playlist is empty.</p>
			{/each}
			{#if pl.continuation}
				{#if moreError}
					<div class="p-3 text-center">
						<Button variant="outline" size="sm" onclick={loadMore} disabled={loadingMore}>
							{loadingMore ? 'Loading…' : 'Try again'}
						</Button>
					</div>
				{:else}
					<!-- The sentinel sits above the skeletons: it triggers the next page as it scrolls
					     into range, so the rest of a long playlist arrives without a button. -->
					<div aria-busy={loadingMore}>
						<div {@attach sentinel}></div>
						{#if loadingMore}
							{#each Array(4) as _, i (i)}
								<TrackRowSkeleton />
							{/each}
						{/if}
					</div>
				{/if}
			{/if}
			{#if recs && recs.length > 0}
				<h3 class="px-1 pt-5 pb-2 text-sm font-semibold">More like this</h3>
				<div class="space-y-0.5 pb-2">
					{#each recs as rec}
						<div class="group flex items-center gap-3 rounded-lg px-2 py-1.5 hover:bg-muted/50">
							{#if rec.thumbnail}
								<img src={rec.thumbnail} alt="" class="h-10 w-10 rounded-md object-cover" />
							{:else}
								<div class="h-10 w-10 rounded-md bg-muted"></div>
							{/if}
							<div class="min-w-0 flex-1">
								<div class="truncate text-sm font-medium">{rec.title}</div>
								<div class="truncate text-xs text-muted-foreground">
									{rec.artists || 'Unknown artist'}
								</div>
							</div>
							<Button
								variant="ghost"
								size="icon"
								class="opacity-0 transition-opacity group-hover:opacity-100"
								aria-label={`Play ${rec.title}`}
								onclick={() => playSong(rec)}
							>
								<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" />
							</Button>
							<Button
								variant="ghost"
								size="icon"
								class="opacity-0 transition-opacity group-hover:opacity-100"
								aria-label={`Add ${rec.title} to this playlist`}
								onclick={() => addRec(rec)}
							>
								<HugeiconsIcon icon={PlusSignIcon} class="h-4 w-4" />
							</Button>
						</div>
					{/each}
				</div>
				<div class="flex justify-center pt-1">
					<Button
						variant="ghost"
						size="sm"
						class="gap-1.5 text-xs text-muted-foreground hover:text-foreground"
						onclick={refreshRecs}
						disabled={recRefreshing}
					>
						<HugeiconsIcon icon={RefreshIcon} class="h-4 w-4" />
						{recRefreshing ? 'Finding more…' : 'Find more like this'}
					</Button>
				</div>
			{/if}
		</div>
	{/if}
</div>

{#if sortOpen}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={() => (sortOpen = false)}
		aria-label="Close menu"
	></button>
	<div
		class="fixed z-50 max-h-[70vh] w-56 origin-top-right animate-in overflow-y-auto rounded-xl border-transparent glass-strong p-2 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95 {sortUp ? 'origin-bottom-right' : 'origin-top-right'}"
		style="right:{sx}px; {sortUp ? 'bottom' : 'top'}:{sy}px;"
	>
		<RadioGroup.Root class="gap-0">
			{#each SORTS as s (s.key)}
				<button
					type="button"
					class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
					onclick={() => chooseSort(s.key)}
				>
					<RadioGroup.Item checked={sort === s.key} class="pointer-events-none mr-1" />
					<span class={sort === s.key ? 'font-semibold text-foreground' : ''}>{s.label}</span>
				</button>
			{/each}
			<div class="my-1 h-px bg-border"></div>
			<button
				type="button"
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={toggleDesc}
			>
				<RadioGroup.Item checked={desc} class="pointer-events-none mr-1" />
				<span class={desc ? 'font-semibold text-foreground' : ''}>Reverse order</span>
			</button>
		</RadioGroup.Root>
	</div>
{/if}
{#if selMenuOpen}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={() => (selMenuOpen = false)}
		aria-label="Close menu"
	></button>
	<div
		class="fixed z-50 min-w-52 origin-top-left animate-in rounded-xl border-transparent glass-strong p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95"
		style="left:{selX}px; top:{selY}px;"
	>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={doPlaySel}
			disabled={!selectedItems.length}
		>
			<HugeiconsIcon icon={PlayIcon} class="h-4 w-4" /> Play
		</button>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={doQueueSel}
			disabled={!selectedItems.length}
		>
			<HugeiconsIcon icon={ArrowDownWideNarrowIcon} class="h-4 w-4" /> Add to queue
		</button>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={doAddSel}
			disabled={!selectedItems.length}
		>
			<HugeiconsIcon icon={Playlist02Icon} class="h-4 w-4" /> Add to playlist…
		</button>
		{#if canMoveSel}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={doMoveSel}
			>
				<HugeiconsIcon icon={Move01Icon} class="h-4 w-4" /> Move to playlist…
			</button>
		{/if}
		{#if canRemoveSel}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-destructive hover:bg-destructive/10"
				onclick={doRemoveSel}
			>
				<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" /> Remove
			</button>
		{/if}
	</div>
	{/if}
	{#if menuOpen}
	<button
		class="fixed inset-0 z-40 cursor-default"
		onclick={() => (menuOpen = false)}
		aria-label="Close menu"
	></button>
	<div
		class="fixed z-50 min-w-52 origin-top-left animate-in rounded-xl border-transparent glass-strong p-1 text-popover-foreground shadow-xl duration-150 fade-in-0 zoom-in-95"
		style="left:{mx}px; top:{my}px;"
	>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={() => run(shufflePlay)}
			disabled={!pl?.items.length}
		>
			<HugeiconsIcon icon={ShuffleIcon} class="h-4 w-4" /> Shuffle play
		</button>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={() => run(() => queue(true))}
			disabled={!pl?.items.length}
		>
			<HugeiconsIcon icon={ArrowUpNarrowWideIcon} class="h-4 w-4" /> Play next
		</button>
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={() => run(() => queue(false))}
			disabled={!pl?.items.length}
		>
			<HugeiconsIcon icon={ArrowDownWideNarrowIcon} class="h-4 w-4" /> Add to queue
		</button>
		<!-- On Repeat is built from local play counts — there is no YouTube playlist to seed a
		     radio from. -->
		{#if !isOnRepeat}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={() => run(() => startRadio('playlist', id, pl?.title))}
			>
				<HugeiconsIcon icon={Radio02Icon} class="h-4 w-4" /> Start radio
			</button>
		{/if}
		<button
			class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
			onclick={() => run(() => addPick(asItem()))}
		>
			<HugeiconsIcon icon={DashboardSquare02Icon} class="h-4 w-4" /> Add to shortcuts
		</button>
		{#if editable}
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm hover:bg-accent/10"
				onclick={() => run(startRename)}
			>
				<HugeiconsIcon icon={PencilEdit02Icon} class="h-4 w-4" /> Edit name
			</button>
			<button
				class="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm text-destructive hover:bg-destructive/10"
				onclick={() => run(() => (confirmingDelete = true))}
			>
				<HugeiconsIcon icon={Delete02Icon} class="h-4 w-4" /> Delete playlist
			</button>
		{/if}
	</div>
{/if}
