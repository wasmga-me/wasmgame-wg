//! A still background layer, drawn only when its key changes.

use super::{Screen, BACKGROUND, SHOWN};
use crate::hash;

impl Screen {
    /// What doesn't change from frame to frame (a board, a table) is drawn
    /// by `draw` into a second layer when `key` changes (or the screen is
    /// resized or reloaded), and copied under each frame.
    pub fn background(&mut self, key: u32, draw: impl FnOnce(&mut Screen)) {
        if self.layers.len() <= BACKGROUND {
            self.layers.push(vec![0; (self.w * self.h) as usize]);
            self.bg_key = 0;
        }
        // a new screen size needs a new background too; never 0, which
        // means nothing drawn yet
        let key = hash(hash(key, &self.sw.to_le_bytes()), &self.sh.to_le_bytes()) | 1;
        if key != self.bg_key {
            self.on_layer(BACKGROUND, draw);
            self.bg_key = key;
        }
        let (shown, background) = self.layers.split_at_mut(BACKGROUND);
        shown[SHOWN].copy_from_slice(&background[0]);
    }

    /// Redraws part of the background in place (call before `background`
    /// in the same frame, with the same key): `draw` runs on the background
    /// layer, usually inside `set_clip` (cleared afterwards). For a board
    /// where one square changes, without redrawing the whole board.
    pub fn background_edit(&mut self, draw: impl FnOnce(&mut Screen)) {
        if self.layers.len() <= BACKGROUND || self.bg_key == 0 {
            return; // nothing drawn yet: `background` draws it all anyway
        }
        self.on_layer(BACKGROUND, |s| {
            draw(s);
            s.no_clip();
        });
    }

    pub(crate) fn forget_background(&mut self) {
        self.bg_key = 0;
    }
}
