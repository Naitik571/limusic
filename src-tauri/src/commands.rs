//! Tauri commands — the ONLY API the UI calls. context/11 UI contract. No YouTube shapes leak
//! past here; the UI never sees a stream URL.

use std::sync::Arc;

use innertube::{
    AlbumPage, ArtistPage, BrowseItem, HomePage, PlaylistContinuation, PlaylistPage, SearchResults,
    SongItem,
};
use tauri::{Emitter, Manager, State};

use crate::state::{AppState, ON_REPEAT_ID, ON_REPEAT_LIMIT, ON_REPEAT_WINDOW_SECS};

type St<'a> = State<'a, Arc<AppState>>;

#[tauri::command]
pub async fn search(state: St<'_>, query: String) -> Result<Vec<SongItem>, String> {
    let client = state
        .clients
        .get(innertube::METADATA_CLIENT)
        .ok_or("metadata client missing")?;
    let result = state
        .it
        .search_songs(client, &query)
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.items)
}

/// Unfiltered search → categorized sections for the search page.
#[tauri::command]
pub async fn search_all(state: St<'_>, query: String) -> Result<SearchResults, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .search_all(client, &query)
        .await
        .map_err(|e| e.to_string())
}

/// Filtered "Show more" search for one category (albums / artists / playlists).
#[tauri::command]
pub async fn search_cards(
    state: St<'_>,
    query: String,
    category: String,
) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .search_cards(client, &query, &category)
        .await
        .map_err(|e| e.to_string())
}

/// Play a track (from a search result). The UI passes the full item so we can seed the queue
/// with its metadata without another round-trip.
#[tauri::command]
pub async fn play(state: St<'_>, item: SongItem) -> Result<(), String> {
    let state = state.inner().clone();
    state.play_song(item).await;
    Ok(())
}

#[tauri::command]
pub async fn play_index(state: St<'_>, index: usize) -> Result<(), String> {
    let state = state.inner().clone();
    state.play_index(index).await;
    Ok(())
}

/// Remove an upcoming track from the queue (not the one playing). Guests are add-only — blocked
/// inside AppState.
#[tauri::command]
pub async fn remove_from_queue(state: St<'_>, index: usize) -> Result<(), String> {
    state.inner().clone().remove_from_queue(index).await;
    Ok(())
}

/// Reorder an upcoming queue slot (drag-to-reorder in the queue panel). Both indices are
/// absolute queue positions, strictly after the playing track.
#[tauri::command]
pub async fn move_queue_item(state: St<'_>, from: usize, to: usize) -> Result<(), String> {
    state.inner().clone().move_from_queue(from, to).await;
    Ok(())
}

/// "Play next" from a ⋯ menu: one track or a whole album/playlist, inserted right after the
/// current song (behind any earlier manual adds). `from` is the album/playlist title, which heads
/// the block in the queue panel.
#[tauri::command]
pub async fn play_next(
    state: St<'_>,
    items: Vec<SongItem>,
    from: Option<String>,
) -> Result<(), String> {
    state.inner().clone().play_next(items, from).await;
    Ok(())
}

/// "Add to queue": the tracks go after everything the user picked, and ahead of anything the app
/// generated behind it (autoplay filler, or a radio's endless feed). `from` heads the block in the
/// queue panel; `continuation` is the source page's next-page token — the rest of a long playlist
/// is walked in in the background.
#[tauri::command]
pub async fn add_to_queue(
    state: St<'_>,
    items: Vec<SongItem>,
    from: Option<String>,
    continuation: Option<String>,
) -> Result<(), String> {
    state
        .inner()
        .clone()
        .add_to_queue(items, from, continuation)
        .await;
    Ok(())
}

/// Clear every upcoming manually-queued track (the queue panel's "Next in queue" section).
#[tauri::command]
pub async fn clear_queued(state: St<'_>) -> Result<(), String> {
    state.inner().clone().clear_queued().await;
    Ok(())
}

/// Drop every played track, keeping the one playing (queue panel's "Clear played").
#[tauri::command]
pub async fn clear_played(state: St<'_>) -> Result<(), String> {
    state.inner().clone().clear_played().await;
    Ok(())
}

#[tauri::command]
pub async fn next_track(state: St<'_>) -> Result<(), String> {
    state.inner().clone().next_in_queue().await;
    Ok(())
}

#[tauri::command]
pub async fn prev_track(state: St<'_>) -> Result<(), String> {
    state.inner().clone().prev_in_queue().await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_shuffle(state: St<'_>) -> Result<(), String> {
    state.inner().clone().toggle_shuffle().await;
    Ok(())
}

/// `mode` ∈ "off" | "all" | "one".
#[tauri::command]
pub async fn set_repeat(state: St<'_>, mode: String) -> Result<(), String> {
    let mode = match mode.as_str() {
        "off" => crate::state::RepeatMode::Off,
        "all" => crate::state::RepeatMode::All,
        "one" => crate::state::RepeatMode::One,
        other => return Err(format!("unknown repeat mode: {other}")),
    };
    state.inner().clone().set_repeat(mode).await;
    Ok(())
}

#[tauri::command]
pub async fn toggle_pause(state: St<'_>) -> Result<(), String> {
    let state = state.inner().clone();
    state.resume_or_toggle().await;
    Ok(())
}

/// Theater mode's fullscreen toggle (#139).
///
/// `setFullscreen` on its own is not enough on Windows. tao decides the client area in
/// WM_NCCALCSIZE: while the real Win32 placement says maximized it clamps the client to the
/// monitor's *work* area, so the "fullscreen" window sits under the taskbar with a frame-thick
/// border around it, and while the window is undecorated-with-shadow it insets the client by the
/// frame thickness. Both are decided before the fullscreen flag is, and tao's own `is_maximized`
/// reads a cached flag that can disagree with the placement, which is why unmaximizing from the
/// UI only fixed it some of the time.
///
/// So: restore from the real placement, go fullscreen, then put the window on the monitor rect
/// with SWP_FRAMECHANGED to force one recalculation with the fullscreen flag set. On the main
/// thread, where the window messages run inline and the order is guaranteed.
#[tauri::command]
pub fn theater_fullscreen(window: tauri::WebviewWindow, on: bool) -> Result<(), String> {
    #[cfg(not(target_os = "windows"))]
    {
        window.set_fullscreen(on).map_err(|e| e.to_string())
    }
    #[cfg(target_os = "windows")]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        use windows::Win32::UI::WindowsAndMessaging::{
            IsZoomed, SetWindowPos, ShowWindow, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE,
            SWP_NOZORDER, SW_MAXIMIZE, SW_RESTORE,
        };

        /// Restoring the window is what makes fullscreen work, so theater has to put the
        /// maximized state back itself on the way out.
        static WAS_MAXIMIZED: AtomicBool = AtomicBool::new(false);

        let w = window.clone();
        window
            .run_on_main_thread(move || {
                let Ok(hwnd) = w.hwnd() else { return };
                if on {
                    let zoomed = unsafe { IsZoomed(hwnd).as_bool() };
                    WAS_MAXIMIZED.store(zoomed, Ordering::Relaxed);
                    if zoomed {
                        unsafe {
                            let _ = ShowWindow(hwnd, SW_RESTORE);
                        }
                    }
                    let _ = w.set_fullscreen(true);
                    if let Ok(Some(m)) = w.current_monitor() {
                        let (p, s) = (m.position(), m.size());
                        unsafe {
                            let _ = SetWindowPos(
                                hwnd,
                                None,
                                p.x,
                                p.y,
                                s.width as i32,
                                s.height as i32,
                                SWP_FRAMECHANGED | SWP_NOZORDER,
                            );
                        }
                    }
                } else {
                    let _ = w.set_fullscreen(false);
                    if WAS_MAXIMIZED.swap(false, Ordering::Relaxed) {
                        unsafe {
                            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
                        }
                    }
                    unsafe {
                        let _ = SetWindowPos(
                            hwnd,
                            None,
                            0,
                            0,
                            0,
                            0,
                            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
                        );
                    }
                }
            })
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
pub async fn toggle_mute(state: St<'_>) -> Result<(), String> {
    let state = state.inner().clone();
    let new_vol = if state.player.get_volume() == 0 {
        state
            .db
            .get_setting("volume")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(100)
    } else {
        0
    };
    state
        .player
        .set_volume(new_vol)
        .map_err(|e| e.to_string())?;
    let _ = state.app.emit("volume", new_vol);
    state.db.set_setting("volume", &new_vol.to_string());
    Ok(())
}

#[tauri::command]
pub async fn seek(state: St<'_>, position: f64) -> Result<(), String> {
    // Routed through AppState so a Listen Together host broadcasts the seek and a guest is blocked.
    state.user_seek(position).await
}

#[tauri::command]
pub async fn set_volume(state: St<'_>, volume: i64) -> Result<(), String> {
    state.player.set_volume(volume).map_err(|e| e.to_string())?;
    // There is one volume and there can be two windows (the mini player). Without this the one
    // that didn't move the slider keeps showing the old level and lies about what you're hearing.
    let _ = state.app.emit("volume", volume);
    state.db.set_setting("volume", &volume.to_string());
    Ok(())
}

#[tauri::command]
pub fn get_volume(state: St<'_>) -> i64 {
    state
        .db
        .get_setting("volume")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(state.player.get_volume())
}

/// Arm or disarm the sleep timer. `mode` is `"off"`, `"end_of_song"`, or `"<minutes>"` (1–1440).
/// Enforced in Rust (see `spawn_sleep_timer`), so it keeps counting even with the window closed.
#[tauri::command]
pub fn set_sleep_timer(state: St<'_>, mode: String) -> Result<(), String> {
    let timer = crate::state::parse_sleep_mode(&mode)?;
    *state.sleep_timer.lock().unwrap() = timer;
    Ok(())
}

/// Current sleep timer for window restore: `"off"`, `"end_of_song"`, or remaining seconds.
#[tauri::command]
pub fn get_sleep_timer(state: St<'_>) -> String {
    match *state.sleep_timer.lock().unwrap() {
        crate::state::SleepTimer::Off => "off".to_string(),
        crate::state::SleepTimer::EndOfSong => "end_of_song".to_string(),
        crate::state::SleepTimer::At(end) => end
            .saturating_duration_since(std::time::Instant::now())
            .as_secs()
            .to_string(),
    }
}

#[tauri::command]
pub async fn get_queue(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.queue_snapshot().await)
}

