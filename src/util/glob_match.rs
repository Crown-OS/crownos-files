//! Shell-style `*`, `?` and `[...]` matching, byte-wise, as shared-mime-info
//! globs use it.

/// Whether `text` matches all of `pattern`.
pub fn glob_matches(pattern: &[u8], text: &[u8]) -> bool {
    let (mut pattern_at, mut text_at) = (0, 0);
    let mut backtrack: Option<(usize, usize)> = None;
    while text_at < text.len() {
        if let Some(advance) = step(&pattern[pattern_at.min(pattern.len())..], text[text_at]) {
            match advance {
                Step::Star => {
                    backtrack = Some((pattern_at + 1, text_at));
                    pattern_at += 1;
                }
                Step::Matched(width) => {
                    pattern_at += width;
                    text_at += 1;
                }
            }
            continue;
        }
        let Some((after_star, star_text)) = backtrack else {
            return false;
        };
        pattern_at = after_star;
        text_at = star_text + 1;
        backtrack = Some((after_star, text_at));
    }
    pattern[pattern_at.min(pattern.len())..]
        .iter()
        .all(|&byte| byte == b'*')
}

enum Step {
    Star,
    Matched(usize),
}

fn step(pattern: &[u8], byte: u8) -> Option<Step> {
    match *pattern.first()? {
        b'*' => Some(Step::Star),
        b'?' => Some(Step::Matched(1)),
        b'[' => match class_matches(pattern, byte) {
            Some((true, width)) => Some(Step::Matched(width)),
            Some((false, _)) => None,
            None => (byte == b'[').then_some(Step::Matched(1)),
        },
        literal => (literal == byte).then_some(Step::Matched(1)),
    }
}

/// Whether `byte` is in the class `pattern` opens, and the class's width;
/// `None` when the class is never closed.
fn class_matches(pattern: &[u8], byte: u8) -> Option<(bool, usize)> {
    let mut at = 1;
    let negated = matches!(pattern.get(at), Some(b'!' | b'^'));
    at += usize::from(negated);
    let mut found = false;
    let mut first = true;
    loop {
        let low = *pattern.get(at)?;
        if low == b']' && !first {
            return Some((found != negated, at + 1));
        }
        first = false;
        match (pattern.get(at + 1), pattern.get(at + 2)) {
            (Some(b'-'), Some(&high)) if high != b']' => {
                found |= (low..=high).contains(&byte);
                at += 3;
            }
            _ => {
                found |= low == byte;
                at += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, text: &str) -> bool {
        glob_matches(pattern.as_bytes(), text.as_bytes())
    }

    #[test]
    fn stars_and_questions() {
        assert!(matches("*.so.[0-9]*", "libfoo.so.6"));
        assert!(matches("*.so.[0-9]*", "libfoo.so.6.1"));
        assert!(!matches("*.so.[0-9]*", "libfoo.so"));
        assert!(matches("cachegrind.out*", "cachegrind.out.123"));
        assert!(matches("a?c", "abc"));
        assert!(!matches("a?c", "ac"));
        assert!(matches("*", ""));
    }

    #[test]
    fn classes_with_ranges_and_negation() {
        assert!(matches("*.anim[1-9j]", "x.anim7"));
        assert!(matches("*.anim[1-9j]", "x.animj"));
        assert!(!matches("*.anim[1-9j]", "x.animk"));
        assert!(matches("[!a]b", "cb"));
        assert!(!matches("[!a]b", "ab"));
        assert!(matches("[]]", "]"));
        assert!(matches("[", "["));
    }
}
