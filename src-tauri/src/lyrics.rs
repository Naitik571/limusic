//! Lyrics fetching. Provider chain (plan `graceful-kindling`):
//!
//! 1. **LRCLIB** `/api/get` (exact match) → synced LRC lyrics. Free, no key, best coverage —
//!    what Metrolist defaults to.
//! 2. **YouTube Music timed** — `next(videoId)` → lyrics browseId → mobile-client browse
//!    (`timedLyricsData`). The same real-time lyrics the YTM app shows.
//! 3. **Musixmatch** (unofficial `apic-desktop` API): token.get → macro.subtitles.get, LRC
//!    synced lines with a token-overlap sanity check against the matched track.
//! 4. Plain fallbacks: LRCLIB plain (from step 1's response) → YT plain (WEB_REMIX browse) →
//!    LRCLIB `/api/search` fuzzy → **Genius** page scrape (unauthenticated internal search +
//!    `data-lyrics-container` extraction), last resort.
//!
//! Results are cached in SQLite (`lyrics_cache`): hits forever, "no lyrics" verdicts for 24h.
//! A run where every provider merely *errored* (offline) caches nothing, so lyrics come back
//! when the network does. Everything is best-effort — a lyrics failure is never a user error.

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// How long a cached "no lyrics found" verdict suppresses refetching.
const MISS_TTL_SECS: i64 = 24 * 3600;

const LRCLIB_ROOT: &str = "https://lrclib.net/api";

/// One display line. `time_ms` present ⇔ the line is synced (a plain-lyrics response has none).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_ms: Option<u64>,
    pub text: String,
}

/// What the UI gets (and what `lyrics_cache` stores as JSON).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyrics {
    /// Attribution shown in the panel footer ("LRCLIB", "Musixmatch", …).
    pub source: String,
    pub synced: bool,
    #[serde(default)]
    pub instrumental: bool,
    pub lines: Vec<LyricLine>,
}

pub struct LyricsRequest {
    pub video_id: String,
    pub title: String,
    pub artists: String,
    pub album: Option<String>,
    /// Track length in seconds (mpv's), tightens LRCLIB matching. `None`/0 when unknown yet.
    pub duration: Option<f64>,
}

/// Cache-through entry point for the `get_lyrics` command.
pub async fn get_lyrics(state: &AppState, req: LyricsRequest) -> Option<Lyrics> {
    let now = now_secs();
    let video_id = req.video_id.clone();
    if let Some(cached) = state.db.get_lyrics(&video_id, now, MISS_TTL_SECS) {
        return cached.and_then(|json| serde_json::from_str(&json).ok());
    }
    let (lyrics, cacheable) = fetch(state, req).await;
    if cacheable {
        let json = lyrics.as_ref().and_then(|l| serde_json::to_string(l).ok());
        state.db.put_lyrics(&video_id, json.as_deref(), now);
    }
    lyrics
}

