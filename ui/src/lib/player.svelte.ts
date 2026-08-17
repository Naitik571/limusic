// Shared reactive app state (playback + auth), set up ONCE by the root layout. Components import
// `playback`/`auth` and read them reactively; the Rust side drives them via Tauri events.
// context/11 UI contract — this module only calls commands / subscribes to events.
import { browser } from '$app/environment';
import * as api from './api';
import type { Account, AccountIdentity, BrowseItem, NowPlaying, QueueState, SongItem } from './api';
import { applyLtState, lt } from './lt.svelte';
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
	// Like state for the current track — seeded from the track's real `likeStatus` on each change,
	// then optimistic on toggle.
	liked: false
});

// --- Offline download manager (Titlebar popover). A reactive list of in-flight / finished /
// failed downloads, fed by the Rust event bus. Each track carries a 0–100 percent so the UI
// can draw a real progress bar. `active` is the count still in flight (yellow dot); when it
// hits 0 the indicator goes green, then resets to idle once the popover is dismissed.
export type DownloadItem = {
	id: string;
	title: string;
	artists?: string;
	thumb?: string | null;
	percent: number; // 0–100
	state: 'downloading' | 'done' | 'error';
	message?: string;
};
export const downloads = $state<{ items: DownloadItem[]; active: number; done: number; errored: number }>(
	{ items: [], active: 0, done: 0, errored: 0 }
);
// Hide the green "all done" state once the user has seen it.
export const dismissDownloads = () => {
	downloads.done = 0;
	downloads.errored = 0;
	for (const it of downloads.items) if (it.state !== 'downloading') it.state = 'downloading', (it.percent = 0);
	downloads.items = downloads.items.filter((i) => i.state === 'downloading');
};

let downloadMonitorStarted = false;
export function startDownloadMonitor() {
	if (downloadMonitorStarted || !browser) return;
	downloadMonitorStarted = true;
	api.onDownloadProgress((p: any) => {
		const id = p.video_id as string;
		let it = downloads.items.find((x) => x.id === id);
		if (!it) {
			it = { id, title: (p.title as string) ?? id, artists: p.artists, thumb: p.thumb, percent: 0, state: 'downloading' };
			downloads.items.push(it);
			downloads.active += 1;
		}
		it.percent = Math.max(it.percent, Math.min(100, Math.round(p.percent ?? 0)));
		it.title = (p.title as string) ?? it.title;
		it.artists = p.artists ?? it.artists;
		it.thumb = p.thumb ?? it.thumb;
	});
	api.onDownloadComplete((p: any) => {
		const id = p.video_id as string;
		const it = downloads.items.find((x) => x.id === id);
		if (it) { it.state = 'done'; it.percent = 100; }
		downloads.active = Math.max(0, downloads.active - 1);
		downloads.done += 1;
	});
	api.onDownloadError((p: any) => {
		const id = p.video_id as string;
		const it = downloads.items.find((x) => x.id === id);
		if (it) { it.state = 'error'; it.message = p.error; }
		else downloads.items.push({ id, title: (p.title as string) ?? id, percent: 0, state: 'error', message: p.error });
		downloads.active = Math.max(0, downloads.active - 1);
		downloads.errored += 1;
	});
}

/**
 * The full-window now-playing view (NowPlaying.svelte): big artwork, plus the Queue/Lyrics tabs.
 * It lives here rather than in the layout because starting something playing opens it, and every
 * "play this" path already goes through this module. The open has to happen at the click: a
 * gapless advance looks exactly like a user play from the `now-playing` event alone.
 */
export const np = $state({ open: false, tab: 'queue' as 'queue' | 'lyrics' });

export const openPlayer = () => (np.open = true);

/** Play one track (a search row, a song card, a shelf), and show it. */
export function playSong(song: SongItem) {
	openPlayer();
	return api.play(song);
}

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
	error: null as string | null,
	// Saved albums and artists. Only the Library page renders them, but they live here rather than in
	// that page's local state so leaving and coming back paints the cached grid instead of a skeleton
	// while three requests go out again.
	albums: [] as BrowseItem[],
	artists: [] as BrowseItem[],
	extrasLoaded: false,
	extrasLoading: false,
	extrasError: null as string | null
});

