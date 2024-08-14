[![Build status](https://img.shields.io/github/actions/workflow/status/enigo-rs/enigo/CI.yml?branch=main)](https://github.com/enigo-rs/enigo/actions/workflows/CI.yml)
[![Docs](https://docs.rs/enigo/badge.svg)](https://docs.rs/enigo)
[![Dependency status](https://deps.rs/repo/github/enigo-rs/enigo/status.svg)](https://deps.rs/repo/github/enigo-rs/enigo)

![Rust version](https://img.shields.io/badge/rust--version-1.75+-brightgreen.svg)
[![Crates.io](https://img.shields.io/crates/v/enigo.svg)](https://crates.io/crates/enigo)

# enigo

Cross platform input simulation in Rust!

- [x] Serialize/Deserialize
- [x] Linux (X11) mouse
- [x] Linux (X11) text
- [x] Linux (Wayland) mouse
- [x] Linux (Wayland) text
- [x] Linux (libei) mouse
- [x] Linux (libei) text
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

If you do not want your users to have to install any runtime dependencies on Linux when using X11, you can try the experimental `x11rb` feature.


## Runtime dependencies

Linux users may have to install `libxdo-dev` if they are using `X11`. For example, on Debian-based distros:

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

## Migrating from a previous version

Please have a look at our [changelog](CHANGES.md) to find out what you have to do, if you used a previous version.

## Permissions

Some platforms have security measures in place to prevent programs from entering keys or controlling the mouse. Have a look at the [permissions](Permissions.md) documentation to see what you need to do to allow it.