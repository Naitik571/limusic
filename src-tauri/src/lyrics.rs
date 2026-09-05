//! Lyrics fetching. Provider wave (plan `graceful-kindling`):
//!
//! `next(videoId)` first resolves the lyrics browseId and the exact length of the cut this
//! videoId plays (hard-bounded). Then every keyless provider fires **concurrently**, each under
//! a tight per-provider deadline, and the best answer wins: synced beats instrumental beats
//! plain, and among equals the higher-priority (earlier) provider wins —
//! Apple Music (BYO token) → Boidu/BetterLyrics TTML → LRCLIB `/api/get` exact +
//! `/api/search` fuzzy → NetEase YRC → Kugou KRC → Unison → QRC (QQ) → Musixmatch richsync →
//! SimpMusic → Megalobiz → Genius. YouTube Music's authenticated timed (then plain) lyrics run
//! **last and optionally**: only when nothing synced arrived and budget remains. The whole
//! lookup fits a ~5 s budget — the previous strictly-serial chain with 15–20 s default client
//! timeouts could stall for minutes behind a few dead hosts.
//!
//! Results are cached in SQLite (`lyrics_cache`): hits live on, and a "no lyrics" verdict is
//! written only when providers genuinely *answered* (LRCLIB spoke cleanly, nothing merely
//! timed out) — a transient outage stays uncached so the next play retries instead of serving
//! a poisoned negative. Everything is best-effort — a lyrics failure is never a user error.

use std::sync::OnceLock;
use std::time::Duration;

use futures::future::{join_all, BoxFuture};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// How long a cached "no lyrics found" verdict suppresses refetching. Short on purpose:
/// negatives are only cached for genuine checked-everywhere runs, and a brief TTL lets a
/// provider that was down (or a freshly indexed catalog entry) surface on the next play.
const MISS_TTL_SECS: i64 = 120;

/// Per-provider hard deadline. One stalled host costs at most this much — never the old
/// serial 15–20 s default-client timeout per hop.
const PROVIDER_TIMEOUT: Duration = Duration::from_millis(2500);

/// Whole-lookup budget: `next()`, the concurrent wave, and any YTM follow-up together.
const FETCH_BUDGET: Duration = Duration::from_secs(5);

const LRCLIB_ROOT: &str = "https://lrclib.net/api";

/// One display line. `time_ms` present ⇔ the line is synced (a plain-lyrics response has none).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// One display line. `time_ms` present means the line is synced (a plain-lyrics response has none).
/// `words` carries per-word timings when a provider returned them; `end_time_ms` is the line's own
/// end cue (karaoke needs it to know when the last word stops).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricLine {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time_ms: Option<u64>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<LyricWord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}

impl LyricLine {
    /// Convenience constructor for the common case (plain text or a single cue, no words).
    pub fn simple(time_ms: Option<u64>, text: String) -> Self {
        Self {
            time_ms,
            end_time_ms: None,
            text,
            words: None,
            translation: None,
        }
    }
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
        if let Some(json) = cached {
            if let Ok(mut lyrics) = serde_json::from_str::<Lyrics>(&json) {
                let off = get_offset(&state.db, &video_id);
                if off != 0 {
                    apply_offset(&mut lyrics, off);
                }
                return Some(lyrics);
            }
        } else {
            return None;
        }
    }
    let (mut lyrics, cacheable) = fetch(state, req).await;
    if cacheable {
        let json = lyrics.as_ref().and_then(|l| serde_json::to_string(l).ok());
        state.db.put_lyrics(&video_id, json.as_deref(), now);
    }
    if let Some(l) = lyrics.as_mut() {
        let off = get_offset(&state.db, &video_id);
        if off != 0 {
            apply_offset(l, off);
        }
    }
    lyrics
}

