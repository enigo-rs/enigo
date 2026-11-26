# Known issues

These were found while working on the devcontainers. They are recorded here but
intentionally not fixed in this PR.

## The browser integration test runs on the protocol of the backend

Each CI entry runs the browser test for its backend, and Firefox has to run on
the protocol that backend injects through: `GDK_BACKEND=wayland` for the portal
and wlroots backends, `GDK_BACKEND=x11` for `x11rb`, which talks to `DISPLAY`
(the compositor's Xwayland in a Wayland session). An X11 backend cannot reach a
Wayland window, so the two are never mixed. The wlroots sessions (`sway`,
`labwc`) additionally run `x11rb` against their own Xwayland, so that path stays
tested for those compositors; neither needs a separate Xvfb.

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

These Wayland runs are not excluded from CI, so they fail until the underlying
limitation is fixed. The `xdg_desktop` browser run skips absolute pointer
motion, which the backend does not implement.

## `xdo` adds Alt to function keys with xkeyboard-config 2.46

xkeyboard-config 2.46 added explicit `Control` and `Alt` entries for level 1 to
the `CTRL+ALT` key type that F1-F12 use. libxdo before 4.20260303.1 reads them
as "level 1 needs Alt", so `Key::F1` is sent as Alt+F1
([xdotool#491](https://github.com/jordansissel/xdotool/issues/491)). Debian and
Ubuntu still ship libxdo 3.20160805, so the Integration workflow builds the
fixed release for its `xdo` job.

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

Mutter 48 (the GNOME container) and KWin 6.3 (the Plasma container) set up
their EIS seat without `ei_text`, because Debian trixie's `libeis` predates
libei 1.6. The backend therefore cannot use `ei_text.utf8` or `ei_text.keysym`
and falls back to `ei_keyboard` plus the compositor's xkb keymap.

That fallback can only press keycodes that already exist in the active keymap
at the current shift level. Two test cases fail because of this, not because of
the container:

- `tests::keyboard::unit_key` fails on the first key that the default us layout
  does not map (`Key::Begin`; `Mode_switch`, `script_switch`, `Shift_Lock` and
  similar keysyms would fail as well) with
  `InvalidInput("failed to map the requested key to a keycode: the provided key is not mapped at the current shift level")`.
- `tests::keyboard::unit_text` fails on the first character that needs a shift
  level or is not in the keymap (`%` and uppercase letters need Shift; `𝕊`,
  emoji, CJK and Arabic are not in the us layout), with the same error.
  `text()` falls back to `key(Key::Unicode(c))` when `fast_text` returns
  `Ok(None)`.

The mouse tests (`unit_button_click`, `unit_10th_click`, `unit_scroll`,
`unit_mouse_drag`) and the other key paths do work against the real compositor.

The library fix is to use `ei_text` when the seat advertises it (already
implemented) or to synthesize a keymap/modifier sequence that can produce the
requested keysym instead of rejecting it. Until then, these two tests are
expected to fail on Mutter and on KWin. They are left enabled so the limitation
stays visible.

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
