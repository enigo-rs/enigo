#!/bin/sh
# Temporary: log changes to the X input focus and to GNOME Shell's overview
# while the tests run. Read the log after a failing browser test and remove this
# script together with the workflow step that dumps it.
set -u

log=${XDG_RUNTIME_DIR:-/tmp}/focus-debug.log
: >"$log"

previous=
while :; do
    focus=$(xdotool getwindowfocus 2>/dev/null || echo none)
    name=$(xdotool getwindowfocus getwindowname 2>/dev/null || echo none)
    browser=$(xdotool search --onlyvisible --name "Enigo Universal Test" 2>/dev/null |
        tr '\n' ' ')
    ewmh=$(xprop -root _NET_ACTIVE_WINDOW 2>/dev/null || echo none)
    overview=$(timeout 2 gdbus call --session \
        --dest org.gnome.Shell --object-path /org/gnome/Shell \
        --method org.freedesktop.DBus.Properties.Get \
        org.gnome.Shell OverviewActive 2>/dev/null || echo unknown)
    state="focus=$focus name=$name browser=${browser:-none} overview=$overview $ewmh"
    if [ "$state" != "$previous" ]; then
        printf '%s %s\n' "$(date +%H:%M:%S)" "$state" >>"$log"
        previous=$state
    fi
    sleep 0.5
done
