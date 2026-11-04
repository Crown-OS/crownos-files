use crate::{
    mime::{table::MimeTable, well_known},
    model::mime_id::MimeId,
    util::hash::FastHashMap,
};

const TEXT_MEDIA_PREFIX: &str = "text/";

/// The `aliases` and `subclasses` relations of shared-mime-info.
#[derive(Debug)]
pub struct Hierarchy {
    aliases: FastHashMap<MimeId, MimeId>,
    parents: FastHashMap<MimeId, Vec<MimeId>>,
    text_plain: MimeId,
}

impl Default for Hierarchy {
    fn default() -> Self {
        Self {
            aliases: FastHashMap::default(),
            parents: FastHashMap::default(),
            text_plain: MimeTable::intern(well_known::TEXT_PLAIN),
        }
    }
}

impl Hierarchy {
    /// Adds an `aliases` file; earlier files win.
    pub fn add_aliases(&mut self, text: &str) {
        for (alias, canonical) in pairs(text) {
            self.aliases.entry(alias).or_insert(canonical);
        }
    }

    /// Adds a `subclasses` file.
    pub fn add_subclasses(&mut self, text: &str) {
        for (child, parent) in pairs(text) {
            let parents = self.parents.entry(child).or_default();
            if !parents.contains(&parent) {
                parents.push(parent);
            }
        }
    }

    pub fn unalias(&self, mime: MimeId) -> MimeId {
        self.aliases.get(&mime).copied().unwrap_or(mime)
    }

    /// `mime` (unaliased) followed by every type it is a kind of, nearest
    /// first: `text/rust` gives `[text/rust, text/plain]`.
    pub fn ancestors(&self, mime: MimeId) -> Vec<MimeId> {
        let mut chain = vec![self.unalias(mime)];
        let mut next = 0;
        while let Some(&current) = chain.get(next) {
            next += 1;
            for parent in self.direct_parents(current) {
                let parent = self.unalias(parent);
                if !chain.contains(&parent) {
                    chain.push(parent);
                }
            }
        }
        chain
    }

    pub fn is_a(&self, mime: MimeId, base: MimeId) -> bool {
        self.ancestors(mime).contains(&self.unalias(base))
    }

    fn direct_parents(&self, mime: MimeId) -> impl Iterator<Item = MimeId> + '_ {
        let listed = self
            .parents
            .get(&mime)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let implicit_text = (mime != self.text_plain
            && MimeTable::name(mime).is_some_and(|name| name.starts_with(TEXT_MEDIA_PREFIX)))
        .then_some(self.text_plain);
        listed.iter().copied().chain(implicit_text)
    }
}

fn pairs(text: &str) -> impl Iterator<Item = (MimeId, MimeId)> + '_ {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once(' '))
        .map(|(left, right)| {
            (
                MimeTable::intern(left.trim()),
                MimeTable::intern(right.trim()),
            )
        })
        .filter(|&(left, right)| left != MimeId::UNKNOWN && right != MimeId::UNKNOWN)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> MimeId {
        MimeTable::intern(name)
    }

    #[test]
    fn aliases_resolve_and_parents_chain_nearest_first() {
        let mut hierarchy = Hierarchy::default();
        hierarchy.add_aliases("text/x-rust text/rust\n");
        hierarchy.add_subclasses("application/x-shellscript application/x-executable\napplication/x-shellscript text/plain\n");
        assert_eq!(hierarchy.unalias(id("text/x-rust")), id("text/rust"));
        assert_eq!(
            hierarchy.ancestors(id("text/x-rust")),
            [id("text/rust"), id("text/plain")]
        );
        assert_eq!(
            hierarchy.ancestors(id("application/x-shellscript")),
            [
                id("application/x-shellscript"),
                id("application/x-executable"),
                id("text/plain")
            ]
        );
        assert!(hierarchy.is_a(id("text/x-rust"), id("text/plain")));
        assert!(!hierarchy.is_a(id("text/plain"), id("text/rust")));
    }
}
