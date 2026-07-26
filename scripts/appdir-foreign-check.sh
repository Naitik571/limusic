#!/usr/bin/env bash
# Check a Limusic AppDir on a distro that isn't the build host. Runs INSIDE a container, with the
# AppDir mounted at /app, and installs the desktop packages it needs itself.
#
#   podman run --rm -v "$PWD/target/release/bundle/appimage/limusic.AppDir:/app:ro,z" \
#     -v "$PWD/scripts/appdir-foreign-check.sh:/check.sh:ro,z" debian:sid bash /check.sh
#
# Also runs against a published release: extract it with `--appimage-extract` and mount
# squashfs-root instead. CI calls it for debian:sid and archlinux on every build.
#
# Three checks, in the order they catch things:
#
#   1. ldd -r on the app binary — every load-time symbol resolves. Catches KI-6, KI-7, KI-8.
#   2. Host libraries the app dlopens are still loadable with the AppDir on the search path.
#      An AppDir is first on LD_LIBRARY_PATH, so anything the host dlopens later (Mesa's EGL
#      vendor, gio modules, pixbuf loaders, IM modules) links against *our* copies of the shared
#      sonames. If ours are older than what the host's copy needs, it fails to load and the caller
#      usually reports something unrelated-looking. That is KI-9, and ldd -r cannot see it.
#   3. An actual launch under Xvfb. The verification hole behind four broken releases in one night
#      was that nothing ever started the app anywhere except the build host's own OS family.
set -uo pipefail

APPDIR=/app
BIN="$APPDIR/usr/bin/limusic-app"
FAIL=0
step() { printf '\n── %s\n' "$1"; }
bad()  { echo "   FAIL: $1"; FAIL=1; }

step "installing a desktop package set"
if command -v apt-get >/dev/null; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends \
    xvfb xauth dbus dbus-x11 \
    libegl-mesa0 libgl1-mesa-dri libglx-mesa0 libgles2 libgl1 libegl1 \
    libharfbuzz0b libharfbuzz-icu0 libkrb5-3 libgssapi-krb5-2 libpango-1.0-0 \
    libasound2t64 libfribidi0 libusb-1.0-0 libcom-err2 libgpg-error0 libexpat1 \
    libfontconfig1 fonts-dejavu-core ca-certificates gvfs
elif command -v pacman >/dev/null; then
  pacman -Sy --noconfirm --quiet \
    xorg-server-xvfb xorg-xauth dbus \
    mesa libglvnd harfbuzz harfbuzz-icu krb5 pango alsa-lib fribidi libusb \
    expat fontconfig ttf-dejavu ca-certificates gvfs
elif command -v dnf >/dev/null; then
  dnf install -y -q \
    xorg-x11-server-Xvfb xorg-x11-xauth dbus-daemon dbus-x11 \
    mesa-libEGL mesa-libGL mesa-libGLES libglvnd harfbuzz krb5-libs pango alsa-lib \
    fribidi libusb1 expat fontconfig dejavu-sans-fonts ca-certificates gvfs
else
  echo "   unknown package manager"; exit 1
fi
# gvfs is deliberately installed: it is what makes a host/bundle GLib mismatch visible.
# A half-installed container makes every check below meaningless, and the missing symbols it
# produces look exactly like a real defect (KI-8). Stop here instead.
for tool in xvfb-run dbus-run-session ldd; do
  command -v "$tool" >/dev/null || { echo "   package install failed: no $tool"; exit 1; }
done

step "ldd -r: every load-time symbol resolves"
LD_LIBRARY_PATH="$APPDIR/usr/lib" ldd -r "$BIN" 2>&1 \
  | grep -E 'not found|undefined symbol' | sort -u > /tmp/ldd.txt
if [ -s /tmp/ldd.txt ]; then sed 's/^/   /' /tmp/ldd.txt; bad "unresolved symbols in $BIN"
else echo "   clean"; fi

