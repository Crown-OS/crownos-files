//! The binary `magic` file of shared-mime-info and content matching.

use memchr::memmem;

use crate::{mime::table::MimeTable, model::mime_id::MimeId};

const HEADER: &[u8] = b"MIME-Magic\0\n";
const DEFAULT_PRIORITY: u8 = 50;

#[derive(Debug)]
struct Rule {
    indent: u32,
    offset: usize,
    range: usize,
    value: Box<[u8]>,
    mask: Option<Box<[u8]>>,
}

impl Rule {
    fn matches(&self, data: &[u8]) -> bool {
        let length = self.value.len();
        if length == 0 {
            return data.len() >= self.offset;
        }
        let end = (self.offset + self.range + length - 1).min(data.len());
        let Some(window) = data.get(self.offset..end) else {
            return false;
        };
        match &self.mask {
            None => memmem::find(window, &self.value).is_some(),
            Some(mask) => window.windows(length).any(|candidate| {
                candidate
                    .iter()
                    .zip(mask.iter().zip(self.value.iter()))
                    .all(|(byte, (mask, value))| byte & mask == value & mask)
            }),
        }
    }

    fn extent(&self) -> usize {
        self.offset + self.range + self.value.len()
    }
}

#[derive(Debug)]
struct Entry {
    priority: u8,
    mime: MimeId,
    rules: Vec<Rule>,
}

/// Content rules ordered by priority, the first match winning.
#[derive(Debug, Default)]
pub struct Magic {
    entries: Vec<Entry>,
}

impl Magic {
    /// Adds one `magic` file; returns `false` when it is not one. Entries
    /// parsed before a malformed one are kept.
    pub fn add(&mut self, bytes: &[u8]) -> bool {
        let Some(mut rest) = bytes.strip_prefix(HEADER) else {
            return false;
        };
        while let Some((entry, after)) = parse_entry(rest) {
            self.entries.push(entry);
            rest = after;
        }
        self.entries
            .sort_by_key(|entry| std::cmp::Reverse(entry.priority));
        true
    }

    /// How many leading bytes of a file the rules can look at.
    pub fn extent(&self) -> usize {
        self.entries
            .iter()
            .flat_map(|entry| &entry.rules)
            .map(Rule::extent)
            .max()
            .unwrap_or(0)
    }

    pub fn lookup(&self, data: &[u8]) -> Option<MimeId> {
        self.entries
            .iter()
            .find(|entry| any_matches(&entry.rules, 0, data))
            .map(|entry| entry.mime)
    }
}

/// Whether one of the sibling rules at `depth` heading `rules` matches with
/// at least one of its children (or has none).
fn any_matches(rules: &[Rule], depth: u32, data: &[u8]) -> bool {
    let mut at = 0;
    while let Some(rule) = rules.get(at) {
        let children = rules[at + 1..]
            .iter()
            .take_while(|child| child.indent > depth)
            .count();
        let subtree = &rules[at + 1..at + 1 + children];
        if rule.matches(data) && (subtree.is_empty() || any_matches(subtree, depth + 1, data)) {
            return true;
        }
        at += 1 + children;
    }
    false
}

fn parse_entry(input: &[u8]) -> Option<(Entry, &[u8])> {
    let input = input.strip_prefix(b"[")?;
    let close = memchr::memchr(b']', input)?;
    let header = std::str::from_utf8(&input[..close]).ok()?;
    let (priority, mime) = header.split_once(':')?;
    let mime = mime.split(':').next()?;
    let mut rest = input[close + 1..].strip_prefix(b"\n")?;
    let mut rules = Vec::new();
    while !rest.is_empty() && rest[0] != b'[' {
        let (rule, after) = parse_rule(rest)?;
        rules.push(rule);
        rest = after;
    }
    let entry = Entry {
        priority: priority.parse().unwrap_or(DEFAULT_PRIORITY),
        mime: MimeTable::intern(mime),
        rules,
    };
    Some((entry, rest))
}

