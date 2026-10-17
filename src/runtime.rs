//! The runtime behind the exports: one game and its screen, driven by the
//! page through the wg_* functions that [`game!`](crate::game) makes.

use core::cell::UnsafeCell;

use crate::{Game, Input, Pointer, Screen, A, B, DIRS, M_LEFT, M_MIDDLE, M_RIGHT};

/// The canvas size until the page sends its own with wg_resize.
const FIRST_SIZE: (i32, i32) = (512, 448);
/// Longest frame step a game gets, in ms (after a stall, say).
const MAX_DT: i32 = 100;
/// The seed a run starts with when a bad save is dropped.
const FALLBACK_SEED: u32 = 0x1234;
/// wg_mouse's buttons: the three mouse buttons, and the touch bit.
const MOUSE_BUTTONS: u32 = M_LEFT | M_RIGHT | M_MIDDLE;
const TOUCH: i32 = 8;

/// The game and its screen, with what the exports remember between calls.
#[doc(hidden)]
pub struct Runtime<G: Game> {
    pub game: G,
    pub screen: Screen,
    /// buttons held on the last frame, to find the newly pressed ones
    buttons: u32,
    /// pointer buttons held at the last wg_mouse
    mouse_buttons: u32,
    /// events from `Game::pointer`, returned with the next frame
    pointer_events: u32,
    /// `Game::picture` when the screen was last drawn
    picture: u32,
    /// draw on the next frame whatever `picture` says (new run, resize, load)
    redraw: bool,
    /// the last frame drew (wg_drawn)
    drawn: bool,
}

impl<G: Game> Runtime<G> {
    fn new() -> Self {
        let game = G::new();
        let mut screen = Screen::new(FIRST_SIZE.0, FIRST_SIZE.1);
        let (pw, ph) = game.plan(screen.sw, screen.sh);
        screen.fit(pw, ph);
        Runtime {
            game,
            screen,
            buttons: 0,
            mouse_buttons: 0,
            pointer_events: 0,
            picture: 0,
            redraw: true,
            drawn: false,
        }
    }

    /// wg_init: a new run, keeping the options the page set.
    pub fn init(&mut self, seed: u32) {
        let keep: Vec<i32> = self.game.options().to_vec();
        self.game.start(seed);
        let opts = self.game.options();
        if opts.len() == keep.len() {
            opts.copy_from_slice(&keep);
        }
        self.buttons = 0;
        self.pointer_events = 0;
        self.redraw = true;
    }

    /// wg_resize: the canvas is now w x h pixels.
    pub fn resize(&mut self, w: i32, h: i32) {
        self.screen.resize(w, h);
        self.redraw = true;
    }

    /// wg_option: sets option i to v when v is one of its choices (a v out of
    /// range only reads it); returns the option's value, -1 for no option i.
    pub fn option(&mut self, i: i32, v: i32) -> i32 {
        let opts = self.game.options();
        let Some(slot) = usize::try_from(i).ok().filter(|&i| i < opts.len() && i < G::CHOICES.len()) else {
            return -1;
        };
        if (0..G::CHOICES[slot]).contains(&v) {
            opts[slot] = v;
        }
        opts[slot]
    }

    /// wg_frame: moves the game on by dt ms and draws it if the picture
    /// changed; returns the frame's events.
    pub fn frame(&mut self, buttons: i32, dt: i32) -> i32 {
        let held = buttons as u32 & (DIRS | A | B);
        let input = Input { held, pressed: held & !self.buttons, dt: dt.clamp(0, MAX_DT) };
        self.buttons = held;
        let events = self.pointer_events | self.game.update(&input);
        self.pointer_events = 0;
        // a game may lay out differently as the screen changes shape
        let (pw, ph) = self.game.plan(self.screen.sw, self.screen.sh);
        if pw != self.screen.pw || ph != self.screen.ph {
            self.screen.fit(pw, ph);
            self.screen.forget_background();
            self.redraw = true;
        }
        let picture = self.game.picture();
        self.drawn = self.redraw || self.game.moving() || picture != self.picture;
        if self.drawn {
            self.game.draw(&mut self.screen);
            self.picture = picture;
            self.redraw = false;
        }
        events as i32
    }

    /// wg_mouse: the pointer at screen pixel x, y (-1, -1 when it left).
    pub fn mouse(&mut self, x: i32, y: i32, buttons: i32) {
        let held = buttons as u32 & MOUSE_BUTTONS;
        let mut p = Pointer {
            x: -1.0,
            y: -1.0,
            inside: x >= 0 && y >= 0,
            held,
            pressed: held & !self.mouse_buttons,
            released: self.mouse_buttons & !held,
            touch: buttons & TOUCH != 0,
        };
        self.mouse_buttons = held;
        if p.inside {
            (p.x, p.y) = self.screen.to_plan(x, y);
        }
        self.pointer_events |= self.game.pointer(&p);
    }

    pub fn drawn(&self) -> i32 {
        self.drawn as i32
    }

    /// wg_state: where the page reads and writes the save.
    pub fn state(&mut self) -> *mut u8 {
        self.game.save() as *mut G::Save as *mut u8
    }

    /// wg_state_loaded: the page just wrote a save into `state()`. Keeps it
    /// when its options are in range and the game takes it; otherwise
    /// starts a new run. Returns 1 when kept.
    pub fn state_loaded(&mut self) -> i32 {
        let ok = self.options_valid() && self.game.loaded();
        // buttons held as the save loads don't count as presses
        self.buttons = !0;
        self.mouse_buttons = 0;
        self.pointer_events = 0;
        self.redraw = true;
        self.screen.forget_background();
        if !ok {
            self.game = G::new();
            self.init(FALLBACK_SEED);
        }
        ok as i32
    }

    /// Every option is one of its choices.
    fn options_valid(&mut self) -> bool {
        let opts = self.game.options();
        opts.len() == G::CHOICES.len() && opts.iter().zip(G::CHOICES).all(|(&o, &n)| (0..n).contains(&o))
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
        pub extern "C" fn wg_lang(code: i32) -> i32 {
            $crate::set_lang(code)
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
