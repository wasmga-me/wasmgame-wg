//! Pictures from PNG files (art drawn or rendered elsewhere), scaled
//! smoothly to the screen and put with their own alpha.

use super::{colour, Screen};
use core::cell::RefCell;

/// Sizes of one image kept scaled, the most recent first.
const KEEP: usize = 4;

/// A picture decoded from a PNG, kept as premultiplied 0xAABBGGRR. Load it
/// once (in `new`, with `include_bytes!`) and keep it in the game, outside
/// the save. Each size it is drawn at is scaled once and kept.
pub struct Image {
    w: i32,
    h: i32,
    px: Vec<u32>,
    scaled: RefCell<Vec<Scaled>>,
}

struct Scaled {
    w: i32,
    h: i32,
    px: Vec<u32>,
}

impl Image {
    /// Decodes a PNG: grey, grey and alpha, RGB or RGBA at 8 bits a
    /// channel, or a palette (with tRNS) or grey at 1 to 8 bits, not
    /// interlaced. Panics on anything else,
    /// so a bad file shows up the first time the game runs.
    pub fn png(bytes: &[u8]) -> Image {
        let (w, h, px) = png::decode(bytes).unwrap_or_else(|e| panic!("PNG: {e}"));
        Image { w, h, px, scaled: RefCell::new(Vec::new()) }
    }

    /// The part x, y, w x h (file pixels) as an image of its own: one
    /// picture of a sheet holding many.
    pub fn crop(&self, x: i32, y: i32, w: i32, h: i32) -> Image {
        assert!(x >= 0 && y >= 0 && w > 0 && h > 0 && x + w <= self.w && y + h <= self.h, "crop outside the image");
        let px = (y..y + h).flat_map(|j| self.px[(j * self.w + x) as usize..][..w as usize].iter().copied()).collect();
        Image { w, h, px, scaled: RefCell::new(Vec::new()) }
    }

    /// Width and height in the file's pixels.
    pub fn size(&self) -> (i32, i32) {
        (self.w, self.h)
    }

    /// Height over width, to draw it at its own shape.
    pub fn aspect(&self) -> f32 {
        self.h as f32 / self.w as f32
    }

    /// Runs `f` on this image scaled to w x h pixels, scaling it first when
    /// that size isn't kept yet.
    fn with_scaled<R>(&self, w: i32, h: i32, f: impl FnOnce(&[u32]) -> R) -> R {
        let mut kept = self.scaled.borrow_mut();
        let i = match kept.iter().position(|s| s.w == w && s.h == h) {
            Some(i) => i,
            None => {
                if kept.len() == KEEP {
                    kept.pop();
                }
                kept.push(Scaled { w, h, px: resample(&self.px, self.w, self.h, w, h) });
                kept.len() - 1
            }
        };
        let s = kept.remove(i);
        kept.insert(0, s);
        f(&kept[0].px)
    }
}

/// Scales premultiplied pixels from sw x sh to dw x dh: each axis averages
/// the source pixels a target pixel covers when shrinking, and blends the
/// two nearest when growing.
fn resample(src: &[u32], sw: i32, sh: i32, dw: i32, dh: i32) -> Vec<u32> {
    let cols = taps(sw, dw);
    let rows = taps(sh, dh);
    // across first, into floats, then down
    let mut mid = vec![[0f32; 4]; (dw * sh) as usize];
    for y in 0..sh as usize {
        let line = &src[y * sw as usize..][..sw as usize];
        for (x, tap) in cols.iter().enumerate() {
            let mut acc = [0f32; 4];
            for &(i, wt) in tap {
                let p = line[i];
                for (c, a) in acc.iter_mut().enumerate() {
                    *a += ((p >> (c * 8)) & 0xFF) as f32 * wt;
                }
            }
            mid[y * dw as usize + x] = acc;
        }
    }
    let mut out = vec![0u32; (dw * dh) as usize];
    for (y, tap) in rows.iter().enumerate() {
        for x in 0..dw as usize {
            let mut acc = [0f32; 4];
            for &(j, wt) in tap {
                let m = mid[j * dw as usize + x];
                for c in 0..4 {
                    acc[c] += m[c] * wt;
                }
            }
            let a = (acc[3] + 0.5).clamp(0.0, 255.0) as u32;
            // premultiplied colour never exceeds its alpha
            let ch = |c: usize| ((acc[c] + 0.5).clamp(0.0, a as f32) as u32) << (c * 8);
            out[y * dw as usize + x] = ch(0) | ch(1) | ch(2) | a << 24;
        }
    }
    out
}

