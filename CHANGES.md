# Unreleased

## Changed

## Added

## Fixed

# 0.1.2

## Changed
- Windows: Bumped `windows` dependency to `0.48` because `0.47` was yanked.

# 0.1.1

## Changed
- Windows: `Key::Control` presses `Control` and no longer the left `Control`.

## Added
- all: Added a ton of keys (e.g F21-F24 keys and the XBUTTON1 & XBUTTON2 mouse buttons are now available on Windows). Some of them are OS specific. Use conditional compilation (e.g `#[cfg(target_os = "windows")]`) to use them to not break the build on other OSs.
- examples: New example `platform_specific.rs` to demonstrate how to use keys/buttons that are platform specific

## Fixed
- macOS: Fixed entering Key::Layout

# 0.1.0
We should have bumped the minor version with the last release. Sorry about that. Have a look at the changes of 0.0.15 if you come from an earlier version.

# 0.0.15

## Changed
- Windows: `mouse_scroll_y` with a positive number scrolls down just like on the other platforms
- Windows: replaced `winapi` with the official `windows` crate
- Rust: Using Rust version 2021
- Rust: Minimum supported Rust version (MSRV) is set in Cargo.toml
- Rust: MSRV is 1.64
- macOS, Windows: Moved the functions `main_display_size` and `mouse_location` from `Enigo` to `MouseControllable`

## Added
- DSL: Additional ParseError variants to give better feedback what the problem was
- DSL: Additional keys
- All: Added support for F10-F20
- CI/CD: Github Workflows to make sure the code builds and the tests pass
- Traits: Added the functions `main_display_size` and `mouse_location` to `MouseControllable`
- Linux: Implemented the functions `main_display_size` and `mouse_location` for `MouseControllable`

## Fixed
- Windows: panicked at `cannot transmute_copy if U is larger than T` (https://github.com/enigo-rs/enigo/issues/121)
- Windows: Inconsistent behavior between the `mouse_move_relative` and `mouse_move_to` functions (https://github.com/enigo-rs/enigo/issues/91)
- Windows, macOS: Stop panicking when `mouse_down` or `mouse_up` is called with either of `MouseButton::ScrollUp`, `MouseButton::ScrollDown`, `MouseButton::ScrollLeft`, `MouseButton::ScrollRight` and instead scroll
- Windows: Always use key codes to be layout independent. Only use scan codes for `Key::Layout` (Fixes https://github.com/enigo-rs/enigo/issues/99, https://github.com/enigo-rs/enigo/issues/84)
- macOS: `key_click` no longer triggers a segmentation fault when called with `Key::Layout` argument (Fixes https://github.com/enigo-rs/enigo/issues/124)
- macOS: Double clicks now work (https://github.com/enigo-rs/enigo/issues/82)