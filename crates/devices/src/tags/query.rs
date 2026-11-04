use std::borrow::Cow;

use compact_str::CompactString;

const TAG_PREFIX: &str = "tag:";
const QUOTE: char = '"';

/// A search box query split into `tag:` filters and the remaining text.
/// `tag:work`, `tag:"two words"`; every tag must match.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchQuery<'a> {
    pub tags: Vec<Cow<'a, str>>,
    pub text: Cow<'a, str>,
}

impl<'a> SearchQuery<'a> {
    pub fn parse(query: &'a str) -> Self {
        let mut tags = Vec::new();
        let mut words: Vec<&str> = Vec::new();
        let mut rest = query.trim_start();
        while !rest.is_empty() {
            if let Some(after) = rest.strip_prefix(TAG_PREFIX) {
                let (tag, remaining) = take_word(after);
                if !tag.is_empty() {
                    tags.push(Cow::Borrowed(tag));
                }
                rest = remaining;
            } else {
                let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                words.push(&rest[..end]);
                rest = &rest[end..];
            }
            rest = rest.trim_start();
        }
        let text = match words.as_slice() {
            [] => Cow::Borrowed(""),
            [word] => Cow::Borrowed(*word),
            _ => Cow::Owned(words.join(" ")),
        };
        Self { tags, text }
    }

    pub fn has_tags(&self) -> bool {
        !self.tags.is_empty()
    }

    /// Whether a file with `file_tags` passes every tag filter, ignoring case.
    pub fn matches_tags(&self, file_tags: &[CompactString]) -> bool {
        self.tags.iter().all(|wanted| {
            file_tags
                .iter()
                .any(|tag| tag.to_lowercase() == wanted.to_lowercase())
        })
    }
}

/// A bare word, or a quoted one, and what follows it.
fn take_word(text: &str) -> (&str, &str) {
    if let Some(quoted) = text.strip_prefix(QUOTE) {
        return match quoted.split_once(QUOTE) {
            Some((word, rest)) => (word, rest),
            None => (quoted, ""),
        };
    }
    let end = text.find(char::is_whitespace).unwrap_or(text.len());
    text.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_tag_filters_from_text() {
        let query = SearchQuery::parse("  report tag:work tag:\"two words\" final ");
        assert_eq!(query.tags, ["work", "two words"]);
        assert_eq!(query.text, "report final");
        assert!(matches!(
            SearchQuery::parse("one").text,
            Cow::Borrowed("one")
        ));
        let bare = SearchQuery::parse("tag: tag:\"open");
        assert_eq!(bare.tags, ["open"]);
        assert_eq!(bare.text, "");
        assert!(!SearchQuery::parse("plain words").has_tags());
    }

    #[test]
    fn every_tag_must_match_ignoring_case() {
        let query = SearchQuery::parse("tag:Work tag:urgent");
        let tags: Vec<CompactString> = vec!["work".into(), "URGENT".into(), "x".into()];
        assert!(query.matches_tags(&tags));
        assert!(!query.matches_tags(&tags[..1]));
        assert!(SearchQuery::parse("text").matches_tags(&[]));
    }
}
