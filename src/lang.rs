//! The page's language (wg_lang), and a game's text in it.
//!
//! The page passes its language before wg_init as two lowercase letters,
//! `'z' | 'h' << 8`. It isn't part of the save, so a save plays on in the
//! language of the page it is loaded on. Text outside ASCII needs its glyphs:
//! see [`crate::Glyphs`].

use std::sync::atomic::{AtomicUsize, Ordering};

/// The languages a game's text comes in, in this order (as wg.h's WG_LANGS).
pub const LANGS: [&str; 9] = ["en", "zh", "ja", "ko", "es", "pt", "fr", "de", "ru"];
pub const EN: usize = 0;
pub const ZH: usize = 1;
pub const JA: usize = 2;
pub const KO: usize = 3;
pub const ES: usize = 4;
pub const PT: usize = 5;
pub const FR: usize = 6;
pub const DE: usize = 7;
pub const RU: usize = 8;

static LANG: AtomicUsize = AtomicUsize::new(EN);

/// The page's language, as an index into [`LANGS`] ([`EN`] for one it lacks).
pub fn lang() -> usize {
    LANG.load(Ordering::Relaxed)
}

/// The text in the page's language, from one string per language in
/// [`LANGS`] order: `wg::tr(&["Undo", "撤销", "元に戻す"])`. English for
/// languages left out or given as "".
pub fn tr(text: &[&'static str]) -> &'static str {
    text.get(lang()).copied().filter(|s| !s.is_empty()).unwrap_or(text[0])
}

/// wg_lang: sets the language when `code` is above 0; returns the one used.
#[doc(hidden)]
pub fn set_lang(code: i32) -> i32 {
    let code_of = |l: &str| l.as_bytes()[0] as i32 | (l.as_bytes()[1] as i32) << 8;
    if code > 0 {
        LANG.store(LANGS.iter().position(|&l| code_of(l) == code).unwrap_or(EN), Ordering::Relaxed);
    }
    code_of(LANGS[lang()])
}
