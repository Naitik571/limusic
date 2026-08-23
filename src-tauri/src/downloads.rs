//! Offline downloads. A downloaded track is the same audio yt-dlp/InnerTube would have streamed,
//! saved to disk so it plays without a network (and without burning a fresh stream-URL resolve).
//! The resolver ([`crate::state::AppState::resolve`]) consults the download catalogue first and,
//! when one exists, plays the local file path instead of the network URL — so "download" quietly
//! upgrades every later play of that track to offline.
//!
//! Design notes:
//! - Audio bytes come straight from the resolved `stream_url` (with the same headers WebAudio would
//!   send), written atomically to `<download_dir>/<video_id>.<format>`. No re-encode, so quality is
//!   whatever the chosen stream itag is.
//! - The dedicated settings (`download_dir`, `download_quality`, `download_format`,
//!   `use_offline`) live in the KV store like every other setting; see `commands.rs`.
//! - Progress rides the Tauri event bus so a thin UI bar can track it without polling. We emit
//!   `download-progress` with a real 0–100 percentage as bytes stream in, `download-complete` on
//!   success, and `download-error` on any failure (with the message) so the UI can toast it.
//! - Resolution mirrors playback: try the orchestrator, then fall back to yt-dlp (the same net
//!   playback uses) so a track that plays also downloads.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::{Db, DownloadTrack};
use crate::orchestrator::Orchestrator;
use crate::state::AppState;
use innertube::AudioQuality;

// --- cancellation ------------------------------------------------------------------------------

/// Live downloads keyed by video id → cancel flag. `cancel_download` flips the flag; the writer
/// checks it between chunks, deletes its `.part` file and reports `download-cancelled`. A flag
/// (checked cooperatively) rather than a task abort, so the writer always gets to clean up.
static ACTIVE: std::sync::OnceLock<Mutex<HashMap<String, Arc<AtomicBool>>>> =
    std::sync::OnceLock::new();

/// Cancel requests for tracks that haven't started yet (queued behind the concurrency window in
/// a batch). Drained by whichever side sees the id first — the batch feeder or the task itself.
static REQUESTED: std::sync::OnceLock<Mutex<std::collections::HashSet<String>>> =
    std::sync::OnceLock::new();

fn active() -> &'static Mutex<HashMap<String, Arc<AtomicBool>>> {
    ACTIVE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn requested() -> &'static Mutex<std::collections::HashSet<String>> {
    REQUESTED.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

/// Set when `cancel_all_downloads` fires. A batch (`download_many`) checks it before feeding
/// each new track to the pool, so a pending 500-track playlist stops immediately instead of
/// finishing every not-yet-started entry. Cleared at the start of each batch.
static BATCH_STOP: AtomicBool = AtomicBool::new(false);

/// Request cancellation of one download — in-flight or still queued. Always succeeds; the
/// relevant side observes the request at its next checkpoint.
pub fn cancel_download(video_id: &str) -> bool {
    if let Some(flag) = active().lock().unwrap().get(video_id) {
        flag.store(true, Ordering::SeqCst);
        return true;
    }
    requested().lock().unwrap().insert(video_id.to_owned());
    false
}

/// Cancel everything: every in-flight track plus any batch that hasn't started them yet.
pub fn cancel_all_downloads() -> usize {
    BATCH_STOP.store(true, Ordering::SeqCst);
    let mut n = 0;
    for flag in active().lock().unwrap().values() {
        flag.store(true, Ordering::SeqCst);
        n += 1;
    }
    n += requested().lock().unwrap().len();
    n
}

/// A resolved stream ready to download.
struct Stream {
    url: String,
    headers: Vec<(String, String)>,
    client: &'static str,
}

/// Where downloads live. A user-set `download_dir` wins; otherwise `<app_data>/downloads`.
pub fn download_dir(app: &AppHandle, db: &Db) -> PathBuf {
    if let Some(custom) = db.get_setting("download_dir") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("downloads")
}

fn download_quality(db: &Db) -> AudioQuality {
    match db.get_setting("download_quality").as_deref() {
        Some("LOW") => AudioQuality::Low,
        Some("AUTO") => AudioQuality::Auto,
        _ => AudioQuality::High,
    }
}

fn download_format(db: &Db) -> String {
    let f = db.get_setting("download_format").unwrap_or_default();
    if f == "opus" || f == "webm" {
        f
    } else {
        "m4a".to_string()
    }
}

