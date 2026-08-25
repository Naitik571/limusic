// The UI's only door to Rust. context/11 UI contract — commands in, events out. The UI never
// touches YouTube; everything here is a Tauri command or event payload.
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** One run of an artist line: its text, plus a channel id when that run links an artist. */
export interface ArtistRun {
	text: string;
	id?: string;
}

export interface SongItem {
	video_id: string;
	title: string;
	artists: string;
	/** Primary artist's channel browseId (`UC…`), when linked — makes the artist name navigable. */
	artist_id?: string;
	/** The artist line run by run — a collab links each name to its own page. Empty/absent when
	 * nothing is linked; render plain `artists` then. */
	artist_runs?: ArtistRun[];
	album?: string;
	/** The album's browseId (`MPRE…`), when linked — makes the album navigable. */
	album_id?: string;
	duration?: string;
	thumbnail?: string;
	/** Item id within a playlist — present only on playlist tracks; needed to remove them. */
	set_video_id?: string;
	/** Whether the signed-in user has liked this track (absent when the response didn't say). */
	liked?: boolean;
	/** Listen Together: name of the guest who added this queue item (session adds only). */
	queued_by?: string;
	/** Queued to play next ("Play next", or a guest's session add) — the "Next in queue" block. */
	queued?: boolean;
	/** Appended by "Add to queue" — its own block at the tail of the queue. */
	queued_end?: boolean;
	/** The album/playlist either block was added from, for its heading in the queue panel. */
	queued_from?: string;
	/** Appended by autoplay radio continuation — drives the queue's "Autoplay" divider + badge. */
	autoplay?: boolean;
	/** This row links a music video rather than the audio track. */
	is_video?: boolean;
	/** One of the user's own YouTube Music uploads. Set by Rust and passed straight back on play:
	 *  only an authenticated client can stream one, and the row is where that is known. */
	is_upload?: boolean;
}

export interface NowPlaying {
	videoId: string;
	title: string;
	artists: string;
	artistId?: string;
	/** The artist line run by run — links each artist of a collab separately. */
	artistRuns?: ArtistRun[];
	thumbnail?: string;
	duration?: string;
	streamClient: string;
	/** Whether the track is in the user's Liked Music (null if unknown). */
	liked?: boolean | null;
}

export type RepeatMode = 'off' | 'all' | 'one';

export interface QueueState {
	items: SongItem[];
	currentIndex: number;
	shuffle?: boolean;
	repeat?: RepeatMode;
	/** What seeded the queue (playlist/album title, "<song> Radio") — the "Next from" header. */
	sourceName?: string | null;
}

export interface Account {
	signedIn: boolean;
	name?: string | null;
	handle?: string | null;
	email?: string | null;
	thumbnail?: string | null;
	channelId?: string | null;
	canSwitch?: boolean;
	/** The cookie authenticated, but a multi-channel login is not complete until one is chosen. */
	selectionRequired?: boolean;
}

export interface AccountIdentity {
	/** Opaque, process-local selector. Raw delegated/data-sync ids stay in Rust. */
	selectionKey: string;
	name: string;
	handle?: string | null;
	email?: string | null;
	thumbnail?: string | null;
	channelId?: string | null;
	selected: boolean;
}

export interface BrowseItem {
	kind: 'song' | 'playlist' | 'album' | 'artist';
	/** videoId (song) or browseId (playlist/album/artist). */
	id: string;
	title: string;
	subtitle?: string;
	thumbnail?: string;
	/** "3:47" — song items from a list-style shelf only (card shelves don't carry one). */
	duration?: string;
	/** Song cards only: the artist line run by run, so a card that gets played keeps its links. */
	artistRuns?: ArtistRun[];
	/** YouTube flags this track/album explicit. */
	explicit?: boolean;
	/** Song cards only: one of the user's own uploads. Carried into the SongItem `asSong` builds,
	 *  because that flag is what picks the login-only client chain when it plays. */
	isUpload?: boolean;
}

export interface HomeSection {
	title: string;
	items: BrowseItem[];
	moreBrowseId?: string;
	moreParams?: string;
}
/** A mood/genre filter chip above the home feed; `params` re-fetches home filtered to it. */
export interface HomeChip {
	title: string;
	params: string;
}
export interface HomePage {
	chips: HomeChip[];
	sections: HomeSection[];
	continuation?: string;
}

