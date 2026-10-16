//! Draws one of everything through the exports a game gets from `wg::game!`
//! and checks the pixels against a known hash, so a change to the engine
//! that moves a pixel shows up. When a change is meant to alter the picture,
//! run `cargo test` and update the hashes it prints.
use wg::{hex, Game, Input, KeyRepeat, Pointer, Rng, Screen, Sprite};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Save {
    opt: [i32; 1],
    frames: i32,
    rng: Rng,
    rep: KeyRepeat,
    moved: i32,
    clicks: i32,
}
unsafe impl wg::Plain for Save {}

#[derive(Default)]
struct Test {
    s: Save,
    card: Sprite,
    gem: Sprite,
}

impl Game for Test {
    type Save = Save;
    const PLAN: (f32, f32) = (400.0, 300.0);
    const CHOICES: &'static [i32] = &[3];

    fn new() -> Self {
        Test::default()
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        self.s = Save { rng: Rng::new(seed), ..Save::default() };
    }
    fn update(&mut self, input: &Input) -> u32 {
        self.s.frames += 1;
        self.s.moved += self.s.rep.dirs(input).count_ones() as i32;
        if input.pressed & wg::A != 0 {
            wg::EV_BLIP
        } else {
            0
        }
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        if p.pressed & wg::M_LEFT != 0 {
            self.s.clicks += 1 + p.x as i32 * 1000 + p.y as i32 * 1_000_000;
            return wg::EV_HIT;
        }
        0
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h > w {
            (300.0, 400.0)
        } else {
            Self::PLAN
        }
    }
    fn draw(&mut self, s: &mut Screen) {
        let theme = self.s.opt[0] as u32;
        s.background(theme, |s| {
            s.clear(hex(0x1d2433));
            let (x0, y0, x1, y1) = s.visible();
            s.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x2b3550), hex(0x101420), 1.0);
            for i in 0..8 {
                s.rect(10.0 + i as f32 * 30.0, 10.0, 26.0, 26.0, 5.0, wg::rgb((40 * i % 256) as u8, 90, 200), 0.8);
            }
        });
        s.background_edit(|s| {
            s.set_clip(10.0, 10.0, 26.0, 26.0);
            s.rect(10.0, 10.0, 26.0, 26.0, 13.0, hex(0xffd166), 1.0);
        });
        s.shadow(40.0, 60.0, 80.0, 50.0, 8.0, 10.0, 0, 0.5);
        s.shadow_hollow(150.0, 60.0, 80.0, 50.0, 8.0, 10.0, 0, 0.5, 6.0);
        s.line(20.0, 130.0, 180.0, 170.0, 3.0, hex(0xd1495b), 1.0);
        s.line_grad(20.0, 180.0, 100.0, 200.0, 1.0, 6.0, hex(0xffffff), hex(0x00ff00), 0.9);
        s.line_grad(100.0, 200.0, 180.0, 180.0, 6.0, 2.0, hex(0x00ff00), hex(0x0000ff), 0.9);
        s.disc(250.0, 150.0, 20.0, hex(0xffffff), 0.7);
        s.ellipse(300.0, 150.0, 6.0, 20.0, hex(0x88ccff), 1.0);
        s.ring(350.0, 150.0, 22.0, 4.0, hex(0xffaa00), 1.0);
        s.tri((240.0, 200.0), (280.0, 240.0), (220.0, 250.0), hex(0x66ff99), 1.0);
        s.poly(&[(300.0, 200.0), (340.0, 200.0), (360.0, 230.0), (330.0, 260.0), (295.0, 240.0)], hex(0xcc66ff), 0.8);
        let w = s.text_width("Score 1234", 18.0);
        s.text("Score 1234", 20.0, 230.0, 18.0, hex(0xffffff), 1.0);
        s.text_c("Middle", 200.0, 260.0, 14.0, hex(0xffd166), 1.0);
        s.text_r("Right", 390.0, 280.0, s.fit_size("Right", 30.0, 40.0), hex(0x88ccff), 0.9);
        s.rect(20.0 + w, 222.0, s.hair(), 16.0, 0.0, hex(0xffffff), 1.0);
        s.sprite_make(&mut self.card, 1, 0.0, 0.0, 30.0, 40.0, |s| {
            s.rect(0.0, 0.0, 30.0, 40.0, 0.0, hex(0x223344), 1.0);
            s.ring(15.0, 20.0, 10.0, 3.0, hex(0xeeeeee), 1.0);
        });
        s.sprite_make_alpha(&mut self.gem, 2, 0.0, 0.0, 24.0, 24.0, |s| {
            s.disc(12.0, 12.0, 11.0, hex(0x33ddaa), 0.8);
        });
        for i in 0..4 {
            s.sprite(&self.card, 200.0 + i as f32 * 34.0, 40.0);
            s.sprite(&self.gem, 205.0 + i as f32 * 34.0, 50.0);
        }
        s.set_clip(0.0, 280.0, 100.0, 20.0);
        s.disc(50.0, 290.0, 30.0, hex(0xff0000), 1.0);
        s.no_clip();
        let (w, h) = s.size_px();
        let (_, y) = s.px(0.0, 295.0);
        let y = (y as i32).clamp(0, h - 1);
        for (i, p) in s.row_px(y).iter_mut().enumerate().take(w as usize / 2) {
            wg::blend(p, hex(0x00ffff), (i % 256) as u32);
        }
        s.span_px(y - 2, 10.5, 200.25, hex(0xff00ff), 0.6);
        s.text(&wg::clock(self.s.frames * 1000 + 65_000), 300.0, 20.0, 12.0, wg::mix(hex(0xffffff), 0, 0.5), 1.0);
    }
    fn score(&self) -> u32 {
        self.s.moved as u32
    }
    fn stat(&self, i: usize) -> i32 {
        [self.s.frames, self.s.clicks][..].get(i).copied().unwrap_or(-1)
    }
}