/// Turn a free-form track name into a filesystem-safe filename component. Keeps unicode letters
/// (song titles are multilingual) but strips path separators, control chars, and the reserved
/// Windows characters so the file lands on disk instead of erroring.
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect::<String>()
        .trim()
        .to_string()
        .chars()
        .take(180)
        .collect()
}

// Static client pool to reuse connections (massive speedup on multi-track downloads). HTTP/2 is
// on because googlevideo serves it and the multiplexing + adaptive window measurably beats HTTP/1.1
// on a single large stream; connection reuse across the batch keeps the resolver warm.
static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();

fn client() -> &'static reqwest::Client {
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .tcp_nodelay(true)
            .pool_max_idle_per_host(8)
            .http2_adaptive_window(true)
            .build()
            .expect("reqwest client build")
    })
}

/// Resolve a stream URL the same way playback would: orchestrator first, yt-dlp as the net.
/// Returns `None` (and the caller reports the error) if both fail.
async fn resolve_stream(
    state: &Arc<AppState>,
    orchestrator: &Arc<Orchestrator>,
    video_id: &str,
) -> Option<Stream> {
    let quality = download_quality(&state.db);
    if let Ok(data) = orchestrator
        .resolve(video_id, quality, &state.disabled_clients())
        .await
    {
        return Some(Stream {
            url: data.stream_url,
            headers: data.headers.into_iter().collect(),
            client: "orchestrator",
        });
    }
    // Last-ditch net, identical to playback's fallback.
    if let Some(s) = state.ytdlp.resolve(video_id).await {
        return Some(Stream {
            url: s.url,
            headers: Vec::new(),
            client: "ytdlp",
        });
    }
    None
}

/// Safely append `ratebypass=yes` ONLY to googlevideo URLs that don't already have it.
/// This defeats YouTube's ~50-200 KB/s throttle without breaking signed URLs.
fn with_ratebypass(url: &str) -> String {
    if url.contains("googlevideo.com") && !url.contains("ratebypass=") {
        format!(
            "{}{}ratebypass=yes",
            url,
            if url.contains('?') { "&" } else { "?" }
        )
    } else {
        url.to_owned()
    }
}

