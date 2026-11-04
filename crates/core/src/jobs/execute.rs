use crate::jobs::{
    context::JobContext,
    copy::run_copy,
    delete::run_delete,
    error::JobError,
    event::{JobEvent, Outcome},
    job::Job,
    r#move::run_move,
    trash::{run_delete_from_trash, run_empty_trash, run_restore, run_trash},
    undo::Reversible,
};

/// Runs `job` to the end on the calling blocking thread, finishing with a
/// `Finished` event.
pub fn execute(job: Job, mut ctx: JobContext) {
    let id = ctx.id;
    let (result, undo) = match ctx.check() {
        Ok(()) => run(job, &mut ctx),
        Err(cancelled) => (Err(cancelled), None),
    };
    let outcome = match result {
        Ok(()) => Outcome::Completed,
        Err(JobError::Cancelled) => Outcome::Cancelled,
        Err(error) => {
            let needs_privilege = error.needs_privilege();
            ctx.send(JobEvent::Failed {
                id,
                error,
                needs_privilege,
            });
            Outcome::Failed
        }
    };
    ctx.send(JobEvent::Finished { id, outcome, undo });
}

fn run(job: Job, ctx: &mut JobContext) -> (Result<(), JobError>, Option<Reversible>) {
    match job {
        Job::Copy(transfers) => {
            let mut created = Vec::new();
            let result = run_copy(&transfers, ctx, &mut created);
            (result, recorded(created, Reversible::Copy))
        }
        Job::Move(transfers) => {
            let mut moved = Vec::new();
            let result = run_move(&transfers, ctx, &mut moved);
            (result, recorded(moved, Reversible::Move))
        }
        Job::Trash(paths) => {
            let mut trashed = Vec::new();
            let result = run_trash(&paths, ctx, &mut trashed);
            (result, recorded(trashed, Reversible::Trash))
        }
        Job::Delete(paths) => (run_delete(&paths, ctx), None),
        Job::Restore(items) => (run_restore(&items, ctx), None),
        Job::DeleteFromTrash(items) => (run_delete_from_trash(&items, ctx), None),
        Job::EmptyTrash => (run_empty_trash(ctx), None),
    }
}

fn recorded<T>(done: Vec<T>, record: fn(Vec<T>) -> Reversible) -> Option<Reversible> {
    (!done.is_empty()).then(|| record(done))
}
