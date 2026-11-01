//! Level controls drawn inside the game: round icon buttons (Restart, Undo,
//! Go) right-aligned in a game's top bar, the same size and look in
//! every game. A game lays out a [`Row`] with [`row`], draws it with
//! [`Row::draw`] and asks [`Row::hit`] where the pointer is.

use crate::{hex, mix, Screen};
use core::f32::consts::PI;

/// What a button shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Icon {
    /// start the level again: a circling arrow
    #[default]
    Restart,
    /// take back the last move: a hooked arrow pointing left
    Undo,
    /// start what was set up (Hive Guard's bees): a play triangle, gold and bigger
    Go,
}

/// A button's colours: the disc, the icon on it and the shadow under it.
#[derive(Clone, Copy)]
struct Style {
    fill: u32,
    icon: u32,
    shadow: u32,
}

/// white discs with dark icons, for light and busy backgrounds alike
const LIGHT: Style = Style { fill: 0xffffff, icon: 0x2b3a55, shadow: 0x1d2a44 };
/// gold with a dark brown icon, for [`Icon::Go`]
const GOLD: Style = Style { fill: 0xffc928, icon: 0x2a1810, shadow: 0x1d2a44 };

/// Radius of a button, in plan units; [`Icon::Go`] is bigger.
pub const R: f32 = 13.0;
pub const R_GO: f32 = 22.0;
/// Space between neighbouring buttons.
pub const GAP: f32 = 6.0;
/// The pointer counts as on a button this far outside it, for fingers.
const SLACK: f32 = 4.0;

/// One button, in plan units.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Button {
    pub icon: Icon,
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

/// Most buttons a row holds.
const MAX: usize = 4;

/// A row of buttons, listed left to right; empty by default. Derefs to a
/// slice of [`Button`]s, with no allocation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Row {
    buttons: [Button; MAX],
    n: usize,
    hidden: bool,
}

impl core::ops::Deref for Row {
    type Target = [Button];
    fn deref(&self) -> &[Button] {
        &self.buttons[..self.n]
    }
}

/// Buttons in a row, centred on height `cy`, the last one's right edge at
/// `right`; up to four.
pub fn row(icons: &[Icon], right: f32, cy: f32) -> Row {
    let mut out = Row::default();
    out.n = icons.len().min(MAX);
    let mut edge = right;
    for (i, &icon) in icons[..out.n].iter().enumerate().rev() {
        let r = if icon == Icon::Go { R_GO } else { R };
        out.buttons[i] = Button { icon, x: edge - r, y: cy, r };
        edge -= 2.0 * r + GAP;
    }
    out
}

impl Row {
    /// The row kept in place but not shown or pressed (between levels), so
    /// what stands beside it doesn't move when it goes.
    pub fn hidden(mut self) -> Row {
        self.hidden = true;
        self
    }

    /// The icon of the button under (x, y), if any.
    pub fn hit(&self, x: f32, y: f32) -> Option<Icon> {
        if self.hidden {
            return None;
        }
        self.iter().find(|b| b.hit(x, y)).map(|b| b.icon)
    }

    /// Draws the row, the `hover` one lightened and those `enabled` says
    /// can't be used now faded; returns the row's left edge (or `right`
    /// with no buttons), where text beside it has to stop.
    pub fn draw(&self, s: &mut Screen, hover: Option<Icon>, enabled: impl Fn(Icon) -> bool, right: f32) -> f32 {
        if !self.hidden {
            for b in self.iter() {
                b.draw(s, hover == Some(b.icon), enabled(b.icon));
            }
        }
        self.first().map_or(right, |b| b.x - b.r)
    }
}

impl Button {
    pub fn hit(&self, x: f32, y: f32) -> bool {
        (x - self.x).hypot(y - self.y) <= self.r + SLACK
    }

