use {KeyboardControllable, Key};
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum ParseError {
    /// When a tag doesn't exist.
    /// Example: {+TEST}{-TEST}
    ///            ^^^^   ^^^^
    UnknownTag(String),

    /// When a { is encountered inside a {TAG}.
    /// Example: {+HELLO{WORLD}
    ///                 ^
    UnexpectedOpen,

    /// When a { is never matched with a }.
    /// Example: {+SHIFT}Hello{-SHIFT
    ///                              ^
    UnmatchedOpen,

    /// Opposite of UnmatchedOpen.
    /// Example: +SHIFT}Hello{-SHIFT}
    ///         ^
    UnmatchedClose,
}
impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            ParseError::UnknownTag(_) => "Unknown tag",
            ParseError::UnexpectedOpen => "Unexpected { inside tag name",
            ParseError::UnmatchedOpen => "Unmatched {. No matching }",
            ParseError::UnmatchedClose => "Unmatched }. No matching {",
        }
    }
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

pub fn parse(enigo: &mut KeyboardControllable, string: &str) -> Result<(), ParseError> {
    let mut capture = None;

    let mut unicode = false;
    let mut escapeopen = false;
    let mut escapeclose = false;

    for c in string.chars() {
        if escapeopen {
            escapeopen = false;

            if c != '{' {
                if capture.is_none() {
                    capture = Some(String::with_capacity(1));
                } else {
                    return Err(ParseError::UnexpectedOpen);
                }
            }
        } else if c == '{' {
            escapeopen = true;
            continue;
        }

        if c == '}' {
            if escapeclose {
                escapeclose = false;
                continue;
            }

            if capture.is_some() {
                match capture.unwrap().as_str() {
                    "+SHIFT" => enigo.key_down(Key::Shift),
                    "-SHIFT" => enigo.key_up(Key::Shift),
                    "+CTRL" => enigo.key_down(Key::Control),
                    "-CTRL" => enigo.key_up(Key::Control),
                    "+UNICODE" => unicode = true,
                    "-UNICODE" => unicode = false,
                    string => return Err(ParseError::UnknownTag(string.to_string())),
                }
                capture = None;
                continue;
            } else {
                escapeclose = true;
            }
        } else if escapeclose {
            return Err(ParseError::UnmatchedClose);
        }

        if let Some(ref mut string) = capture {
            string.push(c);
        } else if unicode {
            enigo.key_sequence(c.to_string().as_str());
        } else {
            enigo.key_click(Key::Layout(c.to_string()));
        }
    }

    if escapeopen || capture.is_some() {
        return Err(ParseError::UnmatchedOpen);
    }
    if escapeclose {
        return Err(ParseError::UnmatchedClose);
    }
    Ok(())
}
