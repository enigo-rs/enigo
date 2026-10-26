#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/lib.sh"
. "$script_dir/session.sh"
require_container
require_command sway
require_command wlr-randr
require_command flock

# Start sway on container start and expose the socket under a stable name so
# WAYLAND_DISPLAY does not depend on which number sway picked. sway refuses to
# run as root, so it runs as the unprivileged user like the tests do.
require_env XDG_RUNTIME_DIR WAYLAND_DISPLAY XDG_SESSION_TYPE \
    XDG_CURRENT_DESKTOP ENIGO_BROWSER_DISPLAY
install -d -m 700 "$XDG_RUNTIME_DIR"
start_browser_display

start_wlroots_session sway sway -c "$script_dir/sway.conf"