/**
 * The On Repeat auto-playlist's synthetic browseId (mirrors `ON_REPEAT_ID` in state.rs). It routes
 * like any other playlist; the only thing the UI does differently is draw an icon cover, because
 * a playlist built from local play counts has no artwork of its own.
 */
export const ON_REPEAT_ID = 'LIMUSIC_ON_REPEAT';

/**
 * Liked Music's browseId. YouTube edits this one through the rating endpoint, not `edit_playlist`,
 * so it is never an add/remove/rename target: liking the song is the edit.
 */
export const LIKED_MUSIC_ID = 'VLLM';

/**
 * YouTube Music's own Library ▸ Songs, despite the name: the songs saved to the account's library.
 * It browses like a playlist (no header, no sort menu), so `getPlaylist` reads it and the Library
 * page's Songs tab pages through it with `getPlaylistMore`.
 */
export const LIBRARY_SONGS_ID = 'FEmusic_liked_videos';

/**
 * Local music (Rust `local.rs`). A file on disk is a song whose `video_id` is `LOCAL:<path>`, and
 * an album of them is a browseId `LOCALALBUM:<key>` — so local items ride every existing surface
 * (cards, queue, Shortcuts, the album page) and play with no network.
 */
export const LOCAL_SONG_PREFIX = 'LOCAL:';
export const LOCAL_ALBUM_PREFIX = 'LOCALALBUM:';
/** An artist on this disk. Renders through the album route: same page, no YouTube channel. */
export const LOCAL_ARTIST_PREFIX = 'LOCALARTIST:';
export const isLocalId = (id: string | undefined | null): boolean =>
	!!id &&
	(id.startsWith(LOCAL_SONG_PREFIX) ||
		id.startsWith(LOCAL_ALBUM_PREFIX) ||
		id.startsWith(LOCAL_ARTIST_PREFIX));

export interface LocalLibrary {
	/** Watched folders, as absolute paths. */
	folders: string[];
	albums: BrowseItem[];
	artists: BrowseItem[];
	songs: SongItem[];
	/** Song/album/artist ids that were in the library but are gone from disk since the last scan. */
	removed: string[];
}

export interface PlaylistPage {
	title?: string;
	subtitle?: string;
	thumbnail?: string;
	/** The playlist's own blurb, which the edit dialog prefills its description with. */
	description?: string;
	/** `PUBLIC` / `PRIVATE` / `UNLISTED`. Only playlists you own report it. */
	privacy?: string;
	/** Custom artwork picked on this machine; falls back to `thumbnail` when unset. */
	cover?: string;
	items: SongItem[];
	continuation?: string;
	/** True only when the signed-in user owns this playlist (rename/delete allowed). */
	owned: boolean;
}
export interface PlaylistContinuation {
	items: SongItem[];
	continuation?: string;
}

export interface ArtistCarousel {
	title: string;
	items: BrowseItem[];
	moreBrowseId?: string;
	moreParams?: string;
}
export interface SearchResults {
	top: BrowseItem[];
	songs: BrowseItem[];
	albums: BrowseItem[];
	artists: BrowseItem[];
	playlists: BrowseItem[];
}

export interface AlbumPage {
	title?: string;
	artist?: string;
	artistId?: string;
	/** The artist line run by run — links each artist of a collaborative album separately. */
	artistRuns?: ArtistRun[];
	artistThumbnail?: string;
	subtitle?: string;
	secondSubtitle?: string;
	description?: string;
	thumbnail?: string;
	items: SongItem[];
	continuation?: string;
	/** The album's audio playlist id (`OLAK5uy_…`) — autoplay's radio seed, and the save target. */
	playlistId?: string;
	/** Already saved to the signed-in user's library. */
	inLibrary: boolean;
}

export interface ArtistPage {
	name?: string;
	thumbnail?: string;
	description?: string;
	subscribers?: string;
	monthlyListeners?: string;
	channelId: string;
	subscribed: boolean;
	topSongs: SongItem[];
	/** `VL…` playlist of all the artist's top songs, behind the shelf's "See all". */
	topSongsId?: string;
	sections: ArtistCarousel[];
}

