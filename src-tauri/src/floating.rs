//! Floating pop-out player (2026-08 Aurora round): a small always-on-top now-playing card.
//!
//! Same SPA-in-a-second-window trick as the mini player ([`crate::mini`]) — the root layout
//! branches on the window label, and every event this app emits is global (`app.emit`), so both
//! webviews are driven by the same playback stream with zero per-window sync.
//!
//! Unlike the mini player, the floating player *coexists* with the main window: it's a
//! permanent-ish companion (position persisted across sessions, bottom-right first time) that
//! the user pops out from the player bar and dismisses with its close button. Nothing hides
//! when it opens.

use std::sync::Arc;

use tauri::{
    AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};

use crate::state::AppState;

pub const LABEL: &str = "floating";

/// Logical size of the card. Portrait: art up top, transport below — wide enough for a title
/// that truncates gracefully, tall enough to not feel cramped next to a maximized player.
const W: f64 = 360.0;
const H: f64 = 560.0;
/// Inset from the screen edge the first time it opens.
const MARGIN: f64 = 24.0;
/// Where the user last dragged it, as physical `"x,y"`. Physical because monitor geometry is,
/// and two displays can disagree on scale factor.
const POS_KEY: &str = "floating_position";

/// Build (or re-show) the card. **Main thread only** — GTK wants window creation there, same
/// rule as `mini::open`; `commands::toggle_floating` does the hop.
pub fn open(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Limusic")
        .inner_size(W, H)
        .resizable(true)
        .min_inner_size(300.0, 480.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        // A square WM shadow around a rounded transparent window looks broken.
        .shadow(false)
        // Positioned before it is shown, so it never flashes wherever the WM guessed first.
        .visible(false)
        .build()
        .map_err(|e| format!("couldn't open the floating player: {e}"))?;

    if let Some(p) = placement(app, &win) {
        let _ = win.set_position(p);
    }
    let _ = win.show();
    let _ = win.set_focus();
    Ok(())
}

/// Tear the card down, remembering where it ended up. Callable from any thread.
pub fn close(app: &AppHandle) {
    if app.get_webview_window(LABEL).is_none() {
        return;
    }
    save_position(app);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = handle.get_webview_window(LABEL) {
            let _ = w.destroy();
        }
    });
}

/// Open when closed, close when open. This is the player-bar button.
pub fn toggle(app: &AppHandle) -> Result<(), String> {
    if app.get_webview_window(LABEL).is_some() {
        close(app);
        Ok(())
    } else {
        open(app)
    }
}

/// Remember where the card currently sits. No-op when it isn't up.
pub fn save_position(app: &AppHandle) {
    let Some(w) = app.get_webview_window(LABEL) else { return };
    if let (Ok(p), Some(state)) = (w.outer_position(), app.try_state::<Arc<AppState>>()) {
        state.db.set_setting(POS_KEY, &format!("{},{}\n", p.x, p.y).trim_end());
    }
}

/// Where to put it: the last position if that spot still exists (a display can be unplugged
/// between sessions), otherwise the bottom-right of whichever display the app is on.
fn placement(app: &AppHandle, win: &WebviewWindow) -> Option<PhysicalPosition<i32>> {
    app.try_state::<Arc<AppState>>()
        .and_then(|s| s.db.get_setting(POS_KEY))
        .and_then(|v| parse_pos(&v))
        .filter(|p| on_a_display(win, *p))
        .or_else(|| bottom_right(win))
}

/// `"x,y"` in physical pixels, as [`save_position`] wrote it.
fn parse_pos(s: &str) -> Option<PhysicalPosition<i32>> {
    let (x, y) = s.split_once(',')?;
    Some(PhysicalPosition::new(
        x.trim().parse().ok()?,
        y.trim().parse().ok()?,
    ))
}

/// Is that point on a display that is currently connected? Checked on the top-left corner,
/// which is what `set_position` sets and what the WM keeps reachable.
fn on_a_display(win: &WebviewWindow, p: PhysicalPosition<i32>) -> bool {
    win.available_monitors()
        .is_ok_and(|monitors| monitors.iter().any(|m| contains(*m.position(), *m.size(), p)))
}

/// Bottom-right of the primary display, inset by [`MARGIN`].
fn bottom_right(win: &WebviewWindow) -> Option<PhysicalPosition<i32>> {
    let mon = win.primary_monitor().ok().flatten()?;
    let size = mon.size();
    let origin = mon.position();
    let (mw, mh) = (size.width as i32, size.height as i32);
    let (ww, wh) = (W as i32, H as i32);
    Some(PhysicalPosition::new(
        origin.x + mw - ww - MARGIN as i32,
        origin.y + mh - wh - MARGIN as i32,
    ))
}

fn contains(origin: PhysicalPosition<i32>, size: PhysicalSize<u32>, p: PhysicalPosition<i32>) -> bool {
    p.x >= origin.x
        && p.y >= origin.y
        && p.x < origin.x + size.width as i32
        && p.y < origin.y + size.height as i32
}