/** Account switches can happen while a library request is still in flight. A generation lets the
 * old response finish harmlessly instead of overwriting the newly selected channel's data. */
let libraryGeneration = 0;

function resetLibraryForAccount() {
	libraryGeneration++;
	library.items = [];
	library.loaded = false;
	library.loading = false;
	library.error = null;
	library.albums = [];
	library.artists = [];
	library.extrasLoaded = false;
	library.extrasLoading = false;
	library.extrasError = null;
}

/** Fetch the library once (or force a refresh). No-op while a load is in flight. */
export async function loadLibrary(force = false) {
	if (library.loading || (library.loaded && !force)) return;
	const generation = libraryGeneration;
	library.loading = true;
	library.error = null;
	try {
		const items = await api.getLibrary();
		if (generation !== libraryGeneration) return;
		library.items = items;
		library.loaded = true;
	} catch (e) {
		if (generation === libraryGeneration) library.error = String(e);
	} finally {
		if (generation === libraryGeneration) library.loading = false;
	}
}

/** Saved albums + artists, same caching rules as `loadLibrary`. */
export async function loadLibraryExtras(force = false) {
	if (library.extrasLoading || (library.extrasLoaded && !force)) return;
	const generation = libraryGeneration;
	library.extrasLoading = true;
	library.extrasError = null;
	try {
		const [albums, artists] = await Promise.all([
			api.getLibraryAlbums(),
			api.getLibraryArtists()
		]);
		if (generation !== libraryGeneration) return;
		library.albums = albums;
		library.artists = artists;
		library.extrasLoaded = true;
	} catch (e) {
		if (generation === libraryGeneration) library.extrasError = String(e);
	} finally {
		if (generation === libraryGeneration) library.extrasLoading = false;
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
	artists: [] as BrowseItem[],
	songs: [] as SongItem[],
	loading: false,
	scanned: false,
	error: null as string | null
});

/**
 * Music that is no longer on disk, from a scan or from a play attempt that found nothing there.
 * Everything holding those ids drops them in the same tick: the Local tab's lists, the Shortcuts
 * grid, sidebar pins, recents. Nothing waits for a refetch, and nothing is left to fail later.
 */
export function forgetLocal(removed: string[]) {
	if (!removed.length) return;
	const gone = new Set(removed);
	local.songs = local.songs.filter((s) => !gone.has(s.video_id));
	local.albums = local.albums.filter((a) => !gone.has(a.id));
	local.artists = local.artists.filter((a) => !gone.has(a.id));
	const dropped = pl.forgetIds(personal, removed);
	savePersonal();
	if (dropped) toast(`Removed ${dropped} shortcut${dropped === 1 ? '' : 's'} for deleted music`);
}