// --- commands (context/11) -----------------------------------------------------------------
export const search = (query: string) => invoke<SongItem[]>('search', { query });
/** Unfiltered search → categorized sections. */
export const searchAll = (query: string) => invoke<SearchResults>('search_all', { query });
/** Filtered "Show more" card search for one category (albums / artists / playlists). */
export const searchCards = (query: string, category: 'albums' | 'artists' | 'playlists') =>
	invoke<BrowseItem[]>('search_cards', { query, category });
export const play = (item: SongItem) => invoke<void>('play', { item });
export const playIndex = (index: number) => invoke<void>('play_index', { index });
/** Remove an upcoming track from the queue (host/local only — guests are add-only). */
export const removeFromQueue = (index: number) => invoke<void>('remove_from_queue', { index });
export const moveQueueItem = (from: number, to: number) => invoke<void>('move_queue_item', { from, to });
/**
 * "Play next": insert tracks right after the current song, behind any earlier manual adds.
 * `from` is the album/playlist they came from — it heads the block in the queue panel.
 */
export const playNext = (items: SongItem[], from?: string) =>
	invoke<void>('play_next', { items, from });
/**
 * "Add to queue": the tracks go after everything the user picked, and ahead of anything the app
 * generated behind it (autoplay filler; a radio's endless feed makes way entirely).
 * `continuation` is the source page's next-page token — the backend walks the rest of a long
 * playlist into the queue in the background.
 */
export const addToQueue = (items: SongItem[], from?: string, continuation?: string) =>
	invoke<void>('add_to_queue', { items, from, continuation });
/** Clear every upcoming manually-queued track (the "Next in queue" section). */
export const clearQueued = () => invoke<void>('clear_queued');
export const nextTrack = () => invoke<void>('next_track');
export const prevTrack = () => invoke<void>('prev_track');
export const toggleShuffle = () => invoke<void>('toggle_shuffle');
export const setRepeat = (mode: RepeatMode) => invoke<void>('set_repeat', { mode });
export const togglePause = () => invoke<void>('toggle_pause');
export const seek = (position: number) => invoke<void>('seek', { position });
export const setVolume = (volume: number) => invoke<void>('set_volume', { volume });
export const getVolume = () => invoke<number>('get_volume');
export const setSleepTimer = (mode: string) => invoke<void>('set_sleep_timer', { mode });
export const getSleepTimer = () => invoke<string>('get_sleep_timer');
export const getQueue = () => invoke<QueueState>('get_queue');

/** What the event stream already reported, for a webview that started after it did. */
export interface PlaybackSnapshot {
	now: NowPlaying | null;
	paused: boolean;
	position: number;
	duration: number;
}
export const getPlayback = () => invoke<PlaybackSnapshot>('get_playback');

// --- settings (context/11) -----------------------------------------------------------------
export const getSettings = () => invoke<Record<string, string>>('get_settings');
export const setSetting = (key: string, value: string) =>
	invoke<void>('set_setting', { key, value });
/** Streamable client keys for the "disabled clients" setting. */
export const getStreamClients = () => invoke<string[]>('get_stream_clients');
/** Wipe both cache tiers (URL cache + mpv on-disk audio cache). */
export const clearCaches = () => invoke<void>('clear_caches');
/** Grant the webview a URL for one font file the user picked, so `@font-face` can load it. */
export const allowFontFile = (path: string) => invoke<void>('allow_font_file', { path });
/** Custom app icon: applies to the window + tray immediately and persists across launches.
 *  `null` restores the bundled default. */
export const setAppIcon = (path: string | null) => invoke<void>('set_app_icon', { path });
/** Apply a built-in icon variant: 'ytm' | 'spotify' | 'limusic_blue' | 'limusic_rose' | 'limusic_amber'. */
export const setAppIconPreset = (name: string) => invoke<void>('set_app_icon_preset', { name });

/** The signed-in user's Liked Music video ids (bounded walk, ~3k). Feeds the heart on every row. */
export const getLikedIds = () => invoke<string[]>('get_liked_ids');

