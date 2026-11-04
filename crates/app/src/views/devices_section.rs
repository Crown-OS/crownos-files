//! The sidebar's Devices: drives with an eject button or a spinner while
//! they mount or eject, and paired phones with a dot that says if they are
//! reachable.

use std::{path::Path, sync::Arc};

use crownos_files_devices::udisks::OwnedObjectPath;
use crownui::{
    kit::{
        SidebarItemOptions, icon_button, icons, sidebar_item, sidebar_section, spinner, use_theme,
    },
    prelude::{
        AnyView, Cx, Listeners, Runtime, Semantic, Signal, Styled, View, bind, for_each, hstack,
        show, vstack,
    },
};

use crate::{
    app::{
        drives::{DriveActivity, DriveRow, open_drive, release_drive},
        peers::Peer,
    },
    state::Store,
    util::theme::tint,
    views::drop_target::folder_drop_target,
};

const STATUS_DOT: f32 = 7.0;
const SPINNER_SIZE: f32 = 14.0;

type DriveKey = (OwnedObjectPath, bool, Option<DriveActivity>);

pub fn devices_section(store: Store) -> impl View {
    show(
        bind(move |runtime: &mut Runtime| {
            store.drives.rows.with(runtime, |rows| !rows.is_empty())
                || store
                    .peers
                    .paired
                    .with(runtime, |paired| !paired.is_empty())
        }),
        move || {
            sidebar_section(
                "Devices",
                vstack((
                    for_each(store.drives.rows, drive_key, move |cx, row| {
                        drive_row(cx, store, row)
                    })
                    .flex_col()
                    .gap(4.0),
                    for_each(
                        store.peers.paired,
                        |peer: &Peer| (peer.id, peer.connected),
                        peer_row,
                    )
                    .flex_col()
                    .gap(4.0),
                ))
                .gap(4.0),
            )
        },
        || vstack(()),
    )
    .flex_col()
}

fn drive_key(row: &DriveRow) -> DriveKey {
    (
        row.volume.object_path.clone(),
        row.volume.is_mounted(),
        row.activity,
    )
}

fn drive_row(cx: &mut Cx, store: Store, row: Signal<DriveRow>) -> AnyView {
    let row = row.get(cx);
    let name = row.volume.name().to_owned();
    let volume = row.volume.object_path.clone();
    let mount: Option<Arc<Path>> = row.mount_point().map(Arc::from);
    let shown = mount.clone();
    let item = sidebar_item(
        name.clone(),
        icons::HARD_DRIVE,
        bind(move |runtime: &mut Runtime| {
            shown
                .as_ref()
                .is_some_and(|dir| store.cwd.with(runtime, |cwd| cwd == dir))
        }),
    );
    let item = match row.activity {
        Some(activity) => item.trailing(spinner().size(SPINNER_SIZE).label(match activity {
            DriveActivity::Mounting => format!("Mounting {name}"),
            DriveActivity::Ejecting => format!("Ejecting {name}"),
        })),
        None if row.can_release() => {
            let released = volume.clone();
            item.trailing(
                icon_button(icons::ARROW_UP_FROM_LINE, format!("Eject {name}")).on_click(
                    move |cx, event| {
                        event.stop_propagation();
                        release_drive(cx, store, &released);
                    },
                ),
            )
        }
        None => item,
    }
    .on_click(move |cx, _| open_drive(cx, store, &volume));
    match mount {
        Some(dir) => folder_drop_target(item, store, dir, cx.signal(false)).boxed(),
        None => item.boxed(),
    }
}

fn peer_row(cx: &mut Cx, peer: Signal<Peer>) -> AnyView {
    let theme = use_theme(cx);
    let peer = peer.get(cx);
    let connected = peer.connected;
    let state = if connected { "connected" } else { "offline" };
    sidebar_item(peer.name.clone(), peer.kind.icon(), false)
        .label(format!("{}, {state}", peer.name))
        .trailing(
            hstack(())
                .size(STATUS_DOT)
                .rounded(STATUS_DOT / 2.0)
                .bg(tint(theme, move |theme| {
                    if connected {
                        theme.status.success
                    } else {
                        theme.status.neutral
                    }
                })),
        )
        .boxed()
}
