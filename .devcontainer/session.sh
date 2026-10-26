# Shared helpers for the devcontainer session scripts.
#
# Source this after lib.sh and after exporting the session environment
# (XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS, DISPLAY/WAYLAND_DISPLAY). D-Bus
# activated services, including the portals, inherit that environment from the
# session bus, so it has to be exported before start_session_bus.

run_as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo "$@"
    else
        echo "root privileges are required but sudo is not installed" >&2
        return 1
    fi
}

dbus_ping() {
    timeout 2 dbus-send --bus="$DBUS_SESSION_BUS_ADDRESS" \
        --dest=org.freedesktop.DBus --type=method_call --print-reply \
        / org.freedesktop.DBus.Peer.Ping >/dev/null 2>&1
}

name_has_owner() {
    timeout 2 dbus-send --bus="$DBUS_SESSION_BUS_ADDRESS" \
        --dest=org.freedesktop.DBus --type=method_call --print-reply \
        / org.freedesktop.DBus.NameHasOwner "string:$1" 2>/dev/null |
        grep -q true
}

system_name_has_owner() {
    timeout 2 dbus-send --system \
        --dest=org.freedesktop.DBus --type=method_call --print-reply \
        / org.freedesktop.DBus.NameHasOwner "string:$1" 2>/dev/null |
        grep -q true
}

start_session_bus() {
    require_command dbus-daemon
    require_command dbus-send
    if dbus_ping; then return 0; fi
    case "$DBUS_SESSION_BUS_ADDRESS" in
        unix:path=*)
            path=${DBUS_SESSION_BUS_ADDRESS#unix:path=}
            rm -f "${path%%,*}"
            ;;
    esac
    dbus-daemon --session --fork --nopidfile --address="$DBUS_SESSION_BUS_ADDRESS" || {
        echo "could not start the session bus at $DBUS_SESSION_BUS_ADDRESS" >&2
        return 1
    }
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        dbus_ping && return 0
        sleep 0.1
    done
    echo "could not connect to the session bus at $DBUS_SESSION_BUS_ADDRESS" >&2
    return 1
}

start_system_bus() {
    require_command dbus-daemon
    require_command dbus-send
    if system_name_has_owner org.freedesktop.DBus; then return 0; fi
    run_as_root install -d -m 755 /run/dbus
    run_as_root rm -f /run/dbus/pid /run/dbus/system_bus_socket
    run_as_root dbus-daemon --system --fork --nopidfile || {
        echo "could not start the system bus" >&2
        return 1
    }
    for _ in 1 2 3 4 5 6 7 8 9 10; do
        system_name_has_owner org.freedesktop.DBus && return 0
        sleep 0.1
    done
    echo "could not connect to the system bus" >&2
    return 1
}

hide_systemd_seats() {
    # GNOME Shell and Plasma pick their login manager by checking whether
    # /run/systemd/seats exists. If it is there but systemd-logind is not on
    # the bus, they crash while constructing the real login manager. Hiding the
    # directory makes them use the dummy manager.
    if [ -d /run/systemd/seats ] && ! system_name_has_owner org.freedesktop.login1; then
        run_as_root rm -rf /run/systemd/seats.enigo-hidden
        run_as_root mv /run/systemd/seats /run/systemd/seats.enigo-hidden
    fi
}

start_pipewire() {
    if ! command -v pipewire >/dev/null 2>&1; then return 0; fi
    # The socket is per XDG_RUNTIME_DIR, so another session's PipeWire (for
    # example when two session scripts run in one container) must not count.
    if [ ! -S "$XDG_RUNTIME_DIR/pipewire-0" ]; then
        rm -f "$XDG_RUNTIME_DIR/pipewire-0" "$XDG_RUNTIME_DIR/pipewire-0.lock" 2>/dev/null || true
        nohup pipewire >"$XDG_RUNTIME_DIR/pipewire.log" 2>&1 9>&- &
    fi
    # The compositors connect to PipeWire while they start (KWin during its
    # screencast init), so wait for the socket instead of racing the daemon.
    if ! wait_for_socket "$XDG_RUNTIME_DIR/pipewire-0"; then
        echo "could not start PipeWire (see $XDG_RUNTIME_DIR/pipewire.log)" >&2
        show_log_tail "$XDG_RUNTIME_DIR/pipewire.log"
        return 1
    fi
}

