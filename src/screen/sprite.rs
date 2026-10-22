//! Sprites: a box drawn once into its own buffer, then copied.

use super::{colour, Screen};

/// Opaque black and white, the two grounds an alpha sprite is drawn on.
const BLACK: u32 = 0xFF00_0000;
const WHITE: u32 = 0xFFFF_FFFF;
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
        self.make_sprite(sp, key, x, y, w, h, false, draw)
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
        draw: impl FnMut(&mut Screen),
    ) {
        self.make_sprite(sp, key, x, y, w, h, true, draw)
    }

    fn make_sprite(
        &mut self,
        sp: &mut Sprite,
        key: u32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        alpha: bool,
        mut draw: impl FnMut(&mut Screen),
    ) {
        // redrawn when the key, the kind, the scale or the box's position
        // within a pixel changes; never 0, which means nothing drawn yet
        let key = hash(key, &[alpha as u8]);
        let key = hash(hash(key, &self.k.to_le_bytes()), &[x.to_bits() as u8, y.to_bits() as u8]) | 1;
        let (pw, ph) = ((w * self.k).ceil() as i32, (h * self.k).ceil() as i32);
        if sp.key == key && sp.w == pw && sp.h == ph {
            return;
        }
        let n = (pw.max(0) * ph.max(0)) as usize;
        let mut px = core::mem::take(&mut sp.px);
        px.clear();
        if alpha {
            // over black a pixel is a*c, over white a*c + (1-a)*255, so the
            // difference gives the alpha; black keeps the colour
            // premultiplied
            let white = self.draw_into(vec![WHITE; n], pw, ph, x, y, &mut draw);
            px.resize(n, BLACK);
            px = self.draw_into(px, pw, ph, x, y, &mut draw);
            for (b, w) in px.iter_mut().zip(white) {
                let diff = |shift: u32| ((w >> shift) & 0xFF).saturating_sub((*b >> shift) & 0xFF);
                let a = 255 - ((diff(0) + diff(8) + diff(16)) / 3).min(255);
                *b = (*b & 0x00FF_FFFF) | a << 24;
            }
        } else {
            px.resize(n, 0);
            px = self.draw_into(px, pw, ph, x, y, &mut draw);
        }
        *sp = Sprite { px, w: pw, h: ph, key, alpha };
    }

    /// Runs `draw` into `buf`, a w x h pixel buffer standing for the plan
    /// box whose top left is x, y, as if it were the screen (same scale, no
    /// clip), and gives the buffer back.
    fn draw_into(
        &mut self,
        buf: Vec<u32>,
        w: i32,
        h: i32,
        x: f32,
        y: f32,
        draw: &mut impl FnMut(&mut Screen),
    ) -> Vec<u32> {
        let keep = (self.cur, self.w, self.h, self.ox, self.oy, self.clip);
        self.layers.push(buf);
        self.cur = self.layers.len() - 1;
        (self.w, self.h) = (w, h);
        (self.ox, self.oy) = (-x * self.k, -y * self.k);
        self.no_clip();
        draw(self);
        (self.cur, self.w, self.h, self.ox, self.oy, self.clip) = keep;
        self.layers.pop().unwrap()
    }

    /// Copies a sprite with its top left at plan x, y, to the nearest pixel,
    /// inside the clip rectangle.
    pub fn sprite(&mut self, sp: &Sprite, x: f32, y: f32) {
        let dx = (self.sx(x) + 0.5).floor() as i32;
        let dy = (self.sy(y) + 0.5).floor() as i32;
        // the sprite's columns i0..i1 and rows j0..j1 fall inside the clip
        let (i0, i1) = ((self.clip.x0 - dx).max(0), sp.w.min(self.clip.x1 - dx));
        let (j0, j1) = ((self.clip.y0 - dy).max(0), sp.h.min(self.clip.y1 - dy));
        if i1 <= i0 {
            return;
        }
        let n = (i1 - i0) as usize;
        for (y, row) in self.rows(dy + j0, dy + j1) {
            let s = ((y - dy) * sp.w + i0) as usize;
            let dst = &mut row[(dx + i0) as usize..][..n];
            let src = &sp.px[s..s + n];
            if !sp.alpha {
                dst.copy_from_slice(src);
                continue;
            }
            for (p, &c) in dst.iter_mut().zip(src) {
                let a = c >> 24;
                if a == 0 {
                    continue;
                }
                if a == 255 {
                    *p = c;
                    continue;
                }
                // premultiplied: out = src + dst * (1 - a)
                let ia = 256 - (a + (a >> 7));
                let d = *p;
                let rb = ((c & 0xFF00FF) + (((d & 0xFF00FF) * ia) >> 8)) & 0xFF00FF;
                let g = ((c & 0x00FF00) + (((d & 0x00FF00) * ia) >> 8)) & 0x00FF00;
                *p = colour::alpha_over(d, ia) | rb | g;
            }
        }
    }
}
