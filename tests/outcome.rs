//! Completion must remain distinguishable from defeat after loading a save.
use wg::{Game, Input, Screen};
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Save { outcome: i32 }
unsafe impl wg::Plain for Save {}
#[derive(Default)]
struct Outcome { s: Save }
impl Game for Outcome {
    type Save = Save;
    fn new() -> Self { Self::default() }
    fn save(&mut self) -> &mut Save { &mut self.s }
    fn start(&mut self, _: u32) { self.s = Save::default(); }
    fn update(&mut self, input: &Input) -> u32 {
        if input.pressed & wg::A != 0 { self.s.outcome=1; }
        if input.pressed & wg::B != 0 { self.s.outcome=2; }
        0
    }
    fn draw(&mut self, _: &mut Screen) {}
    fn score(&self) -> u32 { 500 }
    fn over(&self) -> bool { self.s.outcome!=0 }
    fn won(&self) -> bool { self.s.outcome==1 }
}
wg::game!(Outcome);
#[test]
fn outcome_survives_saved_state() {
    wg_init(1);
    assert_eq!((wg_over(),wg_won()),(0,0));
    wg_frame(wg::A as i32,16);
    assert_eq!((wg_over(),wg_won(),wg_score()),(1,1,500));
    let saved=unsafe {std::slice::from_raw_parts(wg_state(),wg_state_size() as usize).to_vec()};
    wg_init(1);
    wg_frame(wg::B as i32,16);
    assert_eq!((wg_over(),wg_won()),(1,0));
    unsafe {std::slice::from_raw_parts_mut(wg_state(),saved.len()).copy_from_slice(&saved);}
    assert_eq!(wg_state_loaded(),1);
    assert_eq!((wg_over(),wg_won(),wg_score()),(1,1,500));
}