/// Run the provider chain. Second value: cache the outcome — true only when the track's duration
/// was known (LRCLIB matching is loose without it and lands on wrong *cuts* of the song, lyrics
/// seconds off the audio) AND some provider answered definitively (found / not-found) rather
/// than merely erroring (offline must not poison the cache with a 24h "no lyrics").
async fn fetch(state: &AppState, mut req: LyricsRequest) -> (Option<Lyrics>, bool) {
    let mut definitive = false;

    // 0. `next()` up front: it carries the lyrics browseId AND — via its seed item — the exact
    //    length of the cut this videoId plays. The queue item often has no duration (card plays;
    //    stream-cache replays skip /player entirely), and duration is what keeps LRCLIB from
    //    matching a differently-timed cut, so resolve it here where it's always available.
    //    A local file has no videoId to ask about — its duration came off the file itself, and
    //    YouTube has no lyrics browseId for it. Skip straight to LRCLIB (title + artist), which is
    //    the only provider that can answer for it anyway.
    let next = if crate::local::is_local_song(&req.video_id) {
        None
    } else {
        match state
            .it
            .next(state.clients.get(innertube::METADATA_CLIENT).unwrap(), Some(&req.video_id), None)
            .await
        {
            Ok(n) => Some(n),
            Err(e) => {
                tracing::debug!(error = %e, "lyrics: next() failed");
                None
            }
        }
    };
    let browse_id = next.as_ref().and_then(|n| n.lyrics_browse_id.clone());
    if req.duration.is_none() {
        req.duration = next.as_ref().and_then(|n| {
            let item = n.items.iter().find(|i| i.video_id == req.video_id)?;
            duration_str_secs(item.duration.as_deref()?)
        });
    }
    let req = &req;

    // 1. LRCLIB exact match.
    let lr = lrclib_get(req).await;
    if let Ok(hit) = &lr {
        definitive = true;
        if let Some(l) = hit.as_ref().and_then(lrclib_to_lyrics) {
            if l.synced || l.instrumental {
                return (Some(l), req.duration.is_some());
            }
        }
    }

    // 2. YouTube Music timed lyrics.
    if next.is_some() {
        definitive = true; // a next() answer with no lyrics tab IS "YT has no lyrics"
    }
    if let (Some(bid), Some(client)) =
        (&browse_id, state.clients.get(innertube::LYRICS_TIMED_CLIENT))
    {
        match state.it.lyrics_timed(client, bid).await {
            Ok(lines) if !lines.is_empty() => {
                return (
                    Some(Lyrics {
                        source: "YouTube Music".into(),
                        synced: true,
                        instrumental: false,
                        lines: lines
                            .into_iter()
                            .map(|l| LyricLine { time_ms: Some(l.time_ms), text: l.text })
                            .collect(),
                    }),
                    true,
                );
            }
            Ok(_) => {}
            Err(e) => tracing::debug!(error = %e, "lyrics: timed browse failed"),
        }
    }

    // 2b. Musixmatch synced — unofficial desktop API, same LRC shape, decent coverage. Sits
    //    between the official sources and the fuzzy tier: a duration-verified LRCLIB fuzzy hit
    //    still outranks it on *cut* accuracy, which is why fuzzy stays ahead.
    match musixmatch(req).await {
        Ok(Some(l)) => return (Some(l), true),
        Ok(None) => {}
        Err(e) => tracing::debug!(error = %e, "lyrics: musixmatch failed"),
    }

    // 3. LRCLIB fuzzy search — a synced fuzzy match still beats any plain text, so it outranks
    //    the plain tier below. (YT lyrics are region-licensed and can be entirely absent.)
    let searched = lrclib_search(req).await;
    if let Ok(hit) = &searched {
        definitive = true;
        if let Some(l) = hit.as_ref().and_then(lrclib_to_lyrics).filter(|l| l.synced) {
            return (Some(l), req.duration.is_some());
        }
    }

    // --- plain tier -------------------------------------------------------------------------

    // 4a. Plain from LRCLIB's exact match.
    if let Ok(Some(hit)) = &lr {
        if let Some(l) = plain_from_text(hit.plain_lyrics.as_deref(), "LRCLIB") {
            return (Some(l), req.duration.is_some());
        }
    }

    // 4b. Plain from YT (WEB_REMIX).
    if let Some(bid) = &browse_id {
        if let Some(client) = state.clients.get(innertube::METADATA_CLIENT) {
            match state.it.lyrics_plain(client, bid).await {
                Ok(Some(p)) => {
                    // Footer is YT's own attribution ("Source: Musixmatch") — surface it.
                    let source = p.footer.unwrap_or_else(|| "YouTube Music".into());
                    if let Some(l) = plain_from_text(Some(&p.text), &source) {
                        return (Some(l), true);
                    }
                }
                Ok(None) => {}
                Err(e) => tracing::debug!(error = %e, "lyrics: plain browse failed"),
            }
        }
    }

    // 4c. Plain from the fuzzy search.
    if let Ok(Some(hit)) = &searched {
        if let Some(l) = lrclib_to_lyrics(hit) {
            return (Some(l), req.duration.is_some());
        }
    }

    // 4d. Genius plain — the last resort. Heavy (two requests, HTML scrape) and its matching is
    //    the loosest, so it only runs after every cheaper source has said no.
    match genius(req).await {
        Ok(Some(l)) => return (Some(l), true),
        Ok(None) => {}
        Err(e) => tracing::debug!(error = %e, "lyrics: genius failed"),
    }

    (None, definitive)
}

