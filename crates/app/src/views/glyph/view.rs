use crownui::prelude::{AnyView, View};

use super::{
    archive::archive, audio_wave::audio_wave, choice::Glyph, code_page::code_page, disc::disc,
    folder::folder, page::page, page::swatch_page, photo::photo, sheet::sheet, video::video,
};

/// The glyph's view at `scale` times its mockup size.
pub fn glyph_view(glyph: Glyph, scale: f32) -> AnyView {
    match glyph {
        Glyph::Folder(mark) => folder(mark, scale).boxed(),
        Glyph::Photo => photo(scale).boxed(),
        Glyph::Video => video(scale).boxed(),
        Glyph::Page(badge) => page(badge, scale).boxed(),
        Glyph::Vector(badge) => swatch_page(badge, scale).boxed(),
        Glyph::Code(badge) => code_page(badge, scale).boxed(),
        Glyph::Sheet(badge, accent) => sheet(badge, accent, scale).boxed(),
        Glyph::Audio(badge, accent) => audio_wave(badge, accent, scale).boxed(),
        Glyph::Disc => disc(scale).boxed(),
        Glyph::Archive => archive(scale).boxed(),
    }
}
