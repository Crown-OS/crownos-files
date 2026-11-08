//! Sending files to paired devices: the details panel's button, the context
//! menu's "Send to" page and the toasts after crownlink answers.

use crowntest::{Role, role, text};

use super::window::{Window, drain, window};
use crate::app::{
    fixtures::{LAUNCH, pixel},
    peers::{Peer, PeerCommand, PeerEvent, PeerId, PeerKind, finish_peers},
};

fn tablet() -> Peer {
    Peer {
        id: PeerId([4; 32]),
        name: "Galaxy Tab".to_owned(),
        kind: PeerKind::Tablet,
        connected: true,
    }
}

fn report(window: &mut Window, event: PeerEvent) {
    window.answer(|cx, store| finish_peers(cx, store, vec![event]));
}

#[test]
fn the_details_button_sends_the_selection_to_the_first_connected_phone() {
    let mut window = window();
    window.click("keynote.pdf");
    window
        .app
        .click(role(Role::Button).name("Send to Pixel 9 Pro"))
        .settle();
    let keynote = window.dir(LAUNCH).join("keynote.pdf");
    assert_eq!(
        drain(&mut window.workers.peers),
        [PeerCommand::Send {
            id: pixel().id,
            name: "Pixel 9 Pro".to_owned(),
            paths: vec![keynote],
        }]
    );
    report(
        &mut window,
        PeerEvent::Sent {
            name: "Pixel 9 Pro".to_owned(),
            count: 1,
        },
    );
    assert!(window.shows(text("Sent 1 file to Pixel 9 Pro")));
    report(
        &mut window,
        PeerEvent::SendFailed {
            name: "Pixel 9 Pro".to_owned(),
            error: "crownlink is not running".to_owned(),
        },
    );
    assert!(window.shows(text(
        "Couldn't send to Pixel 9 Pro: crownlink is not running"
    )));
}

#[test]
fn the_button_is_hidden_without_a_connected_phone_and_folders_are_not_sent() {
    let mut window = window();
    let offline = Peer {
        connected: false,
        ..pixel()
    };
    report(&mut window, PeerEvent::Paired(vec![offline, tablet()]));
    window.click("keynote.pdf");
    assert!(!window.shows(role(Role::Button).text("Send to")));
    assert!(window.shows(role(Role::MenuItem).name("Galaxy Tab, connected")));
    assert!(window.shows(role(Role::MenuItem).name("Pixel 9 Pro, offline")));

    report(&mut window, PeerEvent::Paired(vec![pixel()]));
    window.click("Brand");
    window
        .app
        .click(role(Role::Button).name("Send to Pixel 9 Pro"))
        .advance_ms(16);
    assert!(drain(&mut window.workers.peers).is_empty());
    assert!(window.shows(text("Only files can be sent to Pixel 9 Pro")));
}

#[test]
fn the_send_to_page_lists_the_reachable_devices() {
    let mut window = window();
    let watch = Peer {
        id: PeerId([5; 32]),
        name: "Watch".to_owned(),
        kind: PeerKind::Watch,
        connected: false,
    };
    report(
        &mut window,
        PeerEvent::Paired(vec![pixel(), tablet(), watch]),
    );
    window.click("palette.svg").key("Shift+F10");
    window
        .app
        .click(role(Role::MenuItem).name("Send to"))
        .settle();
    assert!(window.app.exists(role(Role::Menu)));
    assert!(window.shows(role(Role::MenuItem).name("Pixel 9 Pro")));
    assert!(!window.shows(role(Role::MenuItem).name("Watch")));
    window
        .app
        .click(role(Role::MenuItem).name("Galaxy Tab"))
        .settle();
    assert_eq!(
        drain(&mut window.workers.peers),
        [PeerCommand::Send {
            id: tablet().id,
            name: "Galaxy Tab".to_owned(),
            paths: vec![window.dir(LAUNCH).join("palette.svg")],
        }]
    );
}

#[test]
fn the_send_to_page_says_when_nothing_is_reachable() {
    let mut window = window();
    report(&mut window, PeerEvent::Paired(Vec::new()));
    window.click("palette.svg").key("Shift+F10");
    window
        .app
        .click(role(Role::MenuItem).name("Send to"))
        .settle();
    assert!(window.shows(role(Role::MenuItem).name("No device is connected")));
}