// --- LRCLIB (https://lrclib.net/docs) -------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibTrack {
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
}

/// Shared client. LRCLIB asks integrations to identify themselves via User-Agent.
fn http() -> &'static reqwest::Client {
    static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(concat!(
                "Limusic v",
                env!("CARGO_PKG_VERSION"),
                " (https://github.com/SimoHypers/limusic)"
            ))
            .build()
            .expect("build lyrics http client")
    })
}

/// `/api/get`: exact signature match. `Ok(None)` = definitive "not in LRCLIB" (404);
/// `Err` = transport trouble (don't cache a negative off it).
async fn lrclib_get(req: &LyricsRequest) -> Result<Option<LrclibTrack>, reqwest::Error> {
    let mut q: Vec<(&str, String)> = vec![
        ("track_name", req.title.clone()),
        ("artist_name", req.artists.clone()),
    ];
    if let Some(album) = &req.album {
        q.push(("album_name", album.clone()));
    }
    if let Some(d) = req.duration.filter(|d| *d > 0.0) {
        q.push(("duration", format!("{}", d.round() as i64)));
    }
    let resp = http().get(format!("{LRCLIB_ROOT}/get")).query(&q).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(resp.error_for_status()?.json().await?))
}

/// `/api/search`: fuzzy fallback. Prefers a synced candidate whose duration is within ±5s of
/// ours (when known); returns the best or `Ok(None)`.
async fn lrclib_search(req: &LyricsRequest) -> Result<Option<LrclibTrack>, reqwest::Error> {
    let q = [
        ("track_name", req.title.as_str()),
        ("artist_name", req.artists.as_str()),
    ];
    let list: Vec<LrclibTrack> = http()
        .get(format!("{LRCLIB_ROOT}/search"))
        .query(&q)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let ours = req.duration.filter(|d| *d > 0.0);
    // Distance from our track's length; unknown-length candidates rank last but aren't excluded.
    let dist = |t: &LrclibTrack| match (ours, t.duration) {
        (Some(a), Some(b)) => (a - b).abs(),
        _ => f64::INFINITY,
    };
    let close = |t: &LrclibTrack| ours.is_none() || dist(t) <= 5.0;
    let synced = |t: &LrclibTrack| t.synced_lyrics.as_deref().is_some_and(|s| !s.trim().is_empty());
    // Prefer the synced candidate whose duration is CLOSEST to ours — LRCLIB carries multiple
    // cuts of popular tracks, and a 4s-different cut plays lyrics 4s off the audio.
    let mut best_synced: Option<(f64, LrclibTrack)> = None;
    let mut best_plain: Option<LrclibTrack> = None;
    for t in list {
        if !close(&t) {
            continue;
        }
        if synced(&t) {
            let d = dist(&t);
            if best_synced.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best_synced = Some((d, t));
            }
        } else if best_plain.is_none() {
            best_plain = Some(t);
        }
    }
    Ok(best_synced.map(|(_, t)| t).or(best_plain))
}

/// Best `Lyrics` an LRCLIB track yields: instrumental > synced > plain > nothing.
fn lrclib_to_lyrics(t: &LrclibTrack) -> Option<Lyrics> {
    if t.instrumental {
        return Some(Lyrics {
            source: "LRCLIB".into(),
            synced: false,
            instrumental: true,
            lines: Vec::new(),
        });
    }
    if let Some(lrc) = t.synced_lyrics.as_deref().filter(|s| !s.trim().is_empty()) {
        let lines = parse_lrc(lrc);
        if !lines.is_empty() {
            return Some(Lyrics {
                source: "LRCLIB".into(),
                synced: true,
                instrumental: false,
                lines,
            });
        }
    }
    plain_from_text(t.plain_lyrics.as_deref(), "LRCLIB")
}

/// Plain text → un-timed lines (blank lines kept as stanza breaks).
fn plain_from_text(text: Option<&str>, source: &str) -> Option<Lyrics> {
    let text = text?.trim();
    if text.is_empty() {
        return None;
    }
    Some(Lyrics {
        source: source.to_owned(),
        synced: false,
        instrumental: false,
        lines: text
            .lines()
            .map(|l| LyricLine { time_ms: None, text: l.trim_end().to_owned() })
            .collect(),
    })
}

