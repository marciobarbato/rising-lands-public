//! Graphics 1.7 scripted CAVE, builder/order, placement and tooltip branches.
use crate::render::{i16, w, Context, Error, Result, SavedBackground};
impl Context<'_> {
    /// AffBatiments terrain0x37a, intact40b7f3..40be08 (other modes: facade37a_modes.rs).
    pub(crate) fn facade37a(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // Native calls BatimentGetEtatPourcent before checking the mode.
        if self.u16(0x495d90 + 52 * u32::from(r[8]))? == 0 {
            return self.stop(0x41a93b, "facade37a zero maximum health");
        }
        let (mut x, mut y) = (i16(x - 84), i16(y - 73));
        if !matches!(r[5], 3..=5) {
            return self.facade37a_construction(x, y, index, r);
        }
        let inhabited = self.inhabited(index)?;
        if r[5] == 3 && !inhabited {
            (x, y) = self.facade37a_collapse(x, y, r)?;
        }
        if inhabited {
            if r[4] > 4 {
                return self.stop(0x411147, "facade37a owner palette");
            }
            self.bob(x, y, 0x1c53, true, Some(u32::from(r[4])))?;
            let phase = self.u32(0x508f80)?.wrapping_add(cell) & 7;
            self.bob(i16(x + 2), i16(y + 28), 0x1c54 + phase, true, None)?;
            self.bob(i16(x + 86), i16(y + 37), 0x1c5c + phase, true, None)?;
        } else {
            let parts = [
                (0x1c53, x, y),
                (0x1c54, i16(x + 2), i16(y + 28)),
                (0x1c5c, i16(x + 86), i16(y + 37)),
            ];
            for (id, x, y) in parts {
                self.bob(x, y, id, true, None)?;
            }
            for (id, x, y) in parts {
                self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, id as i32, 1]);
                self.remap_sprite(x, y, id, 0x4e24c4)?;
            }
        }
        if w(r, 6) != 0 {
            return self.aff_flamme(x, y, cell, index, 16);
        }
        Ok(())
    }
    pub(crate) fn set_clip(&mut self, left: i32, top: i32, right: i32, bottom: i32) -> Result<()> {
        let mut bytes = Vec::new();
        for v in [top, bottom, left, right] {
            bytes.extend((v as u16).to_le_bytes());
        }
        self.put("recorte", 0x49a01c, &bytes)?;
        self.clip = [
            left as u16 as i32,
            top as u16 as i32,
            right as u16 as i32,
            bottom as u16 as i32,
        ];
        Ok(())
    }
    /// AffBatiments407ea2 wall branch. ChercheIndiceBatiment41b54a has
    /// already selected the same region-table record in the scene dispatcher.
    pub(crate) fn wall_facade(&mut self, x: i32, y: i32, r: &[u8]) -> Result<()> {
        let maximum = self.u16(0x495d90 + 52 * u32::from(r[8]))?;
        if maximum == 0 {
            return self.stop(0x41a93b, "wall facade zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16;
        let x = x as i16 as i32;
        let y = y as i16 as i32;
        let top = (y + (100 - i32::from(health)) * 32 / 100) as i16 as i32;
        self.set_clip(
            x.max(0),
            top.max(0),
            (self.u16(0x508724)? * 32 - 1) as u16 as i32,
            (self.u16(0x508726)? * 32 - 1) as u16 as i32,
        )?;
        // 0x407f91: the sprite is 0x1cc0 + word +0x7c with no check. Under a non-wall
        // record (a turret whose hover cell lies on a wall object) that word is the
        // colonist exit cell after BatimentRetireColon, so the ID can leave the
        // catalog. PasteBobCmp 0x45b247 then reads past the sprite table and the
        // native differential faults (wall_sold-v1); the port draws nothing there.
        let id = (w(r, 0x7c) + 0x1cc0) & 65535;
        if self.assets.sprite(id).is_err() {
            return Ok(());
        }
        self.bob(x, y, id, true, None)
    }
    /// AffBatiments object code 253 (the gate, [0x495bf9]), intact 407fa7..4080e3:
    /// the wall's clip (BatimentGetEtat), then sprite 0x1d98 + byte +0x76 at the
    /// cell when word +0x7c is 0, else 0x1db8 + byte +0x76 at (x + 5, y + 4),
    /// through the wall's PasteBobCmp tail (408412 / 408418).
    pub(crate) fn gate_facade(&mut self, x: i32, y: i32, r: &[u8]) -> Result<()> {
        let maximum = self.structure_maximum(r)?;
        let health = (200 * w(r, 10) / maximum) as u16;
        let x = x as i16 as i32;
        let y = y as i16 as i32;
        let top = (y + (100 - i32::from(health)) * 32 / 100) as i16 as i32;
        self.set_clip(
            x.max(0),
            top.max(0),
            (self.u16(0x508724)? * 32 - 1) as u16 as i32,
            (self.u16(0x508726)? * 32 - 1) as u16 as i32,
        )?;
        let (id, px, py) = if w(r, 0x7c) == 0 {
            (0x1d98 + u32::from(r[0x76]), x, y)
        } else {
            (0x1db8 + u32::from(r[0x76]), i16(x + 5), i16(y + 4))
        };
        // As for the wall: an id past the sprite catalog makes PasteBobCmp read
        // past the table natively; the port draws nothing there.
        if self.assets.sprite(id).is_err() {
            return Ok(());
        }
        self.bob(px, py, id, true, None)
    }
    pub(crate) fn remap_sprite(&mut self, x: i32, y: i32, id: u32, slot: u32) -> Result<()> {
        let q = self.assets.sprite(id)?;
        let table = self.m.read(self.u32(slot)?, 256)?;
        let [l, t, r, b] = self.clip;
        for sy in 0.max(t - y)..(q.height as i32).min(b.min(599) - y + 1) {
            // Column 800 (collapse clip) wraps to the next row like PasteBobCmp.
            for sx in 0.max(l - x)..(q.width as i32).min(r.min(800) - x + 1) {
                if q.mask[sy as usize * q.width + sx as usize] != 0 {
                    let p = (y + sy) as usize * 800 + (x + sx) as usize;
                    if p >= self.surface.len() {
                        return self
                            .stop(0x45bcb0, "grey remap column 800 on the last surface row");
                    }
                    self.surface[p] = table[self.surface[p] as usize];
                }
            }
        }
        Ok(())
    }
    /// AffBatiments terrain0x2fb, intact408dfc..40935d. Shared native
    /// palette, grayscale and light-mask primitives match the other facades.
    pub(crate) fn facade2fb(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 27), i16(y - 93));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 145 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                self.bob(x, y, 0x1ba4, true, Some(u32::from(r[4])))?;
                let phase = self.u32(0x508f80)?.wrapping_add(cell) & 7;
                self.bob(i16(x + 13), i16(y + 23), 0x1bad + phase, true, None)?;
                self.shadow(i16(x - 36), i16(y + 9), 0x1ba5 + phase, true)?;
            } else {
                self.bob(x, y, 0x1ba4, true, None)?;
                self.bob(i16(x + 13), i16(y + 23), 0x1bad, true, None)?;
                for (id, px, py) in [(0x1ba4, x, y), (0x1bad, i16(x + 13), i16(y + 23))] {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 0);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 145 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1ba3, true, None);
        }
        let offset = (200 - health) * 145 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1ba3, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1ba4, true, None)?;
        self.bob(i16(x + 13), i16(y + 23), 0x1bad, true, None)?;
        if !inhabited {
            for (id, px, py) in [(0x1ba4, x, y), (0x1bad, i16(x + 13), i16(y + 23))] {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffBatiments terrain0x3f8, intact40d89c..40dd7b.
    pub(crate) fn facade3f8(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade3f8 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 28), i16(y - 68));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 83 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                self.bob(x, y, 0x1caf, true, Some(u32::from(r[4])))?;
                let phase = self.u32(0x508f80)?.wrapping_add(cell) & 3;
                self.bob(i16(x + 40), i16(y + 26), 0x1cb0 + phase, true, None)?;
            } else {
                self.bob(x, y, 0x1caf, true, None)?;
                self.bob(i16(x + 40), i16(y + 26), 0x1cb0, true, None)?;
                for (id, px, py) in [(0x1caf, x, y), (0x1cb0, i16(x + 40), i16(y + 26))] {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 18);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 83 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1cae, true, None);
        }
        let offset = (200 - health) * 83 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1cae, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1caf, true, None)?;
        self.bob(i16(x + 40), i16(y + 26), 0x1cb0, true, None)?;
        if !inhabited {
            for (id, px, py) in [(0x1caf, x, y), (0x1cb0, i16(x + 40), i16(y + 26))] {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }

    /// AffBatiments terrain0x406, intact40dd80..40e1f8: type10 facade.
    /// Sprite1cb4 is the base; the level sprite1cd0+(level-1)*4(+phase) is
    /// drawn at(-6,-25). The health ratio41a8e5 reads the record's own type.
    pub(crate) fn facade406(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade406 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 25), i16(y - 37));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let level = u32::from(r[0x8a]);
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 57 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            let (px, py) = (i16(x - 6), i16(y - 25));
            // Native WORD sprite arithmetic: level0 wraps below the base.
            let base = 0x1cd0u32.wrapping_add(level.wrapping_sub(1).wrapping_mul(4));
            if inhabited {
                let id = base.wrapping_add(u32::from(r[0x76])) & 65535;
                self.bob(px, py, id, true, Some(u32::from(r[4])))?;
            } else {
                let id = base & 65535;
                self.bob(px, py, id, true, None)?;
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 10);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 57 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1cb4, true, None);
        }
        let offset = (200 - health) * 57 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1cb4, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        let (px, py) = (i16(x - 6), i16(y - 25));
        self.bob(px, py, 0x1cd0, true, None)?;
        if !inhabited {
            self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, 0x1cd0, 1]);
            self.remap_sprite(px, py, 0x1cd0, 0x4e24c4)?;
        }
        Ok(())
    }
    /// AffBatiments terrain0x436, intact40e975..40ee05: type11 facade.
    /// Sprite1cbf is the base; the level sprite1d50+(level-1)*2(+phase) is
    /// drawn in place, and phase1 adds1fd4+level at(-13,-20) when inhabited.
    pub(crate) fn facade436(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade436 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 44), i16(y - 45));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let level = u32::from(r[0x8a]);
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 69 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            let base = 0x1d50u32.wrapping_add(level.wrapping_sub(1).wrapping_mul(2));
            if inhabited {
                let id = base.wrapping_add(u32::from(r[0x76])) & 65535;
                self.bob(x, y, id, true, Some(u32::from(r[4])))?;
                if r[0x76] == 1 {
                    let id = 0x1fd4u32.wrapping_add(level) & 65535;
                    self.bob(i16(x - 13), i16(y - 20), id, true, None)?;
                }
            } else {
                let id = base & 65535;
                self.bob(x, y, id, true, None)?;
                self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, id as i32, 1]);
                self.remap_sprite(x, y, id, 0x4e24c4)?;
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 11);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 69 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1cbf, true, None);
        }
        let offset = (200 - health) * 69 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1cbf, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1d50, true, None)?;
        if !inhabited {
            self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, 0x1d50, 1]);
            self.remap_sprite(x, y, 0x1d50, 0x4e24c4)?;
        }
        Ok(())
    }
    /// AffBatiments terrain 0x40e, intact 40ee06..40f23b: the type-21 tent.
    /// Base 0x1c6e, tent sprite 0x1d90 (+ (tick + cell) & 7 when inhabited,
    /// in the owner palette); grey when empty; fire tail passes 21.
    pub(crate) fn facade40e(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade40e zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 44), i16(y - 45));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 93 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                let tick = self.u16(0x508f84)?;
                let id = 0x1d90 + (tick.wrapping_add(cell) & 7);
                self.bob(x, y, id, true, Some(u32::from(r[4])))?;
            } else {
                self.bob(x, y, 0x1d90, true, None)?;
                self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, 0x1d90, 1]);
                self.remap_sprite(x, y, 0x1d90, 0x4e24c4)?;
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 21);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 93 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1c6e, true, None);
        }
        let offset = (200 - health) * 93 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1c6e, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1d90, true, None)?;
        if !inhabited {
            self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, 0x1d90, 1]);
            self.remap_sprite(x, y, 0x1d90, 0x4e24c4)?;
        }
        Ok(())
    }
    /// AffBatiments terrain0x42a, intact40e1f9..40e974: the type6 storehouse
    /// (its fire tail passes7 to AffFeuBatiment).
    /// Base1cb5 (height129) and parts1cb6/1cb7/1cb9/1cbb/1cbd; inhabited modes
    /// pick the full or empty pile by the stock words (+0x7c >250, +0x80 >125,
    /// +0x82 >180, +0x7e >80, signed). Native reads the latter three through
    /// the high word of a misaligned dword.
    pub(crate) fn facade42a(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade42a zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 48), i16(y - 97));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let owner = u32::from(r[4]);
        let parts = |x: i32, y: i32| -> [(u32, i32, i32); 5] {
            [
                (0x1cb6, x, y),
                (0x1cb7, i16(x + 15), i16(y + 45)),
                (0x1cb9, i16(x + 75), i16(y + 49)),
                (0x1cbb, i16(x + 94), i16(y + 69)),
                (0x1cbd, i16(x + 48), i16(y + 97)),
            ]
        };
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 129 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                if owner > 4 {
                    // PastePlayerCmp45c705 indexes0x4e24c8 by the raw owner
                    // byte; only the five player tables are reconstructed.
                    return self.stop(0x45c705, "facade42a owner palette table outside players");
                }
                self.bob(x, y, 0x1cb6, true, Some(owner))?;
                let signed = |o: usize| i32::from(w(r, o) as u16 as i16);
                let piles = [
                    (w(r, 0x7c) as i32 > 250, 0x1cb7, 0x1cb8, 15, 45),
                    (signed(0x80) > 125, 0x1cb9, 0x1cba, 75, 49),
                    (signed(0x82) > 180, 0x1cbb, 0x1cbc, 94, 69),
                    (signed(0x7e) > 80, 0x1cbd, 0x1cbe, 48, 97),
                ];
                for (full, a, b, dx, dy) in piles {
                    self.bob(
                        i16(x + dx),
                        i16(y + dy),
                        if full { a } else { b },
                        true,
                        None,
                    )?;
                }
            } else {
                for (id, px, py) in parts(x, y) {
                    self.bob(px, py, id, true, None)?;
                }
                for (id, px, py) in parts(x, y) {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            // The intact42a body jumps into the type-7 tail (40e4d4 -> 40d5d2:
            // +6 test, push 7, shared 40d5e2), so a burning storehouse uses the
            // type-7 flame layout.
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 7);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 129 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1cb5, true, None);
        }
        let offset = (200 - health) * 129 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1cb5, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        for (id, px, py) in parts(x, y) {
            self.bob(px, py, id, true, None)?;
        }
        if !inhabited {
            for (id, px, py) in parts(x, y) {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffBatiments terrain0x35e, intact40abad..40b29c: type14 workshop.
    /// Base1c3d (height113) and four parts1c3e/1c3f/1c47/1c4f; inhabited modes
    /// animate the wheel (frame+owner)&7, the phase byte+0x76 and the load
    /// byte+0x74 ((owner+frame)%3). Mode4 inhabited also calls
    /// AffVehiculeReparer4029d0 (vehicle under repair, vehicle_repair_render.rs).
    pub(crate) fn facade35e(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade35e zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let origin = (x, y);
        let (mut x, mut y) = (i16(x - 74), i16(y - 47));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let owner = u32::from(r[4]);
        let parts = |x: i32, y: i32| -> [(u32, i32, i32); 4] {
            [
                (0x1c3e, x, y),
                (0x1c3f, i16(x + 40), i16(y + 31)),
                (0x1c47, i16(x + 9), i16(y + 46)),
                (0x1c4f, i16(x + 68), i16(y + 3)),
            ]
        };
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 113 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                // 40ad7e..40ad96: mode 4 draws the vehicle under repair first.
                if r[5] == 4 {
                    self.vehicle_repair(origin.0, origin.1, index)?;
                }
                self.bob(x, y, 0x1c3e, true, Some(owner))?;
                // PastePlayerCmp returns with `ret 4`, so [esp] at40adb6/40ae39
                // is the cell word again, not the pushed owner: wheel and load
                // phases animate by (frame + cell).
                let frame = self.u16(0x508f80)?;
                self.bob(
                    i16(x + 40),
                    i16(y + 31),
                    0x1c3f + ((frame + cell) & 7),
                    true,
                    None,
                )?;
                self.bob(
                    i16(x + 9),
                    i16(y + 46),
                    (0x1c47 + u32::from(r[0x76])) & 65535,
                    true,
                    None,
                )?;
                let load = if r[0x74] != 0 {
                    0x1c4f + (cell.wrapping_add(self.u32(0x508f80)?) % 3)
                } else {
                    0x1c4f
                };
                self.bob(i16(x + 68), i16(y + 3), load, true, None)?;
            } else {
                for (id, px, py) in parts(x, y) {
                    self.bob(px, py, id, true, None)?;
                }
                for (id, px, py) in parts(x, y) {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 14);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 113 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1c3d, true, None);
        }
        let offset = (200 - health) * 113 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1c3d, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        for (id, px, py) in parts(x, y) {
            self.bob(px, py, id, true, None)?;
        }
        if !inhabited {
            for (id, px, py) in parts(x, y) {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffBatiments terrain0x388, intact40b29d..40b7f2: type19 facade.
    /// Base1c64 (height88), body1c65 and the roof part1c66(+wheel phase) at
    /// (0,+41); inhabited modes add the smoke2b2d+((frame+owner)&15) at (+38,0).
    pub(crate) fn facade388(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade388 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 47), i16(y - 46));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let owner = u32::from(r[4]);
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 88 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                if owner > 4 {
                    return self.stop(0x45c705, "facade388 owner palette table outside players");
                }
                self.bob(x, y, 0x1c65, true, Some(owner))?;
                // [esp] after PastePlayerCmp's `ret 4` is the cell word (40b4b2/40b4e3).
                let frame = self.u16(0x508f80)?;
                self.bob(x, i16(y + 41), 0x1c66 + ((frame + cell) & 7), true, None)?;
                let smoke = self.u16(0x508f7c)?;
                self.bob(i16(x + 38), y, 0x2b2d + ((smoke + cell) & 15), true, None)?;
            } else {
                self.bob(x, y, 0x1c65, true, None)?;
                self.bob(x, i16(y + 41), 0x1c66, true, None)?;
                for (id, px, py) in [(0x1c65, x, y), (0x1c66, x, i16(y + 41))] {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 19);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 88 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1c64, true, None);
        }
        let offset = (200 - health) * 88 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1c64, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1c65, true, None)?;
        self.bob(x, i16(y + 41), 0x1c66, true, None)?;
        if inhabited {
            // 40b7b1 ->4080e2 ->408418: the healthy inhabited roof shows the
            // first smoke frame2b2d without animation.
            self.bob(i16(x + 38), y, 0x2b2d, true, None)?;
        } else {
            for (id, px, py) in [(0x1c65, x, y), (0x1c66, x, i16(y + 41))] {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffBatiments terrain0x390, intact40be09..40c19b: type-17 facade with
    /// no owner palette. Base1c6f and roof1c70 (height69) at (-6,-33). Modes
    /// 3..5 draw only the animated roof1c70+((frame84+cell)&7); mode3 first
    /// applies the shared three-jitter collapse clip. Construction modes draw
    /// the base under the health clip, then the plain roof. A damage word
    /// (+6 != 0) continues into AffFlamme with type17 (40d5e2): explicit stop.
    pub(crate) fn facade390(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        let _ = index;
        let maximum = self.u16(0x495d90 + 52 * u32::from(r[8]))?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade390 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 6), i16(y - 33));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        if matches!(r[5], 3..=5) {
            if r[5] == 3 {
                let bottom = i16(y + 69 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            // 40bfa7: the roof animates on the0x508f84 counter plus the cell
            // word left at [esp], never on the owner palette.
            let frame = self.u16(0x508f84)?;
            self.bob(x, y, 0x1c70 + ((frame + cell) & 7), true, None)?;
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 17);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 69 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1c6f, true, None);
        }
        let offset = (200 - health) * 69 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1c6f, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1c70, true, None)
    }
    /// AffBatiments terrain0x3ce, intact40cc60..40d1d1: type-20 monastery.
    /// Base1c94, body1c95 and roof1c98 (height116) at (-61,-65); the roof sits
    /// at (+1,+40) and animates on (frame80+cell)&7 only in modes3..5, where a
    /// type-20 record also draws the upgrade sub-building (AffUpgradeMonastere
    /// 402ee9) at (+10,-41). Collapse (mode3, uninhabited) applies the shared
    /// three jitters and then +75 to y. A damage word continues to AffFlamme
    /// type15 (40d5e2): explicit stop.
    pub(crate) fn facade3ce(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        let maximum = self.u16(0x495d90 + 52 * u32::from(r[8]))?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade3ce zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 61), i16(y - 65));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let owner = u32::from(r[4]);
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 116 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
                // 40ce0a: the collapsed monastery body sits75 lower.
                y = i16(y + 75);
            }
            let (ux, uy) = (i16(x + 10), i16(y - 41));
            if inhabited {
                if owner > 4 {
                    return self.stop(0x45c705, "facade3ce owner palette table outside players");
                }
                self.bob(x, y, 0x1c95, true, Some(owner))?;
                if r[8] == 20 {
                    self.upgrade_monastere(ux, uy, cell, index)?;
                }
                // [esp] after PastePlayerCmp's `ret 4` is the cell word (40ce9d).
                let frame = self.u16(0x508f80)?;
                self.bob(
                    i16(x + 1),
                    i16(y + 40),
                    0x1c98 + ((frame + cell) & 7),
                    true,
                    Some(owner),
                )?;
            } else {
                self.bob(x, y, 0x1c95, true, None)?;
                if r[8] == 20 {
                    self.upgrade_monastere(ux, uy, cell, index)?;
                }
                self.bob(i16(x + 1), i16(y + 40), 0x1c98, true, None)?;
                for (id, px, py) in [(0x1c95, x, y), (0x1c98, i16(x + 1), i16(y + 40))] {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 15);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 116 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1c94, true, None);
        }
        let offset = (200 - health) * 116 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1c94, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1c95, true, None)?;
        self.bob(i16(x + 1), i16(y + 40), 0x1c98, true, None)?;
        if !inhabited {
            for (id, px, py) in [(0x1c95, x, y), (0x1c98, i16(x + 1), i16(y + 40))] {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffRentrerHangar 0x402da2..0x402ee9 (EBX = structure): the unit entering,
    /// word +0x88, at its entry cell from BatimentGetCaseEntrer 0x41d057 (type 8:
    /// the origin plus 0x403 for kind 3 and vehicles 11..17, else plus 0x303;
    /// type 14: plus 0x402; other types: 0). When the unit's cell, word +0x7c, is
    /// lit for the local player (CaseEstIlluminee: light byte AND 0x10 clear) and
    /// the unit word is not 0xffff: SetClipping from ((sx minus 1) times 32 minus
    /// 2, (sy minus 2) times 32 minus 19), each floored at 0 as a signed word, to
    /// the view size; AffDeplaceEngin 0x412390 at sx times 32 plus word +0x80, sy
    /// times 32 plus word +0x82; then RestaureClipping 0x40f4e8. The two other
    /// exits (0x402c7a) return without restoring. The original reads unit
    /// 0xffff's bytes +1 and +0x7c, past the pool, before its own 0xffff test;
    /// neither exit draws after those reads, so the port returns at once (an
    /// unmapped page there would be the original's crash).
    pub(crate) fn rentrer_hangar(&mut self, index: u32) -> Result<()> {
        let record = 0x49a0c8 + 276 * (index & 0xffff);
        let unit = self.u16(record + 0x88)?;
        if unit == 0xffff {
            return Ok(());
        }
        let pool = self.u32(0x4e19a0)?;
        let at = pool.wrapping_add(276 * unit);
        let kind = self.u8(at + 1)?;
        let origin = self.u16(record + 2)?;
        let entry = match self.u8(record + 8)? {
            8 if kind == 3 || (11..=17).contains(&kind) => origin + 0x403,
            8 => origin + 0x303,
            14 => origin + 0x402,
            _ => 0,
        } & 0xffff;
        let sx = (entry & 255) as i32 - self.u16(0x508720)? as i32;
        let sy = (entry >> 8) as i32 - self.u16(0x508722)? as i32;
        let local = self.u8(0x508041)?;
        let light = self.u32(0x507d88 + 4 * local)?;
        let cell = self.u16(at + 0x7c)?;
        if self.u8(light.wrapping_add(cell))? & 0x10 != 0 {
            return Ok(());
        }
        let left = i16((sx - 1) * 32 - 2).max(0);
        let top = i16((sy - 2) * 32 - 0x13).max(0);
        let width = (self.u16(0x508724)? as i32 * 32) & 0xffff;
        let height = (self.u16(0x508726)? as i32 * 32) & 0xffff;
        self.set_clip(left, top, width, height)?;
        let x = i16(sx * 32 + self.u16(record + 0x80)? as i32);
        let y = i16(sy * 32 + self.u16(record + 0x82)? as i32);
        let r = self.m.read(at, 276)?.to_vec();
        self.deplace_body(x, y, unit, &r)?;
        let saved = [
            self.u16(0x49a02c)? as i32,
            self.u16(0x49a030)? as i32,
            self.u16(0x49a02e)? as i32,
            self.u16(0x49a032)? as i32,
        ];
        self.set_clip(saved[0], saved[1], saved[2], saved[3])
    }
    /// AffBatiments terrain 0x3b0, intact 40c5a6..40cc60: the hangar (type 8)
    /// facade at (-157, -62). Base 1c7a (height 98), body 1c7b, doors 1c7c at
    /// (0, 55), 1c84 at (137, 55) and 1c8c at (49, 40). Modes 3..5 add the
    /// door phase byte +0x76 to the three door sprites; inhabited draws the
    /// body in the owner palette, uninhabited adds the grey remaps; mode 3
    /// uninhabited collapses with the shared three jitters (bottom y + 98).
    /// A damage word continues to AffFlamme type 8 (40d5e2). Mode 4 inhabited
    /// first draws the unit entering (AffRentrerHangar 402da2).
    pub(crate) fn facade3b0(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        let maximum = self.u16(0x495d90 + 52 * u32::from(r[8]))?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade3b0 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 157), i16(y - 62));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let owner = u32::from(r[4]);
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 98 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            let phase = u32::from(r[0x76]);
            let doors = [
                ((0x1c7c + phase) & 0xffff, x, i16(y + 55)),
                ((0x1c84 + phase) & 0xffff, i16(x + 137), i16(y + 55)),
                ((0x1c8c + phase) & 0xffff, i16(x + 49), i16(y + 40)),
            ];
            if inhabited {
                if r[5] == 4 {
                    self.rentrer_hangar(index)?;
                }
                if owner > 4 {
                    return self.stop(0x45c705, "facade3b0 owner palette table outside players");
                }
                self.bob(x, y, 0x1c7b, true, Some(owner))?;
                for (id, px, py) in doors {
                    self.bob(px, py, id, true, None)?;
                }
            } else {
                self.bob(x, y, 0x1c7b, true, None)?;
                for (id, px, py) in doors {
                    self.bob(px, py, id, true, None)?;
                }
                for (id, px, py) in [(0x1c7b, x, y)].into_iter().chain(doors) {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 8);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 98 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1c7a, true, None);
        }
        let offset = (200 - health) * 98 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1c7a, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        let parts = [
            (0x1c7b, x, y),
            (0x1c7c, x, i16(y + 55)),
            (0x1c84, i16(x + 137), i16(y + 55)),
            (0x1c8c, i16(x + 49), i16(y + 40)),
        ];
        for (id, px, py) in parts {
            self.bob(px, py, id, true, None)?;
        }
        if !inhabited {
            for (id, px, py) in parts {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffBatiments terrain 0x343, intact 40a376..40abad: the type-5 workshop
    /// facade at (-122, -106). Base 1c09 (height 146), body 1c0a, wheel 1c2b at
    /// (+7, +28), smoke 1c0b at (+50, -48), door 1c35 + phase +0x76 at (+86, +48)
    /// and chimney 1c33 at (+39, +92). Inhabited modes 3..5: body in the owner
    /// palette, wheel (frame + cell) & 7; the smoke animates (frame + cell) & 31
    /// when the load byte +0x74 is set, otherwise only when LongRand() % 800 == 0
    /// (CRT rand, a published render effect); one permutation byte in twenty
    /// flickers the chimney by (frame + cell) & 1. Uninhabited adds the grey
    /// remaps; mode 3 uninhabited collapses with the three shared jitters
    /// (bottom y + 146). A damage word continues to AffFlamme type 5 (40d5e2).
    pub(crate) fn facade343(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        let maximum = self.u16(0x495d90 + 52 * u32::from(r[8]))?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade343 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 122), i16(y - 106));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let owner = u32::from(r[4]);
        let parts = |x: i32, y: i32, door: u32| -> [(u32, i32, i32); 5] {
            [
                (0x1c0a, x, y),
                (0x1c2b, i16(x + 7), i16(y + 28)),
                (0x1c0b, i16(x + 50), i16(y - 48)),
                (door, i16(x + 86), i16(y + 48)),
                (0x1c33, i16(x + 39), i16(y + 92)),
            ]
        };
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 146 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            let door = (0x1c35 + u32::from(r[0x76])) & 0xffff;
            if inhabited {
                if owner > 4 {
                    return self.stop(0x45c705, "facade343 owner palette table outside players");
                }
                self.bob(x, y, 0x1c0a, true, Some(owner))?;
                // [esp] after PastePlayerCmp's `ret 4` is the cell word (40a596).
                let frame = self.u16(0x508f80)?;
                let phase = frame.wrapping_add(cell);
                self.bob(i16(x + 7), i16(y + 28), 0x1c2b + (phase & 7), true, None)?;
                // 40a5cd: LongRand only when the load byte is clear; idiv by 800.
                let smoke = r[0x74] != 0 || self.crt_rand()? % 800 == 0;
                let id = if smoke { 0x1c0b + (phase & 31) } else { 0x1c0b };
                self.bob(i16(x + 50), i16(y - 48), id, true, None)?;
                self.bob(i16(x + 86), i16(y + 48), door, true, None)?;
                let cursor = self.u8(0x4e1d18)?;
                let flicker = self.u8(0x4e1c18 + cursor)? % 20 == 0;
                self.put("rng_u8", 0x4e1d18, &[(cursor as u8).wrapping_add(1)])?;
                let id = if flicker {
                    0x1c33 + (phase & 1)
                } else {
                    0x1c33
                };
                self.bob(i16(x + 39), i16(y + 92), id, true, None)?;
            } else {
                for (id, px, py) in parts(x, y, door) {
                    self.bob(px, py, id, true, None)?;
                }
                for (id, px, py) in parts(x, y, door) {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 5);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 146 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1c09, true, None);
        }
        let offset = (200 - health) * 146 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1c09, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        for (id, px, py) in parts(x, y, 0x1c35) {
            self.bob(px, py, id, true, None)?;
        }
        if !inhabited {
            for (id, px, py) in parts(x, y, 0x1c35) {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffBatiments terrain 0x326, intact 409d77..40a376: the type-13 facade at
    /// (-91, -99). Base 1bf7, body 1bf8, wheel 1bf9 at (+16, +32) and top 1c01
    /// at (+20, +97). Inhabited modes 3..5 draw the body plain, the wheel
    /// (frame + cell) & 7 and the top (frame + cell) & 7 in the owner palette;
    /// uninhabited adds the grey remaps; mode 3 uninhabited collapses with the
    /// three shared jitters (bottom y + 146). Construction clips the base by 146
    /// below half health and by 141 above (40a12d / 40a1b1, as the original).
    /// A damage word continues to AffFlamme type 13 (40d5e2). No CRT rand.
    pub(crate) fn facade326(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        let maximum = self.u16(0x495d90 + 52 * u32::from(r[8]))?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade326 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 91), i16(y - 99));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let owner = u32::from(r[4]);
        let parts = |x: i32, y: i32| -> [(u32, i32, i32); 3] {
            [
                (0x1bf8, x, y),
                (0x1bf9, i16(x + 16), i16(y + 32)),
                (0x1c01, i16(x + 20), i16(y + 97)),
            ]
        };
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 146 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                if owner > 4 {
                    return self.stop(0x45c705, "facade326 owner palette table outside players");
                }
                let phase = self.u16(0x508f80)?.wrapping_add(cell) & 7;
                self.bob(x, y, 0x1bf8, true, None)?;
                self.bob(i16(x + 16), i16(y + 32), 0x1bf9 + phase, true, None)?;
                self.bob(i16(x + 20), i16(y + 97), 0x1c01 + phase, true, Some(owner))?;
            } else {
                for (id, px, py) in parts(x, y) {
                    self.bob(px, py, id, true, None)?;
                }
                for (id, px, py) in parts(x, y) {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 13);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 146 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1bf7, true, None);
        }
        let offset = (200 - health) * 141 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1bf7, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        for (id, px, py) in parts(x, y) {
            self.bob(px, py, id, true, None)?;
        }
        if !inhabited {
            for (id, px, py) in parts(x, y) {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffUpgradeMonastere 402ee9..4031e4 (called only from the terrain0x3ce
    /// body): the monastery upgrade sub-building at the caller's (+10,-41).
    /// Progress state = 200 * signed +0x82 / type maximum. State200 draws body
    /// 1c97 (owner palette when inhabited) and roof1ca0, animated on
    /// (frame7c+cell)&7 only when inhabited; otherwise base1c96 clipped by the
    /// progress with a zero left edge, then body and roof. Uninhabited draws
    /// add the grey remaps. No RNG or world write.
    pub(crate) fn upgrade_monastere(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
    ) -> Result<()> {
        let r = 0x49a0c8 + 276 * (index & 0xffff);
        let kind = self.u8(r + 8)?;
        let maximum = self.u16(0x495d90 + 52 * kind)? as i32;
        if maximum == 0 {
            return self.stop(0x402f51, "AffUpgradeMonastere zero type maximum");
        }
        let progress = self.u16(r + 0x82)? as u16 as i16 as i32;
        let state = (progress * 200 / maximum) as i16 as i32;
        let inhabited = self.inhabited(index)?;
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        if state == 200 {
            if inhabited {
                let owner = self.u8(r + 4)?;
                if owner > 4 {
                    return self.stop(
                        0x45c705,
                        "AffUpgradeMonastere owner palette outside players",
                    );
                }
                self.bob(x, y, 0x1c97, true, Some(owner))?;
                let frame = self.u16(0x508f7c)?;
                self.bob(x, y, 0x1ca0 + ((frame + cell) & 7), true, None)?;
            } else {
                self.bob(x, y, 0x1c97, true, None)?;
                self.bob(x, y, 0x1ca0, true, None)?;
                for id in [0x1c97, 0x1ca0] {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, id as i32, 1]);
                    self.remap_sprite(x, y, id, 0x4e24c4)?;
                }
            }
            return Ok(());
        }
        if state < 100 {
            self.set_clip(
                0,
                i16(y + (100 - state) * 75 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1c96, true, None);
        }
        let offset = (200 - state) * 75 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1c96, true, None)?;
        self.set_clip(0, i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1c97, true, None)?;
        self.bob(x, y, 0x1ca0, true, None)?;
        if !inhabited {
            for id in [0x1c97, 0x1ca0] {
                self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, id as i32, 1]);
                self.remap_sprite(x, y, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffBatiments terrain0x3e1, intact40d1d2..40d89b: type7 facade.
    /// Base1ca8 (height98), body1ca9, parts1caa at (+3,+13) and1cac at
    /// (+6,+54). Inhabited modes3..5 draw two permutation bytes: one in
    /// twenty flickers1caa and one in five flickers1cac by (frame+owner)&1.
    pub(crate) fn facade3e1(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        // BatimentGetEtat 0x41a8e5 reads the maximum of the record's own type
        // (the facade body never tests the type itself).
        let maximum = self.structure_maximum(r)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "facade3e1 zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16 as i32;
        let (mut x, mut y) = (i16(x - 52), i16(y - 76));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        let owner = u32::from(r[4]);
        let parts = |x: i32, y: i32| -> [(u32, i32, i32); 3] {
            [
                (0x1ca9, x, y),
                (0x1caa, i16(x + 3), i16(y + 13)),
                (0x1cac, i16(x + 6), i16(y + 54)),
            ]
        };
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 98 + self.jitter()?).min(height - 1);
                y = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, y, width, bottom)?;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                if owner > 4 {
                    return self.stop(0x45c705, "facade3e1 owner palette table outside players");
                }
                self.bob(x, y, 0x1ca9, true, Some(owner))?;
                let frame = self.u16(0x508f80)?;
                let flicker = |c: &mut Self, modulo: u32| -> Result<bool> {
                    let cursor = c.u8(0x4e1d18)?;
                    let rem = c.u8(0x4e1c18 + cursor)? % modulo;
                    c.put("rng_u8", 0x4e1d18, &[(cursor as u8).wrapping_add(1)])?;
                    Ok(rem == 0)
                };
                // [esp] after PastePlayerCmp's `ret 4` is the cell word (40d40d/40d4a0).
                let blink = (frame + cell) & 1;
                let first = if flicker(self, 20)? {
                    0x1caa + blink
                } else {
                    0x1caa
                };
                self.bob(i16(x + 3), i16(y + 13), first, true, None)?;
                let second = if flicker(self, 5)? {
                    0x1cac + blink
                } else {
                    0x1cac
                };
                self.bob(i16(x + 6), i16(y + 54), second, true, None)?;
            } else {
                for (id, px, py) in parts(x, y) {
                    self.bob(px, py, id, true, None)?;
                }
                for (id, px, py) in parts(x, y) {
                    self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                    self.remap_sprite(px, py, id, 0x4e24c4)?;
                }
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 7);
            }
            return Ok(());
        }
        if health < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - health) * 98 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1ca8, true, None);
        }
        let offset = (200 - health) * 98 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1ca8, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        for (id, px, py) in parts(x, y) {
            self.bob(px, py, id, true, None)?;
        }
        if !inhabited {
            for (id, px, py) in parts(x, y) {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }
    /// AffBatiments terrain0x2cb, intact0x4080e8..0x408572. Modes 3..5 with a
    /// damage word continue to AffFlamme type 1 (40835a..40836a, shared 40d5e2)
    /// at the adjusted origin; the construction modes never read +6.
    pub fn type1_facade(&mut self, x: i32, y: i32, cell: u32, index: u32, r: &[u8]) -> Result<()> {
        // 408174: display states 3/4/5 draw the finished body, every other
        // state the construction one (40836f); BatimentGetEtat reads the
        // maximum of the record's own type.
        let maximum = self.structure_maximum(r)?;
        let state = 200 * w(r, 10) / maximum;
        let (mut x, mut y) = (i16(x - 94), i16(y - 154));
        let width = self.u16(0x508724)? as i32 * 32;
        let height = self.u16(0x508726)? as i32 * 32;
        let inhabited = self.inhabited(index)?;
        if matches!(r[5], 3..=5) {
            if r[5] == 3 && !inhabited {
                let bottom = i16(y + 188 + self.jitter()?).min(height - 1);
                let top = i16(y + 200 + self.jitter()? - w(r, 0x84) as i32).max(0);
                self.set_clip(0, top, width, bottom)?;
                y = top;
                x = i16(x + self.jitter()?);
            }
            if inhabited {
                self.bob(x, y, 0x1b76, true, Some(u32::from(r[4])))?;
                self.bob(
                    i16(x + 72),
                    i16(y + 4),
                    0x2b1d + (self.u32(0x508f7c)? & 15),
                    true,
                    Some(u32::from(r[4])),
                )?;
            } else {
                self.bob(x, y, 0x1b76, true, None)?;
                self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, 0x1b76, 1]);
                self.remap_sprite(x, y, 0x1b76, 0x4e24c4)?;
            }
            if w(r, 6) != 0 {
                return self.aff_flamme(x, y, cell, index, 1);
            }
            return Ok(());
        }
        if state < 100 {
            self.set_clip(
                x.max(0),
                i16(y + (100 - state as i32) * 188 / 100).max(0),
                width - 1,
                height - 1,
            )?;
            return self.bob(x, y, 0x1b75, true, None);
        }
        let offset = (200 - state as i32) * 188 / 100;
        self.set_clip(0, 0, width - 1, i16(y + offset).min(height - 1))?;
        self.bob(x, y, 0x1b75, true, None)?;
        self.set_clip(x.max(0), i16(y + offset).max(0), width - 1, height - 1)?;
        self.bob(x, y, 0x1b76, true, None)?;
        if !inhabited {
            self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, 0x1b76, 1]);
            self.remap_sprite(x, y, 0x1b76, 0x4e24c4)?;
        }
        Ok(())
    }
    pub fn construction_facade(
        &mut self,
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        let mode = r[5];
        // The terrain word selects the body (0x2ed: 408606.., 0x30d: 4093c5..);
        // the record's type only enters through BatimentGetEtat and
        // BatimentEstHabiter. `kind` names the body, not the record type.
        let tile = self.u16(self.u32(0x507d70)? + cell * 2)?;
        let kind: u8 = match tile {
            0x2ed => 2,
            0x30d => 3,
            _ => return self.stop(0x40f23b, "construction facade on another terrain word"),
        };
        // Types 2/3 (408606..408ac3, 4093c5..4099a1): the construction modes never
        // read +6; modes 3..5 continue to AffFlamme type 2/3 after the body (below).
        let inhabited = self.inhabited(index)?;
        if inhabited {
            // 4093e9: only display states 3/4/5 reach the inhabited cave body;
            // the construction branch (4099a6) never tests inhabitation.
            if kind == 3 && matches!(mode, 3..=5) {
                return self.inhabited_cave(r, x, y, cell, index);
            }
            // 408ac8..408d6d: an inhabited type-2 structure in a construction mode
            // draws the same body and skips only the grey pass.
            if matches!(mode, 3..=5) {
                return self.stop(0x408ac8, "facade outside captured uninhabited domain");
            }
        }
        let maximum = self.structure_maximum(r)?;
        let (height, scaffold, dx, dy) = if kind == 2 {
            (153, 7031, 150, 93)
        } else {
            (97, 7093, 57, 47)
        };
        let mut x = i16(x - dx);
        let mut y = i16(y - dy);
        let width = self.u16(0x508724)? as i32 * 32;
        let height_window = self.u16(0x508726)? as i32 * 32;
        let state = (200 * w(r, 10) / maximum) as i32;
        // Every display state other than 3/4/5 takes the construction branch
        // (408613 / 4093f6: cmp 5, 3, 4 then jne).
        if !matches!(mode, 3..=5) {
            if state < 100 {
                self.set_clip(
                    0.max(x),
                    0.max(i16(y + (100 - state) * height / 100)),
                    width - 1,
                    height_window - 1,
                )?;
                self.bob(x, y, scaffold, true, None)?;
                return Ok(());
            }
            let offset = (200 - state) * height / 100;
            self.set_clip(0, 0, width - 1, i16(y + offset).min(height_window - 1))?;
            self.bob(x, y, scaffold, true, None)?;
            self.set_clip(
                0.max(x),
                0.max(i16(y + offset)),
                width - 1,
                height_window - 1,
            )?;
        } else if mode == 3 {
            let bottom = i16(y + height + self.jitter()?).min(height_window - 1);
            y = 0.max(i16(y + 200 + self.jitter()? - w(r, 0x84) as i32));
            self.set_clip(0, y, width, bottom)?;
            x = i16(x + self.jitter()?);
        }
        let parts: &[(u32, i32, i32)] = if kind == 2 {
            &[
                (7032, 0, 0),
                (7049, 74, 5),
                (7033, 89, 95),
                (7041, 151, 131),
                (7057, 18, 63),
            ]
        } else {
            &[
                (7094, 0, 0),
                (7095, 3, 10),
                (7103, 43, 0),
                (7111, 60, 23),
                (7119, 64, 41),
                (7127, 12, 46),
                (7135, 21, 16),
            ]
        };
        // 0x409c08: the CAVE construction body (any state outside 3/4/5, health
        // at or above half the maximum) draws 0x1bcf + byte +0x76 at (64, 41); the
        // uninhabited mode-3..5 body (0x40984c) and the grey pass keep 0x1bcf.
        let variant = if kind == 3 && !matches!(mode, 3..=5) {
            u32::from(r[0x76])
        } else {
            0
        };
        for &(id, ox, oy) in parts {
            let id = if id == 7119 { id + variant } else { id };
            self.bob(i16(x + ox), i16(y + oy), id, true, None)?;
        }
        for &(id, ox, oy) in parts {
            if inhabited {
                break;
            }
            let (xx, yy) = (
                i16(x + ox),
                i16(y + if kind == 3 && id == 7119 { 38 } else { oy }),
            );
            self.observe("PasteGrisCmp", 0x45bcb0, &[xx, yy, id as i32, 1]);
            self.remap_sprite(xx, yy, id, 0x4e24c4)?;
        }
        // 408ab3..408ac3 / 409991..4099a1: push 2/3, shared AffFlamme tail at the
        // adjusted origin; the collapse clip and jitters remain in force.
        if matches!(mode, 3..=5) && w(r, 6) != 0 {
            self.aff_flamme(x, y, cell, index, kind)?;
        }
        Ok(())
    }
    /// BatimentGetEtat 0x41a8e5: the maximum health of the record's own type
    /// (0x495d90 + 52 * type); a zero maximum is the native idiv fault.
    pub(crate) fn structure_maximum(&mut self, r: &[u8]) -> Result<u32> {
        let maximum = self.u16(0x495d90 + 52 * u32::from(r[8]))?;
        if maximum == 0 {
            return self.stop(0x41a93b, "BatimentGetEtat zero maximum health");
        }
        Ok(maximum)
    }
    fn jitter(&mut self) -> Result<i32> {
        let cursor = self.u8(0x4e1d18)?;
        let value = self.u8(0x4e1c18 + cursor)? % 3;
        self.put("rng_u8", 0x4e1d18, &[(cursor as u8).wrapping_add(1)])?;
        Ok(value as i32)
    }
    pub fn order_markers(&mut self, r: &[u8]) -> Result<()> {
        if r[0xbd] == 0 || r[0x82] == 0 || r[2] == 17 {
            return Ok(());
        }
        // 412923: target mode 1 draws both cell markers (4127bc), mode 2 the
        // order cell and the depot structure (412637), modes >= 3 nothing
        // (41295d -> 41261f). The marker body never reads the unit kind.
        if r[0x82] > 2 {
            return Ok(());
        }
        let ox = self.u16(0x508720)?;
        let oy = self.u16(0x508722)?;
        let width = self.u16(0x508724)?;
        let height = self.u16(0x508726)?;
        let phase = (self.u32(0x508f7c)? / 5) & 1;
        let mut second = w(r, 0x80);
        if r[0x82] == 2 {
            // 4126e1/412702: BatimentGetCase(+0x80) twice, AX only (EDX is
            // restored to zero by the callee epilogue): the collision-mask
            // centre of any structure type. An index past the 1,000-entry
            // table reads outside it in the original; it stays a stop.
            if second >= 1000 {
                return self.stop(0x412637, "depot marker structure index outside the table");
            }
            second = crate::frame_core::building_cell(self.m, second)? & 65535;
        }
        for (cell, base) in [(w(r, 0xb2), 0x1e43), (second, 0x1e45)] {
            let (cx, cy) = (cell % 256, cell / 256);
            if cx >= ox && cx < ox + width && cy >= oy && cy < oy + height {
                self.bob(
                    (cx - ox) as i32 * 32,
                    (cy - oy) as i32 * 32,
                    base + phase,
                    true,
                    None,
                )?;
            }
        }
        Ok(())
    }
    pub fn construction_builder(&mut self, r: &[u8], x: i32, y: i32, index: u32) -> Result<()> {
        // AffConstruireBatiment 0x414495 is routed by state alone: the selection
        // marker (41450a, not owner-gated), then a body for kinds 6 and 7 only
        // (414893), then the shared info/fatigue tail.
        if r[0] > 4 && r[0] != 200 || self.camera != 0 {
            return self.stop(
                0x414495,
                "builder construction body outside the admitted owner/marker domain",
            );
        }
        let (px, py) = (x + r[0x86] as i8 as i32, y + r[0x87] as i8 as i32);
        if r[0xbd] != 0 {
            // 44106c (table 441004): large marker for kinds 3, 11..16, 25.
            if matches!(r[1], 3 | 11..=16 | 25) {
                self.bob(i16(px - 32), i16(py - 32), 0x2a1f, true, None)?;
            } else {
                self.bob(i16(px), i16(py), 0x2a1e, true, None)?;
            }
            // 0x414581: a kind-7 builder of a wall (0xbe = 9) shows its queued
            // cells at the viewport, sprite 0x1ccf.
            if r[1] == 7 && r[0xbe] == 9 && w(r, 0xd8) != 0 {
                let (ox, oy) = (self.u16(0x508720)?, self.u16(0x508722)?);
                let (vw, vh) = (self.u16(0x508724)?, self.u16(0x508726)?);
                let pool = self.u32(0x4e19a0)?;
                for i in 0..w(r, 0xd8) {
                    let cell = self.u16(pool.wrapping_add(276 * index + 0xc4 + 2 * i))?;
                    let (cx, cy) = (cell % 256, cell / 256);
                    if cx < ox || cx >= ox + vw || cy < oy || cy >= oy + vh {
                        continue;
                    }
                    self.bob(
                        i16((cx - ox) as i32 * 32),
                        i16((cy - oy) as i32 * 32),
                        0x1ccf,
                        true,
                        None,
                    )?;
                }
            }
        }
        // 41487b / 41476b: phase 0 is the carrying or empty body, phase 1 the work
        // body; other phases draw no body and continue to the tail (41477e).
        if !matches!(r[1], 6 | 7) {
            // 0x414893 skips 41477e, which sets the tail's coordinates: the
            // kinds without a body draw it from their zeroed locals (0, 0).
            let (dx, dy) = (r[0x86] as i8 as i32, r[0x87] as i8 as i32);
            return self.unit_tail(r, -dx, 9 - dy, index, true);
        }
        if r[0x89] <= 1 {
            if r[0xa4] >= 8 || !(1..=16).contains(&r[0x91]) {
                return self.stop(0x414495, "repair body outside the captured frame domain");
            }
            let turn = (r[0x91] as u32).div_ceil(2);
            let id = match (r[1], r[0x89], w(r, 10) == 0) {
                (7, 0, true) => 0x53a + turn * 8,
                (7, 0, false) => 0x5ba + turn * 8,
                (7, _, _) => 0x57a + turn * 8,
                (_, 0, true) => 0xb9a + turn * 8,
                (_, 0, false) => 0xbda + turn * 8,
                (_, _, _) => 0xc5e + turn * 4,
            } + r[0xa4] as u32;
            // AffCorpsEnginDecalage 0x410fcc: kind 6 (16, 20), kind 7 (26, 26).
            let (ox, oy) = if r[1] == 6 { (16, 20) } else { (26, 26) };
            self.shadow(i16(px + 2), i16(py + 26), 10784, false)?;
            self.corps_unit(i16(px - ox), i16(py - oy), id, r)?;
        }
        self.unit_tail(r, x, y, index, true)
    }
    pub fn restore_background(&mut self) -> Result<()> {
        let x = i16(self.u16(0x49a046)? as i32);
        let y = i16(self.u16(0x49a048)? as i32);
        let width = self.u16(0x49a04a)? as usize;
        let height = self.u16(0x49a04c)? as usize;
        let ptr = self.u32(0x49a03c)?;
        if width * height > 480000 {
            return self.stop(
                0x4179e1,
                "saved-background geometry outside captured screen domain",
            );
        }
        let data = self.read(ptr, width * height)?;
        self.observe(
            "PasteBob",
            0x45adf3,
            &[x, y, width as i32, height as i32, ptr as i32],
        );
        for row in 0..height {
            let yy = y + row as i32;
            if !(0..600).contains(&yy) {
                continue;
            }
            let left = 0.max(x);
            let right = 800.min(x + width as i32);
            if right > left {
                let source = row * width + (left - x) as usize;
                let dest = yy as usize * 800 + left as usize;
                self.surface[dest..dest + (right - left) as usize]
                    .copy_from_slice(&data[source..source + (right - left) as usize]);
            }
        }
        self.put("restaurar_fundo_u8", 0x49a041, &[0])?;
        self.put("forcar_desenho_u8", 0x4e05a2, &[1])?;
        let old = self.u8(0x49a040)?;
        if old != 0 {
            self.put("fond_u8", 0x49a040, &[0])?;
        }
        Ok(())
    }
    /// Returns the word 0x41721f leaves at AffGame's esp - 52: the y0 it
    /// stages there for the outline call (0x417305), then the x1 of each free
    /// footprint cell it outlines (0x417413). AffChrono reads it back.
    pub fn placement(&mut self) -> Result<u16> {
        let kind = self.u8(0x496207)?;
        let width = self.u16(0x495d8c + 52 * kind)? as i32;
        let height = self.u16(0x495d8e + 52 * kind)? as i32;
        let x = (self.u16(0x5099a8)? as i32 / 32 - width / 2) * 32;
        let y = (self.u16(0x5099aa)? as i32 / 32 - height / 2) * 32;
        self.outline(
            i16(x),
            i16(y),
            i16(x + width * 32),
            i16(y + height * 32),
            31,
            true,
        )?;
        self.shaded_box(
            i16(x),
            i16(y),
            i16(x + width * 32),
            i16(y + height * 32),
            false,
            true,
        )?;
        let mut slot = y as u16;
        for row in 0..height {
            for col in 0..width {
                if self.u8(0x4e05b9 + (row * width + col) as u32)? == 0 {
                    slot = (x + (col + 1) * 32) as u16;
                    self.outline(
                        i16(x + col * 32),
                        i16(y + row * 32),
                        i16(x + (col + 1) * 32),
                        i16(y + (row + 1) * 32),
                        148,
                        true,
                    )?;
                }
            }
        }
        Ok(slot)
    }
    pub(crate) fn shaded_box(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        shadow: bool,
        clipped: bool,
    ) -> Result<()> {
        self.observe(
            if shadow {
                "FilledBoxOmbre"
            } else {
                "FilledBoxGris"
            },
            if shadow { 0x45cc35 } else { 0x45cd91 },
            &[x0, y0, x1, y1, clipped as i32],
        );
        let [l, t, r, b] = if clipped {
            self.clip
        } else {
            [
                0,
                0,
                self.u16(0x49a018)? as i32 - 1,
                self.u16(0x49a01a)? as i32 - 1,
            ]
        };
        if x0 > r || y0 > b || x1 < l || y1 < t {
            return Ok(());
        }
        let (x0, y0, x1, y1) = (
            x0.max(l).max(0),
            y0.max(t).max(0),
            x1.min(r).min(799),
            y1.min(b).min(599),
        );
        let table = self
            .m
            .read(self.u32(if shadow { 0x4e24b4 } else { 0x4e24c4 })?, 256)?;
        if x1 < x0 || y1 < y0 {
            return Ok(());
        }
        for y in y0..=y1 {
            for x in x0..=x1 {
                let p = y as usize * 800 + x as usize;
                self.surface[p] = table[self.surface[p] as usize];
            }
        }
        Ok(())
    }
    pub fn tooltip(&mut self) -> Result<Option<(usize, u32)>> {
        let ptr = self.u32(0x4e060c)?;
        if ptr == 0 {
            return Ok(None);
        }
        if ptr < 0x10000 {
            return self.stop(0x40f8c0, "tooltip pointer outside declared address domain");
        }
        if self.u16(0x4e0610)? < self.u16(0x49620a)? {
            return Ok(None);
        }
        // 40f8b1: AffInfoAide receives the mouse words as they stand (a drag
        // corner 3 px past the top or left edge is 0xfffd); 40f9d9/40f9dc add
        // 24 and 16 as 16-bit words and AffTextEncadre 45a315/45a322 sign-
        // extends them, clamping negatives to 0 and the far edges below.
        let mx = self.u16(0x5099a8)? as u16;
        let my = self.u16(0x5099aa)? as u16;
        let mut text = Vec::new();
        for i in 0..128 {
            let b = self.u8(ptr + i)? as u8;
            if b == 0 {
                break;
            }
            text.push(b);
            if b == 10 {
                break;
            }
        }
        if text.len() == 128 {
            return self.stop(0x40f8c0, "tooltip string exceeds captured bounded length");
        }
        let number = self.u16(0x49620c)?;
        if number != 65535 {
            if text.last() == Some(&10) {
                text.pop();
            }
            text.push(b':');
            text.extend_from_slice(number.to_string().as_bytes());
        }
        if text.len() > 39 {
            return self.stop(0x40f8f6, "tooltip exceeds original local text buffer");
        }
        if self.u32(0x4e1d94)? != 0 || self.u32(0x4e1d98)? != 0 {
            return self.stop(0x458ffd, "tooltip external/multibyte metrics");
        }
        let line = text.split(|&c| c == 0 || c == 10).next().unwrap();
        if line.iter().any(|&c| c < 32) {
            return self.stop(0x458fe9, "tooltip control byte metrics");
        }
        self.put("restaurar_fundo_u8", 0x49a041, &[1])?;
        let mut width = 2;
        for &b in line {
            let glyph = if b == b' ' { b'l' } else { b };
            width += self.assets.sprite(0x23d5 + glyph as u32 - 32)?.width as i32;
        }
        let mut height = self.assets.sprite(0x23d5 + 0x6a - 32)?.height as i32;
        let mut x = 0.max(i32::from(mx.wrapping_add(24) as i16));
        let lifted = if self.u16(0x49a018)? == 640 && u32::from(my) >= self.u16(0x508726)? * 32 {
            my.wrapping_add(120)
        } else {
            my
        };
        let mut y = 0.max(i32::from(lifted.wrapping_add(16) as i16));
        if y + height + 6 > 592 {
            y = 591 - height - 6;
        }
        if x + width + 6 > 800 {
            x = 800 - width - 6;
        }
        width += 6 - i32::from(width % 2 == 0);
        height += 6;
        let old_width = self.u16(0x49a018)?;
        let old_height = self.u16(0x49a01a)?;
        self.put16("largura_tela_u16", 0x49a018, 800)?;
        self.put16("altura_tela_u16", 0x49a01a, 600)?;
        for (name, a, value) in [
            ("tooltip_x", 0x49a046, x),
            ("tooltip_y", 0x49a048, y),
            ("tooltip_w", 0x49a04a, width),
            ("tooltip_h", 0x49a04c, height),
        ] {
            self.put16(name, a, value as u32)?;
        }
        if x < 0 || y < 0 || width < 0 || height < 0 || x + width > 800 || y + height > 600 {
            return self.stop(0x40f8c0, "tooltip save outside captured screen domain");
        }
        let mut saved = Vec::with_capacity((width * height) as usize);
        for row in 0..height {
            let p = (y + row) as usize * 800 + x as usize;
            saved.extend_from_slice(&self.surface[p..p + width as usize]);
        }
        let address = self.u32(0x49a03c)?;
        self.observe("GetBob", 0x45aec2, &[x, y, width, height, address as i32]);
        self.observe("saved_background_pixels", address, &[]);
        if self.audit {
            let event = self.observations.last_mut().unwrap();
            event.kind = 3;
            event.count = saved.len() as u32;
        }
        *self.background = Some(SavedBackground {
            address,
            pixels: saved,
        });
        self.outline(x, y, x + width - 1, y + height - 1, 1, false)?;
        self.shaded_box(x + 1, y + 1, x + width - 2, y + height - 2, true, false)?;
        let rectangle = [x + 3, y + 1, x + width - 2, y + height - 2];
        self.observe(
            "AffText",
            0x459454,
            &[
                rectangle[0],
                rectangle[1],
                rectangle[2],
                rectangle[3],
                1,
                0,
                0,
                text.len() as i32,
            ],
        );
        if self.audit {
            let event = self.observations.last_mut().unwrap();
            event.kind = 4;
            let n = text.len().min(8);
            event.before[..n].copy_from_slice(&text[..n]);
            if text.len() > 8 {
                let end = text.len().min(16);
                event.after[..end - 8].copy_from_slice(&text[8..end]);
            }
        }
        if let Some(boundaries) = self.text_boundaries.as_mut() {
            let expected = boundaries
                .pop_front()
                .ok_or(Error::Invalid("Unrecorded AffText probe boundary"))?;
            if expected.text != text
                || expected.rectangle != rectangle
                || expected.font != 1
                || expected.flags != [0, 0]
                || expected.surface_sha256 != crate::render_assets::sha256(self.surface)
            {
                return Err(Error::Invalid(
                    "AffText probe request/pre-text surface mismatch",
                ));
            }
            self.text_checked += 1;
        } else {
            self.afftext(&text, rectangle, 1, [0, 0])?;
        }
        self.put16("largura_tela_u16", 0x49a018, old_width)?;
        self.put16("altura_tela_u16", 0x49a01a, old_height)?;
        let length = text.split(|&c| c == 0 || c == 10).next().unwrap().len();
        Ok(Some((
            length,
            text.get(length).copied().unwrap_or(0) as u32,
        )))
    }
}
