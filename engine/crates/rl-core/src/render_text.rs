//! AffText 0x459454: original bitmap fonts, paired metrics and signed placement.
use crate::render::{i16, Context, Result};
const BASES: [u32; 6] = [0x23d5, 0x24b5, 0x2595, 0x2675, 0x2755, 0x2835];

impl Context<'_> {
    pub fn afftext(
        &mut self,
        text: &[u8],
        rectangle: [i32; 4],
        font: u32,
        flags: [u32; 2],
    ) -> Result<()> {
        if self.u32(0x4e1d94)? != 0 || self.u32(0x4e1d98)? != 0 {
            return self.stop(0x459635, "AffText external/multibyte font mode");
        }
        if !(1..=6).contains(&font) {
            return self.stop(0x459540, "AffText font outside bitmap banks 1..6");
        }
        if flags[0] != 0 {
            return self.stop(0x459764, "AffText colour substitution");
        }
        if flags[1] > 1 {
            return self.stop(0x459788, "AffText clip flag outside 0/1");
        }
        let line = text.split(|&c| c == 0 || c == 10).next().unwrap();
        if line.len() > 127 || line.iter().any(|&c| c < 32) {
            return self.stop(0x459618, "AffText control byte or signed cursor overflow");
        }
        let metrics = BASES[((font - 1) & !1) as usize];
        let height = self.assets.sprite(metrics + b'j' as u32 - 32)?.height as i32;
        let mut width = 0i32;
        let mut advances = Vec::with_capacity(line.len());
        for &c in line {
            let id = metrics + if c == b' ' { b'l' as u32 } else { c as u32 } - 32;
            let advance = self.assets.sprite(id)?.width as i32;
            advances.push(advance);
            width += advance;
        }
        let [mut x, mut y, right, bottom] = rectangle.map(i16);
        if x != right {
            if i16(width) <= right - x {
                x += (right - x - i16(width)) / 2;
            }
            y += (bottom - y - i16(height)) / 2;
        }
        x = i16(x);
        y = i16(y);
        for (&c, advance) in line.iter().zip(advances) {
            if c != b' ' {
                let id = BASES[font as usize - 1] + c as u32 - 32;
                // PasteBobCmp 0x45b247 without the clip flag clips to the whole
                // surface, (0, 0)..(799, 599) (0x45b28c): the screen clip.
                self.bob(x, y, id, flags[1] != 0, None)?;
            }
            x = i16(x + advance);
        }
        Ok(())
    }
}

/// AffText's measured width of one line (same metrics pairs and space rule),
/// for port-only layouts that centre or hit-test original captions.
pub(crate) fn text_width(
    assets: &crate::render_assets::Assets,
    text: &[u8],
    font: u32,
) -> Result<i32> {
    if !(1..=6).contains(&font) {
        return Err(crate::render::Error::Invalid(
            "text width font outside banks 1..6",
        ));
    }
    let metrics = BASES[((font - 1) & !1) as usize];
    let mut width = 0i32;
    for &c in text.split(|&c| c == 0 || c == 10).next().unwrap_or(&[]) {
        if c < 32 {
            return Err(crate::render::Error::Invalid("text width control byte"));
        }
        let id = metrics + if c == b' ' { b'l' as u32 } else { c as u32 } - 32;
        width += assets.sprite(id)?.width as i32;
    }
    Ok(width)
}

impl Context<'_> {
    /// AffText's layout with PasteOmbreCmp's raster: the glyph shapes darken the
    /// picture through the original shadow table instead of painting it. A
    /// port-only "unavailable" caption style (the original menus have none).
    pub(crate) fn afftext_shadow(
        &mut self,
        text: &[u8],
        rectangle: [i32; 4],
        font: u32,
    ) -> Result<()> {
        let width = text_width(self.assets, text, font)?;
        let metrics = BASES[((font - 1) & !1) as usize];
        let height = self.assets.sprite(metrics + b'j' as u32 - 32)?.height as i32;
        let [mut x, mut y, right, bottom] = rectangle.map(i16);
        if i16(width) <= right - x {
            x += (right - x - i16(width)) / 2;
        }
        y += (bottom - y - i16(height)) / 2;
        for &c in text.split(|&c| c == 0 || c == 10).next().unwrap_or(&[]) {
            let advance = self
                .assets
                .sprite(metrics + if c == b' ' { b'l' as u32 } else { c as u32 } - 32)?
                .width as i32;
            if c != b' ' {
                // Twice: one pass of the table is too faint on TITRE.PIE's wall.
                for _ in 0..2 {
                    self.shadow(x, y, BASES[font as usize - 1] + c as u32 - 32, false)?;
                }
            }
            x = i16(x + advance);
        }
        Ok(())
    }
}
