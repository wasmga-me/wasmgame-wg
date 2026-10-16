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

mod font {
    pub struct Glyph {
        pub off: u32,
        pub w: u8,
        pub h: u8,
        pub left: f32,
        pub adv: f32,
        pub top: f32,
    }
    include!(concat!(env!("OUT_DIR"), "/sdf_font.rs"));
}
mod screen;

pub use screen::{blend, hash, hex, mix, rgb, Screen, Sprite};

use core::cell::UnsafeCell;

/// Buttons held in [`Input::held`] / [`Input::pressed`].
pub const UP: u32 = 1;
pub const DOWN: u32 = 2;
pub const LEFT: u32 = 4;
pub const RIGHT: u32 = 8;
pub const A: u32 = 16;
pub const B: u32 = 32;
pub const DIRS: u32 = UP | DOWN | LEFT | RIGHT;

/// Events a frame returns; the page plays a sound for each.
pub const EV_BLIP: u32 = 1;
pub const EV_HIT: u32 = 2;
pub const EV_LOSE: u32 = 4;
/// The run ended: the page shows the game over screen and posts the score.
pub const EV_GAMEOVER: u32 = 8;
pub const EV_LEVEL: u32 = 16;
pub const EV_WIN: u32 = 32;

/// Pointer buttons.
pub const M_LEFT: u32 = 1;
pub const M_RIGHT: u32 = 2;
pub const M_MIDDLE: u32 = 4;

/// One frame's input. Start never arrives: the page keeps it for pause.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub held: u32,
    /// held now and not on the last frame
    pub pressed: u32,
    /// milliseconds since the last frame, 0..=100; 0 when the page only
    /// wants the picture (a resize, a pause screen going up)
    pub dt: i32,
}

/// The pointer, in plan units. Mouse buttons and touch both arrive here; a
/// touch holds [`M_LEFT`] with `touch` set.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pointer {
    pub x: f32,
    pub y: f32,
    /// false when it left the screen (x and y are then -1)
    pub inside: bool,
    pub held: u32,
    pub pressed: u32,
    pub released: u32,
    pub touch: bool,
}

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
    /// Delete. Return true when the game used it.
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

/// xorshift32, small enough to live in a save.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rng(pub u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        Rng(seed | 1)
    }
    pub fn next(&mut self) -> u32 {
        let mut x = if self.0 == 0 { 0x9E37_79B9 } else { self.0 };
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// 0..n (0 when n is 0)
    pub fn below(&mut self, n: u32) -> u32 {
        if n == 0 {
            0
        } else {
            ((self.next() as u64 * n as u64) >> 32) as u32
        }
    }
    /// lo..=hi
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1).max(0) as u32) as i32
    }
    /// 0.0..1.0
    pub fn float(&mut self) -> f32 {
        (self.next() >> 8) as f32 / (1u32 << 24) as f32
    }
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i as u32 + 1) as usize);
        }
    }
}

/// Held arrows repeat after a delay, like keys in a text box.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyRepeat(pub i32);

impl KeyRepeat {
    /// The arrow bits to act on this frame.
    pub fn dirs(&mut self, input: &Input) -> u32 {
        let dirs = input.held & DIRS;
        if input.pressed & dirs != 0 {
            self.0 = -230;
            return input.pressed & dirs;
        }
        if dirs == 0 {
            self.0 = 0;
            return 0;
        }
        self.0 += input.dt;
        if self.0 >= 70 {
            self.0 -= 70;
            return dirs;
        }
        0
    }
}

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

// ---------- the runtime behind the exports ----------

#[doc(hidden)]
pub struct Runtime<G: Game> {
    pub game: G,
    pub screen: Screen,
    prev: u32,
    mouse: u32,
    pending: u32,
    sig: u32,
    force: bool,
    drawn: bool,
}

impl<G: Game> Runtime<G> {
    fn new() -> Self {
        let game = G::new();
        let mut screen = Screen::new(512, 448);
        let (pw, ph) = game.plan(screen.sw, screen.sh);
        screen.fit(pw, ph);
        Runtime { game, screen, prev: 0, mouse: 0, pending: 0, sig: 0, force: true, drawn: false }
    }

    pub fn init(&mut self, seed: u32) {
        let keep: Vec<i32> = self.game.options().to_vec();
        self.game.start(seed);
        let opts = self.game.options();
        if opts.len() == keep.len() {
            opts.copy_from_slice(&keep);
        }
        self.prev = 0;
        self.pending = 0;
        self.force = true;
    }

    pub fn resize(&mut self, w: i32, h: i32) {
        self.screen.resize(w, h);
        self.force = true;
    }

    pub fn option(&mut self, i: i32, v: i32) -> i32 {
        let opts = self.game.options();
        let Some(slot) = usize::try_from(i).ok().filter(|&i| i < opts.len() && i < G::CHOICES.len()) else {
            return -1;
        };
        if v >= 0 && v < G::CHOICES[slot] {
            opts[slot] = v;
        }
        opts[slot]
    }