step "host libraries the app dlopens still load with the AppDir on the path"
# The graphics stack only. Every other plugin directory the app touches is redirected to the AppDir
# by AppRun (GIO_MODULE_DIR, GDK_PIXBUF_MODULE_FILE, GTK_IM_MODULE_FILE, GTK_PATH), so the host's
# copies are never loaded and probing them only produces false positives; the launch check below
# greps for GLib's "Failed to load module:" and covers them for real. Nothing redirects libglvnd,
# which must find the host's driver because only the host's kernel and Mesa agree on it.
# Only host files matter here: anything whose soname we also bundle is resolved to *our* copy at
# runtime, so the host's version of it is never loaded and a failure on it means nothing.
BROKEN=0
for d in /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib; do
  [ -d "$d" ] || continue
  for f in "$d"/libEGL_*.so.[0-9] "$d"/libGLX_*.so.[0-9] "$d"/libgbm.so.[0-9] "$d"/dri/*.so; do
    [ -f "$f" ] || continue
    [ -e "$APPDIR/usr/lib/$(basename "$f")" ] && continue
    with=$(LD_LIBRARY_PATH="$APPDIR/usr/lib" ldd -r "$f" 2>&1 | grep -E 'undefined symbol|not found' | sort -u)
    [ -n "$with" ] || continue
    without=$(ldd -r "$f" 2>&1 | grep -E 'undefined symbol|not found' | sort -u)
    [ "$with" = "$without" ] && continue   # already broken on its own; not something we caused
    BROKEN=1
    echo "   $f"
    comm -13 <(echo "$without") <(echo "$with") | sed 's/^/      /' | head -5
    ldd "$f" 2>/dev/null | awk '/=> \//{print $1}' | while read -r s; do
      [ -e "$APPDIR/usr/lib/$s" ] && echo "      ^ we shadow: $s"
    done | sort -u
  done
done
if [ "$BROKEN" = 1 ]; then
  bad "the AppDir breaks host libraries the app loads at runtime — drop the shadowing library from the AppDir in scripts/fix-appdir-tls.sh"
else
  echo "   clean"
fi

step "launching the app under Xvfb"
export HOME=/tmp/apphome
mkdir -p "$HOME"
: > /tmp/run.log
timeout 90 dbus-run-session -- xvfb-run -a -s "-screen 0 1280x800x24" "$APPDIR/AppRun" \
  > /tmp/run.log 2>&1 &
RUNPID=$!
# Stop as soon as a webview works (about three seconds) or the app dies. The app never exits on its
# own, so without this the step would always burn the full timeout.
for _ in $(seq 90); do
  grep -q 'webview bridge OK' /tmp/run.log && break
  kill -0 "$RUNPID" 2>/dev/null || break
  sleep 1
done
kill "$RUNPID" 2>/dev/null
wait "$RUNPID" 2>/dev/null
# The app is killed rather than exiting, so its status says nothing; the log is the verdict.
grep -viE 'dbind|StatusNotifier|libEGL warning|DRI3' /tmp/run.log | head -40 | sed 's/^/   /'
if grep -qE 'Could not create .*EGL display|undefined symbol|cannot open shared object file|Failed to load module|webview never became ready|symbol lookup error|core dumped' /tmp/run.log; then
  bad "startup log contains a loader or webview failure (see above)"
fi
# "webview bridge OK" means a WebKit web process came up and round-tripped JS. It is the one line
# that separated a working AppImage from the v0.2.14 one, which logged everything else identically.
grep -q 'webview bridge OK' /tmp/run.log || bad "no webview ever became usable"

printf '\n'
[ "$FAIL" = 0 ] && { echo "PASS: $(. /etc/os-release 2>/dev/null; echo "${PRETTY_NAME:-unknown}")"; exit 0; }
echo "FAIL: $(. /etc/os-release 2>/dev/null; echo "${PRETTY_NAME:-unknown}")"
exit 1