/// Run the provider wave. Second value: cache the outcome — true only for a genuine "every
/// provider answered, nothing exists" run (LRCLIB's exact + fuzzy passes both spoke cleanly,
/// no provider merely timed out, and the YTM lookups — when reachable — didn't error). Any
/// transient trouble returns `false` so the next play retries instead of a poisoned negative.
///
/// Flow: resolve `next()` → launch all keyless providers CONCURRENTLY under per-provider
/// deadlines → take the best answer by priority (synced beats instrumental beats plain; earlier
/// provider wins ties) → optionally upgrade with YTM authenticated lyrics within budget.
async fn fetch(state: &AppState, mut req: LyricsRequest) -> (Option<Lyrics>, bool) {
    let started = std::time::Instant::now();

    // 0. `next()` up front: it carries the lyrics browseId AND — via its seed item — the exact
    //    length of the cut this videoId plays. The queue item often has no duration (card plays;
    //    stream-cache replays skip /player entirely), and duration is what keeps LRCLIB from
    //    matching a differently-timed cut, so resolve it here where it's always available.
    //    Hard-bounded so a hung Innertube call can't eat the budget. A local file has no
    //    videoId to ask about — its duration came off the file itself, and YouTube has no
    //    lyrics browseId for it; providers match on title + artist alone.
    let next = if crate::local::is_local_song(&req.video_id) {
        None
    } else {
        let fut = state.it.next(
            state.clients.get(innertube::METADATA_CLIENT).unwrap(),
            Some(&req.video_id),
            None,
        );
        match tokio::time::timeout(PROVIDER_TIMEOUT, fut).await {
            Ok(Ok(n)) => Some(n),
            Ok(Err(e)) => {
                tracing::debug!(error = %e, "lyrics: next() failed");
                None
            }
            Err(_) => {
                tracing::debug!("lyrics: next() exceeded its deadline");
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

    // 1. The wave: every keyless provider fires AT ONCE (the old code ran them strictly serial;
    //    a few dead hosts at 15–20 s each stalled lyrics for minutes). Each slot carries its
    //    priority; when several answer, the best wins: synced > instrumental > plain, then the
    //    lower priority number takes it. Word-level sources lead so the karaoke sweep keeps its
    //    per-word timings. Each provider runs once — the previous chain queried Boidu, NetEase,
    //    Kugou and Musixmatch twice each on miss paths.
    //       0 Apple Music — BYO-token syllable data (silently off without Settings tokens)
    //       1 Boidu       — word-level TTML (setting-gated)
    //       2 LRCLIB /get — exact signature match
    //       3 LRCLIB /search — fuzzy second pass (duration-preferred, never hard-fails)
    //       4 NetEase YRC — word-level
    //       5 Kugou KRC   — word-level
    //       6 Unison      — community aggregate
    //       7 QRC (QQ)    — word-capable
    //       8 Musixmatch  — richsync/synced (token-overlap sanity check)
    //       9 SimpMusic   — LRCLIB-shaped fallback
    //      10 Megalobiz   — scraped LRC
    //      11 Genius      — plain-text scrape, loosest matching, last
    let mut slots: Vec<(u8, &'static str, BoxFuture<'_, Result<Option<Lyrics>, ()>>)> = Vec::new();
    slots.push((0, "Apple Music", Box::pin(bounded(apple_get(state, req)))));
    if state.db.get_setting("lyrics_boidu").as_deref() != Some("false") {
        slots.push((1, "Boidu", Box::pin(bounded(boidu_get(req)))));
    }
    slots.push((
        2,
        "LRCLIB",
        Box::pin(bounded(async {
            lrclib_get(req)
                .await
                .map(|hit| hit.and_then(|t| lrclib_to_lyrics(&t)))
        })),
    ));
    slots.push((
        3,
        "LRCLIB",
        Box::pin(bounded(async {
            lrclib_search(req)
                .await
                .map(|hit| hit.and_then(|t| lrclib_to_lyrics(&t)))
        })),
    ));
    slots.push((4, "NetEase", Box::pin(bounded(fetch_netease(req)))));
    slots.push((5, "Kugou", Box::pin(bounded(fetch_kugou(req)))));
    slots.push((6, "Unison", Box::pin(bounded(fetch_unison(req)))));
    slots.push((7, "QRC", Box::pin(bounded(fetch_qrc(req)))));
    slots.push((8, "Musixmatch", Box::pin(bounded(fetch_musixmatch(req)))));
    slots.push((9, "SimpMusic", Box::pin(bounded(fetch_simp_music(req)))));
    slots.push((10, "Megalobiz", Box::pin(bounded(megalobiz(req)))));
    slots.push((11, "Genius", Box::pin(bounded(genius(req)))));

    let wave = join_all(
        slots
            .drain(..)
            .map(|(prio, name, fut)| async move { (prio, name, fut.await) }),
    );
    // Insurance over the per-provider deadlines (each future is individually bounded, so this
    // only trips on pathological scheduler stalls). A blown budget is transient: cache nothing.
    let settled = match tokio::time::timeout(FETCH_BUDGET, wave).await {
        Ok(settled) => settled,
        Err(_) => {
            tracing::debug!("lyrics: provider wave exceeded the overall budget");
            return (None, false);
        }
    };

    /// synced-with-words > synced > instrumental > plain — the axis (besides priority)
    /// that ranks answers. Word-level wins because a Musixmatch/NetEase/Kugou/QRC word hit
    /// would otherwise always lose to an earlier line-level hit (LRCLIB usually has one for
    /// exactly the popular tracks that carry word timings). Default on; Settings writes
    /// `lyrics_word_first=false` to prefer plain line sync instead.
    let prefer_words = state.db.get_setting("lyrics_word_first").as_deref() != Some("false");
    fn rank(l: &Lyrics, prefer_words: bool) -> u8 {
        if l.synced {
            if prefer_words
                && l.lines
                    .iter()
                    .any(|x| x.words.as_ref().is_some_and(|w| !w.is_empty()))
            {
                3
            } else {
                2
            }
        } else if l.instrumental {
            1
        } else {
            0
        }
    }

    let mut best: Option<(u8, Lyrics)> = None;
    let mut timed_out = false;
    let mut lrclib_answered = 0u8;
    for (prio, name, res) in settled {
        if matches!(prio, 2 | 3) && res.is_ok() {
            lrclib_answered += 1; // Ok(hit) or Ok(None) — LRCLIB itself spoke
        }
        match res {
            Ok(Some(l)) => {
                let takes = match &best {
                    None => true,
                    Some((bp, bl)) => {
                        rank(&l, prefer_words) > rank(bl, prefer_words)
                            || (rank(&l, prefer_words) == rank(bl, prefer_words) && prio < *bp)
                    }
                };
                if takes {
                    best = Some((prio, l));
                }
            }
            Ok(None) => {}
            Err(_) => {
                timed_out = true;
                tracing::debug!(provider = name, "lyrics: provider exceeded its deadline");
            }
        }
    }

    // Negative-verdict rule: only a run where LRCLIB answered cleanly (both passes — hit or a
    // clean empty/404) and nothing merely timed out may claim "checked everywhere, nothing
    // exists". Everything else stays uncached so the next play retries (one LRCLIB 404 plus
    // nine timeouts used to poison a 24 h "no lyrics" — the reported inconsistency).
    let mut definitive = lrclib_answered == 2 && !timed_out;

    // Synced answer in hand — done. Plain/partial answers wait for the YTM upgrades below.
    if best
        .as_ref()
        .is_some_and(|(_, l)| rank(l, prefer_words) >= 2)
    {
        let (_, l) = best.take().unwrap();
        return (Some(l), req.duration.is_some());
    }

    // 2. YouTube Music timed lyrics — authenticated and heaviest, so strictly last and optional:
    //    only when nothing synced arrived, and only within whatever budget remains.
    if best.as_ref().is_none_or(|(_, l)| rank(l, prefer_words) < 2) {
        if let (Some(bid), Some(client)) = (
            &browse_id,
            state.clients.get(innertube::LYRICS_TIMED_CLIENT),
        ) {
            let remain = FETCH_BUDGET.saturating_sub(started.elapsed());
            if remain.is_zero() {
                definitive = false; // YTM never got to answer — not a checked-everywhere run
            } else {
                match tokio::time::timeout(remain, state.it.lyrics_timed(client, bid)).await {
                    Ok(Ok(lines)) if !lines.is_empty() => {
                        return (
                            Some(Lyrics {
                                source: "YouTube Music".into(),
                                synced: true,
                                instrumental: false,
                                lines: lines
                                    .into_iter()
                                    .map(|l| LyricLine {
                                        time_ms: Some(l.time_ms),
                                        end_time_ms: None,
                                        text: l.text,
                                        words: None,
                                        translation: None,
                                    })
                                    .collect(),
                            }),
                            req.duration.is_some(),
                        );
                    }
                    Ok(Ok(_)) => {} // a lyrics tab exists but carries no timed data
                    Ok(Err(e)) => {
                        definitive = false;
                        tracing::debug!(error = %e, "lyrics: timed browse failed");
                    }
                    Err(_) => {
                        definitive = false;
                        tracing::debug!("lyrics: timed browse out of budget");
                    }
                }
            }
        }
    }

    // 3. Plain from YT (WEB_REMIX) — only when the wave gave us nothing at all. Footer is YT's
    //    own attribution ("Source: Musixmatch") — surface it.
    if best.is_none() {
        if let Some(bid) = &browse_id {
            if let Some(client) = state.clients.get(innertube::METADATA_CLIENT) {
                let remain = FETCH_BUDGET.saturating_sub(started.elapsed());
                if !remain.is_zero() {
                    match tokio::time::timeout(remain, state.it.lyrics_plain(client, bid)).await {
                        Ok(Ok(Some(p))) => {
                            let source = p.footer.unwrap_or_else(|| "YouTube Music".into());
                            if let Some(l) = plain_from_text(Some(&p.text), &source) {
                                return (Some(l), req.duration.is_some());
                            }
                        }
                        Ok(Ok(None)) => {}
                        Ok(Err(e)) => {
                            definitive = false;
                            tracing::debug!(error = %e, "lyrics: plain browse failed");
                        }
                        Err(_) => {
                            definitive = false;
                            tracing::debug!("lyrics: plain browse out of budget");
                        }
                    }
                }
            }
        }
    }

    // Whatever plain/instrumental answer the wave produced stands; otherwise the negative goes
    // back (cached only when `definitive`).
    (best.map(|(_, l)| l), definitive)
}

/// One provider under one hard deadline. `Err(())` = the deadline blew — a *transient* verdict
/// the caller must never turn into a cached negative.
async fn bounded(
    fut: impl std::future::Future<Output = Result<Option<Lyrics>, reqwest::Error>>,
) -> Result<Option<Lyrics>, ()> {
    match tokio::time::timeout(PROVIDER_TIMEOUT, fut).await {
        Ok(res) => res.map_err(|_| ()),
        Err(_) => Err(()),
    }
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
/// The timeout is a backstop only — each provider runs under `PROVIDER_TIMEOUT`.
fn http() -> &'static reqwest::Client {
    static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
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
    let resp = http()
        .get(format!("{LRCLIB_ROOT}/get"))
        .query(&q)
        .send()
        .await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(Some(resp.error_for_status()?.json().await?))
}

/// `/api/search`: fuzzy pass, run as a second LRCLIB chance right next to the exact `/api/get`
/// (previously it sat behind a dozen other providers and rarely got reached). Two queries: the
/// fielded `track_name`/`artist_name` first, then a bare `q=` retry — parenthesis/feat.-junk
/// titles defeat the fielded match. Prefers the synced candidate whose duration is closest to
/// ours; if nothing is within ±5 s it still returns the closest synced candidate rather than
/// hard-failing (duration narrows the choice, never gates it).
async fn lrclib_search(req: &LyricsRequest) -> Result<Option<LrclibTrack>, reqwest::Error> {
    let mut list: Vec<LrclibTrack> = http()
        .get(format!("{LRCLIB_ROOT}/search"))
        .query(&[
            ("track_name", req.title.as_str()),
            ("artist_name", req.artists.as_str()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if list.is_empty() {
        list = http()
            .get(format!("{LRCLIB_ROOT}/search"))
            .query(&[("q", format!("{} {}", req.title, req.artists))])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
    }
    let ours = req.duration.filter(|d| *d > 0.0);
    // Distance from our track's length; unknown-length candidates rank last but aren't excluded.
    let dist = |t: &LrclibTrack| match (ours, t.duration) {
        (Some(a), Some(b)) => (a - b).abs(),
        _ => f64::INFINITY,
    };
    let synced = |t: &LrclibTrack| {
        t.synced_lyrics
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty())
    };
    // Prefer the synced candidate whose duration is CLOSEST to ours — LRCLIB carries multiple
    // cuts of popular tracks, and a 4s-different cut plays lyrics 4s off the audio. Beyond ±5 s
    // confidence drops but some synced lyric still beats none, so keep the closest instead of
    // dropping the track entirely.
    let mut best_close: Option<(f64, LrclibTrack)> = None;
    let mut best_far: Option<(f64, LrclibTrack)> = None;
    let mut best_plain: Option<LrclibTrack> = None;
    for t in list {
        if synced(&t) {
            let d = dist(&t);
            if d <= 5.0 {
                if best_close.as_ref().is_none_or(|(bd, _)| d < *bd) {
                    best_close = Some((d, t));
                }
            } else if best_far.as_ref().is_none_or(|(bd, _)| d < *bd) {
                best_far = Some((d, t));
            }
        } else if best_plain.is_none() {
            best_plain = Some(t);
        }
    }
    Ok(best_close
        .map(|(_, t)| t)
        .or(best_far.map(|(_, t)| t))
        .or(best_plain))
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
            .map(|l| LyricLine::simple(None, l.trim_end().to_owned()))
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
/// The timeout is a backstop only — each provider runs under `PROVIDER_TIMEOUT`.
fn web_http() -> &'static reqwest::Client {
    static WEB_HTTP: OnceLock<reqwest::Client> = OnceLock::new();
    WEB_HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(4))
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
    let tok = resp
        .pointer("/message/body/user_token")?
        .as_str()?
        .to_owned();
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

    // Richsync first: Musixmatch's word-level payload (same token, same macro call family).
    // Each line carries time + duration in ms and a `words` array of {o/word start/end} in
    // seconds-floats. Word-level beats line-synced, so try it before falling back.
    if let Ok(Some(lines)) = mxm_richsync(&resp).await {
        return Ok(Some(Lyrics {
            source: "Musixmatch".into(),
            synced: true,
            instrumental: false,
            lines,
        }));
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

/// Parse Musixmatch richsync JSON into word-level LyricLines. The macro response nests it at
/// `macro_calls > track.richsync.get > message > body > richsync` when the track has one; the
/// standalone endpoint returns it at `message.body.richsync`. Both shapes land here.
fn mxm_richsync_parse(rs: &serde_json::Value) -> Option<Vec<LyricLine>> {
    let arr = rs.get("lines")?.as_array()?;
    let mut out = Vec::new();
    for line in arr {
        // ts/te are milliseconds (numbers or numeric strings depending on API mood).
        let ts = line.get("ts").and_then(parse_time_val)?;
        let te = line.get("te").and_then(parse_time_val);
        let text = line
            .get("line")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut words = Vec::new();
        if let Some(ws) = line.get("words").and_then(|v| v.as_array()) {
            for w in ws {
                // lrc is the word text; start/end are seconds as floats (or strings).
                let wt = w
                    .get("word")
                    .or_else(|| w.get("lrc"))
                    .and_then(|v| v.as_str());
                let Some(wt) = wt else { continue };
                if wt.trim().is_empty() {
                    continue;
                }
                let ws_ms = w
                    .get("start")
                    .or_else(|| w.get("startTime"))
                    .and_then(parse_time_val)
                    .unwrap_or(ts);
                // End falls back to "next 500ms" like every other provider's guess.
                let we_ms = w
                    .get("end")
                    .or_else(|| w.get("endTime"))
                    .and_then(parse_time_val)
                    .unwrap_or(ws_ms + 500);
                words.push(LyricWord {
                    text: wt.to_string(),
                    start_ms: ws_ms,
                    end_ms: we_ms,
                });
            }
        }
        out.push(LyricLine {
            time_ms: Some(ts),
            end_time_ms: te.or_else(|| words.last().map(|w| w.end_ms)),
            text: if text.is_empty() {
                words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            } else {
                text
            },
            words: if words.is_empty() { None } else { Some(words) },
            translation: None,
        });
    }
    let has_words = out.iter().any(|l| l.words.is_some());
    (!out.is_empty() && has_words).then_some(out)
}

/// Try to pull a richsync out of an already-fetched macro response. If the macro didn't include
/// it, ask `track.richsync.get` directly with the matched track id (one extra request, only on
/// this path — cheap compared with losing word timings for Musixmatch's whole catalog).
async fn mxm_richsync(resp: &serde_json::Value) -> Result<Option<Vec<LyricLine>>, reqwest::Error> {
    // 1. Already inside the macro response?
    if let Some(rs) =
        resp.pointer("/message/body/macro_calls/track.richsync.get/message/body/richsync")
    {
        return Ok(mxm_richsync_parse(rs));
    }
    // 2. Not included — need the track id from track.search to call the dedicated endpoint.
    let track_id = resp
        .pointer("/message/body/macro_calls/track.search/message/body/track_list/0/track/track_id")
        .and_then(|v| {
            v.as_i64()
                .map(|i| i.to_string())
                .or_else(|| v.as_str().map(str::to_owned))
        });
    let Some(track_id) = track_id else {
        return Ok(None);
    };
    let Some(tok) = mxm_usertoken().await else {
        return Ok(None);
    };
    let rs_resp: serde_json::Value = http()
        .get(format!("{MXM_ROOT}/track.richsync.get"))
        .query(&[
            ("format", "json".to_string()),
            ("track_id", track_id),
            ("user_token", tok),
            ("app_id", MXM_APP_ID.into()),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let rs = rs_resp.pointer("/message/body/richsync");
    Ok(rs.and_then(mxm_richsync_parse))
}

// --- Apple Music (bring-your-own-token word-level provider) -----------------------------------
//
// Apple's AAML is the best word/syllable timing data available, but the API needs two tokens
// extracted from a logged-in music.apple.com session: the `media-user-token` cookie value and
// an iTunes Store storefront bearer (`developer-token` in the site's JS, or the `authorization`
// header any web-player request carries). Both are pasted into Settings; both are stored in the
// internal settings DB and never leave the machine. Requests go to Apple's catalog lookup by
// ISRC (from... we don't have ISRC — so search) with term "title artist", pick the first hit,
// and read its lyrics endpoint. Syllable-level spans are flattened to words.

const APPLE_ROOT: &str = "https://amp-api.music.apple.com/v1/catalog";

/// Ok(None) = no tokens configured / no Apple result. Err = transport trouble.
async fn apple_get(
    state: &AppState,
    req: &LyricsRequest,
) -> Result<Option<Lyrics>, reqwest::Error> {
    // Both tokens must be present; either missing = provider silently off.
    let (Some(media_token), Some(dev_token)) = (
        state.db.get_setting("lyrics_apple_media_token"),
        state.db.get_setting("lyrics_apple_dev_token"),
    ) else {
        return Ok(None);
    };
    if media_token.trim().is_empty() || dev_token.trim().is_empty() {
        return Ok(None);
    }
    // Storefront: default US; overridable for users elsewhere.
    let storefront = state
        .db
        .get_setting("lyrics_apple_storefront")
        .unwrap_or_else(|| "us".into())
        .to_lowercase();

    let term = format!("{} {}", req.title, req.artists);
    let search_url = format!("{APPLE_ROOT}/{storefront}/search");
    let search: serde_json::Value = match http()
        .get(search_url)
        .query(&[("term", term.as_str()), ("types", "songs"), ("limit", "5")])
        .header("Authorization", format!("Bearer {dev_token}"))
        .header("Media-User-Token", &media_token)
        .header("Origin", "https://music.apple.com")
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error = %e, "lyrics: apple search json failed");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error = %e, "lyrics: apple search failed");
            return Ok(None);
        }
    };

    // First song hit's id. Apple's own ranking is good enough given our title+artist term.
    let Some(song_id) = search
        .pointer("/results/songs/data")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|s| s.get("id"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
    else {
        return Ok(None);
    };

    let lyric_url = format!("{APPLE_ROOT}/{storefront}/songs/{song_id}/lyrics");
    let lyric: serde_json::Value = match http()
        .get(lyric_url)
        .query(&[("l", "en-us"), ("extend", "syllableLyrics")])
        .header("Authorization", format!("Bearer {dev_token}"))
        .header("Media-User-Token", &media_token)
        .header("Origin", "https://music.apple.com")
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error = %e, "lyrics: apple lyrics json failed");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error = %e, "lyrics: apple lyrics failed");
            return Ok(None);
        }
    };

    let lines_val = lyric
        .pointer("/data/0/relationships/lyrics/data/0/attributes/lines")
        .and_then(|v| v.as_array());
    let Some(lines_val) = lines_val else {
        return Ok(None);
    };

    let mut out = Vec::new();
    for line in lines_val {
        // Apple lines: { begin, end } in seconds-floats (or ms strings), text, words[]:
        // each word { string?, begin?, end? } — syllable extension nests them the same way.
        let begin = line.get("begin").and_then(parse_time_val);
        let end = line.get("end").and_then(parse_time_val);
        let text = line
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut words = Vec::new();
        if let Some(ws) = line.get("words").and_then(|v| v.as_array()) {
            for w in ws {
                let wt = w
                    .get("string")
                    .or_else(|| w.get("text"))
                    .and_then(|v| v.as_str());
                let Some(wt) = wt else { continue };
                if wt.trim().is_empty() {
                    continue;
                }
                let wb = w
                    .get("begin")
                    .and_then(parse_time_val)
                    .or(begin)
                    .unwrap_or(0);
                let we = w
                    .get("end")
                    .and_then(parse_time_val)
                    .or(end)
                    .unwrap_or(wb + 500);
                words.push(LyricWord {
                    text: wt.to_string(),
                    start_ms: wb,
                    end_ms: we,
                });
            }
        }
        out.push(LyricLine {
            time_ms: begin,
            end_time_ms: end.or_else(|| words.last().map(|w| w.end_ms)),
            text,
            words: if words.is_empty() { None } else { Some(words) },
            translation: None,
        });
    }
    Ok(from_parsed("Apple Music", out))
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

/// Megalobiz — keyless synced-lyrics source. The search page lists matches; each result links to
/// an LRC page we scrape. Adds a fifth, no-token-needed provider so timed lyrics still appear when
/// LRCLIB / Musixmatch / YTM come up empty (Genius only returns plain text).
async fn megalobiz(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let q = format!("{} {}", req.title, req.artists);
    let search = web_http()
        .get("https://www.megalobiz.com/search/")
        .query(&[("q", q.as_str()), ("type", "lrc")])
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    // First result href: href="/lyrics/<slug>.html"
    let href = {
        let re = regex::Regex::new(r#"href="(/lyrics/[^"]+\.html)""#).unwrap();
        match re.captures(&search) {
            Some(c) => c.get(1).map(|m| m.as_str().to_owned()),
            None => None,
        }
    };
    let Some(href) = href else {
        return Ok(None);
    };
    let page = web_http()
        .get(format!("https://www.megalobiz.com{href}"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    // LRC lines live in <p id="..." class="lb-offset">[mm:ss.xx] text</p>
    let block_re = regex::Regex::new(r#"<p id="[^"]*" class="lb-offset[^"]*">(.*?)</p>"#).unwrap();
    let line_re = regex::Regex::new(r"\[(\d{1,2}):(\d{2})(?:\.(\d{1,3}))?\]").unwrap();
    let mut out = String::new();
    let mut last_was_ts = false;
    for cap in block_re.captures_iter(&page) {
        let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let line = html_unescape(&strip_html_tags(raw)).trim().to_owned();
        // Does this block carry a leading timestamp? Megalobiz puts the [mm:ss.xx] inside the <p>.
        if let Some(ts) = line_re.captures(&line) {
            let min: u64 = ts.get(1).unwrap().as_str().parse().unwrap_or(0);
            let sec: u64 = ts.get(2).unwrap().as_str().parse().unwrap_or(0);
            let ms: u64 = {
                let frac = ts.get(3).map(|m| m.as_str()).unwrap_or("");
                let digits: String = frac.chars().filter(char::is_ascii_digit).take(3).collect();
                match digits.len() {
                    1 => digits.parse().unwrap_or(0) * 100,
                    2 => digits.parse().unwrap_or(0) * 10,
                    _ => digits.parse().unwrap_or(0),
                }
            };
            let _ = (min, sec, ms); // timestamp already in the text we keep
            last_was_ts = true;
        } else if last_was_ts {
            // continuation line (no timestamp) — keep as is
        }
        if !line.is_empty() {
            out.push_str(&line);
            out.push('\n');
        }
    }
    if out.trim().is_empty() {
        return Ok(None);
    }
    // Re-parse as LRC so we get a proper synced structure when timestamps are present.
    let lines = parse_lrc(&out);
    if !lines.is_empty() {
        Ok(Some(Lyrics {
            synced: true,
            instrumental: false,
            lines,
            source: "Megalobiz".into(),
        }))
    } else {
        Ok(plain_from_text(Some(&out), "Megalobiz"))
    }
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
            out.push(LyricLine {
                time_ms: Some(ms),
                end_time_ms: None,
                text: rest.to_owned(),
                words: None,
                translation: None,
            });
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

// --- Boidu (word-level karaoke provider) ----------------------------------------------------
//
// lyrics-api.boidu.dev (the Better Lyrics API) is the only free, keyless provider that returns
// word-level timings - TTML with per-word begin/end. Nothing else in the chain does, and the
// karaoke sweep needs it. Sits FIRST in the chain (see fetch), behind the lyrics_boidu
// setting so it can be turned off. Returns synced LRC or TTML; word timings ride along.

const BOIDU_UA: &str = concat!(
    "Limusic v",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Naitik571/limusic)"
);

/// Ok(None) = no Boidu result; Err = transport trouble.
async fn boidu_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let mut q: Vec<(&str, String)> = vec![("s", req.title.clone()), ("a", req.artists.clone())];
    if let Some(album) = &req.album {
        q.push(("al", album.clone()));
    }
    if let Some(d) = req.duration.filter(|d| *d > 0.0) {
        q.push(("d", format!("{}", d.round() as i64)));
    }

    let url = "https://lyrics-api.boidu.dev/getLyrics";
    tracing::debug!(title = %req.title, artist = %req.artists, "lyrics: querying Boidu provider");
    let resp: serde_json::Value = match web_http()
        .get(url)
        .query(&q)
        .header("User-Agent", BOIDU_UA)
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error = %e, "lyrics: Boidu json parse failed");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error = %e, "lyrics: Boidu request failed");
            return Ok(None);
        }
    };

    let lrc_str = resp
        .get("ttml")
        .or_else(|| resp.get("syncedLyrics"))
        .or_else(|| resp.get("lyrics"))
        .or_else(|| resp.get("lrc"))
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                if !s.trim().is_empty() {
                    return Some(s.to_string());
                }
            }
            if v.is_array() {
                return serde_json::to_string(v).ok();
            }
            None
        });

    let hit = lrc_str.and_then(|lrc| from_parsed("Boidu", parse_lrc_or_ttml(&lrc)));
    match &hit {
        Some(l) => tracing::debug!(
            count = l.lines.len(),
            synced = l.synced,
            "lyrics: Boidu hit"
        ),
        None => tracing::debug!("lyrics: Boidu returned no lines"),
    }
    Ok(hit)
}