// --- offline downloads (Rust downloads.rs) ------------------------------------------------------
export interface DownloadedTrack {
	video_id: string;
	file_path: string;
	title: string;
	artists: string;
	album: string | null;
	duration: number;
	thumb: string | null;
	quality: string;
	format: string;
	size_bytes: number;
	added_at: number;
}
export interface DownloadList {
	items: DownloadedTrack[];
	total_bytes: number;
}
export const downloadTrack = (item: {
	videoId: string;
	title: string;
	artists: string;
	album?: string | null;
	duration: number;
	thumb?: string | null;
}) =>
	invoke<void>('download_track', item);
export const listDownloads = () => invoke<DownloadList>('list_downloads');
export const deleteDownload = (video_id: string) =>
	invoke<void>('delete_download', { video_id });
export const clearDownloads = () => invoke<void>('clear_downloads');
/** Stop one in-flight (or queued-behind-the-batch) download; its partial file is removed. */
export const cancelDownload = (video_id: string) =>
	invoke<boolean>('cancel_download', { video_id });
/** Stop every in-flight track and stop a running batch before its next track starts. */
export const cancelAllDownloads = () => invoke<number>('cancel_all_downloads');

// Download progress events (Rust → UI). Used by the Titlebar indicator.
export const onDownloadProgress = (cb: (p: any) => void) =>
	listen('download-progress', (e) => cb(e.payload));
export const onDownloadComplete = (cb: (p: any) => void) =>
	listen('download-complete', (e) => cb(e.payload));
export const onDownloadError = (cb: (p: any) => void) =>
	listen('download-error', (e) => cb(e.payload));
export const onDownloadCancelled = (cb: (p: any) => void) =>
	listen('download-cancelled', (e) => cb(e.payload));
export const downloadPlaylist = (id: string) =>
	invoke<{ ok: boolean; total: number; skipped: number; downloaded: number; failed: number }>(
		'download_playlist',
		{ id }
	);
/** Auto-offline: walk Liked Music once and fetch anything missing from the offline catalogue.
 *  Errors when the setting is off. */
export const autoOfflineSync = () =>
	invoke<{ ok: boolean; total: number; skipped: number; downloaded: number; failed: number }>(
		'auto_offline_sync'
	);

// --- listen history (local play diary) --------------------------------------------------------
export interface HistoryEntry {
	/** Unix seconds of the play. */
	playedAt: number;
	song: SongItem;
}
export const getHistory = (limit?: number) => invoke<HistoryEntry[]>('get_history', { limit });
export const clearHistory = () => invoke<void>('clear_history');

// --- audio visualizer (Rust emits `setting-changed` when toggled) --------------------------------
/** One published release: the GitHub release description, verbatim markdown. */
export interface ReleaseNote {
	version: string;
	/** `YYYY-MM-DD` */
	date: string;
	body: string;
}

/** Changelog for Settings > About, from the GitHub releases API (cached in Rust per run). */
export const releaseNotes = () => invoke<ReleaseNote[]>('release_notes');
/** False on Linux builds that aren't the AppImage (.rpm, the AUR package): they update through the
 *  package manager, so the UI offers a download link instead of an install button. */
export const canSelfUpdate = () => invoke<boolean>('can_self_update');
/** Open an http(s) link in the real browser, never in the webview itself. */
export const openExternal = (url: string) => invoke<void>('open_external', { url });

// --- auth (context/15) ---------------------------------------------------------------------
export const getAccount = () => invoke<Account>('get_account');
export const getAccountIdentities = () =>
	invoke<AccountIdentity[]>('get_account_identities');
export const switchAccount = (selectionKey: string) =>
	invoke<Account>('switch_account', { selectionKey });
export const signOut = () => invoke<void>('sign_out');
/** Open the in-app Google sign-in webview (context/15 Path A). Result arrives via onAuthChanged. */
export const loginWebview = () => invoke<void>('login_webview');

// --- mini player (Rust mini.rs) ---------------------------------------------------------------
/** Hide the app to the tray and open the mini player widget (a second window running this same SPA). */
export const openMini = () => invoke<void>('open_mini');
/** Close the widget and bring the app back. */
export const closeMini = () => invoke<void>('close_mini');

// --- hi-res cover art (Rust art.rs) ------------------------------------------------------------
export const getHighresArt = (artist: string, title: string) =>
	invoke<string | null>('get_highres_art', { artist, title });

