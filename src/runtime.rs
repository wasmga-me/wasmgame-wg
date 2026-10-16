//! The runtime behind the exports: one game and its screen, driven by the
//! page through the wg_* functions that [`game!`](crate::game) makes.

use core::cell::UnsafeCell;

use crate::{Game, Input, Pointer, Screen, A, B, DIRS};

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
            if i < 0 {
                -1
            } else {
                <$t as $crate::Game>::stat(&WG_GAME.get().game, i as usize)
            }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_reached() -> i32 {
            <$t as $crate::Game>::reached(&WG_GAME.get().game)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_cleared() -> i32 {
            <$t as $crate::Game>::cleared(&WG_GAME.get().game).map_or(-1, |c| c.0)
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_level_score() -> u32 {
            <$t as $crate::Game>::cleared(&WG_GAME.get().game).map_or(0, |c| c.1)
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