wg::game!(Test);

fn picture() -> u32 {
    let n = (wg_w() * wg_h()) as usize;
    let px = unsafe { core::slice::from_raw_parts(wg_fb(), n) };
    px.iter().fold(0, |h, p| wg::hash(h, &p.to_le_bytes()))
}

#[test]
fn draws_the_same() {
    let mut got = Vec::new();
    for &(w, h, theme) in &[(800, 600, 0), (1280, 600, 1), (390, 844, 2), (64, 64, 0)] {
        assert_eq!(wg_option(0, theme), theme);
        wg_init(7);
        wg_resize(w, h);
        wg_mouse(w / 2, h / 3, 1);
        wg_mouse(w / 2, h / 3, 0);
        assert_eq!(wg_frame(wg::A as i32, 16), (wg::EV_BLIP | wg::EV_HIT) as i32);
        assert_eq!(wg_drawn(), 1);
        for _ in 0..30 {
            wg_frame(wg::RIGHT as i32, 16);
        }
        got.push((wg_w(), wg_h(), picture(), wg_score(), wg_stat(1)));
    }
    println!("{got:?}");
    let want = [
        (800, 600, 2880280043, 4, 100200001),
        (1280, 600, 51378408, 4, 100200001),
        (390, 844, 1122853592, 4, 5200001),
        (64, 64, 3823362068, 4, 134153001),
    ];
    assert_eq!(got, want);
}

#[test]
fn rng_and_clock() {
    let mut r = Rng::new(42);
    let v: Vec<u32> = (0..4).map(|_| r.next()).collect();
    let mut items = [1, 2, 3, 4, 5, 6];
    r.shuffle(&mut items);
    assert_eq!(v, [11101449, 2902602413, 2175469510, 3408222863]);
    assert_eq!(items, [1, 5, 2, 4, 6, 3]);
    assert_eq!((r.below(10), r.range(-3, 3), r.float()), (5, -2, 0.5642815));
    assert_eq!(wg::clock(65_999), "1:05");
    assert_eq!(wg::ease_out(1.5), 1.0);
}