/// `[indent]>offset=<u16 length><value>[&<mask>][~word][+range]\n`
fn parse_rule(input: &[u8]) -> Option<(Rule, &[u8])> {
    let (indent, rest) = number(input);
    let rest = rest.strip_prefix(b">")?;
    let (offset, rest) = number(rest);
    let rest = rest.strip_prefix(b"=")?;
    let (length, rest) = rest.split_first_chunk::<2>()?;
    let length = usize::from(u16::from_be_bytes(*length));
    let (value, mut rest) = rest.split_at_checked(length)?;
    let mut mask = None;
    if let Some(after) = rest.strip_prefix(b"&") {
        let (bytes, after) = after.split_at_checked(length)?;
        mask = Some(bytes);
        rest = after;
    }
    let mut word_size = 1;
    if let Some(after) = rest.strip_prefix(b"~") {
        let (size, after) = number(after);
        word_size = size.unwrap_or(1);
        rest = after;
    }
    let mut range = 1;
    if let Some(after) = rest.strip_prefix(b"+") {
        let (length, after) = number(after);
        range = length.unwrap_or(1).max(1);
        rest = after;
    }
    let newline = memchr::memchr(b'\n', rest)?;
    let rule = Rule {
        indent: u32::try_from(indent.unwrap_or(0)).ok()?,
        offset: offset?,
        range,
        value: host_order(value, word_size),
        mask: mask.map(|mask| host_order(mask, word_size)),
    };
    Some((rule, &rest[newline + 1..]))
}

/// Values are stored big-endian; a multi-byte word compares in host order.
fn host_order(bytes: &[u8], word_size: usize) -> Box<[u8]> {
    let mut owned: Box<[u8]> = bytes.into();
    if cfg!(target_endian = "little") && matches!(word_size, 2 | 4) {
        owned.chunks_exact_mut(word_size).for_each(<[u8]>::reverse);
    }
    owned
}

fn number(input: &[u8]) -> (Option<usize>, &[u8]) {
    let digits = input
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    let value = std::str::from_utf8(&input[..digits])
        .ok()
        .and_then(|digits| digits.parse().ok());
    (value, &input[digits..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(indent: &str, offset: usize, value: &[u8], suffix: &[u8]) -> Vec<u8> {
        let mut line = format!("{indent}>{offset}=").into_bytes();
        line.extend_from_slice(&u16::try_from(value.len()).unwrap().to_be_bytes());
        line.extend_from_slice(value);
        line.extend_from_slice(suffix);
        line.push(b'\n');
        line
    }

    fn database() -> Magic {
        let mut file = HEADER.to_vec();
        file.extend_from_slice(b"[90:image/x-magic-test-png]\n");
        file.extend(rule("", 0, b"\x89PNG", b""));
        file.extend_from_slice(b"[60:application/x-magic-test-nested]\n");
        file.extend(rule("", 0, b"NEST", b""));
        file.extend(rule("1", 8, b"OK", b""));
        file.extend_from_slice(b"[50:text/x-magic-test-ranged]\n");
        file.extend(rule("", 0, b"needle", b"+32"));
        file.extend_from_slice(b"[40:application/x-magic-test-masked]\n");
        file.extend(rule("", 0, b"\xF0\x00", b"&\xF0\x00"));
        let mut magic = Magic::default();
        assert!(magic.add(&file));
        magic
    }

    fn name_of(magic: &Magic, data: &[u8]) -> Option<&'static str> {
        magic.lookup(data).and_then(MimeTable::name)
    }

    #[test]
    fn offsets_children_ranges_and_masks() {
        let magic = database();
        assert_eq!(
            name_of(&magic, b"\x89PNG\r\n"),
            Some("image/x-magic-test-png")
        );
        assert_eq!(
            name_of(&magic, b"NEST....OK"),
            Some("application/x-magic-test-nested")
        );
        assert_eq!(name_of(&magic, b"NEST....NO"), None);
        assert_eq!(
            name_of(&magic, b"hay hay needle"),
            Some("text/x-magic-test-ranged")
        );
        assert_eq!(
            name_of(&magic, b"\xF7\x00"),
            Some("application/x-magic-test-masked")
        );
        assert_eq!(name_of(&magic, b"\x0F\x00"), None);
        assert_eq!(magic.extent(), 32 + 6);
    }

    #[test]
    fn rejects_files_without_the_header() {
        assert!(!Magic::default().add(b"not magic"));
    }
}
