//! Filled shapes, anti-aliased: boxes, lines, shadows, discs, rings and
//! polygons.
//!
//! Each shape works out how much of every pixel it covers (0..1) from the
//! distance between the pixel's centre (`i + 0.5`, `j + 0.5`) and its edge,
//! then blends its colour in by that much.

use super::{blend, mix, weight, Screen};

/// Distance from a point to the edge of a rounded box, negative inside. The
/// point is relative to the box's centre and folded into its first quarter:
/// `qx`, `qy` are |x| and |y| less the half-size of the box's straight part.
#[inline(always)]
fn round_box_dist(qx: f32, qy: f32, r: f32) -> f32 {
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

/// How far along the segment from a (with d = b - a, `inv` = 1 / |d|²) the
/// point closest to p is, 0..1, and the squared distance to it.
#[inline(always)]
fn along_segment(ax: f32, ay: f32, dx: f32, dy: f32, inv: f32, px: f32, py: f32) -> (f32, f32) {
    let t = (((px - ax) * dx + (py - ay) * dy) * inv).clamp(0.0, 1.0);
    let (qx, qy) = (ax + t * dx - px, ay + t * dy - py);
    (t, qx * qx + qy * qy)
}

impl Screen {
    // ---------- screen pixels ----------

    /// Fills the whole screen, letterbox included (or the clip rectangle).
    pub fn clear(&mut self, c: u32) {
        let clip = self.clip;
        self.box_px(clip.x0, clip.y0, clip.x1 - clip.x0, clip.y1 - clip.y0, c);
    }

    /// A crisp rectangle on whole screen pixels.
    pub fn box_px(&mut self, x: i32, y: i32, w: i32, h: i32, c: u32) {
        let (x0, y0) = (x.max(self.clip.x0), y.max(self.clip.y0));
        let (x1, y1) = ((x + w).min(self.clip.x1), (y + h).min(self.clip.y1));
        for (_, row) in self.rows(y0, y1) {
            row[x0 as usize..x1.max(x0) as usize].fill(c);
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
        // centre, and the half-size of the straight part between the corners
        let (cx, cy, hw, hh) = (x + w / 2.0, y + h / 2.0, w / 2.0 - r, h / 2.0 - r);
        let (x0, y0, x1, y1) = self.clipped(x, y, x + w, y + h);
        let grad = top != bottom;
        for (j, row) in self.rows(y0, y1) {
            let qy = (j as f32 + 0.5 - cy).abs() - hh;
            let c = if grad { mix(bottom, top, (j as f32 + 0.5 - y) / h) } else { top };
            for i in x0..x1 {
                let qx = (i as f32 + 0.5 - cx).abs() - hw;
                let cov = (0.5 - round_box_dist(qx, qy, r)).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, weight(cov, alpha));
                }
            }
        }
    }

    /// A line from a to b with round ends, radius r, in screen pixels.
    pub fn capsule_px(&mut self, ax: f32, ay: f32, bx: f32, by: f32, r: f32, c: u32, alpha: f32) {
        if r <= 0.0 || alpha <= 0.0 {
            return;
        }
        let (x0, y0, x1, y1) =
            self.clipped(ax.min(bx) - r - 1.0, ay.min(by) - r - 1.0, ax.max(bx) + r + 1.0, ay.max(by) + r + 1.0);
        let (dx, dy) = (bx - ax, by - ay);
        let ll = dx * dx + dy * dy;
        let inv = if ll > 0.0 { 1.0 / ll } else { 0.0 };
        // squared distances inside which a pixel is fully covered, and
        // outside which it isn't touched
        let in2 = if r > 0.5 { (r - 0.5) * (r - 0.5) } else { 0.0 };
        let out2 = (r + 0.5) * (r + 0.5);
        let full = weight(1.0, alpha);
        for (j, row) in self.rows(y0, y1) {
            let py = j as f32 + 0.5;
            for i in x0..x1 {
                let (_, d2) = along_segment(ax, ay, dx, dy, inv, i as f32 + 0.5, py);
                if d2 >= out2 {
                    continue;
                }
                if d2 <= in2 {
                    blend(&mut row[i as usize], c, full);
                    continue;
                }
                let cov = (r + 0.5 - d2.sqrt()).clamp(0.0, 1.0);
                blend(&mut row[i as usize], c, weight(cov, alpha));
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
        let (ax, ay, bx, by) = (self.sx(ax), self.sy(ay), self.sx(bx), self.sy(by));
        let (ra, rb) = (ra * self.k, rb * self.k);
        let rm = ra.max(rb);
        if rm <= 0.0 || alpha <= 0.0 {
            return;
        }
        let (x0, y0, x1, y1) =
            self.clipped(ax.min(bx) - rm - 1.0, ay.min(by) - rm - 1.0, ax.max(bx) + rm + 1.0, ay.max(by) + rm + 1.0);
        let (dx, dy) = (bx - ax, by - ay);
        let ll = dx * dx + dy * dy;
        let inv = if ll > 0.0 { 1.0 / ll } else { 0.0 };
        let out2 = (rm + 0.5) * (rm + 0.5);
        for (j, row) in self.rows(y0, y1) {
            let py = j as f32 + 0.5;
            for i in x0..x1 {
                let (t, d2) = along_segment(ax, ay, dx, dy, inv, i as f32 + 0.5, py);
                if d2 >= out2 {
                    continue;
                }
                let r = ra + (rb - ra) * t;
                let cov = (r + 0.5 - d2.sqrt()).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], mix(cb, ca, t), weight(cov, alpha));
                }
            }
        }
    }

    /// A soft shadow: a rounded box blurred over `soft` units.
    pub fn shadow(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, soft: f32, c: u32, alpha: f32) {
        self.soft_box(x, y, w, h, r, soft, c, alpha, 0.0);
    }

    /// A shadow that leaves out its middle (`hollow` units in from the box's
    /// edge), for shadows under something opaque: cheaper.
    pub fn shadow_hollow(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        soft: f32,
        c: u32,
        alpha: f32,
        hollow: f32,
    ) {
        self.soft_box(x, y, w, h, r, soft, c, alpha, hollow);
    }

    fn soft_box(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, soft: f32, c: u32, alpha: f32, hollow: f32) {
        let k = self.k;
        let (x0, y0, x1, y1) = self.clipped(
            self.sx(x - soft) - 1.0,
            self.sy(y - soft) - 1.0,
            self.sx(x + w + soft) + 1.0,
            self.sy(y + h + soft) + 1.0,
        );
        // the blur's width in pixels
        let spread = soft * k + 1.0;
        let (cx, cy) = (self.sx(x + w / 2.0), self.sy(y + h / 2.0));
        let rr = r.min(w.min(h) / 2.0) * k;
        let (hw, hh) = (w / 2.0 * k - rr, h / 2.0 * k - rr);
        let skip = -hollow * k - rr;
        for (j, row) in self.rows(y0, y1) {
            let qy = (j as f32 + 0.5 - cy).abs() - hh;
            for i in x0..x1 {
                let qx = (i as f32 + 0.5 - cx).abs() - hw;
                if hollow > 0.0 && qx < skip && qy < skip {
                    continue;
                }
                let t = (0.5 - round_box_dist(qx, qy, rr) / spread).clamp(0.0, 1.0);
                if t > 0.0 {
                    let smooth = t * t * (3.0 - 2.0 * t);
                    blend(&mut row[i as usize], c, weight(smooth, alpha));
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
        let full = weight(1.0, alpha);
        let y0 = ((py0 - ry - 1.0).floor() as i32).max(self.clip.y0);
        let y1 = ((py0 + ry + 1.0).ceil() as i32).min(self.clip.y1);
        let clip = self.clip;
        for (j, row) in self.rows(y0, y1) {
            let dy = j as f32 + 0.5 - py0;
            // this row touches pixels within xo of the middle, and covers
            // those within xi fully
            let xo = half_span(rx + 1.0, ry + 1.0, dy);
            let xi = half_span(rx - 1.2, ry - 1.2, dy);
            if xo < 0.0 {
                continue;
            }
            let x0 = ((px0 - xo).floor() as i32).max(clip.x0);
            let x1 = ((px0 + xo).ceil() as i32).min(clip.x1);
            let y = dy * iry;
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
                    blend(&mut row[i as usize], c, weight(cov, alpha));
                }
            }
        }
    }

    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: u32, alpha: f32) {
        self.ellipse(cx, cy, r, r, c, alpha);
    }

    /// A circle's outline `t` units thick, inside radius r.
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, t: f32, c: u32, alpha: f32) {
        let (px0, py0) = (self.sx(cx), self.sy(cy));
        let (outer, inner) = (r * self.k, (r - t) * self.k);
        let y0 = ((py0 - outer - 1.0).floor() as i32).max(self.clip.y0);
        let y1 = ((py0 + outer + 1.0).ceil() as i32).min(self.clip.y1);
        let clip = self.clip;
        for (j, row) in self.rows(y0, y1) {
            let dy = j as f32 + 0.5 - py0;
            // pixels within xi of the middle are in the hole
            let xo = half_span(outer + 1.0, outer + 1.0, dy);
            let xi = half_span(inner - 1.0, inner - 1.0, dy);
            if xo < 0.0 {
                continue;
            }
            let x0 = ((px0 - xo).floor() as i32).max(clip.x0);
            let x1 = ((px0 + xo).ceil() as i32).min(clip.x1);
            for i in x0..x1 {
                let dx = i as f32 + 0.5 - px0;
                if dx.abs() < xi {
                    continue;
                }
                let d = (dx * dx + dy * dy).sqrt();
                let cov = (outer + 0.5 - d).clamp(0.0, 1.0) - (inner + 0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, weight(cov, alpha));
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
        let edge = |k: usize| (pts[k], pts[(k + 1) % n]);
        let area: f32 = (0..n).map(edge).map(|(a, b)| a.0 * b.1 - b.0 * a.1).sum();
        let sign = if area > 0.0 { 1.0 } else { -1.0 };
        // each edge as its outward normal and offset: a pixel's distance
        // outside the edge is nx * x + ny * y - d
        let mut edges = [(0.0f32, 0.0f32, 0.0f32); 8];
        let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for k in 0..n {
            let (a, b) = edge(k);
            let (ax, ay, bx, by) = (self.sx(a.0), self.sy(a.1), self.sx(b.0), self.sy(b.1));
            let (ex, ey) = (bx - ax, by - ay);
            let l = (ex * ex + ey * ey).sqrt().max(1e-6);
            let (nx, ny) = (ey / l * sign, -ex / l * sign);
            edges[k] = (nx, ny, nx * ax + ny * ay);
            minx = minx.min(ax);
            maxx = maxx.max(ax);
            miny = miny.min(ay);
            maxy = maxy.max(ay);
        }
        let edges = &edges[..n];
        let (x0, y0, x1, y1) = self.clipped(minx - 1.0, miny - 1.0, maxx + 1.0, maxy + 1.0);
        for (j, row) in self.rows(y0, y1) {
            let py = j as f32 + 0.5;
            for i in x0..x1 {
                let px = i as f32 + 0.5;
                // outside a convex shape is outside its furthest edge
                let d = edges.iter().fold(f32::MIN, |d, e| d.max(e.0 * px + e.1 * py - e.2));
                let cov = (0.5 - d).clamp(0.0, 1.0);
                if cov > 0.0 {
                    blend(&mut row[i as usize], c, weight(cov, alpha));
                }
            }
        }
    }

    pub fn tri(&mut self, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: u32, alpha: f32) {
        self.poly(&[a, b, c], col, alpha);
    }
}
