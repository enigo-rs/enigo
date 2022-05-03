[![Crate](https://img.shields.io/crates/v/enigo.svg)](https://crates.io/crates/enigo)
[![docs.rs](https://docs.rs/enigo/badge.svg)](https://docs.rs/enigo)
[![dependency status](https://deps.rs/repo/github/pentamassiv/enigo/status.svg)](https://deps.rs/repo/github/pentamassiv/enigo)
[![Build_x86](https://img.shields.io/github/workflow/status/pentamassiv/enigo/Build_x86/main)](https://github.com/pentamassiv/enigo/actions/workflows/build_x86_64.yaml)
[![Build_aarch64](https://img.shields.io/github/workflow/status/pentamassiv/enigo/Build_aarch64/main)](https://github.com/pentamassiv/enigo/actions/workflows/build_aarch64.yaml)
![dependabot status](https://img.shields.io/badge/dependabot-enabled-025e8c?logo=Dependabot)
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

for more look at examples

Runtime dependencies
--------------------

Linux users may have to install libxdo-dev. For example, on Ubuntu:

```Bash
apt install libxdo-dev
```
On Arch: 

```Bash
pacman -S xdotool
```