/// Resolve + download one track's audio to disk, recording it in the catalogue.
///
/// Emits `download-progress` (real byte percentage), `download-complete`, or `download-error`.
/// Idempotent: a second call for an already-downloaded `video_id` is a no-op.
pub async fn download_track(
    app: &AppHandle,
    state: &Arc<AppState>,
    orchestrator: &Arc<Orchestrator>,
    video_id: &str,
    title: &str,
    artists: &str,
    album: Option<&str>,
    duration: i64,
    thumb: Option<&str>,
) -> Result<(), String> {
    if state.db.download_path(video_id).is_some() {
        return Ok(());
    }

    // Register before any work so a click on Cancel during resolve already lands. The guard
    // removes the entry on every exit path.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    active().lock().unwrap().insert(video_id.to_owned(), cancel_flag.clone());
    struct RemoveOnDrop<'a>(&'a str);
    impl Drop for RemoveOnDrop<'_> {
        fn drop(&mut self) {
            active().lock().unwrap().remove(self.0);
        }
    }
    let _remove = RemoveOnDrop(video_id);

    // A cancel that arrived while this track sat queued behind the batch window.
    if requested().lock().unwrap().remove(video_id) {
        let _ = app.emit(
            "download-cancelled",
            serde_json::json!({ "video_id": video_id, "title": title }),
        );
        return Err("cancelled".to_owned());
    }

    let format = download_format(&state.db);
    let stream = match resolve_stream(state, orchestrator, video_id).await {
        Some(s) => s,
        None => {
            let msg = "couldn't resolve a stream (InnerTube + yt-dlp both failed)".to_owned();
            let _ = app.emit(
                "download-error",
                serde_json::json!({ "video_id": video_id, "title": title, "error": msg }),
            );
            return Err(msg);
        }
    };

    let dir = download_dir(app, &state.db);
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {dir:?}: {e}"))?;
    // Human-readable filename (Title - Artist) so the file on disk isn't just the video id.
    // video_id stays the catalogue key, so dedup/offline-playback are unaffected.
    let base_name = sanitize_filename(&format!(
        "{}{}{}",
        title.trim(),
        if artists.trim().is_empty() { "" } else { " - " },
        artists.trim()
    ));
    let base_name = if base_name.is_empty() {
        video_id.to_owned()
    } else {
        base_name
    };
    // Two distinct tracks can share a `Title - Artist` name; the catalogue is keyed by video id,
    // so a second one must disambiguate with an id suffix instead of overwriting the first one's
    // audio while both rows persist. A stray untracked file with the same name is left alone too.
    let id8 = &video_id[..video_id.len().min(8)];
    let mut file_name = format!("{base_name}.{format}");
    let target = dir.join(&file_name);
    let owner = state.db.video_id_for_path(&target.to_string_lossy());
    let taken = match owner {
        // Another track already owns this exact path — disambiguate.
        Some(ref o) if o != video_id => true,
        // Untracked stray file (interrupted run, user drop) — don't clobber it either.
        None if target.exists() => true,
        _ => false,
    };
    if taken {
        file_name = format!("{base_name} [{id8}].{format}");
    }
    let file_path = dir.join(&file_name);
    // The temp name carries the id too: two same-named tracks downloading concurrently would
    // otherwise write into the same .part file.
    let tmp_path = dir.join(format!(".{base_name}.{id8}.{format}.part"));

    // Use the resolved stream URL with safe ratebypass append for googlevideo URLs.
    let stream_url = with_ratebypass(&stream.url);
    let resp = client()
        .get(&stream_url)
        .headers(
            stream
                .headers
                .iter()
                .fold(reqwest::header::HeaderMap::new(), |mut m, (k, v)| {
                    if let (Ok(name), Ok(value)) = (
                        reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                        reqwest::header::HeaderValue::from_bytes(v.as_bytes()),
                    ) {
                        m.insert(name, value);
                    }
                    m
                }),
        )
        .send()
        .await
        .map_err(|e| format!("download request failed: {e}"))?;
    if !resp.status().is_success() {
        let msg = format!("download HTTP {}", resp.status());
        let _ = app.emit(
            "download-error",
            serde_json::json!({ "video_id": video_id, "title": title, "error": msg }),
        );
        return Err(msg);
    }

    let total = resp.content_length().unwrap_or(0);
    let mut downloaded: u64 = 0;
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .map_err(|e| format!("create {tmp_path:?}: {e}"))?;
    let mut stream_body = resp.bytes_stream();
    // Emit an initial progress tick so the UI lights up immediately.
    let _ = app.emit(
        "download-progress",
        serde_json::json!({
            "video_id": video_id,
            "title": title,
            "artists": artists,
            "thumb": thumb,
            "downloaded": 0,
            "total": total,
            "percent": 0,
            "client": stream.client,
        }),
    );
    // Throttle progress emissions to ~10/sec (every 100ms) so the event bus doesn't flood.
    let mut last_emit = std::time::Instant::now();
    while let Some(chunk) = stream_body.next().await {
        // Cancel check between chunks: the flag is only set by cancel_download, and on fire we
        // drop the .part file so no half-written audio is left behind.
        if cancel_flag.load(Ordering::SeqCst) {
            drop(file);
            let _ = std::fs::remove_file(&tmp_path);
            let _ = app.emit(
                "download-cancelled",
                serde_json::json!({ "video_id": video_id, "title": title }),
            );
            return Err("cancelled".to_owned());
        }
        let chunk = chunk.map_err(|e| format!("download body failed: {e}"))?;
        downloaded += chunk.len() as u64;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("write: {e}"))?;
        // Only emit progress if 100ms has passed since last emit (or at 100%).
        let should_emit = last_emit.elapsed().as_millis() >= 100 || downloaded >= total as u64;
        if should_emit {
            last_emit = std::time::Instant::now();
            let percent = if total > 0 {
                ((downloaded as f64 / total as f64) * 100.0) as u32
            } else {
                0
            };
            let _ = app.emit(
                "download-progress",
                serde_json::json!({
                    "video_id": video_id,
                    "title": title,
                    "artists": artists,
                    "thumb": thumb,
                    "downloaded": downloaded,
                    "total": total,
                    "percent": percent,
                    "client": stream.client,
                }),
            );
        }
    }
    tokio::io::AsyncWriteExt::flush(&mut file)
        .await
        .map_err(|e| format!("flush: {e}"))?;
    drop(file);
    std::fs::rename(&tmp_path, &file_path).map_err(|e| format!("rename: {e}"))?;

    let size = downloaded as i64;
    let quality = download_quality(&state.db);
    let rec = DownloadTrack {
        video_id: video_id.to_owned(),
        file_path: file_path.to_string_lossy().into_owned(),
        title: title.to_owned(),
        artists: artists.to_owned(),
        album: album.map(str::to_owned),
        duration,
        thumb: thumb.map(str::to_owned),
        quality: match quality {
            AudioQuality::Low => "LOW",
            AudioQuality::Auto => "AUTO",
            AudioQuality::High => "HIGH",
        }
        .to_owned(),
        format: format.clone(),
        size_bytes: size,
        added_at: crate::db::now_secs(),
    };
    state.db.put_download(&rec);

    let _ = app.emit(
        "download-complete",
        serde_json::json!({
            "video_id": video_id,
            "title": title,
            "size_bytes": size,
            "file_path": rec.file_path,
        }),
    );
    Ok(())
}

