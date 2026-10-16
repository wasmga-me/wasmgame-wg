//! Sprites: a box drawn once into its own buffer, then copied.

use super::Screen;
use crate::hash;

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

impl Screen {
    /// Draws the plan box x, y, w, h into `sp` with `draw(screen)` (in the
    /// same plan units) when `key` or the scale changed; its pixels must end
    /// up opaque. Then put it with [`Screen::sprite`].
    pub fn sprite_make(
        &mut self,
        sp: &mut Sprite,
        key: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        draw: impl FnMut(&mut Screen),
    ) {
        self.sprite_make_(sp, key, x, y, w, h, false, draw)
    }

    /// A sprite with soft or see-through edges (a gem, a coin): drawn twice
    /// (over black and white) to find its alpha, then blended when put. `draw`
    /// must draw the same thing both times.
    pub fn sprite_make_alpha(
        &mut self,
        sp: &mut Sprite,
        key: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        mut draw: impl FnMut(&mut Screen),
    ) {
        self.sprite_make_(sp, key, x, y, w, h, true, |s| draw(s))
    }

    fn sprite_make_<F: FnMut(&mut Screen)>(
        &mut self,
        sp: &mut Sprite,
        key: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        alpha: bool,
        mut draw: F,
    ) {
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

    /// Copies a sprite with its top left at plan x, y, to the nearest pixel,
    /// inside the clip rectangle.
    pub fn sprite(&mut self, sp: &Sprite, x: f32, y: f32) {
        let dx = (self.sx(x) + 0.5).floor() as i32;
        let dy = (self.sy(y) + 0.5).floor() as i32;
        let (i0, i1) = ((self.cx0 - dx).max(0), sp.w.min(self.cx1 - dx));
        if i1 <= i0 {
            return;
        }
        let sw = self.w as usize;
        for j in 0..sp.h {
            let yy = dy + j;
            if yy < self.cy0 || yy >= self.cy1 {
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
