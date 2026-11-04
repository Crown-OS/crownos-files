use compact_str::CompactString;

/// The tags of one file, in the order they were written.
pub type Tags = Vec<CompactString>;

const SEPARATOR: char = ',';
const ESCAPE: char = '\\';

/// Splits a `user.xdg.tags` value on unescaped commas; `\,` is a comma and
/// `\\` a backslash inside a tag. Blank tags are dropped.
pub fn decode_tags(raw: &[u8]) -> Tags {
    let text = String::from_utf8_lossy(raw);
    let mut tags = Tags::new();
    let mut current = CompactString::default();
    let mut chars = text.chars();
    while let Some(char) = chars.next() {
        match char {
            ESCAPE => current.extend(chars.next()),
            SEPARATOR => push_trimmed(&mut tags, &mut current),
            _ => current.push(char),
        }
    }
    push_trimmed(&mut tags, &mut current);
    tags
}

/// The `user.xdg.tags` value for `tags`, escaping commas and backslashes.
pub fn encode_tags<S: AsRef<str>>(tags: &[S]) -> String {
    let mut encoded = String::new();
    for (index, tag) in tags.iter().enumerate() {
        if index > 0 {
            encoded.push(SEPARATOR);
        }
        for char in tag.as_ref().chars() {
            if matches!(char, SEPARATOR | ESCAPE) {
                encoded.push(ESCAPE);
            }
            encoded.push(char);
        }
    }
    encoded
}

fn push_trimmed(tags: &mut Tags, current: &mut CompactString) {
    let tag = current.trim();
    if !tag.is_empty() && !tags.iter().any(|known| known == tag) {
        tags.push(tag.into());
    }
    current.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_commas_and_backslashes_and_round_trips() {
        let tags = ["work", "a, b", "back\\slash", "plain"];
        let encoded = encode_tags(&tags);
        assert_eq!(encoded, "work,a\\, b,back\\\\slash,plain");
        assert_eq!(decode_tags(encoded.as_bytes()), tags);
    }

    #[test]
    fn decoding_trims_drops_blanks_and_duplicates() {
        assert_eq!(decode_tags(b" red , ,blue,red,"), ["red", "blue"]);
        assert!(decode_tags(b"").is_empty());
        assert_eq!(decode_tags(b"trailing\\"), ["trailing"]);
        assert_eq!(decode_tags(b"caf\xc3\xa9"), ["café"]);
    }
}