// --- Musixmatch (unofficial desktop API) -----------------------------------------------------
//
// Community-documented flow, stable for years: `token.get` hands out a usertoken (cached for
// the process lifetime), then `macro.subtitles.get` searches AND returns subtitles in one call.
// The subtitle body is LRC with span markup — reuse `parse_lrc` and strip the tags.

const MXM_ROOT: &str = "https://apic-desktop.musixmatch.com/ws/1.1";
const MXM_APP_ID: &str = "web-desktop-app-v1.0";
/// Acceptance floor for title/artist token overlap between our request and the matched track.
const MXM_MIN_OVERLAP: f64 = 0.35;

/// Browser-ish UA — the desktop endpoint rejects bare clients (curl, reqwest default).
fn web_http() -> &'static reqwest::Client {
    static WEB_HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    WEB_HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/124.0.0.0 Safari/537.36",
            )
            .build()
            .expect("build web http client")
    })
}

fn mxm_token_cell() -> &'static tokio::sync::Mutex<Option<String>> {
    static TOKEN: OnceLock<tokio::sync::Mutex<Option<String>>> = OnceLock::new();
    TOKEN.get_or_init(|| tokio::sync::Mutex::new(None))
}

async fn mxm_usertoken() -> Option<String> {
    if let Some(t) = mxm_token_cell().lock().await.clone() {
        return Some(t);
    }
    let resp: serde_json::Value = http()
        .get(format!("{MXM_ROOT}/token.get"))
        .query(&[("app_id", MXM_APP_ID)])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    let tok = resp.pointer("/message/body/user_token")?.as_str()?.to_owned();
    *mxm_token_cell().lock().await = Some(tok.clone());
    Some(tok)
}

/// `Ok(None)` = definitive "no Musixmatch result"; `Err` = transport/token trouble.
async fn musixmatch(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    // Token unavailable → definitive-ish "skip Musixmatch" (the caller doesn't cache a miss
    // on this path — only `Ok(Some)` marks the run definitive), never a hard error.
    let Some(tok) = mxm_usertoken().await else {
        tracing::debug!("musixmatch: no usertoken");
        return Ok(None);
    };
    let mut q: Vec<(&str, String)> = vec![
        ("format", "json".into()),
        ("q_track", req.title.clone()),
        ("q_artist", req.artists.clone()),
        ("user_token", tok),
        ("app_id", MXM_APP_ID.into()),
    ];
    if let Some(album) = &req.album {
        q.push(("q_album", album.clone()));
    }
    let resp: serde_json::Value = http()
        .get(format!("{MXM_ROOT}/macro.subtitles.get"))
        .query(&q)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // The matched track, for the sanity check: name/artist from the same macro response.
    let matched = resp
        .pointer("/message/body/macro_calls/track.search/message/body/track_list/0/track")
        .and_then(|t| {
            Some((
                t.get("track_name").and_then(|v| v.as_str()).unwrap_or(""),
                t.get("artist_name").and_then(|v| v.as_str()).unwrap_or(""),
            ))
        });
    if let Some((m_title, m_artist)) = matched {
        let title_ok = overlap(&req.title, m_title) >= MXM_MIN_OVERLAP;
        // Artist may be a list ("A, B") — pass if ANY component matches.
        let artist_ok = req
            .artists
            .split(',')
            .map(str::trim)
            .any(|a| overlap(a, m_artist) >= MXM_MIN_OVERLAP);
        if !title_ok || !artist_ok {
            tracing::debug!(
                "musixmatch rejected: ours=({}, {}) theirs=({}, {})",
                req.title,
                req.artists,
                m_title,
                m_artist
            );
            return Ok(None);
        }
    }

    let body = resp
        .pointer("/message/body/macro_calls/track.subtitles.get/message/body/subtitle_list/0/subtitle/subtitle_body")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if body.trim().is_empty() {
        return Ok(None);
    }
    // Span markup ("<span start=… end=…>text</span>") around LRC timestamps — strip tags so
    // `parse_lrc` sees clean lines; single-tag lines still carry their [mm:ss.xx] cues.
    let cleaned = strip_html_tags(body);
    let lines = parse_lrc(&cleaned);
    if !lines.is_empty() {
        return Ok(Some(Lyrics {
            source: "Musixmatch".into(),
            synced: true,
            instrumental: false,
            lines,
        }));
    }
    Ok(plain_from_text(Some(&cleaned), "Musixmatch"))
}

