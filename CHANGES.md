# Unreleased
## Changed
- Rust: MSRV is 1.82

## Added

## Removed

## Fixed

# 0.3.0
## Changed
- all: The keys `Print` and `Snapshot` were deprecated because `Print` had the wrong virtual key associated with it on Windows. Use `Key::PrintScr` instead
- macOS: The simulated input is no longer effected by the state of the physical keyboard. This default behavior can be changed via the Settings (`independent_of_keyboard_state`)
- macOS: Do not coalesce mouse events to allow precise control
- win: Fallback to entering a `Key::Unicode` as unicode if there is no key with that character
- macOS: Removed setting and functions related to the delay on macOS because a sleep is no longer necessary

## Added
- all: `Key::PrintScr`
- all: There finally are some tests in the CI to increase the development speed and prevent regressions
- win, macOS: Allow marking events that were created by enigo. Have a look at the additional field of the `Settings` struct and the new method `get_marker_value` of the `Enigo` struct (only available on Windows and macOS)
- macOS: Fallback to ASCII-capable keyboard layout for handling non-standard input sources
- macOS: Check if the application has the necessary permissions. If they are missing, `enigo` will ask the user to grant them. You can change this default behavior with the `Settings` when constructing an `Enigo` struct.
- all: Added `Token::Location` and `Token::MainDisplay` mostly for debugging purposes. They allow you to check if your expectations are correct
- win: Added a setting to take the mouse speed and acceleration level into account for relative mouse movement or reliably move the mouse to the expected location.

## Removed

## Fixed
- macOS: No more sleeps!! (Only when the `Enigo` struct is dropped) ([#105](https://github.com/enigo-rs/enigo/issues/105))
- win: Respect the language of the current window to determine which scancodes to send
- win: Send the virtual key and its scan code in the events to work with programs that only look at one of the two
- macOS: Moving the mouse no longer breaks simulating input
- win: Fix being unable to enter text containing multiple newline chars
- macOS: Switched keycodes of `Key::Launchpad` and `Key::MissionControl`
- macOS: `CapsLock` works ([#163](https://github.com/enigo-rs/enigo/issues/163))
- macOS: Moving the mouse works also if it is the only function ([#182](https://github.com/enigo-rs/enigo/issues/182))
- linux: libei: Typing is much faster now

# 0.2.1
## Changed
- all: Use serde(default) to make the serialized strings less verbose

## Added
- all: Serialized tokens can be less verbose because serde aliases were added
- win, macOS: Allow marking events that were created by enigo. Have a look at the additional field of the `Settings` struct and the new method `get_marker_value` of the `Enigo` struct (only available on Windows and macOS)
- all: The enums `Button`, `Direction`, `Axis` and `Coordinate` implement `Default`

## Fixed
- windows: The `move_mouse` function moves the mouse to the correct absolute coordinates again

# 0.2.0

## Changed
- All: A new Enigo struct is now always created with some settings
- Rust: MSRV is 1.75
- All held keys are released when Enigo is dropped
- win: Don't panic if it was not possible to move the mouse
- All: Never panic! All functions return Results now
- win: Don't move the mouse to a relative position if it was not possible to get the current position
- All: The feature `with_serde` was renamed to `serde`
- All: Renamed `Key::Layout(char)` to `Key::Unicode(char)` and clarified its docs
- All: Split off entering raw keycodes into it's own function
- All: Renamed `key_sequence` function to `text`
- All: Renamed `enter_key` function to `key`
- All: Renamed `send_mouse_button_event` function to `button`
- All: Renamed `send_motion_notify_event` function to `move_mouse`
- All: Renamed `mouse_scroll_event` function to `scroll`
- All: Renamed `mouse_location` function to `location`
- All: Renamed `MouseButton` enum to `Button`
- DSL: The DSL was removed and replaced with the `Agent` trait. Activate the `serde` feature to use it. Have a look at the `serde` example to get an idea how to use it

## Added
- Linux: Partial support for `libei` was added. Use the experimental feature `libei` to test it. This works on GNOME 46 and above. Entering text often simulates the wrong characters.
- Linux: Support X11 without `xdotools`. Use the experimental feature `x11rb` to test it
- Linux: Partial support for Wayland was added. Use the experimental feature `wayland` to test it. Only the virtual_keyboard and input_method protocol can be used. This is not going to work on GNOME, but should work for example with phosh
- Linux: Added `MicMute` key to enter `XF86_AudioMicMute` keysym
- win: Use DirectInput in addition to the SetCursorPos function in order to support DirectX
- All: You can now chose how long the delay between keypresses should be on each platform and change it during the runtime
- All: You can now use a logger to investigate errors

## Fixed
- *BSD: Fix the build for BSDs
- macOS: Add info how much a mouse was moved relative to the last position
- macOS: A mouse drag with the right key is now possible too
- win, linux: `key_sequence()` and  `key_click(Key::Layout())` can properly enter new lines and tabs
- linux: You can enter `Key::ScrollLock` now
- win: No more sleeps! Simulating input is done in 1 ms instead of 40+ ms. This is most obvious when entering long strings
- macOS: Added keys to control media, brightness, contrast, illumination and more
- macOS: Fix entering text that starts with newline characters

# 0.1.3

## Changed

## Added
- Linux: Add Media and Volume keys

## Fixed
- Linux: Fixed a Segfault when running in release mode

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