// --- Spotify Canvas (Rust canvas.rs, #8) --------------------------------------------------
export const getCanvas = (artist: string, title: string) =>
	invoke<string | null>('get_canvas', { artist, title });

// --- yt-dlp fallback (Rust ytdlp.rs) -----------------------------------------------------------------
export interface YtdlpInfo {
	enabled: boolean;
	installed: boolean;
	last_error: string | null;
}
export const ytdlpInfo = () => invoke<YtdlpInfo>('ytdlp_info');
export const ytdlpInstallNow = () => invoke<void>('ytdlp_install_now');

// --- browse / library (context/08) ---------------------------------------------------------
/** `params` is a `HomeChip.params` token — omit for the unfiltered feed. */
export const getHome = (params?: string) => invoke<HomePage>('get_home', { params });
export const getHomeMore = (token: string) => invoke<HomePage>('get_home_more', { token });
export const getLibrary = () => invoke<BrowseItem[]>('get_library');
export const getLibraryAlbums = () => invoke<BrowseItem[]>('get_library_albums');
export const getLibraryArtists = () => invoke<BrowseItem[]>('get_library_artists');
export const getPlaylist = (id: string) => invoke<PlaylistPage>('get_playlist', { id });
export const getPlaylistMore = (token: string) =>
	invoke<PlaylistContinuation>('get_playlist_more', { token });
/**
 * `start`: the clicked track index, or `null` for "just play it" (random opener under shuffle).
 * `sourceId`: the page's playlist/album playlist id — makes autoplay continue with that
 * context's radio (omit to fall back to song radio seeded from the queue's last track).
 * `sourceName`: the page title, for the queue panel's "Next from" header.
 * `shuffle`: turn shuffle on for this queue — pass items in their real order, Rust shuffles.
 */
export const playPlaylist = (
	items: SongItem[],
	start: number | null,
	sourceId?: string,
	sourceName?: string,
	shuffle?: boolean,
	continuation?: string
) => invoke<void>('play_playlist', { items, start, sourceId, sourceName, shuffle, continuation });
/**
 * Start a radio: an endless YouTube-generated queue seeded on this item. `id` is the videoId
 * (song) or browseId/playlistId (everything else) — Rust resolves it to a radio playlist, so the
 * UI never builds one. `name` titles the queue ("<name> Radio").
 *
 * A song radio on the track that's already playing splices in behind it (no re-buffer); every
 * other case replaces the queue. Rejects when YouTube has no radio for the item.
 */
export const startRadio = (kind: 'song' | 'artist' | 'album' | 'playlist', id: string, name?: string) =>
	invoke<void>('start_radio', { kind, id, name });
export const getAlbum = (id: string) => invoke<AlbumPage>('get_album', { id });
export const getArtist = (id: string) => invoke<ArtistPage>('get_artist', { id });
export const getBrowseGrid = (id: string, params?: string) =>
	invoke<BrowseItem[]>('get_browse_grid', { id, params });
/** Similar songs to a track — the playlist page's "More like this" shelf (read-only). */
export const getSimilarSongs = (videoId: string, limit?: number) =>
	invoke<SongItem[]>('get_similar_songs', { videoId, limit });

// --- local music (local.rs) ------------------------------------------------------------------
/** Rescan the watched folders. Cheap when nothing changed (one stat per file). */
export const getLocalLibrary = () => invoke<LocalLibrary>('get_local_library');
export const addLocalFolder = (path: string) => invoke<LocalLibrary>('add_local_folder', { path });
export const removeLocalFolder = (path: string) =>
	invoke<LocalLibrary>('remove_local_folder', { path });

// --- write actions (context/01 ✎) ----------------------------------------------------------
export const like = (videoId: string, liked: boolean) => invoke<void>('like', { videoId, liked });
export const addToPlaylist = (playlistId: string, videoId: string) =>
	invoke<boolean>('add_to_playlist', { playlistId, videoId });
export const removeFromPlaylist = (playlistId: string, videoId: string, setVideoId: string) =>
	invoke<void>('remove_from_playlist', { playlistId, videoId, setVideoId });
export const createPlaylist = (title: string) => invoke<string>('create_playlist', { title });
/** Name / description / visibility, from the "Edit playlist" dialog. Leave a field out and
 *  YouTube is never told about it, so an untouched one can't be overwritten. */
