#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/lib.sh"
. "$script_dir/session.sh"
require_container
require_command labwc
require_command wlr-randr
require_command flock

# Start labwc on container start and expose the socket under a stable name so
# WAYLAND_DISPLAY does not depend on which number labwc picked. labwc runs as
# the unprivileged user like the tests do.
require_env XDG_RUNTIME_DIR WAYLAND_DISPLAY XDG_SESSION_TYPE \
    XDG_CURRENT_DESKTOP ENIGO_BROWSER_DISPLAY
install -d -m 700 "$XDG_RUNTIME_DIR"
start_browser_display

start_wlroots_session labwc labwc

# The tests expect the same 1024x768 virtual display that the X11 and GNOME
# containers use. labwc's headless output starts at 1280x720, so set the mode.
_wait=0
while [ "$_wait" -lt 20 ]; do
    if timeout 1 env XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" WAYLAND_DISPLAY="$WAYLAND_DISPLAY" \
        wlr-randr --output HEADLESS-1 --custom-mode 1024x768 >/dev/null 2>&1; then
        exit 0
    fi
    sleep 0.1
    _wait=$((_wait + 1))
done
echo "could not set HEADLESS-1 to 1024x768 (see $XDG_RUNTIME_DIR/labwc.log)" >&2
show_log_tail "$XDG_RUNTIME_DIR/labwc.log"
exit 1
