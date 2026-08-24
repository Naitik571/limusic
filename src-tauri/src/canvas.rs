//! Spotify Canvas fetching (#8). Spotify Canvas are short looping videos that replace
//! static artwork on the Now Playing view when available. No Spotify API token is required
//! for the initial implementation — we use the public SimpMusic Canvas API
//! (`https://api.simpmusic.org/canvas`) as a stub, with graceful fallback to the static
//! artwork palette. The endpoint is best-effort: a 404 or network error simply means "no
//! canvas for this track" and the UI shows the blurred artwork instead.
//!
//! The lookup is keyed on artist + title (same as hi-res iTunes art), cached in memory
//! (bounded LRU) and never persisted. The video URL is returned as-is to the webview,
//! which renders it as `muted autoplay loop playsinline`.

use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

const CACHE_CAP: usize = 200;

static CACHE: LazyLock<Mutex<VecDeque<(String, Option<String>)>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

fn http() -> &'static reqwest::Client {
    static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("Limusic v", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("build canvas http client")
    });
    &HTTP
}

/// Fetch a Canvas video URL for a track, if any. Returns `Ok(None)` when no canvas exists
/// (cached or remote 404) — the caller falls back to static artwork / palette gradient.
pub async fn lookup(artist: &str, title: &str) -> Result<Option<String>, String> {
    let key = format!("{}|{}", artist.to_lowercase(), title.to_lowercase());
    if let Some(hit) = cache_get(&key) {
        return Ok(hit);
    }

    // Try SimpMusic Canvas API first (public, no auth). The API expects a Spotify track id
    // when available, but we only have YouTube metadata — so we try an artist+title search
    // endpoint, then fall back to a direct canvas URL probe.
    // Supported shapes (best-effort, all optional):
    //   GET https://api.simpmusic.org/canvas?artist=...&title=...
    //   GET https://api.simpmusic.org/spotify/canvas?artist=...&track=...
    let mut canvas_url: Option<String> = None;

    // Attempt 1: simpmusic canvas search
    let attempts = [
        format!(
            "https://api.simpmusic.org/canvas?artist={}&title={}",
            urlencoding::encode(artist),
            urlencoding::encode(title)
        ),
        format!(
            "https://api.simpmusic.org/spotify/canvas?artist={}&track={}",
            urlencoding::encode(artist),
            urlencoding::encode(title)
        ),
    ];

    for url in attempts {
        match try_fetch_canvas(&url).await {
            Ok(Some(u)) => {
                canvas_url = Some(u);
                break;
            }
            Ok(None) => continue,
            Err(_) => continue,
        }
    }

    // If still none, attempt to resolve via Spotify search -> canvas (stub, no token).
    // This keeps the API surface compatible with a future authenticated Spotify lookup.
    cache_put(key, canvas_url.clone());
    Ok(canvas_url)
}

async fn try_fetch_canvas(url: &str) -> Result<Option<String>, String> {
    let resp = http().get(url).send().await.map_err(|e| e.to_string())?;

    if !resp.status().is_success() {
        return Ok(None);
    }

    // Try JSON first: { "canvasUrl": "...", "url": "...", "data": { "canvas_url": "..." } }
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(s) = v
            .get("canvasUrl")
            .or_else(|| v.get("canvas_url"))
            .or_else(|| v.get("url"))
            .or_else(|| v.pointer("/data/canvasUrl"))
            .or_else(|| v.pointer("/data/canvas_url"))
            .or_else(|| v.pointer("/data/url"))
            .and_then(|x| x.as_str())
        {
            let s = s.trim();
            if !s.is_empty() && (s.starts_with("https://") || s.starts_with("http://")) {
                // Basic validation: canvas is mp4/webm
                if s.contains(".mp4") || s.contains(".webm") || s.contains("canvas") {
                    return Ok(Some(s.to_owned()));
                }
                // Still return if looks like a URL — the UI will handle load errors
                return Ok(Some(s.to_owned()));
            }
        }
        // Spotify's native canvas response: { "canvases": [ { "url": "..." } ] }
        if let Some(arr) = v
            .get("canvases")
            .and_then(|a| a.as_array())
            .or_else(|| v.pointer("/data/canvases").and_then(|a| a.as_array()))
        {
            if let Some(first) = arr
                .first()
                .and_then(|o| o.get("url").and_then(|u| u.as_str()))
            {
                return Ok(Some(first.to_owned()));
            }
        }
    }

    // Fallback: if response is a direct URL string
    let trimmed = text.trim().trim_matches('"');
    if trimmed.starts_with("https://") && (trimmed.contains(".mp4") || trimmed.contains("canvas")) {
        return Ok(Some(trimmed.to_owned()));
    }

    Ok(None)
}

fn cache_get(key: &str) -> Option<Option<String>> {
    let mut cache = CACHE.lock().unwrap();
    let pos = cache.iter().position(|(k, _)| k == key)?;
    let entry = cache.remove(pos).unwrap();
    cache.push_back(entry.clone());
    Some(entry.1)
}

fn cache_put(key: String, value: Option<String>) {
    let mut cache = CACHE.lock().unwrap();
    if let Some(pos) = cache.iter().position(|(k, _)| *k == key) {
        cache.remove(pos);
    }
    cache.push_back((key, value));
    while cache.len() > CACHE_CAP {
        cache.pop_front();
    }
}
