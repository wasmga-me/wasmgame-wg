//! The accessible board (the wg_a11y_* exports in wg.h): the page lays a
//! grid of focusable squares over the board a game draws, so screen
//! readers, keyboards and switches play the same game a click does. A board
//! game implements [`Board`] and calls [`board!`](crate::board) next to
//! [`game!`](crate::game). Write the words with [`tr`](crate::tr), so they
//! come in the page's language.

use core::cell::UnsafeCell;

use crate::Game;

/// What a WG_A11Y_* text kind asks for (wg.h).
#[doc(hidden)]
pub const CELL: i32 = 0;
#[doc(hidden)]
pub const STATUS: i32 = 1;
#[doc(hidden)]
pub const SAID: i32 = 2;
#[doc(hidden)]
pub const KEYS: i32 = 3;

/// Where [`board!`](crate::board) keeps the text it hands the page, and what
/// the last move said until the page reads it.
#[doc(hidden)]
pub struct Text(UnsafeCell<(String, String)>);
// SAFETY: wasm32-unknown-unknown runs one thread
unsafe impl Sync for Text {}

impl Text {
    pub const fn new() -> Self {
        Text(UnsafeCell::new((String::new(), String::new())))
    }
    #[allow(clippy::mut_from_ref)]
    pub fn out(&'static self) -> &'static mut String {
        // SAFETY: one thread, and the page calls one export at a time
        unsafe { &mut (*self.0.get()).0 }
    }
    #[allow(clippy::mut_from_ref)]
    pub fn said(&'static self) -> &'static mut String {
        // SAFETY: as above; never borrowed together with `out` across a call
        unsafe { &mut (*self.0.get()).1 }
    }
}

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
    /// The run so far (mines left, time), read out on I.
    fn status(&self, out: &mut String);
    /// The keys, as the page reads them out: arrows move, Space the main
    /// move, X the second, I the status.
    fn keys(&self) -> &'static str;
    /// The main move (a 0, what Space or a click does) or the second (a 1:
    /// flag, note) on square x, y. Write what it did into `said`, to be read
    /// out; returns `EV_*` bits, sent with the next frame.
    fn act(&mut self, x: i32, y: i32, a: i32, said: &mut String) -> u32;
    /// Changes whenever any square's text or the status does; the page
    /// reads them only then. By default the picture's hash.
    fn rev(&mut self) -> u32 {
        self.picture()
    }
}

/// Exports the wg_a11y_* functions for a [`Board`]: `wg::board!(MyGame);`
/// once, after `wg::game!(MyGame);`.
#[macro_export]
macro_rules! board {
    ($t:ty) => {
        static WG_A11Y: $crate::Text = $crate::Text::new();

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
            let s = &rt.screen;
            match k {
                0 => (s.ox + x * s.k) as i32,
                1 => (s.oy + y * s.k) as i32,
                _ => (size * s.k * 100.0) as i32,
            }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_rev() -> u32 {
            <$t as $crate::Board>::rev(&mut WG_GAME.get().game)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_buf() -> *const u8 {
            WG_A11Y.out().as_ptr()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_text(kind: i32, x: i32, y: i32) -> i32 {
            let g = &WG_GAME.get().game;
            let out = WG_A11Y.out();
            out.clear();
            match kind {
                $crate::board::CELL => <$t as $crate::Board>::cell(g, x, y, out),
                $crate::board::STATUS => <$t as $crate::Board>::status(g, out),
                $crate::board::SAID => {
                    ::core::mem::swap(out, WG_A11Y.said());
                    WG_A11Y.said().clear();
                }
                $crate::board::KEYS => out.push_str(<$t as $crate::Board>::keys(g)),
                _ => {}
            }
            out.len() as i32
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_a11y_act(x: i32, y: i32, a: i32) {
            let said = WG_A11Y.said();
            said.clear();
            WG_GAME.get().board_act(x, y, a, said)
        }
    };
}