export const editPlaylistDetails = (
	playlistId: string,
	changes: { name?: string; description?: string; public?: boolean }
) => invoke<void>('edit_playlist_details', { playlistId, ...changes });
/** Custom playlist artwork. `path` is a file the user picked; `null` drops it. Answers where the
 *  local copy went, and on a removal the thumbnail YouTube rebuilt from the tracks (that one is
 *  worth waiting for: YouTube's own thumbnail is the cover being removed until it lands). */
export const setPlaylistCover = (playlistId: string, path: string | null) =>
	invoke<{ cover?: string; thumbnail?: string }>('set_playlist_cover', { playlistId, path });
export const deletePlaylist = (playlistId: string) =>
	invoke<void>('delete_playlist', { playlistId });
export const subscribe = (channelId: string, subscribed: boolean) =>
	invoke<void>('subscribe', { channelId, subscribed });
/** Save an album to the library (or remove it). `playlistId` is `AlbumPage.playlistId`. */
export const setAlbumSaved = (playlistId: string, saved: boolean) =>
	invoke<void>('set_album_saved', { playlistId, saved });

// --- events (context/11). Each returns an unlisten fn; call it on component teardown. --------
export const onNowPlaying = (cb: (n: NowPlaying) => void): Promise<UnlistenFn> =>
	listen<NowPlaying>('now-playing', (e) => cb(e.payload));
export const onQueueChanged = (cb: (q: QueueState) => void): Promise<UnlistenFn> =>
	listen<QueueState>('queue-changed', (e) => cb(e.payload));
export const onPosition = (cb: (p: number) => void): Promise<UnlistenFn> =>
	listen<{ position: number }>('position', (e) => cb(e.payload.position));
export const onDuration = (cb: (d: number) => void): Promise<UnlistenFn> =>
	listen<{ duration: number }>('duration', (e) => cb(e.payload.duration));
/** Echo of every `set_volume`, so a second window's slider can't drift from what you hear. */
export const onVolume = (cb: (v: number) => void): Promise<UnlistenFn> =>
	listen<number>('volume', (e) => cb(e.payload));
/** The Rust-side sleep timer hit zero (or the current song ended) — playback was paused. */
export const onSleepTimerFired = (cb: () => void): Promise<UnlistenFn> =>
	listen('sleep-timer-fired', (e) => cb());
export const onPlaybackState = (cb: (s: 'playing' | 'paused') => void): Promise<UnlistenFn> =>
	listen<'playing' | 'paused'>('playback-state', (e) => cb(e.payload));
export const onPlaybackError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<{ message: string }>('playback-error', (e) => cb(e.payload.message));
export const onPlaybackNotice = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<{ message: string }>('playback-notice', (e) => cb(e.payload.message));
/** Custom playlist artwork applied here but refused by YouTube Music (it syncs in the background,
 *  so the failure lands long after the picker closed). */
export const onCoverError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<{ message: string }>('cover-error', (e) => cb(e.payload.message));
export const onAuthChanged = (cb: (a: Account) => void): Promise<UnlistenFn> =>
	listen<Account>('auth-changed', (e) => cb(e.payload));
export const onAccountSelectionRequired = (cb: () => void): Promise<UnlistenFn> =>
	listen('account-selection-required', () => cb());
/**
 * Local music disappeared from disk. Fired when a play attempt finds nothing there, carrying the
 * song (and album, if that emptied it) so every view holding those ids can drop them at once.
 */
export const onLocalChanged = (cb: (removed: string[]) => void): Promise<UnlistenFn> =>
	listen<{ removed: string[] }>('local-changed', (e) => cb(e.payload.removed));
/**
 * The real like state for the current track, resolved in Rust when the `now-playing` row carried
 * none (search-sourced tracks). Fires once per unknown track — not on every position tick.
 */
export const onLikeStatus = (cb: (videoId: string, liked: boolean) => void): Promise<UnlistenFn> =>
	listen<{ videoId: string; liked: boolean }>('like-status', (e) =>
		cb(e.payload.videoId, e.payload.liked)
	);

/** videoId → play count over the trailing On Repeat window. Feeds the playlist page's "Most
 *  played" sort; absent keys simply mean the track hasn't been played this month. */
