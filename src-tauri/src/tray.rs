//! System tray: app icon + menu (Show / Play-Pause / Next / Previous / Quit). Menu actions
//! route into the same [`AppState`] methods the OS media keys use (see media.rs), so the tray
//! can never behave differently from SMTC.
//!
//! Built on Tauri's `tray-icon`: menu on right-click, double-click restores the window.

use std::sync::Arc;

use tauri::{AppHandle, Manager};

use crate::state::AppState;

pub use imp::{init, set_playing};

/// Swap the tray's icon at runtime (custom app icon, Settings ▸ General). The builder registers
/// the tray as `"main"`, so `tray_by_id` hands back a handle onto the live icon. Best-effort:
/// a failure is logged, never fatal.
pub fn set_icon(app: &AppHandle, icon: tauri::image::Image<'_>) {
    let Some(tray) = app.tray_by_id("main") else {
        tracing::warn!("tray not initialized, custom icon skipped");
        return;
    };
    if let Err(e) = tray.set_icon(Some(icon)) {
        tracing::warn!(error = %e, "tray icon update failed");
    }
}

/// Bring the main window back from close-to-tray, minimize, or the mini player. Every "come back"
/// path â€” tray menu, tray click, second launch, the widget's restore button â€” goes through here so
/// they can't drift apart.
pub fn show_main(app: &AppHandle) {
    crate::mini::close(app);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Shared by both backends: menu ids are the contract between them.
fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "show" => show_main(app),
        "quit" => {
            // Users now quit mid-song from the tray; persist the exact resume position first.
            if let Some(state) = app.try_state::<Arc<AppState>>() {
                state.flush_position();
            }
            // Same for the widgets' own positions, if that's what they were quitting from.
            crate::mini::save_position(app);
            app.exit(0);
        }
        other => {
            let Some(state) = app.try_state::<Arc<AppState>>() else {
                return;
            };
            let state = state.inner().clone();
            let id = other.to_string();
            tauri::async_runtime::spawn(async move {
                match id.as_str() {
                    "play_pause" => state.resume_or_toggle().await,
                    "next" => state.next_in_queue().await,
                    "prev" => state.prev_in_queue().await,
                    _ => {}
                }
            });
        }
    }
}

mod imp {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
    use tauri::{AppHandle, Manager, Wry};

    use super::{handle_menu, show_main};

    /// Managed handle to the live-label item so the mpv event pump can flip "Play"/"Pause".
    struct TrayState {
        play_pause: MenuItem<Wry>,
    }

    pub fn init(app: &AppHandle) -> tauri::Result<()> {
        let show = MenuItem::with_id(app, "show", "Show Limusic", true, None::<&str>)?;
        let play_pause = MenuItem::with_id(app, "play_pause", "Play", true, None::<&str>)?;
        let next = MenuItem::with_id(app, "next", "Next", true, None::<&str>)?;
        let prev = MenuItem::with_id(app, "prev", "Previous", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &show,
                &PredefinedMenuItem::separator(app)?,
                &play_pause,
                &next,
                &prev,
                &PredefinedMenuItem::separator(app)?,
                &quit,
            ],
        )?;

        // Menu on right-click only, so a left double-click can't also pop it open behind the
        // window it just restored.
        let mut builder = TrayIconBuilder::with_id("main")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .tooltip("Limusic")
            .on_menu_event(|app, event| handle_menu(app, event.id.as_ref()))
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } = event
                {
                    show_main(tray.app_handle());
                }
            });
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        builder.build(app)?;

        app.manage(TrayState { play_pause });
        Ok(())
    }

    pub fn set_playing(app: &AppHandle, playing: bool) {
        if let Some(t) = app.try_state::<TrayState>() {
            let _ = t
                .play_pause
                .set_text(if playing { "Pause" } else { "Play" });
        }
    }
}