/// Settings the UI is allowed to read *and write*. Session/auth material (`session_cookie`,
/// `data_sync_id`, `account_json`, `visitor_data`) and internal blobs (`queue_json`,
/// `queue_position`) never cross into the webview — they'd otherwise ship the login credential to
/// the renderer on every open — and the webview can't overwrite them either.
const UI_SETTINGS: [&str; 22] = [
    "proxy",
    "quality",
    "enable_history",
    "disabled_stream_clients",
    "discord_rpc",
    "close_to_tray",
    "autostart",
    "autoplay",
    "auto_offline",
    "hide_videos",
    "prevent_duplicates",
    "ytdlp_enabled",
    "download_dir",
    "download_quality",
    "download_format",
    "use_offline",
    // Playlists pinned for offline (playlist page toggle). Plain ids, not secrets.
    "offline_playlists",
    // Apple Music bring-your-own-token lyrics (Settings → General). Not session-secret — the
    // user pasted them into this very UI — but they are credentials, so they live in the settings
    // DB and round-trip through this same allowlist. `lyrics_boidu` toggles the Boidu provider.
    "lyrics_apple_media_token",
    "lyrics_apple_dev_token",
    "lyrics_apple_storefront",
    "lyrics_boidu",
    // Keep shuffle on across queue changes (albums/playlists/radio).
    "sticky_shuffle",
    // Custom app icon: an absolute file path on this machine, not a secret.
];

#[tauri::command]
pub async fn get_settings(state: St<'_>) -> Result<serde_json::Value, String> {
    let map: serde_json::Map<String, serde_json::Value> = state
        .db
        .all_settings()
        .into_iter()
        .filter(|(k, _)| UI_SETTINGS.contains(&k.as_str()))
        .map(|(k, v)| (k, serde_json::Value::String(v)))
        .collect();
    Ok(serde_json::Value::Object(map))
}

#[tauri::command]
pub async fn set_setting(
    app: tauri::AppHandle,
    state: St<'_>,
    key: String,
    value: String,
) -> Result<(), String> {
    if !UI_SETTINGS.contains(&key.as_str()) {
        return Err(format!("unknown setting: {key}"));
    }
    state.db.set_setting(&key, &value);
    // Presence connects/clears the moment it's toggled — the user shouldn't have to skip a track
    // to see it take effect.
    if key == "discord_rpc" {
        state.set_discord_enabled(value == "true");
    }
    if key == "ytdlp_enabled" {
        state.ytdlp.set_enabled(value == "true");
    }
    // Applies to what's fetched from here on: the live queue keeps whatever is already in it.
    if key == "hide_videos" {
        state.it.set_hide_videos(value == "true");
    }
    // Registers/removes the login autostart entry on toggle; the OS persists it from there.
    // ponytail: no startup re-sync against the OS state — add reconciliation only if drift is
    // ever reported.
    if key == "autostart" {
        use tauri_plugin_autostart::ManagerExt;
        let al = app.autolaunch();
        let res = if value == "true" {
            al.enable()
        } else if al.is_enabled().unwrap_or(false) {
            al.disable()
        } else {
            Ok(())
        };
        res.map_err(|e| format!("autostart: {e}"))?;
    }
    // Let every window react to a setting flip immediately (separate webviews like the mini/floating
    // players can't share Svelte state, so they listen for this).
    let _ = app.emit(
        "setting-changed",
        serde_json::json!({ "key": key, "value": value }),
    );
    Ok(())
}

/// The signed-in user's Liked Music video ids, newest first. Bounded walk (~30 pages ≈ 3k
/// tracks) — this feeds the heart on every row: search/playlist rows don't carry `likeStatus`,
/// so the UI checks membership here instead of trusting the row. A launch-time + on-demand
/// snapshot; likes made in-app update the frontend set locally.
#[tauri::command]
pub async fn get_liked_ids(state: St<'_>) -> Result<Vec<String>, String> {
    let client = metadata_client(&state)?;
    let page = state
        .it
        .playlist(client, "VLLM")
        .await
        .map_err(|e| e.to_string())?;
    let mut ids: Vec<String> = page.items.iter().map(|i| i.video_id.clone()).collect();
    let mut token = page.continuation;
    let mut pages = 0usize;
    while let Some(t) = token {
        if pages >= 30 {
            break;
        }
        pages += 1;
        let more = state
            .it
            .playlist_continuation(client, &t)
            .await
            .map_err(|e| e.to_string())?;
        ids.extend(more.items.iter().map(|i| i.video_id.clone()));
        token = more.continuation;
    }
    Ok(ids)
}

/// Status of the yt-dlp fallback for the settings screen: whether the toggle is on, whether
/// the binary is installed, and the last download/update error (if any). The UI can also kick
/// the install early, so a toggled-on user who's about to hit a restricted track can warm it.
#[tauri::command]
pub async fn ytdlp_info(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "enabled": state.ytdlp.enabled(),
        "installed": state.ytdlp.installed(),
        "last_error": state.ytdlp.last_error(),
    }))
}

/// Force-install/update yt-dlp now (settings-screen button), so the fallback is warm.
#[tauri::command]
pub async fn ytdlp_install_now(state: St<'_>) -> Result<(), String> {
    state.ytdlp.ensure_ready().await
}

/// Full-resolution iTunes artwork for the now-playing hero cover (Aurora round). Returns None
/// when iTunes has nothing sane — the UI keeps the regular thumbnail.
#[tauri::command]
pub async fn get_highres_art(artist: String, title: String) -> Result<Option<String>, String> {
    crate::art::lookup(&artist, &title).await
}

/// Spotify Canvas: short looping video for Now Playing (#8). Uses SimpMusic API stub;
/// returns None when no canvas exists — UI falls back to blurred artwork / palette gradient.
#[tauri::command]
pub async fn get_canvas(artist: String, title: String) -> Result<Option<String>, String> {
    crate::canvas::lookup(&artist, &title).await
}

/// The streamable client keys the orchestrator tries, for the "disabled clients" setting. Names
/// come from the innertube crate so the UI stays free of YouTube-shaped identity strings.
#[tauri::command]
pub async fn get_stream_clients() -> Result<Vec<String>, String> {
    let mut v = vec![innertube::MAIN_CLIENT.to_string()];
    v.extend(
        innertube::STREAM_FALLBACK_ORDER
            .iter()
            .map(|s| s.to_string()),
    );
    Ok(v)
}