/// For each of n target pixels along an axis of len source pixels: the
/// source pixels it reads and their weights, summing to 1.
fn taps(len: i32, n: i32) -> Vec<Vec<(usize, f32)>> {
    let scale = len as f32 / n as f32;
    (0..n)
        .map(|i| {
            if scale > 1.0 {
                // shrinking: the box i*scale .. (i+1)*scale
                let (a, b) = (i as f32 * scale, (i as f32 + 1.0) * scale);
                let mut tap = Vec::new();
                let mut j = a.floor() as i32;
                while (j as f32) < b && j < len {
                    let cover = (b.min(j as f32 + 1.0) - a.max(j as f32)) / scale;
                    if cover > 0.0 {
                        tap.push((j as usize, cover));
                    }
                    j += 1;
                }
                tap
            } else {
                // growing: between the two nearest centres
                let c = ((i as f32 + 0.5) * scale - 0.5).clamp(0.0, (len - 1) as f32);
                let j = c.floor() as i32;
                let t = c - j as f32;
                if j + 1 < len && t > 0.0 {
                    vec![(j as usize, 1.0 - t), (j as usize + 1, t)]
                } else {
                    vec![(j as usize, 1.0)]
                }
            }
        })
        .collect()
}

/// Puts premultiplied colour `c`, already scaled by the draw's alpha,
/// over the pixel.
#[inline(always)]
fn over(p: &mut u32, c: u32) {
    let a = c >> 24;
    if a == 0 {
        return;
    }
    if a == 255 {
        *p = c;
        return;
    }
    let ia = 256 - (a + (a >> 7));
    let d = *p;
    let rb = ((c & 0xFF00FF) + (((d & 0xFF00FF) * ia) >> 8)) & 0xFF00FF;
    let g = ((c & 0x00FF00) + (((d & 0x00FF00) * ia) >> 8)) & 0x00FF00;
    *p = colour::alpha_over(d, ia) | rb | g;
}

/// Premultiplied colour scaled by `k` (0..256).
#[inline(always)]
fn fade(c: u32, k: u32) -> u32 {
    let rb = (((c & 0x00FF_00FF) * k) >> 8) & 0x00FF_00FF;
    let ag = (((c >> 8) & 0x00FF_00FF) * k) & 0xFF00_FF00;
    rb | ag
}

impl Screen {
    /// Draws an image filling the plan box x, y, w, h (a negative `w`
    /// mirrors it left to right, still filling x..x+|w|), at `alpha`.
    /// Its edges land on whole pixels, so images laid side by side meet
    /// with no seam.
    pub fn image(&mut self, img: &Image, x: f32, y: f32, w: f32, h: f32, alpha: f32) {
        let flip = w < 0.0;
        let w = w.abs();
        let (x0, y0) = ((self.sx(x) + 0.5).floor() as i32, (self.sy(y) + 0.5).floor() as i32);
        let (x1, y1) = ((self.sx(x + w) + 0.5).floor() as i32, (self.sy(y + h) + 0.5).floor() as i32);
        let (dw, dh) = (x1 - x0, y1 - y0);
        if dw <= 0 || dh <= 0 || alpha <= 0.0 || dw > 8192 || dh > 8192 {
            return;
        }
        let (i0, i1) = (self.clip.x0.max(x0), self.clip.x1.min(x1));
        let (j0, j1) = (self.clip.y0.max(y0), self.clip.y1.min(y1));
        if i1 <= i0 || j1 <= j0 {
            return;
        }
        let k = (alpha.min(1.0) * 256.0) as u32;
        let (cur, w_buf) = (self.cur, self.w as usize);
        img.with_scaled(dw, dh, |px| {
            let buf = &mut self.layers[cur];
            for j in j0..j1 {
                let src = &px[((j - y0) * dw) as usize..][..dw as usize];
                let row = &mut buf[j as usize * w_buf..][..w_buf];
                for i in i0..i1 {
                    let u = if flip { x1 - 1 - i } else { i - x0 };
                    let c = src[u as usize];
                    over(&mut row[i as usize], if k >= 256 { c } else { fade(c, k) });
                }
            }
        });
    }

