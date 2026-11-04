use futures_util::StreamExt;
use rustix::process::getuid;
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio_util::task::AbortOnDropHandle;
use zbus::{MatchRule, MessageStream, message::Type};

use crate::udisks::{
    actions::Udisks,
    error::DeviceError,
    proxies::ROOT,
    volume_set::{Arrival, DeviceEvent, VolumeSet},
};

const SIGNAL_QUEUE: usize = 256;

/// Follows UDisks and reports the user-visible volumes as they come and go.
/// Dropping it stops the watch.
#[derive(Debug)]
pub struct DeviceMonitor {
    _task: AbortOnDropHandle<()>,
}

impl DeviceMonitor {
    /// Reports every present volume as `Added { arrival: Present }`, then
    /// follows the changes on the current tokio runtime.
    pub async fn spawn(
        udisks: &Udisks,
    ) -> Result<(Self, UnboundedReceiver<DeviceEvent>), DeviceError> {
        let rule = MatchRule::builder()
            .msg_type(Type::Signal)
            .path_namespace(ROOT)?
            .build();
        let mut signals =
            MessageStream::for_match_rule(rule, udisks.connection(), Some(SIGNAL_QUEUE)).await?;
        let mut tree = udisks.managed_objects().await?;
        let uid = getuid().as_raw();
        let mut volumes = VolumeSet::default();
        let (events, receiver) = mpsc::unbounded_channel();
        for event in volumes.reconcile(&tree, uid, Arrival::Present) {
            events.send(event).ok();
        }
        let task = tokio::spawn(async move {
            while let Some(signal) = signals.next().await {
                if !signal.is_ok_and(|signal| tree.apply(&signal)) {
                    continue;
                }
                for event in volumes.reconcile(&tree, uid, Arrival::Plugged) {
                    if events.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Ok((
            Self {
                _task: AbortOnDropHandle::new(task),
            },
            receiver,
        ))
    }
}
