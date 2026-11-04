use std::{collections::VecDeque, path::PathBuf};

use crate::{
    jobs::job::{Job, Transfer},
    model::trash::TrashedItem,
};

const UNDO_LIMIT: usize = 64;

/// A completed operation that can be reversed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reversible {
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    /// Each source and where it ended up.
    Move(Vec<Transfer>),
    /// A new file or folder.
    Create(PathBuf),
    /// The top-level copies that were made.
    Copy(Vec<PathBuf>),
    Trash(Vec<TrashedItem>),
}

impl Reversible {
    /// The job that undoes this operation. Created entries and copies go to
    /// the trash rather than being deleted.
    pub fn inverse(self) -> Job {
        match self {
            Self::Rename { from, to } => Job::Move(vec![Transfer { from: to, to: from }]),
            Self::Move(moved) => {
                Job::Move(moved.into_iter().rev().map(Transfer::reversed).collect())
            }
            Self::Create(path) => Job::Trash(vec![path]),
            Self::Copy(created) => Job::Trash(created),
            Self::Trash(items) => Job::Restore(items),
        }
    }
}

/// The most recent [`UNDO_LIMIT`] reversible operations.
#[derive(Debug, Default)]
pub struct UndoStack {
    done: VecDeque<Reversible>,
}

impl UndoStack {
    pub fn push(&mut self, operation: Reversible) {
        if self.done.len() == UNDO_LIMIT {
            self.done.pop_front();
        }
        self.done.push_back(operation);
    }

    /// The job reversing the latest operation.
    pub fn undo(&mut self) -> Option<Job> {
        self.done.pop_back().map(Reversible::inverse)
    }

    pub fn peek(&self) -> Option<&Reversible> {
        self.done.back()
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverses_swap_and_reverse() {
        let rename = Reversible::Rename {
            from: "/a".into(),
            to: "/b".into(),
        };
        assert_eq!(
            rename.inverse(),
            Job::Move(vec![Transfer {
                from: "/b".into(),
                to: "/a".into()
            }])
        );
        let moved = Reversible::Move(vec![
            Transfer {
                from: "/x/1".into(),
                to: "/y/1".into(),
            },
            Transfer {
                from: "/x/2".into(),
                to: "/y/2 (2)".into(),
            },
        ]);
        let Job::Move(back) = moved.inverse() else {
            panic!("a move undoes as a move");
        };
        assert_eq!(back[0].from, PathBuf::from("/y/2 (2)"));
        assert_eq!(back[0].to, PathBuf::from("/x/2"));
        assert_eq!(
            Reversible::Create("/n".into()).inverse(),
            Job::Trash(vec!["/n".into()])
        );
        assert_eq!(
            Reversible::Trash(Vec::new()).inverse(),
            Job::Restore(Vec::new())
        );
    }

    #[test]
    fn the_stack_is_bounded_and_last_in_first_out() {
        let mut stack = UndoStack::default();
        for index in 0..=UNDO_LIMIT {
            stack.push(Reversible::Create(PathBuf::from(index.to_string())));
        }
        assert_eq!(stack.done.len(), UNDO_LIMIT);
        assert_eq!(
            stack.undo(),
            Some(Job::Trash(vec![PathBuf::from(UNDO_LIMIT.to_string())]))
        );
        assert_eq!(
            stack.done.front(),
            Some(&Reversible::Create(PathBuf::from("1")))
        );
    }
}
