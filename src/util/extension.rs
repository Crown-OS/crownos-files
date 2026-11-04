/// The text after the last dot, ignoring the leading dot of a hidden name.
pub fn extension(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty() && !extension.is_empty()).then_some(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_real_suffix_counts() {
        assert_eq!(extension("a.tar.gz"), Some("gz"));
        assert_eq!(extension(".bashrc"), None);
        assert_eq!(extension("trailing."), None);
        assert_eq!(extension("plain"), None);
    }
}
