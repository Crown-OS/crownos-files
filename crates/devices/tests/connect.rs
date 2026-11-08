#![cfg(feature = "connect")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{path::Path, time::Duration};

use crownlink_linux::ipc::proto::{
    self as protocol, Battery, DeviceClass, DeviceId, DeviceInfo, FeatureSet, LinkKind,
};
use crownos_files_devices::connect::{Connect, ConnectEvent, DeviceKind, PairedDevice};
use crownos_ipc::{MethodMsg, Server, ServiceBuilder, adapter::tokio::serve_with_outbox, postcard};
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
    time::timeout,
};

const PATIENCE: Duration = Duration::from_secs(10);
const PHONE: DeviceId = DeviceId([1; 32]);
const STRANGER: DeviceId = DeviceId([2; 32]);

enum Push {
    Connected(bool),
    Battery(u8),
}

fn info(id: DeviceId, name: &str, trusted: bool) -> DeviceInfo {
    DeviceInfo {
        id,
        name: name.to_owned(),
        class: DeviceClass::Phone,
        trusted,
        connected: false,
        link: LinkKind::Lan,
        battery: None,
        features: FeatureSet::EMPTY,
        active: FeatureSet::EMPTY,
    }
}

fn emit(server: &mut Server, push: Push) {
    match push {
        Push::Connected(connected) => server.emit(&protocol::DeviceConnected {
            id: PHONE,
            name: "Pixel".to_owned(),
            connected,
        }),
        Push::Battery(percent) => server.emit(&protocol::BatteryChanged {
            id: PHONE,
            percent,
            charging: true,
        }),
    }
    .unwrap();
}

/// Stands in for crownlink: lists two devices and records sent files.
fn fake_crownlink(
    dir: &Path,
    sent: UnboundedSender<String>,
) -> (JoinHandle<()>, UnboundedSender<Push>) {
    let mut server = ServiceBuilder::new(protocol::SERVICE)
        .directory(dir)
        .build()
        .unwrap();
    let (pushes, mut outbox) = mpsc::unbounded_channel::<Push>();
    let task = tokio::spawn(async move {
        let devices = vec![info(PHONE, "Pixel", true), info(STRANGER, "Nearby", false)];
        serve_with_outbox(
            &mut server,
            |server, peer, message| {
                let reply = if message.selector == protocol::devices::SELECTOR {
                    postcard::to_stdvec(&devices)?
                } else if message.selector == protocol::send_file::SELECTOR {
                    let args: protocol::send_file = postcard::from_bytes(&message.payload)?;
                    sent.send(args.path).unwrap();
                    postcard::to_stdvec(&())?
                } else {
                    return Ok(true);
                };
                server.reply(peer, message.req_id, &reply, Vec::new())?;
                Ok(true)
            },
            |cx| outbox.poll_recv(cx),
            emit,
        )
        .await
        .unwrap();
    });
    (task, pushes)
}

async fn next<T>(receiver: &mut UnboundedReceiver<T>) -> T {
    timeout(PATIENCE, receiver.recv()).await.unwrap().unwrap()
}

async fn next_devices(events: &mut UnboundedReceiver<ConnectEvent>) -> Vec<PairedDevice> {
    match next(events).await {
        ConnectEvent::Devices(devices) => devices,
        ConnectEvent::Unavailable => panic!("unexpectedly unavailable"),
    }
}

#[tokio::test]
async fn follows_devices_sends_files_and_reconnects() {
    let dir = tempfile::tempdir().unwrap();
    let (sent, mut received) = mpsc::unbounded_channel();
    let (server, pushes) = fake_crownlink(dir.path(), sent.clone());

    let (connect, mut events) = Connect::spawn_in(dir.path());
    let devices = next_devices(&mut events).await;
    assert_eq!(
        devices,
        [PairedDevice {
            id: PHONE,
            name: "Pixel".to_owned(),
            kind: DeviceKind::Phone,
            connected: false,
            battery: None,
        }]
    );

    pushes.send(Push::Connected(true)).unwrap();
    assert!(next_devices(&mut events).await[0].connected);
    pushes.send(Push::Battery(80)).unwrap();
    let battery = next_devices(&mut events).await[0].battery;
    assert_eq!(
        battery,
        Some(Battery {
            percent: 80,
            charging: true
        })
    );
    assert_eq!(connect.devices()[0].battery, battery);

    let files = [Path::new("/home/u/a.png"), Path::new("/home/u/b.pdf")];
    connect.send_files(PHONE, &files).await.unwrap();
    assert_eq!(next(&mut received).await, "/home/u/a.png");
    assert_eq!(next(&mut received).await, "/home/u/b.pdf");

    server.abort();
    let _ = server.await;
    assert_eq!(next(&mut events).await, ConnectEvent::Unavailable);
    assert!(connect.devices().is_empty());
    assert!(connect.send_files(PHONE, &files).await.is_err());

    let (_server, _pushes) = fake_crownlink(dir.path(), sent);
    assert_eq!(next_devices(&mut events).await.len(), 1);
}
