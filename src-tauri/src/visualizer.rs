//! Real-time spectrum from our own process audio (WASAPI application loopback).
//!
//! Why loopback-of-self instead of tapping mpv: libmpv has no audio-tap API, and system-wide
//! loopback would leak other apps' audio (and need device juggling). Capturing our own process
//! gets exactly what the user hears — mpv plays in-process — with no extra decodes, and silence
//! (paused/idle) naturally flatlines through the same decay path.
//!
//! Pipeline: loopback capture (f32 stereo 44.1k, autoconverted) → mono 2048-sample windows →
//! realfft → 24 log-spaced bands (60 Hz–16 kHz) → dB map + peak-decay smoothing →
//! `visualizer-frame` event at ~21 Hz. A std thread (like the sleep timer), never the UI.
//!
//! Cost: one 2048 FFT per ~46 ms + 24 floats per event — negligible. When nothing plays the
//! magnitudes are ~0 and the decay settles the bars without any special-casing.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use realfft::{num_complex::Complex, RealFftPlanner};
use tauri::Emitter;

const FFT_LEN: usize = 2048;
const BANDS: usize = 24;
const BAND_LO_HZ: f32 = 60.0;
const BAND_HI_HZ: f32 = 16000.0;
const SAMPLE_RATE: f32 = 44100.0;
/// Per-emission decay: bars fall to ~10% in about half a second of silence.
const DECAY: f32 = 0.88;

pub struct Visualizer {
    enabled: AtomicBool,
    running: AtomicBool,
}

impl Visualizer {
    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            running: AtomicBool::new(false),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// Flip the switch; spawns the capture thread once. Disabling stops events (the UI
    /// decays its last frame to flat on its own).
    pub fn set_enabled(self: &Arc<Self>, app: &tauri::AppHandle, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
        if on && !self.running.swap(true, Ordering::SeqCst) {
            let me = self.clone();
            let app = app.clone();
            std::thread::spawn(move || run_loop(&me, &app));
        }
    }
}

/// Log-spaced band edges in FFT bins (bin_hz = SAMPLE_RATE / FFT_LEN).
fn band_bins() -> Vec<(usize, usize)> {
    let bin_hz = SAMPLE_RATE / FFT_LEN as f32;
    let mut out = Vec::with_capacity(BANDS);
    for k in 0..BANDS {
        let lo = BAND_LO_HZ * (BAND_HI_HZ / BAND_LO_HZ).powf(k as f32 / BANDS as f32);
        let hi = BAND_LO_HZ * (BAND_HI_HZ / BAND_LO_HZ).powf((k + 1) as f32 / BANDS as f32);
        let a = (lo / bin_hz).floor().max(1.0) as usize;
        let b = (hi / bin_hz).ceil().max(a as f32 + 1.0) as usize;
        out.push((a, b.min(FFT_LEN / 2)));
    }
    out
}

fn run_loop(me: &Arc<Visualizer>, app: &tauri::AppHandle) {
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_LEN);
    let mut scratch_in = fft.make_input_vec();
    let mut scratch_out = fft.make_output_vec();
    let bins = band_bins();
    let mut smooth = vec![0.0f32; BANDS];

    loop {
        if !me.enabled.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }
        if let Err(e) = capture_session(
            me,
            app,
            fft.clone(),
            &mut scratch_in,
            &mut scratch_out,
            &bins,
            &mut smooth,
        ) {
            tracing::debug!(error = %e, "visualizer capture failed — retrying");
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }
}

