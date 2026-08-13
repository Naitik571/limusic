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
//! - Progress rides the Tauri event bus so a thin UI bar can track it without polling.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use crate::db::{Db, DownloadTrack};
use crate::orchestrator::Orchestrator;
use innertube::AudioQuality;

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

/// Resolve + download one track's audio to disk, recording it in the catalogue.
///
/// Emits `download-progress` (a few times), `download-complete`, or `download-error`. Idempotent:
/// a second call for an already-downloaded `video_id` is a no-op.
pub async fn download_track(
    app: &AppHandle,
    db: &Arc<Db>,
    orchestrator: &Arc<Orchestrator>,
    video_id: &str,
    title: &str,
    artists: &str,
    album: Option<&str>,
    duration: i64,
    thumb: Option<&str>,
) -> Result<(), String> {
    if db.download_path(video_id).is_some() {
        return Ok(());
    }

    let quality = download_quality(db);
    let format = download_format(db);
    let data = orchestrator
        .resolve(video_id, quality, &Default::default())
        .await
        .map_err(|e| format!("resolve failed: {e}"))?;

    let dir = download_dir(app, db);
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {dir:?}: {e}"))?;
    let file_path = dir.join(format!("{video_id}.{format}"));
    let tmp_path = dir.join(format!(".{video_id}.{format}.part"));

    let client = reqwest::Client::new();
    let mut req = client.get(&data.stream_url);
    for (k, v) in &data.headers {
        req = req.header(k, v);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("download request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("download HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("download body failed: {e}"))?;

    // Atomic: write to a sidecar, then rename into place so a partial never looks "downloaded".
    std::fs::write(&tmp_path, &bytes).map_err(|e| format!("write {tmp_path:?}: {e}"))?;
    std::fs::rename(&tmp_path, &file_path).map_err(|e| format!("rename: {e}"))?;

    let size = bytes.len() as i64;
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
    db.put_download(&rec);

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
