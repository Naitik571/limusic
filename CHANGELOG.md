# Changelog

All notable changes to Limusic are documented here.

Changes landing after the latest release accumulate under `## [Unreleased]`
until a version and date are stamped on them.

## [Unreleased]

## [0.7.3] - 2026-09-12

- Rebindable shortcuts — every keyboard shortcut remappable from Settings, with clash detection and Alt-modifier support.
- Spatial navigation — Alt+Arrow moves focus spatially (dialog-contained, with wrap-around).
- Audio visualizer — real-time WASAPI app-loopback spectrum (bars/ring) in Now Playing.
- Custom app icon, OBS overlay token, Downloads view owns downloads (list removed from Settings).
- Hardening — Zip-Slip guard, command-injection guard, yt-dlp guards, poison-tolerant locks, CI locked deps, tighter CSP.

## [0.7.2] - 2026-09-11

- Per-track crossfade, Downloads view (live progress, cancel-all, play-from-disk), lyrics composer (tap-along timing editor).

## [0.7.1] - 2026-09-11

- Radio honesty (refresh radio, autoplay respects its setting), lyrics translation across 9 languages.

## Older releases

Moved verbatim from the README so no history is lost:

## What's different in this fork

All available in the latest release. Fork tracks upstream versioning — this release is `v0.7.3`.

**v0.7.3 — shortcuts rebinding, spatial nav, visualizer, app icon, OBS overlay, UX polish:**
- **Rebindable shortcuts** — every keyboard shortcut (volume, seek, queue, lyrics, zoom) remappable from Settings, with clash detection and Alt-modifier support
- **Spatial navigation** — Alt+Arrow moves focus spatially (dialog-contained, with wrap-around); hardcoded Shift+Arrow volume and Ctrl chords now follow your bindings
- **Audio visualizer** — real-time WASAPI app-loopback spectrum (bars/ring) in Now Playing; engine-chosen buffer, wait-first drain, no shutdown hang
- **Custom app icon** — canvas-decoded PNG pushed to the window from Settings
- **OBS overlay URL** — Now Playing overlay link now carries the remote token
- **Downloads view owns downloads** — the duplicated downloaded-tracks list is gone from Settings; the dedicated Downloads route (live progress, cancel-all, play-from-disk) is the single place
- **Hardening pass** — Zip-Slip guard on artist-pack install, command-injection guard on external URLs, yt-dlp option/integrity guards, DB/sleep-timer poison-tolerant locks, Liked-walk page cap, atomic cipher-cache writes, CI `--locked` + minimal permissions, CSP `base-uri`/`form-action`

**v0.7.0 — native frame, romaji,-prev restart, Romanian, feel:**
- **Previous restarts past 3s** — YTM parity: a press deep in a song restarts it, from the top it steps back
- **System title bar** (Settings → General → System) — native frame with snap layouts and shadows; the bars go chromeless but keep every button
- **Romanian locale** — 284 keys from upstream's catalog, English fallback per key
- **Romaji toggle in lyrics** — kana songs transliterate in one round-trip, timings untouched
- **Feel pass** — page fade on navigation, NowPlaying hero crossfade, press-squash on transport, sidebar nudge
- **Apple layout + preset removed** (−288 lines; stored selections fall back cleanly)

**v0.6.11 — lyrics you can trust + poolside waters:**
- **Musixmatch word-salad rejected** — restricted tracks came back as fake syllable-matched gibberish that parsed as valid synced lyrics and won every tie; the provider now honours the `restricted` flag, a strict shape check drops salad from any provider, and poisoned cache rows self-heal on next play
- **Ctrl+K Enter fixed properly** — bare Enter opens the full search page again (the hook now lives on window capture; the dialog portal meant a wrapper div never saw the keystrokes)
- **Generated fallback covers** — missing/failed artwork lands on seeded art (palette + motif + initials) in cards, rows and palette results instead of blank tiles
- **AUTO vinyl skin** — the pressed disc tints itself from the cover's dominant colour, with a new Library-skin picker to match the deck one
- **Lyric line lookup is binary search**, page-cache hits refresh LRU recency, pill titles shrink-to-fit before ellipsizing
- **One Esc cascade** (picker → settings → queue → takeover → drawer) and **pressed-not-pasted custom covers** (groove texture composited over uploads)
- **Ambient video waters** — aqua/verdant/goldfish themes play real water footage behind the shell (vendored locally, poster stills under reduce-motion); clear + night keep the procedural pool
- **Liquid glass**, CD-grid skin picker, coverflow cursor caption, theme-switch crossfade, accent-following pill, edge-bleed rails, spring row entrances, sing artwash, and a **docked queue panel on wide screens**

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
- **Exponential volume + HUD** — square-law loudness taper (even steps across the whole slider, not just the top), on-screen % readout, 1% steps with Shift, and global Ctrl+Shift+Up/Down that work from the tray
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