fn capture_session(
    me: &Arc<Visualizer>,
    app: &tauri::AppHandle,
    fft: std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    scratch_in: &mut [f32],
    scratch_out: &mut [Complex<f32>],
    bins: &[(usize, usize)],
    smooth: &mut [f32],
) -> Result<(), String> {
    use wasapi::{AudioClient, Direction, SampleType, StreamMode, WaveFormat};

    wasapi::initialize_mta()
        .ok()
        .map_err(|e| format!("COM init: {e:?}"))?;
    let pid = std::process::id();
    let mut audio_client = AudioClient::new_application_loopback_client(pid, true)
        .map_err(|e| format!("loopback: {e:?}"))?;

    // Explicit format (app-loopback has no mix format to query): f32 stereo 44.1k, converted.
    let format = WaveFormat::new(32, 32, &SampleType::Float, 44100, 2, None);
    let (_def, min_period) = audio_client
        .get_device_period()
        .map_err(|e| format!("period: {e:?}"))?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: min_period,
    };
    audio_client
        .initialize_client(&format, &Direction::Capture, &mode)
        .map_err(|e| format!("init: {e:?}"))?;

    let h_event = audio_client
        .set_get_eventhandle()
        .map_err(|e| format!("event: {e:?}"))?;
    let capture = audio_client
        .get_audiocaptureclient()
        .map_err(|e| format!("capture client: {e:?}"))?;
    audio_client
        .start_stream()
        .map_err(|e| format!("start: {e:?}"))?;

    let mut bytes: VecDeque<u8> = VecDeque::with_capacity(FFT_LEN * 8 * 4);
    let mut mono: Vec<f32> = Vec::with_capacity(FFT_LEN);
    // Drop COM cleanly on the way out (any return path).
    let _deinit = DeinitGuard;

    loop {
        if !me.enabled.load(Ordering::Relaxed) {
            let _ = audio_client.stop_stream();
            return Ok(());
        }
        capture
            .read_from_device_to_deque(&mut bytes)
            .map_err(|e| format!("read: {e:?}"))?;
        if h_event.wait_for_event(1000).is_err() {
            continue;
        }
        // Drain whole f32 stereo frames → mono. 8 bytes per frame.
        while bytes.len() >= 8 && mono.len() < FFT_LEN {
            let mut l = [0u8; 4];
            let mut r = [0u8; 4];
            for b in l.iter_mut() {
                *b = bytes.pop_front().unwrap_or(0);
            }
            for b in r.iter_mut() {
                *b = bytes.pop_front().unwrap_or(0);
            }
            mono.push((f32::from_le_bytes(l) + f32::from_le_bytes(r)) * 0.5);
        }
        // Bound the deque against bursts (drop oldest whole frames).
        while bytes.len() > FFT_LEN * 8 * 4 {
            for _ in 0..8 {
                bytes.pop_front();
            }
        }
        if mono.len() < FFT_LEN {
            continue;
        }
        let frame: Vec<f32> = mono.drain(..FFT_LEN).collect();
        emit_bands(app, &fft, scratch_in, scratch_out, bins, smooth, &frame);
    }
}

struct DeinitGuard;
impl Drop for DeinitGuard {
    fn drop(&mut self) {
        wasapi::deinitialize();
    }
}

fn emit_bands(
    app: &tauri::AppHandle,
    fft: &std::sync::Arc<dyn realfft::RealToComplex<f32>>,
    scratch_in: &mut [f32],
    scratch_out: &mut [Complex<f32>],
    bins: &[(usize, usize)],
    smooth: &mut [f32],
    frame: &[f32],
) {
    scratch_in.copy_from_slice(frame);
    if fft.process(scratch_in, scratch_out).is_err() {
        return;
    }
    let scale = FFT_LEN as f32;
    for (k, (a, b)) in bins.iter().enumerate() {
        let mut sum = 0.0f32;
        let mut n = 0u32;
        for c in &scratch_out[*a..(*b).min(scratch_out.len())] {
            sum += (c.re * c.re + c.im * c.im).sqrt() / scale;
            n += 1;
        }
        let mag = if n > 0 { sum / n as f32 } else { 0.0 };
        // -80 dB floor → 0, full-scale sine (~-6 dB) → ~0.93.
        let v = ((20.0 * (mag + 1e-5).log10()) + 80.0) / 80.0;
        let v = v.clamp(0.0, 1.0);
        smooth[k] = v.max(smooth[k] * DECAY);
    }
    let _ = app.emit("visualizer-frame", &smooth.to_vec());
}
