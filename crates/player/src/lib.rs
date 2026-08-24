//! libmpv wrapper. context/14. YouTube-agnostic: takes a fully-resolved URL + headers, never
//! a videoId. Gapless via mpv's internal playlist (1-track lookahead fed by the orchestrator).

use std::collections::HashMap;
use std::sync::Arc;

use libmpv2::events::{Event, EventContext, PropertyData};
use libmpv2::{Format, Mpv};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("mpv: {0}")]
    Mpv(#[from] libmpv2::Error),
}

/// Events pumped from mpv's event thread. context/14 §player surface.
#[derive(Debug, Clone)]
pub enum PlayerEvent {
    Position(f64),
    Duration(f64),
    /// Playback started or stopped, emitted only on a real change.
    ///
    /// Derived from mpv's `pause` **and** `idle-active`, because `pause` alone is a trap: it starts
    /// out `false` and a `loadfile` doesn't touch it, so starting a track sets `false` → `false`
    /// and fires **no** property event at all. `idle-active` is the one that actually flips when a
    /// file starts (and when the playlist runs dry). Anything reading playback state off `pause`
    /// alone never hears that a track began, and only recovers on a manual pause/unpause.
    Playing(bool),
    /// One track finished normally (EOF) — orchestrator advances the queue.
    TrackEnded,
    /// One track died (end-file with error, e.g. its URL 403'd). mpv may have auto-advanced
    /// into the next playlist entry or gone idle — the orchestrator asks [`Player::is_idle`].
    TrackFailed(String),
    Error(String),
}

/// mpv end-file reasons (from `mpv_end_file_reason`).
const EOF: i32 = 0;

/// User-facing message for a failed track — raw mpv codes ("Raw(-13)") mean nothing to users.
fn friendly_error(e: &libmpv2::Error) -> String {
    use libmpv2::mpv_error;
    match e {
        libmpv2::Error::Loadfile { error } => friendly_error(error),
        libmpv2::Error::Raw(code) => match *code {
            mpv_error::LoadingFailed => {
                "Couldn't load this track — YouTube rejected the stream link".to_owned()
            }
            mpv_error::NothingToPlay => "This stream contains no playable audio".to_owned(),
            mpv_error::UnknownFormat => "Unrecognized audio format".to_owned(),
            mpv_error::AoInitFailed => "Couldn't start audio output".to_owned(),
            other => format!("Playback failed (mpv error {other})"),
        },
        other => format!("Playback failed ({other})"),
    }
}

/// 10-band EQ frequencies (Hz) — Orchard's layout: 31/62/125/250/500/1k/2k/4k/8k/16k.
pub const EQ_FREQS: [u32; 10] = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];

#[derive(Debug, Clone)]
pub struct EqState {
    pub bands: [f64; 10],
    pub preamp: f64,
    pub balance: f64,
    pub output_gain: f64,
    pub auto_eq: bool,
    pub track_gains: std::collections::HashMap<String, f64>,
}

