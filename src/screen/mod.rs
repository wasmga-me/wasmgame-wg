//! A framebuffer at the screen's own size with anti-aliased shapes and text,
//! drawn in the game's own units (its plan, say 400x300) scaled to fit: the
//! Rust side of wgfb.h and wgview.h. Every method takes plan units unless
//! its name ends in `_px`.

mod background;
mod colour;
mod shapes;
mod sprite;
mod text;

use colour::cov256;
pub use colour::{blend, hex, mix, rgb};
pub use sprite::Sprite;

/// Most pixels a buffer may have (the page caps the canvas at 2.4 MP).
const MAX_SIDE: i32 = 4096;

/// The screen: layer 0 is what the page shows; more layers hold a still
/// background (see [`Screen::background`]).
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
    /// plan to screen: pixel = (v_x + x * k, v_y + y * k)
    pub k: f32,
    pub ox: f32,
    pub oy: f32,
    /// the plan's size
    pub pw: f32,
    pub ph: f32,
    bg_key: u32,
    /// pixels outside x0..x1, y0..y1 are left alone (see `set_clip`)
    cx0: i32,
    cy0: i32,
    cx1: i32,
    cy1: i32,
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
            cx0: 0,
            cy0: 0,
            cx1: 0,
            cy1: 0,
        };
        s.resize(w, h);
        s
    }

    pub(crate) fn resize(&mut self, w: i32, h: i32) {
        let w = w.clamp(64, MAX_SIDE);
        let h = h.clamp(64, MAX_SIDE);
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
        self.layers[0].as_ptr()
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

    fn buf(&mut self) -> &mut [u32] {
        &mut self.layers[self.cur]
    }

    /// Clipped pixel bounds x0, y0, x1, y1.
    fn clip(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> (i32, i32, i32, i32) {
        (
            (x0.floor() as i32).max(self.cx0),
            (y0.floor() as i32).max(self.cy0),
            (x1.ceil() as i32).min(self.cx1),
            (y1.ceil() as i32).min(self.cy1),
        )
    }

    /// Draws only inside this plan rectangle (whole pixels it touches) until
    /// `no_clip`: for redrawing one part of the background, say.
    pub fn set_clip(&mut self, x: f32, y: f32, w: f32, h: f32) {
        self.no_clip();
        let (x0, y0, x1, y1) = self.clip(self.sx(x), self.sy(y), self.sx(x + w), self.sy(y + h));
        (self.cx0, self.cy0, self.cx1, self.cy1) = (x0, y0, x1.max(x0), y1.max(y0));
    }

    pub fn no_clip(&mut self) {
        (self.cx0, self.cy0, self.cx1, self.cy1) = (0, 0, self.w, self.h);
    }

    /// Row j of the buffer being drawn into, for games that draw their own
    /// spans (a road, a sky); blend into it with [`blend`].
    pub fn row_px(&mut self, j: i32) -> &mut [u32] {
        let sw = self.w as usize;
        let j = j.clamp(0, self.h - 1) as usize;
        &mut self.layers[self.cur][j * sw..(j + 1) * sw]
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
        if j < self.cy0 || j >= self.cy1 || x1 <= x0 || alpha <= 0.0 {
            return;
        }
        let (a, b) = (((x0.floor() as i32).max(self.cx0)), ((x1.ceil() as i32).min(self.cx1)));
        let full = cov256(1.0, alpha);
        let row = self.row_px(j);
        for i in a..b {
            let cov = ((i as f32 + 1.0).min(x1) - (i as f32).max(x0)).clamp(0.0, 1.0);
            blend(&mut row[i as usize], c, if cov >= 1.0 { full } else { cov256(cov, alpha) });
        }
    }
}
