use std::borrow::Cow;

use crate::{
    mime::table::MimeTable,
    model::mime_id::MimeId,
    util::{glob_match::glob_matches, hash::FastHashMap},
};

const CASE_SENSITIVE_FLAG: &str = "cs";
const NO_GLOBS: &str = "__NOGLOBS__";
const GLOB_SPECIALS: &[char] = &['*', '?', '['];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GlobHit {
    mime: MimeId,
    weight: u8,
    length: u16,
}

impl GlobHit {
    fn outranks(self, other: Option<Self>) -> bool {
        other.is_none_or(|other| (self.weight, self.length) > (other.weight, other.length))
    }
}

#[derive(Debug, Default)]
struct PatternMaps {
    literals: FastHashMap<Box<str>, GlobHit>,
    suffixes: FastHashMap<Box<str>, GlobHit>,
    /// The first bytes of every suffix, so a lookup only probes names where
    /// one could start.
    suffix_starts: [u64; 4],
}

impl PatternMaps {
    fn mark_suffix_start(&mut self, byte: u8) {
        self.suffix_starts[usize::from(byte >> 6)] |= 1 << (byte & 63);
    }

    fn may_start_suffix(&self, byte: u8) -> bool {
        self.suffix_starts[usize::from(byte >> 6)] & (1 << (byte & 63)) != 0
    }

    fn best_suffix(&self, name: &str, best: &mut Option<GlobHit>) {
        for (at, byte) in name.bytes().enumerate() {
            if !self.may_start_suffix(byte) {
                continue;
            }
            if let Some(&hit) = name.get(at..).and_then(|suffix| self.suffixes.get(suffix))
                && hit.outranks(*best)
            {
                *best = Some(hit);
            }
        }
    }
}

/// The `globs2` patterns of shared-mime-info, split into hash lookups for
/// literal names and `*suffix` patterns so classifying a name costs a few
/// probes rather than a scan over 1500 patterns.
#[derive(Debug, Default)]
pub struct Globs {
    folded: PatternMaps,
    exact: PatternMaps,
    full: Vec<(Box<str>, GlobHit, bool)>,
}

impl Globs {
    /// Adds the lines of one `globs2` file. Files are added most important
    /// first, and a pattern keeps the first (highest weighted) type given.
    pub fn add(&mut self, text: &str) {
        for line in text.lines().filter(|line| !line.starts_with('#')) {
            let mut fields = line.split(':');
            let (Some(weight), Some(mime), Some(pattern)) =
                (fields.next(), fields.next(), fields.next())
            else {
                continue;
            };
            let Ok(weight) = weight.parse::<u8>() else {
                continue;
            };
            if pattern.is_empty() || pattern == NO_GLOBS {
                continue;
            }
            let case_sensitive = fields
                .next()
                .is_some_and(|flags| flags.split(',').any(|flag| flag == CASE_SENSITIVE_FLAG));
            let hit = GlobHit {
                mime: MimeTable::intern(mime),
                weight,
                length: u16::try_from(pattern.len()).unwrap_or(u16::MAX),
            };
            self.add_pattern(pattern, hit, case_sensitive);
        }
    }

    fn add_pattern(&mut self, pattern: &str, hit: GlobHit, case_sensitive: bool) {
        let pattern: Cow<'_, str> = if case_sensitive {
            Cow::Borrowed(pattern)
        } else {
            Cow::Owned(pattern.to_lowercase())
        };
        let maps = if case_sensitive {
            &mut self.exact
        } else {
            &mut self.folded
        };
        if !pattern.contains(GLOB_SPECIALS) {
            maps.literals.entry(pattern.into()).or_insert(hit);
            return;
        }
        match pattern.strip_prefix('*') {
            Some(suffix) if !suffix.is_empty() && !suffix.contains(GLOB_SPECIALS) => {
                maps.mark_suffix_start(suffix.as_bytes()[0]);
                maps.suffixes.entry(suffix.into()).or_insert(hit);
            }
            _ => self.full.push((pattern.into(), hit, case_sensitive)),
        }
    }

    /// The type a file named `name` most likely has: a literal name wins,
    /// then the heaviest, longest matching pattern.
    pub fn lookup(&self, name: &str) -> Option<MimeId> {
        if let Some(hit) = self.exact.literals.get(name) {
            return Some(hit.mime);
        }
        let folded = fold_case(name);
        if let Some(hit) = self.folded.literals.get(folded.as_ref()) {
            return Some(hit.mime);
        }
        let mut best = None;
        self.exact.best_suffix(name, &mut best);
        self.folded.best_suffix(&folded, &mut best);
        for (pattern, hit, case_sensitive) in &self.full {
            let subject = if *case_sensitive { name } else { &folded };
            if hit.outranks(best) && glob_matches(pattern.as_bytes(), subject.as_bytes()) {
                best = Some(*hit);
            }
        }
        best.map(|hit| hit.mime)
    }
}

fn fold_case(name: &str) -> Cow<'_, str> {
    if name.chars().any(char::is_uppercase) {
        Cow::Owned(name.to_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GLOBS: &str = "\
# comment
80:text/x-c++src:*.C:cs
55:application/x-compressed-tar:*.tar.gz
50:text/x-csrc:*.c:cs
50:application/gzip:*.gz
50:text/x-makefile:makefile
50:text/x-readme:readme*
50:application/x-sharedlib:*.so.[0-9]*
50:text/x-ts:*.ts
45:video/mp2t:*.ts
50:application/x-trash:*~
";

    fn globs() -> Globs {
        let mut globs = Globs::default();
        globs.add(GLOBS);
        globs
    }

    fn name_of(globs: &Globs, file: &str) -> Option<&'static str> {
        globs.lookup(file).and_then(MimeTable::name)
    }

    #[test]
    fn the_longest_suffix_and_case_rules_decide() {
        let globs = globs();
        assert_eq!(
            name_of(&globs, "src.tar.gz"),
            Some("application/x-compressed-tar")
        );
        assert_eq!(name_of(&globs, "notes.GZ"), Some("application/gzip"));
        assert_eq!(name_of(&globs, "main.c"), Some("text/x-csrc"));
        assert_eq!(name_of(&globs, "main.C"), Some("text/x-c++src"));
        assert_eq!(name_of(&globs, "backup~"), Some("application/x-trash"));
    }

    #[test]
    fn literals_full_globs_and_weights() {
        let globs = globs();
        assert_eq!(name_of(&globs, "Makefile"), Some("text/x-makefile"));
        assert_eq!(name_of(&globs, "README.first"), Some("text/x-readme"));
        assert_eq!(
            name_of(&globs, "libz.so.1"),
            Some("application/x-sharedlib")
        );
        assert_eq!(name_of(&globs, "index.ts"), Some("text/x-ts"));
        assert_eq!(name_of(&globs, "LICENSE"), None);
    }
}
