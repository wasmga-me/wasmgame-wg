//! A framebuffer at the screen's own size with anti-aliased shapes and text,
//! drawn in the game's own units (its plan, say 400x300) scaled to fit: the
//! Rust side of wgfb.h and wgview.h. Every method takes plan units unless
//! its name ends in `_px`.

mod background;
mod colour;
mod image;
mod shapes;
mod sprite;
mod text;

use colour::weight;
pub use colour::{blend, hex, mix, rgb};
pub use image::Image;
pub use sprite::Sprite;
pub use text::{add_glyphs, Glyph, Glyphs};

/// The sides a buffer may have, in pixels (the page caps the canvas at
/// 2.4 MP).
const MIN_SIDE: i32 = 64;
const MAX_SIDE: i32 = 4096;

/// A rectangle of whole pixels, x0..x1 by y0..y1.
#[derive(Clone, Copy, Default)]
struct Clip {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

/// The layer the page shows.
const SHOWN: usize = 0;
/// The layer holding the still background (see [`Screen::background`]).
const BACKGROUND: usize = 1;

/// The screen: layer SHOWN is what the page shows, BACKGROUND holds
/// a still background once a game draws one, and a sprite being made is
/// drawn into a layer of its own on top.
pub struct Screen {
    layers: Vec<Vec<u32>>,
    /// the layer being drawn into
    cur: usize,
    /// size of the buffer being drawn into, in pixels
    w: i32,
    h: i32,
    /// screen size in pixels
    pub sw: i32,
    pub sh: i32,
    /// plan to screen: pixel = (ox + x * k, oy + y * k)
    pub k: f32,
    pub ox: f32,
    pub oy: f32,
    /// the plan's size
    pub pw: f32,
    pub ph: f32,
    /// what the background layer was drawn for, 0 for nothing yet
    bg_key: u32,
    /// pixels outside it are left alone (see `set_clip`)
    clip: Clip,
}

impl Screen {
    pub(crate) fn new(w: i32, h: i32) -> Self {
        let mut s = Screen {
            layers: vec![Vec::new()],
            cur: 0,
            w: 0,
            h: 0,
            sw: 0,
            sh: 0,
            k: 1.0,
            ox: 0.0,
            oy: 0.0,
            pw: 1.0,
            ph: 1.0,
            bg_key: 0,
            clip: Clip::default(),
        };
        s.resize(w, h);
        s
    }

    pub(crate) fn resize(&mut self, w: i32, h: i32) {
        let w = w.clamp(MIN_SIDE, MAX_SIDE);
        let h = h.clamp(MIN_SIDE, MAX_SIDE);
        for l in &mut self.layers {
            l.clear();
            l.resize((w * h) as usize, 0);
        }
        self.w = w;
        self.h = h;
        self.sw = w;
        self.sh = h;
        self.bg_key = 0;
        self.no_clip();
        self.fit(self.pw, self.ph);
    }

    #[doc(hidden)]
    pub fn pixels_ptr(&self) -> *const u32 {
        self.layers[SHOWN].as_ptr()
    }

    /// Scales a plan of pw x ph units to fit the screen, centred.
    pub fn fit(&mut self, pw: f32, ph: f32) {
        self.pw = pw;
        self.ph = ph;
        self.k = (self.sw as f32 / pw).min(self.sh as f32 / ph);
        self.ox = (self.sw as f32 - pw * self.k) / 2.0;
        self.oy = (self.sh as f32 - ph * self.k) / 2.0;
    }

    /// The visible area in plan units (wider or taller than the plan when
    /// the screen's shape differs): x0, y0, x1, y1. Fill it for full-bleed
    /// backgrounds.
    pub fn visible(&self) -> (f32, f32, f32, f32) {
        (-self.ox / self.k, -self.oy / self.k, (self.sw as f32 - self.ox) / self.k, (self.sh as f32 - self.oy) / self.k)
    }

    /// A screen pixel (from the page's pointer) in plan units.
    pub fn to_plan(&self, px: i32, py: i32) -> (f32, f32) {
        ((px as f32 + 0.5 - self.ox) / self.k, (py as f32 + 0.5 - self.oy) / self.k)
    }

