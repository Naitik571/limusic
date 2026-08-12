//! iTunes artwork upgrade (2026-08 Aurora round): the now-playing hero cover at full res.
//!
//! YouTube's own thumbnails top out around 1080 px and are often softer than the master art.
//! iTunes serves its artwork at any size up to 100000×100000 via the `-999` suffix trick
//! (`artworkUrl100` → `100000x100000-999`), so the hero cover gets a genuine full-resolution
//! swap when iTunes has the song. The app's own thumbnail stays the primary source — the iTunes
//! art is a swap-in on top, replaced on load failure, and it only ever decorates the one big
//! cover in Now Playing.
//!
//! Lookup is one search call keyed on artist+title; results are cached in memory (bounded LRU)
//! because this fires on every track change and never needs to survive a restart. A wrong iTunes
//! match degrades to the same art the player bar already shows — harmless either way.

use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

const CACHE_CAP: usize = 200;

/// `(normalized key, artwork URL or None-for-certain-not-found)`, oldest first.
static CACHE: LazyLock<Mutex<VecDeque<(String, Option<String>)>>> =
    LazyLock::new(|| Mutex::new(VecDeque::new()));

fn http() -> &'static reqwest::Client {
    static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(concat!("Limusic v", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("build itunes http client")
    });
    &HTTP
}

/// Full-resolution iTunes artwork for an artist+title, if it's in the cache or the store.
/// `Ok(None)` = definitive miss (cached or search returned nothing) — the caller keeps the
/// regular thumbnail.
pub async fn lookup(artist: &str, title: &str) -> Result<Option<String>, String> {
    let key = format!("{}|{}", artist.to_lowercase(), title.to_lowercase());
    if let Some(hit) = cache_get(&key) {
        return Ok(hit);
    }

    let term = format!("{artist} {title}");
    let resp: serde_json::Value = http()
        .get("https://itunes.apple.com/search")
        .query(&[("term", term.as_str()), ("entity", "song"), ("limit", "1")])
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let art = resp
        .pointer("/results/0/artworkUrl100")
        .and_then(|v| v.as_str())
        .map(|u| u.replace("100x100bb", "100000x100000-999"))
        // Some artwork URLs use a different size token; normalize anything we didn't match.
        .map(|u| {
            if u.contains("100000x100000") { u } else { u }
        });

    cache_put(key, art.clone());
    Ok(art)
}

fn cache_get(key: &str) -> Option<Option<String>> {
    let mut cache = CACHE.lock().unwrap();
    let pos = cache.iter().position(|(k, _)| k == key)?;
    let entry = cache.remove(pos).unwrap();
    cache.push_back(entry.clone()); // most-recently-used to the back
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