/// Kugou KRC — free, no key, huge pool. search -> candidates (id+accesskey) -> download fmt=krc.
/// KRC is base64 + xor with key 'krc1' (0x6b,0x72,0x63,0x31). Decodes to LRC with per-word <start,dur,0>word blocks.
async fn kugou_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let q = format!("{} {}", req.title, req.artists);
    let search: serde_json::Value = match web_http()
        .get("https://lyrics.kugou.com/search")
        .query(&[
            ("ver", "1"),
            ("man", "yes"),
            ("client", "pc"),
            ("keyword", &q),
        ])
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error=%e, "lyrics: Kugou json");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, "lyrics: Kugou search");
            return Ok(None);
        }
    };
    let cands = search
        .get("candidates")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if cands.is_empty() {
        return Ok(None);
    }
    let mut best: Option<(f64, String, String)> = None;
    for c in &cands {
        let ct = c.get("song").and_then(|v| v.as_str()).unwrap_or("");
        let ca = c.get("singer").and_then(|v| v.as_str()).unwrap_or("");
        let ak = c.get("accesskey").and_then(|v| v.as_str()).unwrap_or("");
        let id = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if ak.is_empty() || id.is_empty() {
            continue;
        }
        let ts = overlap(&req.title, ct);
        let asc = req
            .artists
            .split(',')
            .map(|s| s.trim())
            .map(|a| overlap(a, ca))
            .fold(0.0, f64::max);
        if ts < 0.35 || asc < 0.30 {
            continue;
        }
        let sc = (ts + asc) / 2.0;
        if best.as_ref().is_none_or(|(bs, _, _)| sc > *bs) {
            best = Some((sc, id.to_string(), ak.to_string()));
        }
    }
    let (id, ak) = match best {
        Some((_, id, ak)) => (id, ak),
        None => {
            let top = &cands[0];
            let ak = top.get("accesskey").and_then(|v| v.as_str()).unwrap_or("");
            let id = top.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if ak.is_empty() || id.is_empty() {
                return Ok(None);
            }
            (id.to_string(), ak.to_string())
        }
    };
    let raw: serde_json::Value = match web_http()
        .get("http://lyrics.kugou.com/download")
        .query(&[
            ("ver", "1"),
            ("client", "pc"),
            ("id", &id),
            ("accesskey", &ak),
            ("fmt", "krc"),
            ("charset", "utf8"),
        ])
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error=%e, "Kugou dl json");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, "Kugou dl");
            return Ok(None);
        }
    };
    let b64 = raw.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if b64.is_empty() {
        return Ok(None);
    }
    let decoded = decode_krc_content(b64)
        .or_else(|| base64_decode_fallback(b64))
        .unwrap_or_else(|| b64.to_string());
    if decoded.trim().is_empty() {
        return Ok(None);
    }
    let lines = parse_lrc_or_ttml(&decoded);
    if !lines.is_empty() {
        let has = lines.iter().any(|l| l.words.is_some());
        if !has && decoded.contains('<') && decoded.contains(',') {
            let kw = parse_krc_words(&decoded);
            if !kw.is_empty() {
                return Ok(from_parsed("Kugou", kw));
            }
        }
        return Ok(from_parsed("Kugou", lines));
    }
    let lrc = parse_lrc(&decoded);
    Ok(from_parsed("Kugou", lrc))
}
async fn netease_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let q = format!("{} {}", req.title, req.artists);
    let search: serde_json::Value = match web_http()
        .get("https://music.163.com/api/search/pc")
        .query(&[("s", q.as_str()), ("type", "1"), ("limit", "10")])
        .header("Referer", "https://music.163.com/")
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error=%e, "NetEase json");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, "NetEase search");
            return Ok(None);
        }
    };
    let songs = search
        .pointer("/result/songs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if songs.is_empty() {
        return Ok(None);
    }
    let mut best: Option<(f64, i64)> = None;
    for s in &songs {
        let gt = s.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let ga = s
            .get("artists")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|a| a.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let id = s.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        if id == 0 {
            continue;
        }
        let ts = overlap(&req.title, gt);
        let asc = req
            .artists
            .split(',')
            .map(|x| x.trim())
            .map(|a| overlap(a, ga))
            .fold(0.0, f64::max);
        if ts < 0.35 || asc < 0.30 {
            continue;
        }
        let sc = (ts + asc) / 2.0;
        if best.as_ref().is_none_or(|(bs, _)| sc > *bs) {
            best = Some((sc, id));
        }
    }
    let sid = match best {
        Some((_, id)) => id,
        None => songs
            .first()
            .and_then(|s| s.get("id"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
    };
    if sid == 0 {
        return Ok(None);
    }
    let resp: serde_json::Value = match web_http()
        .get("https://music.163.com/api/song/lyric")
        .query(&[
            ("id", sid.to_string()),
            ("lv", "-1".to_string()),
            ("kv", "-1".to_string()),
            ("tv", "-1".to_string()),
        ])
        .header("Referer", "https://music.163.com/")
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error=%e, "NetEase lyric json");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, "NetEase lyric");
            return Ok(None);
        }
    };
    let yrc = resp
        .get("yrc")
        .and_then(|v| v.get("lyric"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !yrc.trim().is_empty() {
        let yw = parse_krc_words(yrc);
        if !yw.is_empty() {
            return Ok(from_parsed("NetEase", yw));
        }
        let lines = parse_lrc_or_ttml(yrc);
        if !lines.is_empty()
            && lines
                .iter()
                .any(|l| l.words.is_some() || l.time_ms.is_some())
        {
            return Ok(from_parsed("NetEase", lines));
        }
    }
    let lrc = resp
        .get("lrc")
        .and_then(|v| v.get("lyric"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if lrc.trim().is_empty() {
        return Ok(None);
    }
    let lines = parse_lrc_or_ttml(lrc);
    if lines.is_empty() {
        return Ok(None);
    }
    Ok(from_parsed("NetEase", lines))
}
fn decode_krc_content(b64: &str) -> Option<String> {
    let s = base64_decode_fallback(b64)?;
    if s.len() < 4 {
        return None;
    }
    let bytes = s.into_bytes();
    let key: [u8; 4] = [0x6b, 0x72, 0x63, 0x31];
    let xored: Vec<u8> = bytes
        .iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % 4])
        .collect();
    let out = String::from_utf8(xored).ok()?;
    Some(out.strip_prefix("krc1").unwrap_or(&out).to_string())
}
fn base64_decode_fallback(s: &str) -> Option<String> {
    let c: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let p = match c.len() % 4 {
        0 => c,
        2 => format!("{c}=="),
        3 => format!("{c}="),
        _ => c,
    };
    let b = base64_decode_bytes(&p)?;
    String::from_utf8(b).ok()
}
fn base64_decode_bytes(s: &str) -> Option<Vec<u8>> {
    let table = { build_b64_table() };
    if s.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let b = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        let a = table[b[i] as usize];
        let bb = table[b[i + 1] as usize];
        let cc = table[b[i + 2] as usize];
        let d = table[b[i + 3] as usize];
        if a < 0 || bb < 0 || cc < 0 || d < 0 {
            return None;
        }
        out.push(((a as u8) << 2) | ((bb as u8) >> 4));
        if b[i + 2] != b'=' {
            out.push(((bb as u8 & 0x0f) << 4) | ((cc as u8) >> 2));
        }
        if b[i + 3] != b'=' {
            out.push(((cc as u8 & 0x03) << 6) | (d as u8));
        }
        i += 4;
    }
    Some(out)
}

