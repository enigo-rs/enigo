[![Build Status](https://travis-ci.org/enigo-rs/enigo.svg?branch=master)](https://travis-ci.org/enigo-rs/enigo)
[![Build Status](https://ci.appveyor.com/api/projects/status/github/pythoneer/enigo)](https://ci.appveyor.com/project/pythoneer/enigo)
[![Dependency Status](https://dependencyci.com/github/pythoneer/enigo/badge)](https://dependencyci.com/github/pythoneer/enigo)
[![Docs](https://docs.rs/enigo/badge.svg)](https://docs.rs/enigo)
[![Crates.io](https://img.shields.io/crates/v/enigo.svg)](https://crates.io/crates/enigo)
[![Gitter chat](https://badges.gitter.im/gitterHQ/gitter.png)](https://gitter.im/enigo-rs/Lobby)


# enigo
Cross platform input simulation in Rust!

- [x] Linux X11 mouse
- [x] Linux X11 text
- [ ] Linux X11 keyboard DSL
- [ ] Linux Wayland mouse
- [ ] Linux Wayland text
- [ ] Linux Wayland keyboard DSL
- [x] macOS mouse
- [x] macOS text
- [ ] macOS keyboard DSL
- [x] Win mouse
- [ ] Win text
- [ ] Win keyboard DSL


```Rust

let mut enigo = Enigo::new();

enigo.mouse_move_to(500, 200);
enigo.mouse_click(1);
//only on linux and macos currently
enigo.key_sequence("hello world");

```

for more look at examples
