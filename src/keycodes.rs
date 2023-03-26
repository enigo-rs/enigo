/// A key on the keyboard.
/// For alphabetical keys, use [`Key::Layout`] for a system independent key.
/// If a key is missing, you can use the raw keycode with [`Key::Raw`]. Some of
/// the keys are only available on a specific platform. Use conditional
/// compilation to use them.
#[cfg_attr(feature = "with_serde", derive(Serialize, Deserialize))]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    /// alt key on Linux and Windows (option key on macOS)
    Alt,
    /// backspace key
    Backspace,
    /// caps lock key
    CapsLock,
    #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// command key on macOS (super key on Linux, windows key on Windows)
    Command,
    /// control key
    Control,
    /// delete key
    Delete,
    /// down arrow key
    DownArrow,
    /// end key
    End,
    /// escape key (esc)
    Escape,
    /// F1 key
    F1,
    /// F2 key
    F2,
    /// F3 key
    F3,
    /// F4 key
    F4,
    /// F5 key
    F5,
    /// F6 key
    F6,
    /// F7 key
    F7,
    /// F8 key
    F8,
    /// F9 key
    F9,
    /// F10 key
    F10,
    /// F11 key
    F11,
    /// F12 key
    F12,
    /// F13 key
    F13,
    /// F14 key
    F14,
    /// F15 key
    F15,
    /// F16 key
    F16,
    /// F17 key
    F17,
    /// F18 key
    F18,
    /// F19 key
    F19,
    /// F20 key
    F20,
    #[cfg(target_os = "windows")]
    /// F21 key
    F21,
    #[cfg(target_os = "windows")]
    /// F22 key
    F22,
    #[cfg(target_os = "windows")]
    /// F23 key
    F23,
    #[cfg(target_os = "windows")]
    /// F24 key
    F24,
    /// home key
    Home,
    /// left arrow key
    LeftArrow,
    /// meta key (also known as "windows", "super", and "command")
    Meta,
    /// option key on macOS (alt key on Linux and Windows)
    Option,
    /// page down key
    PageDown,
    /// page up key
    PageUp,
    /// return key
    Return,
    /// right arrow key
    RightArrow,
    /// shift key
    Shift,
    /// space key
    Space,
    #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// super key on linux (command key on macOS, windows key on Windows)
    Super,
    /// tab key (tabulator)
    Tab,
    /// up arrow key
    UpArrow,
    #[deprecated(since = "0.0.12", note = "now renamed to Meta")]
    /// windows key on Windows (super key on Linux, command key on macOS)
    Windows,
    /// keyboard layout dependent key
    Layout(char),
    /// raw keycode eg 0x38
    Raw(u16),
}
