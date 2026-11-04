use std::{path::Path, sync::Arc};

use tokio::sync::mpsc::{
    UnboundedReceiver, UnboundedSender, WeakUnboundedSender, unbounded_channel,
};
use tokio_util::{sync::CancellationToken, task::AbortOnDropHandle};

use crate::{
    fs::lister::list_dir,
    model::{files_op::FilesOp, ticket::Ticket},
    util::hash::FastHashMap,
};

enum Request {
    Load(Arc<Path>),
    Cancel(Arc<Path>),
    Finished(Arc<Path>, Ticket),
}

struct Listing {
    ticket: Ticket,
    cancel: CancellationToken,
    dirty: bool,
}

/// Runs directory listings with at most one in flight per directory: a load
/// requested while one runs marks it dirty, and it runs once more after.
/// Dropping the refresher cancels every listing.
pub struct Refresher {
    requests: UnboundedSender<Request>,
    _task: AbortOnDropHandle<()>,
}

impl Refresher {
    /// Starts the refresher on the current tokio runtime.
    pub fn spawn(ops: UnboundedSender<FilesOp>) -> Self {
        let (requests, receiver) = unbounded_channel();
        let task = tokio::spawn(serve(ops, requests.downgrade(), receiver));
        Self {
            requests,
            _task: AbortOnDropHandle::new(task),
        }
    }

    pub fn load(&self, dir: Arc<Path>) {
        let _ = self.requests.send(Request::Load(dir));
    }

    /// Stops listing `dir`, e.g. when the user navigated away mid-listing.
    pub fn cancel(&self, dir: Arc<Path>) {
        let _ = self.requests.send(Request::Cancel(dir));
    }
}

async fn serve(
    ops: UnboundedSender<FilesOp>,
    requests: WeakUnboundedSender<Request>,
    mut receiver: UnboundedReceiver<Request>,
) {
    let root = CancellationToken::new();
    let _cancel_all = root.clone().drop_guard();
    let mut listings: FastHashMap<Arc<Path>, Listing> = FastHashMap::default();
    let start = |dir: &Arc<Path>| start_listing(dir, &root, &ops, &requests);
    while let Some(request) = receiver.recv().await {
        match request {
            Request::Load(dir) => match listings.get_mut(&dir) {
                Some(listing) => listing.dirty = true,
                None => {
                    let listing = start(&dir);
                    listings.insert(dir, listing);
                }
            },
            Request::Cancel(dir) => {
                if let Some(listing) = listings.remove(&dir) {
                    listing.cancel.cancel();
                }
            }
            Request::Finished(dir, ticket) => match listings.get_mut(&dir) {
                Some(listing) if listing.ticket == ticket && listing.dirty => {
                    *listing = start(&dir);
                }
                Some(listing) if listing.ticket == ticket => {
                    listings.remove(&dir);
                }
                _ => {}
            },
        }
    }
}

fn start_listing(
    dir: &Arc<Path>,
    root: &CancellationToken,
    ops: &UnboundedSender<FilesOp>,
    requests: &WeakUnboundedSender<Request>,
) -> Listing {
    let ticket = Ticket::next();
    let cancel = root.child_token();
    let (dir, ops, requests, token) = (dir.clone(), ops.clone(), requests.clone(), cancel.clone());
    tokio::task::spawn_blocking(move || {
        list_dir(&dir, ticket, &token, |op| {
            if ops.send(op).is_err() {
                token.cancel();
            }
        });
        if let Some(requests) = requests.upgrade() {
            let _ = requests.send(Request::Finished(dir, ticket));
        }
    });
    Listing {
        ticket,
        cancel,
        dirty: false,
    }
}
