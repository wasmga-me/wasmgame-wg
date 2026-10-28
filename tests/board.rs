//! A tiny board game through the exports `wg::board!` makes: the page's
//! side of the accessible board (src/scripts/engines/board.ts on the site).
use wg::{Board, Game, Input, Screen};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Save {
    lit: [i32; 6],
    cur: [i32; 2],
}
unsafe impl wg::Plain for Save {}

#[derive(Default)]
struct Lamps {
    s: Save,
}

impl Game for Lamps {
    type Save = Save;
    fn new() -> Self {
        Lamps::default()
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn start(&mut self, _seed: u32) {
        self.s = Save::default();
    }
    fn update(&mut self, _input: &Input) -> u32 {
        0
    }
    fn draw(&mut self, _s: &mut Screen) {}
    fn score(&self) -> u32 {
        self.s.lit.iter().sum::<i32>() as u32
    }
}

impl Board for Lamps {
    fn grid(&self) -> Option<(i32, i32)> {
        Some((3, 2))
    }
    fn cursor(&self) -> (i32, i32) {
        (self.s.cur[0], self.s.cur[1])
    }
    fn focus(&mut self, x: i32, y: i32) {
        self.s.cur = [x, y];
    }
    fn rect(&self) -> (f32, f32, f32) {
        (100.0, 50.0, 40.0)
    }
    fn cell(&self, x: i32, y: i32, out: &mut String) {
        out.push_str(if self.s.lit[(y * 3 + x) as usize] == 1 { "lit" } else { "dark" });
    }
    fn status(&self, out: &mut String) {
        out.push_str(&format!("{} lit", self.score()));
    }
    fn keys(&self) -> &'static str {
        "Space switches a lamp."
    }
    fn act(&mut self, x: i32, y: i32, _a: i32, said: &mut String) -> u32 {
        self.focus(x, y);
        self.s.lit[(y * 3 + x) as usize] ^= 1;
        self.cell(x, y, said);
        wg::EV_BLIP
    }
}

wg::game!(Lamps);
wg::board!(Lamps);

fn text(kind: i32, x: i32, y: i32) -> String {
    let n = wg_a11y_text(kind, x, y) as usize;
    // SAFETY: wg_a11y_buf holds n bytes of UTF-8 until the next call
    String::from_utf8(unsafe { std::slice::from_raw_parts(wg_a11y_buf(), n) }.to_vec()).unwrap()
}

#[test]
fn board() {
    wg_init(1);
    wg_resize(800, 600);
    assert_eq!(wg_a11y_grid(), 3 << 16 | 2);
    // plan 400 x 300 at 800 x 600: twice the size
    assert_eq!((wg_a11y_rect(0), wg_a11y_rect(1), wg_a11y_rect(2)), (200, 100, 8000));
    assert_eq!(text(0, 2, 1), "dark");
    let rev = wg_a11y_rev();
    wg_a11y_act(2, 1, 0);
    assert_eq!(wg_a11y_cursor(), 1 << 16 | 2);
    assert_ne!(wg_a11y_rev(), rev);
    assert_eq!(text(0, 2, 1), "lit");
    assert_eq!(text(1, 0, 0), "1 lit");
    // what the move said is read once
    assert_eq!(text(2, 0, 0), "lit");
    assert_eq!(text(2, 0, 0), "");
    assert_eq!(text(3, 0, 0), "Space switches a lamp.");
    // its events come with the next frame
    assert_eq!(wg_frame(0, 16), wg::EV_BLIP as i32);
    wg_a11y_focus(0, 0);
    assert_eq!(wg_a11y_cursor(), 0);
    // a grid of squares has no boxes of its own
    assert_eq!(wg_a11y_box(0, 0, 0), -1);
}
