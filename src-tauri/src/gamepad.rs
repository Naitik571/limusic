//! Gamepad support (Xbox / PlayStation / any GilRs-supported pad).
//!
//! Runs a background thread that polls **every** connected gamepad and maps its buttons/axes to
//! Tauri events (`gamepad` with an `action` payload). The frontend (`player.svelte.ts`) handles
//! those events — same parity as keyboard shortcuts and OS media keys.
//! Because this lives in the Rust host, not the webview, it works **even when the app is
//! minimized to tray or as the floating mini-player** (no window focus required).
//!
//! Events go to the **main window only** (`emit_to("main")`). Both the main and mini windows run
//! the same frontend module with its own listener, so a broadcast here used to execute every
//! action once per live webview — with the widget open (exactly when a controller is in use)
//! play/pause and mute fired twice and cancelled out, and next skipped two tracks. The main
//! webview is only ever hidden (to tray), never destroyed while the app runs, so addressing it
//! keeps tray/mini playback control working with exactly one consumer.
//!
//! Full keymap (standard GilRs names; Xbox labels in parens):
//!   South (A)          play / pause          -> "playpause"
//!   East  (B)          next track            -> "next"
//!   West  (X)          previous track        -> "prev"
//!   North (Y)          toggle mute           -> "mute"
//!   Select / Back      previous track        -> "prev"
//!   Start              toggle mini-player    -> "togglemini"
//!   LB / RB shoulder   volume -5/+5          -> "voldown"/"volup"
//!   LT / RT triggers   volume -5/+5 (hold = repeat; analog axis on most pads)
//!   DPad Up/Down       volume +/-            -> "volup"/"voldown"
//!   DPad Left/Right    seek -10/+10          -> "seekback"/"seekfwd"
//!   Left Stick X       scrub (hold)          -> continuous "seekback"/"seekfwd"
//!   Left Stick Y       volume scrub (hold)   -> continuous "voldown"/"volup"
//!   Right Stick X      fast scrub (hold)     -> continuous "seekback"/"seekfwd" @ 2× step
//!
//! Stability rules (all of them exist because real pads misbehave in ways that looked like app
//! bugs):
//! - Sticks only act **while |value| > DEADZONE** (0.35). A release that snaps back through the
//!   deadzone therefore emits nothing on the way home.
//! - Hold-repeat actions are throttled to one event per REPEAT_EVERY, per input — an analog
//!   trigger or stick held past its threshold streams readings at ~125 Hz on Windows (WGI polls
//!   every 8 ms), which would otherwise flood the frontend.
//! - A connect/disconnect (pad waking from idle, Bluetooth reconnect — which is exactly what an
//!   alt-tab looks like to a wireless pad) resets the repeat clocks and swallows everything for
//!   CONNECT_SETTLE, dropping the burst of stale readings WGI replays on wake.

use std::time::Duration;

use gilrs::{Axis, Button, EventType, Gilrs};
use tauri::{AppHandle, Emitter};

/// Window the actions are delivered to (see module docs for why not a broadcast).
const TARGET: &str = "main";

/// Stick values beyond this count as intentional (applied to both axes of both sticks).
const DEADZONE: f32 = 0.35;
/// Analog triggers fire at this pull; below it they count as released.
const TRIGGER_THRESHOLD: f32 = 0.7;
/// Minimum gap between two emitted actions from the same held stick/trigger (~10 Hz repeat).
const REPEAT_EVERY: Duration = Duration::from_millis(100);
/// How long after a (dis)connect all events stay dropped while the pad's reading stream settles.
const CONNECT_SETTLE: Duration = Duration::from_millis(500);

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
    // Last emission per hold-repeat input, so a held stick scrubs at ~10 Hz rather than every poll
    // tick. Reset on (dis)connect together with the settle window.
    let mut last_left_x = std::time::Instant::now();
    let mut last_left_y = std::time::Instant::now();
    let mut last_right_x = std::time::Instant::now();
    let mut last_left_trigger = std::time::Instant::now();
    let mut last_right_trigger = std::time::Instant::now();
    // While `now < settle_until`, every event is dropped (see CONNECT_SETTLE).
    let mut settle_until = std::time::Instant::now();
    loop {
        while let Some(ev) = gilrs.next_event_blocking(Some(Duration::from_millis(200))) {
            let now = std::time::Instant::now();
            // A pad waking up replays its whole state as a burst of events — volume jumps and
            // phantom seeks if taken at face value. Drop them, and start the repeat clocks from
            // now so whatever position the stick rests in can't emit immediately afterwards.
            if matches!(ev.event, EventType::Connected | EventType::Disconnected) {
                settle_until = now + CONNECT_SETTLE;
                last_left_x = now;
                last_left_y = now;
                last_right_x = now;
                last_left_trigger = now;
                last_right_trigger = now;
                continue;
            }
            if now < settle_until {
                continue;
            }
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
                    if v.abs() > DEADZONE && now.duration_since(last_left_x) >= REPEAT_EVERY {
                        last_left_x = now;
                        Some(if v > 0.0 { "seekfwd" } else { "seekback" })
                    } else {
                        None
                    }
                }
                EventType::AxisChanged(Axis::LeftStickY, v, _) => {
                    if v.abs() > DEADZONE && now.duration_since(last_left_y) >= REPEAT_EVERY {
                        last_left_y = now;
                        // Stick is inverted (up = negative in gilrs)
                        Some(if v < 0.0 { "volup" } else { "voldown" })
                    } else {
                        None
                    }
                }
                EventType::AxisChanged(Axis::RightStickX, v, _) => {
                    if v.abs() > DEADZONE && now.duration_since(last_right_x) >= REPEAT_EVERY {
                        last_right_x = now;
                        Some(if v > 0.0 {
                            "seekfwd_fast"
                        } else {
                            "seekback_fast"
                        })
                    } else {
                        None
                    }
                }
                // Triggers on some pads report as axes (0..1) instead of buttons — same mapping as
                // the button arms above, throttled like the sticks so a full pull repeats at ~10 Hz
                // instead of once per WGI poll.
                EventType::AxisChanged(Axis::LeftZ, v, _) => {
                    if v > TRIGGER_THRESHOLD
                        && now.duration_since(last_left_trigger) >= REPEAT_EVERY
                    {
                        last_left_trigger = now;
                        Some("voldown")
                    } else {
                        None
                    }
                }
                EventType::AxisChanged(Axis::RightZ, v, _) => {
                    if v > TRIGGER_THRESHOLD
                        && now.duration_since(last_right_trigger) >= REPEAT_EVERY
                    {
                        last_right_trigger = now;
                        Some("volup")
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(a) = action {
                let _ = app.emit_to(TARGET, "gamepad", a);
            }
        }
    }
}
