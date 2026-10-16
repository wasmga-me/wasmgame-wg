//! The trait a game implements, and what its save may hold.

use crate::{Input, KeyRepeat, Pointer, Rng, Screen};

/// A type that is safe to fill from any bytes a save holds: integers,
/// floats, arrays of them and `#[repr(C)]` structs of those. Never bool,
/// char, enums, references or Vec (a save could hold a bad value).
///
/// # Safety
/// Every bit pattern must be a valid value, and the layout must be fixed
/// (`#[repr(C)]` for structs) so a save from an older build still reads.
pub unsafe trait Plain: Copy + 'static {}
macro_rules! plain {
    ($($t:ty),*) => { $(unsafe impl Plain for $t {})* };
}
plain!(u8, i8, u16, i16, u32, i32, u64, i64, f32, f64, Rng, KeyRepeat);
unsafe impl<T: Plain, const N: usize> Plain for [T; N] {}

/// A game. `new` makes it with its default options and no run; the page then
/// calls `start` (after setting options when the catalog has a `setup`).
pub trait Game: Sized + 'static {
    /// Everything a save keeps. Put a version number in it and check it in
    /// [`Game::loaded`], so saves from an older layout are dropped.
    type Save: Plain;
    /// The plan: the units the game lays itself out in, fitted to the screen.
    const PLAN: (f32, f32) = (400.0, 300.0);
    /// How many choices each option has, in the catalog `setup` order.
    const CHOICES: &'static [i32] = &[];

    fn new() -> Self;
    fn save(&mut self) -> &mut Self::Save;
    /// The picked options (inside the save), one per entry of `CHOICES`.
    fn options(&mut self) -> &mut [i32] {
        &mut []
    }
    /// Starts a new run with the options picked; `options()` is kept as is.
    fn start(&mut self, seed: u32);
    /// Moves the game on; returns `EV_*` bits.
    fn update(&mut self, input: &Input) -> u32;
    /// Pointer moves and button changes; returns `EV_*` bits (sent with the
    /// next frame).
    fn pointer(&mut self, _p: &Pointer) -> u32 {
        0
    }
    /// A key the pad doesn't cover (wg_key, from the page): the character's
    /// code point for single characters ('1' is 49), 8 Backspace, 127
    /// Delete. Digits come by physical key, so '0'..'9' on any layout and
    /// the number pad; with Shift they add 0x100 (Shift+1 is 0x131). Return
    /// true when the game used it (its key repeats are then swallowed).
    fn key(&mut self, _code: i32) -> bool {
        false
    }
    /// Draws the whole picture. Only called when [`Game::picture`] changed,
    /// [`Game::moving`] is true, or the screen was resized or reloaded.
    fn draw(&mut self, s: &mut Screen);
    /// Something on screen is animating: draw every frame.
    fn moving(&self) -> bool {
        false
    }
    /// A hash of everything the picture depends on. By default the save's
    /// bytes; add anything else `draw` reads (a hover, say) by overriding.
    fn picture(&mut self) -> u32 {
        hash(0, bytes_of(self.save()))
    }
    /// The plan for a screen of w x h pixels, to lay out differently on a
    /// tall phone and a wide desktop. Read `Screen::pw`/`ph` when drawing.
    fn plan(&self, _w: i32, _h: i32) -> (f32, f32) {
        Self::PLAN
    }
    fn score(&self) -> u32;
    /// True from the moment the run is lost or won (no autosave from then).
    fn over(&self) -> bool {
        false
    }
    /// For a level picker (a catalog `setup` option with `levels`, whose
    /// choice is the level to start at, 0 for the first): the level the run
    /// is on now. The page remembers the furthest per device and offers
    /// only levels up to it. -1 when the game has no levels.
    fn reached(&self) -> i32 {
        -1
    }
    /// For a level select: the level this run cleared last (0 for the
    /// first) and the score made on that level alone, or None. Return
    /// `EV_LEVEL` from `update` on the frame a level is cleared (the last
    /// one too, with `EV_WIN | EV_GAMEOVER`): the page then reads this, keeps
    /// the player's best for that level and posts it to the level's board.
    fn cleared(&self) -> Option<(i32, u32)> {
        None
    }
    /// Stat i for the game over screen, in the catalog `stats` order; -1
    /// hides it.
    fn stat(&self, _i: usize) -> i32 {
        -1
    }
    /// A save was just copied into `save()`: check every field and work out
    /// again whatever follows from it. Return false to drop it.
    fn loaded(&mut self) -> bool {
        true
    }
}

/// The save as bytes.
pub fn bytes_of<T: Plain>(t: &T) -> &[u8] {
    // SAFETY: Plain types have no invalid bit patterns; reading padding is
    // fine for a hash and a byte copy
    unsafe { core::slice::from_raw_parts(t as *const T as *const u8, core::mem::size_of::<T>()) }
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