// --- Genius (unauthenticated internal API + page scrape) --------------------------------------

/// `Ok(None)` = no hit / no lyrics on the page; `Err` = transport trouble.
async fn genius(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    // 1. Search via the internal endpoint the site itself uses (no OAuth token needed).
    let q = format!("{} {}", req.title, req.artists);
    let resp: serde_json::Value = web_http()
        .get("https://genius.com/api/search/multi")
        .query(&[("q", &q)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // Collect song hits, score by token overlap, keep the best that clears the bar.
    let mut best: Option<(f64, String)> = None;
    for section in resp
        .pointer("/response/sections")
        .and_then(|s| s.as_array())
        .into_iter()
        .flatten()
    {
        if section.get("type").and_then(|t| t.as_str()) != Some("song") {
            continue;
        }
        for hit in section
            .get("hits")
            .and_then(|h| h.as_array())
            .into_iter()
            .flatten()
        {
            let Some(r) = hit.get("result") else { continue };
            let (Some(path), Some(g_title)) = (
                r.get("path").and_then(|v| v.as_str()),
                r.get("title").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            let g_artist = r
                .get("primary_artist")
                .and_then(|a| a.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let title_score = overlap(&req.title, g_title);
            let artist_score = req
                .artists
                .split(',')
                .map(str::trim)
                .map(|a| overlap(a, g_artist))
                .fold(0.0_f64, f64::max);
            // Both must clear the bar — Genius search can return same-named covers by other
            // artists, and wrong-artist lyrics are worse than none.
            if title_score < 0.4 || artist_score < 0.3 {
                continue;
            }
            let score = (title_score + artist_score) / 2.0;
            if best.as_ref().is_none_or(|(bs, _)| score > *bs) {
                best = Some((score, path.to_owned()));
            }
        }
    }
    let Some((_, path)) = best else {
        return Ok(None);
    };

    // 2. Scrape the song page: lyrics live in `data-lyrics-container` divs (links, <br>, <i>).
    let html = web_http()
        .get(format!("https://genius.com{path}"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let mut blocks: Vec<String> = Vec::new();
    for cap in GENIUS_CONTAINER.captures_iter(&html) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        // <br> and block boundaries become line breaks before the tag strip.
        let with_breaks = raw.replace("<br>", "\n").replace("</div>", "\n");
        let text = html_unescape(&strip_html_tags(&with_breaks));
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            blocks.push(trimmed.to_owned());
        }
    }
    if blocks.is_empty() {
        return Ok(None);
    }
    Ok(plain_from_text(Some(&blocks.join("\n\n")), "Genius"))
}

/// `<div data-lyrics-container="true" …> … </div>` — non-greedy up to the first close tag; the
/// containers don't nest divs (spans/links only), so a simple match is safe enough.
static GENIUS_CONTAINER: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r#"data-lyrics-container="true"[^>]*>((?s).*?)</div>"#).unwrap()
});

/// Strip every tag, keeping the text between them.
fn strip_html_tags(s: &str) -> String {
    static TAGS: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"<[^>]*>").unwrap());
    TAGS.replace_all(s, "").into_owned()
}

/// Minimal entity decode for what lyric pages actually use.
fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
}

