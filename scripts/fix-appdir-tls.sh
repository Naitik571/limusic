#!/usr/bin/env bash
# Make the bundled AppDir survive contact with a host that isn't the build machine, then repack and
# re-sign the AppImage in place.
#
# linuxdeploy bundles two things that are coupled to the *build host's* configuration and therefore
# cannot travel:
#
#   1. The TLS trust stack (gnutls, p11-kit, nettle, hogweed, tasn1). Its CA anchors come from
#      p11-kit at /usr/lib64/pkcs11 + /etc/pki/ca-trust, which are NOT bundled — so anywhere but
#      Fedora the bundled gnutls loads fine and then reports "System trust contains zero trusted
#      certificates".
#   2. The glib-networking GIO module that provides GTlsBackend. The GTK plugin writes
#      GIO_EXTRA_MODULES containing a literal newline plus an absolute path into the build machine's
#      own target/ directory, so GLib parses one garbage path, finds no module, and the webview
#      falls back to GDummyTlsBackend (supports_tls=False) — no HTTPS at all: no thumbnails, no
#      player.js, no PoToken, and mpv gets handed a dead URL.
#
# Both get the same treatment: drop the bundled copies, use the host's. Every mainstream desktop
# distro ships gnutls + glib-networking (GNOME depends on them), and this is the standard AppImage
# rule — never bundle a library that reads host configuration. The alternative, bundling
# p11-kit-trust.so plus our own CA store, is more code and more to keep current.
#
# libssl/libcrypto stay bundled: since innertube pinned rustypipe to rustls the Rust side no longer
# touches OpenSSL, and ffmpeg/libmpv use it without needing trust anchors.
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

# 1. The trust-store-coupled libraries. `rm -f` so a lib that isn't there can't abort us under -e:
#    the bundled set differs between build hosts, and "already absent" is the desired state anyway.
echo "==> Pruning host-coupled TLS libraries from the AppDir…"
for lib in libgnutls.so.30 libp11-kit.so.0 libnettle.so.8 libhogweed.so.6 libtasn1.so.6; do
  rm -fv "$APPDIR"/usr/lib/"$lib"* "$APPDIR"/usr/lib64/"$lib"*
done

# 2. Both bundled gio module dirs — the host's glib-networking serves the webview instead.
echo "==> Dropping the bundled gio modules…"
rm -rfv "$APPDIR"/usr/lib/gio/modules "$APPDIR"/usr/lib64/gio/modules

# 3. Point GIO_EXTRA_MODULES at the host's module dirs, covering the three layouts in the wild
#    (Fedora/RHEL, Debian/Ubuntu, Arch). Appended rather than edited in place: AppRun *sources* the
#    hook, so the last assignment wins, and appending can't be broken by linuxdeploy reshaping the
#    lines above it.
HOOK="$APPDIR/apprun-hooks/linuxdeploy-plugin-gtk.sh"
[ -f "$HOOK" ] || { echo "no GTK apprun hook at $HOOK — did linuxdeploy's plugin layout change?"; exit 1; }
echo "==> Overriding GIO_EXTRA_MODULES with host paths…"
grep -q '^# Limusic: the value written above' "$HOOK" || cat >> "$HOOK" <<'EOF'

# Limusic: the value written above is unusable — it contains a literal newline and an absolute path
# into the build machine's target/ dir. Host paths only; the bundled modules were removed.
export GIO_EXTRA_MODULES="/usr/lib64/gio/modules:/usr/lib/x86_64-linux-gnu/gio/modules:/usr/lib/gio/modules"
EOF

# 4. Repack with the packer Tauri already downloaded for the original bundle.
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

# 5. Re-sign: repacking invalidated the signature the bundler made, and latest.json is generated
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
