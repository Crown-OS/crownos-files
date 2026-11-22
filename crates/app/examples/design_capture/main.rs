//! Captures every state of CrownOS Files for the CrownOS-Design Figma file. Run both appearances
//! with `../../../crownui/capture/scripts/capture-app.sh .`; output lands in `$CROWN_DESIGN_DIR`.

mod browsing;
mod dialogs;
mod fixture;
mod menus;
mod notices;
mod picture;
mod sidebar;
mod window;

use std::path::PathBuf;

use capture::Carrier;
use crownui::AppConfig;

use crate::{
    fixture::Fixture,
    window::{Held, Window},
};

const DESIGN_DIR: &str = "CROWN_DESIGN_DIR";

/// What every window shares: the fixture home and the real window config.
pub struct Stage {
    pub fixture: Fixture,
    pub config: AppConfig,
}

impl Stage {
    pub fn window(&self, start: &str) -> anyhow::Result<Window<'_>> {
        Window::open(&self.fixture, &self.config, start)
    }

    pub fn window_held(&self, start: &str, held: Held) -> anyhow::Result<Window<'_>> {
        Window::open_held(&self.fixture, &self.config, start, held)
    }
}

fn main() -> anyhow::Result<()> {
    let output =
        std::env::var_os(DESIGN_DIR).map_or_else(|| PathBuf::from("target/design"), PathBuf::from);
    let stage = Stage {
        fixture: Fixture::create(&output)?,
        config: crownos_files::app_config(),
    };
    let mut carrier = Carrier::new(
        "Files",
        "crownos-files: the file manager window over a sample home folder, every view, menu, dialog and notice, captured headlessly from the current source.",
    );
    browsing::capture(&stage, &mut carrier)?;
    sidebar::capture(&stage, &mut carrier)?;
    menus::capture(&stage, &mut carrier)?;
    dialogs::capture(&stage, &mut carrier)?;
    notices::capture(&stage, &mut carrier)?;
    let path = carrier.write(None)?;
    println!("{}", path.display());
    capture::finish()
}
