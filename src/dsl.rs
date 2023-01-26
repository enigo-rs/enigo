use crate::{Key, KeyboardControllable};
use std::error::Error;
use std::fmt;

/// An error that can occur when parsing DSL
#[derive(Debug, PartialEq, Eq)]
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

    /// When {} is encountered with no tag
    /// Example: {+SHIFT}Hello{}
    ///                       ^^
    EmptyTag,

    /// When {UNICODE} is encountered without an action
    /// Use {+UNICODE} or {-UNICODE} to enable / disable unicode
    MissingUnicodeAction,
}
impl Error for ParseError {
    fn description(&self) -> &str {
        match *self {
            Self::UnknownTag(_) => "Unknown tag",
            Self::UnexpectedOpen => "Unescaped open bracket ({) found inside tag name",
            Self::UnmatchedOpen => "Unmatched open bracket ({). No matching close (})",
            Self::UnmatchedClose => "Unmatched close bracket (}). No previous open ({)",
            Self::EmptyTag => "Empty tag",
            Self::MissingUnicodeAction => "Missing unicode action. {+UNICODE} or {-UNICODE}",
        }
    }
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.description())
    }
}

/// Evaluate the DSL. This tokenizes the input and presses the keys.
pub fn eval<K>(enigo: &mut K, input: &str) -> Result<(), ParseError>
where
    K: KeyboardControllable,
{
    for token in tokenize(input)? {
        match token {
            Token::Sequence(buffer) => {
                for key in buffer.chars() {
                    enigo.key_click(Key::Layout(key));
                }
            }
            Token::Unicode(buffer) => enigo.key_sequence(&buffer),
            Token::KeyUp(key) => enigo.key_up(key),
            Token::KeyDown(key) => enigo.key_down(key),
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Sequence(String),
    Unicode(String),
    KeyUp(Key),
    KeyDown(Key),
}

fn tokenize(input: &str) -> Result<Vec<Token>, ParseError> {
    let mut unicode = false;

    let mut tokens = Vec::new();
    let mut buffer = String::new();
    let mut iter = input.chars().peekable();

    fn flush(tokens: &mut Vec<Token>, buffer: String, unicode: bool) {
        if !buffer.is_empty() {
            if unicode {
                tokens.push(Token::Unicode(buffer));
            } else {
                tokens.push(Token::Sequence(buffer));
            }
        }
    }

    while let Some(c) = iter.next() {
        if c == '{' {
            match iter.next() {
                Some('{') => buffer.push('{'),
                Some(mut c) => {
                    flush(&mut tokens, buffer, unicode);
                    buffer = String::new();

                    let mut tag = String::new();
                    loop {
                        tag.push(c);
                        match iter.next() {
                            Some('{') => match iter.peek() {
                                Some(&'{') => {
                                    iter.next();
                                    c = '{'
                                }
                                _ => return Err(ParseError::UnexpectedOpen),
                            },
                            Some('}') => match iter.peek() {
                                Some(&'}') => {
                                    iter.next();
                                    c = '}'
                                }
                                _ => break,
                            },
                            Some(new) => c = new,
                            None => return Err(ParseError::UnmatchedOpen),
                        }
                    }
                    let action = match tag.chars().next() {
                        Some(first) => match first {
                            '+' => Action::Down,
                            '-' => Action::Up,
                            _ => Action::Press,
                        },
                        None => return Err(ParseError::EmptyTag),
                    };
                    let key = if action != Action::Press {
                        &tag[1..]
                    } else {
                        &tag
                    };
                    if tag == "UNICODE" {
                        unicode = match action {
                            Action::Down => true,
                            Action::Up => false,
                            Action::Press => return Err(ParseError::MissingUnicodeAction),
                        };
                        continue;
                    }
                    tokens.append(&mut action.into_token(
                        match key {
                            "SHIFT" => Key::Shift,
                            "CTRL" => Key::Control,
                            "META" => Key::Meta,
                            "ALT" => Key::Alt,
                            "TAB" => Key::Tab,
                            "BACKSPACE" => Key::Backspace,
                            "CAPSLOCK" => Key::CapsLock,
                            "CONTROL" => Key::Control,
                            "DELETE" => Key::Delete,
                            "DEL" => Key::Delete,
                            "DOWNARROW" => Key::DownArrow,
                            "END" => Key::End,
                            "ESCAPE" => Key::Escape,
                            "F1" => Key::F1,
                            "F2" => Key::F2,
                            "F3" => Key::F3,
                            "F4" => Key::F4,
                            "F5" => Key::F5,
                            "F6" => Key::F6,
                            "F7" => Key::F7,
                            "F8" => Key::F8,
                            "F9" => Key::F9,
                            "F10" => Key::F10,
                            "F11" => Key::F11,
                            "F12" => Key::F12,
                            "HOME" => Key::Home,
                            "LEFTARROW" => Key::LeftArrow,
                            "OPTION" => Key::Option,
                            "PAGEDOWN" => Key::PageDown,
                            "PAGEUP" => Key::PageUp,
                            "RETURN" => Key::Return,
                            "RIGHTARROW" => Key::RightArrow,
                            "UPARROW" => Key::UpArrow,
                            _ => return Err(ParseError::UnknownTag(tag)),
                        }
                    ));
                }
                None => return Err(ParseError::UnmatchedOpen),
            }
        } else if c == '}' {
            match iter.next() {
                Some('}') => buffer.push('}'),
                _ => return Err(ParseError::UnmatchedClose),
            }
        } else {
            buffer.push(c);
        }
    }

    flush(&mut tokens, buffer, unicode);

    Ok(tokens)
}

#[derive(Debug, PartialEq)]
enum Action {
    Down,
    Up,
    Press,
}

impl Action {
    pub fn into_token(&self, key: Key) -> Vec<Token> {
        match self {
            Self::Down => vec![Token::KeyDown(key)],
            Self::Up => vec![Token::KeyUp(key)],
            Self::Press => vec![
                Token::KeyDown(key),
                Token::KeyUp(key),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success() {
        assert_eq!(
            tokenize("{{Hello World!}} {+CTRL}hi{-CTRL}"),
            Ok(vec![
                Token::Sequence("{Hello World!} ".into()),
                Token::KeyDown(Key::Control),
                Token::Sequence("hi".into()),
                Token::KeyUp(Key::Control),
            ])
        );
        assert_eq!(
            tokenize("{+CTRL}f{-CTRL}hi{RETURN}"),
            Ok(vec![
                Token::KeyDown(Key::Control),
                Token::Sequence("f".into()),
                Token::KeyUp(Key::Control),
                Token::Sequence("hi".into()),
                Token::KeyDown(Key::Return),
                Token::KeyUp(Key::Return),
            ])
        );
    }

    #[test]
    fn unexpected_open() {
        assert_eq!(tokenize("{hello{}world}"), Err(ParseError::UnexpectedOpen));
    }

    #[test]
    fn unmatched_open() {
        assert_eq!(
            tokenize("{this is going to fail"),
            Err(ParseError::UnmatchedOpen)
        );
    }

    #[test]
    fn unmatched_close() {
        assert_eq!(
            tokenize("{+CTRL}{{this}} is going to fail}"),
            Err(ParseError::UnmatchedClose)
        );
    }
}
