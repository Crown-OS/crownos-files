use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TICKET: AtomicU64 = AtomicU64::new(1);

/// Identifies one listing of a directory. Later listings get larger tickets,
/// so chunks from a superseded listing are recognised and dropped.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ticket(u64);

impl Ticket {
    pub fn next() -> Self {
        Self(NEXT_TICKET.fetch_add(1, Ordering::Relaxed))
    }
}
