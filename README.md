[![Build status](https://img.shields.io/github/actions/workflow/status/enigo-rs/enigo/CI.yml?branch=master)](https://github.com/enigo-rs/enigo/actions/workflows/CI.yml)
[![Docs](https://docs.rs/enigo/badge.svg)](https://docs.rs/enigo)
[![Dependency status](https://deps.rs/repo/github/enigo-rs/enigo/status.svg)](https://deps.rs/repo/github/enigo-rs/enigo)

![Rust version](https://img.shields.io/badge/rust--version-1.64+-brightgreen.svg)
[![Crates.io](https://img.shields.io/crates/v/enigo.svg)](https://crates.io/crates/enigo)
[![Discord chat](https://img.shields.io/discord/315925376486342657.svg)](https://discord.gg/Eb8CsnN)
[![Gitter chat](https://badges.gitter.im/gitterHQ/gitter.png)](https://gitter.im/enigo-rs/Lobby)

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
- [x] Custom Parser

```Rust
let mut enigo = Enigo::new();

enigo.mouse_move_to(500, 200);
enigo.mouse_click(MouseButton::Left);
enigo.key_sequence_parse("{+CTRL}a{-CTRL}{+SHIFT}Hello World{-SHIFT}");
```

For more look at examples

## Runtime dependencies

Linux users may have to install `libxdo-dev`. For example, on Debian-based distros:

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