    /// Draws an image w x h (plan units) centred on cx, cy and turned by
    /// `angle` radians (clockwise on screen), at `alpha`. A negative `w`
    /// mirrors it before turning.
    pub fn image_turned(&mut self, img: &Image, cx: f32, cy: f32, w: f32, h: f32, angle: f32, alpha: f32) {
        if angle == 0.0 {
            self.image(img, cx - w.abs() / 2.0, cy - h / 2.0, w, h, alpha);
            return;
        }
        let flip = w < 0.0;
        let (dw, dh) = ((w.abs() * self.k).round() as i32, (h * self.k).round() as i32);
        if dw <= 0 || dh <= 0 || alpha <= 0.0 || dw > 8192 || dh > 8192 {
            return;
        }
        let (pcx, pcy) = (self.sx(cx), self.sy(cy));
        let (c, s) = (angle.cos(), angle.sin());
        // the turned box's reach from its centre
        let (hw, hh) = (dw as f32 / 2.0, dh as f32 / 2.0);
        let rx = hw * c.abs() + hh * s.abs();
        let ry = hw * s.abs() + hh * c.abs();
        let (i0, j0, i1, j1) = self.clipped(pcx - rx - 1.0, pcy - ry - 1.0, pcx + rx + 1.0, pcy + ry + 1.0);
        if i1 <= i0 || j1 <= j0 {
            return;
        }
        let k = (alpha.min(1.0) * 256.0) as u32;
        let (cur, w_buf) = (self.cur, self.w as usize);
        img.with_scaled(dw, dh, |px| {
            let buf = &mut self.layers[cur];
            // the image pixel at (u, v), clear outside it
            let at = |u: i32, v: i32| -> u32 {
                if u < 0 || v < 0 || u >= dw || v >= dh {
                    0
                } else {
                    px[(v * dw + if flip { dw - 1 - u } else { u }) as usize]
                }
            };
            for j in j0..j1 {
                let row = &mut buf[j as usize * w_buf..][..w_buf];
                let py = j as f32 + 0.5 - pcy;
                for i in i0..i1 {
                    let pxx = i as f32 + 0.5 - pcx;
                    // back into the image, then between its four nearest pixels
                    let u = pxx * c + py * s + hw - 0.5;
                    let v = -pxx * s + py * c + hh - 0.5;
                    if u < -1.0 || v < -1.0 || u > dw as f32 || v > dh as f32 {
                        continue;
                    }
                    let (u0, v0) = (u.floor() as i32, v.floor() as i32);
                    let (fu, fv) = (((u - u0 as f32) * 256.0) as u32, ((v - v0 as f32) * 256.0) as u32);
                    let top = lerp(at(u0, v0), at(u0 + 1, v0), fu);
                    let bottom = lerp(at(u0, v0 + 1), at(u0 + 1, v0 + 1), fu);
                    let c = lerp(top, bottom, fv);
                    over(&mut row[i as usize], if k >= 256 { c } else { fade(c, k) });
                }
            }
        });
    }
}

/// Between two premultiplied colours, t of the way (0..256).
#[inline(always)]
fn lerp(a: u32, b: u32, t: u32) -> u32 {
    if a == b {
        return a;
    }
    let (ka, kb) = (256 - t, t);
    let rb = ((((a & 0x00FF_00FF) * ka) + ((b & 0x00FF_00FF) * kb)) >> 8) & 0x00FF_00FF;
    let ag = ((((a >> 8) & 0x00FF_00FF) * ka) + (((b >> 8) & 0x00FF_00FF) * kb)) & 0xFF00_FF00;
    rb | ag
}

mod png {
    //! Just enough PNG for game art: 8 bits a channel (or a packed palette or
    //! grey), not interlaced.

    fn be(b: &[u8]) -> u32 {
        u32::from_be_bytes([b[0], b[1], b[2], b[3]])
    }