/// Let the webview fetch one font file the user picked in the Themes tab, so a `@font-face` can
/// point at it.
///
/// Same runtime-scope trick as local artwork (`local::allow_covers`): the static asset scope stays
/// empty, and only the exact file gets a URL. The extension check keeps the command from being a
/// general "give the page a URL for any path on this machine" — today only the main window holds a
/// capability to call commands at all, and this stays safe if that ever widens.
#[tauri::command]
pub async fn allow_font_file(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri::Manager;
    const FONT_EXTS: [&str; 4] = ["ttf", "otf", "woff", "woff2"];
    let p = std::path::Path::new(&path);
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !FONT_EXTS.contains(&ext.as_str()) {
        return Err(format!("not a font file: {path}"));
    }
    // Scope grants succeed for paths that don't exist, so check here: this failing is how the UI
    // learns a loaded font was deleted or moved, and drops it instead of listing a dead entry.
    if !p.is_file() {
        return Err(format!("font file not found: {path}"));
    }
    let scope = app.asset_protocol_scope();
    scope.allow_file(&path).map_err(|e| e.to_string())?;
    // The scope check canonicalizes what it is asked about, so a font reached through a symlinked
    // folder needs the real path allowed too (see local::allow_covers).
    if let Ok(real) = p.canonicalize() {
        let _ = scope.allow_file(real);
    }
    Ok(())
}

/// Wipe both cache tiers (URL cache + mpv on-disk audio cache). context/14.
#[tauri::command]
pub async fn clear_caches(state: St<'_>) -> Result<(), String> {
    state.clear_caches();
    Ok(())
}

// --- auth (context/15) ---------------------------------------------------------------------

#[tauri::command]
pub async fn get_account(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.account_snapshot())
}

#[tauri::command]
pub async fn get_account_identities(state: St<'_>) -> Result<Vec<serde_json::Value>, String> {
    state.account_identities().await
}

#[tauri::command]
pub async fn switch_account(
    state: St<'_>,
    selection_key: String,
) -> Result<serde_json::Value, String> {
    state.switch_account(&selection_key).await
}

#[tauri::command]
pub async fn sign_out(state: St<'_>) -> Result<(), String> {
    let state = state.inner().clone();
    state.sign_out().await;
    Ok(())
}

/// Open the in-app Google sign-in webview (context/15 Path A). Completes asynchronously; the UI
/// hears back via `auth-changed` (success) or `login-error`.
#[tauri::command]
pub async fn login_webview(state: St<'_>) -> Result<(), String> {
    let state = state.inner().clone();
    let app = state.app.clone();
    crate::session::open_login(app, state);
    Ok(())
}

/// The current track, play state, position and duration in one shot. Events are the normal
/// channel; this is for a webview that started after them (the mini player, or the main window
/// on a cold start, where the queue is restored before the UI subscribes).
#[tauri::command]
pub async fn get_playback(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.playback_snapshot().await)
}

// --- mini player (mini.rs) ------------------------------------------------------------------

/// Swap the app for the floating widget: the main window hides to the tray behind it.
#[tauri::command]
pub async fn open_mini(app: tauri::AppHandle) -> Result<(), String> {
    // GTK wants window creation on the main thread, so hop and post the result back rather than
    // logging a failure the user would only see as a click that did nothing.
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = tx.send(crate::mini::open(&handle));
    })
    .map_err(|e| e.to_string())?;
    rx.await
        .map_err(|_| "the mini player never answered".to_string())?
}

/// Swap back. Same path as the tray, so the widget and the tray can't disagree about what
/// "show Limusic" means.
#[tauri::command]
pub async fn close_mini(app: tauri::AppHandle) -> Result<(), String> {
    crate::tray::show_main(&app);
    Ok(())
}

// --- browse / library (context/08) ---------------------------------------------------------

fn metadata_client(state: &Arc<AppState>) -> Result<&innertube::YouTubeClient, String> {
    state
        .clients
        .get(innertube::METADATA_CLIENT)
        .ok_or_else(|| "metadata client missing".into())
}

#[tauri::command]
pub async fn get_home(state: St<'_>, params: Option<String>) -> Result<HomePage, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .home(client, params.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_home_more(state: St<'_>, token: String) -> Result<HomePage, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .home_continuation(client, &token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library(state: St<'_>) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(&state)?;
    let mut items = state
        .it
        .library_playlists(client)
        .await
        .map_err(|e| e.to_string())?;
    // On Repeat leads the library once there's anything in it. Hidden while empty rather than
    // shown as a dead tile on a fresh install.
    let songs = on_repeat_songs(&state);
    if !songs.is_empty() {
        items.insert(
            0,
            BrowseItem {
                kind: "playlist",
                id: ON_REPEAT_ID.into(),
                title: "On Repeat".into(),
                subtitle: Some(format!("{} songs", songs.len())),
                thumbnail: None, // the UI draws an icon cover for this one
                duration: None,
                artist_runs: Vec::new(),
                is_video: false,
            },
        );
    }
    // A card has nowhere to put two images, so a custom cover simply is the artwork here.
    for item in &mut items {
        if let Some(cover) = custom_cover(&state, &item.id) {
            item.thumbnail = Some(cover);
        }
    }
    Ok(items)
}

#[tauri::command]
pub async fn get_library_albums(state: St<'_>) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .library_albums(client)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library_artists(state: St<'_>) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .library_artists(client)
        .await
        .map_err(|e| e.to_string())
}

/// videoId → how many times it was played, over the same trailing window On Repeat is built from
/// (the history table is pruned to it, so there is no older data to offer). Feeds the playlist
/// page's "Most played" sort; a track the map doesn't mention has not been played this month.
#[tauri::command]
pub fn play_counts(state: St<'_>) -> std::collections::HashMap<String, i64> {
    state
        .db
        .play_counts(now_secs() - ON_REPEAT_WINDOW_SECS)
        .into_iter()
        .collect()
}

/// A playlist or album page. `id` is the browseId (`VL…` / `MPRE…`); Liked Songs is `VLLM`, and
/// `LIMUSIC_ON_REPEAT` is the local auto-playlist rather than anything YouTube knows about.
#[tauri::command]
pub async fn get_playlist(state: St<'_>, id: String) -> Result<PlaylistPage, String> {
    if id == ON_REPEAT_ID {
        let items = on_repeat_songs(&state);
        return Ok(PlaylistPage {
            title: Some("On Repeat".into()),
            subtitle: Some(format!(
                "{} songs you've played most this month",
                items.len()
            )),
            thumbnail: None,
            description: None,
            privacy: None,
            cover: None,
            items,
            continuation: None,
            owned: false, // nothing to rename or delete; it rebuilds itself from what you play
        });
    }
    let client = metadata_client(&state)?;
    let mut page = state
        .it
        .playlist(client, &id)
        .await
        .map_err(|e| e.to_string())?;
    // Alongside YouTube's own thumbnail, not over it: the dialog offers to drop the custom one.
    page.cover = custom_cover(&state, &id);
    Ok(page)
}

/// The On Repeat track list: most-played first, over the trailing window. Rows whose stored JSON
/// no longer parses (a `SongItem` shape change) are dropped rather than failing the whole page.
fn on_repeat_songs(state: &Arc<AppState>) -> Vec<SongItem> {
    let since = now_secs() - ON_REPEAT_WINDOW_SECS;
    state
        .db
        .top_plays(since, ON_REPEAT_LIMIT)
        .into_iter()
        .filter_map(|(json, _plays)| serde_json::from_str(&json).ok())
        .map(shed_queue_context)
        .collect()
}

