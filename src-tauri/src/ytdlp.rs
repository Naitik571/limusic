//! Managed yt-dlp fallback (2026-08 research round).
//!
//! The orchestrator's InnerTube cascade (WEB_REMIX → direct clients → rustypipe) is the primary
//! path; yt-dlp is the last-ditch net for restricted / PoToken-enforced / region-locked tracks.
//! yt-dlp's `tv,android_vr` player client dodges the whole cipher/PoToken dance because the
//! binary is re-released every few days and self-updates, so extractor rot fixes itself.
//!
//! The binary is *managed*, not bundled: downloaded from the official GitHub release on first
//! need (atomic `.part` → rename, size sanity floor), then self-updated with `yt-dlp -U` on a
//! 72 h cadence (stamp file in the bin dir; the stamp is refreshed even when the update fails so
//! a flaky network can't cause retry storms). Download happens in the background at startup, so
//! the first restricted track usually finds it already installed.
//!
//! License note: yt-dlp is Unlicense — shipping/self-downloading the binary is license-clean for
//! an MIT app. (YTubic's design, re-expressed for this codebase.)

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

/// `-U` cadence. yt-dlp ships fixes ~daily; 72 h keeps us within a couple of releases.
const UPDATE_INTERVAL_SECS: u64 = 72 * 3600;
/// A real yt-dlp is never under 1 MB; anything smaller is a truncated HTML error page.
const MIN_BIN_SIZE: u64 = 1_000_000;
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(25);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One resolved stream, ready to hand to mpv.
pub struct YtDlpStream {
    pub url: String,
}

struct State {
    /// A download/update is in flight — don't double-start one.
    busy: bool,
    last_error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        State { busy: false, last_error: None }
    }
}

pub struct YtDlp {
    app: AppHandle,
    http: reqwest::Client,
    enabled: AtomicBool,
    state: Mutex<State>,
}

impl YtDlp {
    pub fn new(app: AppHandle, enabled: bool) -> Self {
        YtDlp {
            app,
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            enabled: AtomicBool::new(enabled),
            state: Mutex::new(State::default()),
        }
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.app
            .path()
            .app_data_dir()
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("bin")
    }

    pub fn bin_path(&self) -> PathBuf {
        let name = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
        self.bin_dir().join(name)
    }

    pub fn installed(&self) -> bool {
        self.bin_path().is_file()
    }

    pub fn last_error(&self) -> Option<String> {
        self.state.lock().unwrap().last_error.clone()
    }

    /// Make the binary exist and reasonably fresh. Serialized so parallel failure paths can't
    /// download twice; every caller just awaits the same mutex.
    pub async fn ensure_ready(&self) -> Result<(), String> {
        if !self.enabled() {
            return Err("yt-dlp fallback is disabled".into());
        }
        {
            let mut s = self.state.lock().unwrap();
            if s.busy {
                // Someone else is handling it; treat as success — the caller retries its
                // resolve right after and will see the fresh binary.
                return Ok(());
            }
            s.busy = true;
        }
        let result = self.download_or_update().await;
        self.state.lock().unwrap().busy = false;
        match &result {
            Ok(()) => self.state.lock().unwrap().last_error = None,
            Err(e) => self.state.lock().unwrap().last_error = Some(e.clone()),
        }
        result
    }

    async fn download_or_update(&self) -> Result<(), String> {
        let bin = self.bin_path();
        if bin.is_file() {
            // Already installed → maybe self-update (72 h cadence, stamp refreshed either way).
            let stamp = self.bin_dir().join("ytdlp-update-stamp");
            let due = std::fs::read_to_string(&stamp)
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                .map(|t| std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs().saturating_sub(t) >= UPDATE_INTERVAL_SECS)
                    .unwrap_or(true))
                .unwrap_or(true);
            if due {
                let _ = self.run_update(&bin).await;
                let _ = std::fs::write(&stamp, format!("{}", now_secs()));
            }
            return Ok(());
        }

        // First install: download the official single-file release.
        std::fs::create_dir_all(self.bin_dir()).map_err(|e| format!("bin dir: {e}"))?;
        let url = if cfg!(windows) {
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
        } else {
            "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp"
        };
        let resp = tokio::time::timeout(DOWNLOAD_TIMEOUT, self.http.get(url).send())
            .await
            .map_err(|_| "yt-dlp download timed out".to_string())?
            .map_err(|e| format!("yt-dlp download: {e}"))?
            .error_for_status()
            .map_err(|e| format!("yt-dlp download: {e}"))?;
        let bytes = tokio::time::timeout(DOWNLOAD_TIMEOUT, resp.bytes())
            .await
            .map_err(|_| "yt-dlp download timed out".to_string())?
            .map_err(|e| format!("yt-dlp download: {e}"))?;
        if bytes.len() < MIN_BIN_SIZE as usize {
            return Err(format!(
                "yt-dlp download suspiciously small ({} bytes) — aborting",
                bytes.len()
            ));
        }
        let part = self.bin_dir().join("yt-dlp.part");
        std::fs::write(&part, &bytes).map_err(|e| format!("write: {e}"))?;
        std::fs::rename(&part, &bin).map_err(|e| format!("rename: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755));
        }
        let _ = std::fs::write(self.bin_dir().join("ytdlp-update-stamp"), format!("{}", now_secs()));
        tracing::info!("yt-dlp installed at {}", bin.display());
        Ok(())
    }

    async fn run_update(&self, bin: &PathBuf) -> Result<(), String> {
        let mut cmd = tokio::process::Command::new(bin);
        cmd.arg("-U")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        match tokio::time::timeout(UPDATE_TIMEOUT, cmd.status()).await {
            Ok(Ok(st)) if st.success() => {
                tracing::info!("yt-dlp self-updated");
                Ok(())
            }
            Ok(Ok(st)) => Err(format!("yt-dlp -U exited {st}")),
            Ok(Err(e)) => Err(format!("yt-dlp -U: {e}")),
            Err(_) => Err("yt-dlp -U timed out".into()),
        }
    }

    /// Resolve a videoId to a playable URL. `None` = disabled, not ready, or yt-dlp itself
    /// failed — the caller treats it exactly like a rustypipe failure.
    pub async fn resolve(&self, video_id: &str) -> Option<YtDlpStream> {
        if !self.enabled() {
            return None;
        }
        if self.ensure_ready().await.is_err() {
            return None;
        }
        let bin = self.bin_path();
        let mut cmd = tokio::process::Command::new(&bin);
        cmd.args([
            "-j",
            "-f",
            "bestaudio[ext=webm]/bestaudio",
            "--no-playlist",
            "--no-warnings",
            "--socket-timeout",
            "10",
            "--extractor-args",
            "youtube:player_client=tv,android_vr",
            video_id,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let out = match tokio::time::timeout(RESOLVE_TIMEOUT, cmd.output()).await {
            Ok(Ok(o)) => o,
            _ => return None,
        };
        if !out.status.success() {
            tracing::debug!(video_id, "yt-dlp resolve failed (exit {})", out.status);
            return None;
        }
        let url = serde_json::from_slice::<serde_json::Value>(&out.stdout)
            .ok()
            .and_then(|v| v.get("url").and_then(|u| u.as_str()).map(str::to_owned));
        url.map(|url| YtDlpStream { url })
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
