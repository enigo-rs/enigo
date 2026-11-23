/// Split `text` into UTF-8 chunks of at most `max_bytes` each.
///
/// Chunks never split a Unicode scalar value. If the next scalar needs more
/// than `max_bytes` (possible when `max_bytes < 4`), that scalar is still
/// emitted alone so iteration makes progress. `max_bytes == 0` yields nothing.
pub(super) fn utf8_chunks(text: &str, max_bytes: usize) -> impl Iterator<Item = &str> {
    std::iter::from_fn({
        let mut rest = text;
        move || {
            if rest.is_empty() || max_bytes == 0 {
                return None;
            }
            let mut end = rest.len().min(max_bytes);
            while end > 0 && !rest.is_char_boundary(end) {
                end -= 1;
            }
            if end == 0 {
                // Next scalar needs more than max_bytes; emit it alone.
                end = rest.chars().next().map_or(0, char::len_utf8);
                if end == 0 {
                    return None;
                }
            }
            let (chunk, next) = rest.split_at(end);
            rest = next;
            Some(chunk)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::utf8_chunks;

    fn chunks<'a>(text: &'a str, max_bytes: usize) -> Vec<&'a str> {
        utf8_chunks(text, max_bytes).collect()
    }

    #[test]
    fn empty_inputs() {
        assert!(chunks("", 0).is_empty());
        assert!(chunks("", 1).is_empty());
        assert!(chunks("", 4000).is_empty());
        assert!(chunks("hello", 0).is_empty());
        assert!(chunks("😀", 0).is_empty());
    }

    #[test]
    fn max_bytes_one_ascii() {
        assert_eq!(chunks("ab", 1), ["a", "b"]);
        assert_eq!(chunks("xyz", 1), ["x", "y", "z"]);
    }

    #[test]
    fn max_bytes_one_or_two_with_multibyte() {
        // é is U+00E9 → 2 bytes (C3 A9)
        assert_eq!(chunks("é", 1), ["é"]); // exceeds budget to make progress
        assert_eq!(chunks("éé", 2), ["é", "é"]);
        assert_eq!(chunks("aé", 2), ["a", "é"]); // 'a'+start of é doesn't fit → "a", then "é"

        // emoji is 4 bytes
        assert_eq!(chunks("😀", 1), ["😀"]);
        assert_eq!(chunks("😀", 2), ["😀"]);
        assert_eq!(chunks("😀x", 2), ["😀", "x"]);
    }

    #[test]
    fn emoji_and_ascii_at_cut_boundary() {
        // "hi" = 2 bytes, "😀" = 4 → total 6 before '!'
        assert_eq!(chunks("hi😀!", 5), ["hi", "😀!"]); // remainder "😀!" is exactly 5
        assert_eq!(chunks("hi😀!", 6), ["hi😀", "!"]);
        assert_eq!(chunks("hi😀!", 4), ["hi", "😀", "!"]);
        assert_eq!(chunks("a😀b😀c", 4), ["a", "😀", "b", "😀", "c"]);
        assert_eq!(chunks("a😀b😀c", 5), ["a😀", "b😀", "c"]);
    }

    #[test]
    fn three_byte_and_combining_marks() {
        // CJK ideographs are 3 bytes each in UTF-8
        assert_eq!(chunks("日本語", 3), ["日", "本", "語"]);
        assert_eq!(chunks("日本語", 5), ["日", "本", "語"]); // 5 is mid-second-char
        assert_eq!(chunks("日本語", 6), ["日本", "語"]);

        // combining acute accent (U+0301) is 2 bytes; stays with previous only
        // if they fit in the same budget — with max 1 they split
        let combining = "a\u{0301}"; // á
        assert_eq!(chunks(combining, 1), ["a", "\u{0301}"]);
        assert_eq!(chunks(combining, 3), [combining]);
    }

    #[test]
    fn unusual_max_bytes() {
        assert_eq!(chunks("abc", 3), ["abc"]);
        assert_eq!(chunks("abcdef", 3), ["abc", "def"]);
        assert_eq!(chunks("hi", 100), ["hi"]);
        assert_eq!(chunks("x", usize::MAX), ["x"]);
        // Exactly one byte over a clean ASCII split
        assert_eq!(chunks("abcd", 3), ["abc", "d"]);
        // Protocol-sized limit still works for mixed text
        let text = "hello 😀 日本語!";
        assert_eq!(chunks(text, 4000).concat(), text);
        assert_eq!(chunks(text, 4000), [text]);
    }
}
