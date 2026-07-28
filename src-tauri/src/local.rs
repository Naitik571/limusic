//! Local music: scan folders on disk, read tags, and hand mpv a file path instead of a stream URL.
//!
//! Everything local rides the surfaces that already exist. A file is a `SongItem` whose `video_id`
//! is `LOCAL:<absolute path>`, an album is a `BrowseItem` with id `LOCALALBUM:<key>`, and both are
//! intercepted before anything YouTube-shaped runs: `AppState::resolve` for playback,
//! `commands::get_album` for the album page. So queueing, gapless, shuffle, media keys, Shortcuts,
//! and drag-and-drop work on local music without knowing it exists — and work offline, because
//! nothing on this path touches the network.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use innertube::{AlbumPage, BrowseItem, SongItem};
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};

use crate::db::{Db, LocalTrack};

/// A local file's synthetic videoId: this prefix + the absolute path.
pub const SONG_PREFIX: &str = "LOCAL:";
/// A local album's synthetic browseId: this prefix + `LocalTrack::album_key`.
pub const ALBUM_PREFIX: &str = "LOCALALBUM:";
/// Where the folder list lives (JSON array of absolute paths).
const FOLDERS_SETTING: &str = "local_folders";

/// Extensions we pick up. Playback itself is mpv, which decodes far more than this — the list is
/// only about what a music folder plausibly holds, so a scan doesn't try to probe every stray file.
const AUDIO_EXT: [&str; 15] = [
    "mp3", "flac", "m4a", "m4b", "aac", "ogg", "oga", "opus", "wav", "wma", "aiff", "aif", "ape",
    "wv", "mka",
];
/// Cover images sitting next to the tracks, in preference order (used when nothing is embedded).
const COVER_FILES: [&str; 6] =
    ["cover.jpg", "cover.png", "folder.jpg", "folder.png", "front.jpg", "album.jpg"];

/// How deep a folder tree is walked. Music/Artist/Album/Disc 1 is four; ten is room to spare
/// without letting a symlink cycle spin forever.
const MAX_DEPTH: usize = 10;

pub fn is_local_song(video_id: &str) -> bool {
    video_id.starts_with(SONG_PREFIX)
}

/// The file behind a local videoId, or `None` when this isn't one.
pub fn song_path(video_id: &str) -> Option<&str> {
    video_id.strip_prefix(SONG_PREFIX)
}

/// What the Local tab renders. `removed` is what vanished from disk since the last scan — the UI
/// prunes those ids out of Shortcuts and recents immediately instead of leaving dead tiles.
#[derive(Debug, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalLibrary {
    pub folders: Vec<String>,
    pub albums: Vec<BrowseItem>,
    pub songs: Vec<SongItem>,
    pub removed: Vec<String>,
}

// --- folders ------------------------------------------------------------------------------------

pub fn folders(db: &Db) -> Vec<String> {
    db.get_setting(FOLDERS_SETTING)
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
}

fn set_folders(db: &Db, folders: &[String]) {
    db.set_setting(FOLDERS_SETTING, &serde_json::to_string(folders).unwrap_or_else(|_| "[]".into()));
}

/// Add a folder (no-op if it's already watched, or nested inside one that is).
pub fn add_folder(db: &Db, path: String) {
    let mut list = folders(db);
    if list.iter().any(|f| path == *f || path.starts_with(&format!("{f}/"))) {
        return;
    }
    // A new parent supersedes the children already in the list, so files aren't scanned twice.
    list.retain(|f| !f.starts_with(&format!("{path}/")));
    list.push(path);
    set_folders(db, &list);
}

pub fn remove_folder(db: &Db, path: &str) {
    let mut list = folders(db);
    list.retain(|f| f != path);
    set_folders(db, &list);
}

// --- scanning -----------------------------------------------------------------------------------

/// Re-read the watched folders and return the whole library. Files whose mtime hasn't moved keep
/// their stored tags, so a rescan of an unchanged collection is a stat() per file.
///
/// Blocking (disk IO + tag parsing) — call it from `spawn_blocking`.
pub fn scan(db: &Db, covers_dir: &Path) -> LocalLibrary {
    let before: HashSet<String> = db.local_tracks(None).iter().map(album_id_of).collect();
    let known = db.local_mtimes();
    let mut found: HashSet<String> = HashSet::new();

    for folder in folders(db) {
        walk(Path::new(&folder), 0, &mut |file| {
            let path = file.to_string_lossy().to_string();
            let mtime = mtime_of(file);
            found.insert(path.clone());
            if known.get(&path) == Some(&mtime) {
                return; // unchanged since the last scan
            }
            match read_track(file, &path, mtime, covers_dir) {
                Some(t) => db.put_local_track(&t),
                None => tracing::debug!(path, "unreadable audio file — skipped"),
            }
        });
    }

    let gone: Vec<String> = known.keys().filter(|p| !found.contains(*p)).cloned().collect();
    if !gone.is_empty() {
        tracing::info!(count = gone.len(), "local files disappeared — dropped from the library");
        db.delete_local_tracks(&gone);
    }

    let tracks = db.local_tracks(None);
    let after: HashSet<String> = tracks.iter().map(album_id_of).collect();
    // Ids the UI may still be showing: the deleted songs, plus albums that lost their last track.
    let mut removed: Vec<String> = gone.into_iter().map(|p| format!("{SONG_PREFIX}{p}")).collect();
    removed.extend(before.difference(&after).cloned());

    LocalLibrary { folders: folders(db), albums: albums_of(&tracks), songs: songs_of(&tracks), removed }
}

