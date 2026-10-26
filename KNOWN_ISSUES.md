# Known issues

These were found while working on the devcontainers. They are recorded here but
intentionally not fixed in this PR.

## A second `Enigo` can hang with `libei` or `xdg_desktop`

When `Enigo::new` is called outside an existing Tokio runtime, both backends
create a short-lived current-thread runtime for the portal calls and drop it
afterwards. `ashpd`/`zbus` keep background tasks on the runtime that created the
connection, so creating a second `Enigo` in the same process can hang.

This was observed while testing restore-token reuse. It should be fixed
separately; the tests intentionally do not work around it. Because each test
creates its own `Enigo`, the `libei` and `xdg_desktop` test suites can hang
after the first test until the library keeps a runtime alive or documents and
requires an ambient runtime. The CI jobs for those backends are experimental.

## `xdg_desktop` limits in the headless GNOME container

The container runs GNOME Shell headless, so the `RemoteDesktop` and Screenshot
portals are available and keyboard, buttons and scrolling are tested against
them. Two paths are skipped:

- `main_display()` takes a Screenshot portal screenshot, but the backend passes
the returned `file://` URI to `image::open` as a filesystem path. The URI needs
to be converted to a path first (for example with `Url::to_file_path`, which
would make `url` a regular dependency instead of a dev-dependency).
- Absolute pointer dragging needs a ScreenCast stream and
`notify_pointer_motion_absolute()` is not wired up.

Those are library limitations, not container ones. Testing `libei` does not have
the first limitation, but `main_display()` is not implemented there either.

## `libei` cannot enter keys that are not in the active keymap

Mutter 48 (the version in Debian trixie and in the GNOME container) sets up its
EIS seat without the `ei_text` interface and does not advertise
`EIS_DEVICE_CAP_TEXT`. The backend therefore cannot use `ei_text.utf8` or
`ei_text.keysym` and falls back to `ei_keyboard` plus the xkb keymap that the
compositor sent.

That fallback can only press keycodes that already exist in the active keymap
in levels 0 and 1. Two test cases fail because of this, not because of the
container:

- `tests::keyboard::unit_key` fails on the first key that the default us layout
  does not map (`Key::Begin`; `Mode_switch`, `script_switch`, `Shift_Lock` and
  similar keysyms would fail as well) with
  `InvalidInput("failed to map the requested key to a keycode: the provided key is not mapped in the current xkb keymap")`.
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

## The wayland backend needs a keymap from the compositor

The wayland backend starts from the xkb keymap that the compositor sends for
`wl_seat` and only falls back to `Keymap2::default()` when no keymap was
received. That fallback is still a stub (`DEFAULT_KEYMAP` is the string
`"xkb_keymap {"`), which xkbcommon rejects, so `update_keymap()` fails and
`Enigo::new` returns `EstablishCon("no successful connection")`. `Con::new`
also uses a fixed number of roundtrips and does not wait for a keymap event
that arrives after the seat capabilities when the seat only gains a keyboard
because of the virtual keyboard the backend just created.

This is visible in the headless sway container and in the CI `sway` job.
Sway deliberately does not configure a layout for virtual keyboards
(`sway_keyboard_configure` skips `sway_keyboard_set_layout` when
`input_device->is_virtual` is set), and when a client's virtual keyboard is
destroyed sway clears the seat's current keyboard. The session has no physical
keyboard, so there is never a keymap to start from and every test that calls
`Enigo::new` fails, including the mouse-only tests.

The library fix is to compile a real default keymap (for example the default
`us` rules with xkbcommon) instead of the stub, and/or to wait for the seat
keymap event before calling `update_keymap()`. The `labwc` container runs the
same backend against a compositor that assigns the seat keymap to every virtual
keyboard (`new_keyboard()` calls `wlr_keyboard_set_keymap()` and then
`wlr_seat_set_keyboard()`), so all wayland tests pass there. The sway container
is kept to keep this limitation visible.

## KWin pushes the pointer out of screen-edge corners

KWin treats the top-left corner as an active screen edge and moves the pointer
back into the screen by `ElectricBorderPushbackPixels` (default 1) after it
reaches the corner. A request to move to `(0, 0)` therefore reports `(1, 1)` in
the Plasma X11 container. `unit_move_mouse_to` and `unit_move_mouse_rel` assert
the exact corner and can fail there, depending on when KWin activates the
edge. Setting `ElectricBorderPushbackPixels=0` in `kwinrc` makes them pass, but
the container keeps KWin's defaults. The tests need to tolerate compositor-side
pointer adjustments at screen edges, or skip the exact-corner cases under KWin.

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
cannot answer, so `Enigo::new` hangs instead of failing.
