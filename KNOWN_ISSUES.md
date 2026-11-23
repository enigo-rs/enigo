# Known issues

These were found while working on the devcontainers. They are recorded here but
intentionally not fixed in this PR.

## The browser integration test runs once per protocol

In a Wayland session the browser test runs twice. The Wayland run launches
Firefox on Wayland (`GDK_BACKEND=wayland`, `MOZ_ENABLE_WAYLAND=1`) and uses the
compiled Wayland backend, so the events travel over the compositor's own
protocol (EIS through libei/xdg_desktop, or the wlroots virtual-input protocol).
The XWayland run launches Firefox with `GDK_BACKEND=x11` and uses `x11rb`
against `DISPLAY`, which is the compositor's Xwayland. There is no separate
Xvfb: both runs use the session's own compositor and window manager.

Wayland browser runs currently fail in some sessions for the same reasons as
the unit tests, or before them:

- GNOME Wayland with `libei`: the browser test enters text and keys that the
  Mutter EIS seat cannot map without `ei_text` (see below).
- GNOME Wayland with `xdg_desktop`: the headless GNOME portal accepts the
  keyboard and pointer requests (`notify_keyboard_keysym`, `notify_pointer_*`)
  but does not deliver them. This was checked with an X11 `xev` window that had
  the X input focus and still received no key events, so it is not a focus
  problem on the browser side. The Wayland run fails at the first key. This
  needs a follow-up investigation of the portal/headless session.
- Plasma Wayland: the KDE RemoteDesktop portal is unavailable because the
  headless KWin cannot provide a screencast stream.

The failing Wayland runs are marked expected-to-fail in CI so the failure stays
visible while the XWayland run and the wlroots suite still report their own
results. The `sway` and `labwc` markers predate the xkbcommon keymap refactor:
the wayland backend now compiles a default `us` keymap when the compositor
assigns none (sway) and sends modifier keys as key events in addition to the
`wl_keyboard` modifier state (labwc). Those markers can be removed once the
sessions have been re-run. The `xdg_desktop` browser run skips absolute pointer
motion, which the backend does not implement; the XWayland run is the reference
for the full browser test and passes against Mutter's Xwayland and KWin's
Xwayland.

## `xdg_desktop` limits in the headless GNOME container

The container runs GNOME Shell headless, so the `RemoteDesktop` and Screenshot
portals are available and keyboard, buttons, scrolling and `main_display()` are
tested against them. Absolute pointer dragging is still skipped:

- It needs a ScreenCast stream and `notify_pointer_motion_absolute()` is not
  wired up. The browser integration test therefore skips its absolute mouse
  moves for this backend.

That is a library limitation, not a container one. `libei` also does not
implement `main_display()`.

## `libei` cannot enter keys that are not in the active keymap

Mutter 48 (the version in Debian trixie and in the GNOME container) sets up its
EIS seat without the `ei_text` interface and does not advertise
`EIS_DEVICE_CAP_TEXT`. The backend therefore cannot use `ei_text.utf8` or
`ei_text.keysym` and falls back to `ei_keyboard` plus the xkb keymap that the
compositor sent.

That fallback can only press keycodes that already exist in the active keymap
at the current shift level. Two test cases fail because of this, not because of
the container:

- `tests::keyboard::unit_key` fails on the first key that the default us layout
  does not map (`Key::Begin`; `Mode_switch`, `script_switch`, `Shift_Lock` and
  similar keysyms would fail as well) with
  `InvalidInput("failed to map the requested key to a keycode: the provided key is not mapped at the current shift level")`.
- `tests::keyboard::unit_text` fails on the first character the keymap cannot
  produce (the special `𝕊` character, emoji, CJK and Arabic text as used by the
  test), with the same error. `text()` falls back to `key(Key::Unicode(c))`
  when `fast_text` returns `Ok(None)`.

The mouse tests (`unit_button_click`, `unit_10th_click`, `unit_scroll`,
`unit_mouse_drag`) and the other key paths do work against the real compositor.

The library fix is to use `ei_text` when the seat advertises it (already
implemented) or to synthesize a keymap/modifier sequence that can produce the
requested keysym instead of rejecting it. Until then, these two tests are
expected to fail on Mutter. They are left enabled so the limitation stays
visible.

## The KDE RemoteDesktop portal is unavailable in the headless Plasma container

KWin's screencast plugin needs OpenGL, but its virtual and windowed backends
only provide QPainter here. KWin therefore does not expose
`zkde_screencast_unstable_v1`, so the KDE RemoteDesktop portal cannot create a
session and the `libei` and `xdg_desktop` tests cannot connect. Nesting KWin in
Xvfb or another compositor does not help. A normal Plasma session with a GPU or
a VM is needed. `kwin_x11` on Xvfb does use OpenGL.

## Linux backend selection is compile-time

Cargo features decide which backends are compiled. `Enigo::new` tries the
compiled backends and keeps the ones that connect; methods then fall back in a
fixed order (`xdg_desktop`, `wayland`, `x11`, `libei`). A user therefore has to
enable the features for their session, and the async runtime for the portal
backends, instead of the library choosing the best backend that is available.

The goal is to compile all Linux backends and select one at runtime, so that
`Enigo::new` and the test suite work in every session without feature flags.
For now each devcontainer compiles the backend its session targets, so no
special build is needed. Enabling every backend today would also make the
`#[cfg_attr(..., ignore)]` cases for `location()` and `main_display()` disappear
from the run, because those attributes test the compiled feature set rather
than the backend actually selected. The test gating has to become runtime-aware
before all containers can use the same command.

## Portal version requirement

The restore-token seed and `ConnectToEIS` need `xdg-desktop-portal` 1.20 or
newer. Debian trixie (used by the container and CI) is new enough; Ubuntu 24.04
is not. Debian bookworm has 1.16, which has no restore tokens: the GNOME
`RemoteDesktop` backend then waits for a consent dialog that a headless session
cannot answer.

The backends now check the `RemoteDesktop` interface version before starting a
session. `libei` reports an error when `ConnectToEIS` is unavailable, because
there is no way to connect to the EIS without it. `xdg_desktop` logs a warning
and continues without the restore token when the portal is too old to support
one, so the portal asks for permission again instead of `Enigo::new` failing.
In a headless session that still means waiting for a dialog that nobody can
answer, but interactive users are not blocked by the version check.
