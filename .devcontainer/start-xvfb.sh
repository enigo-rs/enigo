#!/bin/sh
set -eu

script_dir=$(CDPATH= cd "$(dirname "$0")" && pwd)
. "$script_dir/lib.sh"
. "$script_dir/session.sh"
require_container

# Start the Xvfb X11 session. The session environment comes from
# devcontainer.json (containerEnv) or the CI setup action.
require_env XDG_RUNTIME_DIR DISPLAY

install -d -m 700 "$XDG_RUNTIME_DIR"
ensure_x11_unix_dir
start_xvfb
