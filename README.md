[![Build status](https://img.shields.io/github/actions/workflow/status/enigo-rs/enigo/CI.yml?branch=main)](https://github.com/enigo-rs/enigo/actions/workflows/CI.yml)
[![Docs](https://docs.rs/enigo/badge.svg)](https://docs.rs/enigo)
[![Dependency status](https://deps.rs/repo/github/enigo-rs/enigo/status.svg)](https://deps.rs/repo/github/enigo-rs/enigo)

![Rust version](https://img.shields.io/badge/rust--version-1.71+-brightgreen.svg)
[![Crates.io](https://img.shields.io/crates/v/enigo.svg)](https://crates.io/crates/enigo)

# enigo

Cross platform input simulation in Rust!

- [x] Linux (X11) mouse
- [x] Linux (X11) text
- [ ] Linux (Wayland) mouse
- [ ] Linux (Wayland) text
- [x] MacOS mouse
- [x] MacOS text
- [x] Win mouse
- [x] Win text
- [x] Serialize/Deserialize

```Rust
let mut enigo = Enigo::new(&Settings::default()).unwrap();

enigo.move_mouse(500, 200, Abs).unwrap();
enigo.button(Button::Left, Click).unwrap();
enigo.text("Hello World! here is a lot of text  ❤️").unwrap();
```

For more look at the examples.

## Runtime dependencies

Linux users may have to install `libxdo-dev` if they are using `X11`. For example, on Debian-based distros:

```Bash
apt-get install libxdo-dev
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