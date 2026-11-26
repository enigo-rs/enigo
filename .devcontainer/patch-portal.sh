#!/bin/sh
# Build xdg-desktop-portal-kde from a patched source and install it as a
# drop-in replacement for the distro package.
#
# TEMPORARY: this works around the missing screencast in the headless Plasma
# session. Remove this script and packages/plasma-portal-build.txt (and the
# "Build a patched xdg-desktop-portal-kde" block in the Containerfile) once the
# fix is accepted upstream and a fixed xdg-desktop-portal-kde is packaged.
set -eu

REPO="https://github.com/pentamassiv/xdg-desktop-portal-kde"
# Pinned commit from the ci_fix branch. Pinning (rather than tracking the
# branch) keeps image builds reproducible.
COMMIT="8324e9ccc0a2b5950d9b246868df81930e78717d"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

git init --quiet "$WORKDIR/src"
git -C "$WORKDIR/src" fetch --quiet --depth 1 "$REPO" "$COMMIT"
git -C "$WORKDIR/src" checkout --quiet --detach FETCH_HEAD

cmake -S "$WORKDIR/src" -B "$WORKDIR/build" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX=/usr \
    -DBUILD_TESTING=OFF

cmake --build "$WORKDIR/build"

# The D-Bus service file
# (/usr/share/dbus-1/services/org.freedesktop.impl.portal.desktop.kde.service)
# execs this exact path, so install the freshly built binary over the package's.
install -m 0755 "$WORKDIR/build/bin/xdg-desktop-portal-kde" \
    /usr/lib/x86_64-linux-gnu/libexec/xdg-desktop-portal-kde
