// Shared reactive app state (playback + auth), set up ONCE by the root layout. Components import
// `playback`/`auth` and read them reactively; the Rust side drives them via Tauri events.
// context/11 UI contract — this module only calls commands / subscribes to events.
import { browser } from '$app/environment';
import * as api from './api';
import type { Account, BrowseItem, NowPlaying, QueueState, SongItem } from './api';
import { applyLtState } from './lt.svelte';
import { clearCached } from './pagecache';
import * as pl from './personal';
import type { Personal } from './personal';

export const playback = $state({
	now: null as NowPlaying | null,
	queue: { items: [], currentIndex: 0 } as QueueState,
	paused: false,
	position: 0,
	duration: 0,
	volume: 100,
	error: null as string | null,
	// Like state for the current track — seeded from the track's real `likeStatus` on each change,
	// then optimistic on toggle.
	liked: false
});

export const auth = $state({
	account: null as Account | null,
	// Bumped on every sign-in/out. The root layout keys the page on it, so the current route
	// remounts and refetches — home/browse data is per-account and otherwise stays stale until
	// the user navigates away and back.
	epoch: 0
});

// The signed-in user's library (playlists + liked), shared by the sidebar list and the Library page
// so a create reflects in both instantly (context/11 UI contract, optimistic updates).
export const library = $state({
	items: [] as BrowseItem[],
	loaded: false,
	loading: false,
	error: null as string | null
});

/** Fetch the library once (or force a refresh). No-op while a load is in flight. */
export async function loadLibrary(force = false) {
	if (library.loading || (library.loaded && !force)) return;
	library.loading = true;
	library.error = null;
	try {
		library.items = await api.getLibrary();
		library.loaded = true;
	} catch (e) {
		library.error = String(e);
	} finally {
		library.loading = false;
	}
}

/** Create a playlist and optimistically prepend it so every view updates immediately. */
export async function createLibraryPlaylist(title: string): Promise<void> {
	const id = await api.createPlaylist(title);
	// YouTube's library browse is eventually-consistent and won't include a brand-new playlist for a
	// few seconds, so surface it immediately instead of refetching.
	const browseId = id.startsWith('VL') ? id : `VL${id}`;
	library.items = [{ kind: 'playlist', id: browseId, title }, ...library.items];
}

/** Optimistically bump the "N tracks" count in a library playlist's subtitle (sidebar + Library). */
export function bumpLibraryTrackCount(playlistId: string, delta: number) {
	library.items = library.items.map((it) => {
		if (it.id !== playlistId || !it.subtitle) return it;
		const subtitle = it.subtitle.replace(/\d+\s+tracks?/, (m) => {
			const n = Math.max(0, parseInt(m) + delta);
			return `${n} track${n === 1 ? '' : 's'}`;
		});
		return { ...it, subtitle };
	});
}

// --- Local music (Rust local.rs) --------------------------------------------------------------
// Shared like `library` is: the Library page renders it, and the app rescans at startup so tiles
// pointing at deleted files disappear before anyone clicks one.

export const local = $state({
	folders: [] as string[],
	albums: [] as BrowseItem[],
	songs: [] as SongItem[],
	loading: false,
	scanned: false,
	error: null as string | null
});

/**
 * Take a scan result: replace the library, then forget anything it says is gone from disk.
 * That prune is the "the user deleted their music" story — a Shortcuts tile or a sidebar pin for a
 * deleted album vanishes on the spot rather than waiting to be clicked and failing.
 */
function applyLocal(lib: api.LocalLibrary) {
	local.folders = lib.folders;
	local.albums = lib.albums;
	local.songs = lib.songs;
	local.scanned = true;
	local.error = null;
	const dropped = pl.forgetIds(personal, lib.removed);
	if (lib.removed.length) savePersonal();
	if (dropped) toast(`Removed ${dropped} shortcut${dropped === 1 ? '' : 's'} for deleted music`);
}

async function runLocal(call: () => Promise<api.LocalLibrary>) {
	local.loading = true;
	try {
		applyLocal(await call());
	} catch (e) {
		local.error = String(e);
	} finally {
		local.loading = false;
	}
}

export const scanLocal = () => runLocal(api.getLocalLibrary);
export const addLocalFolder = (path: string) => runLocal(() => api.addLocalFolder(path));
export const removeLocalFolder = (path: string) => runLocal(() => api.removeLocalFolder(path));

