//! Gamepad support (Xbox / PlayStation / any GilRs-supported pad).
//!
//! Runs a background thread that polls **every** connected gamepad and maps its buttons/axes to
//! Tauri events (`gamepad` with an `action` payload). The frontend (`player.svelte.ts`) handles
//! those events — same parity as keyboard shortcuts and OS media keys.
//! Because this lives in the Rust host, not the webview, it works **even when the app is
//! minimized to tray or as the floating mini-player** (no window focus required).
//!
//! Full keymap (standard GilRs names; Xbox labels in parens):
//!   South (A)          play / pause          -> "playpause"
//!   East  (B)          next track            -> "next"
//!   West  (X)          previous track        -> "prev"
//!   North (Y)          toggle mute           -> "mute"
//!   Select / Back      previous track        -> "prev"
//!   Start              toggle mini-player    -> "togglemini"
//!   Left/Right trigger volume -5/+5          -> "voldown"/"volup"
//!   Left/Right shoulder (LB/RB) seek -10/+10 -> "seekback"/"seekfwd"
//!   DPad Up/Down       volume +/-            -> "volup"/"voldown"
//!   DPad Left/Right    seek -10/+10          -> "seekback"/"seekfwd"
//!   Left Stick X       scrub (hold)          -> continuous "seekback"/"seekfwd"
//!   Left Stick Y       volume scrub (hold)   -> continuous "voldown"/"volup"
//!   Right Stick X      fast scrub (hold)     -> continuous "seekback"/"seekfwd" @ 2× step
//!
//! Enhancements over the first draft:
//! - All gamepads listened to (not just gamepad 0) — useful for couch setups with two pads.
//! - Shoulder + trigger mapped so a single hand can control the app without using the face pad.
//! - Dual-stick scrubbing: horizontal + vertical for seek + volume.
//! - Select mapped so every controller (including JoyCons) has at least one extra button.

use std::time::Duration;

use gilrs::{Axis, Button, EventType, Gilrs};
use tauri::{AppHandle, Emitter};

/// Spawn the gamepad poller. Cheap if no pad is ever connected — `next_event_blocking` idles the
/// thread until input or a 1 s timeout.
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
    // Throttles so a held stick scrubs at ~10 Hz rather than every poll tick.
    let mut last_left_x = std::time::Instant::now();
    let mut last_left_y = std::time::Instant::now();
    let mut last_right_x = std::time::Instant::now();
    loop {
        while let Some(ev) = gilrs.next_event_blocking(Some(Duration::from_millis(200))) {
            let action = match ev.event {
                EventType::ButtonPressed(Button::South, _) => Some("playpause"),
                EventType::ButtonPressed(Button::East, _) => Some("next"),
                EventType::ButtonPressed(Button::West, _) => Some("prev"),
                EventType::ButtonPressed(Button::North, _) => Some("mute"),
                EventType::ButtonPressed(Button::Select, _) => Some("prev"),
                EventType::ButtonPressed(Button::Start, _) => Some("togglemini"),
                EventType::ButtonPressed(Button::LeftTrigger, _) => Some("voldown"),
                EventType::ButtonPressed(Button::RightTrigger, _) => Some("volup"),
                EventType::ButtonPressed(Button::LeftTrigger2, _) => Some("seekback"),
                EventType::ButtonPressed(Button::RightTrigger2, _) => Some("seekfwd"),
                EventType::ButtonPressed(Button::DPadUp, _) => Some("volup"),
                EventType::ButtonPressed(Button::DPadDown, _) => Some("voldown"),
                EventType::ButtonPressed(Button::DPadLeft, _) => Some("seekback"),
                EventType::ButtonPressed(Button::DPadRight, _) => Some("seekfwd"),
                EventType::AxisChanged(Axis::LeftStickX, v, _) => {
                    const DEAD: f32 = 0.35;
                    if v.abs() > DEAD && last_left_x.elapsed() >= Duration::from_millis(100) {
                        last_left_x = std::time::Instant::now();
                        Some(if v > 0.0 { "seekfwd" } else { "seekback" })
                    } else {
                        None
                    }
                }
                EventType::AxisChanged(Axis::LeftStickY, v, _) => {
                    const DEAD: f32 = 0.35;
                    if v.abs() > DEAD && last_left_y.elapsed() >= Duration::from_millis(100) {
                        last_left_y = std::time::Instant::now();
                        // Stick is inverted (up = negative in gilrs)
                        Some(if v < 0.0 { "volup" } else { "voldown" })
                    } else {
                        None
                    }
                }
                EventType::AxisChanged(Axis::RightStickX, v, _) => {
                    const DEAD: f32 = 0.35;
                    if v.abs() > DEAD && last_right_x.elapsed() >= Duration::from_millis(100) {
                        last_right_x = std::time::Instant::now();
                        Some(if v > 0.0 { "seekfwd_fast" } else { "seekback_fast" })
                    } else {
                        None
                    }
                }
                // Triggers on some pads report as axes (0..1)
                EventType::AxisChanged(Axis::LeftZ, v, _) if v > 0.7 => Some("voldown"),
                EventType::AxisChanged(Axis::RightZ, v, _) if v > 0.7 => Some("volup"),
                _ => None,
            };
            if let Some(a) = action {
                let _ = app.emit("gamepad", a);
            }
        }
    }
}
