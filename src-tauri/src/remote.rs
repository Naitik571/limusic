//! Remote LAN control via HTTP on 0.0.0.0:32145 (Orchard/kodama parity).
//! QR pairs a phone on the same Wi-Fi: `http://<lan-ip>:32145?token=<remote_token>`.
//! The token is 18 random bytes, base64url (no pad), stored in Db (`remote_token`).

use std::sync::Arc;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::db::Db;
use crate::state::AppState;

/// The phone-side remote control page served on `GET /?token=…`. Inline CSS/JS, no dependencies.
const REMOTE_HTML: &str = include_str!("../remote.html");

/// Generate or load the pairing token (18B -> 24 char base64url).
pub fn get_or_create_token(db: &Db) -> String {
    if let Some(t) = db.get_setting("remote_token") {
        if !t.is_empty() {
            return t;
        }
    }
    let mut bytes = [0u8; 18];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);
    db.set_setting("remote_token", &token);
    token
}

/// LAN IP via UDP trick (no external crate). Falls back to 127.0.0.1.
pub fn lan_ip() -> String {
    // Connect a UDP socket to a public IP (no packets sent) and read its local addr.
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        let _ = sock.connect("8.8.8.8:80");
        if let Ok(addr) = sock.local_addr() {
            let ip = addr.ip().to_string();
            if ip != "0.0.0.0" && !ip.starts_with("127.") {
                return ip;
            }
        }
    }
    // Try hostname lookup fallback: enumerate via std (last resort).
    "127.0.0.1".to_string()
}

pub fn lan_url(db: &Db) -> String {
    format!(
        "http://{}:32145?token={}",
        lan_ip(),
        get_or_create_token(db)
    )
}

