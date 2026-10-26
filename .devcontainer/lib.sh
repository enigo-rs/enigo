# Shared by the devcontainer scripts. They start services and change
# container-local state, so they must not run on the host by accident.

require_container() {
    if [ -f /.dockerenv ] ||
        [ -f /run/.containerenv ] ||
        [ -n "${container:-}" ] ||
        [ -n "${KUBERNETES_SERVICE_HOST:-}" ]; then
        return 0
    fi
    if grep -qaE '(docker|podman|libpod|containerd|kubepods)' /proc/1/cgroup 2>/dev/null; then
        return 0
    fi
    echo "refusing to run outside a container: this would start services and change container-local state" >&2
    return 1
}

require_command() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "required command '$1' is not installed" >&2
        return 1
    }
}

require_env() {
    # Fail before starting a session when its environment is missing. Accept
    # set-but-empty values such as WAYLAND_DISPLAY on the X11 sessions.
    for _require_env_name in "$@"; do
        eval "_require_env_value=\${$_require_env_name+x}"
        if [ -z "$_require_env_value" ]; then
            echo "required environment variable $_require_env_name is not set" >&2
            return 1
        fi
    done
}

show_log_tail() {
    if [ -f "$1" ]; then
        echo "--- $1" >&2
        tail -20 "$1" >&2
    fi
}
