use std::path::Path;

use crownos_files_core::thumbs::Frame;
use impeller::{Pixels, encode_png};

type Rgb = [f32; 3];

const MEDIA_EXTENSIONS: [&str; 7] = [".png", ".jpg", ".webp", ".avif", ".webm", ".mp4", ".mkv"];
const PNG_EDGE: u32 = 512;
const ICON_EDGE: u32 = 64;
const ICON_RADIUS: f32 = 14.0;

/// A generated landscape: a sky gradient, a sun and two ridges.
#[derive(Clone, Copy)]
pub struct Picture {
    sky_top: Rgb,
    sky_bottom: Rgb,
    sun: Rgb,
    ridge: Rgb,
    ground: Rgb,
}

const PALETTES: [Picture; 7] = [
    Picture {
        sky_top: [12.0, 20.0, 58.0],
        sky_bottom: [46.0, 182.0, 150.0],
        sun: [190.0, 255.0, 214.0],
        ridge: [14.0, 32.0, 46.0],
        ground: [6.0, 14.0, 22.0],
    },
    Picture {
        sky_top: [255.0, 128.0, 92.0],
        sky_bottom: [255.0, 214.0, 152.0],
        sun: [255.0, 246.0, 222.0],
        ridge: [128.0, 62.0, 96.0],
        ground: [62.0, 30.0, 62.0],
    },
    Picture {
        sky_top: [118.0, 178.0, 240.0],
        sky_bottom: [242.0, 222.0, 190.0],
        sun: [255.0, 250.0, 232.0],
        ridge: [214.0, 150.0, 92.0],
        ground: [176.0, 108.0, 60.0],
    },
    Picture {
        sky_top: [84.0, 146.0, 222.0],
        sky_bottom: [204.0, 232.0, 250.0],
        sun: [255.0, 255.0, 240.0],
        ridge: [72.0, 102.0, 124.0],
        ground: [38.0, 88.0, 112.0],
    },
    Picture {
        sky_top: [166.0, 146.0, 232.0],
        sky_bottom: [250.0, 210.0, 230.0],
        sun: [255.0, 242.0, 250.0],
        ridge: [122.0, 92.0, 172.0],
        ground: [88.0, 58.0, 140.0],
    },
    Picture {
        sky_top: [150.0, 186.0, 176.0],
        sky_bottom: [222.0, 232.0, 222.0],
        sun: [250.0, 250.0, 240.0],
        ridge: [60.0, 102.0, 82.0],
        ground: [28.0, 60.0, 44.0],
    },
    Picture {
        sky_top: [38.0, 48.0, 112.0],
        sky_bottom: [242.0, 132.0, 108.0],
        sun: [255.0, 204.0, 152.0],
        ridge: [42.0, 40.0, 82.0],
        ground: [20.0, 20.0, 50.0],
    },
];

impl Picture {
    /// The picture standing in for an image or video named `name`.
    pub fn for_name(name: &str) -> Option<Self> {
        MEDIA_EXTENSIONS
            .iter()
            .any(|extension| name.ends_with(extension))
            .then(|| PALETTES[seed(name) % PALETTES.len()])
    }

    /// RGBA8 rows of a 3:2 landscape whose longest edge is `edge`.
    pub fn render(self, edge: u32) -> (u32, u32, Vec<u8>) {
        let (width, height) = (edge, edge * 2 / 3);
        let phase = self.sky_top[0] * 0.05 + self.ridge[1] * 0.03;
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            let v = y as f32 / height as f32;
            for x in 0..width {
                let u = x as f32 / width as f32;
                let color = self.shade(u, v, phase);
                rgba.extend(color.map(|channel| channel.clamp(0.0, 255.0) as u8));
                rgba.push(u8::MAX);
            }
        }
        (width, height, rgba)
    }

    fn shade(self, u: f32, v: f32, phase: f32) -> Rgb {
        let far_ridge =
            0.6 + 0.08 * (u * 6.0 + phase).sin() + 0.035 * (u * 15.0 + phase * 2.0).sin();
        let near_ridge = 0.76 + 0.05 * (u * 9.0 + phase * 3.0).sin();
        if v > near_ridge {
            return mix(self.ground, self.ridge, (1.0 - v) * 0.8);
        }
        if v > far_ridge {
            return mix(self.ridge, self.sky_bottom, (near_ridge - v) * 1.4);
        }
        let sky = mix(self.sky_top, self.sky_bottom, v / far_ridge);
        let distance = ((u - 0.68).powi(2) + ((v - 0.38) * 0.66).powi(2)).sqrt();
        let glow = (1.0 - distance / 0.22).max(0.0).powi(2);
        let disc = if distance < 0.055 { 1.0 } else { glow * 0.7 };
        mix(sky, self.sun, disc)
    }
}

/// Writes `picture` as a real image file, so the folder holds what the window shows.
pub fn write_png(path: &Path, picture: Picture) -> anyhow::Result<()> {
    let (width, height, rgba) = picture.render(PNG_EDGE);
    let png = encode_png(&Pixels {
        width,
        height,
        rgba,
    })?;
    std::fs::write(path, png)?;
    Ok(())
}

/// A rounded-square app icon in the colors of palette `index`.
pub fn app_icon(index: usize) -> Option<Frame> {
    let picture = PALETTES[index % PALETTES.len()];
    let edge = ICON_EDGE as f32;
    let mut rgba = Vec::with_capacity((ICON_EDGE * ICON_EDGE * 4) as usize);
    for y in 0..ICON_EDGE {
        for x in 0..ICON_EDGE {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let color = mix(picture.sky_top, picture.ridge, (px + py) / (2.0 * edge));
            rgba.extend(color.map(|channel| channel as u8));
            rgba.push((rounded_coverage(px, py, edge) * 255.0) as u8);
        }
    }
    Frame::new(ICON_EDGE, ICON_EDGE, rgba)
}

fn rounded_coverage(x: f32, y: f32, edge: f32) -> f32 {
    let corner = |at: f32| {
        if at < ICON_RADIUS {
            ICON_RADIUS - at
        } else if at > edge - ICON_RADIUS {
            at - (edge - ICON_RADIUS)
        } else {
            0.0
        }
    };
    let distance = corner(x).hypot(corner(y));
    (ICON_RADIUS + 0.5 - distance).clamp(0.0, 1.0)
}

fn mix(from: Rgb, to: Rgb, amount: f32) -> Rgb {
    let amount = amount.clamp(0.0, 1.0);
    [0, 1, 2].map(|channel| from[channel] + (to[channel] - from[channel]) * amount)
}

fn seed(name: &str) -> usize {
    name.bytes().fold(7usize, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(usize::from(byte))
    })
}
