//! What a frame gets from the page: buttons, the pointer, and key repeat
//! for held arrows.

/// Buttons held in [`Input::held`] / [`Input::pressed`].
pub const UP: u32 = 1;
pub const DOWN: u32 = 2;
pub const LEFT: u32 = 4;
pub const RIGHT: u32 = 8;
pub const A: u32 = 16;
pub const B: u32 = 32;
pub const DIRS: u32 = UP | DOWN | LEFT | RIGHT;

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

/// How long an arrow is held before it repeats, and how often it then
/// repeats, in ms.
const REPEAT_DELAY: i32 = 300;
const REPEAT_EVERY: i32 = 70;

/// Held arrows repeat after a delay, like keys in a text box.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default)]
pub struct KeyRepeat(pub i32);

impl KeyRepeat {
    /// The arrow bits to act on this frame.
    pub fn dirs(&mut self, input: &Input) -> u32 {
        let dirs = input.held & DIRS;
        if input.pressed & dirs != 0 {
            // counts up to REPEAT_EVERY, so the first repeat comes after
            // REPEAT_DELAY
            self.0 = REPEAT_EVERY - REPEAT_DELAY;
            return input.pressed & dirs;
        }
        if dirs == 0 {
            self.0 = 0;
            return 0;
        }
        self.0 += input.dt;
        if self.0 >= REPEAT_EVERY {
            self.0 -= REPEAT_EVERY;
            return dirs;
        }
        0
    }
}