/// A play record is the whole `SongItem` as it sat in the queue, so it carries that slot's queue
/// metadata: `queued`/`queued_by` when the track was "added to queue" (in a Listen Together session,
/// stamped with who added it), `autoplay` when radio appended it, `set_video_id` from whatever
/// playlist it was played from. None of that describes the song, so On Repeat sheds it: otherwise
/// the row wears a session member's name forever, and playing On Repeat drops it into "Next in
/// queue" instead of the playlist. Strips on read so rows already stored this way are fixed too.
fn shed_queue_context(s: SongItem) -> SongItem {
    SongItem {
        queued: false,
        queued_end: false,
        queued_from: None,
        queued_by: None,
        autoplay: false,
        set_video_id: None,
        ..s
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[tauri::command]
pub async fn get_playlist_more(
    state: St<'_>,
    token: String,
) -> Result<PlaylistContinuation, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .playlist_continuation(client, &token)
        .await
        .map_err(|e| e.to_string())
}

/// An album page. `id` is the album browseId (`MPRE…`).
#[tauri::command]
pub async fn get_album(state: St<'_>, id: String) -> Result<AlbumPage, String> {
    // A local album is built from SQLite, so it opens the same page while offline (local.rs).
    if let Some(key) = id.strip_prefix(crate::local::ALBUM_PREFIX) {
        return Ok(crate::local::album_page(&state.db, key));
    }
    // A local artist rides this route too: same page shape, and none of the artist route's
    // YouTube furniture applies to files on disk (see `local::artist_page`).
    if let Some(name) = id.strip_prefix(crate::local::ARTIST_PREFIX) {
        return Ok(crate::local::artist_page(&state.db, name));
    }
    let client = metadata_client(&state)?;
    state.it.album(client, &id).await.map_err(|e| e.to_string())
}

/// An artist page. `id` is the channel browseId (`UC…`).
#[tauri::command]
pub async fn get_artist(state: St<'_>, id: String) -> Result<ArtistPage, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .artist(client, &id)
        .await
        .map_err(|e| e.to_string())
}

/// A card grid reached from a carousel's "More" button (e.g. an artist's full albums list).
#[tauri::command]
pub async fn get_browse_grid(
    state: St<'_>,
    id: String,
    params: Option<String>,
) -> Result<Vec<BrowseItem>, String> {
    let client = metadata_client(&state)?;
    state
        .it
        .browse_grid(client, &id, params.as_deref())
        .await
        .map_err(|e| e.to_string())
}

/// Play a playlist/album: the given items become the queue (no radio). `start` is the clicked
/// track index; `None`/omitted means "just play it" (random opener when shuffle is on).
/// `source_id` (the page's playlist/album playlist id) makes autoplay continue with that
/// context's radio when the queue runs out. `source_name` (the page title) feeds the queue
/// panel's "Next from" header; `shuffle: true` (page Shuffle buttons) turns shuffle on for
/// this queue — pass the items in their real order, the backend shuffles. `continuation` is the
/// page's next-page token when it has one: pass the tracks that are loaded and the backend walks
/// the rest into the queue in the background, so playback starts on page 1.
#[tauri::command]
pub async fn play_playlist(
    state: St<'_>,
    items: Vec<SongItem>,
    start: Option<usize>,
    source_id: Option<String>,
    source_name: Option<String>,
    shuffle: Option<bool>,
    continuation: Option<String>,
) -> Result<(), String> {
    let state = state.inner().clone();
    state
        .play_tracks(
            items,
            start,
            source_id,
            source_name,
            shuffle.unwrap_or(false),
            continuation,
        )
        .await;
    Ok(())
}

/// Start a radio seeded on a song, artist, album or playlist (context/08). `kind` is
/// `song` | `artist` | `album` | `playlist`; `id` is the videoId (song) or browseId/playlistId
/// (everything else) — the backend resolves it to a radio playlist. `name` titles the queue.
///
/// Starting a song radio on the track that's already playing keeps it playing and replaces only
/// what comes after it; every other case replaces the queue.
#[tauri::command]
pub async fn start_radio(
    state: St<'_>,
    kind: String,
    id: String,
    name: Option<String>,
) -> Result<(), String> {
    let state = state.inner().clone();
    state.start_radio(&kind, &id, name).await
}

/// Similar songs to a track — the same radio endpoint that powers autoplay (context/08),
/// fetched read-only so a playlist page can show a "More like this" shelf. The radio playlist
/// is seeded directly (`RDAMVM<videoId>`): a bare next(videoId) returns only the seed plus an
/// automix preview. The seed itself is dropped; up to `limit` (default 8) tracks come back.
#[tauri::command]
pub async fn get_similar_songs(
    state: St<'_>,
    video_id: String,
    limit: Option<usize>,
) -> Result<Vec<SongItem>, String> {
    let client = metadata_client(&state)?;
    let radio_id = format!("RDAMVM{video_id}");
    let next = state
        .it
        .next(client, Some(&video_id), Some(&radio_id))
        .await
        .map_err(|e| e.to_string())?;
    Ok(next
        .items
        .into_iter()
        .filter(|i| i.video_id != video_id)
        .take(limit.unwrap_or(8))
        .collect())
}

// --- write actions (context/01 ✎, context/15) ----------------------------------------------

fn require_login(state: &Arc<AppState>) -> Result<&innertube::YouTubeClient, String> {
    if !state.it.is_logged_in() {
        return Err("Sign in first to use this.".into());
    }
    metadata_client(state)
}

#[tauri::command]
pub async fn like(state: St<'_>, video_id: String, liked: bool) -> Result<(), String> {
    let client = require_login(&state)?;
    state
        .it
        .like(client, &video_id, liked)
        .await
        .map_err(|e| e.to_string())
}

/// Like, dislike, or clear a track's rating. One command for all three: YouTube's states are
/// mutually exclusive, so a dislike un-likes in the same call and the UI never has to send two.
#[tauri::command]
pub async fn rate(
    state: St<'_>,
    video_id: String,
    rating: innertube::Rating,
) -> Result<(), String> {
    let client = require_login(&state)?;
    state
        .it
        .rate(client, &video_id, rating)
        .await
        .map_err(|e| e.to_string())
}

/// Save an album to the library, or remove it. `playlist_id` is the album's `OLAK5uy_…`
/// (`AlbumPage.playlistId`).
#[tauri::command]
pub async fn set_album_saved(
    state: St<'_>,
    playlist_id: String,
    saved: bool,
) -> Result<(), String> {
    let client = require_login(&state)?;
    state
        .it
        .like_playlist(client, &playlist_id, saved)
        .await
        .map_err(|e| e.to_string())
}

/// Login, plus the guard every playlist edit needs: On Repeat has no YouTube playlist behind it, so
/// its synthetic id must never reach `edit_playlist`, which answers 400 for an id it doesn't know.
fn editable_playlist<'a>(
    state: &'a Arc<AppState>,
    playlist_id: &str,
) -> Result<&'a innertube::YouTubeClient, String> {
    if playlist_id == ON_REPEAT_ID {
        return Err("On Repeat builds itself from what you play.".into());
    }
    require_login(state)
}