/// Recurse into `dir`, calling `on_file` for every audio file. Errors (permissions, a folder that
/// was unplugged) are skipped rather than failing the scan.
fn walk(dir: &Path, depth: usize, on_file: &mut impl FnMut(&Path)) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        // `file_type` doesn't follow symlinks, so a link back up the tree is never descended into.
        match entry.file_type() {
            Ok(t) if t.is_dir() => walk(&path, depth + 1, on_file),
            Ok(t) if t.is_file() && is_audio(&path) => on_file(&path),
            _ => {}
        }
    }
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| AUDIO_EXT.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn mtime_of(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Read one file's tags. Missing tags fall back to the filename and the parent folder, and a file
/// lofty can't parse at all is still listed from its filename — mpv plays far more formats than
/// any tag reader understands, and a track that silently never appears is worse than one with a
/// thin label.
fn read_track(file: &Path, path: &str, mtime: i64, covers_dir: &Path) -> Option<LocalTrack> {
    let tagged = lofty::probe::Probe::open(file).ok().and_then(|p| p.read().ok());
    let duration_secs = tagged.as_ref().map(|t| t.properties().duration().as_secs() as i64).unwrap_or(0);
    let tag = tagged.as_ref().and_then(|t| t.primary_tag().or_else(|| t.first_tag()));

    let stem = || file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let title = tag.and_then(|t| t.title().map(|s| s.to_string())).filter(|s| !s.is_empty());
    let artist = tag.and_then(|t| t.artist().map(|s| s.to_string())).filter(|s| !s.is_empty());
    let album = tag.and_then(|t| t.album().map(|s| s.to_string())).filter(|s| !s.is_empty());
    // The album artist is what groups a compilation into one album instead of one per track.
    let album_artist = tag
        .and_then(|t| t.get_string(&ItemKey::AlbumArtist).map(|s| s.to_string()))
        .filter(|s| !s.is_empty());

    let dir = file.parent();
    let folder_name =
        dir.and_then(|d| d.file_name()).map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let title = title.unwrap_or_else(stem);
    let artist = artist.unwrap_or_else(|| "Unknown artist".into());
    let album = album.unwrap_or(folder_name);
    let key_artist = album_artist.unwrap_or_else(|| artist.clone());

    let album_key = album_key(&key_artist, &album, dir);
    let cover = cover_for(&album_key, tag, dir, covers_dir);

    Some(LocalTrack {
        path: path.to_owned(),
        title,
        artist,
        album,
        album_key,
        track_no: tag.and_then(|t| t.track()).unwrap_or(0) as i64,
        duration_secs,
        cover,
        mtime,
    })
}

/// Forget a single file the moment it turns out to be gone (a play attempt found nothing there).
/// Returns the ids the UI might still be showing: the song, plus its album when that was the last
/// track left in it. Same shape as a scan's `removed`, so both feed one prune on the UI side.
pub fn forget_missing(db: &Db, path: &str) -> Vec<String> {
    let key = db.local_album_key(path);
    db.delete_local_tracks(&[path.to_owned()]);
    let mut ids = vec![format!("{SONG_PREFIX}{path}")];
    if let Some(k) = key {
        if db.local_tracks(Some(&k)).is_empty() {
            ids.push(format!("{ALBUM_PREFIX}{k}"));
        }
    }
    ids
}

/// A stable, readable album id: `artist--album`, sanitized to a safe filename (it doubles as the
/// extracted cover's name). Deliberately not a hash — this id is persisted in Shortcuts, so it has
/// to survive across releases. Tracks with no album at all fall back to their folder.
fn album_key(artist: &str, album: &str, dir: Option<&Path>) -> String {
    let raw = if album.is_empty() {
        dir.map(|d| d.to_string_lossy().to_string()).unwrap_or_else(|| "unknown".into())
    } else {
        format!("{artist}--{album}")
    };
    let mut key: String = raw
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    key.truncate(80);
    key
}

/// The album's cover: the first embedded picture (written once into `covers_dir`), else an image
/// sitting next to the tracks. Extraction is skipped when the file is already there, so a rescan
/// doesn't rewrite artwork.
fn cover_for(
    album_key: &str,
    tag: Option<&lofty::tag::Tag>,
    dir: Option<&Path>,
    covers_dir: &Path,
) -> Option<String> {
    // The extension matters: the webview gets these over Tauri's asset protocol, which types the
    // response from the file name.
    for ext in ["jpg", "png"] {
        let cached = covers_dir.join(format!("{album_key}.{ext}"));
        if cached.exists() {
            return Some(cached.to_string_lossy().to_string());
        }
    }
    if let Some(pic) = tag.and_then(|t| t.pictures().first()) {
        let ext = match pic.mime_type() {
            Some(lofty::picture::MimeType::Png) => "png",
            _ => "jpg",
        };
        let out = covers_dir.join(format!("{album_key}.{ext}"));
        std::fs::create_dir_all(covers_dir).ok();
        if std::fs::write(&out, pic.data()).is_ok() {
            return Some(out.to_string_lossy().to_string());
        }
    }
    let dir = dir?;
    COVER_FILES
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
}

// --- shaping into the app's models ----------------------------------------------------------

fn album_id_of(t: &LocalTrack) -> String {
    format!("{ALBUM_PREFIX}{}", t.album_key)
}

pub fn to_song(t: &LocalTrack) -> SongItem {
    SongItem {
        video_id: format!("{SONG_PREFIX}{}", t.path),
        title: t.title.clone(),
        artists: t.artist.clone(),
        album: Some(t.album.clone()),
        album_id: Some(album_id_of(t)),
        // 0 means "the tag reader couldn't say"; mpv fills the real length in once it plays.
        duration: (t.duration_secs > 0).then(|| fmt_duration(t.duration_secs)),
        thumbnail: t.cover.clone(),
        ..Default::default()
    }
}

fn songs_of(tracks: &[LocalTrack]) -> Vec<SongItem> {
    tracks.iter().map(to_song).collect()
}

/// One card per album, ordered by artist then title (the order the DB already returns is by album).
fn albums_of(tracks: &[LocalTrack]) -> Vec<BrowseItem> {
    let mut counts: HashMap<&str, (usize, &LocalTrack)> = HashMap::new();
    for t in tracks {
        let e = counts.entry(t.album_key.as_str()).or_insert((0, t));
        e.0 += 1;
        // Prefer a track that actually has artwork as the card's face.
        if e.1.cover.is_none() && t.cover.is_some() {
            e.1 = t;
        }
    }
    let mut albums: Vec<BrowseItem> = counts
        .values()
        .map(|(n, t)| BrowseItem {
            kind: "album",
            id: album_id_of(t),
            title: t.album.clone(),
            subtitle: Some(format!("{} • {} song{}", t.artist, n, if *n == 1 { "" } else { "s" })),
            thumbnail: t.cover.clone(),
            duration: None,
        })
        .collect();
    albums.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    albums
}

/// The album page for a local album, in the same shape the YouTube one uses so the route is shared.
pub fn album_page(db: &Db, album_key: &str) -> AlbumPage {
    let tracks = db.local_tracks(Some(album_key));
    let total: i64 = tracks.iter().map(|t| t.duration_secs).sum();
    let first = tracks.first();
    AlbumPage {
        title: first.map(|t| t.album.clone()),
        artist: first.map(|t| t.artist.clone()),
        artist_id: None,
        artist_runs: Vec::new(),
        artist_thumbnail: None,
        subtitle: Some("On this device".into()),
        second_subtitle: Some(format!(
            "{} song{} • {}",
            tracks.len(),
            if tracks.len() == 1 { "" } else { "s" },
            fmt_duration(total)
        )),
        description: None,
        thumbnail: first.and_then(|t| t.cover.clone()),
        items: songs_of(&tracks),
        continuation: None,
        // No YouTube playlist behind it: no radio to seed, nothing to save to the library.
        playlist_id: None,
        in_library: false,
    }
}

/// Seconds → "3:47" / "1:08:20".
fn fmt_duration(secs: i64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Everything the player needs for a local file. The "stream url" is just the path — mpv opens it
/// directly, so playback works with no network at all. Errs when the file is gone (the user
/// deleted it since the scan): the queue skips it like any other unplayable track.
pub fn playback_data(video_id: &str, path: &str) -> Result<crate::orchestrator::PlaybackData, ()> {
    if !Path::new(path).is_file() {
        return Err(());
    }
    Ok(crate::orchestrator::PlaybackData {
        video_id: video_id.to_owned(),
        stream_url: path.to_owned(),
        itag: 0,
        headers: HashMap::new(),
        // Never expires, and never enters the URL cache (see `AppState::resolve`).
        expires_in_seconds: i64::MAX / 2,
        // No loudness metadata in the general case; mpv plays the file as mastered.
        loudness_db: None,
        playback_url: None,
        title: None,
        artists: None,
        duration: None,
        thumbnail: None,
        stream_client: "local".to_owned(),
    })
}

/// The covers directory, alongside the SQLite file (not inside the audio cache — "Clear caches"
/// must not wipe artwork that only a full re-tag would regenerate).
pub fn covers_dir(app: &tauri::AppHandle) -> PathBuf {
    use tauri::Manager;
    app.path().app_data_dir().unwrap_or_else(|_| std::env::temp_dir()).join("covers")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn album_key_is_stable_and_filename_safe() {
        let k = album_key("Daft Punk", "Discovery / Deluxe", None);
        assert_eq!(k, "daft-punk--discovery---deluxe");
        assert!(k.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        // Same album, two tracks → one key, whatever the per-track artist says.
        assert_eq!(album_key("Daft Punk", "Discovery / Deluxe", None), k);
    }

    #[test]
    fn durations_format_like_the_rest_of_the_app() {
        assert_eq!(fmt_duration(227), "3:47");
        assert_eq!(fmt_duration(4100), "1:08:20");
        assert_eq!(fmt_duration(5), "0:05");
    }

    #[test]
    fn folders_ignore_nesting_in_both_directions() {
        let db = Db::open(std::path::Path::new(":memory:")).unwrap();
        add_folder(&db, "/music".into());
        add_folder(&db, "/music/rock".into()); // already covered by the parent
        assert_eq!(folders(&db), vec!["/music".to_string()]);
        add_folder(&db, "/other/jazz".into());
        add_folder(&db, "/other".into()); // supersedes the child
        assert_eq!(folders(&db), vec!["/music".to_string(), "/other".to_string()]);
        remove_folder(&db, "/music");
        assert_eq!(folders(&db), vec!["/other".to_string()]);
    }

    #[test]
    fn a_scan_reports_what_left_the_disk() {
        let db = Db::open(std::path::Path::new(":memory:")).unwrap();
        let dir = std::env::temp_dir().join("limusic-local-scan-test");
        std::fs::create_dir_all(&dir).unwrap();
        add_folder(&db, dir.to_string_lossy().to_string());
        // Two tracks the library "knows" about; neither file exists any more.
        for (n, key) in [("a", "band--one"), ("b", "band--two")] {
            db.put_local_track(&LocalTrack {
                path: dir.join(format!("{n}.mp3")).to_string_lossy().to_string(),
                title: n.into(),
                artist: "Band".into(),
                album: n.into(),
                album_key: key.into(),
                track_no: 1,
                duration_secs: 100,
                cover: None,
                mtime: 1,
            });
        }

        let lib = scan(&db, &dir.join("covers"));
        assert!(lib.songs.is_empty() && lib.albums.is_empty(), "the rows are gone from the library");
        assert_eq!(lib.removed.len(), 4, "two songs and two albums are reported as removed");
        assert!(lib.removed.iter().any(|id| id.ends_with("a.mp3")), "the song id the UI knows");
        assert!(
            lib.removed.contains(&format!("{ALBUM_PREFIX}band--one")),
            "and the album id, so its Shortcuts tile can go too"
        );
        assert!(scan(&db, &dir.join("covers")).removed.is_empty(), "a second scan reports nothing");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn forgetting_one_file_only_reports_the_album_when_it_empties() {
        let db = Db::open(std::path::Path::new(":memory:")).unwrap();
        let track = |path: &str, key: &str| LocalTrack {
            path: path.into(),
            title: path.into(),
            artist: "Band".into(),
            album: "Album".into(),
            album_key: key.into(),
            track_no: 1,
            duration_secs: 10,
            cover: None,
            mtime: 1,
        };
        db.put_local_track(&track("/m/a.mp3", "band--album"));
        db.put_local_track(&track("/m/b.mp3", "band--album"));

        assert_eq!(
            forget_missing(&db, "/m/a.mp3"),
            vec!["LOCAL:/m/a.mp3".to_string()],
            "the album still has a track, so only the song id is reported"
        );
        assert_eq!(
            forget_missing(&db, "/m/b.mp3"),
            vec!["LOCAL:/m/b.mp3".to_string(), "LOCALALBUM:band--album".to_string()],
            "the last track takes the album with it"
        );
        assert!(db.local_tracks(None).is_empty(), "and both rows are gone");
        assert!(forget_missing(&db, "/m/a.mp3").len() == 1, "forgetting twice is harmless");
    }

    #[test]
    fn a_missing_file_is_not_playable() {
        assert!(playback_data("LOCAL:/nope/gone.mp3", "/nope/gone.mp3").is_err());
    }
}