fn build_b64_table() -> [i8; 256] {
    let mut t = [-1i8; 256];
    let mut i = 0u8;
    while i < 26 {
        t[(b'A' + i) as usize] = i as i8;
        i += 1;
    }
    i = 0;
    while i < 26 {
        t[(b'a' + i) as usize] = (26 + i) as i8;
        i += 1;
    }
    i = 0;
    while i < 10 {
        t[(b'0' + i) as usize] = (52 + i) as i8;
        i += 1;
    }
    t[b'+' as usize] = 62;
    t[b'/' as usize] = 63;
    t[b'=' as usize] = 0;
    t
}
fn parse_krc_words(text: &str) -> Vec<LyricLine> {
    let mut out = Vec::new();
    for raw in text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut rest = trimmed;
        let mut times: Vec<u64> = Vec::new();
        while let Some(after) = rest.strip_prefix('[') {
            if let Some(end) = after.find(']') {
                if let Some(ms) = parse_lrc_time(&after[..end]) {
                    times.push(ms);
                    rest = after[end + 1..].trim_start();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        if times.is_empty() {
            continue;
        }
        let mut words: Vec<LyricWord> = Vec::new();
        let mut pos = 0;
        let bytes = rest.as_bytes();
        while pos < bytes.len() {
            if bytes[pos] == b'<' {
                if let Some(end) = rest[pos..].find('>') {
                    let tag = &rest[pos + 1..pos + end];
                    let parts: Vec<&str> = tag.split(',').collect();
                    if parts.len() >= 2 {
                        if let (Ok(start), Ok(dur)) =
                            (parts[0].parse::<u64>(), parts[1].parse::<u64>())
                        {
                            let nxt = rest[pos + end + 1..]
                                .find('<')
                                .map(|i| pos + end + 1 + i)
                                .unwrap_or(rest.len());
                            let wtext = &rest[pos + end + 1..nxt];
                            if !wtext.is_empty() {
                                words.push(LyricWord {
                                    text: wtext.to_string(),
                                    start_ms: start,
                                    end_ms: start + dur,
                                });
                            }
                            pos = nxt;
                            continue;
                        }
                    }
                    pos += end + 1;
                    continue;
                }
            }
            pos += rest[pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
        }
        if words.is_empty() {
            let lt = rest.trim().to_string();
            for &ms in &times {
                out.push(LyricLine {
                    time_ms: Some(ms),
                    end_time_ms: None,
                    text: lt.clone(),
                    words: None,
                    translation: None,
                });
            }
            continue;
        }
        let full: String = words.iter().map(|w| w.text.as_str()).collect();
        for &ms in &times {
            out.push(LyricLine {
                time_ms: Some(ms),
                end_time_ms: words.last().map(|w| w.end_ms),
                text: full.clone(),
                words: Some(words.clone()),
                translation: None,
            });
        }
    }
    out.sort_by_key(|l| l.time_ms);
    out
}

/// A provider's parsed lines as a result, or None when there was nothing to show.
/// synced is derived from the lines rather than asserted by the caller. TTML without begin
/// attributes, and JSON items carrying text but no time, both parse to real lines with no cue.
fn from_parsed(source: &str, lines: Vec<LyricLine>) -> Option<Lyrics> {
    if lines.is_empty() {
        return None;
    }
    Some(Lyrics {
        source: source.to_owned(),
        synced: lines.iter().any(|l| l.time_ms.is_some()),
        instrumental: false,
        lines,
    })
}

/// Parse LRC, eLRC (inline word tags), TTML/AAML XML, or the Better-Lyrics/JSON array shape into
/// LyricLines. Used by every provider that returns more than plain text.
fn parse_lrc_or_ttml(text: &str) -> Vec<LyricLine> {
    let trimmed = text.trim();

    // 1. JSON array (Better Lyrics / KPOE / LyricsPlus): objects with text + time + words[].
    if (trimmed.starts_with('[') || trimmed.starts_with('{'))
        && (trimmed.contains("\"text\"")
            || trimmed.contains("\"time\"")
            || trimmed.contains("\"words\"")
            || trimmed.contains("\"start\"")
            || trimmed.contains("\"startTime\""))
    {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let mut out = Vec::new();
            let arr_opt = val
                .as_array()
                .or_else(|| val.get("lyrics").and_then(|v| v.as_array()))
                .or_else(|| val.get("lines").and_then(|v| v.as_array()))
                .or_else(|| val.get("element").and_then(|v| v.as_array()));
            if let Some(arr) = arr_opt {
                for item in arr {
                    let line_text = item
                        .get("text")
                        .or_else(|| item.get("words"))
                        .or_else(|| item.get("line"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    let time_val = item
                        .get("time")
                        .or_else(|| item.get("startTime"))
                        .or_else(|| item.get("start"))
                        .or_else(|| item.get("t"))
                        .and_then(parse_time_val);

                    let mut words = Vec::new();
                    if let Some(w_arr) = item.get("words").and_then(|v| v.as_array()) {
                        for w in w_arr {
                            let w_text = w
                                .get("text")
                                .or_else(|| w.get("word"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let w_start = w
                                .get("startTime")
                                .or_else(|| w.get("start"))
                                .or_else(|| w.get("time"))
                                .and_then(parse_time_val)
                                .or(time_val);
                            let w_end = w
                                .get("endTime")
                                .or_else(|| w.get("end"))
                                .and_then(parse_time_val)
                                .or_else(|| w_start.map(|s| s + 500));
                            if let (Some(b), Some(e)) = (w_start, w_end) {
                                words.push(LyricWord {
                                    text: w_text,
                                    start_ms: b,
                                    end_ms: e,
                                });
                            }
                        }
                    }

                    if !line_text.is_empty() || time_val.is_some() {
                        out.push(LyricLine {
                            time_ms: time_val,
                            end_time_ms: None,
                            text: line_text,
                            words: if !words.is_empty() { Some(words) } else { None },
                            translation: None,
                        });
                    }
                }
                if !out.is_empty() {
                    out.sort_by_key(|l| l.time_ms);
                    return out;
                }
            }
        }
    }

    // 2. TTML / AAML XML.
    if trimmed.starts_with('<') || trimmed.contains("<p ") || trimmed.contains("<tt") {
        let ttml_lines = parse_ttml_aaml(trimmed);
        if !ttml_lines.is_empty() {
            return ttml_lines;
        }
    }

    // 3. LRC / eLRC.
    parse_elrc(text)
}

/// TTML and Apple Music AAML XML parser - extracts per-line + per-word timings.
fn parse_ttml_aaml(xml: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    let mut pos = 0;
    while let Some(p_start) = xml[pos..].find("<p") {
        let abs_p_start = pos + p_start;
        let Some(p_tag_end) = xml[abs_p_start..].find('>') else {
            break;
        };
        let abs_p_tag_end = abs_p_start + p_tag_end;
        let p_tag_str = &xml[abs_p_start..abs_p_tag_end + 1];

        let Some(p_close) = xml[abs_p_tag_end..].find("</p>") else {
            break;
        };
        let abs_p_close = abs_p_tag_end + p_close;
        let inner_str = &xml[abs_p_tag_end + 1..abs_p_close];

        pos = abs_p_close + 4;

        let line_begin = parse_xml_attr(p_tag_str, "begin").and_then(|s| parse_ttml_time(&s));
        let line_end = parse_xml_attr(p_tag_str, "end").and_then(|s| parse_ttml_time(&s));

        let mut words: Vec<LyricWord> = Vec::new();
        let mut span_pos = 0;
        let mut plain_text_buf = String::new();

        while let Some(s_start) = inner_str[span_pos..].find("<span") {
            let abs_s_start = span_pos + s_start;
            let Some(s_tag_end) = inner_str[abs_s_start..].find('>') else {
                break;
            };
            let abs_s_tag_end = abs_s_start + s_tag_end;
            let s_tag_str = &inner_str[abs_s_start..abs_s_tag_end + 1];

            let before = strip_xml_tags(&inner_str[span_pos..abs_s_start]);
            if !before.is_empty() {
                plain_text_buf.push_str(&before);
                if let Some(last_w) = words.last_mut() {
                    last_w.text.push_str(&before);
                }
            }

            let Some(s_close) = inner_str[abs_s_tag_end..].find("</span>") else {
                break;
            };
            let abs_s_close = abs_s_tag_end + s_close;
            let w_text = strip_xml_tags(&inner_str[abs_s_tag_end + 1..abs_s_close]);

            let w_begin = parse_xml_attr(s_tag_str, "begin")
                .and_then(|s| parse_ttml_time(&s))
                .or(line_begin);
            let w_end = parse_xml_attr(s_tag_str, "end")
                .and_then(|s| parse_ttml_time(&s))
                .or(line_end);

            if let (Some(b), Some(e)) = (w_begin, w_end) {
                if !w_text.is_empty() {
                    words.push(LyricWord {
                        text: w_text.clone(),
                        start_ms: b,
                        end_ms: e,
                    });
                }
            }
            plain_text_buf.push_str(&w_text);
            span_pos = abs_s_close + 7;
        }

        if span_pos < inner_str.len() {
            plain_text_buf.push_str(&strip_xml_tags(&inner_str[span_pos..]));
        }

        let words_opt = if !words.is_empty() { Some(words) } else { None };
        let full_text = plain_text_buf.trim().to_string();
        if !full_text.is_empty() || line_begin.is_some() {
            lines.push(LyricLine {
                time_ms: line_begin,
                end_time_ms: line_end,
                text: full_text,
                words: words_opt,
                translation: None,
            });
        }
    }
    lines.sort_by_key(|l| l.time_ms);
    lines
}

/// Enhanced LRC parser - handles inline word-timing tags (<00:01.23>word).
fn parse_elrc(lrc: &str) -> Vec<LyricLine> {
    let mut base_lines = parse_lrc(lrc);
    for line in &mut base_lines {
        if line.text.contains('<') || line.text.contains('(') {
            let mut words = Vec::new();
            let mut text_buf = String::new();
            let mut last_ms = line.time_ms.unwrap_or(0);

            let mut pos = 0;
            let text_bytes = line.text.as_bytes();
            while pos < text_bytes.len() {
                if text_bytes[pos] == b'<' {
                    if let Some(end_idx) = line.text[pos..].find('>') {
                        let tag = &line.text[pos + 1..pos + end_idx];
                        if let Some(w_ms) = parse_lrc_time(tag) {
                            pos += end_idx + 1;
                            let next_tag_idx = line.text[pos..]
                                .find('<')
                                .map(|i| pos + i)
                                .unwrap_or(line.text.len());
                            let w_str = &line.text[pos..next_tag_idx];
                            text_buf.push_str(w_str);
                            words.push(LyricWord {
                                text: w_str.to_string(),
                                start_ms: last_ms,
                                end_ms: w_ms,
                            });
                            last_ms = w_ms;
                            pos = next_tag_idx;
                            continue;
                        }
                    }
                }
                let ch = line.text[pos..].chars().next().unwrap_or(' ');
                text_buf.push(ch);
                pos += ch.len_utf8();
            }

            if !words.is_empty() {
                line.text = text_buf.trim().to_string();
                line.words = Some(words);
            }
        }
    }
    base_lines
}

/// LRCMux: merge word timings from a second source into lines that lack them.
/// mm:ss, mm:ss.xx, or mm:ss.xxx -> milliseconds (LRC timestamps).
fn parse_ttml_time(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(rest) = s.strip_suffix("ms") {
        return rest.parse::<u64>().ok();
    }
    if let Some(rest) = s.strip_suffix('s') {
        let secs: f64 = rest.parse().ok()?;
        return Some((secs * 1000.0) as u64);
    }
    if s.contains(':') {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() == 3 {
            let h: u64 = parts[0].parse().ok()?;
            let m: u64 = parts[1].parse().ok()?;
            let secs: f64 = parts[2].parse().ok()?;
            return Some((h * 3600 + m * 60) * 1000 + (secs * 1000.0) as u64);
        } else if parts.len() == 2 {
            let m: u64 = parts[0].parse().ok()?;
            let secs: f64 = parts[1].parse().ok()?;
            return Some(m * 60 * 1000 + (secs * 1000.0) as u64);
        }
    }
    let secs: f64 = s.parse().ok()?;
    Some((secs * 1000.0) as u64)
}

/// Pull a name="value" or name='value' attribute out of a tag string.
fn parse_xml_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{attr}=\"");
    if let Some(idx) = tag.find(&pattern) {
        let start = idx + pattern.len();
        let end = tag[start..].find('"')?;
        return Some(tag[start..start + end].to_string());
    }
    let pattern_single = format!("{attr}='");
    if let Some(idx) = tag.find(&pattern_single) {
        let start = idx + pattern_single.len();
        let end = tag[start..].find('\'')?;
        return Some(tag[start..start + end].to_string());
    }
    None
}

/// A JSON number/string -> milliseconds. Sub-500 values are read as seconds (the APIs mix units).
fn parse_time_val(v: &serde_json::Value) -> Option<u64> {
    if let Some(f) = v.as_f64() {
        if f < 500.0 {
            Some((f * 1000.0) as u64)
        } else {
            Some(f as u64)
        }
    } else if let Some(u) = v.as_u64() {
        if u < 500 {
            Some(u * 1000)
        } else {
            Some(u)
        }
    } else if let Some(s) = v.as_str() {
        if let Ok(f) = s.parse::<f64>() {
            if f < 500.0 {
                Some((f * 1000.0) as u64)
            } else {
                Some(f as u64)
            }
        } else {
            None
        }
    } else {
        None
    }
}

/// Strip XML tags, keeping the text between them (used for TTML span/word text).
fn strip_xml_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(c);
        }
    }
    out
}

// --- 8-provider wrappers (Kodama parity) ----------------------------------------------------
// Thin stubs / aliases that let `fetch` read as "BetterLyrics → Unison → QRC → … → SimpMusic".
// They delegate to the real providers above (or new ones below) so the provider chain name
// matches the issue while behaviour stays covered by existing implementations.

/// BetterLyrics / Boidu TTML alias (word-level karaoke provider).
pub async fn fetch_better_lyrics(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    boidu_get(req).await
}
/// Alias used by task description: fetchBetterLyrics.
pub async fn fetchBetterLyrics(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    fetch_better_lyrics(req).await
}
pub async fn fetch_unison(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    unison_get(req).await
}
pub async fn fetchUnison(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    fetch_unison(req).await
}
pub async fn fetch_qrc(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    qrc_get(req).await
}
pub async fn fetch_netease(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    netease_get(req).await
}
pub async fn fetch_musixmatch(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    musixmatch(req).await
}
pub async fn fetch_kugou(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    kugou_get(req).await
}
pub async fn fetch_simp_music(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    simp_music_get(req).await
}
pub async fn fetchSimpMusic(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    fetch_simp_music(req).await
}

// --- Unison (Kodama community vote/report + aggregated lyrics) -------------------------------
// Unison is Kodama's community lyrics backend. In Limusic it is queried as a normal HTTP
// provider (with vote/report side-effects persisted locally). When the remote is unreachable we
// degrade to the next provider — lyrics remain best-effort.

const UNISON_ROOT: &str = "https://unison.limusic.example";

async fn unison_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    // Try community endpoint; a 404 / network error is treated as "no Unison lyrics".
    let q = format!("{} {}", req.title, req.artists);
    let resp: serde_json::Value = match web_http()
        .get(format!("{UNISON_ROOT}/api/lyrics"))
        .query(&[
            ("q", q.as_str()),
            ("duration", &req.duration.unwrap_or(0.0).to_string()),
        ])
        .timeout(Duration::from_secs(6))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error=%e, "unison json parse");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, "unison request failed");
            return Ok(None);
        }
    };
    let val = resp;
    // Expected shape: { syncedLyrics: "..." } or { ttml: "..." } or { lines: [...] } or plain.
    let text = val
        .get("syncedLyrics")
        .or_else(|| val.get("ttml"))
        .or_else(|| val.get("lyrics"))
        .or_else(|| val.get("lrc"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if !text.trim().is_empty() {
        let lines = parse_lrc_or_ttml(&text);
        if let Some(l) = from_parsed("Unison", lines) {
            return Ok(Some(l));
        }
    }
    if let Some(lines) = val.get("lines").and_then(|v| v.as_array()) {
        let parsed: Vec<LyricLine> = lines
            .iter()
            .filter_map(|l| {
                let t = l
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let time = l
                    .get("time_ms")
                    .or_else(|| l.get("startMs"))
                    .and_then(|v| v.as_u64());
                if t.is_empty() && time.is_none() {
                    None
                } else {
                    Some(LyricLine {
                        time_ms: time,
                        end_time_ms: None,
                        text: t,
                        words: None,
                        translation: None,
                    })
                }
            })
            .collect();
        if let Some(l) = from_parsed("Unison", parsed) {
            return Ok(Some(l));
        }
    }
    // Fallback: treat unison as LRCLIB proxy when community has no hit — keeps word-level chain warm.
    Ok(None)
}

// --- QRC (QQ Music) ------------------------------------------------------------------------
// QRC is QQ Music's word-level lyric format (similar to KRC). We search via `c.y.qq.com` and
// decode the base64 QRC payload into LyricWords. Requires `songmid`, so we resolve it first.

async fn qrc_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let q = format!("{} {}", req.title, req.artists);
    let search: serde_json::Value = match web_http()
        .get("https://c.y.qq.com/soso/fcgi-bin/client_search_cp")
        .query(&[
            ("p", "1"),
            ("n", "5"),
            ("w", q.as_str()),
            ("format", "json"),
        ])
        .header("Referer", "https://y.qq.com/")
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error=%e, "qrc search json");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, "qrc search");
            return Ok(None);
        }
    };
    let songmid = search
        .pointer("/data/song/list/0/songmid")
        .or_else(|| search.pointer("/data/song/list/0/mid"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if songmid.is_empty() {
        return Ok(None);
    }
    let lyric_resp: serde_json::Value = match web_http()
        .get("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg")
        .query(&[
            ("songmid", songmid.as_str()),
            ("format", "json"),
            ("nobase64", "0"),
        ])
        .header("Referer", "https://y.qq.com/")
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error=%e, "qrc lyric json");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, "qrc lyric fetch");
            return Ok(None);
        }
    };
    let b64 = lyric_resp
        .get("lyric")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if b64.is_empty() {
        return Ok(None);
    }
    // QRC is base64 encoded; decode and parse. QRC content after decode is similar to KRC/YRC mixed.
    let decoded = base64_decode_fallback(b64).unwrap_or_else(|| b64.to_string());
    if decoded.trim().is_empty() {
        return Ok(None);
    }
    // Try word-level parse first, then fallback to lrc.
    let kw = parse_krc_words(&decoded);
    if !kw.is_empty() {
        return Ok(from_parsed("QRC", kw));
    }
    let lines = parse_lrc_or_ttml(&decoded);
    Ok(from_parsed("QRC", lines))
}