    /// Makes the screen an overlay the page lays over a picture of its own
    /// (a game drawn in 3D): each pixel keeps its coverage as alpha, so a
    /// screen cleared to 0 holds premultiplied colour where it was drawn on
    /// and stays clear elsewhere. Off, every pixel drawn on is opaque.
    pub fn overlay(&mut self, on: bool) {
        colour::set_overlay(on);
    }

    /// One screen pixel in plan units, for hairlines.
    pub fn hair(&self) -> f32 {
        1.0 / self.k
    }

    #[inline(always)]
    fn sx(&self, x: f32) -> f32 {
        self.ox + x * self.k
    }
    #[inline(always)]
    fn sy(&self, y: f32) -> f32 {
        self.oy + y * self.k
    }

    /// The whole pixels a box of screen pixels touches, inside the clip
    /// rectangle: x0, y0, x1, y1 (empty when x1 <= x0 or y1 <= y0).
    fn clipped(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> (i32, i32, i32, i32) {
        (
            (x0.floor() as i32).max(self.clip.x0),
            (y0.floor() as i32).max(self.clip.y0),
            (x1.ceil() as i32).min(self.clip.x1),
            (y1.ceil() as i32).min(self.clip.y1),
        )
    }

    /// Rows y0..y1 of the buffer being drawn into, with their y; y0 and y1
    /// come from [`Screen::clipped`].
    fn rows(&mut self, y0: i32, y1: i32) -> impl Iterator<Item = (i32, &mut [u32])> {
        let (w, y0) = (self.w as usize, y0.max(0));
        let rows = self.layers[self.cur].chunks_exact_mut(w).enumerate();
        rows.skip(y0 as usize).take((y1 - y0).max(0) as usize).map(|(j, row)| (j as i32, row))
    }

    /// Draws only inside this plan rectangle (whole pixels it touches) until
    /// `no_clip`: for redrawing one part of the background, say.
    pub fn set_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.no_clip();
        let (x0, y0, x1, y1) = self.clipped(self.sx(x), self.sy(y), self.sx(x + w), self.sy(y + h));
        self.clip = Clip { x0, y0, x1: x1.max(x0), y1: y1.max(y0) };
    }

    pub fn no_clip(&mut self) {
        self.clip = Clip { x0: 0, y0: 0, x1: self.w, y1: self.h };
    }

    /// Row j of the buffer being drawn into, for games that draw their own
    /// spans (a road, a sky); blend into it with [`blend`].
    pub fn row_px(&mut self, j: i32) -> &mut [u32] {
        let w = self.w as usize;
        let j = j.clamp(0, self.h - 1) as usize;
        &mut self.layers[self.cur][j * w..(j + 1) * w]
    }

    /// Runs `draw` on layer `layer`, then goes back to the one before.
    fn on_layer(&mut self, layer: usize, draw: impl FnOnce(&mut Screen)) {
        let keep = self.cur;
        self.cur = layer;
        draw(self);
        self.cur = keep;
    }

    /// Size of the buffer being drawn into, in pixels.
    pub fn size_px(&self) -> (i32, i32) {
        (self.w, self.h)
    }

    /// Plan to screen pixels.
    pub fn px(&self, x: f32, y: f32) -> (f32, f32) {
        (self.sx(x), self.sy(y))
    }

    /// One row of pixels j from x0 to x1 (screen pixels, fractional ends
    /// anti-aliased), clipped.
    pub fn span_px(&mut self, j: i32, x0: f32, x1: f32, c: u32, alpha: f32) {
        if j < self.clip.y0 || j >= self.clip.y1 || x1 <= x0 || alpha <= 0.0 {
            return;
        }
        let (a, b) = ((x0.floor() as i32).max(self.clip.x0), (x1.ceil() as i32).min(self.clip.x1));
        let full = weight(1.0, alpha);
        let row = self.row_px(j);
        for i in a..b {
            let cov = ((i as f32 + 1.0).min(x1) - (i as f32).max(x0)).clamp(0.0, 1.0);
            blend(&mut row[i as usize], c, if cov >= 1.0 { full } else { weight(cov, alpha) });
        }
    }
}
