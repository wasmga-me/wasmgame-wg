# wg: wasmga.me native games in Rust

The engine for [wasmga.me](https://wasmga.me)'s own games written in Rust:
the Rust side of the C games' `wg.h`, `wgfb.h` and `wgview.h`. A game
implements `wg::Game`, calls `wg::game!(MyGame)` once, and gets every `wg_*`
export the wasmga.me play page reads,
smooth drawing at the screen's own size, and the same Inter font as the C
games (`sdf_font.h`). The games themselves live in
[wasmga-me/games](https://github.com/wasmga-me/games), one folder each;
`wgdemo` there is a whole small game to copy.

```toml
[dependencies]
wg = { git = "https://github.com/wasmga-me/wasmgame-wg" }
```

Each game's `Cargo.lock` pins the engine commit; `cargo update -p wg` moves a
game to the latest one.

## New game

1. In the games repo, `cp -r wgdemo <id>`, rename the package in its
   `Cargo.toml` to `<id>` (the module is built as `public/games/<id>/<id>.wasm`),
   delete its `target/` (keep `Cargo.lock`, it pins the engine).
2. Write the game (below), then `sh build.sh` (or
   `cargo build --release --target wasm32-unknown-unknown` in the folder while
   iterating; `rustup target add wasm32-unknown-unknown` once).
3. On the site: a catalog entry with `engine: "native"`, `setup`, `stats`, `pointer: true`
   if it takes clicks or touches, and `swipe: true` for swipe games. Commit
   the built `.wasm` and the game's `Cargo.lock`: Workers Builds has no Rust,
   so build.sh keeps the committed build there.

## The Game trait

- `type Save`: a `#[repr(C)]` struct of integers, floats, arrays, `wg::Rng`
  and `wg::KeyRepeat`, with `unsafe impl wg::Plain for Save {}`. It is the
  whole save (autosave, Resume, Saves, cloud), copied byte for byte, so no
  bool, enum, Vec or references; keep a version number in it. Options live
  in it too (`opt: [i32; N]`, returned by `options()`, with `CHOICES` saying
  how many each has, in catalog `setup` order).
- `new()` makes the game with default options; `start(seed)` starts a run
  with the options picked (the runtime keeps `options()` as they were).
- `update(&Input) -> u32` moves it on: `held`/`pressed` button bits
  (`wg::UP`, `A`, ...) and `dt` in ms (0 means the page only wants the
  picture). Returns `wg::EV_*`; return `EV_GAMEOVER` once when the run ends
  (the page then shows game over and posts `score()`).
- `pointer(&Pointer) -> u32`: mouse and touch in plan units, with `pressed`
  and `released` worked out (`M_LEFT`, `M_RIGHT`, `touch` set for fingers).
- `draw(&mut Screen)`: draws the whole picture. It runs only when
  `picture()` changed (by default a hash of the save; override it when the
  save holds a timer that isn't shown every millisecond, or when `draw`
  reads anything outside the save), when `moving()` is true, or after a
  resize or a load.
- `over()` is true from the moment the run is lost or won (no autosave then,
  no Resume); `stat(i)` gives the catalog `stats` in order (-1 hides one).
- Levels: a catalog `setup` option with `"levels": N` (no `choices`; they
  are made as Level 1..N) is a level picker. Give it N in `CHOICES`, start
  the run at the level picked, and return the level the run is on from
  `reached()` (0 for the first). The page keeps the furthest level reached on
  this device and shows a level select (a grid of levels, locked past the
  furthest reached, each with the player's best on it). Each level has its
  own scoreboard too: return `EV_LEVEL` from `update` when a level is
  cleared (the last one as well) and the level and the score made on it
  alone from `cleared()`.
- `loaded()` checks a save just copied in: return false to drop it, and
  rebuild anything that follows from the save (and reset animations).
- `PLAN` (or `plan(w, h)` to lay out differently on a tall phone) is the
  size in the game's own units; `Screen` scales it to fit and centres it.

Panics go to the page's error screen with the game's stack (`wg_panic`).

## Screen

All in plan units: `clear`, `rect`, `rect_grad`, `shadow`, `shadow_hollow`,
`line`, `line_grad`, `disc`, `ellipse`, `ring`, `poly`, `tri`, `text`,
`text_c`, `text_r`, `text_width`, `fit_size`; `visible()` is the area the
screen shows (wider or taller than the plan), `hair()` one pixel. Colours are
`wg::hex(0xRRGGBB)` or `wg::rgb(r, g, b)`, `wg::mix` blends two.

Keep frames under ~10 ms at 2.4 MP: draw what stays still (a board, a table)
once with `s.background(key, |s| ...)`, which redraws only when `key`
changes, and things drawn many times the same (card backs) once into a
`wg::Sprite` with `sprite_make`, then `s.sprite(&sp, x, y)`.

Art made elsewhere (a PNG, say models rendered into a sprite sheet) loads
with `wg::Image::png(include_bytes!("art.png"))` once in `new()`; `crop`
cuts one sprite out of a sheet. `s.image(&img, x, y, w, h, alpha)` draws it
filling a plan box (a negative `w` mirrors it), `s.image_turned(&img, cx, cy,
w, h, angle, alpha)` turned about its middle. Each size an image is drawn at
is scaled once, smoothly, and kept.

Also: `set_clip`/`no_clip`, `background_edit` (redraw one square of the
background inside a clip, without redrawing the board), `sprite_make_alpha`
(soft-edged sprites: drawn over black and white, blended when put), and for
games drawing their own rows (a road): `span_px`, `row_px` with `wg::blend`,
`px`, `size_px`.

A game the page draws in 3D (it exports `wg3d_scene`, as `partial-combat/` in
[wasmga-me/games](https://github.com/wasmga-me/games) does) calls
`s.overlay(true)` and clears the screen to 0 each frame: drawing then keeps
each pixel's coverage as alpha, and the page lays the screen over the scene as
a HUD.

Keys the pad lacks (digits, Backspace) reach `Game::key(code)` through the
optional `wg_key` export, when the page sends them.

The font's characters are those in `sdf_font.h`; add more with
`tools/sdf-font.py` (Pillow and NumPy) and copy the file to
[wasmgame-wg-c](https://github.com/wasmga-me/wasmgame-wg-c) so the C games
get them too.

## Languages

The page tells the game its language before `wg_init` (`wg_lang`); pick the
text with `wg::tr(&["Undo", "撤销", "元に戻す"])`, one string per language in
`wg::LANGS` order (English for any left out; Traditional Chinese, last, falls
back to the Simplified text), or match on `wg::lang()` around
a `format!`. Text outside ASCII needs its glyphs: `python3 tools/sdf-font.py
--extra src/glyphs.rs src/*.rs` (from the game's folder, with this repo's
tool) writes every character the game's string literals use beyond
`sdf_font.h` (accented Latin and Cyrillic from Inter, kana, Hangul and Han
from Noto Sans) into `src/glyphs.rs` and `src/glyphs.bin`; `mod glyphs;` and
`wg::add_glyphs(&glyphs::GLYPHS)` in `new()` turn them on. Run it again after
changing the text.

## Accessible board

A board game (a grid of squares, played a move at a time) can also be
played with a screen reader, a keyboard alone or a switch: implement
`wg::Board` and add `wg::board!(MyGame);` after `wg::game!`. The play page
then lays a grid of focusable squares over the board the game draws; arrows
move between squares, Space makes the main move, X the second (flag, note),
I reads the status, and what each move did is read out. The game supplies
the words (each square's state, the status, what a move did, the keys), in
the page's language with `wg::tr`, and the board's place in plan units; the
page adds each square's name (A1, B1 ...). A board that isn't a grid of
squares (tubes in a rack, a hex board, screws on plates) also gives each
square its own box with `Board::cell_box`; the page then lays each one where
it is drawn and the game's words name it. `tests/board.rs` is a whole small
example. Sapper (`games-src/sapper.c` on the site) is the C version.

## Working on the engine

`src/lib.rs` holds the events and re-exports; `game.rs` the `Game` trait and
`Plain`, `input.rs` buttons, pointer and key repeat, `rng.rs` the `Rng`,
`runtime.rs` the exports and `game!`, `board.rs` the accessible board and
`board!`, and `screen/` the drawing (`mod.rs`
the screen and its clip, then `shapes.rs`, `text.rs`, `background.rs`,
`sprite.rs`, `colour.rs`).

`cargo test` draws one of everything through the exports and checks the
pixels by hash, so a change that should leave the picture alone proves it;
when a change means to alter it, update the hashes the test prints. Run
`cargo fmt` before committing.
