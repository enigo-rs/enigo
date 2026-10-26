#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/../lib.sh"
. "$script_dir/../session.sh"
require_container
require_command gnome-shell
require_command gdbus
require_command gsettings

# Start a normal headless GNOME X11 session: a session and system bus, PipeWire,
# Xvfb and GNOME Shell as the X11 window manager. The tests use the x11rb
# backend against Xvfb.
require_env XDG_RUNTIME_DIR XDG_DATA_HOME XDG_CONFIG_HOME \
    DBUS_SESSION_BUS_ADDRESS DISPLAY XDG_SESSION_TYPE XDG_CURRENT_DESKTOP \
    GDK_BACKEND GTK_A11Y NO_AT_BRIDGE ENIGO_RESTORE_TOKEN_FILE \
    XDG_DESKTOP_PORTAL_TEST_APP_ID XDG_DESKTOP_PORTAL_TEST_APP_INFO_KIND \
    XDG_DESKTOP_PORTAL_TEST_HOST_APPID

start_session_services
start_xvfb

# The input tests press every key and move the pointer to the hot corner.
# Disable the shortcuts that would open a modal over the session.
disable_gnome_shell_shortcuts

start_dbus_service org.gnome.Shell "GNOME Shell on $DISPLAY" \
    "$XDG_RUNTIME_DIR/gnome-shell.log" gnome-shell --x11

# GNOME Shell shows the overview late in startup. Hide it without simulating a
# key, which would race with the tests.
hide_gnome_overview
