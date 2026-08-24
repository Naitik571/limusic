//! Limusic Tauri app. Wires transport + player + db + orchestrator behind the command boundary.

mod art;
mod artist_packs;
mod canvas;
mod cipher;
mod commands;
mod db;
mod discord;
mod downloads;
mod gamepad;
mod lastfm;
mod listentogether;
mod local;
mod lyrics;
mod media;
mod mini;
mod orchestrator;
mod potoken;
mod remote;
mod session;
mod state;
mod tray;
mod webview;
mod ytdlp;

use std::sync::Arc;
use std::time::Duration;

use innertube::{Clients, InnerTube, Locale, Session};
use player::{Player, PlayerEvent};
use tauri::{Emitter, Manager};

use cipher::{CipherDeobfuscator, PlayerConfigStore};
use db::Db;
use orchestrator::Orchestrator;
use potoken::PoTokenGenerator;
use state::AppState;

/// 1 Hz sleep-timer tick. When the countdown deadline passes: pause playback, emit
/// `sleep-timer-fired` so open windows clear their chip, and disarm. `EndOfSong` is handled in
/// `AppState::on_track_ended`, not here. The thread outlives any window (tray/mini-player
/// playback keeps counting), which is the point of enforcing it in Rust rather than the UI.
fn spawn_sleep_timer(state: Arc<crate::state::AppState>) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let fire = {
            let mut timer = state.sleep_timer.lock().unwrap();
            match *timer {
                crate::state::SleepTimer::At(end) if std::time::Instant::now() >= end => {
                    *timer = crate::state::SleepTimer::Off;
                    true
                }
                _ => false,
            }
        };
        if fire {
            let _ = state.player.pause();
            let _ = state.app.emit("sleep-timer-fired", ());
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,limusic_app=debug".into()),
        )
        .init();

    tauri::Builder::default()
        // Must be the first plugin registered (its documented requirement). A second launch —
        // e.g. clicking the app icon while we're hidden in the tray — re-shows this instance
        // instead of spawning a second one (which would fight over SQLite and mpv).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Folder picker for the local-music library (local.rs).
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            {
                let builder = tauri_plugin_global_shortcut::Builder::new();
                let shortcuts = [
                    "CommandOrControl+Shift+Up",
                    "CommandOrControl+Shift+Down",
                ];
                // Use only safe shortcuts; media keys are handled via SMTC already and global registration can fail on some systems
                match builder.with_shortcuts(shortcuts) {
                    Ok(b) => b
                        .with_handler(|app, shortcut, event| {
                            if event.state != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                                return;
                            }
                            let Some(state) = app.try_state::<Arc<AppState>>() else {
                                return;
                            };
                            let state = state.inner().clone();
                            match shortcut.to_string().as_str() {
                                "CommandOrControl+Shift+Up" => {
                                    let vol = (state.player.get_volume() + 5).min(100);
                                    let _ = state.player.set_volume(vol);
                                    state.db.set_setting("volume", &vol.to_string());
                                    let _ = app.emit("volume", vol);
                                }
                                "CommandOrControl+Shift+Down" => {
                                    let vol = (state.player.get_volume() - 5).max(0);
                                    let _ = state.player.set_volume(vol);
                                    state.db.set_setting("volume", &vol.to_string());
                                    let _ = app.emit("volume", vol);
                                }
                                _ => {}
                            }
                        })
                        .build(),
                    Err(e) => {
                        tracing::warn!(error = %e, "global shortcuts registration failed, continuing without them");
                        tauri_plugin_global_shortcut::Builder::new().build()
                    }
                }
            }
        )
        .setup(|app| {
            let handle = app.handle().clone();

            // App data dir for the SQLite file and mpv's on-disk audio cache.
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::env::temp_dir());
            std::fs::create_dir_all(&data_dir).ok();
            let cache_dir = data_dir.join("audio-cache");
            std::fs::create_dir_all(&cache_dir).ok();

            // Shared: the PoToken generator persists its session token through the same file,
            // and it is built before AppState takes ownership of everything else.
            let db = Arc::new(
                Db::open(&data_dir.join("limusic.sqlite")).unwrap_or_else(|e| {
                    tracing::error!(error = %e, "open sqlite failed, trying temp dir");
                    Db::open(&std::env::temp_dir().join("limusic.sqlite"))
                        .unwrap_or_else(|e2| {
                            tracing::error!(error = %e2, "temp sqlite also failed, using in-memory");
                            Db::open(std::path::Path::new(":memory:")).expect("in-memory sqlite must open")
                        })
                }),
            );

            // Session bootstrap (context/15 startup ordering): load the persisted login session
            // (cookie/dataSyncId/visitorData) from settings; fetch visitorData anonymously
            // (context/04 Â§A) only if we've never stored one.
            let proxy = db.get_setting("proxy");
            let cookie = db.get_setting("session_cookie").filter(|s| !s.is_empty());
            let data_sync_id = state::persisted_data_sync_id(&db);
            let visitor_data = db.get_setting("visitor_data").filter(|s| !s.is_empty());
            // First run (no stored visitorData): bootstrap it in the background after the window is
            // up, rather than blocking setup on a network GET (up to 60s on a bad connection). See
            // the spawned task after AppState is created.
            let needs_visitor_bootstrap = visitor_data.is_none();
            if cookie.is_some() {
                tracing::info!("loaded persisted login session");
            }

            let visitor_for_prewarm = visitor_data.clone();
            let session = Session {
                locale: Locale::default(),
                visitor_data,
                data_sync_id,
                cookie,
            };
            let it = InnerTube::new(session, proxy.as_deref()).expect("build InnerTube");
            it.set_hide_videos(db.get_setting("hide_videos").as_deref() == Some("true"));
            let clients = Clients::bundled();

            let mut player = match Player::new(cache_dir.to_str().unwrap()) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(error = %e, "init libmpv failed, app will open without audio");
                    // Create a fallback player that at least allows UI to open; audio will be unavailable
                    // Try once more with minimal config
                    Player::new(std::env::temp_dir().to_str().unwrap_or("C:\\Temp")).unwrap_or_else(|e2| {
                        tracing::error!(error = %e2, "fallback libmpv also failed, using dummy");
                        // Last resort: create a dummy player that doesn't use mpv (will fail later but lets UI open)
                        // For now panic with a user-visible error instead of silent close
                        panic!("libmpv init failed: {} (fallback also failed: {}) - ensure libmpv-2.dll is next to the exe", e, e2);
                    })
                }
            };
            let events = player.take_events().unwrap_or_else(|| {
                tracing::error!("player events missing, creating dummy channel");
                let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
                rx
            });

            // Phase 2 extraction stack: cipher + PoToken hidden webviews behind the orchestrator.
            let config = Arc::new(PlayerConfigStore::new(&data_dir));
            let cipher = Arc::new(CipherDeobfuscator::new(handle.clone(), &data_dir, config));
            let potoken = Arc::new(PoTokenGenerator::new(handle.clone(), db.clone()));
            // yt-dlp fallback (2026-08 round): managed binary, prewarmed in the background so the
            // first restricted track doesn't pay the download cost. Enabled by default; the
            // settings toggle flips it live.
            let ytdlp = Arc::new(ytdlp::YtDlp::new(
                handle.clone(),
                db.get_setting("ytdlp_enabled")
                    .as_deref()
                    .map_or(true, |v| v == "true"),
            ));
            {
                let y = ytdlp.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = y.ensure_ready().await;
                });
            }
            let orchestrator = Arc::new(Orchestrator::new(
                it.clone(),
                clients.clone(),
                cipher.clone(),
                potoken.clone(),
                Some(ytdlp.clone()),
            ));

            // OS media controls (MPRIS/SMTC/NowPlaying). Its callback resolves AppState lazily, so
            // it's fine to spawn before AppState is managed. context/16, D11.
            let media = media::spawn(handle.clone());

            // Gamepad poller (Xbox/PS). Emits `gamepad` events the frontend turns into playback
            // actions; works while the app is backgrounded/tray-minimized.
            gamepad::start(handle.clone());

            // Discord rich presence â€” off unless the user opted in; parks on its channel until then.
            let discord = discord::spawn(db.get_setting("discord_rpc").as_deref() == Some("true"));

            // Last.fm scrobbler â€” parks until a session key exists (titlebar connect flow).
            let lastfm = lastfm::spawn(
                db.get_setting("lastfm_session_key")
                    .filter(|s| !s.is_empty()),
            );

            // Listen Together session (context/19). Server URL is a DB setting so "home PC â†’ VPS" is
            // config, not a rebuild. The sync channel feeds the guest-playback bridge below.
            let lt_url = db
                .get_setting("lt_server_url")
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "wss://fedora-1.tail9c4985.ts.net/ws".into());
            let (lt, lt_sync_rx) = listentogether::LtSession::new(handle.clone(), lt_url);

            let app_state = Arc::new(AppState::new(
                it,
                clients,
                player,
                db,
                handle.clone(),
                orchestrator,
                ytdlp,
                lt,
                cache_dir.clone(),
                media,
                discord,
                lastfm,
            ));
            app.manage(app_state.clone());

            // Local music artwork reaches the webview over the asset protocol, whose configured
            // scope is empty â€” the folders it may read are the ones the user picked (local.rs).
            local::allow_music_paths(&handle, &app_state.db);

            // System tray: playback controls + show/quit while running in the background.
            if let Err(e) = tray::init(&handle) {
                tracing::warn!(error = %e, "tray init failed (continuing without tray)");
            }

            // Bridge: apply Listen Together sync commands (guest playback / host seed) to AppState.
            {
                let st = app_state.clone();
                let mut rx = lt_sync_rx;
                tauri::async_runtime::spawn(async move {
                    while let Some(cmd) = rx.recv().await {
                        st.apply_sync(cmd).await;
                    }
                });
            }

            // Restore audio settings (EQ / crossfade / device) from DB
            {
                let bands = app_state.get_eq_bands();
                for (i, g) in bands.iter().enumerate() {
                    let _ = app_state.player.set_eq(i, *g);
                }
                let pre: f64 = app_state
                    .db
                    .get_setting("eq_preamp")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let _ = app_state.player.set_preamp(pre);
                let bal: f64 = app_state
                    .db
                    .get_setting("eq_balance")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let _ = app_state.player.set_balance(bal);
                let og: f64 = app_state
                    .db
                    .get_setting("eq_output_gain")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let _ = app_state.player.set_output_gain(og);
                let cf: f64 = app_state
                    .db
                    .get_setting("crossfade_secs")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let mode = app_state
                    .db
                    .get_setting("crossfade_mode")
                    .unwrap_or_else(|| "standard".into());
                let _ = app_state.player.set_crossfade(cf, &mode);
                // Video Sync: restore mpv `vid` from persisted setting
                if app_state.db.get_setting("video_sync").as_deref() == Some("true") {
                    let _ = app_state.player.set_video_sync(true);
                }
                // Volume: restore persisted level (exponential curve EXPONENT=3)
                if let Some(v) = app_state
                    .db
                    .get_setting("volume")
                    .and_then(|s| s.parse::<i64>().ok())
                {
                    let _ = app_state.player.set_volume(v.clamp(0, 100));
                    let _ = handle.emit("volume", v.clamp(0, 100));
                }
            }
            // Restore the last session's queue (paused, not autoplaying). context/11 Â§state.
            {
                let st = app_state.clone();
                tauri::async_runtime::spawn(async move {
                    st.restore_queue().await;
                });
            }

            // First-run visitorData bootstrap, off the startup path. `set_visitor_data` writes
            // through the shared session (Arc<RwLock>), so the orchestrator's InnerTube clone sees
            // it; resolves degrade gracefully (no PoToken) until it lands. context/04 Â§A.
            if needs_visitor_bootstrap {
                let st = app_state.clone();
                let potoken = potoken.clone();
                tauri::async_runtime::spawn(async move {
                    match st.it.fetch_visitor_data().await {
                        Ok(vd) => {
                            st.it.set_visitor_data(Some(vd.clone()));
                            st.db.set_setting("visitor_data", &vd);
                            tracing::info!("visitorData bootstrapped (background)");
                            potoken.prewarm(&vd).await;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "visitorData bootstrap failed (continuing)")
                        }
                    }
                });
            }

            // 1 Hz sleep-timer tick: pause + notify when the countdown expires. A plain thread â€”
            // the check is a few ns, `Player::pause` is sync, and the app already has several
            // std threads (media, discord, lastfm) for the same reason.
            remote::spawn(app_state.clone());
            artist_packs::spawn_index_poller(handle.clone(), app_state.db.clone());

            spawn_sleep_timer(app_state.clone());

            // Pump mpv events â†’ UI events + queue advance. context/11 events, context/14 Â§TrackEnded.
            spawn_event_pump(app_state, handle, events);

            // Prewarm the webviews off the first-play path (context/04 Â§startup). The delays let
            // the event loop come up first (run_on_main_thread needs it pumping).
            {
                let cipher = cipher.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    cipher.prewarm().await;
                });
            }
            if let Some(vd) = visitor_for_prewarm {
                let potoken = potoken.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(2500)).await;
                    potoken.prewarm(&vd).await;
                });
            }
            // Mint-and-destroy policy (Phase-0 decision): drop the PoToken webview when idle.
            {
                let potoken = potoken.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        potoken.teardown_if_idle(Duration::from_secs(60)).await;
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search,
            commands::search_all,
            commands::search_cards,
            commands::play,
            commands::play_index,
            commands::remove_from_queue,
            commands::move_queue_item,
            commands::clear_queued,
            commands::add_to_queue,
            commands::play_next,
            commands::next_track,
            commands::prev_track,
            commands::toggle_shuffle,
            commands::set_repeat,
            commands::toggle_pause,
            commands::seek,
            commands::set_volume,
            commands::set_sleep_timer,
            commands::get_sleep_timer,
            commands::get_queue,
            commands::get_playback,
            commands::get_settings,
            commands::set_setting,
            commands::get_stream_clients,
            commands::ytdlp_info,
            commands::get_highres_art,
            commands::get_canvas,
            commands::get_volume,
            commands::ytdlp_install_now,
            commands::clear_caches,
            commands::list_downloads,
            commands::download_track,
            commands::download_playlist,
            commands::cancel_download,
            commands::cancel_all_downloads,
            commands::delete_download,
            commands::clear_downloads,
            commands::get_account,
            commands::get_account_identities,
            commands::switch_account,
            commands::sign_out,
            commands::login_webview,
            commands::open_mini,
            commands::close_mini,
            commands::get_home,
            commands::get_home_more,
            commands::get_library,
            commands::get_library_albums,
            commands::get_library_artists,
            commands::get_playlist,
            commands::get_playlist_more,
            commands::play_counts,
            commands::get_album,
            commands::get_local_library,
            commands::add_local_folder,
            commands::remove_local_folder,
            commands::allow_font_file,
            commands::get_artist,
            commands::get_browse_grid,
            commands::play_playlist,
            commands::start_radio,
            commands::get_similar_songs,
            commands::like,
            commands::set_album_saved,
            commands::add_to_playlist,
            commands::remove_from_playlist,
            commands::create_playlist,
            commands::edit_playlist_details,
            commands::set_playlist_cover,
            commands::delete_playlist,
            commands::subscribe,
            commands::lt_get_state,
            commands::lt_set_server_url,
            commands::lt_create_room,
            commands::lt_join_room,
            commands::lt_leave,
            commands::lt_approve_join,
            commands::lt_reject_join,
            commands::lt_kick,
            commands::lt_transfer_host,
            commands::lt_suggest,
            commands::lt_approve_suggestion,
            commands::lt_reject_suggestion,
            commands::lt_request_sync,
            commands::get_lyrics,
            commands::get_lyric_offset,
            commands::set_lyric_offset,
            commands::lyrics_vote,
            commands::lyrics_report,
            commands::translate_lyrics,
            commands::romanize_lyrics,
            commands::set_video_sync,
            commands::get_video_sync,
            commands::lastfm_connect,
            commands::lastfm_disconnect,
            commands::lastfm_status,
            commands::release_notes,
            commands::can_self_update,
            commands::open_external,
            commands::get_eq,
            commands::set_eq,
            commands::get_eq_bands,
            commands::set_eq_bands,
            commands::set_preamp,
            commands::set_balance,
            commands::set_output_gain,
            commands::set_autoeq,
            commands::set_track_gain,
            commands::get_output_devices,
            commands::set_output_device,
            commands::get_crossfade,
            commands::set_crossfade,
            commands::set_best_mix,
            commands::get_lan_url,
            commands::get_remote_token,
            commands::pair_remote,
            commands::list_artist_packs,
            commands::install_artist_pack,
            commands::install_artist_pack_zip,
            commands::remove_artist_pack,
            commands::get_artist_pack,
            commands::fetch_artist_packs_index,
        ])
        .on_window_event(|window, event| {
            // Close-to-tray: âœ• hides the main window and playback keeps running; real quit is
            // the tray's Quit item (or the "close_to_tray=false" setting). Label-gated: the
            // hidden cipher/PoToken webviews are windows too and must close normally.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                match window.label() {
                    "main" => {
                        let hide = window
                            .app_handle()
                            .try_state::<Arc<AppState>>()
                            .map(|s| close_hides(s.db.get_setting("close_to_tray").as_deref()))
                            .unwrap_or(true);
                        if hide {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                    }
                    // Nothing in the widget closes it, but a WM shortcut still can. Turn that into
                    // the ordinary "back to the app" path â€” closing it on its own would leave the
                    // app running with no window at all.
                    mini::LABEL => {
                        api.prevent_close();
                        tray::show_main(window.app_handle());
                    }
                    _ => {}
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|handle, event| {
            // The hidden cipher/PoToken webviews are windows too, so closing the main window no
            // longer auto-exits the app. Quit when the main window is destroyed.
            if let tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Destroyed,
                ..
            } = &event
            {
                if label == "main" {
                    handle.exit(0);
                }
            }
        });
}