/// Case-insensitive token-overlap (Dice-ish) between two strings, 0..1.
fn overlap(a: &str, b: &str) -> f64 {
    let toks = |s: &str| -> Vec<String> {
        s.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| !t.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let (ta, tb) = (toks(a), toks(b));
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let common = ta.iter().filter(|t| tb.contains(t)).count();
    2.0 * common as f64 / (ta.len() + tb.len()) as f64
}

// --- LRC parsing ----------------------------------------------------------------------------

/// Parse LRC text (`[mm:ss.xx] line`) into sorted lines. Handles multiple timestamps per line
/// (`[t1][t2]text` — the line repeats at both cues) and skips metadata tags (`[ar:…]`).
/// Timestamped empty lines are kept: they're instrumental gaps the UI can show as such.
fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut out = Vec::new();
    for raw in lrc.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(after) = rest.strip_prefix('[') {
            let Some(end) = after.find(']') else { break };
            match parse_lrc_time(&after[..end]) {
                Some(ms) => {
                    times.push(ms);
                    rest = after[end + 1..].trim_start();
                }
                // Not a timestamp: a metadata tag ([ar:…] — no times yet, line skipped) or
                // bracketed lyric text ("[Chorus]" — keep it as the line's text).
                None => break,
            }
        }
        for &ms in &times {
            out.push(LyricLine { time_ms: Some(ms), text: rest.to_owned() });
        }
    }
    out.sort_by_key(|l| l.time_ms);
    out
}

/// `mm:ss`, `mm:ss.xx`, or `mm:ss.xxx` → milliseconds.
fn parse_lrc_time(tag: &str) -> Option<u64> {
    let (m, rest) = tag.split_once(':')?;
    let m: u64 = m.trim().parse().ok()?;
    let (s, frac) = match rest.split_once('.') {
        Some((s, f)) => (s, Some(f)),
        None => (rest, None),
    };
    let s: u64 = s.trim().parse().ok()?;
    let ms = match frac {
        Some(f) => {
            let digits: String = f.chars().filter(char::is_ascii_digit).take(3).collect();
            let val: u64 = digits.parse().ok()?;
            match digits.len() {
                1 => val * 100,
                2 => val * 10,
                _ => val,
            }
        }
        None => 0,
    };
    Some((m * 60 + s) * 1000 + ms)
}

/// `"3:21"` / `"1:02:03"` → seconds.
fn duration_str_secs(s: &str) -> Option<f64> {
    let mut total: u64 = 0;
    for part in s.split(':') {
        total = total * 60 + part.trim().parse::<u64>().ok()?;
    }
    (total > 0).then_some(total as f64)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_lrc() {
        let lrc = "[ar:Fleetwood Mac]\n[00:27.93] Listen to the wind blow\n[00:31.16] Watch the sun rise\n";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, Some(27930));
        assert_eq!(lines[0].text, "Listen to the wind blow");
        assert_eq!(lines[1].time_ms, Some(31160));
    }

    #[test]
    fn multi_timestamp_line_repeats() {
        let lines = parse_lrc("[00:10.00][01:10.00]la la la");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, Some(10000));
        assert_eq!(lines[1].time_ms, Some(70000));
        assert!(lines.iter().all(|l| l.text == "la la la"));
    }

    #[test]
    fn keeps_bracketed_lyric_text_and_gap_lines() {
        let lines = parse_lrc("[00:05.5][Chorus] yeah\n[00:20.123]\n[00:30] plain seconds");
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].time_ms, Some(5500));
        assert_eq!(lines[0].text, "[Chorus] yeah");
        assert_eq!(lines[1].time_ms, Some(20123));
        assert_eq!(lines[1].text, "");
        assert_eq!(lines[2].time_ms, Some(30000));
    }

    #[test]
    fn overlap_scores_similar_strings() {
        assert!(overlap("Fleetwood Mac", "Fleetwood Mac") > 0.9);
        assert!(overlap("The Chain", "The Chain (Live)") > 0.5);
        assert_eq!(overlap("aaa", "bbb"), 0.0);
        assert!(overlap("A, B", "A") > 0.0);
    }

    #[test]
    fn strips_genius_markup() {
        let html = "Line one<br>Line <a href=\"/x\">two</a> &amp; more";
        let out = html_unescape(&strip_html_tags(&html.replace("<br>", "\n")));
        assert!(out.contains("Line one"));
        assert!(out.contains("Line two & more"));
        assert!(!out.contains('<'));
    }

    #[test]
    fn plain_text_splits_lines() {
        let l = plain_from_text(Some("one\ntwo\n\nthree"), "LRCLIB").unwrap();
        assert!(!l.synced);
        assert_eq!(l.lines.len(), 4);
        assert_eq!(l.lines[2].text, "");
    }
}
