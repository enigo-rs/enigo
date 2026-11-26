#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/../lib.sh"
. "$script_dir/../session.sh"
require_container
require_command kwin_wayland
require_command plasmashell
require_command xdpyinfo
require_command gdbus

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

configure_kwin

if ! name_has_owner org.kde.KWin; then
    reset_wayland_session
fi
# KWin gates restricted Wayland interfaces (org_kde_kwin_fake_input and friends)
# behind the client's X-KDE-Wayland-Interfaces desktop entry, which it resolves
# through the KSycoca service database. The headless container has no menu
# files, so that database is empty and the KDE RemoteDesktop portal is denied
# fake-input. Disable the permission check so the portal can bind fake_input
# and deliver the injected key and pointer events.
export KWIN_WAYLAND_NO_PERMISSION_CHECKS=1
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

# The KDE RemoteDesktop portal shows a consent dialog unless the calling app is
# "mega-authorized", and a headless session can never answer that dialog. Grant
# the test app ids the permission so RemoteDesktop.Start() proceeds without it.
timeout 10 gdbus call --session \
    --dest org.freedesktop.impl.portal.PermissionStore \
    --object-path /org/freedesktop/impl/portal/PermissionStore \
    --method org.freedesktop.impl.portal.PermissionStore.Set \
    kde-authorized true remote-desktop \
    "{'': ['yes'], 'enigo-test': ['yes']}" \
    "<@a{sv} {}>" \
    >/dev/null || {
    echo "could not seed the KDE RemoteDesktop mega-authorization" >&2
    exit 1
}