    /// Draws the button; `hover` lightens it, a button that can't be used
    /// now (`!enabled`) is drawn faded.
    pub fn draw(&self, s: &mut Screen, hover: bool, enabled: bool) {
        let style = if self.icon == Icon::Go { GOLD } else { LIGHT };
        // hover tints the disc with its icon's colour
        let fill = if hover && enabled { mix(hex(style.icon), hex(style.fill), 0.14) } else { hex(style.fill) };
        let (x, y, r) = (self.x, self.y, self.r);
        let a = if enabled { 1.0 } else { 0.45 };
        s.disc(x, y + 1.5, r, hex(style.shadow), 0.3 * a);
        s.disc(x, y, r, fill, 0.92 * a);
        // the icon stays opaque (its strokes overlap at the joins) and fades
        // into the disc instead
        let ink = if enabled { hex(style.icon) } else { mix(hex(style.icon), fill, 0.45) };
        let a = 1.0;
        // stroke half-width, and the icon's size, both with the button
        let w = 0.7 * r / R;
        let k = r / R;
        match self.icon {
            Icon::Restart => {
                // most of a circle, open at the top right, with the head at its end
                let rr = 6.0 * k;
                let (a0, a1) = (-0.35 * PI, 1.45 * PI);
                arc(s, x, y, rr, a0, a1, w, ink, a);
                let (ex, ey) = (x + rr * a1.cos(), y + rr * a1.sin());
                // heading along the circle at the end (clockwise on screen)
                let (tx, ty) = (-a1.sin(), a1.cos());
                head(s, ex, ey, tx, ty, 3.6 * k, w, ink, a);
            }
            Icon::Undo => {
                // a shaft left along the top, curling down round the right
                let rr = 4.0 * k;
                let (cx, cy) = (x + 1.5 * k, y + 1.0 * k);
                let top = cy - rr;
                s.line(x - 5.5 * k, top, cx, top, w, ink, a);
                arc(s, cx, cy, rr, -0.5 * PI, 0.5 * PI, w, ink, a);
                s.line(cx, cy + rr, x - 1.0 * k, cy + rr, w, ink, a);
                head(s, x - 5.5 * k, top, -1.0, 0.0, 3.6 * k, w, ink, a);
            }
            Icon::Go => {
                let h = 0.42 * r;
                s.tri((x - 0.6 * h, y - h), (x - 0.6 * h, y + h), (x + 1.05 * h, y), ink, a);
            }
        }
    }
}

/// An arc of a circle, a0 to a1 (radians, y down), as short strokes.
fn arc(s: &mut Screen, cx: f32, cy: f32, r: f32, a0: f32, a1: f32, w: f32, c: u32, alpha: f32) {
    let n = ((a1 - a0).abs() * r / 1.5).ceil().max(2.0) as i32;
    let at = |i: i32| {
        let t = a0 + (a1 - a0) * i as f32 / n as f32;
        (cx + r * t.cos(), cy + r * t.sin())
    };
    let mut p = at(0);
    for i in 1..=n {
        let q = at(i);
        s.line(p.0, p.1, q.0, q.1, w, c, alpha);
        p = q;
    }
}

/// An open arrowhead at (x, y) pointing along (dx, dy), a unit vector.
fn head(s: &mut Screen, x: f32, y: f32, dx: f32, dy: f32, len: f32, w: f32, c: u32, alpha: f32) {
    for side in [-1.0f32, 1.0] {
        // 40 degrees either side of straight back
        let (cs, sn) = (0.766f32, 0.643 * side);
        let (bx, by) = (-dx * cs - -dy * sn, -dx * sn + -dy * cs);
        s.line(x, y, x + bx * len, y + by * len, w, c, alpha);
    }
}

/// Whether a key [`Game::key`](crate::Game::key) got is the one that
/// presses Restart: R, which the page sends as the character it types, so
/// also what R types on Russian, Greek and Korean layouts. Undo goes on the
/// pad's B (X, Backspace), as in Decant.
pub fn is_restart_key(code: i32) -> bool {
    matches!(char::from_u32(code as u32), Some('r' | 'к' | 'ρ' | 'ㄱ'))
}