// --- Personalization: the Shortcuts grid, sidebar pins, play recency (see personal.ts) ----------
// The Shortcuts grid holds what the user puts in it, plus the one tile the app suggests (On
// Repeat, via `seedOnRepeatPick`). See `personal.ts`.
// localStorage rather than SQLite: only the webview ever reads this, so a table + commands + a
// `UI_SETTINGS` allowlist entry would buy nothing. Loaded at module scope (guarded like the layout's
// `initTheme`) so the sidebar and home grid render sorted on the very first paint.
// ponytail: move to db.rs if it ever needs to be account-scoped or readable outside the webview.
const PERSONAL_KEY = 'limusic:personal';

export const personal = $state<Personal>(pl.empty());

if (browser) {
	try {
		Object.assign(personal, pl.hydrate(JSON.parse(localStorage.getItem(PERSONAL_KEY) ?? 'null')));
	} catch {
		// Unreadable blob — start clean rather than break startup.
	}
}

function savePersonal() {
	if (!browser) return;
	try {
		localStorage.setItem(PERSONAL_KEY, JSON.stringify(personal));
	} catch {
		// Quota or a locked store: personalization is best-effort, never fatal.
	}
}

/** Add to Shortcuts (evicting the tile gone longest unplayed when the grid is full). */
export function addPick(item: BrowseItem) {
	const added = pl.addPick(personal, item);
	savePersonal();
	toast(added ? 'Added to shortcuts' : 'Already in shortcuts');
}

/** Drop landed: move (or add) a tile so it sits before `beforeId` — null appends. No toast: the
 *  grid rearranging under the cursor is its own feedback. */
export function placePick(item: BrowseItem, beforeId: string | null) {
	pl.placePick(personal, item, beforeId);
	savePersonal();
}

export function removePick(id: string) {
	pl.removePick(personal, id);
	savePersonal();
}

/** How many songs On Repeat needs before it's worth a tile on the grid. */
const ON_REPEAT_SEED_MIN = 5;

/**
 * Put On Repeat on the Shortcuts grid once it has enough songs to be useful: the one tile the app
 * adds by itself. Called on every home visit; `seedPick` owns the "should this go on" decision, so
 * removing the tile is permanent no matter how many times this runs. Cheap to repeat: On Repeat is
 * built from local SQLite, so the fetch never touches the network.
 */
export async function seedOnRepeatPick() {
	try {
		const onRepeat = await api.getPlaylist(api.ON_REPEAT_ID);
		if (onRepeat.items.length < ON_REPEAT_SEED_MIN) return;
		const added = pl.seedPick(personal, {
			kind: 'playlist',
			id: api.ON_REPEAT_ID,
			title: onRepeat.title ?? 'On Repeat',
			// Not a track count: the tile is stored as-is, so a number here would go stale the next
			// time the playlist re-ranks itself.
			subtitle: 'Your most played'
		});
		if (added) savePersonal();
	} catch {
		// No tile this time; the next home visit tries again.
	}
}

/** Called from every card click app-wide, so only persist when the id was actually on the grid. */
export function touchPick(id: string) {
	if (pl.touchPick(personal, id)) savePersonal();
}

export function togglePin(id: string) {
	const result = pl.togglePin(personal, id);
	if (result === 'full') toast(`Unpin one first — ${pl.MAX_PINS} pins max`);
	else savePersonal();
	return result;
}

// Like state that outlives one row. A song's `liked` flag is a snapshot from whenever its page was
// fetched, and the same song shows up in several places at once (a list row, its ⋯ menu, the player
// bar). One override map keyed by videoId keeps them all telling the same story; the current track
// stays owned by `playback.liked`, which the Rust side reseeds on every track change.
const likedSongs = $state<Record<string, boolean>>({});

export function isLiked(song: SongItem): boolean {
	if (playback.now?.videoId === song.video_id) return playback.liked;
	return likedSongs[song.video_id] ?? song.liked ?? false;
}

/** Optimistic like toggle, reverted if YouTube rejects it. */
export async function toggleLike(song: SongItem) {
	const next = !isLiked(song);
	const isNow = playback.now?.videoId === song.video_id;
	likedSongs[song.video_id] = next;
	if (isNow) playback.liked = next;
	try {
		await api.like(song.video_id, next);
		toast(next ? 'Added to liked songs' : 'Removed from liked songs');
	} catch (e) {
		likedSongs[song.video_id] = !next;
		if (isNow) playback.liked = !next;
		toast(String(e));
	}
}

/**
 * Play a playlist/album/artist and record that it was played, which is what sorts the sidebar and
 * seeds Shortcuts. Every "play these tracks from somewhere" call site goes through this.
 * `sourceId` (playlist/album pages only) points autoplay at that context's radio.
 */
