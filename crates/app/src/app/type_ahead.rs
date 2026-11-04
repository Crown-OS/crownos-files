use std::time::{Duration, Instant};

/// How long after the last key a new key starts a new prefix.
const RESET_AFTER: Duration = Duration::from_millis(900);

/// Typed letters that jump to the first entry starting with them.
#[derive(Debug, Default)]
pub struct TypeAhead {
    typed: String,
    last_key: Option<Instant>,
}

impl TypeAhead {
    /// Adds `character` typed at `now` and returns the prefix to look for.
    pub fn push(&mut self, character: char, now: Instant) -> &str {
        if self
            .last_key
            .is_none_or(|last| now.duration_since(last) > RESET_AFTER)
        {
            self.typed.clear();
        }
        self.last_key = Some(now);
        self.typed.extend(character.to_lowercase());
        &self.typed
    }
}

/// Whether `name` starts with the lowercase `prefix`, ignoring case.
pub fn starts_with_folded(name: &str, prefix: &str) -> bool {
    let mut folded = name.chars().flat_map(char::to_lowercase);
    prefix
        .chars()
        .all(|expected| folded.next() == Some(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_keys_build_a_prefix_and_a_pause_starts_over() {
        let mut type_ahead = TypeAhead::default();
        let start = Instant::now();
        assert_eq!(type_ahead.push('K', start), "k");
        assert_eq!(
            type_ahead.push('e', start + Duration::from_millis(200)),
            "ke"
        );
        assert_eq!(type_ahead.push('r', start + Duration::from_secs(3)), "r");
        assert!(starts_with_folded("Keynote.pdf", "key"));
        assert!(!starts_with_folded("Brand", "bx"));
        assert!(!starts_with_folded("B", "br"));
    }
}