/// Hard-delete a download: drop the row and remove the file. Returns an error only on the DB
/// side; a missing file is ignored (the catalogue is the source of truth).
pub fn delete_track(db: &Db, video_id: &str) -> Result<(), String> {
    if let Some(path) = db.download_path(video_id) {
        let _ = std::fs::remove_file(&path);
    }
    db.delete_download(video_id);
    Ok(())
}

/// Everything `download_track` needs to know about one track, pre-assembled by the batch caller
/// (the playlist/album page walker in commands.rs) so a multi-track download stays readable.
pub struct DownloadCandidate {
    pub video_id: String,
    pub title: String,
    pub artists: String,
    pub album: Option<String>,
    pub duration: i64,
    pub thumb: Option<String>,
}

/// How many tracks pull at once when the user downloads a whole playlist/album. Four parallel
/// streams keeps a long playlist moving without hammering YouTube, the connection pool, or the
/// disk; more than that barely helps (the bottleneck is the shared pipe, not our client).
pub const DOWNLOAD_CONCURRENCY: usize = 4;

/// True when the track is already saved — the playlist walker uses it to count "skipped" before
/// the worker tasks start, and `download_track` re-checks it so a duplicate sneaking in past the
/// dedupe is still a no-op.
pub fn is_downloaded(state: &AppState, video_id: &str) -> bool {
    state.db.download_path(video_id).is_some()
}

/// Download a batch of tracks with at most [`DOWNLOAD_CONCURRENCY`] streams in flight at once.
/// Already-downloaded tracks are skipped (the caller counts them; this also re-checks), each
/// success/error still rides the usual `download-*` events, and the whole batch resolves even if
/// individual tracks fail. `cancel_all_downloads` stops new tracks from starting mid-batch.
/// Returns `(completed, failed, cancelled)`.
pub async fn download_many(
    app: &AppHandle,
    state: &Arc<AppState>,
    orchestrator: &Arc<Orchestrator>,
    candidates: Vec<DownloadCandidate>,
) -> (usize, usize, usize) {
    // A fresh batch clears any stale stop from a previous "cancel all" — otherwise the first
    // playlist download after one cancellation would refuse to start.
    BATCH_STOP.store(false, Ordering::SeqCst);
    // Acquire before spawn: the loop only starts a new task once a permit frees, so at most
    // DOWNLOAD_CONCURRENCY tasks ever exist (no pile of idle tasks on a 1000-track playlist).
    let sem = Arc::new(tokio::sync::Semaphore::new(DOWNLOAD_CONCURRENCY));
    let mut handles = Vec::with_capacity(candidates.len());
    let mut cancelled_before_start = 0usize;
    for c in candidates {
        // Either an explicit "cancel all", or this exact track was cancelled while it sat queued
        // behind the concurrency window (drain the request here so the task never starts).
        let individually = requested().lock().unwrap().remove(&c.video_id);
        if BATCH_STOP.load(Ordering::SeqCst) || individually {
            let _ = app.emit(
                "download-cancelled",
                serde_json::json!({ "video_id": c.video_id, "title": c.title }),
            );
            cancelled_before_start += 1;
            continue;
        }
        let permit = match sem.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => break, // semaphore dropped — never happens, but don't spin
        };
        let app = app.clone();
        let state = state.clone();
        let orchestrator = orchestrator.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit; // held for the whole download
            download_track(
                &app,
                &state,
                &orchestrator,
                &c.video_id,
                &c.title,
                &c.artists,
                c.album.as_deref(),
                c.duration,
                c.thumb.as_deref(),
            )
            .await
        }));
    }
    let mut completed = 0usize;
    let mut failed = 0usize;
    for handle in handles {
        match handle.await {
            Ok(Ok(())) => completed += 1,
            Ok(Err(msg)) if msg == "cancelled" => {}
            _ => failed += 1,
        }
    }
    (completed, failed, cancelled_before_start)
}