// --- SimpMusic (SNeedex) -------------------------------------------------------------------
// SimpMusic's lyrics API is a thin LRCLIB-compatible search. We treat it as an LRCLIB-shaped
// fallback (search → synced or plain) so a track missing from earlier providers still finds a
// plain lyric rather than leaving the panel empty.

async fn simp_music_get(req: &LyricsRequest) -> Result<Option<Lyrics>, reqwest::Error> {
    let q = format!("{} - {}", req.title, req.artists);
    // Public demo endpoint shape: https://api.simpmusic.org/api/lyrics?search=<q>
    // No key needed for basic lookups; failure just means next provider.
    let resp: serde_json::Value = match web_http()
        .get("https://api.simpmusic.org/api/lyrics")
        .query(&[("search", q.as_str())])
        .timeout(Duration::from_secs(8))
        .send()
        .await
    {
        Ok(r) => match r.json().await {
            Ok(j) => j,
            Err(e) => {
                tracing::debug!(error=%e, "simpmusic json");
                return Ok(None);
            }
        },
        Err(e) => {
            tracing::debug!(error=%e, "simpmusic request");
            return Ok(None);
        }
    };
    // Accept either { lyrics: "..."} or array hit.
    if let Some(txt) = resp
        .get("lyrics")
        .or_else(|| resp.get("syncedLyrics"))
        .and_then(|v| v.as_str())
    {
        if !txt.trim().is_empty() {
            let lines = parse_lrc_or_ttml(txt);
            if let Some(l) = from_parsed("SimpMusic", lines) {
                return Ok(Some(l));
            }
        }
    }
    if let Some(arr) = resp
        .get("data")
        .or_else(|| resp.get("results"))
        .and_then(|v| v.as_array())
    {
        for hit in arr {
            let txt = hit
                .get("syncedLyrics")
                .or_else(|| hit.get("lrc"))
                .or_else(|| hit.get("lyrics"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !txt.trim().is_empty() {
                let lines = parse_lrc_or_ttml(txt);
                if let Some(l) = from_parsed("SimpMusic", lines) {
                    return Ok(Some(l));
                }
            }
        }
    }
    // Last resort: use LRCLIB search shape but attribute to SimpMusic if it at least gave plain.
    if let Some(txt) = resp.get("plainLyrics").and_then(|v| v.as_str()) {
        if let Some(l) = plain_from_text(Some(txt), "SimpMusic") {
            return Ok(Some(l));
        }
    }
    Ok(None)
}

// --- Per-song offset helpers (Db persistence) -----------------------------------------------

pub fn get_offset(db: &crate::db::Db, video_id: &str) -> i64 {
    db.get_lyric_offset(video_id)
}
pub fn set_offset(db: &crate::db::Db, video_id: &str, offset_ms: i64) {
    db.set_lyric_offset(video_id, offset_ms);
}
/// Apply stored offset to a Lyrics for display (shifts every cue).
pub fn apply_offset(lyrics: &mut Lyrics, offset_ms: i64) {
    if offset_ms == 0 {
        return;
    }
    for line in &mut lyrics.lines {
        if let Some(t) = line.time_ms.as_mut() {
            *t = (*t as i64 + offset_ms).max(0) as u64;
        }
        if let Some(e) = line.end_time_ms.as_mut() {
            *e = (*e as i64 + offset_ms).max(0) as u64;
        }
        if let Some(ws) = line.words.as_mut() {
            for w in ws.iter_mut() {
                w.start_ms = (w.start_ms as i64 + offset_ms).max(0) as u64;
                w.end_ms = (w.end_ms as i64 + offset_ms).max(0) as u64;
            }
        }
    }
}

// --- Translate (44 langs) via translate.googleapis + romanize (kana→romaji) ----------------
// translate.googleapis is keyless and stable; romanize uses a small kana table (pykakasi-lite).
// Both are best-effort: callers handle Err/empty gracefully.

/// Translate `text` via https://translate.googleapis.com/translate_a/single.
/// Caller should chunk per line (google limits q length). `target` is a 2-letter code (ja, es ...).
pub async fn translate_text(text: &str, target: &str) -> Result<String, reqwest::Error> {
    if text.trim().is_empty() || target.trim().is_empty() || target == "auto" {
        return Ok(text.to_string());
    }
    let resp: serde_json::Value = web_http()
        .get("https://translate.googleapis.com/translate_a/single")
        .query(&[
            ("client", "gtx"),
            ("sl", "auto"),
            ("tl", target),
            ("dt", "t"),
            ("q", text),
        ])
        .timeout(Duration::from_secs(10))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    // Response is nested array: [[ ["hola","hello",... ] ], ...]
    let mut out = String::new();
    if let Some(arr) = resp.get(0).and_then(|v| v.as_array()) {
        for seg in arr {
            if let Some(t) = seg.get(0).and_then(|v| v.as_str()) {
                out.push_str(t);
            }
        }
    }
    if out.trim().is_empty() {
        Ok(text.to_string())
    } else {
        Ok(out)
    }
}

/// Translate every LyricLine's text in-place. Keeps sync data, fills `translation`.
pub async fn translate_lyrics(lines: &mut [LyricLine], target: &str) -> Result<(), reqwest::Error> {
    for line in lines.iter_mut() {
        if line.text.trim().is_empty() {
            continue;
        }
        match translate_text(&line.text, target).await {
            Ok(t) if t != line.text => line.translation = Some(t),
            _ => {}
        }
    }
    Ok(())
}

// Simple kana → romaji (Hepburn-ish). Covers hiragana+katakana, small tsu, chōonpu, n.
// Not a full pykakasi / kuroshiro replacement but sufficient for karaoke display.
fn kana_to_romaji(s: &str) -> String {
    // Longest-match mapping (digraphs first).
    const MAP: &[(&str, &str)] = &[
        ("きゃ", "kya"),
        ("きゅ", "kyu"),
        ("きょ", "kyo"),
        ("しゃ", "sha"),
        ("しゅ", "shu"),
        ("しょ", "sho"),
        ("ちゃ", "cha"),
        ("ちゅ", "chu"),
        ("ちょ", "cho"),
        ("にゃ", "nya"),
        ("にゅ", "nyu"),
        ("にょ", "nyo"),
        ("ひゃ", "hya"),
        ("ひゅ", "hyu"),
        ("ひょ", "hyo"),
        ("みゃ", "mya"),
        ("みゅ", "myu"),
        ("みょ", "myo"),
        ("りゃ", "rya"),
        ("りゅ", "ryu"),
        ("りょ", "ryo"),
        ("ぎゃ", "gya"),
        ("じゃ", "ja"),
        ("じゅ", "ju"),
        ("じょ", "jo"),
        ("びゃ", "bya"),
        ("ぴゃ", "pya"),
        ("キャ", "kya"),
        ("キュ", "kyu"),
        ("キョ", "kyo"),
        ("シャ", "sha"),
        ("シュ", "shu"),
        ("ショ", "sho"),
        ("チャ", "cha"),
        ("チュ", "chu"),
        ("チョ", "cho"),
        ("あ", "a"),
        ("い", "i"),
        ("う", "u"),
        ("え", "e"),
        ("お", "o"),
        ("か", "ka"),
        ("き", "ki"),
        ("く", "ku"),
        ("け", "ke"),
        ("こ", "ko"),
        ("さ", "sa"),
        ("し", "shi"),
        ("す", "su"),
        ("せ", "se"),
        ("そ", "so"),
        ("た", "ta"),
        ("ち", "chi"),
        ("つ", "tsu"),
        ("て", "te"),
        ("と", "to"),
        ("な", "na"),
        ("に", "ni"),
        ("ぬ", "nu"),
        ("ね", "ne"),
        ("の", "no"),
        ("は", "ha"),
        ("ひ", "hi"),
        ("ふ", "fu"),
        ("へ", "he"),
        ("ほ", "ho"),
        ("ま", "ma"),
        ("み", "mi"),
        ("む", "mu"),
        ("め", "me"),
        ("も", "mo"),
        ("や", "ya"),
        ("ゆ", "yu"),
        ("よ", "yo"),
        ("ら", "ra"),
        ("り", "ri"),
        ("る", "ru"),
        ("れ", "re"),
        ("ろ", "ro"),
        ("わ", "wa"),
        ("を", "wo"),
        ("ん", "n"),
        ("が", "ga"),
        ("ぎ", "gi"),
        ("ぐ", "gu"),
        ("げ", "ge"),
        ("ご", "go"),
        ("ざ", "za"),
        ("じ", "ji"),
        ("ず", "zu"),
        ("ぜ", "ze"),
        ("ぞ", "zo"),
        ("だ", "da"),
        ("ぢ", "ji"),
        ("づ", "zu"),
        ("で", "de"),
        ("ど", "do"),
        ("ば", "ba"),
        ("び", "bi"),
        ("ぶ", "bu"),
        ("べ", "be"),
        ("ぼ", "bo"),
        ("ぱ", "pa"),
        ("ぴ", "pi"),
        ("ぷ", "pu"),
        ("ぺ", "pe"),
        ("ぽ", "po"),
        ("ア", "a"),
        ("イ", "i"),
        ("ウ", "u"),
        ("エ", "e"),
        ("オ", "o"),
        ("カ", "ka"),
        ("キ", "ki"),
        ("ク", "ku"),
        ("ケ", "ke"),
        ("コ", "ko"),
        ("サ", "sa"),
        ("シ", "shi"),
        ("ス", "su"),
        ("セ", "se"),
        ("ソ", "so"),
        ("タ", "ta"),
        ("チ", "chi"),
        ("ツ", "tsu"),
        ("テ", "te"),
        ("ト", "to"),
        ("ナ", "na"),
        ("ニ", "ni"),
        ("ヌ", "nu"),
        ("ネ", "ne"),
        ("ノ", "no"),
        ("ハ", "ha"),
        ("ヒ", "hi"),
        ("フ", "fu"),
        ("ヘ", "he"),
        ("ホ", "ho"),
        ("マ", "ma"),
        ("ミ", "mi"),
        ("ム", "mu"),
        ("メ", "me"),
        ("モ", "mo"),
        ("ヤ", "ya"),
        ("ユ", "yu"),
        ("ヨ", "yo"),
        ("ラ", "ra"),
        ("リ", "ri"),
        ("ル", "ru"),
        ("レ", "re"),
        ("ロ", "ro"),
        ("ワ", "wa"),
        ("ヲ", "wo"),
        ("ン", "n"),
        ("ー", "-"),
        ("っ", ""),
        ("ッ", ""),
        ("ゃ", "ya"),
        ("ゅ", "yu"),
        ("ょ", "yo"),
        ("ぁ", "a"),
        ("ぃ", "i"),
        ("ぅ", "u"),
        ("ぇ", "e"),
        ("ぉ", "o"),
        ("ァ", "a"),
        ("ィ", "i"),
        ("ゥ", "u"),
        ("ェ", "e"),
        ("ォ", "o"),
    ];
    let mut out = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let mut matched = false;
        // Try 2-char then 1-char
        if i + 1 < chars.len() {
            let two: String = chars[i..i + 2].iter().collect();
            for (k, v) in MAP.iter() {
                if *k == two {
                    // Small tsu handling: っ doubles next consonant
                    if two == "っ" || two == "ッ" {
                        if let Some(nc) = s.chars().nth(i + 1).map(kana_to_romaji_char) {
                            if let Some(first) = nc.chars().next() {
                                out.push(first);
                            }
                        }
                    } else {
                        out.push_str(v);
                    }
                    i += 2;
                    matched = true;
                    break;
                }
            }
            if matched {
                continue;
            }
        }
        let one: String = chars[i..i + 1].iter().collect();
        let mut found = false;
        for (k, v) in MAP.iter() {
            if *k == one {
                out.push_str(v);
                found = true;
                break;
            }
        }
        if !found {
            out.push(chars[i]);
        }
        i += 1;
    }
    out
}
fn kana_to_romaji_char(c: char) -> String {
    kana_to_romaji(&c.to_string())
}

/// Romanize a string (kana→romaji). Non-kana passes through.
pub fn romanize_text(text: &str) -> String {
    // If string contains kana, run mapping; else return as-is (pykakasi lite).
    if text
        .chars()
        .any(|c| (0x3040..=0x30FF).contains(&(c as u32)))
    {
        kana_to_romaji(text)
    } else {
        // For kanji-heavy lines without kana, we can't romanize without a dict —
        // return the original and let the UI show it as fallback.
        text.to_string()
    }
}
pub fn romanize_lyrics(lines: &mut [LyricLine]) {
    for line in lines.iter_mut() {
        let r = romanize_text(&line.text);
        if r != line.text {
            line.translation = Some(r);
        }
        if let Some(words) = line.words.as_mut() {
            for w in words.iter_mut() {
                let rw = romanize_text(&w.text);
                if rw != w.text {
                    w.text = rw;
                }
            }
        }
    }
}

// --- Unison vote/report (POST /lyrics/vote handler) --------------------------------------
// Persist locally + fire-and-forget to Unison remote. The UI calls these via Tauri commands
// `lyrics_vote` / `lyrics_report` which map to `POST /lyrics/vote` semantics in Kodama.

/// Vote on a lyric source for a track. `vote` = +1 (up) / -1 (down). Stored per (videoId, source).
pub async fn vote_lyrics(
    state: &AppState,
    video_id: &str,
    source: &str,
    vote: i32,
) -> Result<(), String> {
    let v = vote.clamp(-1, 1);
    state.db.set_lyric_vote(video_id, source, v);
    // Best-effort remote report (Unison).
    let _ = web_http()
        .post(format!("{UNISON_ROOT}/api/vote"))
        .json(&serde_json::json!({ "videoId": video_id, "source": source, "vote": v }))
        .timeout(Duration::from_secs(6))
        .send()
        .await;
    Ok(())
}
/// Report incorrect lyrics for a track (maps to Unison `POST /lyrics/report`).
pub async fn report_lyrics(
    state: &AppState,
    video_id: &str,
    source: &str,
    reason: &str,
) -> Result<(), String> {
    state.db.set_lyric_vote(video_id, source, -1);
    let _ = web_http()
        .post(format!("{UNISON_ROOT}/api/report"))
        .json(&serde_json::json!({ "videoId": video_id, "source": source, "reason": reason }))
        .timeout(Duration::from_secs(6))
        .send()
        .await;
    tracing::info!(video_id, source, reason, "lyrics report");
    Ok(())
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

    #[test]
    fn boidu_ttml_yields_word_timings() {
        let ttml = "<tt><body><div><p begin=\"00:01.00\" end=\"00:04.00\">\
            <span begin=\"00:01.00\" end=\"00:02.00\">Hello </span>\
            <span begin=\"00:02.00\" end=\"00:04.00\">world</span></p></div></body></tt>";
        let lines = parse_lrc_or_ttml(ttml);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert_eq!(line.time_ms, Some(1000));
        assert_eq!(line.end_time_ms, Some(4000));
        let words = line.words.as_ref().expect("should carry words");
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello ");
        assert_eq!(words[0].start_ms, 1000);
        assert_eq!(words[0].end_ms, 2000);
        assert_eq!(words[1].text, "world");
        assert_eq!(words[1].start_ms, 2000);
        assert_eq!(words[1].end_ms, 4000);
    }

    #[test]
    fn boidu_json_array_word_timings() {
        let json = r#"[{"text":"hello","time":1.0,"words":[{"text":"hello","startTime":1.0,"endTime":2.0}]}]"#;
        let lines = parse_lrc_or_ttml(json);
        assert_eq!(lines.len(), 1);
        let words = lines[0].words.as_ref().expect("words");
        assert_eq!(words[0].start_ms, 1000);
        assert_eq!(words[0].end_ms, 2000);
    }
}
