//! A framebuffer at the screen's own size with anti-aliased shapes and text,
//! drawn in the game's own units (its plan, say 400x300) scaled to fit: the
//! Rust side of wgfb.h and wgview.h. Every method takes plan units unless
//! its name ends in `_px`.

use crate::font::{Glyph, SDF_CHARS, SDF_DATA, SDF_EM, SDF_GLYPHS, SDF_SPREAD};

/// Most pixels a buffer may have (the page caps the canvas at 2.4 MP).
const MAX_SIDE: i32 = 4096;

/// Colour as the page reads it: 0xAABBGGRR.
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF00_0000 | (b as u32) << 16 | (g as u32) << 8 | r as u32
}

/// `0xRRGGBB` as written in CSS, for colours copied from a design.
pub const fn hex(c: u32) -> u32 {
    rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Blends colour `c` over the pixel with coverage `a` (0..256).
#[inline(always)]
pub fn blend(p: &mut u32, c: u32, a: u32) {
    if a >= 256 {
        *p = c;
        return;
    }
    if a == 0 {
        return;
    }
    let d = *p;
    let ia = 256 - a;
    let rb = (((c & 0xFF00FF) * a + (d & 0xFF00FF) * ia) >> 8) & 0xFF00FF;
    let g = (((c & 0x00FF00) * a + (d & 0x00FF00) * ia) >> 8) & 0x00FF00;
    *p = 0xFF00_0000 | rb | g;
}

/// Mixes two colours: `t` = 0..1 of `c` over `d`.
pub fn mix(c: u32, d: u32, t: f32) -> u32 {
    let mut out = d;
    blend(&mut out, c, (t.clamp(0.0, 1.0) * 256.0) as u32);
    out
}

#[inline(always)]
fn cov256(cov: f32, alpha: f32) -> u32 {
    (cov * alpha * 256.0) as u32
}

/// Distance from a point (relative to a rounded box's centre) to its edge.
#[inline(always)]
fn rbox_dist(qx: f32, qy: f32, r: f32) -> f32 {
    if qx <= 0.0 && qy <= 0.0 {
        qx.max(qy) - r
    } else if qx <= 0.0 {
        qy - r
    } else if qy <= 0.0 {
        qx - r
    } else {
        (qx * qx + qy * qy).sqrt() - r
    }
}

/// Half the width of an ellipse at height dy from its middle, -1 outside.
fn half_span(rx: f32, ry: f32, dy: f32) -> f32 {
    if rx <= 0.0 || ry <= 0.0 {
        return -1.0;
    }
    let t = 1.0 - dy * dy / (ry * ry);
    if t > 0.0 {
        rx * t.sqrt()
    } else {
        -1.0
    }
}

/// Something drawn many times the same (a card back), drawn once and then
/// copied with [`Screen::sprite`]. Keep it in the game, outside the save.
#[derive(Default)]
pub struct Sprite {
    px: Vec<u32>,
    w: i32,
    h: i32,
    key: u32,
    alpha: bool,
}

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
        let mut s = Screen { layers: vec![Vec::new()], cur: 0, w: 0, h: 0, sw: 0, sh: 0, k: 1.0, ox: 0.0, oy: 0.0, pw: 1.0, ph: 1.0, bg_key: 0, cx0: 0, cy0: 0, cx1: 0, cy1: 0 };
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

    // ---------- screen pixels ----------

    /// Fills the whole screen, letterbox included (or the clip rectangle).
    pub fn clear(&mut self, c: u32) {
        let (x0, y0, x1, y1) = (self.cx0, self.cy0, self.cx1, self.cy1);
        self.box_px(x0, y0, x1 - x0, y1 - y0, c);
    }

    /// A crisp rectangle on whole screen pixels.
    pub fn box_px(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        let (x0, y0) = (x.max(self.cx0), y.max(self.cy0));
        let (x1, y1) = ((x + w).min(self.cx1), (y + h).min(self.cy1));
        let sw = self.w as usize;
        for j in y0..y1 {
            let row = j as usize * sw;
            self.buf()[row + x0 as usize..row + x1.max(x0) as usize].fill(c);
        }
    }

    /// Rounded rectangle in screen pixels.
    pub fn rrect_px(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: u32, alpha: f32) {
        self.rrect_grad_px(x, y, w, h, r, c, c, alpha);
    }

    fn rrect_grad_px(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, top: u32, bottom: u32, alpha: f32) {
        if w <= 0.0 || h <= 0.0 || alpha <= 0.0 {
            return;
        }
        let r = r.min(w.min(h) / 2.0).max(0.0);
        let (cx, cy, hw, hh) = (x + w / 2.0, y + h / 2.0, w / 2.0 - r, h / 2.0 - r);
        let (x0, y0, x1, y1) = self.clip(x, y, x + w, y + h);
        let sw = self.w as usize;
        let grad = top != bottom;
        let buf = &mut self.layers[self.cur];
        for j in y0..y1 {
            let qy = (j as f32 + 0.5 - cy).abs() - hh;
            let c = if grad { mix(bottom, top, (j as f32 + 0.5 - y) / h) } else { top };
            let row = &mut buf[j as usize * sw..(j as usize + 1) * sw];
            for i in x0..x1 {
                let qx = (i as f32 + 0.5 - cx).abs() - hw;
                let cov = (0.5 - rbox_dist(qx, qy, r)).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, cov256(cov, alpha));
                }
            }
        }
    }

    /// A line from a to b with round ends, radius r, in screen pixels.
    pub fn capsule_px(&mut self, ax: f32, ay: f32, bx: f32, by: f32, r: f32, c: u32, alpha: f32) {
        if r <= 0.0 || alpha <= 0.0 {
            return;
        }
        let (x0, y0, x1, y1) = self.clip(ax.min(bx) - r - 1.0, ay.min(by) - r - 1.0, ax.max(bx) + r + 1.0, ay.max(by) + r + 1.0);
        let (dx, dy) = (bx - ax, by - ay);
        let ll = dx * dx + dy * dy;
        let inv = if ll > 0.0 { 1.0 / ll } else { 0.0 };
        let in2 = if r > 0.5 { (r - 0.5) * (r - 0.5) } else { 0.0 };
        let out2 = (r + 0.5) * (r + 0.5);
        let full = cov256(1.0, alpha);
        let sw = self.w as usize;
        let buf = &mut self.layers[self.cur];
        for j in y0..y1 {
            let py = j as f32 + 0.5;
            let row = &mut buf[j as usize * sw..(j as usize + 1) * sw];
            for i in x0..x1 {
                let px = i as f32 + 0.5;
                let t = (((px - ax) * dx + (py - ay) * dy) * inv).clamp(0.0, 1.0);
                let (qx, qy) = (ax + t * dx - px, ay + t * dy - py);
                let d2 = qx * qx + qy * qy;
                if d2 >= out2 {
                    continue;
                }
                if d2 <= in2 {
                    blend(&mut row[i as usize], c, full);
                    continue;
                }
                let cov = (r + 0.5 - d2.sqrt()).clamp(0.0, 1.0);
                blend(&mut row[i as usize], c, cov256(cov, alpha));
            }
        }
    }

    // ---------- plan units ----------

    /// A rounded box.
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: u32, alpha: f32) {
        let k = self.k;
        self.rrect_px(self.sx(x), self.sy(y), w * k, h * k, r * k, c, alpha);
    }

    /// A rounded box whose colour runs from `top` to `bottom`.
    pub fn rect_grad(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, top: u32, bottom: u32, alpha: f32) {
        let k = self.k;
        self.rrect_grad_px(self.sx(x), self.sy(y), w * k, h * k, r * k, top, bottom, alpha);
    }

    /// A line with round ends, half-width r.
    pub fn line(&mut self, ax: f32, ay: f32, bx: f32, by: f32, r: f32, c: u32, alpha: f32) {
        self.capsule_px(self.sx(ax), self.sy(ay), self.sx(bx), self.sy(by), r * self.k, c, alpha);
    }

    /// A line whose half-width and colour change along it, ra and ca at a to
    /// rb and cb at b; consecutive ones that share ends join without a seam.
    pub fn line_grad(&mut self, ax: f32, ay: f32, bx: f32, by: f32, ra: f32, rb: f32, ca: u32, cb: u32, alpha: f32) {
        let (ax, ay, bx, by, ra, rb) = (self.sx(ax), self.sy(ay), self.sx(bx), self.sy(by), ra * self.k, rb * self.k);
        let rm = ra.max(rb);
        if rm <= 0.0 || alpha <= 0.0 {
            return;
        }
        let (x0, y0, x1, y1) = self.clip(ax.min(bx) - rm - 1.0, ay.min(by) - rm - 1.0, ax.max(bx) + rm + 1.0, ay.max(by) + rm + 1.0);
        let (dx, dy) = (bx - ax, by - ay);
        let ll = dx * dx + dy * dy;
        let inv = if ll > 0.0 { 1.0 / ll } else { 0.0 };
        let out2 = (rm + 0.5) * (rm + 0.5);
        let sw = self.w as usize;
        let buf = &mut self.layers[self.cur];
        for j in y0..y1 {
            let py = j as f32 + 0.5;
            let row = &mut buf[j as usize * sw..(j as usize + 1) * sw];
            for i in x0..x1 {
                let px = i as f32 + 0.5;
                let t = (((px - ax) * dx + (py - ay) * dy) * inv).clamp(0.0, 1.0);
                let (qx, qy) = (ax + t * dx - px, ay + t * dy - py);
                let d2 = qx * qx + qy * qy;
                if d2 >= out2 {
                    continue;
                }
                let r = ra + (rb - ra) * t;
                let cov = (r + 0.5 - d2.sqrt()).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], mix(cb, ca, t), cov256(cov, alpha));
                }
            }
        }
    }

    /// A soft shadow: a rounded box blurred over `soft` units.
    pub fn shadow(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, soft: f32, c: u32, alpha: f32) {
        self.shadow_(x, y, w, h, r, soft, c, alpha, 0.0);
    }

    /// A shadow that leaves out its middle (`hollow` units in from the box's
    /// edge), for shadows under something opaque: cheaper.
    pub fn shadow_hollow(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, soft: f32, c: u32, alpha: f32, hollow: f32) {
        self.shadow_(x, y, w, h, r, soft, c, alpha, hollow);
    }

    fn shadow_(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, soft: f32, c: u32, alpha: f32, hollow: f32) {
        let k = self.k;
        let (x0, y0, x1, y1) = self.clip(self.sx(x - soft) - 1.0, self.sy(y - soft) - 1.0, self.sx(x + w + soft) + 1.0, self.sy(y + h + soft) + 1.0);
        let s = soft * k + 1.0;
        let (cx, cy) = (self.sx(x + w / 2.0), self.sy(y + h / 2.0));
        let rr = r.min(w.min(h) / 2.0) * k;
        let (hw, hh) = (w / 2.0 * k - rr, h / 2.0 * k - rr);
        let skip = -hollow * k - rr;
        let sw = self.w as usize;
        let buf = &mut self.layers[self.cur];
        for j in y0..y1 {
            let qy = (j as f32 + 0.5 - cy).abs() - hh;
            let row = &mut buf[j as usize * sw..(j as usize + 1) * sw];
            for i in x0..x1 {
                let qx = (i as f32 + 0.5 - cx).abs() - hw;
                if hollow > 0.0 && qx < skip && qy < skip {
                    continue;
                }
                let t = (0.5 - rbox_dist(qx, qy, rr) / s).clamp(0.0, 1.0);
                if t > 0.0 {
                    blend(&mut row[i as usize], c, cov256(t * t * (3.0 - 2.0 * t), alpha));
                }
            }
        }
    }

    /// An ellipse; rx may shrink to nothing, as a disc turning over.
    pub fn ellipse(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: u32, alpha: f32) {
        let (rx, ry) = ((rx * self.k).max(0.35), ry * self.k);
        if ry <= 0.0 || alpha <= 0.0 {
            return;
        }
        let (px0, py0) = (self.sx(cx), self.sy(cy));
        let (irx, iry) = (1.0 / rx, 1.0 / ry);
        let full = cov256(1.0, alpha);
        let y0 = ((py0 - ry - 1.0).floor() as i32).max(self.cy0);
        let y1 = ((py0 + ry + 1.0).ceil() as i32).min(self.cy1);
        let (sw, cx0, cx1) = (self.w as usize, self.cx0, self.cx1);
        let buf = &mut self.layers[self.cur];
        for j in y0..y1 {
            let dy = j as f32 + 0.5 - py0;
            let xo = half_span(rx + 1.0, ry + 1.0, dy);
            let xi = half_span(rx - 1.2, ry - 1.2, dy);
            if xo < 0.0 {
                continue;
            }
            let x0 = ((px0 - xo).floor() as i32).max(cx0);
            let x1 = ((px0 + xo).ceil() as i32).min(cx1);
            let y = dy * iry;
            let row = &mut buf[j as usize * sw..(j as usize + 1) * sw];
            for i in x0..x1 {
                let dx = i as f32 + 0.5 - px0;
                if dx.abs() < xi {
                    blend(&mut row[i as usize], c, full);
                    continue;
                }
                let x = dx * irx;
                let f = x * x + y * y - 1.0;
                // distance to the edge, from the field's value over its slope
                let (gx, gy) = (x * irx, y * iry);
                let g = 2.0 * (gx * gx + gy * gy).sqrt();
                let cov = (0.5 - if g > 0.0 { f / g } else { -1.0 }).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, cov256(cov, alpha));
                }
            }
        }
    }

    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: u32, alpha: f32) {
        self.ellipse(cx, cy, r, r, c, alpha);
    }

    /// A circle's outline `t` units thick, inside radius r.
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, t: f32, c: u32, alpha: f32) {
        let (px0, py0, ro, ri) = (self.sx(cx), self.sy(cy), r * self.k, (r - t) * self.k);
        let y0 = ((py0 - ro - 1.0).floor() as i32).max(self.cy0);
        let y1 = ((py0 + ro + 1.0).ceil() as i32).min(self.cy1);
        let (sw, cx0, cx1) = (self.w as usize, self.cx0, self.cx1);
        let buf = &mut self.layers[self.cur];
        for j in y0..y1 {
            let dy = j as f32 + 0.5 - py0;
            let xo = half_span(ro + 1.0, ro + 1.0, dy);
            let xi = half_span(ri - 1.0, ri - 1.0, dy);
            if xo < 0.0 {
                continue;
            }
            let x0 = ((px0 - xo).floor() as i32).max(cx0);
            let x1 = ((px0 + xo).ceil() as i32).min(cx1);
            let row = &mut buf[j as usize * sw..(j as usize + 1) * sw];
            for i in x0..x1 {
                let dx = i as f32 + 0.5 - px0;
                if dx.abs() < xi {
                    continue;
                }
                let d = (dx * dx + dy * dy).sqrt();
                let cov = (ro + 0.5 - d).clamp(0.0, 1.0) - (ri + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, cov256(cov, alpha));
                }
            }
        }
    }

    /// A filled convex polygon of up to 8 points, either winding.
    pub fn poly(&mut self, pts: &[(f32, f32)], c: u32, alpha: f32) {
        let n = pts.len();
        if !(3..=8).contains(&n) || alpha <= 0.0 {
            return;
        }
        let mut area = 0.0;
        for k in 0..n {
            let (a, b) = (pts[k], pts[(k + 1) % n]);
            area += a.0 * b.1 - b.0 * a.1;
        }
        let sgn = if area > 0.0 { 1.0 } else { -1.0 };
        let mut edges = [(0.0f32, 0.0f32, 0.0f32); 8];
        let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for k in 0..n {
            let (a, b) = (pts[k], pts[(k + 1) % n]);
            let (ax, ay, bx, by) = (self.sx(a.0), self.sy(a.1), self.sx(b.0), self.sy(b.1));
            let (ex, ey) = (bx - ax, by - ay);
            let l = (ex * ex + ey * ey).sqrt().max(1e-6);
            // outward normal and offset
            let (nx, ny) = (ey / l * sgn, -ex / l * sgn);
            edges[k] = (nx, ny, nx * ax + ny * ay);
            minx = minx.min(ax);
            maxx = maxx.max(ax);
            miny = miny.min(ay);
            maxy = maxy.max(ay);
        }
        let (x0, y0, x1, y1) = self.clip(minx - 1.0, miny - 1.0, maxx + 1.0, maxy + 1.0);
        let sw = self.w as usize;
        let buf = &mut self.layers[self.cur];
        for j in y0..y1 {
            let py = j as f32 + 0.5;
            let row = &mut buf[j as usize * sw..(j as usize + 1) * sw];
            for i in x0..x1 {
                let px = i as f32 + 0.5;
                let mut d = f32::MIN;
                for e in &edges[..n] {
                    d = d.max(e.0 * px + e.1 * py - e.2);
                }
                let cov = (0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, cov256(cov, alpha));
                }
            }
        }
    }

    pub fn tri(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: u32, alpha: f32) {
        self.poly(&[a, b, c], col, alpha);
    }

    // ---------- text (Inter, from sdf_font.h) ----------

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
            let g = &SDF_GLYPHS[glyph(ch)];
            self.draw_glyph(g, x + g.left * k, top + g.top * k, k, c, alpha);
            x += g.adv * k;
        }
        x
    }

    fn draw_glyph(&mut self, g: &Glyph, gx: f32, gy: f32, k: f32, c: u32, alpha: f32) {
        let (w, h) = (g.w as i32, g.h as i32);
        if w == 0 || alpha <= 0.0 {
            return;
        }
        let f = &SDF_DATA[g.off as usize..g.off as usize + (w * h) as usize];
        let (x0, y0, x1, y1) = self.clip(gx, gy, gx + w as f32 * k, gy + h as f32 * k);
        let inv = 1.0 / k;
        let scale = SDF_SPREAD / 127.0 * k;
        let at = |x: i32, y: i32| -> f32 {
            if x < 0 || y < 0 || x >= w || y >= h {
                0.0
            } else {
                f[(y * w + x) as usize] as f32
            }
        };
        let sw = self.w as usize;
        let buf = &mut self.layers[self.cur];
        for j in y0..y1 {
            let fy = (j as f32 + 0.5 - gy) * inv - 0.5;
            let iy = fy.floor() as i32;
            let ty = fy - iy as f32;
            if iy < -1 || iy >= h {
                continue;
            }
            let row = &mut buf[j as usize * sw..(j as usize + 1) * sw];
            for i in x0..x1 {
                let fx = (i as f32 + 0.5 - gx) * inv - 0.5;
                let ix = fx.floor() as i32;
                let tx = fx - ix as f32;
                if ix < -1 || ix >= w {
                    continue;
                }
                let v = (at(ix, iy) * (1.0 - tx) + at(ix + 1, iy) * tx) * (1.0 - ty) + (at(ix, iy + 1) * (1.0 - tx) + at(ix + 1, iy + 1) * tx) * ty;
                let cov = ((v - 128.0) * scale + 0.5).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, cov256(cov, alpha));
                }
            }
        }
    }

    // ---------- a still background ----------

    /// What doesn't change from frame to frame (a board, a table) is drawn
    /// by `draw` into a second layer when `key` changes (or the screen is
    /// resized or reloaded), and copied under each frame.
    pub fn background(&mut self, key: u32, draw: impl FnOnce(&mut Screen)) {
        if self.layers.len() < 2 {
            self.layers.push(vec![0; (self.w * self.h) as usize]);
            self.bg_key = 0;
        }
        let key = hash(hash(key, &self.sw.to_le_bytes()), &self.sh.to_le_bytes()) | 1;
        if key != self.bg_key {
            let keep = self.cur;
            self.cur = 1;
            draw(self);
            self.cur = keep;
            self.bg_key = key;
        }
        let (front, back) = self.layers.split_at_mut(1);
        front[0].copy_from_slice(&back[0]);
    }

    /// Redraws part of the background in place (call before `background`
    /// in the same frame, with the same key): `draw` runs on the background
    /// layer, usually inside `set_clip` (cleared afterwards). For a board
    /// where one square changes, without redrawing the whole board.
    pub fn background_edit(&mut self, draw: impl FnOnce(&mut Screen)) {
        if self.layers.len() < 2 || self.bg_key == 0 {
            return; // nothing drawn yet: `background` draws it all anyway
        }
        let keep = self.cur;
        self.cur = 1;
        draw(self);
        self.no_clip();
        self.cur = keep;
    }

    pub(crate) fn forget_background(&mut self) {
        self.bg_key = 0;
    }

    // ---------- sprites ----------

    /// Draws the plan box x, y, w, h into `sp` with `draw(screen)` (in the
    /// same plan units) when `key` or the scale changed; its pixels must end
    /// up opaque. Then put it with [`Screen::sprite`].
    pub fn sprite_make(&mut self, sp: &mut Sprite, key: u32, x: f32, y: f32, w: f32, h: f32, draw: impl FnMut(&mut Screen)) {
        self.sprite_make_(sp, key, x, y, w, h, false, draw)
    }

    /// A sprite with soft or see-through edges (a gem, a coin): drawn twice
    /// (over black and white) to find its alpha, then blended when put. `draw`
    /// must draw the same thing both times.
    pub fn sprite_make_alpha(&mut self, sp: &mut Sprite, key: u32, x: f32, y: f32, w: f32, h: f32, mut draw: impl FnMut(&mut Screen)) {
        self.sprite_make_(sp, key, x, y, w, h, true, |s| draw(s))
    }

    fn sprite_make_<F: FnMut(&mut Screen)>(&mut self, sp: &mut Sprite, key: u32, x: f32, y: f32, w: f32, h: f32, alpha: bool, mut draw: F) {
        let key = hash(key, &[alpha as u8]);
        let key = hash(hash(key, &self.k.to_le_bytes()), &[x.to_bits() as u8, y.to_bits() as u8]) | 1;
        let (pw, ph) = ((w * self.k).ceil() as i32, (h * self.k).ceil() as i32);
        if sp.key == key && sp.w == pw && sp.h == ph {
            return;
        }
        sp.px.clear();
        sp.px.resize((pw.max(0) * ph.max(0)) as usize, 0);
        self.layers.push(core::mem::take(&mut sp.px));
        let keep = (self.cur, self.w, self.h, self.ox, self.oy, self.cx0, self.cy0, self.cx1, self.cy1);
        self.cur = self.layers.len() - 1;
        self.w = pw;
        self.h = ph;
        self.ox = -x * self.k;
        self.oy = -y * self.k;
        self.no_clip();
        if alpha {
            // drawn over black and over white; the difference is the alpha
            self.layers[self.cur].fill(0xFF00_0000);
            self.layers.push(vec![0xFFFF_FFFF; (pw.max(0) * ph.max(0)) as usize]);
            self.cur += 1;
            draw(self);
            self.cur -= 1;
            draw(self);
            let white = self.layers.pop().unwrap();
            for (b, w) in self.layers[self.cur].iter_mut().zip(white) {
                // over black a pixel is a*c, over white a*c + (1-a)*255, so
                // the difference gives the alpha; black keeps the colour
                // premultiplied
                let diff = |sh: u32| ((w >> sh) & 0xFF).saturating_sub((*b >> sh) & 0xFF);
                let a = 255 - ((diff(0) + diff(8) + diff(16)) / 3).min(255);
                *b = (*b & 0x00FF_FFFF) | a << 24;
            }
        } else {
            draw(self);
        }
        (self.cur, self.w, self.h, self.ox, self.oy, self.cx0, self.cy0, self.cx1, self.cy1) = keep;
        sp.px = self.layers.pop().unwrap();
        sp.alpha = alpha;
        sp.w = pw;
        sp.h = ph;
        sp.key = key;
    }

    /// Copies a sprite with its top left at plan x, y, to the nearest pixel.
    pub fn sprite(&mut self, sp: &Sprite, x: f32, y: f32) {
        let dx = (self.sx(x) + 0.5).floor() as i32;
        let dy = (self.sy(y) + 0.5).floor() as i32;
        let (i0, i1) = ((-dx).max(0), sp.w.min(self.w - dx));
        if i1 <= i0 {
            return;
        }
        let sw = self.w as usize;
        for j in 0..sp.h {
            let yy = dy + j;
            if yy < 0 || yy >= self.h {
                continue;
            }
            let d = yy as usize * sw + (dx + i0) as usize;
            let s = (j * sp.w + i0) as usize;
            let n = (i1 - i0) as usize;
            let (dst, src) = (&mut self.layers[self.cur][d..d + n], &sp.px[s..s + n]);
            if !sp.alpha {
                dst.copy_from_slice(src);
                continue;
            }
            for (p, &c) in dst.iter_mut().zip(src) {
                let a = c >> 24;
                if a == 0 {
                    continue;
                }
                // premultiplied: out = src + dst * (1 - a)
                let ia = 256 - (a + (a >> 7));
                let d = *p;
                let rb = ((c & 0xFF00FF) + (((d & 0xFF00FF) * ia) >> 8)) & 0xFF00FF;
                let g = ((c & 0x00FF00) + (((d & 0x00FF00) * ia) >> 8)) & 0x00FF00;
                *p = 0xFF00_0000 | rb | g;
            }
        }
    }
}

/// Middle of the digits' height below the line top, in em units.
const DIGIT_MID: f32 = 24.6;

fn glyph(ch: char) -> usize {
    if ch.is_ascii() {
        SDF_CHARS.iter().position(|&c| c == ch as u8).unwrap_or(0)
    } else {
        0
    }
}

fn text_width_px(s: &str, size: f32) -> f32 {
    let k = size / SDF_EM;
    s.chars().map(|ch| SDF_GLYPHS[glyph(ch)].adv * k).sum()
}

/// FNV-1a over bytes, for redraw signatures and cache keys.
pub fn hash(h: u32, bytes: &[u8]) -> u32 {
    let mut h = if h == 0 { 2_166_136_261 } else { h };
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(16_777_619);
    }
    h
}