    /// Width, height and premultiplied 0xAABBGGRR pixels.
    pub fn decode(bytes: &[u8]) -> Result<(i32, i32, Vec<u32>), &'static str> {
        if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
            return Err("not a PNG");
        }
        let (mut w, mut h, mut kind, mut bits) = (0usize, 0usize, 0u8, 8usize);
        let mut idat = Vec::new();
        let mut palette: Vec<[u8; 4]> = Vec::new();
        let mut at = 8;
        while at + 8 <= bytes.len() {
            let len = be(&bytes[at..]) as usize;
            let name = &bytes[at + 4..at + 8];
            let body = bytes.get(at + 8..at + 8 + len).ok_or("cut short")?;
            match name {
                b"IHDR" => {
                    w = be(body) as usize;
                    h = be(&body[4..]) as usize;
                    kind = body[9];
                    bits = body[8] as usize;
                    if body[12] != 0 {
                        return Err("interlaced");
                    }
                }
                b"PLTE" => palette = body.chunks_exact(3).map(|c| [c[0], c[1], c[2], 255]).collect(),
                b"tRNS" if kind == 3 => {
                    for (p, &a) in palette.iter_mut().zip(body) {
                        p[3] = a;
                    }
                }
                b"IDAT" => idat.extend_from_slice(body),
                b"IEND" => break,
                _ => {}
            }
            at += 12 + len;
        }
        let chans = match kind {
            0 | 3 => 1,
            4 => 2,
            2 => 3,
            6 => 4,
            _ => return Err("colour type"),
        };
        // grey and palette may pack 1, 2 or 4 bits a pixel
        let packed = matches!(kind, 0 | 3) && matches!(bits, 1 | 2 | 4);
        if bits != 8 && !packed {
            return Err("only 8 bits per channel");
        }
        if w == 0 || h == 0 || w > 8192 || h > 8192 {
            return Err("size");
        }
        let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat).map_err(|_| "bad data")?;
        let stride = (w * chans * bits).div_ceil(8);
        if raw.len() < (stride + 1) * h {
            return Err("cut short");
        }
        let mut cur = vec![0u8; stride];
        let mut prev = vec![0u8; stride];
        let mut out = Vec::with_capacity(w * h);
        for y in 0..h {
            let line = &raw[y * (stride + 1)..][..stride + 1];
            unfilter(line[0], &line[1..], &prev, &mut cur, chans)?;
            for x in 0..w {
                let unpacked;
                let p = if packed {
                    let v = cur[x * bits / 8] >> (8 - bits - x * bits % 8) & ((1 << bits) - 1) as u8;
                    // grey stretched to 0..255
                    unpacked = [if kind == 0 { v * (255 / ((1 << bits) - 1)) as u8 } else { v }];
                    &unpacked[..]
                } else {
                    &cur[x * chans..][..chans]
                };
                let [r, g, b, a] = match kind {
                    0 => [p[0], p[0], p[0], 255],
                    4 => [p[0], p[0], p[0], p[1]],
                    2 => [p[0], p[1], p[2], 255],
                    6 => [p[0], p[1], p[2], p[3]],
                    _ => *palette.get(p[0] as usize).ok_or("palette")?,
                };
                let pre = |c: u8| (c as u32 * a as u32 + 127) / 255;
                out.push(pre(r) | pre(g) << 8 | pre(b) << 16 | (a as u32) << 24);
            }
            core::mem::swap(&mut cur, &mut prev);
        }
        Ok((w as i32, h as i32, out))
    }

    fn unfilter(f: u8, line: &[u8], prev: &[u8], out: &mut [u8], bpp: usize) -> Result<(), &'static str> {
        for i in 0..line.len() {
            let a = if i >= bpp { out[i - bpp] as i16 } else { 0 };
            let b = prev[i] as i16;
            let c = if i >= bpp { prev[i - bpp] as i16 } else { 0 };
            let add = match f {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                _ => return Err("filter"),
            };
            out[i] = line[i].wrapping_add(add as u8);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba(x: u32, y: u32) -> (u32, u32, u32, u32) {
        (x * 40, y * 60, 200, if x < 3 { 255 } else { 128 })
    }

    #[test]
    fn decodes_every_kind() {
        let img = Image::png(include_bytes!("../../tests/data/rgba.png"));
        assert_eq!(img.size(), (6, 4));
        for y in 0..4 {
            for x in 0..6 {
                let (r, g, b, a) = rgba(x, y);
                let pre = |c: u32| (c * a + 127) / 255;
                assert_eq!(img.px[(y * 6 + x) as usize], pre(r) | pre(g) << 8 | pre(b) << 16 | a << 24);
            }
        }
        let rgb = Image::png(include_bytes!("../../tests/data/rgb.png"));
        assert_eq!(rgb.px[7], crate::rgb(40, 60, 200));
        let part = rgb.crop(1, 1, 2, 3);
        assert_eq!((part.size(), part.px[0], part.px[5]), ((2, 3), crate::rgb(40, 60, 200), crate::rgb(80, 180, 200)));
        let pal = Image::png(include_bytes!("../../tests/data/palette.png"));
        assert_eq!(pal.size(), (6, 4));
        // index 0 is see-through
        assert!(pal.px.iter().any(|&p| p == 0));
    }

    #[test]
    fn draws_at_size_and_scaled() {
        let img = Image::png(include_bytes!("../../tests/data/rgb.png"));
        let mut s = Screen::new(64, 64);
        s.fit(64.0, 64.0);
        s.clear(0xFF00_0000);
        s.image(&img, 2.0, 3.0, 6.0, 4.0, 1.0);
        assert_eq!(s.layers[0][(3 + 1) * 64 + 2 + 1], crate::rgb(40, 60, 200));
        // mirrored, the first column lands last
        s.image(&img, 10.0, 3.0, -6.0, 4.0, 1.0);
        assert_eq!(s.layers[0][3 * 64 + 15], crate::rgb(0, 0, 200));
        // half size averages pairs
        s.image(&img, 20.0, 20.0, 3.0, 2.0, 1.0);
        assert_eq!(s.layers[0][20 * 64 + 20], crate::rgb(20, 30, 200));
        // turned all the way round, it lands where it was
        let mut t = Screen::new(64, 64);
        t.fit(64.0, 64.0);
        t.clear(0xFF00_0000);
        t.image_turned(&img, 5.0, 5.0, 6.0, 4.0, core::f32::consts::TAU, 1.0);
        let p = t.layers[0][(3 + 1) * 64 + 2 + 1];
        assert!((p & 0xFF).abs_diff(40) <= 2 && ((p >> 8) & 0xFF).abs_diff(60) <= 2, "{p:x}");
    }
}