export const getPlayCounts = (): Promise<Record<string, number>> =>
	invoke<Record<string, number>>('play_counts');

export const onLoginError = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<string>('login-error', (e) => cb(e.payload));
export const onLoginDone = (cb: () => void): Promise<UnlistenFn> =>
	listen('login-done', () => cb());

// --- lyrics ---------------------------------------------------------------------------------
/** A single timed word within a line - drives the karaoke sweep. Only present on word-level
 *  providers (Boidu), so the UI falls back to whole-line highlighting elsewhere. */
export interface LyricWord {
	text: string;
	start_ms: number;
	end_ms: number;
}
export interface LyricLine {
	/** Start cue in milliseconds; present ⇔ the line is synced. */
	time_ms?: number;
	/** End cue in milliseconds (karaoke needs it to know when the last word stops). */
	end_time_ms?: number;
	text: string;
	/** Per-word timings when the provider returned them. */
	words?: LyricWord[];
	/** Line translation (Netease), when available. */
	translation?: string;
}
export interface Lyrics {
	/** Attribution for the panel footer ("LRCLIB", "Source: Musixmatch", …). */
	source: string;
	synced: boolean;
	instrumental: boolean;
	lines: LyricLine[];
}
/** Cached on the Rust side (provider chain: LRCLIB → YT Music). `null` = none found. */
export const getLyrics = (args: {
	videoId: string;
	title: string;
	artists: string;
	album?: string;
	duration?: number;
}) => invoke<Lyrics | null>('get_lyrics', args);

/** Per-song offset (ms) persisted in Rust `lyric_offsets`. */
export const getLyricOffset = (videoId: string) => invoke<number>('get_lyric_offset', { videoId });
export const setLyricOffset = (videoId: string, offset_ms: number) => invoke<void>('set_lyric_offset', { videoId, offsetMs: offset_ms });
/** Unison vote/report (POST /lyrics/vote semantics). */
export const lyricsVote = (videoId: string, source: string, vote: number) => invoke<void>('lyrics_vote', { videoId, source, vote });
export const lyricsReport = (videoId: string, source: string, reason: string) => invoke<void>('lyrics_report', { videoId, source, reason });
/** Translate via translate.googleapis (44 langs) + romanize (kana→romaji). */
export const translateLyrics = (text: string, target: string) => invoke<string>('translate_lyrics', { text, target });
export const romanizeLyrics = (text: string) => invoke<string>('romanize_lyrics', { text });
/** Video Sync toggle (mpv `vo=libmpv` + `vid` switch). */
export const setVideoSync = (enabled: boolean) => invoke<void>('set_video_sync', { enabled });
export const getVideoSync = () => invoke<boolean>('get_video_sync');

// --- Last.fm scrobbling ---------------------------------------------------------------------
export interface LastfmState {
	connected: boolean;
	username?: string | null;
	/** Set when a connect attempt failed (timeout, network, rejected) — show it as a toast. */
	error?: string | null;
}
export const lastfmStatus = () => invoke<LastfmState>('lastfm_status');
/** Opens the browser auth flow; the outcome arrives via onLastfmState, not this promise. */
export const lastfmConnect = () => invoke<void>('lastfm_connect');
/** Also cancels an in-flight connect (the auth poll checks and bails). */
export const lastfmDisconnect = () => invoke<void>('lastfm_disconnect');
export const onLastfmState = (cb: (s: LastfmState) => void): Promise<UnlistenFn> =>
	listen<LastfmState>('lastfm-state', (e) => cb(e.payload));

// --- Listen Together (context/19) -----------------------------------------------------------
export interface LtUser {
	user_id: string;
	username: string;
	is_host: boolean;
	is_connected: boolean;
}
export interface LtTrack {
	id: string;
	title: string;
	artist: string;
	thumbnail?: string | null;
	duration_ms: number;
	/** Name of the guest who added this track to the session queue. */
	queued_by?: string | null;
}
export interface LtPendingJoin {
	userId: string;
	username: string;
}
export interface LtSuggestion {
	id: string;
	from_user_id: string;
	from_username: string;
	track: LtTrack;
}
export interface LtState {
	status: 'disconnected' | 'connecting' | 'connected';
	role: 'none' | 'host' | 'guest';
	/** Asked to create/join and awaiting the room (host approval) — show a waiting state. */
	requesting: boolean;
	roomCode: string | null;
	myId: string | null;
	serverUrl: string;
	users: LtUser[];
	currentTrack: LtTrack | null;
	queue: LtTrack[];
	pendingJoins: LtPendingJoin[];
	suggestions: LtSuggestion[];
}

