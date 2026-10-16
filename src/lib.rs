//! The wasmga.me native game ABI (games-src/wg.h) for games written in Rust,
//! with smooth drawing at the screen's own size (wgfb.h and wgview.h, in
//! [`Screen`]) and the same Inter font as the C games (sdf_font.h).
//!
//! A game implements [`Game`] and calls [`game!`] once; that exports every
//! wg_* function the page (src/scripts/engines/native.ts) looks for. The
//! page draws the setup, pause and game over screens from the catalog's
//! `setup` and `stats`, so a game has no menus of its own: it starts a run
//! in [`Game::start`] with the options picked, says when the run is over,
//! and lists its stats.
//!
//! What gets saved (autosave, Resume, the Saves menu, cloud sync) is the
//! [`Game::Save`] struct, copied byte for byte. Keep everything that defines
//! the run in it, options included, and everything else (animations, hover,
//! sprites) in the game struct around it. See README.md next to this file.

mod game;
mod input;
mod rng;
mod runtime;
mod screen;

pub use game::{bytes_of, hash, Game, Plain};
pub use input::{Input, KeyRepeat, Pointer, A, B, DIRS, DOWN, LEFT, M_LEFT, M_MIDDLE, M_RIGHT, RIGHT, UP};
pub use rng::Rng;
#[doc(hidden)]
pub use runtime::{Runtime, Slot};
pub use screen::{blend, hex, mix, rgb, Screen, Sprite};

/// Events a frame returns; the page plays a sound for each.
pub const EV_BLIP: u32 = 1;
pub const EV_HIT: u32 = 2;
pub const EV_LOSE: u32 = 4;
/// The run ended: the page shows the game over screen and posts the score.
pub const EV_GAMEOVER: u32 = 8;
pub const EV_LEVEL: u32 = 16;
pub const EV_WIN: u32 = 32;

/// Milliseconds as "1:05".
pub fn clock(ms: i32) -> String {
    let s = ms.max(0) / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Smooth 0..1 easing for animations.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t) * (1.0 - t)
}
