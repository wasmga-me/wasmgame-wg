//! The page's language (wg_lang), and a game's text in it.
//!
//! The page passes its language before wg_init as two lowercase letters,
//! `'z' | 'h' << 8`. It isn't part of the save, so a save plays on in the
//! language of the page it is loaded on. Text outside ASCII needs its glyphs:
//! see [`crate::Glyphs`].

use std::sync::atomic::{AtomicI32, Ordering};

const fn code(a: u8, b: u8) -> i32 {
    a as i32 | (b as i32) << 8
}

pub const EN: i32 = code(b'e', b'n');
pub const ZH: i32 = code(b'z', b'h');

static LANG: AtomicI32 = AtomicI32::new(EN);

/// The page's language, such as [`ZH`].
pub fn lang() -> i32 {
    LANG.load(Ordering::Relaxed)
}

/// Whether the page is in Chinese.
pub fn zh() -> bool {
    lang() == ZH
}

/// `en`, or `zh` on a Chinese page: `wg::tr("Undo", "撤销")`.
pub fn tr(en: &'static str, zh: &'static str) -> &'static str {
    if self::zh() {
        zh
    } else {
        en
    }
}

/// wg_lang: sets the language when `code` is above 0; returns it.
#[doc(hidden)]
pub fn set_lang(code: i32) -> i32 {
    if code > 0 {
        LANG.store(code, Ordering::Relaxed);
    }
    lang()
}