export const ltGetState = () => invoke<LtState>('lt_get_state');
export const ltSetServerUrl = (url: string) => invoke<void>('lt_set_server_url', { url });
export const ltCreateRoom = (username: string) => invoke<void>('lt_create_room', { username });
export const ltJoinRoom = (code: string, username: string) =>
	invoke<void>('lt_join_room', { code, username });
export const ltLeave = () => invoke<void>('lt_leave');
export const ltApproveJoin = (userId: string) => invoke<void>('lt_approve_join', { userId });
export const ltRejectJoin = (userId: string) => invoke<void>('lt_reject_join', { userId });
export const ltKick = (userId: string) => invoke<void>('lt_kick', { userId });
export const ltTransferHost = (userId: string) => invoke<void>('lt_transfer_host', { userId });
export const ltApproveSuggestion = (id: string) => invoke<void>('lt_approve_suggestion', { id });
export const ltRejectSuggestion = (id: string) => invoke<void>('lt_reject_suggestion', { id });
export const ltRequestSync = () => invoke<void>('lt_request_sync');

export const onLtState = (cb: (s: LtState) => void): Promise<UnlistenFn> =>
	listen<LtState>('lt-state', (e) => cb(e.payload));
export const onLtNotice = (cb: (msg: string) => void): Promise<UnlistenFn> =>
	listen<string>('lt-notice', (e) => cb(e.payload));
// Gamepad events (Rust poller → UI). The payload is an action string the player maps to
// the same actions the keyboard shortcuts trigger. Works while the app is backgrounded.
export const onGamepad = (cb: (action: string) => void): Promise<UnlistenFn> =>
	listen<string>('gamepad', (e) => cb(e.payload));

// --- Crossfade / best mix -------------------------------------------------------
export interface CrossfadeState { secs: number; mode: string; best_mix: boolean }
export const getCrossfade = () => invoke<CrossfadeState>('get_crossfade');
export const setCrossfade = (secs: number, mode: string) => invoke<void>('set_crossfade', { secs, mode });
export const setBestMix = (on: boolean) => invoke<void>('set_best_mix', { on });
// --- Remote LAN QR (#5) ---------------------------------------------------------
export const getLanUrl = () => invoke<string>('get_lan_url');
export const getRemoteToken = () => invoke<string>('get_remote_token');
export const pairRemote = (token: string) => invoke<boolean>('pair_remote', { token });
/** The LAN pairing URL as a scannable QR code (SVG markup from Rust). */
export const getRemoteQr = () => invoke<string>('get_remote_qr');

// --- Artist Packs (#9) ---------------------------------------------------------
export interface ArtistPack {
    id: string;
    name: string;
    version: string;
    description?: string | null;
    artist_ids: string[];
    aliases: string[];
    layout?: string | null;
    style_css?: string | null;
    installed_at: number;
    thumbnail?: string | null;
}
export interface ArtistPackIndexEntry {
    id: string;
    name: string;
    version: string;
    description?: string | null;
    artist_ids: string[];
    aliases: string[];
    url: string;
    thumbnail?: string | null;
}
export interface ArtistPackIndex { packs: ArtistPackIndexEntry[]; }
export const listArtistPacks = () => invoke<ArtistPack[]>('list_artist_packs');
export const getArtistPack = (id: string) => invoke<ArtistPack | null>('get_artist_pack', { id });
export const removeArtistPack = (id: string) => invoke<void>('remove_artist_pack', { id });
export const installArtistPack = (url: string) => invoke<ArtistPack>('install_artist_pack', { url });
export const installArtistPackZip = (path: string) => invoke<ArtistPack>('install_artist_pack_zip', { path });
export const fetchArtistPacksIndex = () => invoke<ArtistPackIndex>('fetch_artist_packs_index');
export const onArtistPacksIndex = (cb: (idx: ArtistPackIndex) => void) => listen<ArtistPackIndex>('artist-packs-index', (e) => cb(e.payload));

