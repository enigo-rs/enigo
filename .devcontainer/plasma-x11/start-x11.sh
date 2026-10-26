#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/../lib.sh"
. "$script_dir/../session.sh"
require_container
require_command kwin_x11
require_command plasmashell

# Start a normal headless KDE Plasma X11 session: a session and system bus,
# PipeWire, Xvfb, KWin as the X11 window manager and plasmashell.
require_env XDG_RUNTIME_DIR XDG_DATA_HOME XDG_CONFIG_HOME \
    DBUS_SESSION_BUS_ADDRESS DISPLAY XDG_SESSION_TYPE XDG_CURRENT_DESKTOP \
    KDE_FULL_SESSION KDE_SESSION_VERSION QT_QPA_PLATFORM

start_session_services
start_xvfb

start_dbus_service org.kde.KWin "KWin on $DISPLAY" \
    "$XDG_RUNTIME_DIR/kwin.log" kwin_x11 --replace
start_dbus_service org.kde.plasmashell "plasmashell" \
    "$XDG_RUNTIME_DIR/plasmashell.log" plasmashell
