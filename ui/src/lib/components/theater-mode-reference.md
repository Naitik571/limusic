# Theater Mode Port Reference

## What upstream theater mode is

Upstream `TheaterMode.svelte` is a dedicated fullscreen takeover driven by a Rust `theater_fullscreen` command. It is NOT just a CSS overlay inside the existing window. It:

- changes the actual window fullscreen state via `window.set_fullscreen(true/false)`
- on Windows, restores the window from maximized before entering fullscreen, otherwise fullscreen is a no-op when the window is maximized
- provides a two-column layout: cover art + transport controls on the left, lyrics on the right
- includes idle chrome that fades after 3.5s of no mouse movement
- pre-bakes artwork into a blurred wash and accent-derived glow blobs
- exits on Esc, close button, or `beforeNavigate`

## Required Rust changes

### commands.rs

```rust
#[tauri::command]
pub async fn theater_fullscreen(window: tauri::WebviewWindow, on: bool) -> Result<(), String> {
    if on {
        // Windows: restore from maximized first, otherwise set_fullscreen is ignored
        #[cfg(target_os = "windows")]
        {
            use tauri::Window;
            if let Ok(w) = window::get_window(&window, window.label()) {
                let _ = w.is_maximized();
            }
        }
        window.set_fullscreen(true).map_err(|e| e.to_string())
    } else {
        window.set_fullscreen(false).map_err(|e| e.to_string())
    }
}
```

### lib.rs

Add `commands::theater_fullscreen` to `invoke_handler(tauri::generate_handler![...])`.

## Required frontend changes

### api.ts

```typescript
export const theaterFullscreen = (on: boolean) => invoke<void>('theater_fullscreen', { on });
```

### player.svelte.ts

Add `theaterOpen: false` to the `ui` state object.

### TheaterMode.svelte

- mount `onMount(() => api.theaterFullscreen(true))`
- cleanup `return () => api.theaterFullscreen(false).catch(() => {})`
- `beforeNavigate(() => ui.theaterOpen = false)`
- idle timer: `setTimeout(() => (idle = true), 3500)`, clear on `pointermove`
- artwork wash + glow + mesh backdrop
- cover art with fallback cascade
- seek bar, play/pause, prev/next, shuffle, repeat, like, volume
- lyrics pane with wheel-scroll + `data-theater-lyrics`

### +layout.svelte

Mount `<TheaterMode />` when `ui.theaterOpen && playback.now`.

### NowPlaying.svelte

Wire the existing sing-mode button to `ui.theaterOpen = true` instead of local `sing` state.

## Fork compatibility notes

- The fork uses `playback.liked: boolean`, not `playback.rating`. Replace all `playback.rating === 'like'` with `playback.liked`.
- Import `ui` from `$lib/player.svelte` in any component that reads `ui.theaterOpen`; importing only `playback` causes `Cannot find name 'ui'` at the call site.
- Do NOT reuse the existing sing-mode overlay for theater mode. Theater mode needs actual window fullscreen, which the sing-mode overlay does not provide.
