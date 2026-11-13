//! Graphics 1.8/1.9 layer plans, ported from committed original-first tables.
use crate::render::{i16, w, Context, Result};
impl Context<'_> {
    pub fn inhabited_cave(
        &mut self,
        r: &[u8],
        x: i32,
        y: i32,
        cell: u32,
        index: u32,
    ) -> Result<()> {
        // Owners0..4 index the player palette (PastePlayerCmp) and the level
        // table; the camera plays no part in the body. Native-proven on the
        // MONDE03 (owner0) and MONDE17 consumer-path (owner1) worlds. Modes 3
        // and 4 share the mode-5 body (4093e9..40951c: the collapse jitters run
        // only for an uninhabited CAVE); construction modes keep the stop.
        if r[4] > 4 || !matches!(r[5], 3..=5) {
            return self.stop(
                0x40958e,
                "inhabited CAVE outside owners0..4 modes3..5 domain",
            );
        }
        // The stock indicator 0x4dfb30 is not clamped natively (more stock than
        // the caves hold after one is lost): it only selects the sprite id.
        let level = self.u8(0x4dfb30 + 6 * r[4] as u32)?;
        // Any +0x74/+0x75/+0x76: +0x74 is only tested against zero (0x409650),
        // +0x76 only offsets the sprite id (0x4096e6: 0x1bcf + v below 8, else
        // 0x1be1 + v, inside the catalog for every byte), and the door frame
        // needs +0x76 == 15 with +0x75 != 0 (TuyauEstSortie 0x420c7c).
        let (x, y) = (i16(x - 57), i16(y - 47));
        let clock = self.u32(0x508f80)?;
        let phase = (clock.wrapping_add(cell)) & 7;
        self.bob(x, y, 7094, true, Some(r[4] as u32))?;
        for (id, dx, dy) in [
            (7095 + phase, 3, 10),
            (7103 + phase, 43, 0),
            (7111 + phase, 60, 23),
            (7127 + if r[0x74] != 0 { phase } else { 0 }, 12, 46),
            (7135 + level, 21, 16),
            (
                if r[0x76] < 8 { 7119 } else { 7137 } + r[0x76] as u32,
                64,
                if r[0x76] < 8 { 41 } else { 38 },
            ),
        ] {
            self.bob(i16(x + dx), i16(y + dy), id, true, None)?;
        }
        if r[0x76] == 15 && r[0x75] != 0 {
            self.bob(i16(x + 64), i16(y + 38), 7151 + phase, true, None)?;
        }
        if w(r, 6) != 0 {
            self.damaged_structure_fire(x, y, cell, index)?;
        }
        Ok(())
    }
    /// AffWorkEngin 0x414b31: EnginGetTaille 0x44106c (table 441004), the
    /// large marker 0x2a1f at (-32, -32) for kinds 3, 11..16, 25, else the
    /// small 0x2a1e in place.
    fn selection_marker(&mut self, r: &[u8], x: i32, y: i32) -> Result<()> {
        let (px, py) = (x + r[0x86] as i8 as i32, y + r[0x87] as i8 as i32);
        if matches!(r[1], 3 | 11..=16 | 25) {
            self.bob(i16(px - 32), i16(py - 32), 0x2a1f, true, None)
        } else {
            self.bob(i16(px), i16(py), 0x2a1e, true, None)
        }
    }
    /// AffWorkEngin 0x414b04: the selection and order markers, only for the
    /// local player's selected units.
    fn work_selection(&mut self, r: &[u8], x: i32, y: i32) -> Result<()> {
        if r[0xbd] != 0 && r[0] as u32 == self.camera {
            self.selection_marker(r, x, y)?;
            self.order_markers(r)?;
        }
        Ok(())
    }
    pub fn work_body(&mut self, r: &[u8], x: i32, y: i32, index: u32, overlay: bool) -> Result<()> {
        let (kind, task) = (r[1], r[0x89]);
        if !matches!(kind, 5 | 6 | 7 | 13) {
            // 414d33: kinds other than 5/6/7/13 have no work body. 414b04 drew
            // the local player's selection and order markers; 4152c9 goes to
            // the shared info/fatigue tail with the prelude's coordinates,
            // still (0, 0) (+0x24 = BX ^ BX, +0x20 = 0), as AffAttaqueEngin's
            // body-less kinds do. No count or direction is read.
            self.work_selection(r, x, y)?;
            let info = overlay
                && ((r[0] as u32 == self.camera && r[0xbd] != 0)
                    || self.u16(0x496ac2)? == index
                    || self.u8(0x508f8f)? != 0);
            return self.tail_at(r, 0, 0, index, overlay, info);
        }
        // AffWorkEngin414a8a: the body goes through AffCorpsEngin411147, whose
        // palette dispatch covers owners0..4 and the neutral owner200.
        // AffWorkEngin never reads the +0x99 jitter (no permutation read).
        if r[0] > 4 && r[0] != 200
            || self.camera != 0
            || kind != 13 && (r[0xa5] != 8 || r[0xa4] >= 8)
            || !(1..=16).contains(&r[0x91])
        {
            return self.stop(
                0x414a8a,
                "worker outside captured owner/count/effect bounds",
            );
        }
        if kind == 7 && matches!(task, 1 | 2) {
            // 4151c9: tasks 1/2 clear the tail flag after 414b04 drew the
            // local player's selection and order markers (this route comes
            // straight from AffEngin, before AffDeplaceEngin's overlay block).
            return self.work_selection(r, x, y);
        }
        // Native4152c1/415089 (farmer) and4151a0 (builder): unlisted tasks
        // draw no body and continue to the shared info/fatigue tail at414f7c.
        let body = if kind == 5 {
            match task {
                0 | 2 | 3 => Some((if w(r, 10) == 0 { 1594 } else { 1690 }, 8, 11, 11)),
                1 => Some((1662, 4, 11, 11)),
                _ => None,
            }
        } else if kind == 7 {
            match task {
                0 | 3..=5 => Some((if w(r, 10) == 0 { 1338 } else { 1466 }, 8, 26, 26)),
                // 415168: builder task12 body0x57a without the bag test.
                12 => Some((0x57a, 8, 26, 26)),
                _ => None,
            }
        } else if kind == 6 {
            // 414b04: the selection marker comes first; kind 6 reaches here
            // straight from AffEngin's state dispatch, not through
            // AffDeplaceEngin's overlay block as the camera's workers do.
            if r[0xbd] != 0 && r[0] as u32 == self.camera {
                self.selection_marker(r, x, y)?;
            }
            // 4152a8 (mechanic): tasks 0/1 -> 4151d4, the body 0xb9a with an
            // empty bag (BagVide +0x0a), else 0xbda; task 2 -> 415289, always
            // 0xbda; task 3 -> 41529d clears the tail flag (+0x3c): no body
            // and no tail; other tasks draw no body and keep the tail. The
            // body sits at the AffCorpsEnginDecalage offsets (-16, -20).
            match task {
                0 | 1 => Some((if w(r, 10) == 0 { 0xb9a } else { 0xbda }, 8, 16, 20)),
                2 => Some((0xbda, 8, 16, 20)),
                3 => {
                    if r[0] as u32 == self.camera {
                        self.order_markers(r)?;
                    }
                    return Ok(());
                }
                _ => None,
            }
        } else if kind == 13 {
            // 414f5a: tasks0/2/3/4 -> 414d98 (shadow bank0x1112, body0x10d2),
            // task1 -> 414e8e (0x1132 / 0x10f2); the frame is the kind-13
            // movement frame (dir/2)*count + (dir-1)/2 + phase*(dir%2), drawn at
            // the AffCorpsEnginDecalage offsets (-55,-56); other tasks draw none.
            let dir = i32::from(r[0x91]);
            let frame = ((dir / 2) * i32::from(r[0xa5])
                + (dir - 1) / 2
                + i32::from(r[0xa4]) * (dir % 2)) as u32;
            let banks = match task {
                0 | 2..=4 => Some((0x1112u32, 0x10d2u32)),
                1 => Some((0x1132, 0x10f2)),
                _ => None,
            };
            if r[0] as u32 == self.camera {
                self.order_markers(r)?;
            }
            if let Some((shadow, body)) = banks {
                let (px, py) = (x + r[0x86] as i8 as i32 - 55, y + r[0x87] as i8 as i32 - 56);
                self.shadow(i16(px), i16(py), shadow.wrapping_add(frame) & 0xffff, false)?;
                self.corps_unit(i16(px), i16(py), body.wrapping_add(frame) & 0xffff, r)?;
            }
            return self.unit_tail(r, x, y, index, overlay);
        } else {
            return self.stop(0x414a8a, "uncaptured worker kind");
        };
        // Native414b04: selection and order markers only for the camera's units.
        if r[0] as u32 == self.camera {
            self.order_markers(r)?;
        }
        if let Some((base, stride, ax, ay)) = body {
            let (px, py) = (x + r[0x86] as i8 as i32, y + r[0x87] as i8 as i32);
            self.shadow(i16(px + 2), i16(py + 26), 10784, false)?;
            self.corps_unit(
                i16(px - ax),
                i16(py - ay),
                base + (r[0x91] as u32).div_ceil(2) * stride + r[0xa4] as u32,
                r,
            )?;
        }
        self.unit_tail(r, x, y, index, overlay)
    }
}
impl Context<'_> {
    pub fn structure_overlay18(
        &mut self,
        r: &[u8],
        cell: u32,
        origin: (i32, i32),
        frame: bool,
        gauge: bool,
    ) -> Result<()> {
        // BatimentPoseCadre 0x4022c8 and AffJaugeBatiment 0x4020ec never read
        // the display state or the camera; the owner indexes the player
        // palette (0..4, the caller stops outside the selection words).
        if !matches!(r[8], 2 | 3) || r[4] > 4 {
            return self.stop(
                if frame { 0x4022c8 } else { 0x4020ec },
                "structure selection outside types 2/3, owners 0..4",
            );
        }
        let cave = r[8] == 3;
        let (right, height, gy, max, cap) = if cave {
            (83, 97, 34, 100, 2)
        } else {
            (175, 153, 90, 600, 16)
        };
        let (ax, ay) = origin;
        let ox = self.u16(0x508720)?;
        let oy = self.u16(0x508722)?;
        let (x, y) = (
            (cell as i32 % 256 - ox as i32) * 32 - ax,
            (cell as i32 / 256 - oy as i32) * 32 - ay,
        );
        if frame {
            let phase = (self.u32(0x508f80)? & 7) as i32;
            for (id, px, py) in [
                (0x2a79, x + phase - 20, y + phase),
                (0x2a78, x + phase - 20, y + height - phase),
                (0x2a7a, x + right - phase, y + phase),
                (0x2a77, x + right - phase, y + height - phase),
            ] {
                self.bob(i16(px), i16(py), id, true, Some(r[4] as u32))?;
            }
        }
        if !gauge {
            return Ok(());
        }
        let _ = (max, cap);
        // AffJaugeBatiment 0x4021bf..0x402257 for any health/damage: the bar
        // height is (BatimentGetEtat - BatimentGetDegat) * 60 / 200 with
        // colours 145 / 180 / 48 (16-bit compare), as for every other type;
        // the slots follow the bag capacity (401aff iterates every slot).
        let mut gx = i16(x - 23);
        let gy = i16(y + gy);
        if ox == 0 && gx < 0 {
            gx = 0;
        }
        self.bob(i16(gx + 9), gy, 0x2a7b, true, None)?;
        let maximum = self.u16(0x495d90 + 52 * r[8] as u32)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "structure gauge zero maximum health");
        }
        let health = (200 * w(r, 10) / maximum) as u16;
        let damage = (200 * w(r, 6) / maximum) as u16;
        let height = (i32::from(health) - i32::from(damage)) * 60 / 200;
        let colour = if height as u16 <= 20 {
            145
        } else if height as u16 <= 40 {
            180
        } else {
            48
        };
        self.filled(
            i16(gx + 10),
            i16(gy + 61 - height),
            i16(gx + 14),
            i16(gy + 60),
            colour,
        )?;
        let (bx, by) = self.bag_slots(r, gx, gy)?;
        if cave {
            // 0x401b89/0x401b95: the row starts 0x3f right of the x and 0x20
            // below the y the bag loop leaves, so it follows the record's own
            // bag capacity (2 for a real CAVE: gx + 63, gy + 48).
            // Level is not clamped natively (stock above the caves' capacity):
            // above 7 all seven slots fill (0x401bf2 compares level to the slot).
            let level = self.u8(0x4dfb30 + 6 * r[4] as u32)?;
            // Native401bdd..401c14 tests each of the seven slots against level.
            for i in 0..7 {
                let active = (i as u32) < level;
                self.bob(
                    i16(bx + 63 + 8 * i),
                    i16(by + 32),
                    if active { 0x2a3d } else { 0x2a3b },
                    true,
                    active.then_some(r[4] as u32),
                )?;
            }
        }
        Ok(())
    }
}
