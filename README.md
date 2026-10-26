[![Build status](https://img.shields.io/github/actions/workflow/status/enigo-rs/enigo/build.yml?branch=main)](https://github.com/enigo-rs/enigo/actions/workflows/build.yml)
[![Docs](https://docs.rs/enigo/badge.svg)](https://docs.rs/enigo)
[![Dependency status](https://deps.rs/repo/github/enigo-rs/enigo/status.svg)](https://deps.rs/repo/github/enigo-rs/enigo)

![Rust version](https://img.shields.io/badge/rust--version-1.93.1+-brightgreen.svg)
[![Crates.io](https://img.shields.io/crates/v/enigo.svg)](https://crates.io/crates/enigo)

# enigo

Cross platform input simulation in Rust!

- [x] Serialize/Deserialize
- [x] Linux (X11) mouse
- [x] Linux (X11) text
- [x] Linux (Wayland) mouse (Experimental)
- [x] Linux (Wayland) text (Experimental)
- [x] Linux (libei) mouse (Experimental)
- [x] Linux (libei) text (Experimental)
- [x] MacOS mouse
- [x] MacOS text
- [x] Windows mouse
- [x] Windows text

Enigo also works on *BSDs if they use X11 or Wayland. I don't have a machine to test it and there are no Github Action runners for it, so the BSD support is not explicitly listed.

```Rust
let mut enigo = Enigo::new(&Settings::default()).unwrap();

enigo.move_mouse(500, 200, Abs).unwrap();
enigo.button(Button::Left, Click).unwrap();
enigo.text("Hello World! here is a lot of text  ❤️").unwrap();
```

For more, look at the ([examples](examples)).

## Features

By default, enigo currently works on Windows, macOS and Linux (X11). If you want to be able to serialize and deserialize commands for enigo ([example](examples/serde.rs)), you need to activate the `serde` feature.

There are multiple ways how to simulate input on Linux and not all systems support everything. Enigo can also use wayland protocols and libei to simulate input but there are currently some bugs with it. That is why they are hidden behind feature flags.

On Windows, mouse coordinates and display size use physical pixels. Enigo temporarily switches the calling thread to per-monitor DPI awareness for those queries, so you do not need to change the process DPI awareness mode.

## Runtime dependencies

Linux users may have to install `libxdo-dev` if they are using the `xdo` feature. For example, on Debian-based distros:

```Bash
apt install libxdo-dev
```

On Arch:

```Bash
pacman -S xdotool
```

On Fedora:

```Bash
dnf install libX11-devel libxdo-devel
```

On Gentoo:

```Bash
emerge -a xdotool
```

## Permissions

Some platforms have security measures in place to prevent programs from entering keys or controlling the mouse. Have a look at the [permissions](Permissions.md) documentation to see what you need to do to allow it.

## Migrating from a previous version

Please have a look at our [changelog](CHANGES.md) to find out what you have to do, if you used a previous version.

## Debugging

If you encounter an issue and want to debug it, turn on log messages as described [here](DEBUGGING.md).


## Testing this crate

*Warning*: The tests will move the mouse, enter text, press keys and open some applications. Read the test cases before you run them so you know what to expect. It's best to close everything so that the tests don't mess with your system. Some of them run for a long time because they are intended to be run in the CI. Tests that share input state are serialized with [`serial_test`](https://crates.io/crates/serial_test), so you can run them normally:

```Bash
cargo test
```

The browser integration test needs the `serde` feature, Firefox and an X11
display. Every devcontainer installs Firefox and provides a dedicated Xvfb
display, so the test also runs in the Wayland containers:

```Bash
cargo test --features serde --test integration_browser_events -- --test-threads=1
```

The devcontainers compile one backend per session because backend selection is
still compile-time; see `KNOWN_ISSUES.md` for the runtime-selection goal.

| Session | Run |
| --- | --- |
| Xvfb (X11, default) | `cargo test` (x11rb) or `cargo test --no-default-features --features xdo` (xdo) |
| sway | `cargo test --no-default-features --features wayland --lib` |
| labwc | `cargo test --no-default-features --features wayland --lib` |
| GNOME Wayland (libei) | `cargo test --no-default-features --features libei,tokio,platform_specific --lib` |
| GNOME Wayland (xdg_desktop) | `cargo test --no-default-features --features xdg_desktop,tokio,platform_specific --lib` |
| GNOME Wayland (Xwayland, x11rb) | `cargo test --no-default-features --features x11rb --lib` |
| GNOME X11 (Xorg) | `cargo test --no-default-features --features x11rb --lib` |
| Plasma Wayland (Xwayland, x11rb) | `cargo test --no-default-features --features x11rb --lib` |
| Plasma X11 (Xorg) | `cargo test --no-default-features --features x11rb --lib` |

The containers run the tests as an unprivileged `enigo` user. The GNOME
Wayland container seeds a portal restore token so the first connection does not
wait for a permission dialog.

Set `ENIGO_RESTORE_TOKEN` or `ENIGO_RESTORE_TOKEN_FILE` to reuse a portal restore
token in the examples; the examples also accept `--restore-token <token>`.