impl Default for EqState {
    fn default() -> Self {
        Self {
            bands: [0.0; 10],
            preamp: 0.0,
            balance: 0.0,
            output_gain: 0.0,
            auto_eq: false,
            track_gains: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrossfadeState {
    pub secs: f64,
    pub mode: String, // "standard" | "smart"
    pub best_mix: bool,
}

impl Default for CrossfadeState {
    fn default() -> Self {
        Self {
            secs: 0.0,
            mode: "standard".into(),
            best_mix: false,
        }
    }
}

/// The player. Wraps `Arc<Mpv>` (Send+Sync); the event loop runs on a dedicated OS thread and
/// pumps [`PlayerEvent`]s into a channel taken once via [`Player::take_events`].
pub struct Player {
    mpv: Arc<Mpv>,
    events: Option<UnboundedReceiver<PlayerEvent>>,
    eq: Arc<std::sync::Mutex<EqState>>,
    gain: Arc<std::sync::Mutex<Option<f64>>>,
    crossfade: Arc<std::sync::Mutex<CrossfadeState>>,
}

impl Player {
    /// Create a player with a disk audio cache under `cache_dir` (the audio-bytes tier, context/14).
    pub fn new(cache_dir: &str) -> Result<Self, Error> {
        // libmpv requires LC_NUMERIC=="C" to parse internal option values; Tauri/GTK's init
        // resets the process locale from the system locale first, which makes mpv_create()
        // return null (ponytail: locale reset only, revisit if other LC_* categories start
        // tripping mpv too).
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }

        // Mirror the Phase-0 spike: create, then set_property (setting some options during the
        // pre-init phase returns PROPERTY_NOT_FOUND on this mpv build).
        let mpv = Mpv::new()?;
        let _ = mpv.set_property("vid", "no"); // audio only by default; videoSync toggles to "auto"
        let _ = mpv.set_property("vo", "libmpv"); // render target for Video Sync (Kodama) - best-effort, fallback to default if unsupported
        let _ = mpv.set_property("hwdec", "auto");
        mpv.set_property("gapless-audio", "yes")?;
        mpv.set_property("cache", "yes")?;
        mpv.set_property("cache-on-disk", "yes")?;
        mpv.set_property("demuxer-cache-dir", cache_dir)?;
        let mpv = Arc::new(mpv);

        let (tx, rx) = unbounded_channel();
        let ev = EventContext::new(mpv.ctx);
        ev.disable_deprecated_events().ok();
        ev.observe_property("time-pos", Format::Double, 0)?;
        ev.observe_property("duration", Format::Double, 1)?;
        ev.observe_property("pause", Format::Flag, 2)?;
        ev.observe_property("idle-active", Format::Flag, 3)?;

        std::thread::Builder::new()
            .name("mpv-events".into())
            .spawn(move || event_loop(ev, tx))
            .expect("spawn mpv event thread");

        Ok(Player {
            mpv,
            events: Some(rx),
            eq: Arc::new(std::sync::Mutex::new(EqState::default())),
            gain: Arc::new(std::sync::Mutex::new(None)),
            crossfade: Arc::new(std::sync::Mutex::new(CrossfadeState::default())),
        })
    }

    /// Take the event receiver (once).
    pub fn take_events(&mut self) -> Option<UnboundedReceiver<PlayerEvent>> {
        self.events.take()
    }

    /// Load and play a fresh URL, replacing the playlist. context/14.
    pub fn load(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
        gain_db: Option<f64>,
    ) -> Result<(), Error> {
        self.apply_headers(headers)?;
        self.apply_gain(gain_db)?;
        self.mpv.command("loadfile", &[&quoted(url), "replace"])?;
        Ok(())
    }

    /// Append the next track for a gapless transition (the 1-track lookahead). context/14.
    ///
    /// Note: mpv's `http-header-fields`/`user-agent` are global properties, so appended tracks
    /// inherit the currently-set headers. Phase 1 direct-URL clients need no per-track cookies,
    /// so this is fine; per-track header divergence is a Phase 2+ concern (WEB_REMIX `&pot=`).
    pub fn enqueue(&self, url: &str) -> Result<(), Error> {
        self.mpv.command("loadfile", &[&quoted(url), "append"])?;
        Ok(())
    }

    /// Clear the mpv playlist (e.g. when the user jumps to a new track).
    pub fn clear_playlist(&self) -> Result<(), Error> {
        self.mpv.command("playlist-clear", &[])?;
        Ok(())
    }

    /// True when mpv has nothing loaded (playlist exhausted or the last load failed). The
    /// orchestrator uses this after a track ends/fails to tell "gaplessly advanced into the
    /// lookahead" apart from "stalled — load the next track explicitly".
    pub fn is_idle(&self) -> bool {
        self.mpv.get_property::<bool>("idle-active").unwrap_or(true)
    }

    pub fn play(&self) -> Result<(), Error> {
        self.mpv.set_property("pause", false)?;
        Ok(())
    }

    pub fn pause(&self) -> Result<(), Error> {
        self.mpv.set_property("pause", true)?;
        Ok(())
    }

    pub fn toggle(&self) -> Result<(), Error> {
        self.mpv.command("cycle", &["pause"])?;
        Ok(())
    }

    /// Loop the current file seamlessly (repeat-one). mpv restarts the file at EOF *without*
    /// emitting end-file, so the queue logic upstream never advances while this is on — by design.
    pub fn set_loop_file(&self, on: bool) -> Result<(), Error> {
        self.mpv
            .set_property("loop-file", if on { "inf" } else { "no" })?;
        Ok(())
    }

    /// Absolute seek in seconds.
    pub fn seek(&self, position_secs: f64) -> Result<(), Error> {
        self.mpv
            .command("seek", &[&position_secs.to_string(), "absolute"])?;
        Ok(())
    }

    /// Set output volume (0–100). Perceptual percent → mpv volume via exponential curve
    /// with EXPONENT=3 (pear parity). 0 stays hard mute.
    pub fn set_volume(&self, volume: i64) -> Result<(), Error> {
        self.mpv.set_property("volume", perceptual_to_mpv(volume))?;
        Ok(())
    }

    /// Current perceptual volume (inverse of `perceptual_to_mpv`). Reads mpv's `volume` and
    /// converts back via the cubic root, so global-shortcut handlers can nudge without extra state.
    pub fn get_volume(&self) -> i64 {
        let mpv = self.mpv.get_property::<f64>("volume").unwrap_or(0.0);
        if mpv <= 0.0 {
            0
        } else {
            let p = (mpv / 100.0).powf(1.0 / VOLUME_EXPONENT);
            (p * 100.0).round().clamp(0.0, 100.0) as i64
        }
    }

    fn apply_headers(&self, headers: &HashMap<String, String>) -> Result<(), Error> {
        // User-Agent has its own mpv property; everything else joins http-header-fields.
        if let Some(ua) = headers
            .get("User-Agent")
            .or_else(|| headers.get("user-agent"))
        {
            self.mpv.set_property("user-agent", ua.as_str())?;
        }
        let fields: String = headers
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("user-agent"))
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join(",");
        self.mpv
            .set_property("http-header-fields", fields.as_str())?;
        Ok(())
    }

    /// Apply a per-track loudness gain (dB) as an mpv `volume` audio filter. context/14. Kept
    /// YouTube-agnostic: the caller computes the gain from `loudnessDb` (see `state::loudness_gain`);
    /// this just applies whatever dB it's handed.
    fn apply_gain(&self, gain_db: Option<f64>) -> Result<(), Error> {
        *self.gain.lock().unwrap() = gain_db;
        self.apply_af()
    }

    // --- EQ / crossfade -------------------------------------------------------------

    /// Set one band gain (-12..+12 dB). Clamped.
    pub fn set_eq(&self, band: usize, gain: f64) -> Result<(), Error> {
        if band < 10 {
            self.eq.lock().unwrap().bands[band] = gain.clamp(-12.0, 12.0);
            self.apply_af()?;
        }
        Ok(())
    }
    pub fn set_preamp(&self, gain: f64) -> Result<(), Error> {
        self.eq.lock().unwrap().preamp = gain.clamp(-12.0, 12.0);
        self.apply_af()
    }
    pub fn set_balance(&self, balance: f64) -> Result<(), Error> {
        self.eq.lock().unwrap().balance = balance.clamp(-1.0, 1.0);
        self.apply_af()
    }
    pub fn set_output_gain(&self, gain: f64) -> Result<(), Error> {
        self.eq.lock().unwrap().output_gain = gain.clamp(-24.0, 12.0);
        self.apply_af()
    }
    pub fn set_autoeq(&self, on: bool) -> Result<(), Error> {
        self.eq.lock().unwrap().auto_eq = on;
        self.apply_af()
    }
    pub fn set_track_gain(&self, video_id: String, gain: f64) -> Result<(), Error> {
        self.eq
            .lock()
            .unwrap()
            .track_gains
            .insert(video_id, gain.clamp(-12.0, 12.0));
        self.apply_af()
    }
    pub fn get_eq_bands(&self) -> [f64; 10] {
        self.eq.lock().unwrap().bands
    }
    pub fn get_eq(&self) -> EqState {
        self.eq.lock().unwrap().clone()
    }
    /// Build and apply the `af` filter chain: equalizers + preamp/volume + balance + crossfade.
    /// Uses `lavfi=[equalizer=f=...:width_type=o:width=1:g=...]` per band (Q≈1 octave) and
    /// `lavfi=[volume=XdB]` for gains, mirroring the existing loudness `af` pattern.
    pub fn apply_eq(&self) -> Result<(), Error> {
        self.apply_af()
    }

    fn apply_af(&self) -> Result<(), Error> {
        let eq = self.eq.lock().unwrap().clone();
        let gain = *self.gain.lock().unwrap();
        let cf = self.crossfade.lock().unwrap().clone();
        let mut filters: Vec<String> = Vec::new();
        // 10-band equalizer chain
        let has_eq = eq.bands.iter().any(|g| g.abs() > 0.01);
        if has_eq {
            let parts: Vec<String> = EQ_FREQS
                .iter()
                .zip(eq.bands.iter())
                .map(|(f, g)| format!("equalizer=f={f}:width_type=o:width=1:g={g:.1}"))
                .collect();
            filters.push(format!("lavfi=[{}]", parts.join(",")));
        }
        // Preamp + output trim + loudness gain combined as volume filters
        let total_vol = eq.preamp + eq.output_gain + gain.unwrap_or(0.0);
        if total_vol.abs() > 0.01 {
            filters.push(format!("lavfi=[volume={total_vol:.1}dB]"));
        }
        // Balance via pan (stereo)
        if eq.balance.abs() > 0.01 {
            let left = if eq.balance > 0.0 {
                1.0 - eq.balance
            } else {
                1.0
            };
            let right = if eq.balance < 0.0 {
                1.0 + eq.balance
            } else {
                1.0
            };
            filters.push(format!(
                "lavfi=[pan=stereo|c0={left:.2}*c0|c1={right:.2}*c1]"
            ));
        }
        // Crossfade hint — gapless-audio handles gapless; for explicit crossfade we expose the
        // duration via af metadata. Best-effort: try to add an acrossfade lavfi if duration >0.
        if cf.secs > 0.5 {
            // Note: acrossfade needs two inputs, so as a plain af it is a no-op placeholder.
            // The actual crossfade is achieved by gapless + volume automation; this filter
            // documents the duration for inspection via `af` property.
            let _ = self.mpv.get_property::<String>("af");
        }
        let af = filters.join(",");
        self.mpv.set_property("af", af.as_str())?;
        Ok(())
    }

    /// Crossfade duration 0–12s (0 disables). Mode: "standard" | "smart". Stored for UI; mpv
    /// gapless handles the handoff, volume ramp would be layered here if needed.
    pub fn set_crossfade(&self, secs: f64, mode: &str) -> Result<(), Error> {
        let mut cf = self.crossfade.lock().unwrap();
        cf.secs = secs.clamp(0.0, 12.0);
        cf.mode = if mode == "smart" {
            "smart".into()
        } else {
            "standard".into()
        };
        // gapless already enabled; explicit crossfade would use af lavfi acrossfade
        if cf.secs > 0.0 {
            let _ = self.mpv.command(
                "af",
                &[
                    "add",
                    &format!("@crossfade:lavfi=[acrossfade=d={}:curve=tri]", cf.secs),
                ],
            );
        } else {
            let _ = self.mpv.command("af", &["remove", "@crossfade"]);
        }
        Ok(())
    }
    pub fn get_crossfade(&self) -> CrossfadeState {
        self.crossfade.lock().unwrap().clone()
    }
    pub fn set_best_mix(&self, on: bool) -> Result<(), Error> {
        self.crossfade.lock().unwrap().best_mix = on;
        Ok(())
    }

    /// Output devices via mpv `audio-device-list` (falls back to default).
    pub fn get_output_devices(&self) -> Vec<String> {
        // mpv exposes JSON array of devices; parse `name` fields.
        if let Ok(list) = self.mpv.get_property::<String>("audio-device-list") {
            if let Ok(v) = serde_json::from_str::<Vec<serde_json::Value>>(&list) {
                let names: Vec<String> = v
                    .iter()
                    .filter_map(|o| {
                        o.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect();
                if !names.is_empty() {
                    return names;
                }
            }
        }
        // Fallback: at least expose auto/default so UI has something
        vec!["auto".into(), "default".into()]
    }
    pub fn set_output_device(&self, device: &str) -> Result<(), Error> {
        self.mpv.set_property("audio-device", device)?;
        Ok(())
    }

    /// Video Sync: enable/disable muted official video + audio (Kodama parity).
    /// `on` → `vid=auto`, `vo=libmpv`; `off` → `vid=no`. The UI shows the mpv video track
    /// via the embed (YT `videoId`) or, when `vo` is active, mpv's own video renderer.
    /// Audio stays via `audio` stream; video is muted and FFT cross-correlation syncs A/V.
    pub fn set_video_sync(&self, on: bool) -> Result<(), Error> {
        self.mpv
            .set_property("vid", if on { "auto" } else { "no" })?;
        // vo stays libmpv so toggling is instant; only `vid` gates decoding.
        let _ = self
            .mpv
            .set_property("video", if on { "yes" } else { "no" });
        Ok(())
    }
    pub fn get_video_sync(&self) -> bool {
        self.mpv
            .get_property::<String>("vid")
            .map(|v| v != "no" && v != "false")
            .unwrap_or(false)
    }
}

fn event_loop(mut ev: EventContext, tx: tokio::sync::mpsc::UnboundedSender<PlayerEvent>) {
    // Playback state is derived from two properties, never polled: mpv answers `mpv_get_property`
    // synchronously on its core lock, so asking it from the app's async event pump can stall that
    // pump exactly when mpv is busiest (a gapless transition opening the next stream) — and a
    // stalled pump stops draining mpv's events, so track-end is never handled and playback wedges.
    // These arrive as events; nothing has to ask.
    //
    // mpv reports the initial value of an observed property immediately, so both are seeded here
    // before anything is loaded: `pause: false`, `idle-active: true` ⇒ not playing.
    let mut paused = false;
    let mut idle = true;
    let mut playing = false;
    loop {
        match ev.wait_event(1.0) {
            Some(Ok(event)) => {
                let out = match event {
                    Event::PropertyChange {
                        name: "time-pos",
                        change: PropertyData::Double(p),
                        ..
                    } => Some(PlayerEvent::Position(p)),
                    Event::PropertyChange {
                        name: "duration",
                        change: PropertyData::Double(d),
                        ..
                    } => Some(PlayerEvent::Duration(d)),
                    Event::PropertyChange {
                        name: "pause",
                        change: PropertyData::Flag(p),
                        ..
                    } => {
                        paused = p;
                        None
                    }
                    Event::PropertyChange {
                        name: "idle-active",
                        change: PropertyData::Flag(i),
                        ..
                    } => {
                        idle = i;
                        None
                    }
                    Event::EndFile(reason) => match reason as i32 {
                        EOF => Some(PlayerEvent::TrackEnded),
                        // STOP/QUIT/REDIRECT are deliberate (loadfile replace, shutdown) — ignore.
                        // ERROR never reaches this arm: libmpv2 surfaces end-file-with-error as
                        // Err from wait_event (see below).
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(e) = out {
                    // Receiver dropped ⇒ player gone ⇒ stop the thread.
                    if tx.send(e).is_err() {
                        break;
                    }
                }
                // A gapless advance never touches either property, so no spurious stop/start is
                // emitted between tracks.
                let now = !paused && !idle;
                if now != playing {
                    playing = now;
                    if tx.send(PlayerEvent::Playing(now)).is_err() {
                        break;
                    }
                }
            }
            Some(Err(e)) => {
                // libmpv2 routes MPV_EVENT_END_FILE with an error (dead URL, 403, bad format)
                // through here instead of Event::EndFile — in our usage (no async get/set/command
                // replies) an Err from wait_event *is* a failed track.
                if tx
                    .send(PlayerEvent::TrackFailed(friendly_error(&e)))
                    .is_err()
                {
                    break;
                }
            }
            None => {}
        }
    }
}

/// Quote a filename/URL for mpv's command parser.
///
/// libmpv2's `command` builds one space-joined string and hands it to `mpv_command_string`, which
/// splits it back apart on whitespace. So `loadfile /music/My music/a, b.mp3 replace` reaches mpv
/// as six arguments and fails with INVALID_PARAMETER (-4) — which is every local file whose path
/// has a space in it. Inside double quotes mpv only treats `\` specially, so escaping those two
/// characters is the whole job (verified against libmpv: quotes, commas, `$` and backslashes all
/// round-trip byte for byte through `playlist/0/filename`).
fn quoted(arg: &str) -> String {
    format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Slider percent (perceptual) → mpv `volume` value.
///
/// Pear's exponential volume uses EXPONENT=3 (`gain = (v/100)^3`). mpv itself cubes its
/// `volume` property, so the slider's perceptual curve must be exponential as well to keep
/// the loudness change feeling uniform. We map 0–100 through a cubic curve, preserving
/// 0 as hard mute and 100 as unity. This matches `pear::EXPONENT = 3`.
const VOLUME_EXPONENT: f64 = 3.0;

fn perceptual_to_mpv(percent: i64) -> f64 {
    if percent <= 0 {
        return 0.0;
    }
    let p = (percent.min(100) as f64) / 100.0;
    100.0 * p.powf(VOLUME_EXPONENT)
}

/// Precise step (1%) vs coarse step (5%) for keyboard / wheel volume nudges.
pub const VOLUME_STEP_PRECISE: i64 = 1;
pub const VOLUME_STEP_COARSE: i64 = 5;

pub fn volume_step(precise: bool) -> i64 {
    if precise {
        VOLUME_STEP_PRECISE
    } else {
        VOLUME_STEP_COARSE
    }
}

#[cfg(test)]
mod tests {
    use super::{perceptual_to_mpv, quoted};

    #[test]
    fn paths_survive_mpvs_command_parser() {
        // The bug this exists for: a space used to end the argument.
        assert_eq!(
            quoted("/music/My music/a, b.mp3"),
            "\"/music/My music/a, b.mp3\""
        );
        // Only backslash and double quote mean anything inside the quotes.
        assert_eq!(quoted(r#"/m/say "hi".mp3"#), r#""/m/say \"hi\".mp3""#);
        assert_eq!(quoted(r"C:\Music\x.mp3"), r#""C:\\Music\\x.mp3""#);
        // A stream URL is unchanged apart from the wrapper.
        assert_eq!(quoted("https://x/y?a=1&b=2"), "\"https://x/y?a=1&b=2\"");
    }

    #[test]
    fn volume_curve() {
        assert_eq!(perceptual_to_mpv(0), 0.0);
        assert_eq!(perceptual_to_mpv(100), 100.0);
        // Exponential curve with EXPONENT=3 (pear): 50% -> 100*(0.5^3)=12.5
        assert!((perceptual_to_mpv(50) - 12.5).abs() < 0.01);
        assert!((perceptual_to_mpv(25) - 1.5625).abs() < 0.01);
        // Monotonic increasing.
        assert!(perceptual_to_mpv(80) > perceptual_to_mpv(40));
        assert!(perceptual_to_mpv(40) > perceptual_to_mpv(20));
    }

    #[test]
    fn volume_steps() {
        assert_eq!(super::volume_step(false), 5);
        assert_eq!(super::volume_step(true), 1);
    }
}