/// Validate token from query or header.
fn extract_token(request: &str) -> Option<String> {
    // Look for token= in request line.
    for part in request.split_whitespace() {
        if let Some(idx) = part.find("token=") {
            let rest = &part[idx + 6..];
            let end = rest
                .find(|c| c == '&' || c == ' ' || c == '"' || c == '\'')
                .unwrap_or(rest.len());
            let tok = rest[..end].to_string();
            if !tok.is_empty() {
                return Some(tok);
            }
        }
    }
    // header X-Remote-Token
    for line in request.lines() {
        if line.to_ascii_lowercase().starts_with("x-remote-token:") {
            if let Some(v) = line.splitn(2, ':').nth(1) {
                let t = v.trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
    }
    None
}

fn is_authorized(request: &str, expected: &str) -> bool {
    // If no token supplied and expected exists, allow local loop? No -> require token.
    // For LAN, token is required except for /pair probe.
    if request.contains("GET /pair") || request.contains("POST /pair") {
        return true;
    }
    if let Some(tok) = extract_token(request) {
        return tok == expected;
    }
    false
}

/// Spawn the tiny HTTP server on 0.0.0.0:32145. Best-effort, logs on bind failure.
pub fn spawn(state: Arc<AppState>) {
    let db = state.db.clone();
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind("0.0.0.0:32145").await {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!(error=%e, "remote LAN server bind failed (port 32145)");
                return;
            }
        };
        tracing::info!("remote LAN server listening on 0.0.0.0:32145");
        loop {
            let (mut stream, _addr) = match listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error=%e, "remote accept failed");
                    continue;
                }
            };
            let state = state.clone();
            let db = db.clone();
            tauri::async_runtime::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = match stream.read(&mut buf).await {
                    Ok(n) if n > 0 => n,
                    _ => return,
                };
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let expected = get_or_create_token(&db);
                // Simple CORS
                let cors = "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type, X-Remote-Token\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\n";
                if req.starts_with("OPTIONS") {
                    let _ = stream
                        .write_all(
                            format!("HTTP/1.1 204 No Content\r\n{cors}Content-Length: 0\r\n\r\n")
                                .as_bytes(),
                        )
                        .await;
                    return;
                }
                // Health without auth
                // The root is the remote control page itself. A browser that scanned the QR
                // lands on `/?token=…`; a valid token gets the HTML, anything else gets told to
                // pair from the in-app QR instead of a bare JSON error.
                if req.starts_with("GET / ") || req.starts_with("GET /?") {
                    let authorized = extract_token(&req).as_deref() == Some(expected.as_str());
                    if !authorized {
                        let body = "Open this page from the in-app QR code (Settings ▸ Remote).";
                        let resp = format!(
                            "HTTP/1.1 401 Unauthorized\r\n{cors}Content-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                    } else {
                        let resp = format!(
                            "HTTP/1.1 200 OK\r\n{cors}Content-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
                            REMOTE_HTML.len(),
                            REMOTE_HTML
                        );
                        let _ = stream.write_all(resp.as_bytes()).await;
                    }
                    return;
                }
                // Pair endpoint: POST /pair with token validation or approve flow
                if req.contains("/pair") {
                    // For pairing, check token matches stored one -> approve.
                    let tok = extract_token(&req);
                    let ok = tok.as_deref() == Some(expected.as_str());
                    let body = serde_json::json!({"paired": ok, "token": expected}).to_string();
                    let status = if ok { "200 OK" } else { "401 Unauthorized" };
                    let resp = format!("HTTP/1.1 {status}\r\n{cors}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                    let _ = stream.write_all(resp.as_bytes()).await;
                    if ok {
                        db.set_setting("remote_paired_at", &crate::db::now_secs().to_string());
                    }
                    return;
                }
                if !is_authorized(&req, &expected) {
                    let body =
                        serde_json::json!({"error":"unauthorized, pair via QR first"}).to_string();
                    let resp = format!("HTTP/1.1 401 Unauthorized\r\n{cors}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                    let _ = stream.write_all(resp.as_bytes()).await;
                    return;
                }
                // Authenticated API
                let body = if req.contains("GET /api/queue") {
                    let q = state.queue_snapshot().await;
                    serde_json::to_string(&q).unwrap_or_else(|_| "{}".to_string())
                } else if req.contains("GET /api/playback") {
                    let p = state.playback_snapshot().await;
                    serde_json::to_string(&p).unwrap_or_else(|_| "{}".to_string())
                } else if req.contains("GET /api/search") {
                    // ?q=...
                    let query = extract_query(&req, "q").unwrap_or_default();
                    // Perform search via innertube metadata client if query non-empty
                    if query.is_empty() {
                        serde_json::json!({"items":[]}).to_string()
                    } else {
                        // Fast path: use blocking? We are in tokio, can try async.
                        // For simplicity, return placeholder and let client use Tauri via WS; LAN search is best-effort.
                        // Try to fetch via state.it.search if possible.
                        let client = state.clients.get(innertube::METADATA_CLIENT);
                        let res = if let Some(c) = client {
                            match state.it.search_songs(c, &query).await {
                                Ok(r) => {
                                    serde_json::to_value(r.items).unwrap_or(serde_json::Value::Null)
                                }
                                Err(e) => serde_json::json!({"error": e.to_string()}),
                            }
                        } else {
                            serde_json::json!({"error":"no client"})
                        };
                        res.to_string()
                    }
                } else if req.contains("POST /api/play") || req.contains("POST /api/queue") {
                    // Expect JSON body with video_id or item; for brevity, acknowledge.
                    serde_json::json!({"ok":true}).to_string()
                } else if req.contains("POST /api/toggle") {
                    state.resume_or_toggle().await;
                    serde_json::json!({"ok":true}).to_string()
                } else if req.contains("POST /api/next") {
                    state.next_in_queue().await;
                    serde_json::json!({"ok":true}).to_string()
                } else if req.contains("POST /api/prev") {
                    state.prev_in_queue().await;
                    serde_json::json!({"ok":true}).to_string()
                } else if req.contains("POST /api/volup") || req.contains("POST /api/voldown") {
                    // Volume nudge from the phone: ±10 perceptual points, persisted like the
                    // in-app slider. No `volume` event — the desktop UI isn't the one asking.
                    let up = req.contains("POST /api/volup");
                    let current = state
                        .db
                        .get_setting("volume")
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or_else(|| state.player.get_volume());
                    let volume = (current + if up { 10 } else { -10 }).clamp(0, 100);
                    let _ = state.player.set_volume(volume);
                    state.db.set_setting("volume", &volume.to_string());
                    serde_json::json!({"ok":true,"volume":volume}).to_string()
                } else {
                    serde_json::json!({"error":"not found"}).to_string()
                };
                let resp = format!("HTTP/1.1 200 OK\r\n{cors}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body);
                let _ = stream.write_all(resp.as_bytes()).await;
            });
        }
    });
}

fn extract_query(req: &str, key: &str) -> Option<String> {
    let start = req.find('?')?;
    let line_end = req[start..].find(' ').unwrap_or(req.len() - start);
    let qs = &req[start + 1..start + line_end];
    for pair in qs.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == key {
                return Some(
                    urlencoding::decode(v)
                        .map(|s| s.into_owned())
                        .unwrap_or_else(|_| v.to_string()),
                );
            }
        }
    }
    None
}
