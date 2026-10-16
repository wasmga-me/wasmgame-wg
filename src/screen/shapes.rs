//! Filled shapes, anti-aliased: boxes, lines, shadows, discs, rings and
//! polygons.

use super::{blend, cov256, mix, Screen};

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

impl Screen {
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
        let (x0, y0, x1, y1) =
            self.clip(ax.min(bx) - r - 1.0, ay.min(by) - r - 1.0, ax.max(bx) + r + 1.0, ay.max(by) + r + 1.0);
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
        let (x0, y0, x1, y1) =
            self.clip(ax.min(bx) - rm - 1.0, ay.min(by) - rm - 1.0, ax.max(bx) + rm + 1.0, ay.max(by) + rm + 1.0);
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
        self.shadow_(x, y, w, h, r, soft, c, alpha, hollow);
    }

    fn shadow_(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, soft: f32, c: u32, alpha: f32, hollow: f32) {
        let k = self.k;
        let (x0, y0, x1, y1) = self.clip(
            self.sx(x - soft) - 1.0,
            self.sy(y - soft) - 1.0,
            self.sx(x + w + soft) + 1.0,
            self.sy(y + h + soft) + 1.0,
        );
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
}
