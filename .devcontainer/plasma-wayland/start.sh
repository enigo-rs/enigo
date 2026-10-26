#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/../lib.sh"
. "$script_dir/../session.sh"
require_container
require_command kwin_wayland
require_command plasmashell
require_command xdpyinfo

# Start a normal headless KDE Plasma Wayland session: a session and system bus,
# PipeWire, KWin with its virtual backend and Xwayland, and plasmashell.
#
# KWin's package carries a cap_sys_nice file capability that default container
# capability sets reject at exec time. The image (and the CI job) removes it
# with setcap; KWin then runs without real-time scheduling.
require_env XDG_RUNTIME_DIR XDG_DATA_HOME XDG_CONFIG_HOME \
    DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY DISPLAY XDG_SESSION_TYPE \
    XDG_CURRENT_DESKTOP KDE_FULL_SESSION KDE_SESSION_VERSION \
    QT_QPA_PLATFORM

start_session_services

if ! name_has_owner org.kde.KWin; then
    reset_wayland_session
fi
start_dbus_service org.kde.KWin "KWin" "$XDG_RUNTIME_DIR/kwin.log" \
    kwin_wayland --virtual --xwayland --socket "$WAYLAND_DISPLAY"
if ! wait_for_socket "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"; then
    echo "KWin did not create its Wayland socket" >&2
    show_log_tail "$XDG_RUNTIME_DIR/kwin.log"
    exit 1
fi

start_dbus_service org.kde.plasmashell "plasmashell" \
    "$XDG_RUNTIME_DIR/plasmashell.log" plasmashell

if ! wait_for_x; then
    echo "could not connect to Xwayland on $DISPLAY" >&2
    ls -l /tmp/.X11-unix 2>/dev/null >&2 || true
    show_log_tail "$XDG_RUNTIME_DIR/kwin.log"
    exit 1
fi
