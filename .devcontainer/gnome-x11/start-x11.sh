#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/../lib.sh"
. "$script_dir/../session.sh"
require_container
require_command gnome-shell

# Start a normal headless GNOME X11 session: a session and system bus, PipeWire,
# Xvfb and GNOME Shell as the X11 window manager. The tests use the x11rb
# backend against Xvfb.
require_env XDG_RUNTIME_DIR XDG_DATA_HOME XDG_CONFIG_HOME \
    DBUS_SESSION_BUS_ADDRESS DISPLAY XDG_SESSION_TYPE XDG_CURRENT_DESKTOP \
    GDK_BACKEND GTK_A11Y NO_AT_BRIDGE ENIGO_RESTORE_TOKEN_FILE \
    XDG_DESKTOP_PORTAL_TEST_APP_ID XDG_DESKTOP_PORTAL_TEST_APP_INFO_KIND \
    XDG_DESKTOP_PORTAL_TEST_HOST_APPID ENIGO_BROWSER_DISPLAY

start_session_services
start_xvfb

start_dbus_service org.gnome.Shell "GNOME Shell on $DISPLAY" \
    "$XDG_RUNTIME_DIR/gnome-shell.log" gnome-shell --x11
