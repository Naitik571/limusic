//! Waveform peaks for the mooziac-style island seekbar. Decodes the track's audio once
//! (downloaded file when present, otherwise the resolved stream URL fetched into memory),
//! reduces it to normalized peak bars, and caches them in SQLite so every later play, seek
//! preview or mini-player mount is a single indexed row read.
//!
//! Decoding runs on a blocking thread (symphonia is synchronous) and never touches the
//! playback path: a missing/slow waveform only means the seekbar falls back to its plain
//! fill until the peaks land.

use std::io::Cursor;
use std::sync::Arc;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::db::Db;
use crate::orchestrator::Orchestrator;
use crate::state::AppState;

/// Bars stored per track; the UI resamples to whatever width it needs.
pub const WAVEFORM_BARS: usize = 120;
/// Never pull more than this for peak computation (a cap, not a target — tracks are ~3–10MB).
const WAVEFORM_MAX_BYTES: u64 = 48_000_000;

/// Peaks (0–255) for `video_id`, `count` bars wide. Cached after the first computation.
pub async fn waveform_peaks(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    orchestrator: &Arc<Orchestrator>,
    video_id: &str,
    count: Option<u32>,
) -> Result<Vec<u8>, String> {
    let count = count.unwrap_or(WAVEFORM_BARS as u32).clamp(24, 240) as usize;
    if let Some((stored, _)) = state.db.get_waveform(video_id) {
        if !stored.is_empty() {
            return Ok(resample(&stored, count));
        }
    }
    let bytes = fetch_audio_bytes(app, state, orchestrator, video_id).await?;
    let video = video_id.to_owned();
    let peaks = tokio::task::spawn_blocking(move || decode_peaks(&bytes, WAVEFORM_BARS))
        .await
        .map_err(|e| format!("waveform task failed: {e}"))??;
    state.db.put_waveform(&video, &peaks, WAVEFORM_BARS as i64);
    Ok(resample(&peaks, count))
}

/// Prefer the downloaded file (no network); otherwise resolve + fetch the stream URL.
async fn fetch_audio_bytes(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    orchestrator: &Arc<Orchestrator>,
    video_id: &str,
) -> Result<Vec<u8>, String> {
    if let Some(path) = state.db.download_path(video_id) {
        if let Ok(bytes) = std::fs::read(&path) {
            if !bytes.is_empty() {
                return Ok(bytes);
            }
        }
    }
    let stream = crate::downloads::resolve_stream(state, orchestrator, video_id)
        .await
        .ok_or_else(|| "couldn't resolve a stream for waveform".to_owned())?;
    let url = crate::downloads::with_ratebypass(&stream.url);
    let mut headers = reqwest::header::HeaderMap::new();
    for (k, v) in &stream.headers {
        if let (Ok(name), Ok(value)) = (
            reqwest::header::HeaderName::from_bytes(k.as_bytes()),
            reqwest::header::HeaderValue::from_bytes(v.as_bytes()),
        ) {
            headers.insert(name, value);
        }
    }
    let resp = crate::downloads::client()
        .get(&url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| format!("waveform fetch failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("waveform HTTP {}", resp.status()));
    }
    // Cap the body: peaks don't need more than the head of a very long mix, and this
    // bounds memory on multi-hour streams.
    let mut bytes = Vec::new();
    use futures::StreamExt;
    let mut body = resp.bytes_stream();
    while let Some(chunk) = body.next().await {
        if (bytes.len() as u64) >= WAVEFORM_MAX_BYTES {
            break;
        }
        let chunk = chunk.map_err(|e| format!("waveform body failed: {e}"))?;
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err("empty audio for waveform".to_owned());
    }
    Ok(bytes)
}

/// Decode `bytes` to mono peak bars (0–255). Undecodable packets are skipped; an error only
/// surfaces when nothing decodable came out at all. Format-agnostic: the probe sniffs the
/// bytes, so m4a/webm/mp3/opus all work without a mime hint.
fn decode_peaks(bytes: &[u8], bars: usize) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(bytes.to_owned());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("waveform probe failed: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != symphonia::core::codecs::CODEC_TYPE_NULL)
        .ok_or_else(|| "no audio track for waveform".to_owned())?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(
            &track.codec_params,
            &symphonia::core::codecs::DecoderOptions::default(),
        )
        .map_err(|e| format!("waveform decoder failed: {e}"))?;

    // Stash one peak per packet, then bucket into bars (no rewind needed).
    let mut packet_peaks: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
        buf.copy_interleaved_ref(decoded);
        packet_peaks.push(buf.samples().iter().fold(0.0f32, |a, &s| a.max(s.abs())));
    }
    if packet_peaks.is_empty() {
        return Err("no decodable audio for waveform".to_owned());
    }
    let per_bar = (packet_peaks.len() as f32 / bars as f32).max(1.0);
    let mut out = vec![0u8; bars];
    for (i, slot) in out.iter_mut().enumerate() {
        let start = (i as f32 * per_bar) as usize;
        let end = (((i + 1) as f32 * per_bar) as usize).max(start + 1);
        let peak = packet_peaks[start.min(packet_peaks.len() - 1)..end.min(packet_peaks.len())]
            .iter()
            .fold(0.0f32, |a, &b| a.max(b));
        // Square-root loudness curve: quiet detail stays visible, loud stays loud.
        *slot = (peak.sqrt().clamp(0.0, 1.0) * 255.0) as u8;
    }
    // Normalize so the loudest bar always hits full height (quiet masters still draw a wave).
    let max = *out.iter().max().unwrap_or(&0);
    if max > 0 {
        for slot in out.iter_mut() {
            *slot = ((*slot as u32 * 255 / max as u32).min(255)) as u8;
        }
    }
    Ok(out)
}

/// Nearest-neighbour resample of stored bars to the requested width.
fn resample(stored: &[u8], count: usize) -> Vec<u8> {
    if stored.len() == count || stored.is_empty() {
        return stored.to_vec();
    }
    (0..count)
        .map(|i| stored[(i * stored.len()) / count])
        .collect()
}
