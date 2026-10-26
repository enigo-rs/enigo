#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/../lib.sh"
. "$script_dir/../session.sh"
require_container
require_command gnome-shell
require_command gdbus
require_command gsettings
require_command xdpyinfo

# This script is used both as the devcontainer's postStartCommand and by CI.
# It starts a system bus and a session bus, a headless GNOME Shell Wayland
# session with Xwayland, and seeds the RemoteDesktop permission store with a
# restore token so the first portal connection does not wait for a permission
# dialog.
#
# The tests rotate the token and save it to ENIGO_RESTORE_TOKEN_FILE, so only
# the first connection in a fresh container needs the seed.

# xdg-desktop-portal 1.20 (Debian trixie) only reads
# XDG_DESKTOP_PORTAL_TEST_APP_ID. The APP_INFO_KIND/HOST_APPID pair is used by
# newer releases, so containerEnv sets all three: without an app ID the portal
# derives the empty ID and only the empty permission-store entry matches.
require_env XDG_RUNTIME_DIR XDG_DATA_HOME XDG_CONFIG_HOME \
    DBUS_SESSION_BUS_ADDRESS WAYLAND_DISPLAY DISPLAY XAUTHORITY \
    XDG_SESSION_TYPE XDG_CURRENT_DESKTOP GDK_BACKEND GTK_A11Y NO_AT_BRIDGE \
    ENIGO_RESTORE_TOKEN_FILE XDG_DESKTOP_PORTAL_TEST_APP_ID \
    XDG_DESKTOP_PORTAL_TEST_APP_INFO_KIND XDG_DESKTOP_PORTAL_TEST_HOST_APPID

if [ "$XAUTHORITY" != "$XDG_RUNTIME_DIR/.Xauthority" ]; then
    echo "refusing to replace XAUTHORITY=$XAUTHORITY; unset it or set it to $XDG_RUNTIME_DIR/.Xauthority" >&2
    exit 1
fi

TOKEN=11111111-1111-1111-1111-111111111111

start_session_services

# The input tests press every key and move the pointer to the hot corner.
# Disable the shortcuts that would open a modal over the session.
disable_gnome_shell_shortcuts

if ! name_has_owner org.gnome.Shell; then
    reset_wayland_session
    rm -f "$XAUTHORITY"
fi
start_dbus_service org.gnome.Shell "GNOME Shell" "$XDG_RUNTIME_DIR/gnome-shell.log" \
    gnome-shell --headless --wayland \
    --wayland-display="$WAYLAND_DISPLAY" \
    --virtual-monitor 1024x768

if ! wait_for_socket "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ||
    ! wait_for_name org.gnome.Mutter.RemoteDesktop; then
    echo "GNOME Shell did not expose its Wayland socket or RemoteDesktop name" >&2
    show_log_tail "$XDG_RUNTIME_DIR/gnome-shell.log"
    exit 1
fi

# GNOME Shell shows the overview late in startup. Hide it without simulating a
# key, which would race with the tests.
hide_gnome_overview

# Xwayland starts with the shell, but the socket and the X authority file may
# not be ready yet. Mutter writes the cookie to a random name in XDG_RUNTIME_DIR,
# so link it to the path clients expect and probe until the display accepts us.
_wait=0
while [ "$_wait" -lt 20 ]; do
    if [ ! -e "$XAUTHORITY" ]; then
        for auth_file in "$XDG_RUNTIME_DIR"/.mutter-Xwaylandauth.*; do
            if [ -f "$auth_file" ]; then
                ln -sfn "$auth_file" "$XAUTHORITY"
                break
            fi
        done
    fi
    timeout 1 xdpyinfo -display "$DISPLAY" >/dev/null 2>&1 && break
    sleep 0.2
    _wait=$((_wait + 1))
done
if ! timeout 2 xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then
    echo "could not connect to Xwayland on $DISPLAY (XAUTHORITY=$XAUTHORITY)" >&2
    ls -l /tmp/.X11-unix 2>/dev/null >&2 || true
    show_log_tail "$XDG_RUNTIME_DIR/gnome-shell.log"
    exit 1
fi

# The GNOME portal backend expects its restore data in this shape:
# (provider, version, (creation_time, last_used_time, devices, clipboard, streams))
# Devices is keyboard | pointer and the stream list is empty.
timeout 10 gdbus call --session \
    --dest org.freedesktop.impl.portal.PermissionStore \
    --object-path /org/freedesktop/impl/portal/PermissionStore \
    --method org.freedesktop.impl.portal.PermissionStore.Set \
    remote-desktop true "$TOKEN" \
    "{'': ['yes'], 'enigo-test': ['yes']}" \
    "<@(suv) ('GNOME', uint32 1, <@(xxuba(uuv)) (int64 0, int64 0, uint32 3, false, @a(uuv) [])>)>" \
    >/dev/null || {
    echo "could not seed the RemoteDesktop restore token" >&2
    exit 1
}

# The non-interactive Screenshot portal asks for permission before the first
# screenshot. Seed it as well, otherwise the tests wait for a dialog that
# nobody can answer in a headless session.
timeout 10 gdbus call --session \
    --dest org.freedesktop.impl.portal.PermissionStore \
    --object-path /org/freedesktop/impl/portal/PermissionStore \
    --method org.freedesktop.impl.portal.PermissionStore.Set \
    screenshot true screenshot \
    "{'': ['yes'], 'enigo-test': ['yes']}" \
    "<@a{sv} {}>" \
    >/dev/null || {
    echo "could not seed the Screenshot permission" >&2
    exit 1
}

printf '%s' "$TOKEN" >"$ENIGO_RESTORE_TOKEN_FILE" || {
    echo "could not write the restore token to $ENIGO_RESTORE_TOKEN_FILE" >&2
    exit 1
}