export function playFrom(
	source: BrowseItem,
	items: SongItem[],
	start: number | null,
	sourceId?: string,
	shuffle?: boolean
) {
	pl.noteRecent(personal, source);
	pl.touchPick(personal, source.id);
	savePersonal();
	return api.playPlaylist(items, start, sourceId, source.title, shuffle);
}

// Transient UI state for write actions.
export const ui = $state({
	addSongs: null as SongItem[] | null, // add-to-playlist picker target(s), full items for optimistic appends
	toast: null as string | null,
	settingsOpen: false, // the settings modal
	ltOpen: false // the Listen Together modal
});

export function toast(msg: string) {
	ui.toast = msg;
	setTimeout(() => {
		if (ui.toast === msg) ui.toast = null;
	}, 2500);
}

export function openAddToPlaylist(song: SongItem) {
	ui.addSongs = [song];
}

/** Open the picker to add several tracks at once (e.g. a whole album). */
export function openAddManyToPlaylist(songs: SongItem[]) {
	ui.addSongs = songs.length ? songs : null;
}

// Last successful add-to-playlist — the open playlist page appends these optimistically.
export const lastPlaylistAdd = $state({ playlistId: '', songs: [] as SongItem[], epoch: 0 });

export function notePlaylistAdd(playlistId: string, songs: SongItem[]) {
	lastPlaylistAdd.playlistId = playlistId;
	// Strip per-context fields: set_video_id belongs to the source playlist, autoplay/queued_by to
	// the queue — none apply to the row's new home.
	lastPlaylistAdd.songs = songs.map((s) => ({
		...s,
		set_video_id: undefined,
		autoplay: undefined,
		queued_by: undefined
	}));
	lastPlaylistAdd.epoch++;
}

let started = false;

/** Wire the Tauri event listeners once and seed initial state. Returns a teardown fn. */
export function initApp(): () => void {
	if (started) return () => {};
	started = true;
	const subs = [
		api.onNowPlaying((n) => {
			playback.now = n;
			playback.liked = n.liked ?? false; // reflect the track's real like status when known
			playback.error = null; // a track started → clear any stale dead-end banner
			// Feeds Shortcuts recency and the community shelf's artist seed. Every play lands here,
			// gapless advances included, so it's the one hook that sees them all.
			pl.touchPick(personal, n.videoId);
			if (n.artists) pl.noteArtist(personal, n.artistId ?? n.artists, pl.firstArtist(n.artists));
			savePersonal();
		}),
		api.onQueueChanged((q) => (playback.queue = q)),
		api.onPosition((p) => (playback.position = p)),
		api.onDuration((d) => (playback.duration = d)),
		api.onPlaybackState((s) => (playback.paused = s === 'paused')),
		api.onPlaybackError((msg) => (playback.error = msg)),
		api.onPlaybackNotice((msg) => toast(msg)), // auto-skipped an unplayable track
		api.onAuthChanged((a) => {
			auth.account = a;
			if (a.signedIn) loadLibrary(true);
			else {
				library.items = [];
				library.loaded = false;
			}
			clearCached();
			auth.epoch++;
		}),
		api.onLoginError((msg) => toast(msg)),
		api.onLoginDone(() => toast('Signed in')),
		// Listen Together (context/19): mirror the Rust session state; surface notices as toasts.
		api.onLtState((s) => applyLtState(s)),
		api.onLtNotice((msg) => toast(msg))
	];
	api.getQueue()
		.then((q) => {
			playback.queue = q;
			// On a cold start the backend restores the queue (paused) before the UI subscribes, so
			// the now-playing event is missed. Seed the player-bar card from the restored current
			// item; hitting play resolves it for real and re-emits now-playing.
			if (!playback.now) {
				const cur = q.items[q.currentIndex];
				if (cur) {
					playback.now = {
						videoId: cur.video_id,
						title: cur.title,
						artists: cur.artists,
						artistId: cur.artist_id,
						thumbnail: cur.thumbnail,
						duration: cur.duration,
						streamClient: 'restored',
						liked: null
					};
					playback.paused = true;
				}
			}
		})
		.catch(() => {});
	api.getAccount()
		.then((a) => {
			auth.account = a;
			if (a.signedIn) loadLibrary();
		})
		.catch(() => {});
	// Scan the local folders once at startup: it seeds the Library's Local tab and, more to the
	// point, prunes shortcuts for music that was deleted while the app was closed.
	scanLocal();
	// Seed the Listen Together state (server URL, any active room after a UI reload).
	api.ltGetState().then(applyLtState).catch(() => {});
	return () => subs.forEach((u) => u.then((f) => f()));
}
