pub fn parse(string: &str) {
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
                    "+SHIFT" => println!("\n> shift on"),
                    "-SHIFT" => println!("\n> shift off"),
                    _ => println!("\n> unknown"),
                    // TODO!!
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
            print!("{}", c);
            // TODO!!
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
