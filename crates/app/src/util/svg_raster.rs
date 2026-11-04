//! Painting a small SVG document to straight-alpha RGBA on the CPU, with the
//! kit's Inter available for its text.

use std::sync::Arc;

use crownui::{kit::inter_font, prelude::Size};
use resvg::{
    tiny_skia::{Pixmap, Transform},
    usvg::{self, fontdb},
};

thread_local! {
    static FONTS: Arc<fontdb::Database> = {
        let mut fonts = fontdb::Database::new();
        fonts.load_font_data(inter_font().to_vec());
        Arc::new(fonts)
    };
}

/// `body` in a `logical`-sized view box, painted at `scale` device pixels
/// per logical one, as `width × height × 4` bytes, top row first.
pub fn rasterize(body: &str, logical: Size, scale: f32) -> Option<Vec<u8>> {
    let (width, height) = (
        (logical.width * scale).round().max(1.0) as u32,
        (logical.height * scale).round().max(1.0) as u32,
    );
    let document = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {} {}">{body}</svg>"#,
        logical.width, logical.height
    );
    let options = usvg::Options {
        fontdb: FONTS.with(Arc::clone),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&document, &options).ok()?;
    let mut pixmap = Pixmap::new(width, height)?;
    let fit = Transform::from_scale(
        width as f32 / tree.size().width(),
        height as f32 / tree.size().height(),
    );
    resvg::render(&tree, fit, &mut pixmap.as_mut());
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let straight = pixel.demultiply();
                [
                    straight.red(),
                    straight.green(),
                    straight.blue(),
                    straight.alpha(),
                ]
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rect_paints_its_color_at_device_scale_and_leaves_the_rest_clear() {
        let body = r##"<rect x="0" y="0" width="5" height="10" fill="#ff0000"/>"##;
        let Some(rgba) = rasterize(body, Size::new(10.0, 10.0), 2.0) else {
            panic!("the document parses");
        };
        assert_eq!(rgba.len(), 20 * 20 * 4);
        assert_eq!(rgba[..4], [255, 0, 0, 255]);
        let right = (5 * 20 + 15) * 4;
        assert_eq!(rgba[right + 3], 0);
    }
}
