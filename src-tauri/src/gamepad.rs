//! Gamepad support (Xbox / PlayStation / any GilRs-supported pad).
//!
//! Runs a background thread that polls the first connected gamepad and maps its buttons/axes to
//! Tauri events (`gamepad` with an `action` payload). The frontend (`player.svelte.ts`) handles
//! those events by calling the same playback actions the keyboard shortcuts and OS media keys use.
//! Because this lives in the Rust host — not the webview — it works **even when the app is
//! minimized to the tray or running only as the floating mini-player** (no window focus required).
//!
//! Keymap (standard GilRs button names; Xbox labelled in parens):
//!   South (A)        play / pause        -> "playpause"
//!   East  (B)        next track          -> "next"
//!   West  (X)        previous track      -> "prev"
//!   North (Y)        toggle mute         -> "mute"
//!   DPad Up/Down     volume +5 / −5      -> "volup" / "voldown"
//!   DPad Left/Right  seek −10s / +10s    -> "seekback" / "seekfwd"
//!   Left Stick X     scrub (hold)        -> continuous "seekback"/"seekfwd"
//!   Start            toggle mini-player  -> "togglemini"
//!
//! Buttons are edge-triggered (fire once per press); the left stick scrubs while held.

use std::time::Duration;

use gilrs::{Axis, Button, EventType, Gilrs};
use tauri::{AppHandle, Emitter};

/// Spawn the gamepad poller. Cheap if no pad is ever connected — `next_event_blocking` idles the
/// thread until input or a 1s timeout, so there's no busy loop.
pub fn start(app: AppHandle) {
    std::thread::spawn(move || run(app));
}

fn run(app: AppHandle) {
    let mut gilrs = match Gilrs::new() {
        Ok(g) => g,
        Err(e) => {
            tracing::warn!("gamepad init failed (no controller support): {e}");
            return;
        }
    };
    tracing::info!("gamepad poller started");
    // Throttle so a held stick scrubs at a steady ~10 Hz rather than every poll tick.
    let mut last_stick = std::time::Instant::now();
    loop {
        while let Some(ev) = gilrs.next_event_blocking(Some(Duration::from_millis(200))) {
            let action = match ev.event {
                EventType::ButtonPressed(Button::South, _) => Some("playpause"),
                EventType::ButtonPressed(Button::East, _) => Some("next"),
                EventType::ButtonPressed(Button::West, _) => Some("prev"),
                EventType::ButtonPressed(Button::North, _) => Some("mute"),
                EventType::ButtonPressed(Button::DPadUp, _) => Some("volup"),
                EventType::ButtonPressed(Button::DPadDown, _) => Some("voldown"),
                EventType::ButtonPressed(Button::DPadLeft, _) => Some("seekback"),
                EventType::ButtonPressed(Button::DPadRight, _) => Some("seekfwd"),
                EventType::ButtonPressed(Button::Start, _) => Some("togglemini"),
                EventType::AxisChanged(Axis::LeftStickX, value, _) => {
                    const DEAD: f32 = 0.35;
                    if value.abs() > DEAD && last_stick.elapsed() >= Duration::from_millis(100) {
                        last_stick = std::time::Instant::now();
                        Some(if value > 0.0 { "seekfwd" } else { "seekback" })
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(a) = action {
                let _ = app.emit("gamepad", a);
            }
        }
    }
}
