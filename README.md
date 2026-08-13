<div align="center">

<img src="./assets/docs/limusic-github-image.png" alt="Limusic Banner" width="100%">

# Limusic

**A native desktop YouTube Music client — Rust + Tauri, ad-free, no Electron.**

<p align="center">
  <a href="https://github.com/Naitik571/limusic/releases/latest"><img alt="GitHub Downloads" src="https://img.shields.io/github/downloads/Naitik571/limusic/total?style=for-the-badge&label=DOWNLOADS&color=a4c400"></a>
  <a href="https://github.com/Naitik571/limusic/releases/latest"><img alt="GitHub Release" src="https://img.shields.io/github/v/release/Naitik571/limusic?display_name=release&style=for-the-badge&color=a10935"></a>
  <img alt="License" src="https://img.shields.io/github/license/Naitik571/limusic?style=for-the-badge&color=1881cc">
  <br>
  <img src="https://img.shields.io/badge/Linux-FCC624?style=for-the-badge&logo=linux&logoColor=black">
  <img src="https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logoColor=white">
  <img src="https://img.shields.io/badge/Tauri_2-24C8D8?style=for-the-badge&logo=tauri&logoColor=white">
  <img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white">
</p>

**Limusic** talks directly to YouTube's internal API and plays audio through libmpv — no bundled
browser runtime, no backend server, no ads in the audio. It started as a desktop rebuild of the
playback engine behind [Metrolist](https://github.com/mostafaalagamy/Metrolist), an Android
YouTube Music client, and grew from there.

</div>

---

## Features

- **Ad-free playback** — streams come straight from YouTube's API, ads never do
- **Search & browse** — songs, albums, artists, playlists, and the YTM home feed
- **Sign in** with your YouTube Music account: in-app Google login or cookie-paste
- **Your library** — playlists, liked songs, and write actions (like, add to playlist, create/rename/delete playlists, subscribe)
- **Gapless playback**, powered by libmpv
- **Queue** with radio/automix continuation, drag-to-reorder, restored across restarts
- **Synced lyrics** — line-by-line side panel with auto-scroll and click-to-jump, plus an Apple Music-style word-by-word karaoke sweep on the active line
- **Mini Player** — Minimize the player and keep enjoying your music
- **Sleep timer** — pause after 15/30/60 minutes or at the end of the song; keeps counting even with the window closed
- **Local Music** — ability to play your own local music, with all metadata still intact
- **Last.fm scrobbling** — connect once from the title bar, every play is scrobbled
- **Discord Rich Presence** — artwork, live progress bar, one click to toggle
- **OS media keys** and now-playing integration (MPRIS on Linux, SMTC on Windows)
- **System tray** — close the window, keep the music; play/pause and skip from the tray, optional start-on-login
- **Listen Together** — synced listening rooms over a small self-hosted relay
- **Offline downloads** — save any track for offline listening from its ⋮ menu; downloaded tracks play straight from disk, so they work with no network and save bandwidth (Settings → Downloads controls the location, default quality and format)
- **Audio visualizer** — a live spectrum behind the cover in the mini and floating players; toggle it in Settings → Playback
- **Self-updating builds** (AppImage on Linux, setup.exe on Windows)
- **Customization via Themes and Fonts** — Customize your music player to your hearts content

---

## What's different in this fork

A growing set of changes on top of upstream, all merged into `master`.

**Earlier rounds (base upstream `v0.3.12`):**

- **No more quiet playback** — the per-track loudness filter is gone. Upstream
  applied an attenuate-only gain derived from YouTube's `loudnessDb` field
  (typically +2…+7 dB on real libraries), so tracks were cut by up to 7 dB.
  Playback is now unmodified, and any stale filter left by an older build is
  cleared as soon as a track loads.
- **Keyboard shortcuts** — see the table in the next section: `Space`/`K`
  play-pause, `Shift`+`N`/`P` next-previous, `M` mute, arrows for volume and
  seek, `J`/`L` for ±10 s seeks. They work in both the main window and the
  mini player.
- **Sleep timer** — the moon button in the player bar pauses playback after
  15/30/60 minutes or at the end of the song. The countdown is enforced by the
  Rust core, so it keeps running (and pausing) even with the window closed.
- **Drag to reorder the queue** — grab any upcoming track and drop it on
  another row to move it into that slot. The playing track stays put, and
  guests in a Listen Together room can't reorder.
- **Test-stability fix** — the Discord presence backoff test no longer flakes
  on an exact-boundary timing race that randomly failed CI.

**The v0.3.16 round** — player ergonomics, lyrics, queue and playlist UX:

- **Maximized-player gestures** — in the expanded player, scrolling over the
  square cover art adjusts the volume (one notch per wheel tick, with a live
  volume badge that fades out), and clicking the cover toggles play/pause. The
  wheel is deliberately ignored anywhere else in the maximized view, so
  browsing nearby content can never change the volume by accident.
- **Smooth lyrics** — the lyrics panel glides with a hand-tuned 650 ms
  ease-in-out-quint tween instead of the browser's native jumpy snap, and
  yields while you scroll yourself, resuming three seconds after your last
  input — it never fights your hand.
- **Queue history peek** — scroll up at the top of the queue to reveal the
  songs that already played, four at a time, all the way back to the start of
  the session. The reveal survives track changes (it only resets when the
  queue itself is rebuilt).
- **Live search suggestions** — both the home search bar and the /search page
  suggest songs, artists, albums and playlists as you type (debounced,
  keyboard-navigable, click-to-play) instead of waiting for Enter.
- **"More like this" playlist recommendations** — open any editable playlist
  and a shelf suggests similar songs. Every recommendation has its own play
  button so you can audition it before committing, songs already in the
  playlist are never suggested, adding a song immediately swaps in a fresh
  one, and a "Find more like this" button at the bottom rotates the seed to
  the next playlist track and fetches a new batch.
- **Playlist multi-select & bulk actions** — Ctrl/Shift-click to select several
  tracks at once, then right-click to move them to another playlist or remove
  them all in one go.
- **Edit-home drag fix** — drag-to-reorder on the home-page editor is driven
  by pointer events, which work reliably in the WebView2 runtime where
  HTML5 drag-and-drop is broken.

**The latest round** — ported from upstream: restricted tracks, faster
startup, and a play/pause flash:

- **Restricted tracks play again** — YouTube changed its players so the old
  regex-based signature/n-transform extraction stopped matching everywhere.
  The cipher tables now come from the same two community registries the other
  players read (Metrolist's faraday + Zemer's zemer-cipher), polled and merged
  at runtime with the tracked table kept deliberately empty; every release
  build bakes in a snapshot so even a first run with no network access to the
  registries can decipher. Stream URLs are validated with a HEAD before use
  (WEB_REMIX included — previously only the fallback clients were checked),
  the broken IOS client was dropped for ANDROID_VR, and a stream that gets
  rejected mid-playback is retried on another client.
- **Faster startup (PoToken persistence)** — the BotGuard session token
  (~12 h validity per Google's own `/GenerateIT`) used to be re-minted on
  every launch, standing up a hidden web process and running the full
  bootstrap (~1.6 s of startup) to re-learn a string we were told stays valid
  until tomorrow. It now round-trips through the settings database (internal
  key — the webview can neither read nor write it), so a second launch
  skips the bootstrap entirely. A token Google stops honouring early is
  dropped the moment a web-client stream is rejected instead of being
  replayed for the rest of its nominal lifetime.
- **Play/pause flash over the art** — clicking the cover art in the maximized
  player toggles playback and flashes the action just taken (play or pause
  icon) over the artwork, so the click reads as deliberate.

**The v0.3.17–v0.3.19 rounds** — karaoke, offline playback, a visualizer, and polish:

- **Word-level karaoke lyrics** — the active line now sweeps word-by-word with a
  gradient fill (Apple Music style) using per-word timing from Boidu. The sweep is
  interpolated every animation frame between the player's ~250 ms position ticks, so
  it glides instead of stepping even on a throttled position feed.
- **Offline downloads** — a dedicated **Settings → Downloads** tab sets the download
  location (folder picker), default quality (Low/Auto/High) and audio format
  (M4A/Opus/WebM), plus an "use downloads when available" toggle. Any track's ⋮ menu
  has Download / Remove download. Saved files play straight from disk through the
  resolver, so they work fully offline and skip the stream-URL resolve on replay.
- **Audio visualizer** — a reactive spectrum behind the cover in both the mini and
  floating players, toggled in **Settings → Playback**. It's driven by real playback
  state (playing/paused + position) rather than a second audio decode, so it costs no
  extra bandwidth and reacts live across every window.
- **Aurora visual polish** — a translucent, glass-toned refresh of the player surfaces.

---

<h2 align="center">Download & Install</h2>

<p align="center">
  <a href="https://github.com/Naitik571/limusic/releases/latest">
    <img src="https://img.shields.io/badge/GitHub_Releases-100000?style=for-the-badge&logo=github&logoColor=white" height="40">
  </a>
</p>

| Platform | File | Notes |
|---|---|---|
| Linux | `.AppImage` | Self-updating, libmpv bundled. Needs glibc 2.39+ (Ubuntu 24.04+, Debian 13+, Fedora 40+) |
| Linux (Fedora/RHEL) | `.rpm` | Needs `mpv-libs` installed (`sudo dnf install mpv-libs`) |
| Windows | `-setup.exe` | Self-updating |
| Windows | `.msi` | Plain installer, no auto-update |
| macOS | none yet | Build from source, see [docs/BUILD-PLATFORMS.md](docs/BUILD-PLATFORMS.md) |

---

## Keyboard Shortcuts

OS media keys (SMTC on Windows, MPRIS on Linux) work even while the window is
unfocused. Inside the app, the standard set follows YT Music's web conventions:

| Key | Action |
|---|---|
| `Space` / `K` | Play / pause |
| `Shift` + `N` / `Shift` + `P` | Next / previous track |
| `M` | Mute (restores the previous level) |
| `↑` / `↓` | Volume +5 / −5 |
| `←` / `→` | Seek −5s / +5s |
| `J` / `L` | Seek −10s / +10s |

Shortcuts yield to whatever is focused: typing in a search box, or Space on a
focused button, still do the native thing.

---

## Scrobbling & Discord

Both live in the title bar, next to the window controls.

- **Last.fm** — click the Last.fm mark, approve Limusic in the browser tab that
  opens, and you're connected for good. Tracks scrobble at the halfway point (or
  four minutes, whichever comes first), which is Last.fm's own rule. Click again
  to see the account or disconnect.
- **Discord** — click the Discord mark to toggle Rich Presence. Green dot means
  it's live. The card shows the track, artist, album art, and a progress bar, and
  it disappears when you pause.

Building from source? Last.fm needs your own API credentials — they're not in the
repo. Get a key at [last.fm/api/account/create](https://www.last.fm/api/account/create)
and put it in `src-tauri/lastfm.keys`:

```
LIMUSIC_LASTFM_API_KEY=your_key
LIMUSIC_LASTFM_API_SECRET=your_secret
```

Without that file everything else still builds and runs; the Last.fm button just
reports that it isn't configured.

---

## Lyrics

Open the panel with the microphone button in the player bar, next to the queue
button. It takes the same side of the window as the queue, so opening one closes
the other.

Lyrics come from [LRCLIB](https://lrclib.net) first, then YouTube Music's own
timed lyrics, falling back to plain un-timed text when nobody has a synced
version. Matching is keyed on the track's exact length, because popular songs
exist as several cuts and the wrong one drifts a few seconds out. Results are
cached locally, so replaying a track is instant.

Note that YouTube Music's lyrics are licensed per region and are missing
entirely in some countries — where that's the case, LRCLIB does all the work.

---

## Listen Together

Synced listening with friends. Everyone streams their own audio from YouTube;
the room only relays play/pause, seeks, track changes and the queue. One person
hosts the relay:

```bash
cargo run -p sync-server        # plain WebSocket on 0.0.0.0:8080
```

Front it with something that terminates TLS (Tailscale Funnel, Cloudflare
Tunnel), then paste the `wss://` URL into the Listen Together panel in the app.
Rooms have join codes and the host approves every join and every track
suggestion.

---

## Building from Source

Fedora:

```bash
sudo dnf install mpv-libs mpv-libs-devel webkit2gtk4.1-devel \
  gcc gcc-c++ make openssl-devel librsvg2-devel
cd ui && pnpm install && cd ..
cargo tauri build
```

Windows and macOS instructions live in [docs/BUILD-PLATFORMS.md](docs/BUILD-PLATFORMS.md).

---

## How It Works, Briefly

- A pure Rust crate speaks YouTube's InnerTube API, impersonating several
  official client identities and falling back between them when one fails.
- YouTube's stream URLs are protected by obfuscated JavaScript (the signature
  cipher and the `n` parameter) and by BotGuard attestation. Limusic runs that
  JavaScript where it expects to run, in a real webview, hidden, and never lets
  any of it touch the UI process.
- Audio goes through libmpv: gapless transitions and an on-disk cache.
- The UI is a SvelteKit SPA that only ever talks to the Rust core. It never
  contacts YouTube itself.

---

## Disclaimer

This project is not affiliated with, funded, authorized, endorsed by, or in
any way associated with YouTube, Google LLC, or any of their affiliates and
subsidiaries.

All trademarks, service marks, and intellectual property rights referenced in
this project belong to their respective owners.

---

## License

[GPL-3.0](LICENSE)