/// âœ• hides to tray unless the user explicitly set close_to_tray=false (unset â†’ default on).
fn close_hides(setting: Option<&str>) -> bool {
    setting != Some("false")
}

/// Decide whether a position tick is worth forwarding to the UI. Passes ~4 Hz of steady
/// playback through, plus any discontinuity (seek/track change) immediately so the slider
/// never lags a jump. Pure so it's testable; the pump owns the state.
// ponytail: fixed 250ms cadence; make it adaptive only if someone ever wants sub-second UI time.
struct PositionThrottle {
    last_emit: std::time::Instant,
    last_pos: f64,
}

impl PositionThrottle {
    fn new() -> Self {
        Self {
            last_emit: std::time::Instant::now() - std::time::Duration::from_secs(1),
            last_pos: f64::NAN,
        }
    }
    fn should_emit(&mut self, pos: f64, now: std::time::Instant) -> bool {
        let dt = now.duration_since(self.last_emit);
        // A jump is any move that couldn't be normal playback since the last emit (+0.75s slack).
        let jumped =
            self.last_pos.is_nan() || (pos - self.last_pos).abs() > dt.as_secs_f64() + 0.75;
        if jumped || dt >= std::time::Duration::from_millis(250) {
            self.last_emit = now;
            self.last_pos = pos;
            return true;
        }
        false
    }
}

