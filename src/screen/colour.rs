//! Colours as the page reads them, and blending.

/// Colour as the page reads it: 0xAABBGGRR.
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF00_0000 | (b as u32) << 16 | (g as u32) << 8 | r as u32
}

/// `0xRRGGBB` as written in CSS, for colours copied from a design.
pub const fn hex(c: u32) -> u32 {
    rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

/// Blends colour `c` over the pixel with coverage `a` (0..256).
#[inline(always)]
pub fn blend(p: &mut u32, c: u32, a: u32) {
    if a >= 256 {
        *p = c;
        return;
    }
    if a == 0 {
        return;
    }
    let d = *p;
    let ia = 256 - a;
    let rb = (((c & 0xFF00FF) * a + (d & 0xFF00FF) * ia) >> 8) & 0xFF00FF;
    let g = (((c & 0x00FF00) * a + (d & 0x00FF00) * ia) >> 8) & 0x00FF00;
    *p = 0xFF00_0000 | rb | g;
}

/// Mixes two colours: `t` = 0..1 of `c` over `d`.
pub fn mix(c: u32, d: u32, t: f32) -> u32 {
    let mut out = d;
    blend(&mut out, c, (t.clamp(0.0, 1.0) * 256.0) as u32);
    out
}

/// The blend weight (0..256, for [`blend`]) of a pixel a shape covers
/// `cov` (0..1) of, drawn at `alpha`.
#[inline(always)]
pub(crate) fn weight(cov: f32, alpha: f32) -> u32 {
    (cov * alpha * 256.0) as u32
}
