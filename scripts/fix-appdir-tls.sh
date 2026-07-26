#!/usr/bin/env bash
# Repair the bundled AppDir so it runs on hosts that aren't the build machine, then repack and
# re-sign the AppImage in place.
#
# Two defects, both from linuxdeploy:
#
#   1. GIO_EXTRA_MODULES is written with a literal newline in it, plus an absolute path into the
#      build machine's own target/ directory. GLib parses one garbage path, finds no module, and
#      the webview falls back to GDummyTlsBackend (supports_tls=False): no HTTPS at all, so no
#      thumbnails, no player.js, no PoToken, and mpv gets handed a dead URL. Fixed by pointing it
#      at the AppDir's own module directories.
#
#   2. libjack.so.0 is on linuxdeploy's excludelist (it normally must match the host's JACK), but
#      libmpv.so.2 and libavdevice.so.60 DT_NEED it — so on a host with no JACK at all the app
#      cannot even start: "error while loading shared libraries: libjack.so.0". Fixed by bundling
#      it. That also makes the host's pipewire-jack shim unreachable, which is what killed v0.2.10:
#      the shim came from the host, wanted pw_log_topic_register from pipewire 1.6, and resolved
#      against the pipewire 1.0 in the bundle.
#
# WHAT THIS SCRIPT MUST NOT DO: prune libraries out of the AppDir. v0.2.11 tried that and broke
# worse than what it fixed. Two independent reasons, both learned the hard way:
#
#   - Sonames are not portable. Arch ships libnettle.so.9; the bundle's libarchive and libsrt want
#     .so.8, so dropping it made the app unloadable there.
#   - Host gio modules are built against the host's GLib. Once GIO_EXTRA_MODULES pointed at host
#     directories, GLib tried to load *every* module there — and gvfs on Debian testing wants
#     g_variant_builder_init_static, which arrived in GLib 2.84 and isn't in the 2.80 we bundle.
#
# The trust store that started all of this needs no help now: Ubuntu's gnutls has
# /etc/ssl/certs/ca-certificates.crt compiled in, and that path exists on Debian, Ubuntu, Fedora and
# Arch alike (verified against the bundled stack in containers on each). Building on Fedora is what
# made the anchors unreachable, and the build moved to Ubuntu.
#
# Usage:  scripts/fix-appdir-tls.sh [bundle-dir]     (default: target/release/bundle/appimage)
# Runs from CI (.github/workflows/linux-release.yml) and by hand after a local
# `cargo tauri build --bundles appimage` when you want to test the repaired AppDir.
set -euo pipefail
cd "$(dirname "$0")/.."

BUNDLE="${1:-target/release/bundle/appimage}"
APPDIR="$(readlink -f "$BUNDLE/limusic.AppDir" 2>/dev/null || true)"
[ -n "$APPDIR" ] && [ -d "$APPDIR" ] || {
  echo "no AppDir at $BUNDLE/limusic.AppDir — run \`cargo tauri build --bundles appimage\` first"; exit 1; }
APPIMAGE="$(ls "$BUNDLE"/limusic_*.AppImage 2>/dev/null | head -1 || true)"
[ -n "$APPIMAGE" ] || { echo "no limusic_*.AppImage in $BUNDLE"; exit 1; }
APPIMAGE="$(readlink -f "$APPIMAGE")"

# Copy every DT_NEEDED of $1 that the AppDir doesn't already have. glibc and the loader are the
# host's job; everything else has to travel with us, or we just move "cannot open shared object
# file" one library along (jackd2's libjack needs libdb-5.3, which Arch doesn't ship at all).
bundle_deps_of() {
  local of="$1" name path
  while read -r name path; do
    case "$name" in libc.so.*|libm.so.*|libpthread.so.*|libdl.so.*|librt.so.*|ld-linux*) continue;; esac
    [ -e "$APPDIR/usr/lib/$name" ] && continue
    [ -e "$path" ] || continue
    cp -L "$path" "$APPDIR/usr/lib/$name"
    echo "==> bundled $name (dependency of $(basename "$of"))"
  done < <(ldd "$of" | awk '/=> \//{print $1, $3}')
}

# 1. libjack: bundle the build host's copy. Prefer a real jackd2 libjack over a pipewire-jack shim
#    if the host has both, since the shim drags libpipewire's version coupling back in.
if [ -e "$APPDIR/usr/lib/libjack.so.0" ]; then
  echo "==> libjack already bundled"
else
  JACK=""
  for cand in /usr/lib/x86_64-linux-gnu/libjack.so.0 /usr/lib64/libjack.so.0 /usr/lib/libjack.so.0; do
    [ -e "$cand" ] && { JACK="$cand"; break; }
  done
  # Fallback for hosts that keep it off the default path — Fedora's pipewire-jack lives in
  # /usr/lib64/pipewire-0.3/jack/. CI hits the standard path above and gets jackd2's real libjack;
  # this only matters for local test builds, where the shim pairs with that host's own libpipewire.
  # `|| true`: head closing the pipe early makes ldconfig die of SIGPIPE, which pipefail would
  # otherwise turn into a silent fatal exit 141.
  [ -n "$JACK" ] || JACK="$(ldconfig -p 2>/dev/null | awk '/libjack\.so\.0 /{print $NF}' | head -1 || true)"
  [ -n "$JACK" ] || { echo "libjack.so.0 not found on the build host — install libjack-jackd2-0"; exit 1; }
  cp -L "$JACK" "$APPDIR/usr/lib/libjack.so.0"
  echo "==> bundled libjack.so.0 from $JACK"
  bundle_deps_of "$JACK"