fn spawn_event_pump(
    state: Arc<AppState>,
    app: tauri::AppHandle,
    mut events: tokio::sync::mpsc::UnboundedReceiver<PlayerEvent>,
) {
    tauri::async_runtime::spawn(async move {
        let mut throttle = PositionThrottle::new();
        while let Some(ev) = events.recv().await {
            match ev {
                PlayerEvent::Position(p) => {
                    if throttle.should_emit(p, std::time::Instant::now()) {
                        let _ = app.emit("position", serde_json::json!({ "position": p }));
                    }
                    state.on_position(p).await;
                }
                PlayerEvent::Duration(d) => {
                    let _ = app.emit("duration", serde_json::json!({ "duration": d }));
                    state.on_duration(d).await;
                }
                PlayerEvent::Playing(playing) => {
                    let _ = app.emit("playback-state", if playing { "playing" } else { "paused" });
                    if !playing {
                        state.flush_position(); // persist exact resume position on pause
                        let _ = app.emit(
                            "position",
                            serde_json::json!({ "position": state.current_position() }),
                        );
                    }
                    state.media_set_playing(playing);
                    // Keep the tray's toggle label honest â€” this arm is the same chokepoint
                    // MPRIS uses, so tray state can't drift from media-key state.
                    tray::set_playing(&app, playing);
                    state.lt_on_play_state(playing).await; // Listen Together host â†’ broadcast
                }
                PlayerEvent::TrackEnded => {
                    state.on_track_ended().await;
                }
                PlayerEvent::TrackFailed(msg) => {
                    // The track died (dead/403 URL etc). on_track_failed records a WEB_REMIX 403
                    // (context/06 Â§2), evicts the poisoned cache, and retries the track once via
                    // the fallback clients â€” only toast the error if it gave up and advanced.
                    tracing::warn!(error = %msg, "track failed");
                    if !state.on_track_failed().await {
                        let _ = app.emit("playback-error", serde_json::json!({ "message": msg }));
                    }
                }
                PlayerEvent::Error(msg) => {
                    tracing::error!(error = %msg, "player error");
                    let _ = app.emit("playback-error", serde_json::json!({ "message": msg }));
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{close_hides, PositionThrottle};
    use std::time::{Duration, Instant};

    #[test]
    fn close_hides_unless_explicitly_disabled() {
        assert!(close_hides(None)); // fresh install â†’ tray on
        assert!(close_hides(Some("true")));
        assert!(close_hides(Some("garbage")));
        assert!(!close_hides(Some("false")));
    }

    #[test]
    fn steady_playback_throttles_to_250ms() {
        let mut t = PositionThrottle::new();
        let base = Instant::now();
        // First tick ever â†’ emitted regardless of cadence.
        assert!(t.should_emit(0.0, base));
        // 100ms later, small forward move â†’ still within the 250ms window, suppressed.
        assert!(!t.should_emit(0.1, base + Duration::from_millis(100)));
        assert!(!t.should_emit(0.2, base + Duration::from_millis(200)));
        // 250ms accumulated since last emit â†’ emitted again.
        assert!(t.should_emit(0.25, base + Duration::from_millis(250)));
    }

    #[test]
    fn forward_jump_emits_immediately() {
        let mut t = PositionThrottle::new();
        let base = Instant::now();
        assert!(t.should_emit(10.0, base));
        // 50ms later but position jumped +30s (e.g. media-key seek) â†’ emit despite short dt.
        assert!(t.should_emit(40.0, base + Duration::from_millis(50)));
    }

    #[test]
    fn backward_jump_emits_immediately() {
        let mut t = PositionThrottle::new();
        let base = Instant::now();
        assert!(t.should_emit(60.0, base));
        // 50ms later but position jumped -30s â†’ emit despite short dt.
        assert!(t.should_emit(30.0, base + Duration::from_millis(50)));
    }

    #[test]
    fn first_tick_ever_emits() {
        let mut t = PositionThrottle::new();
        // NaN last_pos (fresh throttle) â†’ always emits on the very first tick, even at t=now.
        assert!(t.should_emit(5.0, Instant::now()));
    }
}
