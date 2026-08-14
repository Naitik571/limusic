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

use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use tauri::{AppHandle, Emitter, Manager};

use crate::db::{Db, DownloadTrack};
use crate::orchestrator::Orchestrator;
use crate::state::AppState;
use innertube::AudioQuality;

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
    let file_path = dir.join(format!("{base_name}.{format}"));
    let tmp_path = dir.join(format!(".{base_name}.{format}.part"));

    // Use the resolved stream URL exactly as playback would (mutating signed googlevideo URLs
    // breaks the signature and fails the download). The tuned client below keeps throughput up.
    let client = reqwest::Client::builder()
        .tcp_nodelay(true)
        .pool_max_idle_per_host(8)
        .build()
        .map_err(|e| format!("client build failed: {e}"))?;
    let mut req = client.get(&stream.url);
    for (k, v) in &stream.headers {
        req = req.header(k, v);
    }
    let resp = req
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
    while let Some(chunk) = stream_body.next().await {
        let chunk = chunk.map_err(|e| format!("download body failed: {e}"))?;
        downloaded += chunk.len() as u64;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .map_err(|e| format!("write: {e}"))?;
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
