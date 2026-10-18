//! Text in Inter, from the signed distance fields in sdf_font.h.

use super::{blend, weight, Screen};
use font::{SDF_CHARS, SDF_DATA, SDF_EM, SDF_GLYPHS, SDF_SPREAD};
use std::sync::OnceLock;

/// One glyph's signed distance field: where it starts in its data, its size
/// in field pixels, and its left bearing, advance and top in em units of 40.
pub struct Glyph {
    pub off: u32,
    pub w: u8,
    pub h: u8,
    pub left: f32,
    pub adv: f32,
    pub top: f32,
}

/// A game's glyphs beyond Inter's (its translations), made by
/// `tools/sdf-font.py --extra src/glyphs.rs src/*.rs`; `chars` is sorted.
pub struct Glyphs {
    pub chars: &'static [char],
    pub glyphs: &'static [Glyph],
    pub data: &'static [u8],
}

/// The font tables, made from sdf_font.h by build.rs.
mod font {
    use super::Glyph;
    include!(concat!(env!("OUT_DIR"), "/sdf_font.rs"));
}

static EXTRA: OnceLock<&'static Glyphs> = OnceLock::new();

/// Adds a game's own glyphs to the text functions, once, from `Game::new`.
pub fn add_glyphs(glyphs: &'static Glyphs) {
    let _ = EXTRA.set(glyphs);
}

impl Screen {
    /// Width of `s` at `size` (the em) in plan units.
    pub fn text_width(&self, s: &str, size: f32) -> f32 {
        text_width_px(s, size)
    }

    /// Text with its left edge at x and the middle of its digits at cy;
    /// returns the x after it. Characters the font lacks draw as spaces.
    pub fn text(&mut self, s: &str, x: f32, cy: f32, size: f32, c: u32, alpha: f32) -> f32 {
        let k = self.k;
        let end = self.text_px(s, self.sx(x), self.sy(cy), size * k, c, alpha);
        (end - self.ox) / k
    }

    /// Text centred on x.
    pub fn text_c(&mut self, s: &str, cx: f32, cy: f32, size: f32, c: u32, alpha: f32) {
        let w = self.text_width(s, size);
        self.text(s, cx - w / 2.0, cy, size, c, alpha);
    }

    /// Text ending at x.
    pub fn text_r(&mut self, s: &str, rx: f32, cy: f32, size: f32, c: u32, alpha: f32) {
        let w = self.text_width(s, size);
        self.text(s, rx - w, cy, size, c, alpha);
    }

    /// The biggest size up to `size` at which `s` fits in `max_w`.
    pub fn fit_size(&self, s: &str, size: f32, max_w: f32) -> f32 {
        let w = self.text_width(s, size);
        if w > max_w && w > 0.0 {
            size * max_w / w
        } else {
            size
        }
    }

    /// Text in screen pixels: left edge x, digits' middle cy, em `size`.
    pub fn text_px(&mut self, s: &str, mut x: f32, cy: f32, size: f32, c: u32, alpha: f32) -> f32 {
        let k = size / SDF_EM;
        let top = cy - DIGIT_MID * k;
        for ch in s.chars() {
            let (g, data) = glyph(ch);
            self.draw_glyph((g, data), x + g.left * k, top + g.top * k, k, c, alpha);
            x += g.adv * k;
        }
        x
    }

    fn draw_glyph(&mut self, (g, data): (&Glyph, &[u8]), gx: f32, gy: f32, k: f32, c: u32, alpha: f32) {
        let (w, h) = (g.w as i32, g.h as i32);
        if w == 0 || alpha <= 0.0 {
            return;
        }
        let f = &data[g.off as usize..g.off as usize + (w * h) as usize];
        let (x0, y0, x1, y1) = self.clipped(gx, gy, gx + w as f32 * k, gy + h as f32 * k);
        let inv = 1.0 / k;
        // the field holds 128 on the glyph's edge and moves 127 over
        // SDF_SPREAD field pixels; this turns it into screen pixels
        let scale = SDF_SPREAD / 127.0 * k;
        // the field at x, y, empty outside the glyph's box
        let at = |x: i32, y: i32| -> f32 {
            if x < 0 || y < 0 || x >= w || y >= h {
                0.0
            } else {
                f[(y * w + x) as usize] as f32
            }
        };
        for (j, row) in self.rows(y0, y1) {
            // where the pixel's centre falls in the field, and the four
            // field values around it to blend
            let fy = (j as f32 + 0.5 - gy) * inv - 0.5;
            let iy = fy.floor() as i32;
            let ty = fy - iy as f32;
            if iy < -1 || iy >= h {
                continue;
            }
            for i in x0..x1 {
                let fx = (i as f32 + 0.5 - gx) * inv - 0.5;
                let ix = fx.floor() as i32;
                let tx = fx - ix as f32;
                if ix < -1 || ix >= w {
                    continue;
                }
                let v = (at(ix, iy) * (1.0 - tx) + at(ix + 1, iy) * tx) * (1.0 - ty)
                    + (at(ix, iy + 1) * (1.0 - tx) + at(ix + 1, iy + 1) * tx) * ty;
                let cov = ((v - 128.0) * scale + 0.5).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, weight(cov, alpha));
                }
            }
        }
    }
}

/// Middle of the digits' height below the line top, in em units.
const DIGIT_MID: f32 = 24.6;

/// The glyph for `ch` and the data its field is in: Inter's, the game's own
/// (add_glyphs), or a space.
fn glyph(ch: char) -> (&'static Glyph, &'static [u8]) {
    if ch.is_ascii() {
        if let Some(i) = SDF_CHARS.iter().position(|&c| c == ch as u8) {
            return (&SDF_GLYPHS[i], SDF_DATA);
        }
    }
    // a game's own glyphs: other scripts, and ASCII sdf_font.h lacks (~ @ {)
    if let Some(extra) = EXTRA.get() {
        if let Ok(i) = extra.chars.binary_search(&ch) {
            return (&extra.glyphs[i], extra.data);
        }
    }
    (&SDF_GLYPHS[0], SDF_DATA)
}

fn text_width_px(s: &str, size: f32) -> f32 {
    let k = size / SDF_EM;
    s.chars().map(|ch| glyph(ch).0.adv * k).sum()
}
