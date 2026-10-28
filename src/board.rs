//! The accessible board (the wg_a11y_* exports in wg.h): the page lays a
//! grid of focusable squares over the board a game draws, so screen
//! readers, keyboards and switches play the same game a click does. A board
//! game implements [`Board`] and calls [`board!`](crate::board) next to
//! [`game!`](crate::game). Write the words with [`tr`](crate::tr), so they
//! come in the page's language.

use crate::{bytes_of, hash, Game};

/// What a WG_A11Y_* text kind asks for (wg.h).
#[doc(hidden)]
pub const CELL: i32 = 0;
#[doc(hidden)]
pub const STATUS: i32 = 1;
#[doc(hidden)]
pub const SAID: i32 = 2;
#[doc(hidden)]
pub const KEYS: i32 = 3;

/// A board game, played on the page's accessible grid too.
pub trait Board: Game {
    /// The board's size in squares, (cols, rows); `None` while none shows.
    fn grid(&self) -> Option<(i32, i32)>;
    /// The square the game's cursor is on.
    fn cursor(&self) -> (i32, i32);
    /// The page's focus moved to square x, y: the cursor follows it.
    fn focus(&mut self, x: i32, y: i32);
    /// Where the board is drawn, in plan units: left, top, a square's size.
    fn rect(&self) -> (f32, f32, f32);
    /// Square x, y in words ("3 mines nearby"); the page adds its name (C4).
    fn cell(&self, x: i32, y: i32, out: &mut String);
    /// For a board that isn't a grid of squares (tubes in a rack, hexes,
    /// screws on plates): square x, y's own box in plan units, left, top,
    /// width, height. The page lays each square there and leaves its name
    /// to `cell`, which then says it ("tube 3", "red screw").
    fn cell_box(&self, _x: i32, _y: i32) -> Option<(f32, f32, f32, f32)> {
        None
    }
    /// The run so far (mines left, time), read out on I.
    fn status(&self, out: &mut String);
    /// The keys, as the page reads them out: arrows move, Space the main
    /// move, X the second, I the status.
    fn keys(&self) -> &'static str;
    /// The main move (a 0, what Space or a click does) or the second (a 1:
    /// flag, note) on square x, y. Write what it did into `said`, to be read
    /// out; returns `EV_*` bits, sent with the next frame.
    fn act(&mut self, x: i32, y: i32, a: i32, said: &mut String) -> u32;
    /// Changes whenever any square's text does; the page reads them only
    /// then. By default a hash of the save.
    fn rev(&mut self) -> u32 {
        hash(0, bytes_of(self.save()))
    }
}

/// Exports the wg_a11y_* functions for a [`Board`]: `wg::board!(MyGame);`
/// once, after `wg::game!(MyGame);`.
#[macro_export]
macro_rules! board {
    ($t:ty) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_grid() -> i32 {
            <$t as $crate::Board>::grid(&WG_GAME.get().game).map_or(0, |(c, r)| c << 16 | r)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_cursor() -> i32 {
            let (x, y) = <$t as $crate::Board>::cursor(&WG_GAME.get().game);
            y << 16 | x
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_focus(x: i32, y: i32) {
            <$t as $crate::Board>::focus(&mut WG_GAME.get().game, x, y)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_rect(k: i32) -> i32 {
            let rt = WG_GAME.get();
            let (x, y, size) = <$t as $crate::Board>::rect(&rt.game);
            let (px, py) = rt.screen.px(x, y);
            match k {
                0 => px as i32,
                1 => py as i32,
                _ => (size * rt.screen.k * 100.0) as i32,
            }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_box(x: i32, y: i32, k: i32) -> i32 {
            let rt = WG_GAME.get();
            let Some((l, t, w, h)) = <$t as $crate::Board>::cell_box(&rt.game, x, y) else { return -1 };
            let (px, py) = rt.screen.px(l, t);
            match k {
                0 => px as i32,
                1 => py as i32,
                2 => (w * rt.screen.k) as i32,
                _ => (h * rt.screen.k) as i32,
            }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_rev() -> u32 {
            <$t as $crate::Board>::rev(&mut WG_GAME.get().game)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_buf() -> *const u8 {
            WG_GAME.get().a11y.0.as_ptr()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_text(kind: i32, x: i32, y: i32) -> i32 {
            let rt = WG_GAME.get();
            let (out, said) = &mut rt.a11y;
            out.clear();
            match kind {
                $crate::board::CELL => <$t as $crate::Board>::cell(&rt.game, x, y, out),
                $crate::board::STATUS => <$t as $crate::Board>::status(&rt.game, out),
                // read once: `said` is left empty
                $crate::board::SAID => ::core::mem::swap(out, said),
                $crate::board::KEYS => out.push_str(<$t as $crate::Board>::keys(&rt.game)),
                _ => {}
            }
            out.len() as i32
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_act(x: i32, y: i32, a: i32) {
            WG_GAME.get().board_act(x, y, a)
        }
    };
}
