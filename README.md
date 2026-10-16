# wg: wasmga.me native games in Rust

The engine for [wasmga.me](https://wasmga.me)'s own games written in Rust:
the Rust side of the C games' `wg.h`, `wgfb.h` and `wgview.h`. A game
implements `wg::Game`, calls `wg::game!(MyGame)` once, and gets every `wg_*`
export the play page (wasmgame `src/scripts/engines/native.ts`) reads,
smooth drawing at the screen's own size, and the same Inter font as the C
games (`sdf_font.h`). The games themselves live in the wasmgame repo under
`games-src/<id>/`; `games-src/wgdemo` there is a whole small game to copy.

```toml
[dependencies]
wg = { git = "https://github.com/iris-offloading/wasmgame-wg" }
```

Each game's `Cargo.lock` pins the engine commit; `cargo update -p wg` moves a
game to the latest one.

## New game

1. `cp -r games-src/wgdemo games-src/<id>`, rename the package in its
   `Cargo.toml` to `<id>` (the module is built as `public/games/<id>/<id>.wasm`),
   delete its `target/` (keep `Cargo.lock`, it pins the engine).
2. Write the game (below), then `sh games-src/build.sh` (or
   `cargo build --release --target wasm32-unknown-unknown` in the folder while
   iterating; `rustup target add wasm32-unknown-unknown` once).
3. Catalog entry with `engine: "native"`, `setup`, `stats`, `pointer: true`
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
  this device and offers only levels up to it, so the picker appears once a
  player has got past level 1.
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

Also: `set_clip`/`no_clip`, `background_edit` (redraw one square of the
background inside a clip, without redrawing the board), `sprite_make_alpha`
(soft-edged sprites: drawn over black and white, blended when put), and for
games drawing their own rows (a road): `span_px`, `row_px` with `wg::blend`,
`px`, `size_px`.

Keys the pad lacks (digits, Backspace) reach `Game::key(code)` through the
optional `wg_key` export, when the page sends them.

The font's characters are those in `sdf_font.h`; add more with
`tools/sdf-font.py` (Pillow and NumPy) and copy the file to wasmgame's
`games-src/` so the C games get them too.