fi

# 1b. The gio TLS module. linuxdeploy's GTK plugin bundles it on Fedora but not on Ubuntu, so copy
#     it in ourselves rather than depend on that. Only this one module: gvfs and libproxy are built
#     against the host's GLib and blow up against the older one we bundle, which is exactly what
#     v0.2.11 shipped.
if ls "$APPDIR"/usr/lib/gio/modules/libgiognutls.so >/dev/null 2>&1; then
  echo "==> gio TLS module already bundled"
else
  TLSMOD=""
  for dir in "$(pkg-config --variable=giomoduledir gio-2.0 2>/dev/null || true)" \
             /usr/lib/x86_64-linux-gnu/gio/modules /usr/lib64/gio/modules /usr/lib/gio/modules; do
    [ -n "$dir" ] && [ -e "$dir/libgiognutls.so" ] && { TLSMOD="$dir/libgiognutls.so"; break; }
  done
  [ -n "$TLSMOD" ] || { echo "libgiognutls.so not found — install glib-networking on the build host"; exit 1; }
  mkdir -p "$APPDIR/usr/lib/gio/modules"
  cp -L "$TLSMOD" "$APPDIR/usr/lib/gio/modules/libgiognutls.so"
  echo "==> bundled the gio TLS module from $TLSMOD"
  bundle_deps_of "$TLSMOD"
fi

# 2. Point GIO_EXTRA_MODULES at the AppDir's own module directories — never the host's, see the
#    header. Appended rather than edited in place: AppRun *sources* the hook, so the last assignment
#    wins, and appending can't be broken by linuxdeploy reshaping the lines above it.
HOOK="$APPDIR/apprun-hooks/linuxdeploy-plugin-gtk.sh"
[ -f "$HOOK" ] || { echo "no GTK apprun hook at $HOOK — did linuxdeploy's plugin layout change?"; exit 1; }
echo "==> Overriding GIO_EXTRA_MODULES with the bundled module dirs…"
grep -q '^# Limusic: the value written above' "$HOOK" || cat >> "$HOOK" <<'EOF'

# Limusic: the value written above is unusable — it contains a literal newline and an absolute path
# into the build machine's target/ dir. Bundled dirs only: host modules are built against the host's
# GLib and fail to load into ours (gvfs wants g_variant_builder_init_static, GLib >= 2.84).
export GIO_EXTRA_MODULES="$APPDIR/usr/lib/gio/modules:$APPDIR/usr/lib64/gio/modules"
EOF

# …and make sure there is actually something there to load. An AppDir with no TLS module is the
# original bug in a new hat: the app starts, looks fine, and can't reach YouTube.
if ! ls "$APPDIR"/usr/lib/gio/modules/libgio*.so "$APPDIR"/usr/lib64/gio/modules/libgio*.so >/dev/null 2>&1; then
  echo "no gio modules bundled — install glib-networking on the build host, or the webview gets no TLS backend"
  exit 1
fi
echo "==> bundled gio modules: $(ls "$APPDIR"/usr/lib/gio/modules "$APPDIR"/usr/lib64/gio/modules 2>/dev/null | grep -c '\.so')"

# 3. Repack with the packer Tauri already downloaded for the original bundle.
# Globbed, not hardcoded: the exact filename is Tauri's business and CI runs a different CLI version.
PACKER="$(ls "$HOME"/.cache/tauri/linuxdeploy-plugin-appimage*.AppImage 2>/dev/null | head -1 || true)"
[ -n "$PACKER" ] && [ -x "$PACKER" ] || {
  echo "no linuxdeploy appimage plugin in $HOME/.cache/tauri — did \`tauri build --bundles appimage\` run?"; exit 1; }
echo "==> Repacking $(basename "$APPIMAGE")…"
rm -f "$APPIMAGE"
# NO_STRIP: linuxdeploy ships an ancient `strip` that chokes on modern ELF sections (DT_RELR).
# APPIMAGE_EXTRACT_AND_RUN: the packer is itself an AppImage and CI runners have no FUSE.
ARCH=x86_64 NO_STRIP=true OUTPUT="$APPIMAGE" APPIMAGE_EXTRACT_AND_RUN=1 \
  "$PACKER" --appdir "$APPDIR"
[ -f "$APPIMAGE" ] || { echo "packer produced no $APPIMAGE"; exit 1; }
chmod +x "$APPIMAGE"

# 4. Re-sign: repacking invalidated the signature the bundler made, and latest.json is generated
#    from the .sig — a stale one silently breaks every self-update.
if [ -n "${TAURI_SIGNING_PRIVATE_KEY:-}" ]; then
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
  echo "==> Re-signing…"
  rm -f "$APPIMAGE.sig"
  if command -v tauri >/dev/null; then tauri signer sign "$APPIMAGE"; else cargo tauri signer sign "$APPIMAGE"; fi
  [ "$APPIMAGE.sig" -nt "$APPIMAGE" ] || { echo "no fresh .sig next to $APPIMAGE after signing"; exit 1; }
  echo "==> Signed: $APPIMAGE.sig"
else
  # Local test runs don't need a valid signature. Every path that ships one checks for the key first.
  echo "==> TAURI_SIGNING_PRIVATE_KEY not set — skipping the re-sign (unsignable AppImage, test only)"
  rm -f "$APPIMAGE.sig"
fi

echo "==> Done: $APPIMAGE"