#[tauri::command]
pub async fn add_to_playlist(
    state: St<'_>,
    playlist_id: String,
    video_id: String,
) -> Result<bool, String> {
    let client = editable_playlist(&state, &playlist_id)?;
    state
        .it
        .playlist_add(client, &playlist_id, &video_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_from_playlist(
    state: St<'_>,
    playlist_id: String,
    video_id: String,
    set_video_id: String,
) -> Result<(), String> {
    let client = editable_playlist(&state, &playlist_id)?;
    state
        .it
        .playlist_remove(client, &playlist_id, &video_id, &set_video_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_playlist(state: St<'_>, title: String) -> Result<String, String> {
    let client = require_login(&state)?;
    state
        .it
        .create_playlist(client, &title)
        .await
        .map_err(|e| e.to_string())
}

/// Edit a playlist you own, from the "Edit playlist" dialog: name, description, visibility.
///
/// Each field is `None` when the user left it alone, and only what changed is sent: an edit of
/// the name must not blank a description we failed to read back off the page.
#[tauri::command]
pub async fn edit_playlist_details(
    state: St<'_>,
    playlist_id: String,
    name: Option<String>,
    description: Option<String>,
    public: Option<bool>,
) -> Result<(), String> {
    let client = editable_playlist(&state, &playlist_id)?;
    // The switch is two-state; YouTube's third value (UNLISTED) is only ever left as it was.
    let privacy = public.map(|p| if p { "PUBLIC" } else { "PRIVATE" });
    state
        .it
        .playlist_edit_details(
            client,
            &playlist_id,
            name.as_deref(),
            description.as_deref(),
            privacy,
        )
        .await
        .map_err(|e| e.to_string())
}

/// Custom playlist artwork, in both places it lives.
///
/// Setting one is local-first: the picked image is copied in beside the local-music covers and
/// answered straight back, then pushed to YouTube Music in the background (`sync_cover`), because
/// the upload is three round trips and nobody should watch a spinner for their own file.
///
/// Dropping one waits, and that is deliberate. Once a cover has been up there, YouTube's own
/// thumbnail *is* that cover, so a local-first removal would fall back to the very image being
/// removed and only reach the rebuilt collage a beat later: two swaps, the first of them pointless.
/// The clear is a single small call, so it answers with the thumbnail YouTube rebuilt and the UI
/// changes once.
#[tauri::command]
pub async fn set_playlist_cover(
    app: tauri::AppHandle,
    state: St<'_>,
    playlist_id: String,
    path: Option<String>,
) -> Result<CoverResult, String> {
    use tauri::Manager;
    // What YouTube's uploader will take. WebP is not on the list: it answers 415 for one, and a
    // cover that only works on this machine is worse than one the picker never offered.
    const IMAGE_EXTS: [&str; 3] = ["jpg", "jpeg", "png"];

    let key = cover_key(&playlist_id);
    let stored = state.db.get_setting(&key);
    let Some(src) = path else {
        // YouTube first, so the local copy is still on screen while it answers. Its refusal is
        // never fatal though: dropping the cover from this machine is what the user clicked, and
        // an account that was not allowed to set one up there has nothing to clear anyway.
        let thumbnail = match clear_cover_on_youtube(&state, &playlist_id).await {
            Ok(t) => {
                state.db.delete_setting(&synced_key(&playlist_id));
                t
            }
            Err(e) => {
                tracing::warn!(playlist_id, error = %e, "custom cover not cleared on YouTube Music");
                // Only worth saying when a cover of ours actually reached the account: otherwise
                // there was nothing up there to keep, and the warning would be a lie.
                if state.db.get_setting(&synced_key(&playlist_id)).is_some() {
                    let _ = state.app.emit(
                        "cover-error",
                        serde_json::json!({
                            "message": "Removed here, but YouTube Music kept its copy.",
                        }),
                    );
                }
                None
            }
        };
        state.db.delete_setting(&key);
        if let Some(old) = stored {
            let _ = std::fs::remove_file(old);
        }
        return Ok(CoverResult {
            cover: None,
            thumbnail,
        });
    };
    let src = std::path::Path::new(&src);
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !IMAGE_EXTS.contains(&ext.as_str()) {
        return Err("Pick a JPEG or PNG image: YouTube Music won't take anything else.".into());
    }
    // ponytail: a flat size cap instead of downscaling. It keeps a 40px sidebar thumb from
    // decoding a camera raw in the webview and the upload from swallowing one; reach for the
    // `image` crate and a real resize only if 8 MB turns out to bother anyone.
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
    if src.metadata().map(|m| m.len()).unwrap_or(0) > MAX_BYTES {
        return Err("That image is over 8 MB. Pick a smaller one.".into());
    }
    let dir = crate::local::covers_dir(&app).join("playlists");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Timestamped, so replacing a cover can't be served out of the webview's cache under the name
    // it already has. The id is filtered to filename characters rather than trusted: it arrives
    // from the UI, and a `..` in it would write outside this directory.
    let stem: String = playlist_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    let dest = dir.join(format!("{stem}-{}.{ext}", crate::db::now_secs()));
    std::fs::copy(src, &dest).map_err(|e| e.to_string())?;
    // Only now is the cover it replaces safe to unlink. Dropping it any earlier means a picked
    // file this command goes on to refuse (wrong format, too big, unreadable) takes the artwork
    // already on screen down with it, and the toast talks about the new file while the old one is
    // the thing that just disappeared.
    if let Some(old) = stored {
        let _ = std::fs::remove_file(old);
    }
    let dest = dest.to_string_lossy().to_string();
    // The covers directory is allowed recursively at startup, but the first cover on a fresh
    // install is written after that ran, so name this file explicitly too.
    let _ = app.asset_protocol_scope().allow_file(&dest);
    state.db.set_setting(&key, &dest);
    sync_cover(&state, &playlist_id, dest.clone());
    Ok(CoverResult {
        cover: Some(dest),
        thumbnail: None,
    })
}

/// What the UI needs to draw after a cover changed: where the local copy is, and (on a removal)
/// the thumbnail YouTube rebuilt in its place.
#[derive(serde::Serialize)]
pub struct CoverResult {
    cover: Option<String>,
    thumbnail: Option<String>,
}

/// Send the cover on to YouTube Music behind the picker's back: the local copy is already on
/// screen, and the upload is a three-call round trip nobody should wait through.
///
/// A failure is a toast, not a rollback: the artwork is still right here, and it is still this
/// playlist's cover on this machine. Signed out (or On Repeat, which YouTube has never heard of),
/// there is nothing to sync and local is all there ever was.
fn sync_cover(state: &Arc<AppState>, playlist_id: &str, path: String) {
    if playlist_id == ON_REPEAT_ID || !state.it.is_logged_in() {
        return;
    }
    let state = Arc::clone(state);
    let playlist_id = playlist_id.to_owned();
    tauri::async_runtime::spawn(async move {
        let Some(client) = state.clients.get(innertube::METADATA_CLIENT) else {
            return;
        };
        // Read here, not on the command's thread: the file was just written and the caller has its
        // answer already.
        let result = match std::fs::read(&path) {
            Ok(image) => {
                state
                    .it
                    .playlist_set_cover(client, &playlist_id, image)
                    .await
            }
            Err(e) => Err(innertube::Error::Other(e.to_string())),
        };
        match result {
            // Remembered so a later removal knows whether YouTube has anything of ours to drop.
            Ok(()) => state.db.set_setting(&synced_key(&playlist_id), "1"),
            Err(e) => {
                tracing::warn!(playlist_id, error = %e, "playlist cover didn't reach YouTube Music");
                let message = match e {
                    // The one refusal with a known cause and no fix inside this app. Say it once,
                    // plainly, and leave the cover where it already is: on this machine.
                    innertube::Error::CoverRefused => format!("Artwork saved on this device. {e}"),
                    e => format!("Artwork saved here, but the upload to YouTube Music failed: {e}"),
                };
                let _ = state
                    .app
                    .emit("cover-error", serde_json::json!({ "message": message }));
            }
        }
    });
}

/// Drop the custom thumbnail from the account, answering the one YouTube rebuilt from the tracks.
/// Nothing to do (and nothing to answer with) when there is no account behind the playlist.
async fn clear_cover_on_youtube(
    state: &Arc<AppState>,
    playlist_id: &str,
) -> Result<Option<String>, String> {
    if playlist_id == ON_REPEAT_ID || !state.it.is_logged_in() {
        return Ok(None);
    }
    let client = metadata_client(state)?;
    state
        .it
        .playlist_clear_cover(client, playlist_id)
        .await
        .map_err(|e| e.to_string())
}

fn cover_key(playlist_id: &str) -> String {
    // Browse ids arrive `VL`-prefixed and playlist ids don't; one playlist, one key either way.
    format!(
        "playlist_cover:{}",
        playlist_id.strip_prefix("VL").unwrap_or(playlist_id)
    )
}

/// Set once a cover of ours has actually landed on the account, so a removal knows whether there
/// is anything up there to warn about failing to clear.
fn synced_key(playlist_id: &str) -> String {
    format!("{}:synced", cover_key(playlist_id))
}

/// The custom artwork stored for a playlist, if the file is still there. The user owns that
/// directory and can empty it, and a dead path renders as a broken image.
fn custom_cover(state: &Arc<AppState>, playlist_id: &str) -> Option<String> {
    let path = state.db.get_setting(&cover_key(playlist_id))?;
    std::path::Path::new(&path).is_file().then_some(path)
}

#[tauri::command]
pub async fn delete_playlist(state: St<'_>, playlist_id: String) -> Result<(), String> {
    let client = editable_playlist(&state, &playlist_id)?;
    state
        .it
        .delete_playlist(client, &playlist_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn subscribe(state: St<'_>, channel_id: String, subscribed: bool) -> Result<(), String> {
    let client = require_login(&state)?;
    state
        .it
        .subscribe(client, &channel_id, subscribed)
        .await
        .map_err(|e| e.to_string())
}

// --- local music (local.rs) ------------------------------------------------------------------

/// Rescan the watched folders and return the library. The scan is the deletion check too: its
/// `removed` list is every id that was on screen but is gone from disk, so the UI can drop those
/// tiles without waiting for anyone to click a dead one.
#[tauri::command]
pub async fn get_local_library(state: St<'_>) -> Result<crate::local::LocalLibrary, String> {
    scan_local(&state).await
}

#[tauri::command]
pub async fn add_local_folder(
    state: St<'_>,
    path: String,
) -> Result<crate::local::LocalLibrary, String> {
    crate::local::add_folder(&state.db, path);
    scan_local(&state).await
}

/// Stop watching a folder. Its tracks disappear from the library on the rescan that follows (they
/// come back untouched if the folder is added again — nothing on disk is modified).
#[tauri::command]
pub async fn remove_local_folder(
    state: St<'_>,
    path: String,
) -> Result<crate::local::LocalLibrary, String> {
    crate::local::remove_folder(&state.db, &path);
    scan_local(&state).await
}

/// Disk IO + tag parsing off the async runtime's worker threads.
async fn scan_local(state: &Arc<AppState>) -> Result<crate::local::LocalLibrary, String> {
    let app = state.app.clone();
    let state = state.clone();
    let covers = crate::local::covers_dir(&state.app);
    let lib = tauri::async_runtime::spawn_blocking(move || crate::local::scan(&state.db, &covers))
        .await
        .map_err(|e| e.to_string())?;
    // Artwork reaches the page over the asset protocol, which starts out allowing nothing.
    crate::local::allow_covers(&app, &lib.songs);
    Ok(lib)
}

// --- Listen Together (context/19) ----------------------------------------------------------

/// Current client-side LT state (status, role, room, participants, pending joins, suggestions).
#[tauri::command]
pub async fn lt_get_state(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.lt.snapshot().await)
}

/// Set + persist the sync server URL (e.g. the Tailscale Funnel `wss://…` address).
#[tauri::command]
pub async fn lt_set_server_url(state: St<'_>, url: String) -> Result<(), String> {
    let url = url.trim().to_string();
    state.db.set_setting("lt_server_url", &url);
    state.lt.set_server_url(url).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_create_room(state: St<'_>, username: String) -> Result<(), String> {
    state.lt.create_room(username).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_join_room(state: St<'_>, code: String, username: String) -> Result<(), String> {
    state.lt.join_room(code, username).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_leave(state: St<'_>) -> Result<(), String> {
    state.lt.leave().await;
    Ok(())
}

#[tauri::command]
pub async fn lt_approve_join(state: St<'_>, user_id: String) -> Result<(), String> {
    state.lt.approve_join(user_id).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_reject_join(state: St<'_>, user_id: String) -> Result<(), String> {
    state.lt.reject_join(user_id).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_kick(state: St<'_>, user_id: String) -> Result<(), String> {
    state.lt.kick(user_id).await;
    Ok(())
}

#[tauri::command]
pub async fn lt_transfer_host(state: St<'_>, user_id: String) -> Result<(), String> {
    state.lt.transfer_host(user_id).await;
    Ok(())
}

/// Guest: send a track to the session queue (auto-approved by the host client, which stamps
/// who added it).
#[tauri::command]
pub async fn lt_suggest(state: St<'_>, item: SongItem) -> Result<(), String> {
    state.lt.suggest(crate::state::song_to_track(&item)).await;
    Ok(())
}

/// Host: approve a suggestion — add it to the real queue and notify the suggester. (Unused since
/// guest adds auto-approve, kept for a future "require approval" setting.)
#[tauri::command]
pub async fn lt_approve_suggestion(state: St<'_>, id: String) -> Result<(), String> {
    if let Some(track) = state.lt.approve_suggestion(id).await {
        state.inner().clone().lt_enqueue_track(track).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn lt_reject_suggestion(state: St<'_>, id: String) -> Result<(), String> {
    state.lt.reject_suggestion(id).await;
    Ok(())
}

/// Guest: force a re-sync with the room (drift correction).
#[tauri::command]
pub async fn lt_request_sync(state: St<'_>) -> Result<(), String> {
    state.lt.request_sync().await;
    Ok(())
}

// --- lyrics ---------------------------------------------------------------------------------

/// Lyrics for a track (cached). The UI passes the metadata it already has from `now-playing`;
/// `duration` is mpv's length in seconds. `None` = no lyrics found anywhere.
#[tauri::command]
pub async fn get_lyrics(
    state: St<'_>,
    video_id: String,
    title: String,
    artists: String,
    album: Option<String>,
    duration: Option<f64>,
) -> Result<Option<crate::lyrics::Lyrics>, String> {
    Ok(crate::lyrics::get_lyrics(
        state.inner(),
        crate::lyrics::LyricsRequest {
            video_id,
            title,
            artists,
            album,
            duration,
        },
    )
    .await)
}

/// Per-song lyric offset (ms) — persisted in `lyric_offsets`. The UI applies it as chip
/// ±250 ms steps and as a drag; the returned `Lyrics` from `get_lyrics` already has it applied
/// when re-fetched, but the live offset is kept client-side until the next fetch.
#[tauri::command]
pub fn get_lyric_offset(state: St<'_>, video_id: String) -> i64 {
    state.db.get_lyric_offset(&video_id)
}
#[tauri::command]
pub fn set_lyric_offset(state: St<'_>, video_id: String, offset_ms: i64) -> Result<(), String> {
    state
        .db
        .set_lyric_offset(&video_id, offset_ms.clamp(-5000, 5000));
    Ok(())
}
/// Unison vote/report (POST /lyrics/vote semantics). `vote` = 1 or -1.
#[tauri::command]
pub async fn lyrics_vote(
    state: St<'_>,
    video_id: String,
    source: String,
    vote: i32,
) -> Result<(), String> {
    crate::lyrics::vote_lyrics(&state, &video_id, &source, vote).await
}
#[tauri::command]
pub async fn lyrics_report(
    state: St<'_>,
    video_id: String,
    source: String,
    reason: String,
) -> Result<(), String> {
    crate::lyrics::report_lyrics(&state, &video_id, &source, &reason).await
}
/// Translate via translate.googleapis (44 langs). `target` is a BCP code (e.g. `en`, `ja`).
#[tauri::command]
pub async fn translate_lyrics(text: String, target: String) -> Result<String, String> {
    crate::lyrics::translate_text(&text, &target)
        .await
        .map_err(|e| e.to_string())
}
/// Romanize (kana → romaji, pykakasi-lite). Returns the romanized string.
#[tauri::command]
pub fn romanize_lyrics(text: String) -> String {
    crate::lyrics::romanize_text(&text)
}

// --- Video Sync ------------------------------------------------------------------------

#[tauri::command]
pub fn set_video_sync(state: St<'_>, enabled: bool) -> Result<(), String> {
    state
        .player
        .set_video_sync(enabled)
        .map_err(|e| e.to_string())?;
    // also persist for next launch as internal setting
    state
        .db
        .set_setting("video_sync", if enabled { "true" } else { "false" });
    Ok(())
}
#[tauri::command]
pub fn get_video_sync(state: St<'_>) -> bool {
    state.db.get_setting("video_sync").as_deref() == Some("true") || state.player.get_video_sync()
}

// --- Changelog ------------------------------------------------------------------------------

#[derive(Clone, serde::Serialize)]
pub struct ReleaseNote {
    version: String,
    /// `YYYY-MM-DD`, or empty for an unpublished tag.
    date: String,
    /// The release description, verbatim markdown. The About tab renders it.
    body: String,
}

/// What's new, read straight from the GitHub releases API so the release description is the only
/// place the changelog is written. Cached for the process: the list only changes when a release
/// is cut, and unauthenticated GitHub allows 60 requests an hour.
#[tauri::command]
pub async fn release_notes() -> Result<Vec<ReleaseNote>, String> {
    static CACHE: std::sync::OnceLock<Vec<ReleaseNote>> = std::sync::OnceLock::new();
    if let Some(cached) = CACHE.get() {
        return Ok(cached.clone());
    }
    #[derive(serde::Deserialize)]
    struct GhRelease {
        tag_name: String,
        published_at: Option<String>,
        body: Option<String>,
        draft: bool,
        prerelease: bool,
    }
    let http = reqwest::Client::builder()
        .user_agent(concat!("Limusic/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;
    let releases: Vec<GhRelease> = http
        .get("https://api.github.com/repos/Naitik571/limusic/releases?per_page=20")
        .header("User-Agent", concat!("Limusic/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let notes: Vec<ReleaseNote> = releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .map(|r| ReleaseNote {
            version: r.tag_name.trim_start_matches('v').to_string(),
            date: r
                .published_at
                .and_then(|d| d.split('T').next().map(str::to_string))
                .unwrap_or_default(),
            body: r.body.unwrap_or_default(),
        })
        .collect();
    Ok(CACHE.get_or_init(|| notes).clone())
}

/// Whether this build can install an update itself, or only point the user at the download.
///
/// Windows runs the NSIS installer, which works however the app was installed.
#[tauri::command]
pub fn can_self_update(app: tauri::AppHandle) -> bool {
    // Windows runs the NSIS installer, which works however the app was installed.
    let _ = app;
    true
}

/// Open a link from the UI in the real browser. An `<a href>` inside the webview would navigate
/// the app itself off the SPA, with no way back.
#[tauri::command]
pub async fn open_external(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("only http(s) links".into());
    }
    crate::lastfm::open_browser(&url)
}

// --- Last.fm scrobbling ---------------------------------------------------------------------

/// Start the browser auth flow. Returns once the authorize page is open; the outcome (session
/// stored, or an error) arrives via the `lastfm-state` event.
#[tauri::command]
pub async fn lastfm_connect(state: St<'_>) -> Result<(), String> {
    crate::lastfm::connect(state.inner().clone()).await
}

#[tauri::command]
pub async fn lastfm_disconnect(state: St<'_>) -> Result<(), String> {
    crate::lastfm::disconnect(&state);
    Ok(())
}

/// `{ connected, username }` from the persisted session — seeds the titlebar button on mount.
#[tauri::command]
pub async fn lastfm_status(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(crate::lastfm::status(&state))
}

// --- offline downloads ----------------------------------------------------------------------

/// Save a track for offline playback. Resolves its stream and writes the audio to the download
/// directory; the resolver then plays the local file on every later play. Emits progress/complete
/// events. Re-running on an already-downloaded track is a no-op.
/// `respect_quarantine` (automatic callers only): skip tracks that failed repeatedly recently
/// instead of retrying them. Manual downloads omit it and always try.
#[tauri::command]
pub async fn download_track(
    app: tauri::AppHandle,
    state: St<'_>,
    video_id: String,
    title: String,
    artists: String,
    album: Option<String>,
    // `Option` + default below: the UI sometimes sends an M:SS string (→ NaN → null in JSON)
    // or `null` outright, and the field is only catalogue metadata, so a missing value is fine.
    duration: Option<i64>,
    thumb: Option<String>,
    respect_quarantine: Option<bool>,
) -> Result<(), String> {
    crate::downloads::download_track(
        &app,
        &state,
        &state.orchestrator,
        &video_id,
        &title,
        &artists,
        album.as_deref(),
        duration.unwrap_or(0),
        thumb.as_deref(),
        respect_quarantine.unwrap_or(false),
    )
    .await
}

/// Parse a `"m:ss"` / `"h:mm:ss"` duration string into seconds (0 if absent/unparseable).
fn duration_secs(s: Option<&str>) -> i64 {
    let Some(s) = s else { return 0 };
    let parts: Vec<i64> = s.split(':').filter_map(|p| p.trim().parse().ok()).collect();
    match parts.as_slice() {
        [secs] => *secs,
        [m, secs] => m * 60 + secs,
        [h, m, secs] => h * 3600 + m * 60 + secs,
        _ => 0,
    }
}

#[cfg(test)]
mod duration_tests {
    use super::duration_secs;

    #[test]
    fn parses_every_shape_the_ui_sends() {
        assert_eq!(duration_secs(Some("3:21")), 201);
        assert_eq!(duration_secs(Some("1:02:03")), 3723);
        assert_eq!(duration_secs(Some("45")), 45);
        // Garbage and absences are catalogue metadata only — zero, never an error.
        assert_eq!(duration_secs(None), 0);
        assert_eq!(duration_secs(Some("")), 0);
        assert_eq!(duration_secs(Some("not:a:time")), 0);
        assert_eq!(
            duration_secs(Some("3:xx")),
            3,
            "parseable segments still count"
        );
    }
}

/// Download every (non-local, not-yet-saved) track in a playlist or album. Walks ALL pages so a
/// long playlist downloads in full — the old behaviour stopped at the first ~100 tracks — then
/// pulls the missing ones DOWNLOAD_CONCURRENCY at a time. Reports a full summary
/// `{ ok, total, skipped, downloaded, failed }` so the UI can toast exactly what happened:
/// `skipped` counts both local files and tracks already in the offline catalogue, so re-running a
/// download only fetches what's actually missing (60 of 100 already saved → 40 fetched).
#[tauri::command]
pub async fn download_playlist(
    app: tauri::AppHandle,
    state: St<'_>,
    id: String,
) -> Result<serde_json::Value, String> {
    download_playlist_walk(&app, state.inner(), &id).await
}

/// One walk per playlist at a time. A startup backfill, a playlist-page top-up and a manual
/// "Download" click can otherwise overlap on the same list: both compute candidates from the
/// same catalogue state, both download the same missing tracks into the same `.part` file, and
/// the loser renames garbage over the winner. The second caller simply waits, then re-walks —
/// by then everything is skipped and it returns almost instantly.
static WALK_LOCKS: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
> = std::sync::OnceLock::new();

fn walk_locks(
) -> &'static std::sync::Mutex<std::collections::HashMap<String, Arc<tokio::sync::Mutex<()>>>> {
    WALK_LOCKS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// The playlist-download walk, shared by the command and the auto-offline backfill (which calls
/// it with the Liked Music browse id at startup / on enable). Walks every page, dedupes, skips
/// what's already on disk or a local file, then fetches the remainder `DOWNLOAD_CONCURRENCY` at
/// a time. Summary shape: `{ ok, total, skipped, downloaded, failed, cancelled }`.
async fn download_playlist_walk(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    id: &str,
) -> Result<serde_json::Value, String> {
    // Serialize overlapping walks of the same list (see WALK_LOCKS).
    let slot = {
        walk_locks()
            .lock()
            .unwrap()
            .entry(id.to_owned())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    };
    let _slot = slot.lock().await;
    let client = metadata_client(state)?;
    let page = state
        .it
        .playlist(client, id)
        .await
        .map_err(|e| e.to_string())?;

    // Walk every page (bounded the same way the queue fill is). A continuation that hands back an
    // empty page ends the walk.
    let mut items = page.items;
    let mut token = page.continuation;
    let mut pages = 0usize;
    while let Some(t) = token {
        if pages >= 50 {
            break;
        }
        pages += 1;
        let more = state
            .it
            .playlist_continuation(client, &t)
            .await
            .map_err(|e| e.to_string())?;
        items.extend(more.items);
        token = more.continuation;
    }

    // Dedupe by video id (a playlist can carry the same song twice), drop local files,
    // quarantined failures and tracks that are already downloaded, then fetch only the
    // remainder. Quarantined tracks are skipped silently (not counted): they are neither
    // downloaded nor "already saved", and the next manual attempt clears them.
    let mut seen = std::collections::HashSet::new();
    let mut candidates: Vec<crate::downloads::DownloadCandidate> = Vec::new();
    let mut skipped = 0u32;
    for item in items {
        if crate::local::is_local_song(&item.video_id) {
            skipped += 1;
            continue;
        }
        if !seen.insert(item.video_id.clone()) {
            continue; // duplicate row — don't count it against anything, it's one song
        }
        if crate::downloads::download_quarantined(&state.db, &item.video_id) {
            continue;
        }
        if crate::downloads::is_downloaded(state, &item.video_id) {
            skipped += 1;
            continue;
        }
        candidates.push(crate::downloads::DownloadCandidate {
            video_id: item.video_id,
            title: item.title,
            artists: item.artists,
            album: item.album,
            duration: duration_secs(item.duration.as_deref()),
            thumb: item.thumbnail,
        });
    }

    let total = candidates.len() as u32 + skipped;
    let (completed, failed, cancelled) =
        crate::downloads::download_many(app, state, &state.orchestrator, candidates).await;
    Ok(serde_json::json!({
        "ok": true,
        "total": total,
        "skipped": skipped,
        "downloaded": completed as u32,
        "failed": failed as u32,
        "cancelled": cancelled as u32,
    }))
}

/// Stop one in-flight (or queued) download. The writer drops its partial file and reports
/// `download-cancelled`; the manager row resolves to "Cancelled".
#[tauri::command]
pub async fn cancel_download(_state: St<'_>, video_id: String) -> Result<bool, String> {
    Ok(crate::downloads::cancel_download(&video_id))
}

/// Stop everything: every in-flight track and any batch that hasn't started them yet.
#[tauri::command]
pub async fn cancel_all_downloads(_state: St<'_>) -> Result<u32, String> {
    Ok(crate::downloads::cancel_all_downloads() as u32)
}

/// Auto-offline: when enabled (Settings → Downloads), walk Liked Music once and fetch anything
/// not yet in the offline catalogue. Runs at startup (delayed so it never slows the launch) and
/// again when the setting is turned on mid-session. The playlist walk dedupes and skips
/// what's already on disk, so a backfill costs exactly the missing tracks.
pub async fn auto_offline_backfill(app: tauri::AppHandle, state: Arc<AppState>) {
    let mode = state.db.get_setting("auto_offline").unwrap_or_default();
    if mode != "liked" && mode != "liked_playlists" {
        return;
    }
    tracing::info!("auto-offline: syncing Liked Music");
    match download_playlist_walk(&app, &state, "VLLM").await {
        Ok(v) => tracing::info!(summary = %v, "auto-offline: Liked Music sync finished"),
        Err(e) => tracing::warn!(error = %e, "auto-offline: Liked Music sync failed"),
    }
}

/// Same walk, invoked from the UI the moment the setting is switched on — so the user doesn't
/// have to relaunch to get their first backfill. Errors when the setting is off.
#[tauri::command]
pub async fn auto_offline_sync(
    app: tauri::AppHandle,
    state: St<'_>,
) -> Result<serde_json::Value, String> {
    let mode = state.db.get_setting("auto_offline").unwrap_or_default();
    if mode != "liked" && mode != "liked_playlists" {
        return Err("auto-offline is off".into());
    }
    download_playlist_walk(&app, state.inner(), "VLLM").await
}

// --- listen history (the local play diary) ---------------------------------------------------

/// One entry of the History page: what played, and when. `plays` is the same table On Repeat
/// ranks — this is the chronological diary view of it, duplicates and all.
#[derive(serde::Serialize)]
pub struct HistoryEntry {
    #[serde(rename = "playedAt")]
    played_at: i64,
    song: SongItem,
}

#[tauri::command]
pub async fn get_history(state: St<'_>, limit: Option<u32>) -> Result<Vec<HistoryEntry>, String> {
    let limit = limit.unwrap_or(500).clamp(1, 2000) as i64;
    Ok(state
        .db
        .recent_plays(limit)
        .into_iter()
        .filter_map(
            |(played_at, json)| match serde_json::from_str::<SongItem>(&json) {
                Ok(song) => Some(HistoryEntry { played_at, song }),
                Err(_) => None, // a row from an older schema — skip rather than break the page
            },
        )
        .collect())
}

/// Wipe the play diary. On Repeat rebuilds from new plays; history goes blank.
#[tauri::command]
pub async fn clear_history(state: St<'_>) -> Result<(), String> {
    state.db.clear_plays();
    Ok(())
}

/// Catalogue of downloaded tracks, newest first, with `total_bytes`. Mirrors what the settings
/// "Downloads" list renders.
#[tauri::command]
pub async fn list_downloads(state: St<'_>) -> Result<serde_json::Value, String> {
    let rows = state.db.list_downloads();
    let total = state.db.downloads_total_bytes();
    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|d| {
            serde_json::json!({
                "video_id": d.video_id,
                "file_path": d.file_path,
                "title": d.title,
                "artists": d.artists,
                "album": d.album,
                "duration": d.duration,
                "thumb": d.thumb,
                "quality": d.quality,
                "format": d.format,
                "size_bytes": d.size_bytes,
                "added_at": d.added_at,
            })
        })
        .collect();
    Ok(serde_json::json!({ "items": items, "total_bytes": total }))
}

/// Remove one downloaded track (file + catalogue row).
#[tauri::command]
pub async fn delete_download(state: St<'_>, video_id: String) -> Result<(), String> {
    crate::downloads::delete_track(&state.db, &video_id)
}

/// Wipe every download. The files go with the rows.
#[tauri::command]
pub async fn clear_downloads(state: St<'_>) -> Result<(), String> {
    for d in state.db.list_downloads() {
        let _ = std::fs::remove_file(&d.file_path);
        state.db.delete_download(&d.video_id);
    }
    Ok(())
}

// --- Crossfade / best mix ------------------------------------------------------------

#[tauri::command]
pub async fn get_crossfade(state: St<'_>) -> Result<serde_json::Value, String> {
    Ok(state.get_crossfade())
}
#[tauri::command]
pub async fn set_crossfade(state: St<'_>, secs: f64, mode: String) -> Result<(), String> {
    state.set_crossfade(secs, &mode);
    Ok(())
}
#[tauri::command]
pub async fn set_best_mix(state: St<'_>, on: bool) -> Result<(), String> {
    let s = state.inner().clone();
    s.set_best_mix(on);
    if on {
        s.apply_best_mix_sort().await;
    }
    Ok(())
}

// --- Remote LAN QR (#5) ------------------------------------------------------------------
#[tauri::command]
pub fn get_lan_url(state: St<'_>) -> String {
    crate::remote::lan_url(&state.db)
}
#[tauri::command]
pub fn get_remote_token(state: St<'_>) -> String {
    crate::remote::get_or_create_token(&state.db)
}
#[tauri::command]
pub fn pair_remote(state: St<'_>, token: String) -> Result<bool, String> {
    let expected = crate::remote::get_or_create_token(&state.db);
    if token == expected {
        state
            .db
            .set_setting("remote_paired_at", &crate::db::now_secs().to_string());
        Ok(true)
    } else {
        Ok(false)
    }
}

/// The LAN pairing URL as a scannable QR code (SVG markup). The UI renders it in a light box —
/// QR needs contrast against the dark theme to scan.
#[tauri::command]
pub fn get_remote_qr(state: St<'_>) -> Result<String, String> {
    let url = crate::remote::lan_url(&state.db);
    let code = qrcode::QrCode::with_error_correction_level(url.as_bytes(), qrcode::EcLevel::M)
        .map_err(|e| format!("qr encode failed: {e}"))?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .min_dimensions(220, 220)
        .build())
}

// --- Artist Packs (#9) -------------------------------------------------------------------
#[tauri::command]
pub async fn list_artist_packs(
    state: St<'_>,
) -> Result<Vec<crate::artist_packs::ArtistPack>, String> {
    Ok(crate::artist_packs::list_packs(&state.db))
}
#[tauri::command]
pub async fn get_artist_pack(
    state: St<'_>,
    id: String,
) -> Result<Option<crate::artist_packs::ArtistPack>, String> {
    Ok(crate::artist_packs::get_pack(&state.db, &id))
}
#[tauri::command]
pub async fn remove_artist_pack(
    app: tauri::AppHandle,
    state: St<'_>,
    id: String,
) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    crate::artist_packs::remove_pack(&state.db, &data_dir, &id)
}
#[tauri::command]
pub async fn install_artist_pack(
    app: tauri::AppHandle,
    state: St<'_>,
    url: String,
) -> Result<crate::artist_packs::ArtistPack, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    crate::artist_packs::install_from_url(state.inner().db.clone(), data_dir, url).await
}
#[tauri::command]
pub async fn install_artist_pack_zip(
    app: tauri::AppHandle,
    state: St<'_>,
    path: String,
) -> Result<crate::artist_packs::ArtistPack, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    crate::artist_packs::install_from_zip(&state.db, &data_dir, std::path::Path::new(&path))
}
#[tauri::command]
pub async fn fetch_artist_packs_index() -> Result<crate::artist_packs::ArtistPackIndex, String> {
    crate::artist_packs::fetch_index().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_repeat_rows_shed_the_queue_slot_they_were_played_from() {
        let played = SongItem {
            video_id: "abc".into(),
            title: "Grace".into(),
            queued: true,
            queued_by: Some("simohypers".into()),
            autoplay: true,
            set_video_id: Some("SVI".into()),
            ..Default::default()
        };
        let row = shed_queue_context(played.clone());
        assert_eq!(
            row,
            SongItem {
                video_id: "abc".into(),
                title: "Grace".into(),
                ..Default::default()
            }
        );
        assert_eq!(row.title, played.title, "the song itself survives");
    }
}