ensure_x11_unix_dir() {
    # The X server needs /tmp/.X11-unix. It is normally provided by
    # systemd-tmpfiles, which containers do not run. Repair a mode left behind
    # by an earlier run as well.
    if [ ! -d /tmp/.X11-unix ]; then
        run_as_root install -d -m 1777 /tmp/.X11-unix
    fi
    run_as_root chmod 1777 /tmp/.X11-unix
}

start_session_services() {
    # Everything a desktop session needs before the compositor starts: the
    # runtime directories, the session and system buses, PipeWire and the X11
    # socket directory. Sway and labwc start only the compositor, so they do not
    # use this.
    install -d -m 700 "$XDG_RUNTIME_DIR"
    if [ -n "${XDG_DATA_HOME:-}" ]; then
        install -d -m 700 "$XDG_DATA_HOME"
    fi
    if [ -n "${XDG_CONFIG_HOME:-}" ]; then
        install -d -m 700 "$XDG_CONFIG_HOME"
    fi
    start_session_bus
    start_system_bus
    hide_systemd_seats
    start_pipewire
    ensure_x11_unix_dir
}

wait_for_name() {
    _wait=0
    while [ "$_wait" -lt 20 ]; do
        name_has_owner "$1" && return 0
        sleep 0.2
        _wait=$((_wait + 1))
    done
    echo "timed out waiting for D-Bus name $1" >&2
    return 1
}

start_dbus_service() {
    # Start a process that owns a D-Bus name and wait for that name. Skip the
    # start when the name is already owned, so session scripts are idempotent.
    _name=$1
    _description=$2
    _log=$3
    shift 3
    if name_has_owner "$_name"; then return 0; fi
    nohup "$@" >"$_log" 2>&1 9>&- &
    if ! wait_for_name "$_name"; then
        echo "could not start $_description (see $_log)" >&2
        show_log_tail "$_log"
        return 1
    fi
}

wait_for_socket() {
    _wait=0
    while [ "$_wait" -lt 100 ]; do
        [ -S "$1" ] && return 0
        sleep 0.1
        _wait=$((_wait + 1))
    done
    echo "timed out waiting for socket $1" >&2
    return 1
}

find_wlroots_socket() {
    # Return the basename of a live wlroots Wayland socket other than the
    # stable symlink, so a stale socket from an earlier run is not picked.
    for socket in "$XDG_RUNTIME_DIR"/wayland-*; do
        if [ -S "$socket" ] &&
            [ "${socket##*/}" != "$WAYLAND_DISPLAY" ] &&
            timeout 1 env XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
                WAYLAND_DISPLAY="${socket##*/}" wlr-randr >/dev/null 2>&1; then
            echo "${socket##*/}"
            return 0
        fi
    done
    return 1
}

start_wlroots_session() {
    # Start a headless wlroots compositor and expose its socket under
    # WAYLAND_DISPLAY. The first argument names the log and lock file.
    _name=$1
    shift
    if ! socket=$(find_wlroots_socket); then
        (
            flock 9
            if ! socket=$(find_wlroots_socket); then
                nohup env -u WAYLAND_DISPLAY \
                    XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
                    WLR_BACKENDS=headless \
                    WLR_LIBINPUT_NO_DEVICES=1 \
                    WLR_RENDERER=pixman \
                    "$@" >"$XDG_RUNTIME_DIR/$_name.log" 2>&1 9>&- &
                _wait=0
                while [ "$_wait" -lt 50 ]; do
                    socket=$(find_wlroots_socket) && break
                    sleep 0.1
                    _wait=$((_wait + 1))
                done
            fi
        ) 9>"$XDG_RUNTIME_DIR/$_name.lock"
    fi
    socket=$(find_wlroots_socket) || true
    if [ -z "$socket" ]; then
        echo "could not start $_name (see $XDG_RUNTIME_DIR/$_name.log)" >&2
        show_log_tail "$XDG_RUNTIME_DIR/$_name.log"
        return 1
    fi
    ln -sfn "$socket" "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
}

