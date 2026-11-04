use compact_str::CompactString;
use memchr::{memchr2_iter, memmem::Finder};

/// The search box's instant filter: a byte substring match on names that is
/// case-insensitive unless the query has an uppercase letter, optionally
/// narrowed to a set of names (the files a `tag:` filter matched).
#[derive(Debug, Clone)]
pub struct Filter {
    query: Box<str>,
    matcher: Option<Matcher>,
    names: Option<Box<[CompactString]>>,
}

#[derive(Debug, Clone)]
enum Matcher {
    Exact(Box<Finder<'static>>),
    FoldedAscii(Box<[u8]>),
}

impl Filter {
    /// `None` for an empty query, which filters nothing.
    pub fn new(query: &str) -> Option<Self> {
        (!query.is_empty()).then(|| Self {
            query: query.into(),
            matcher: Matcher::of(query),
            names: None,
        })
    }

    /// Admits only `names` among those `query` matches; an empty query
    /// admits every one of `names`.
    pub fn among(query: &str, mut names: Vec<CompactString>) -> Self {
        names.sort_unstable();
        Self {
            query: query.into(),
            matcher: Matcher::of(query),
            names: Some(names.into_boxed_slice()),
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn matches(&self, name: &[u8]) -> bool {
        let admitted = self.names.as_ref().is_none_or(|names| {
            names
                .binary_search_by(|known| known.as_bytes().cmp(name))
                .is_ok()
        });
        admitted
            && self
                .matcher
                .as_ref()
                .is_none_or(|matcher| matcher.matches(name))
    }
}

impl Matcher {
    fn of(query: &str) -> Option<Self> {
        if query.is_empty() {
            None
        } else if query.chars().any(char::is_uppercase) {
            Some(Self::Exact(Box::new(
                Finder::new(query.as_bytes()).into_owned(),
            )))
        } else {
            Some(Self::FoldedAscii(
                query.as_bytes().to_ascii_lowercase().into_boxed_slice(),
            ))
        }
    }

    fn matches(&self, name: &[u8]) -> bool {
        match self {
            Self::Exact(finder) => finder.find(name).is_some(),
            Self::FoldedAscii(needle) => contains_folded(name, needle),
        }
    }
}

impl PartialEq for Filter {
    fn eq(&self, other: &Self) -> bool {
        self.query == other.query && self.names == other.names
    }
}

impl Eq for Filter {}

fn contains_folded(haystack: &[u8], needle: &[u8]) -> bool {
    let Some(&first) = needle.first() else {
        return true;
    };
    memchr2_iter(first, first.to_ascii_uppercase(), haystack).any(|start| {
        haystack
            .get(start..start + needle.len())
            .is_some_and(|window| window.eq_ignore_ascii_case(needle))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(query: &str, name: &str) -> bool {
        Filter::new(query).is_some_and(|filter| filter.matches(name.as_bytes()))
    }

    #[test]
    fn lowercase_queries_ignore_case() {
        assert!(matches("report", "Q3 REPORT.pdf"));
        assert!(matches("q3", "Q3 report.pdf"));
        assert!(matches(".pdf", "a.PDF"));
    }

    #[test]
    fn an_uppercase_letter_makes_the_query_exact() {
        assert!(matches("Report", "Q3 Report.pdf"));
        assert!(!matches("Report", "q3 report.pdf"));
    }

    #[test]
    fn misses_and_partial_tails_do_not_match() {
        assert!(!matches("xyz", "abc"));
        assert!(!matches("abcd", "xxabc"));
        assert!(Filter::new("").is_none());
    }

    #[test]
    fn a_name_set_narrows_the_query() {
        let tagged = Filter::among("", vec!["b.txt".into(), "a.md".into()]);
        assert!(tagged.matches(b"a.md") && tagged.matches(b"b.txt"));
        assert!(!tagged.matches(b"c.md"));
        let both = Filter::among("md", vec!["b.txt".into(), "a.md".into()]);
        assert!(both.matches(b"a.md") && !both.matches(b"b.txt"));
        assert_ne!(tagged, Filter::among("", vec!["a.md".into()]));
        assert!(!Filter::among("", Vec::new()).matches(b"a.md"));
    }

    #[test]
    fn non_ascii_queries_match_their_bytes() {
        assert!(matches("été", "Photos été 2024"));
    }
}
