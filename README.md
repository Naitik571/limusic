<div align="center">

<img src="./assets/docs/limusic-github-image.png" alt="Limusic Banner" width="100%">

# Limusic
**A native desktop YouTube Music client — Rust + Tauri, ad-free, no Electron.**

<p align="center">
  <a href="https://github.com/Naitik571/limusic/releases/latest"><img alt="GitHub Downloads" src="https://img.shields.io/github/downloads/Naitik571/limusic/total?style=for-the-badge&label=DOWNLOADS&color=a4c400"></a>
  <a href="https://github.com/Naitik571/limusic/releases/latest"><img alt="GitHub Release" src="https://img.shields.io/github/v/release/Naitik571/limusic?display_name=release&style=for-the-badge&color=a10935"></a>
  <img alt="License" src="https://img.shields.io/github/license/Naitik571/limusic?style=for-the-badge&color=1881cc">
  <br>
  <img src="https://img.shields.io/badge/Windows-0078D6?style=for-the-badge&logoColor=white">
  <img src="https://img.shields.io/badge/Tauri_2-24C8D8?style=for-the-badge&logoColor=white">
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
- **Search & browse** — songs, albums, artists, playlists, and the YTM home feed; **paste any YouTube/YT Music link** (playlist, album, song, artist) from the titlebar and open it directly
- **Sign in** with your YouTube Music account: in-app Google login or cookie-paste — **channel switcher** for accounts with multiple YouTube channels
- **Your library** — playlists, liked songs, write actions (like, add to playlist, create/rename/delete playlists, subscribe), a **playlist search** to filter tracks by title/artist/album, **drag-and-drop into playlists**, one-click **dedupe**, and multi-select bulk actions
- **Gapless playback**, powered by libmpv
- **Smart crossfade** — 1–12s overlap between tracks, plus **Best Mix** harmonic queue sorting
- **Karaoke lyrics from 8 sources** — LRCLIB, Boidu, Unison, QRC, NetEase, Musixmatch, Kugou, SimpMusic — with a per-song sync offset
- **Ambient mode & Canvas** — the cover becomes an app-wide contrast-solved backdrop; Spotify Canvas loops behind the artwork when available
- **Layouts** — Default, Grove (floating player island), Canopy (transport in the top bar), Compact and Wide
- **Auto-offline** — new liked songs and playlist adds download themselves; Liked Music syncs at launch
- **History** — a local, private play diary in the sidebar: newest first, grouped by day, clear anytime
- **LAN remote** — scan a QR and control playback from your phone
- **Artist packs** — per-artist themes from the community index or a local ZIP
- **Type-to-search** — start typing on any playlist page to filter it instantly
- **Queue** with radio/automix continuation, drag-to-reorder, clear-played, restored across restarts
- **Ctrl+K command palette** — searches everything and drives the app (layouts, settings tabs, sleep timer, updates); right-click any result for the full track menu
- **Synced lyrics** — line-by-line side panel with auto-scroll and click-to-jump, plus an Apple Music-style word-by-word karaoke sweep on the active line. Immersive: blurred album-art backdrop, gradient edge fades and a soft glow on the active line
- **Island mini player** — floating Dynamic-Island pill with a live **waveform seekbar** (click/drag to scrub), like button, sleep countdown and expandable up-next; the classic Mini Player is still there too
- **Sleep timer** — Off / end-of-song / 15-30-60 min presets in Settings with a live countdown; Rust-enforced even with the window closed
- **Local Music** — play your own local files with all metadata intact
- **Last.fm scrobbling** — connect once from the title bar, every play is scrobbled
- **Discord Rich Presence** — artwork, live progress bar, one click to toggle
- **OS media keys** and now-playing integration (SMTC on Windows)
- **System tray** — close the window, keep the music; play/pause and skip from the tray, optional start-on-login
- **Listen Together** — synced listening rooms over a small self-hosted relay
- **Gamepad** — full controller support (Xbox/PS/JoyCon): play/pause, next/prev, volume, seek, fast-scrub, mini-player toggle — works even when the app is tray-minimized (see Gamepad table below)
- **Offline downloads** — save any track from its ⋮ menu; a **download manager** in the title bar shows live per-track progress. Playlists download in parallel (4 at a time, skips what's already on disk), repeat failures quarantine instead of retrying forever, and overlapping walks serialize. **Keep entire playlists offline** with per-playlist pins that top up new tracks automatically. yt-dlp fallback when a stream can't be resolved. **Downloaded tracks show a persistent indicator in their rows** and play straight from disk, offline.
- **Themes with real personality** — five palette themes beyond the accent colours: **Pixel** (8-bit CRT scanlines, Press Start 2P), **Arcade** (Bungee + dual glow), **Synthwave** (Orbitron + neon grid + sunset wash), **Gruvbox** (JetBrains Mono + warm paper), **Nord** (icy calm, IBM Plex) — each with its own fonts, background treatment, focus ring and selection colour. All six new font families are also available as overrides for any theme.
- **Self-updating builds** (setup.exe on Windows) + **Ctrl/Cmd +/− zoom**
- **Customization** — accent colour, hue, radius, font overrides and locally-loaded font files under Settings → Themes

---

## What's different in this fork

All available in the latest release. Fork tracks upstream versioning — this release is `v0.6.10`.

**v0.6.10 — island mini player (mooziac-style):**
- **Waveform seekbar** — every track's audio is decoded once in Rust (symphonia, pure-Rust, no system deps) into normalized peaks cached in SQLite; the pill glows white-cyan behind the playhead. Click **or drag-scrub** to seek, full keyboard support kept, thin-fill fallback until peaks land
- **Dynamic Island look** — near-black glass shell, white-on-dark chrome, hairline highlight; reads as hardware on any backdrop
- **Like button + sleep countdown chip** on the pill (chip appears only while a timer runs, click to cancel)
- **Tap-to-expand up-next** — chevron opens a floating sheet with the next tracks (tap to jump), shuffle + repeat toggles

**v0.6.9 — queue tools, offline pins, downloads that behave:**
- **Queue**: type-to-filter removed per feedback — kept **Clear played** (drops the played prefix, gapless lookahead survives)
- **Sleep timer presets in Settings** — Off / End of song / 15-30-60 min with a live countdown badge; backend-enforced with the window closed
- **Drag songs into playlists** — any song row drags onto sidebar playlists or onto a playlist page; locals and On Repeat are rejected with a reason
- **Playlist dedupe + Keep-offline pins** — one-click duplicate removal; per-playlist offline toggle with auto top-up of new tracks on every visit
- **Ctrl+K right-click fixed properly** — the row menu renders outside the dialog (which trapped all clicks), and dismissing it restores the palette with the search intact
- **Downloads overhaul** — Clear-finished actually clears; failed/cancelled tracks no longer wear ghost "Downloaded" badges; repeat failures quarantine for 7 days instead of retrying on every trigger; overlapping playlist walks serialize instead of writing the same file twice
- **Smoother scrolling** — async image decoding everywhere, lazy feed images, poolside home sections culled + no scroll-time entrance animations
- **Dead radio screens deleted** (~780 lines that never opened)
- **Lyrics start flush at the top** — the fixed 35vh opening gap is gone

**v0.6.8 — palette + stability pass:**
- **Ctrl+K row menus work at all** — first fix pass for the dialog-trapped clicks
- **Locale fallback hardened** — old fr/id/pt-BR saves migrate cleanly; partial catalogs fall back per key
- **Marquee restored** — long titles scroll again instead of sitting static
- **Playlist render caps** — 250-row pages with Show-more instead of mounting 5,000 rows at once
- **WEB_REMIX 403s expire** — one transient failure no longer bans a track forever (30-min TTL)

**v0.6.7 — theater mode, sticky shuffle, upstream-port fixes:**
- Theater mode replaces sing mode, sticky shuffle across queues, liked-state sync, palette right-click behavior, coverflow + stacked-fan library views, reactive hero accents, lyrics font picker

**v0.6.5 — upstream ports + fullscreen lyrics everywhere:**
- **Immersive sing mode, every layout** — fullscreen lyrics now owns the whole window: the titlebar (or Canopy's transport bar) hides while you sing, Poolside's lyrics drawer gained a ⤢ fullscreen button. Esc or ✕ brings the bar back
- **Uploads tab** (port) — Library ▸ Uploads lists the tracks you uploaded to YouTube Music, paged and searchable like Songs
- **Dislike skips & unqueues** (port) — the ⋯ menu's new *Dislike* rates the track on YouTube, skips it if it's playing, and drops every upcoming copy from the queue
- **Player-bar title links to the album** (port) — click the now-playing title to open its album
- **WEB_REMIX actually plays now** (port) — BotGuard runs outside the webview (rustypipe-botguard), so PoTokens land in the class googlevideo accepts; session token re-mints instead of degrading for the whole session
- **i18n infrastructure** (port) — English + Turkish catalogs bundled, language picker in Settings ▸ General ▸ Language
- **Home order honoured on fresh load** (port) — a section you dragged up no longer vanishes until you scroll to it

**v0.6.1 — Poolside Vinyl (BETA layout):**
- A full-app reskin: **Y2K / Frutiger Aero poolwater** — the whole app floats over an animated swimming-pool surface with drifting light caustics, glow blobs and swimming koi
- **Skeuomorphic picture-disc vinyls** — album art printed on the disc, concentric grooves, specular sheen, spindle hole; records **spin while playing** and stop when paused, sliding out of **kraft paper sleeves**
- **Now Playing deck** — current + up-next discs side by side, queue panel, hairline seek, full transport, recently-played thumbnail strip
- **Poolside Library** — ALBUMS / SONGS / ARTISTS / FOLDERS pill tabs, frosted search, glossy **Import Music** button (local folders), tilted cover grid with hover spring, featured enlarged tile
- **Album coverflow** — 3D perspective fan of sleeves with tooltip + back button; the selected album slides its disc out
- **Poolside mini-player** — spinning disc, mono uppercase title, hairline progress, aqua play button
- **"Add Custom CD Covers!"** — serif-overlay feature: print your own image onto any album's disc (persists per album)
- **Dusk mode** — darker poolwater variant; **Exit beta** chip returns to the Default layout anytime
- Beta: find it in Settings ▸ Appearance ▸ Layout ▸ **Poolside (Beta)**


**v0.6.0 — feel & finish (9 UI upgrades):**
- **Shared-element artwork transition** — the cover on the card you clicked flies into the player and lands as the big artwork; the player stops sliding up from nowhere and starts arriving from *where you clicked*
- **Artwork swipe pager** — swipe the big cover left/right to skip tracks, with drag-follow visual feedback
- **Sing mode** — fullscreen lyrics-only view with huge karaoke type; one mic button to enter, Esc to leave
- **Dual-language lyrics** — a Translate toggle in the lyrics footer renders a muted second line under the original, cached per line, works in Sing mode too
- **Ctrl+K controls the whole app** — the palette gains an Actions group: switch layouts, toggle ambient/artwork-accent/tabbed-player, jump to any Settings tab, sleep timer, mini player, check updates
- **Hover-expand sidebar** — below full-width windows the icon rail floats open to its labeled width while hovered and collapses on leave
- **Heart burst + fly-to-playlist** — liking pops a spark burst; adding to a playlist flies a little "+" from the click
- **Gamepad focus polish** — using the controller shows a clear focus ring and the queue auto-scrolls to the playing track
- **Searchable settings** — a filter box in Settings matches rows live, with "found in" chips jumping between tabs
- **Glass intensity slider** — dial the blur/translucency of every glass surface (Appearance)


**v0.5.12 — auto-offline & history:**
- **Auto-offline** — Settings ▸ Downloads: new liked songs (and playlist adds, in the wider mode) download themselves in the background. Turning it on syncs your existing Liked Music immediately and again at each launch — the walk skips what's already on disk, so it only ever fetches what's missing
- **History page** — new sidebar entry between Library and Settings: everything you played, newest first, grouped by day, with shuffle-all and a one-click clear. It's the same local diary On Repeat ranks, and it never leaves the machine

**v0.5.11 — the big feature drop:**
- **Smart crossfade** — 1–12s overlap between tracks, standard or smart, plus **Best Mix** harmonic queue sorting
- **Karaoke lyrics from 8 sources** — LRCLIB, Boidu, Unison, QRC, NetEase, Musixmatch, Kugou and SimpMusic in priority order, with a per-song sync offset that persists
- **Ambient mode** — the playing cover becomes an app-wide backdrop, with a veil solved per-artwork so text always stays readable (subtle / balanced / vivid)
- **Spotify Canvas** — the looping video plays behind the artwork when the track has one
- **Layouts** — Default, **Grove** (rounded feed card + floating player island), **Canopy** (transport lives in the top bar — no bottom bar at all), Compact and Wide; queue/lyrics dock as a real column, the player takes over the row in every layout
- **LAN remote** — scan the QR in Settings, control playback from your phone: now-playing, play/pause/skip, volume and the queue, right in the browser
- **Artist packs** — per-artist themes installed from the community index or a local ZIP
- **Exponential volume + HUD** — cubic loudness curve, on-screen % readout, 1% steps with Shift, and global Ctrl+Shift+Up/Down that work from the tray
- **Type-to-search** — start typing on any playlist page and it filters instantly, with match highlighting
- **Playlist menu gains Play / Shuffle play**; Ctrl+K no longer collides with K (play/pause); right-click works on palette results; Liked Music and Episodes for Later covers render again
- **Personal uploads play again** — authenticated TVHTML5 → WEB_CREATOR chain, and uploads no longer count as music videos (they were vanishing under "hide music videos")
- **Settings redesigned** (upstream port): grouped cards, icon rail, segmented controls — every fork setting kept

**v0.5.10 — uploads & polish:**
- Uploads route to authenticated clients and actually stream (they used to skip as "unavailable")
- Fixed: Ctrl+K also pausing playback, missing Liked Music / Episodes for Later covers, dead right-click inside the Ctrl+K palette

**v0.5.8–v0.5.9 — upstream ports + fork fixes:**
- **Ctrl+K anywhere search palette**, **Ctrl+E** now-playing toggle, **Ctrl+>/< volume** — the real #81 shortcuts, with a `⌘K` chip on the search field and one owner for every Ctrl-chord
- **Right-click opens the app's own menus** — rows, cards, sidebar, player bar, artwork, playlist/album/artist headers; placed from their real size at the pointer; Shift+right-click and text fields keep WebKit's menu
- **Themes adapt to the album art** (upstream #69) — the playing cover's hue drives accent and surfaces in light and dark, crossfading per track; sits on top of whichever theme you have
- **Scroll to change volume** — wheel over either slider, the whole player bar, or the big artwork; persisted once the gesture stops (upstream's nudge engine under the fork's wider hit areas)
- **Download manager: cancel** — per-track ✕ and Cancel-all, partial `.part` files cleaned up; **collision-safe filenames** — two tracks with the same `Title - Artist` get an id suffix instead of overwriting each other
- **Discord shows the artist** in the member-list line (#88), dialogs centre by layout so text stays crisp (#75)
- **Stream resilience**: stale sessions no longer surface as raw 403s, dead stream URLs sweep at open, WAL journal for the settings DB, failed DB writes are logged instead of swallowed
- **Apple Music lyrics tokens actually save now** — the settings allowlist was rejecting them
- **Smoother lyrics** — cancellable eased scroll tween (no more platform-dependent scrollIntoView), calmer line-state easing
- **CI** runs the full Rust test suite + clippy + fmt + svelte-check on every push/PR

**v0.5.3 — upstream parity + full gamepad + theme polish:**
- **Open a YouTube/Music link** — new Link button in the title bar accepts any `youtube.com` / `music.youtube.com` / `youtu.be` URL (playlist, album, artist, song). Song links start radio; other kinds open their page. Reaches link-only playlists that never appear in search.
- **Playlist page now scrolls as one** — the header scrolls away with the tracks instead of pinning 1/3 of the window (matches album page behaviour)
- **Shortcuts + Jump-back-in stay live** — adding tracks to a playlist (or a cover change) immediately updates Shortcuts and library/recent tiles, no restart needed
- **Full gamepad** — every button mapped for one-handed control: shoulders/triggers for volume/seek, left-stick Y for volume scrub, **right-stick X for fast scrub ±30s**, Select → previous, stick deadzone 0.35 @ ~10 Hz, trigger axes fallback for XInput. Frontend handles `seekfwd_fast`/`seekback_fast` alongside normal seek. Hot-plug best-effort.
- **Theme personality polish** — Pixel's dither/scanlines, Synthwave's grid + dual glow, Gruvbox's warm corner glow tightened so switching themes actually feels different

**v0.5.0–v0.5.1 — parallel downloads, indicators, search, themes:**
- **Parallel playlist downloads** — walks every page, skips existing files, pulls 4 at a time with a done/skipped/failed summary
- **Download indicators** — persistent dot/check in rows for tracks already offline (stays in sync as files are removed)
- **Playlist search** — filter loaded tracks by title/artist/album while scrolling
- **Lyrics polish** — active line as focal point (gradient + primary glow / karaoke scale-up), past lines recede, cleaner unsynced typography
- **Themes with personalities** — Pixel, Arcade, Synthwave, Gruvbox, Nord (see Features → Themes) + six new font families available to any theme
- **Edit playlists** (from upstream `0.4.8/0.5.0` port) — name/description/privacy + cover upload from the playlist menu; playlist search refinement
- **Interface zoom** — `Ctrl/Cmd +/−` to scale the UI (about text + layout), and the player view can show queue & lyrics as tabs

**v0.4.7 — channel switcher & artist polish:**
- **Channel switcher** — accounts with multiple YouTube channels can pick which one requests act as, from the title bar; multi-channel sign-in pauses for choice and survives restarts
- **Artist monthly listeners** + **top songs "show all"** into the full playlist page

**v0.4.3–v0.4.4 — sorter, fast downloads, Megalobiz:**
- **Playlist sorting** — playlist page sorts by Title/Artist/Album/Newest/Oldest/Plays (default keeps playlist order), stable as more pages load, with reverse toggle
- **Fast downloads** — `ratebypass=yes` only on bare `googlevideo` URLs (never on signed ones — avoids 403), pooled HTTP/2, throttled progress → defeats the ~50–200 KB/s throttle; files named `Title - Artist` from real metadata
- **Megalobiz synced lyrics** — 5th fallback source (after LRCLIB, YouTube timed, Musixmatch, Genius)

**v0.4.2 — downloads that actually work, immersive lyrics, gamepad, one mini player:**
- **Downloads that work** — byte-level progress, yt-dlp fallback when resolve fails, title-bar download manager with live progress + retry; playlists download per-track
- **Immersive lyrics** — blurred art backdrop, edge fades, soft glow on active line (karaoke sweep unchanged)
- **Gamepad (first pass)** — Xbox/PS pad drives playback from a background thread even when tray-minimized
- **One mini player** — Mini + Floating merged into a single extendable component (resize live via toggle)

**v0.3.17–v0.3.19 — karaoke & offline:**
- **Word-level karaoke** — active line sweeps word-by-word (gradient fill) via Boidu timings, interpolated per frame between ~250 ms position ticks
- **Offline downloads** — Settings → Downloads (location, quality, format, use-when-available); any track's ⋮ → Download/Remove; plays from disk (no stream resolve), fully offline; yt-dlp fallback
- **Aurora polish** — translucent glass refresh of player surfaces

**v0.3.17+ — cipher resilience, startup speed, player polish:**
- **Restricted tracks play again** — cipher tables from community registries (faraday + zemer), polled + merged at runtime, baked snapshot in release builds; validates stream URLs via HEAD (including `WEB_REMIX`); dropped broken `IOS`, retried mid-playback
- **Faster startup (PoToken persistence)** — BotGuard token (~12 h) persisted to DB so second launch skips the hidden-webview bootstrap (~1.6 s saved); invalidated on first rejected web-client stream
- **Play/pause flash** — click maximized cover toggles and flashes the action icon

**v0.3.16 — player, lyrics, queue & playlist UX:**
- **Maximized-player gestures** — wheel over cover = volume (one tick, fades), click cover = play/pause; ignored elsewhere so nearby content never changes volume by accident
- **Smooth lyrics** — 650 ms quint tween instead of snap; yields to your scroll, resumes 3s after last input
- **Queue history peek** — scroll up at the top of the queue to reveal previously played (4 at a time, survives track changes)
- **Live search suggestions** — home + `/search` typeahead (debounced, keyboard-navigable)
- **"More like this" playlist recommendations** — editable playlists show auditionable suggestions; avoids dupes, swaps on add, seed-rotate button
- **Playlist multi-select + bulk actions** — Ctrl/Shift select, right-click move/remove
- **Edit-home drag fix** — pointer events on the home editor (WebView2 HTML5 DnD is broken)

**Base fixes (upstream `v0.3.12` base):**
- **No more quiet playback** — removed the attenuate-only `loudnessDb` gain filter (typically +2…+7 dB cut) so audio is unmodified; stale filters from older builds are cleared on track load
- **Keyboard shortcuts** — `Space`/`K` play/pause, `Shift+N`/`P` next/prev, `M` mute, arrows volume/seek (`↑`/`↓` ±5, `←`/`→` ±5s), `J`/`L` seek ±10s. Work in both windows, yield to focused inputs.
- **Sleep timer** — player-bar moon button: pause after 15/30/60 min or end-of-song; Rust-enforced with window closed
- **Drag to reorder the queue** — pointer-driven (not HTML5 DnD, broken in some webviews); playing track stays put, guests can't reorder in Listen Together
- **Test-stability** — Discord backoff boundary race fix (no more random CI flake)
<h2 align="center">Download & Install</h2>

<p align="center">
  <a href="https://github.com/Naitik571/limusic/releases/latest">
    <img src="https://img.shields.io/badge/GitHub_Releases-100000?style=for-the-badge&logo=github&logoColor=white" height="40">
  </a>
</p>

| Platform | File | Notes |
|---|---|---|
| Windows | `-setup.exe` | Self-updating |
| Windows | `.msi` | Plain installer, no auto-update |

---

## Keyboard Shortcuts

OS media keys (SMTC on Windows) work even while the window is
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

## Gamepad

Plug in an Xbox / PlayStation / JoyCon (any pad GilRs supports) and it drives
playback from a background thread — so it works **even when the app is minimized to
the tray**, no focus needed. Buttons:

| Button | Action |
|---|---|
| `A` (South) | Play / pause |
| `B` (East) | Next track |
| `X` (West) | Previous track |
| `Y` (North) | Mute |
| `Select` / Back | Previous track |
| `LB` / `RB` | Seek −10s / +10s |
| `LT` / `RT` (triggers) | Volume −5 / +5 |
| D-pad ↑ / ↓ | Volume +5 / −5 |
| D-pad ← / → | Seek −10s / +10s |
| Left stick X | Seek (hold, ~10 Hz) |
| Left stick Y | Volume (hold, ~10 Hz) |
| Right stick X | **Fast seek** −30s / +30s (hold) |
| `Start` | Toggle the mini player |

The mapping is fixed for now; if your pad isn't detected, make sure it's connected
before the app starts (hot-plug is best-effort).

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

Windows:

```powershell
# 1. libmpv dev package (shinchiro build) — put libmpv-2.dll + mpv.lib in .libmpv\ and point the
#    linker at it. See docs/BUILD-PLATFORMS.md for the exact steps.
$env:RUSTFLAGS = "-L native=C:\path\to\.libmpv"
# 2. Frontend deps, then the build.
cd ui && pnpm install && cd ..
cargo tauri build
```

Full Windows instructions live in [docs/BUILD-PLATFORMS.md](docs/BUILD-PLATFORMS.md).

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
