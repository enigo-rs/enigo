[![Build Status](https://travis-ci.org/pythoneer/enigo.svg?branch=master)](https://travis-ci.org/pythoneer/enigo)
[![Build Status](https://ci.appveyor.com/api/projects/status/github/pythoneer/enigo)](https://ci.appveyor.com/project/pythoneer/enigo)
[![Crates.io](https://img.shields.io/crates/v/enigo.svg)](https://crates.io/crates/enigo)


# enigo
Cross platform input simulation in Rust

- [x] Linux X11 mouse
- [x] Linux X11 text
- [ ] Linux X11 keyboard DSL
- [ ] Linux Wayland mouse
- [ ] Linux Wayland text
- [ ] Linux Wayland keyboard DSL
- [x] macOS mouse
- [ ] macOS text
- [ ] macOS keyboard DSL
- [x] Win mouse
- [ ] Win text
- [ ] Win keyboard DSL


```Rust

let mut enigo = Enigo::new();

enigo.mouse_move_to(500, 200);
enigo.mouse_click(1);
//only on linux currently
enigo.key_sequence("Das → ❤ ist ein Hörz!!!");

```

for more look at examples
