use memchr::{memchr2_iter, memmem::Finder};

/// The search box's instant filter: a byte substring match on names that is
/// case-insensitive unless the query has an uppercase letter.
#[derive(Debug, Clone)]
pub struct Filter {
    query: Box<str>,
    matcher: Matcher,
}

#[derive(Debug, Clone)]
enum Matcher {
    Exact(Box<Finder<'static>>),
    FoldedAscii(Box<[u8]>),
}

impl Filter {
    /// `None` for an empty query, which filters nothing.
    pub fn new(query: &str) -> Option<Self> {
        if query.is_empty() {
            return None;
        }
        let matcher = if query.chars().any(char::is_uppercase) {
            Matcher::Exact(Box::new(Finder::new(query.as_bytes()).into_owned()))
        } else {
            Matcher::FoldedAscii(query.as_bytes().to_ascii_lowercase().into_boxed_slice())
        };
        Some(Self {
            query: query.into(),
            matcher,
        })
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn matches(&self, name: &[u8]) -> bool {
        match &self.matcher {
            Matcher::Exact(finder) => finder.find(name).is_some(),
            Matcher::FoldedAscii(needle) => contains_folded(name, needle),
        }
    }
}

impl PartialEq for Filter {
    fn eq(&self, other: &Self) -> bool {
        self.query == other.query
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
    fn non_ascii_queries_match_their_bytes() {
        assert!(matches("été", "Photos été 2024"));
    }
}
