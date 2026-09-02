//! System tray: app icon + menu (Show / Play-Pause / Next / Previous / Restart / Quit). Menu actions
//! route into the same [`AppState`] methods the OS media keys use (see media.rs), so the tray
//! can never behave differently from SMTC.
//!
//! Built on Tauri's `tray-icon`: menu on right-click, double-click restores the window.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

pub use imp::{init, set_playing};

/// Bring the main window back from close-to-tray, minimize, or the mini player. Every "come back"
/// path — tray menu, tray click, second launch, the widget's restore button — goes through here so
/// they can't drift apart.
pub fn show_main(app: &AppHandle) {
    crate::mini::close(app);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        set_main_visible(app, true);
    }
}

/// Tell the main window's SPA whether anyone can see it.
///
/// WebKitGTK does not pass a GTK hide down to the page: `document.visibilityState` stays
/// "visible" for a window that is closed to the tray, so the SPA cannot work this out on its own
/// and keeps restyling for nobody. That is worse than wasted work. While the window is unmapped
/// the web process never gives any of it back: measured over 20 track changes with the window
/// hidden, it grew 137 MB and held it, then dropped the whole lot within two seconds of the window
/// being shown again. A night in the tray is thousands of those. See `theme.svelte.ts`.
pub fn set_main_visible(app: &AppHandle, visible: bool) {
    let _ = app.emit_to("main", "ui-visible", visible);
}

/// Shared by both backends: menu ids are the contract between them.
fn handle_menu(app: &AppHandle, id: &str) {
    match id {
        "show" => show_main(app),
        "quit" | "restart" => {
            // Users now quit mid-song from the tray; persist the exact resume position first.
            if let Some(state) = app.try_state::<Arc<AppState>>() {
                state.flush_position();
            }
            // Same for the widget's own position, if that's what they were quitting from.
            crate::mini::save_position(app);
            if id == "restart" {
                // `request_restart`, not `restart`: it goes through RunEvent::Exit, which is
                // where the single-instance plugin releases the D-Bus name. Skip that and the
                // relaunched process hands off to the still-dying one and exits, leaving no app.
                //
                // ponytail: under `cargo tauri dev` this leaves you with no window. The CLI
                // exits when its child does, taking the vite server with it, so the relaunched
                // binary navigates to a dead devUrl and the transparent frameless window paints
                // nothing (taskbar + tray entry, no visible window). Release builds embed the
                // frontend, so restart is only usable there.
                app.request_restart();
            } else {
                app.exit(0);
            }
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
        let restart = MenuItem::with_id(app, "restart", "Restart", true, None::<&str>)?;
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
                &restart,
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
