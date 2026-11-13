//! AffGame HUD tail through the last offscreen pixel write (0x417d30).
use crate::render::{i16, Context, Result};
impl Context<'_> {
    /// MessageBoxAffiche0x469c44: one native notification, two text rows.
    pub(crate) fn message_box(&mut self) -> Result<()> {
        let head = self.u16(0x507d54)?;
        if head == self.u16(0x507d56)? {
            return Ok(());
        }
        if head >= 256 {
            return self.stop(0x469c44, "MessageBoxAffiche queue index");
        }
        let record = 0x4e7754 + 518 * head;
        if self.u16(record + 0x104)? == 399 {
            return Ok(());
        }
        let margin = i32::from(self.u8(0x508f90)? as i8);
        let width = self.u16(0x508724)? as i32;
        let height = self.u16(0x508726)? as i32;
        let y = i16((height - margin) * 32 - 48);
        let mut writes = Vec::new();
        let result = crate::frame_core::init_zone_plan(self.m, 1, &mut writes);
        for (address, bytes) in writes {
            self.put("message_mouse_zone", address, &bytes)?;
        }
        result?;
        if self.u8(record + 0x106)? != 0 {
            self.bob(
                i16(margin * 32),
                i16((height - 2 - margin) * 32),
                0x2f3d + (self.u16(0x508f80)? & 31),
                true,
                None,
            )?;
        }
        let x = i16(margin * 32);
        let right = i16((width - 1 - margin) * 32);
        let second = self.u16(record + 0x102)?;
        for (offset, top, bottom) in [(0, y, i16(y + 16)), (second, i16(y + 16), i16(y + 32))] {
            if offset == 65535 {
                continue;
            }
            if offset >= 250 {
                return self.stop(
                    0x469d7f,
                    "MessageBoxAffiche text split outside profile buffer",
                );
            }
            let bytes = self.read(record + 4 + offset, (250 - offset) as usize)?;
            let Some(end) = bytes.iter().position(|&b| b == 0) else {
                return self.stop(0x469d61, "MessageBoxAffiche unterminated profile text");
            };
            self.afftext(&bytes[..end], [x, top, right, bottom], 1, [0, 1])?;
        }
        Ok(())
    }
    pub fn hud(&mut self, event: bool) -> Result<()> {
        self.hud_to_chrono()?;
        self.info_zone()?;
        if self.u32(0x4e060c)? != 0 {
            self.tooltip()?;
        }
        if self.u8(0x4e17ac)? != 0 {
            return self.stop(0x417c7f, "AffDebug enabled");
        }
        if event {
            self.event_affiche()?;
        }
        self.message_box()?;
        // AffDrapeauAlliance 0x417581: for each other player, the allied flag
        // (PlayerEstAllie), or the contacting player's flag (counter at
        // 0x5080b0 + 16 * camera + 4 * p, PlayerEstContacterPourAlliance)
        // shown while (frame / 10) is even. Pole 0x2b13 and cloth 0x2b14 +
        // (0x508f80 & 7) in the player's colours, 20 pixels apart from
        // ((W - 1) * 32 - 80, (H - 1) * 32), both shifted by the shake byte.
        let players = self.u32(0x5080a8)?;
        let shake = i32::from(self.u8(0x508f90)? as u8 as i8) * 32;
        let right = (i32::from(self.u16(0x508724)? as u16) - 1) * 32 - 80;
        let bottom = (i32::from(self.u16(0x508726)? as u16) - 1) * 32;
        for p in 0..4 {
            if p == self.camera {
                continue;
            }
            let allied =
                self.camera <= 4 && self.u8(players + 20 * self.camera + 6)? & (1 << p) != 0;
            if !allied {
                if self.u32(0x5080b0 + 16 * self.camera + 4 * p)? == 0 {
                    continue;
                }
                if (self.u32(0x508f7c)? / 10) & 1 != 0 {
                    continue;
                }
            }
            let (x, y) = (i16(right + 20 * p as i32 - shake), i16(bottom - shake));
            self.bob(x, y, 0x2b13, true, Some(p))?;
            let cloth = 0x2b14 + (self.u8(0x508f80)? & 7);
            self.bob(x, y, cloth, true, Some(p))?;
        }
        let mx = self.u16(0x5099a8)?;
        let my = self.u16(0x5099aa)?;
        self.put16("mouse_x_copia_u16", 0x49a042, mx)?;
        self.put16("mouse_y_copia_u16", 0x49a044, my)?;
        if self.u8(0x49a054)? == 0 {
            let base = self.u16(0x49a010)?;
            self.bob(
                i16(mx as i32),
                i16(my as i32),
                (base + self.u16(0x49a014)?) & 65535,
                true,
                None,
            )?;
            let second = self.u16(0x4e062c)?;
            if second != 0 {
                if second > 16 {
                    return self.stop(0x417cfc, "second cursor count outside 0..16");
                }
                self.bob(
                    i16(mx as i32),
                    i16(my as i32),
                    (base + 16 - second) & 65535,
                    true,
                    None,
                )?;
            }
        }
        Ok(())
    }
    /// AffGame 0x417b32..0x417c56: counters, action icons, placement, target
    /// marker, market, minimap and AffChrono. AffGame calls them all at one
    /// esp, so their frames overlap; with the panel hidden AffChrono adds the
    /// word at that esp - 52 as its BobGetTaille width without calling it
    /// (0x401e89, 0x402025). That word is what the last of these calls left:
    /// tracked here as `slot`, low 16 bits (AffChrono uses DI only).
    pub(crate) fn hud_to_chrono(&mut self) -> Result<Option<u16>> {
        for (k, x) in [492, 540, 588].into_iter().enumerate() {
            let a = 0x4e2eac + 8 * (4 * self.camera + k as u32);
            self.counter(self.u16(a)?, self.u8(a + 2)?, x, 557)?;
        }
        // AffCompteurMatiere 0x417f38 pushes EDX (y = 0x22d) at esp - 52.
        let mut slot = Some(0x22d);
        if self.u8(0x4e05a2)? != 0 {
            // 0x416f29 keeps its icon x at [esp+0x10]: 0x20 + 4 * 0x3c at exit.
            slot = Some(0x110);
        }
        self.action_icons()?;
        let mx = self.u16(0x5099a8)?;
        let my = self.u16(0x5099aa)?;
        if mx <= self.u16(0x508724)? * 32
            && my <= self.u16(0x508726)? * 32
            && self.u16(0x4e05ae)? == 3
        {
            slot = Some(self.placement()?);
        }
        let cell = self.u16(0x496204)?;
        if cell != 65535 {
            let mut id = self.u16(0x4e05ac)?;
            if !(0x1e03..=0x1e09).contains(&id) {
                return self.stop(
                    0x417ba2,
                    "target marker outside captured animation IDs 0x1e03..0x1e09",
                );
            }
            if self.u8(0x508f7c)? & 1 == 0 {
                id += 1;
                self.put16("alvo_animacao_u16", 0x4e05ac, id)?;
            }
            let x = (cell as i32 % 256 - self.u16(0x508720)? as i32) * 32 + 12;
            let y = (cell as i32 / 256 - self.u16(0x508722)? as i32) * 32 + 12;
            // PasteImageClip 0x45c455 pushes its EAX (x) at esp - 52.
            slot = Some(x as u16);
            self.bob(i16(x), i16(y), id, true, Some(self.camera))?;
            if id == 0x1e0a {
                self.put16("alvo_animacao_u16", 0x4e05ac, 0x1e03)?;
                self.put16("alvo_celula_u16", 0x496204, 65535)?;
            }
        }
        slot = match self.u8(0x4e2e7c + self.camera)? {
            // 0x46677d ends with PasteImage 0x45b247 (CL = 0): [esp+0x10] = 0x257.
            1 | 2 => Some(0x257),
            // AffMarcheProposition 0x465b8c stores the player byte there.
            3 => slot.map(|w| (w & 0xff00) | self.camera as u16 & 0xff),
            // 0x46590f's PasteImage pushes EBP there: AffGame's frame
            // address, a host stack address the port does not model.
            4 => None,
            _ => slot,
        };
        self.market(self.camera)?;
        if self.u8(0x497285)? != 0 {
            self.minimap()?;
            // AffMiniMap 0x46a70c reuses the slot; AffChrono then takes its
            // width from BobGetTaille and never reads it.
            slot = None;
        }
        self.chrono(slot)?;
        Ok(slot)
    }
    /// AffChrono0x401cc6: live recruitment progress, no simulation writes.
    /// `slot` is the word at AffGame's esp - 52 when AffChrono is entered
    /// (see `hud_to_chrono`); None where the port does not model it.
    pub fn chrono(&mut self, slot: Option<u16>) -> Result<()> {
        let mut selected = self.u16(0x496ac4)?;
        if selected == 65535 {
            selected = self.u16(0x495be0 + 2 * self.camera)?;
        }
        if selected == 65535 {
            return Ok(());
        }
        if selected >= 1000 {
            return self.stop(0x401d18, "AffChrono selected structure index");
        }
        let a = 0x49a0c8 + 276 * selected;
        if self.u8(a + 4)? != self.camera {
            return Ok(());
        }
        let kind = self.u8(a + 8)?;
        if !matches!(kind, 1 | 2 | 5 | 6 | 13..=16 | 18 | 20) || !self.inhabited(selected)? {
            return Ok(());
        }
        let (mut x, mut y) = (0, 0);
        if self.u8(0x497285)? != 0 {
            let margin = (self.u8(0x508f90)? as u8 as i8 as i32) << 5;
            y = margin;
            x = if self.u16(0x507d68)? > self.u16(0x49a018)? / 2 {
                margin
            } else {
                self.u16(0x49a018)? as i32 - self.assets.sprite(0x1e6c)?.width as i32 - margin
            };
        }
        if self.u8(a + 0x74)? != 0 {
            let show = if kind == 14 {
                self.u16(a + 0x7e)? == 1
            } else if kind == 5 {
                !(self.u8(a + 0x76)? == 7 && self.u8(a + 0x75)? != 0)
            } else {
                true
            };
            if show {
                let target = self.u8(a + if kind == 14 { 0x88 } else { 0x7b })?;
                let percent = crate::chain_recruit::status(self.m, selected, target)?;
                self.shadow(i16(x), i16(y), 0x1e6c, false)?;
                self.bob(
                    i16(x),
                    i16(y),
                    0x1e4b + ((percent & 65535) * 32 / 100),
                    true,
                    None,
                )?;
            }
        }
        if kind == 20 && self.u8(a + 0x104)? == 1 {
            // 0x402010..0x402085 with [esp+8]: the BobGetTaille width of
            // 0x401e2b, or, with the panel hidden (0x401e89 skips the call),
            // the word AffGame's earlier HUD calls left in that slot.
            let width = if self.u8(0x497285)? != 0 {
                self.assets.sprite(0x1e6c)?.width as i32
            } else if let Some(word) = slot {
                i32::from(word)
            } else {
                return self.stop(
                    0x402025,
                    "AffChrono research progress, panel hidden: stack word after AffMarche settlement",
                );
            };
            let margin = (self.u8(0x508f90)? as u8 as i8 as i32) << 5;
            x = if x & 0xffff == margin {
                x + width
            } else {
                x - width
            };
            let divisor = self.u16(0x495bf0)?;
            if divisor == 0 {
                return self.stop(0x423ea8, "AffChrono research progress divisor");
            }
            let percent = self.u32(a + 0x108)?.wrapping_mul(100) / divisor;
            self.shadow(i16(x), i16(y), 0x1e6c, false)?;
            self.bob(
                i16(x),
                i16(y),
                (0x1e4b + (percent & 65535) * 32 / 100) & 65535,
                true,
                None,
            )?;
        }
        Ok(())
    }
    fn counter(&mut self, value: u32, highlight: u32, mut x: i32, y: i32) -> Result<()> {
        // IntToBCD 0x467cd1 divides by 1000, 100, 10 and 1 and stores each
        // quotient as a byte: above 9999 the thousands byte is the plain
        // quotient (at most 65 for the u16 stock word), drawn as at most four
        // dots below; the other digits are the usual ones.
        let digits = [value / 1000, value / 100 % 10, value / 10 % 10, value % 10];
        let mut leading = true;
        for (i, digit) in digits.into_iter().enumerate() {
            let bit = (highlight >> (3 - i)) & 1;
            if digit == 0 && leading && i != 3 && bit == 0 {
                if i != 0 {
                    self.bob(x, y, 0x2a21, false, None)?;
                }
                x += 12;
                continue;
            }
            if i == 0 {
                for j in 0..digit.min(4) {
                    self.bob(x + 12 + 8 * j as i32, y - 9, 0x2a3c, false, None)?;
                }
            } else {
                self.bob(x, y, 0x2a22 + 2 * digit + bit, false, None)?;
            }
            leading = false;
            x += 12;
        }
        Ok(())
    }
    pub(crate) fn action_icons(&mut self) -> Result<()> {
        if self.u8(0x4e05a2)? == 0 {
            return Ok(());
        }
        let selected = self.u16(0x496d4a)?;
        let left = self.u16(0x4e2440)?;
        self.bob(
            8,
            547,
            if left == 0x29b1 {
                0x29b5
            } else if selected == 8 {
                0x2a39
            } else {
                0x2a37
            },
            false,
            None,
        )?;
        for i in 0..4 {
            let a = 0x4e2410 + 6 * i;
            let x = 31 + 60 * i as i32;
            let id = self.u16(a)?;
            if id == 0x29b1 {
                self.bob(x, 547, 0x29b1 + i, false, None)?;
                continue;
            }
            self.bob(x, 547, 0x29b7, false, None)?;
            self.bob(x + 2, 549, id, false, None)?;
            match self.u16(a + 2)? {
                0 => {
                    // 0x417055: mask 0x2a1d - value * 42 / 100 (signed idiv, 16-bit id)
                    // for any value; 101 and 102 give the 100 mask. Values past
                    // the mask range select ids outside the catalog, where
                    // PasteNoirBlancCmp reads past the sprite table.
                    let value = self.u16(a + 4)?;
                    let mask = 0x2a1du32.wrapping_sub(value * 42 / 100) & 0xffff;
                    self.observe("PasteNoirBlancCmp", 0x45bf3d, &[x + 2, 549, mask as i32, 0]);
                    let Ok(q) = self.assets.sprite(mask) else {
                        return self.stop(
                            0x417086,
                            "AffZoneIconeAction percent mask outside the catalog",
                        );
                    };
                    let table = self.m.read(self.u32(0x4e24bc)?, 256)?;
                    for sy in 0..q.height.min(600 - 549) {
                        for sx in 0..q.width.min(800 - (x as usize + 2)) {
                            let src = sy * q.width + sx;
                            if q.mask[src] != 0 {
                                let p = (549 + sy) * 800 + x as usize + 2 + sx;
                                self.surface[p] = table[self.surface[p] as usize];
                            }
                        }
                    }
                }
                1 => {}
                2 => {
                    let value = self.u16(a + 4)?;
                    if value > 999 {
                        return self.stop(0x417086, "AffZoneIconeAction counter outside 0..999");
                    }
                    let mut leading = true;
                    for (j, d) in [value / 100, value / 10 % 10, value % 10]
                        .into_iter()
                        .enumerate()
                    {
                        if leading && d == 0 && j < 2 {
                            continue;
                        }
                        self.bob(x + 22 + 12 * j as i32, 582, 0x2a22 + 2 * d, false, None)?;
                        leading = false;
                    }
                }
                _ => return self.stop(0x417086, "AffZoneIconeAction slot kind outside 1.5"),
            }
            if selected == i {
                self.bob(x + 2, 549, 0x2a36, false, None)?;
            }
        }
        let right = self.u16(0x4e2446)?;
        self.bob(
            272,
            547,
            if right == 0x29b1 {
                0x29b6
            } else if selected == 9 {
                0x2a3a
            } else {
                0x2a38
            },
            false,
            None,
        )
    }
    fn minimap(&mut self) -> Result<()> {
        let mx = self.u16(0x507d68)?;
        let my = self.u16(0x507d6a)?;
        let w = self.u16(0x496fe0)?;
        let h = self.u16(0x496fe2)?;
        let cw = self.u16(0x497168)?;
        let ch = self.u16(0x49716a)?;
        if w == 0 || h == 0 || mx + 1 + w > 800 || my + 1 + h > 600 || cw == 0 || ch == 0 {
            return self.stop(0x46a70c, "AffMiniMap outside captured geometry");
        }
        self.outline(
            mx as i32,
            my as i32,
            (mx + w + 1) as i32,
            (my + h + 1) as i32,
            20,
            true,
        )?;
        let data = self.m.read(self.u32(0x507d5c)?, (w * h) as usize)?;
        for y in 0..h as usize {
            let p = (my as usize + 1 + y) * 800 + mx as usize + 1;
            self.surface[p..p + w as usize]
                .copy_from_slice(&data[y * w as usize..(y + 1) * w as usize]);
        }
        let ox = self.u16(0x508720)?;
        let oy = self.u16(0x508722)?;
        self.outline(
            (ox * w / cw + mx + 1) as i32,
            (oy * h / ch + my + 1) as i32,
            ((ox + self.u16(0x508724)?) * w / cw + mx + 1) as i32,
            ((oy + self.u16(0x508726)?) * h / ch + my + 1) as i32,
            1,
            true,
        )
    }
    fn info_zone(&mut self) -> Result<()> {
        self.bob(298, 547, 0x29b8, false, None)?;
        let value = self.u16(0x4e0618)?;
        if value != 0 {
            return self.info_row(value, 0x2a46, 548);
        }
        let value = self.u16(0x4e0612)?;
        if value != 0 {
            self.info_row(value, 0x2a43, 548)?;
        }
        let value = self.u16(0x4e0614)?;
        if value != 0 {
            self.info_row(value, 0x2a44, 574)?;
        } else {
            let value = self.u16(0x4e0616)?;
            if value != 0 {
                self.info_row(value, 0x2a42, 574)?;
            }
        }
        Ok(())
    }
    fn info_row(&mut self, value: u32, id: u32, top: i32) -> Result<()> {
        self.bob(299, top, id, false, None)?;
        self.bob(327, top + 4, 0x2a45, false, None)?;
        if value > 9999 {
            return self.stop(0x40f77b, "AffZoneInfo cost counter exceeds 9999");
        }
        let mut x = 328;
        let mut leading = true;
        for (i, d) in [value / 1000, value / 100 % 10, value / 10 % 10, value % 10]
            .into_iter()
            .enumerate()
        {
            if leading && d == 0 && i < 3 {
                if i == 2 {
                    self.bob(x, top + 5, 0x2a21, false, None)?;
                    x += 12;
                }
                continue;
            }
            self.bob(x, top + 5, 0x2a22 + 2 * d, false, None)?;
            x += 12;
            leading = false;
        }
        Ok(())
    }
}
