//! `rwx` for one audience of a file's permission bits.

const READ: u32 = 0o4;
const WRITE: u32 = 0o2;
const EXECUTE: u32 = 0o1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    Owner,
    Group,
    Others,
}

impl Audience {
    const fn shift(self) -> u32 {
        match self {
            Self::Owner => 6,
            Self::Group => 3,
            Self::Others => 0,
        }
    }
}

/// `rw-` and the like: what `audience` may do with a file of `mode`.
pub fn rwx(mode: u32, audience: Audience) -> String {
    let bits = mode >> audience.shift();
    [(READ, 'r'), (WRITE, 'w'), (EXECUTE, 'x')]
        .into_iter()
        .map(|(bit, letter)| if bits & bit != 0 { letter } else { '-' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_each_audience() {
        assert_eq!(rwx(0o100754, Audience::Owner), "rwx");
        assert_eq!(rwx(0o100754, Audience::Group), "r-x");
        assert_eq!(rwx(0o100754, Audience::Others), "r--");
        assert_eq!(rwx(0, Audience::Owner), "---");
    }
}
