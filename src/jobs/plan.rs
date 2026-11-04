use std::path::Path;

use tokio_util::sync::CancellationToken;

use crate::jobs::{
    error::JobError,
    traverse::{Node, walk},
};

/// Everything under one source, measured before any of it is touched.
#[derive(Debug, Clone, Default)]
pub struct RootPlan {
    pub nodes: Vec<Node>,
    pub bytes: u64,
}

impl RootPlan {
    pub fn scan(root: &Path, cancel: &CancellationToken) -> Result<Self, JobError> {
        let mut plan = Self::default();
        walk(root, cancel, |node| {
            plan.bytes += node.len;
            plan.nodes.push(node);
        })?;
        Ok(plan)
    }

    pub fn items(&self) -> u64 {
        self.nodes.len() as u64
    }
}

/// Bytes and items across several plans.
pub fn totals<'a>(plans: impl IntoIterator<Item = &'a RootPlan>) -> (u64, u64) {
    plans.into_iter().fold((0, 0), |(bytes, items), plan| {
        (bytes + plan.bytes, items + plan.items())
    })
}