    pub fn frame(&mut self, buttons: i32, dt: i32) -> i32 {
        let held = buttons as u32 & (DIRS | A | B);
        let input = Input { held, pressed: held & !self.prev, dt: dt.clamp(0, 100) };
        self.prev = held;
        let ev = self.pending | self.game.update(&input);
        self.pending = 0;
        let (pw, ph) = self.game.plan(self.screen.sw, self.screen.sh);
        if pw != self.screen.pw || ph != self.screen.ph {
            self.screen.fit(pw, ph);
            self.screen.forget_background();
            self.force = true;
        }
        let sig = self.game.picture();
        self.drawn = self.force || self.game.moving() || sig != self.sig;
        if self.drawn {
            self.game.draw(&mut self.screen);
            self.sig = sig;
            self.force = false;
        }
        ev as i32
    }

    pub fn mouse(&mut self, x: i32, y: i32, b: i32) {
        let held = b as u32 & 7;
        let mut p = Pointer {
            x: -1.0,
            y: -1.0,
            inside: x >= 0 && y >= 0,
            held,
            pressed: held & !self.mouse,
            released: self.mouse & !held,
            touch: b & 8 != 0,
        };
        self.mouse = held;
        if p.inside {
            (p.x, p.y) = self.screen.to_plan(x, y);
        }
        self.pending |= self.game.pointer(&p);
    }

    pub fn drawn(&self) -> i32 {
        self.drawn as i32
    }

    pub fn state(&mut self) -> *mut u8 {
        self.game.save() as *mut G::Save as *mut u8
    }

    pub fn state_loaded(&mut self) -> i32 {
        let ok = {
            let opts = self.game.options();
            opts.len() == G::CHOICES.len() && opts.iter().zip(G::CHOICES).all(|(&o, &n)| o >= 0 && o < n)
        } && self.game.loaded();
        // buttons held as the save loads don't count as presses
        self.prev = !0;
        self.mouse = 0;
        self.pending = 0;
        self.force = true;
        self.screen.forget_background();
        if !ok {
            self.game = G::new();
            self.init(0x1234);
        }
        ok as i32
    }
}

/// Where [`game!`] keeps the one game; made on first use.
#[doc(hidden)]
pub struct Slot<G: Game>(UnsafeCell<Option<Runtime<G>>>);
// SAFETY: wasm32-unknown-unknown runs one thread
unsafe impl<G: Game> Sync for Slot<G> {}

impl<G: Game> Slot<G> {
    pub const fn new() -> Self {
        Slot(UnsafeCell::new(None))
    }
    #[allow(clippy::mut_from_ref)]
    pub fn get(&'static self) -> &'static mut Runtime<G> {
        // SAFETY: one thread, and the page calls one export at a time
        let slot = unsafe { &mut *self.0.get() };
        if slot.is_none() {
            panic_to_page();
            *slot = Some(Runtime::new());
        }
        slot.as_mut().unwrap()
    }
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    /// The page throws this UTF-8 message as the crash (error screen).
    fn wg_panic(msg: *const u8, len: usize);
}

/// Sends panics to the page's error screen, with the game's stack.
fn panic_to_page() {
    #[cfg(target_arch = "wasm32")]
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        unsafe { wg_panic(msg.as_ptr(), msg.len()) };
    }));
}

/// Exports the wg_* ABI for a [`Game`]: `wg::game!(MyGame);` once, at the
/// crate root.
#[macro_export]
macro_rules! game {
    ($t:ty) => {
        static WG_GAME: $crate::Slot<$t> = $crate::Slot::new();

        #[unsafe(no_mangle)]
        pub extern "C" fn wg_init(seed: u32) {
            WG_GAME.get().init(seed)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_frame(buttons: i32, dt: i32) -> i32 {
            WG_GAME.get().frame(buttons, dt)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_fb() -> *const u32 {
            WG_GAME.get().screen.pixels_ptr()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_w() -> i32 {
            WG_GAME.get().screen.sw
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_h() -> i32 {
            WG_GAME.get().screen.sh
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_resize(w: i32, h: i32) {
            WG_GAME.get().resize(w, h)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_mouse(x: i32, y: i32, b: i32) {
            WG_GAME.get().mouse(x, y, b)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_key(code: i32) -> i32 {
            <$t as $crate::Game>::key(&mut WG_GAME.get().game, code) as i32
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_drawn() -> i32 {
            WG_GAME.get().drawn()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_option(i: i32, v: i32) -> i32 {
            WG_GAME.get().option(i, v)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_over() -> i32 {
            <$t as $crate::Game>::over(&WG_GAME.get().game) as i32
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_stat(i: i32) -> i32 {
            if i < 0 { -1 } else { <$t as $crate::Game>::stat(&WG_GAME.get().game, i as usize) }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_reached() -> i32 {
            <$t as $crate::Game>::reached(&WG_GAME.get().game)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_score() -> u32 {
            <$t as $crate::Game>::score(&WG_GAME.get().game)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_state() -> *mut u8 {
            WG_GAME.get().state()
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_state_size() -> i32 {
            ::core::mem::size_of::<<$t as $crate::Game>::Save>() as i32
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_state_loaded() -> i32 {
            WG_GAME.get().state_loaded()
        }
    };
}