/** Take a scan result: replace the library, then prune whatever it reports as gone. */
function applyLocal(lib: api.LocalLibrary) {
	local.folders = lib.folders;
	local.albums = lib.albums;
	local.artists = lib.artists;
	local.songs = lib.songs;
	local.scanned = true;
	local.error = null;
	forgetLocal(lib.removed);
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

/** No-op while a scan is already running: the startup scan and opening the Local tab overlap. */
export const scanLocal = () =>
	local.loading ? Promise.resolve() : runLocal(api.getLocalLibrary);
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
	toast.success(added ? 'Added to shortcuts' : 'Already in shortcuts');
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

/**
 * Home's arrangement, as set in the Edit modal. `order` is every section key the modal listed, in
 * display order, hidden ones included — a hidden section that keeps its slot comes back where it was.
 */
export function saveHomeLayout(order: string[], hidden: string[]) {
	personal.home = { order, hidden };
	savePersonal();
}

/** Called from every card click app-wide, so only persist when the id was actually on the grid. */
export function touchPick(id: string) {
	if (pl.touchPick(personal, id)) savePersonal();
}

export function togglePin(id: string) {
	const result = pl.togglePin(personal, id);
	if (result === 'full') toast.error(`Unpin one first — ${pl.MAX_PINS} pins max`);
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

/** Like/unlike whatever is playing. Thin wrapper so the player bar and the mini player share one
 *  implementation (and one optimistic path) with every list row. */
export function toggleNowPlayingLike(): Promise<void> {
	const n = playback.now;
	if (!n) return Promise.resolve();
	return toggleLike({ video_id: n.videoId, title: n.title, artists: n.artists });
}

// --- Volume ------------------------------------------------------------------------------------
// Shared by the player bar and the mini player, which means there is one behaviour to get right
// instead of two to keep in step.

// Live while dragging (the user hears it), but trailing-throttled so a drag doesn't flood IPC.
let volTimer: ReturnType<typeof setTimeout> | null = null;

export function dragVolume(v: number) {
	playback.volume = v;
	if (volTimer) return;
	volTimer = setTimeout(() => {
		volTimer = null;
		api.setVolume(playback.volume);
	}, 100);
}

/** Pointer released: always send the final value, throttle window or not. */
export function commitVolume(v: number) {
	if (volTimer) {
		clearTimeout(volTimer);
		volTimer = null;
	}
	playback.volume = v;
	api.setVolume(v);
}

// Mute *is* volume 0 — no separate flag, so dragging the slider off zero un-mutes for free and the
// icon can't disagree with what you hear. Remembers the level to come back to; falls back to 100
// when the user dragged to zero themselves (nothing was remembered).
let preMute = 100;

export function toggleMute() {
	const muted = playback.volume === 0;
	if (!muted) preMute = playback.volume;
	commitVolume(muted ? preMute || 100 : 0);
}

// --- Keyboard shortcuts ------------------------------------------------------------------------
// One global handler, registered by `initApp` so the main window *and* the mini-player window
// (which runs this same module) behave identically. Keymap follows YT Music's web conventions,
// which are what desktop music players (and users migrating from the YTM web app) expect:
//
//   Space / K        play/pause
//   Shift+N / Shift+P next / previous track
//   M                mute (remembered level restored)
//   ArrowUp/Down     volume +5 / −5
//   ArrowLeft/Right  seek −5s / +5s
//   J / L            seek −10s / +10s
//
// OS media keys (SMTC on Windows, MPRIS on Linux) are handled natively in Rust (`media.rs`)
// and keep working while the window is unfocused — these keys cover the in-app case.
//
// Native widgets keep their own keyboard behaviour: typing in a field, Space activating a
// focused button, arrows on a focused slider. The guard below yields to them — without it,
// Space would toggle playback *instead of* activating whichever button has focus.

const SHORTCUT_STEP = 5; // volume percent / seek seconds for the arrow keys

function onShortcut(e: KeyboardEvent) {
	const target = e.target as HTMLElement | null;
	// Text-entry contexts always win — typing must never trigger playback.
	if (target?.closest('input, textarea, select, [contenteditable="true"]')) return;
	// Rows and buttons keep focus after a click (TrackRow is role="button" and plays on
	// Space). They consume Space/Enter themselves; everything else (m, arrows, N/P…) should
	// still work with focus sitting on one of them — otherwise every shortcut dies after
	// the first song-row click, which reads as "sometimes buggy".
	const onButton = !!target?.closest('button, [role="button"]');
	if (onButton && (e.key === ' ' || e.key === 'Enter')) return;
	const key = e.key;
	const pos = playback.position;
	if (key === ' ' || key.toLowerCase() === 'k') {
		e.preventDefault();
		api.togglePause();
	} else if (e.shiftKey && key.toLowerCase() === 'n') {
		e.preventDefault();
		api.nextTrack();
	} else if (e.shiftKey && key.toLowerCase() === 'p') {
		e.preventDefault();
		api.prevTrack();
	} else if (key.toLowerCase() === 'm') {
		e.preventDefault();
		toggleMute();
	} else if (key === 'ArrowUp') {
		e.preventDefault();
		commitVolume(Math.min(100, playback.volume + SHORTCUT_STEP));
	} else if (key === 'ArrowDown') {
		e.preventDefault();
		commitVolume(Math.max(0, playback.volume - SHORTCUT_STEP));
	} else if (key === 'ArrowLeft') {
		e.preventDefault();
		api.seek(Math.max(0, pos - 5));
	} else if (key === 'ArrowRight') {
		e.preventDefault();
		api.seek(pos + 5);
	} else if (key.toLowerCase() === 'j') {
		e.preventDefault();
		api.seek(Math.max(0, pos - 10));
	} else if (key.toLowerCase() === 'l') {
		e.preventDefault();
		api.seek(pos + 10);
	}
}

// --- Sleep timer -------------------------------------------------------------------------------
// Rust enforces the actual pause (a 1 Hz tick thread in lib.rs, so it keeps counting with the
// window closed — tray/mini-player playback included). This side only mirrors it for the chip:
// the mode, the countdown, and clearing when the `sleep-timer-fired` event lands.
export type SleepTimerMode = 'off' | 'end_of_song' | 'minutes';
// `remaining` (seconds) lives on the object so it can be exported: Svelte 5 forbids exporting
// reassigned module state, but property mutation of an exported $state object is fine — the
// same shape as `playback`.
export const sleepTimer = $state<{ mode: SleepTimerMode; endAt: number; remaining: number }>({
	mode: 'off',
	endAt: 0,
	remaining: 0
});
let sleepTick: ReturnType<typeof setInterval> | undefined;

function stopSleepTick() {
	if (sleepTick) {
		clearInterval(sleepTick);
		sleepTick = undefined;
	}
}
function startSleepTick() {
	if (sleepTick) return;
	sleepTick = setInterval(() => {
		sleepTimer.remaining = Math.max(0, sleepTimer.remaining - 1);
		// Local countdown hit zero a moment before the Rust tick (they fire within ~1s of each
		// other). Clear the chip; the fired event still lands for the toast.
		if (sleepTimer.remaining === 0) {
			sleepTimer.mode = 'off';
			stopSleepTick();
		}
	}, 1000);
}

/** Arm the sleep timer: `'off'`, `'end_of_song'`, or `'minutes'` with a length. Rust enforces it. */
export function setSleepTimer(mode: SleepTimerMode, minutes = 30) {
	if (mode === 'off') {
		sleepTimer.mode = 'off';
		stopSleepTick();
		api.setSleepTimer('off').catch((e) => toast.error(String(e)));
		return;
	}
	if (mode === 'end_of_song') {
		sleepTimer.mode = 'end_of_song';
		stopSleepTick();
		api.setSleepTimer('end_of_song').catch((e) => toast.error(String(e)));
		return;
	}
	sleepTimer.mode = 'minutes';
	sleepTimer.endAt = Date.now() + minutes * 60_000;
	sleepTimer.remaining = minutes * 60;
	startSleepTick();
	api.setSleepTimer(String(minutes)).catch((e) => toast.error(String(e)));
}

function clearSleepTimer() {
	sleepTimer.mode = 'off';
	sleepTimer.remaining = 0;
	stopSleepTick();
}

/** Hand over to the floating widget (Rust `mini.rs`); the app hides to the tray behind it. */
export function openMiniPlayer() {
	api.openMini().catch((e) => toast.error(String(e)));
}

/** Advance the repeat mode: off → all → one → off. */
export function cycleRepeat(): Promise<void> {
	const r = playback.queue.repeat ?? 'off';
	return api.setRepeat(r === 'off' ? 'all' : r === 'all' ? 'one' : 'off');
}

/** Optimistic like toggle, reverted if YouTube rejects it. */
export async function toggleLike(song: SongItem) {
	const next = !isLiked(song);
	const isNow = playback.now?.videoId === song.video_id;
	likedSongs[song.video_id] = next;
	if (isNow) playback.liked = next;
	try {
		await api.like(song.video_id, next);
		toast.success(next ? 'Added to liked songs' : 'Removed from liked songs');
	} catch (e) {
		likedSongs[song.video_id] = !next;
		if (isNow) playback.liked = !next;
		toast.error(String(e));
	}
}

/**
 * Play a playlist/album/artist and record that it was played, which is what sorts the sidebar and
 * seeds Shortcuts. Every "play these tracks from somewhere" call site goes through this.
 * `sourceId` (playlist/album pages only) points autoplay at that context's radio.
 * `continuation` (the playlist page's next-page token) hands the rest of a long playlist to the
 * backend to walk in the background, so playback starts on the tracks already loaded.
 */
export function playFrom(
	source: BrowseItem,
	items: SongItem[],
	start: number | null,
	sourceId?: string,
	shuffle?: boolean,
	continuation?: string
) {
	pl.noteRecent(personal, source);
	pl.touchPick(personal, source.id);
	savePersonal();
	openPlayer();
	return api.playPlaylist(items, start, sourceId, source.title, shuffle, continuation);
}

/**
 * "Play next" / "Add to queue" from any surface (song menus, card menus, page headers). One
 * implementation so the wording is the same everywhere. Guests get their toast from the session
 * flow instead ("Added to the session queue."), so this one stays quiet for them.
 */
export async function enqueue(
	items: SongItem[],
	next: boolean,
	from?: string,
	continuation?: string
) {
	if (!items.length) return;
	try {
		// A "Play next" block is capped at the tracks the page has loaded: shoving 5000 in front of
		// what's playing isn't what anyone means by "next". "Add to queue" walks the rest.
		await (next ? api.playNext(items, from) : api.addToQueue(items, from, continuation));
	} catch (e) {
		toast.error(String(e));
		return;
	}
	if (lt.role === 'guest') return;
	const n = items.length;
	if (next) toast.success(n === 1 ? 'Playing next' : `${n} songs play next`);
	else toast.success(n === 1 ? 'Added to queue' : `Added ${n} songs to the queue`);
}

/**
 * Start a radio from any surface (song menus, card menus, page headers). One implementation so the
 * feedback is the same everywhere: radio is a network round trip before anything audibly happens,
 * so it says so up front rather than looking like the click was swallowed.
 */
export async function startRadio(
	kind: 'song' | 'artist' | 'album' | 'playlist',
	id: string,
	name?: string
) {
	toast('Starting radio…');
	openPlayer();
	try {
		await api.startRadio(kind, id, name);
	} catch (e) {
		toast.error(String(e));
	}
}

// Transient UI state for write actions.
export const ui = $state({
	addSongs: null as SongItem[] | null, // add-to-playlist picker target(s), full items for optimistic appends
	// When set, the picker also removes each song from this playlist after a successful add —
	// a true move, not a copy. Cleared on open so it never leaks between sessions.
	moveFrom: '' as string,
	toast: null as Toast | null,
	settingsOpen: false, // the settings modal
	settingsTab: '' as string, // when set, the settings modal opens on this tab (consumed on open)
	ltOpen: false, // the Listen Together modal
	channelPickerOpen: false,
	channelPickerRequired: false, // true while a multi-channel login is not finalized yet
	channelIdentities: [] as AccountIdentity[]
});

export function openChannelPicker(required = false) {
	ui.channelPickerRequired = required;
	ui.channelIdentities = [];
	ui.channelPickerOpen = true;
}

export type Toast = { msg: string; kind: 'info' | 'success' | 'error' };

// A counter, not the toast itself: $state proxies the stored object, so `ui.toast === t` is never
// true and the toast would never clear. It also means a repeated message can't cut its own retry short.
let seq = 0;

function show(msg: string, kind: Toast['kind']) {
	const id = ++seq;
	ui.toast = { msg, kind };
	setTimeout(() => {
		if (seq === id) ui.toast = null;
	}, 2500);
}

/** Sonner-shaped. Bare `toast(msg)` is a neutral notice; .success/.error pick the icon. */
export const toast = Object.assign((msg: string) => show(msg, 'info'), {
	info: (msg: string) => show(msg, 'info'),
	success: (msg: string) => show(msg, 'success'),
	error: (msg: string) => show(msg, 'error')
});

export function openAddToPlaylist(song: SongItem) {
	ui.addSongs = [song];
}

/** Open the picker to add several tracks at once (e.g. a whole album). */
export function openAddManyToPlaylist(songs: SongItem[]) {
	ui.moveFrom = '';
	ui.addSongs = songs.length ? songs : null;
}

/** Open the picker to MOVE several tracks: added to the target, then removed from `fromId`. */
export function openMoveToPlaylist(songs: SongItem[], fromId: string) {
	ui.moveFrom = fromId;
	ui.addSongs = songs.length ? songs : null;
}

// Last successful add-to-playlist — the open playlist page appends these optimistically.
export const lastPlaylistAdd = $state({ playlistId: '', songs: [] as SongItem[], epoch: 0 });

// Last successful MOVE — the source page drops these rows optimistically.
export const lastPlaylistMove = $state({ fromId: '', songs: [] as SongItem[], epoch: 0 });

export function notePlaylistMove(fromId: string, songs: SongItem[]) {
	lastPlaylistMove.fromId = fromId;
	lastPlaylistMove.songs = songs;
	lastPlaylistMove.epoch++;
}

export function notePlaylistAdd(playlistId: string, songs: SongItem[]) {
	lastPlaylistAdd.playlistId = playlistId;
	// Strip per-context fields: set_video_id belongs to the source playlist, the queue markers to
	// the queue — none apply to the row's new home.
	lastPlaylistAdd.songs = songs.map((s) => ({
		...s,
		set_video_id: undefined,
		autoplay: undefined,
		queued: undefined,
		queued_end: undefined,
		queued_from: undefined,
		queued_by: undefined
	}));
	lastPlaylistAdd.epoch++;
}

let started = false;

/**
 * Wire the Tauri event listeners once and seed initial state. Returns a teardown fn.
 *
 * `mini` is the floating-widget window (mini.rs): it runs this same module, and the events are
 * emitted app-wide so it gets playback for free — but it has no library, no local tab, no account
 * menu and no Listen Together UI, so it skips those fetches rather than duplicating the app's.
 */
export function initApp(mini = false): () => void {
	if (started) return () => {};
	started = true;
	const subs = [
		api.onNowPlaying((n) => {
			playback.now = n;
			playback.liked = n.liked ?? false; // reflect the track's real like status when known
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
		api.onVolume((v) => {
			// Not while our own drag is in flight: the echo is a value the pointer has already
			// moved past, and applying it would yank the thumb backwards mid-drag.
			if (!volTimer) playback.volume = v;
		}),
		api.onPlaybackError((msg) => toast.error(msg)),
		api.onPlaybackNotice((msg) => toast(msg)), // auto-skipped an unplayable track
		api.onSleepTimerFired(() => {
			clearSleepTimer();
			toast('Sleep timer ended playback');
		}),
		api.onLocalChanged(forgetLocal), // a local file turned out to be gone — drop it everywhere
		api.onAuthChanged((a) => {
			auth.account = a;
			resetLibraryForAccount();
			// Signing out doesn't empty the library: On Repeat and anything saved on this machine
			// are still there, and the backend answers both without touching YouTube.
			if (!mini) loadLibrary(true);
			if (!a.signedIn) {
				ui.channelPickerOpen = false;
				ui.channelPickerRequired = false;
				ui.channelIdentities = [];
			}
			clearCached();
			auth.epoch++;
		}),
		api.onAccountSelectionRequired(() => openChannelPicker(true)),
		api.onLoginError((msg) => toast.error(msg)),
		api.onLoginDone(() => toast.success('Signed in')),
		// Listen Together (context/19): mirror the Rust session state; surface notices as toasts.
		api.onLtState((s) => applyLtState(s)),
		api.onLtNotice((msg) => toast(msg))
	];
	// Keyboard shortcuts: a plain listener (not a Tauri event) — the handler above lives in this
	// module, so registering it here gives both the app window and the mini player the same keys.
	const onKey = (e: KeyboardEvent) => onShortcut(e);
	window.addEventListener('keydown', onKey);
	subs.push(Promise.resolve(() => window.removeEventListener('keydown', onKey)));
	// Gamepad: the Rust poller emits `gamepad` with an action string; handle it exactly like the
	// matching keyboard shortcut so a controller works while the app is backgrounded/tray-minimized.
	const onGamepad = (action: string) => {
		const pos = playback.position;
		switch (action) {
			case 'playpause':
				api.togglePause();
				break;
			case 'next':
				api.nextTrack();
				break;
			case 'prev':
				api.prevTrack();
				break;
			case 'mute':
				toggleMute();
				break;
			case 'volup':
				commitVolume(Math.min(100, playback.volume + 5));
				break;
			case 'voldown':
				commitVolume(Math.max(0, playback.volume - 5));
				break;
			case 'seekfwd':
				api.seek(pos + 10);
				break;
			case 'seekback':
				api.seek(Math.max(0, pos - 10));
				break;
			case 'togglemini':
				openMiniPlayer();
				break;
		}
	};
	const gp = api.onGamepad(onGamepad);
	subs.push(gp);
	const teardown = () => subs.forEach((u) => u.then((f) => f()));
	api.getQueue()
		.then((q) => (playback.queue = q))
		.catch(() => {});
	// The Rust sleep timer may be running from before this window opened (it survives window
	// close). Restore the chip so the countdown isn't silently missing from the bar.
	api.getSleepTimer()
		.then((s) => {
			if (s === 'end_of_song') sleepTimer.mode = 'end_of_song';
			else if (s !== 'off') {
				sleepTimer.mode = 'minutes';
				sleepTimer.endAt = Date.now() + Number(s) * 1000;
				sleepTimer.remaining = Number(s);
				startSleepTick();
			}
		})
		.catch(() => {});
	// The events above are fire-and-forget, and this window missed every one that already fired:
	// on a cold start the backend restores the queue before the UI subscribes, and the mini player
	// is created mid-song. Ask for the current state once rather than guessing at it.
	api.getPlayback()
		.then((s) => {
			if (playback.now) return; // a real now-playing event beat us to it
			playback.now = s.now;
			playback.liked = s.now?.liked ?? false;
			playback.paused = s.paused;
			playback.position = s.position;
			playback.duration = s.duration;
		})
		.catch(() => {});
	if (mini) return teardown;
	api.getAccount()
		.then((a) => {
			auth.account = a;
			if (a.signedIn && a.selectionRequired) {
				openChannelPicker(true);
				return;
			}
			loadLibrary();
			if (a.signedIn) {
				// Only when the stored answer might be the provisional one: databases that predate
				// `canSwitch` default it to true so the action stays discoverable, and this is what
				// demotes single-channel users back to no switcher. A stored `false` is already
				// authoritative, so most launches skip the request entirely.
				if (a.canSwitch) {
					api.getAccountIdentities()
						.then((identities) => {
							if (auth.account?.signedIn) auth.account.canSwitch = identities.length > 1;
						})
						.catch(() => {});
				}
			}
		})
		.catch(() => {});
	// Scan the local folders once at startup: it seeds the Library's Local tab and, more to the
	// point, prunes shortcuts for music that was deleted while the app was closed.
	scanLocal();
	// Seed the Listen Together state (server URL, any active room after a UI reload).
	api.ltGetState().then(applyLtState).catch(() => {});
	return teardown;
}
