use {Enigo, Key, KeyboardControllable};

pub(crate) fn parse(enigo: &mut KeyboardControllable, string: &str) {
    let mut capture = None;

    let mut escapeopen = false;
    let mut escapeclose = false;

    for c in string.chars() {
        if escapeopen {
            escapeopen = false;

            if c != '{' {
                if capture.is_none() {
                    capture = Some(String::with_capacity(1));
                }
            }
        } else {
            if c == '{' {
                escapeopen = true;
                continue;
            }
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
                    _ => {}
                }
                capture = None;
                continue;
            } else {
                escapeclose = true;
            }
        }

        if let Some(ref mut string) = capture {
            string.push(c);
        } else {
            enigo.key_click(Key::Layout(c.to_string()));
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test() {
        super::parse("{+SHIFT}{{+SHIFT}} Hello {{{{{enter}}}}} World {{-SHIFT}}{-SHIFT} lol");
    }
}