wait_for_x() {
    require_command xdpyinfo
    _wait=0
    while [ "$_wait" -lt 20 ]; do
        timeout 1 xdpyinfo -display "$DISPLAY" >/dev/null 2>&1 && return 0
        sleep 0.2
        _wait=$((_wait + 1))
    done
    echo "could not connect to X server on $DISPLAY" >&2
    return 1
}

reset_x_display() {
    # Remove a stale lock and socket before an X server or Xwayland takes the
    # display. Only local displays are supported by the session scripts.
    display=${1%%.*}
    case "$display" in
        :*) ;;
        *)
            echo "DISPLAY must be a local display such as :99, got '$1'" >&2
            return 1
            ;;
    esac
    display_number=${display#:}
    case "$display_number" in
        ''|*[!0-9]*)
            echo "invalid DISPLAY '$1'" >&2
            return 1
            ;;
    esac
    rm -f "/tmp/.X$display_number-lock" "/tmp/.X11-unix/X$display_number"
}

reset_wayland_session() {
    # A compositor that is not running cannot own these sockets; remove them so
    # the next instance can use the configured names.
    rm -f "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.lock"
    reset_x_display "$DISPLAY"
}

start_xvfb() {
    require_command Xvfb
    require_command xdpyinfo
    if timeout 2 xdpyinfo -display "$DISPLAY" >/dev/null 2>&1; then return 0; fi
    display=${DISPLAY%%.*}
    reset_x_display "$DISPLAY"
    nohup Xvfb "$display" -screen 0 1024x768x24 -nolisten tcp \
        >"$XDG_RUNTIME_DIR/xvfb.log" 2>&1 9>&- &
    if ! wait_for_x; then
        echo "could not start Xvfb on $DISPLAY (see $XDG_RUNTIME_DIR/xvfb.log)" >&2
        show_log_tail "$XDG_RUNTIME_DIR/xvfb.log"
        return 1
    fi
}

disable_gnome_shell_shortcuts() {
    # The input tests press every key. Super opens the overview and Print opens
    # the screenshot UI; both grab the keyboard and would swallow the events of
    # the browser test. The pointer also starts in the top-left hot corner,
    # which the mouse tests visit. Disable these shortcuts instead of
    # dismissing their modals with simulated keys.
    gsettings set org.gnome.desktop.interface enable-hot-corners false
    gsettings set org.gnome.mutter overlay-key ''
    gsettings set org.gnome.shell.keybindings show-screenshot-ui "[]"
    gsettings set org.gnome.shell.keybindings screenshot "[]"
    gsettings set org.gnome.shell.keybindings screenshot-window "[]"
}

hide_gnome_overview() {
    # GNOME Shell starts in the overview and can show it late in startup. There
    # is no gsettings key to suppress it, and Escape is one of the keys under
    # test, so hide it over D-Bus instead of simulating a key. Wait for the
    # overview to appear first, otherwise the shell reopens it after this
    # function has hidden it.
    _wait=0
    while [ "$_wait" -lt 50 ]; do
        overview=$(timeout 2 gdbus call --session \
            --dest org.gnome.Shell \
            --object-path /org/gnome/Shell \
            --method org.freedesktop.DBus.Properties.Get \
            org.gnome.Shell OverviewActive 2>/dev/null || true)
        case "$overview" in
            *true*) break ;;
        esac
        sleep 0.1
        _wait=$((_wait + 1))
    done

    _wait=0
    while :; do
        if timeout 2 gdbus call --session \
            --dest org.gnome.Shell \
            --object-path /org/gnome/Shell \
            --method org.freedesktop.DBus.Properties.Set \
            org.gnome.Shell OverviewActive "<false>" >/dev/null 2>&1 &&
            timeout 2 gdbus call --session \
                --dest org.gnome.Shell \
                --object-path /org/gnome/Shell \
                --method org.freedesktop.DBus.Properties.Get \
                org.gnome.Shell OverviewActive 2>/dev/null | grep -q false; then
            break
        fi
        _wait=$((_wait + 1))
        if [ "$_wait" -ge 50 ]; then
            echo "could not hide the GNOME Shell overview" >&2
            return 1
        fi
        sleep 0.1
    done
}
