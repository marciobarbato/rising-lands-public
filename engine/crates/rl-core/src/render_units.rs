//! Normal bodies and Graphics 1.7 input overlays; unsupported branches stop.
use crate::render::{i16, w, Context, Result};
impl Context<'_> {
    /// AffDyingEngin0x41559a, original corpse animation and stencil fade.
    fn dying_unit(&mut self, x: i32, y: i32, _index: u32, r: &[u8]) -> Result<()> {
        const BASE: [u32; 26] = [
            2226, 2386, 3346, 3554, 2818, 1762, 3106, 1538, 2610, 1890, 2018, 2226, 2226, 2226,
            2226, 2226, 2226, 2226, 2226, 2226, 5621, 5237, 6005, 6389, 6773, 3938,
        ];
        const OFF: [(i32, i32); 26] = [
            (-40, -22),
            (-23, -9),
            (-32, -16),
            (-45, -56),
            (-40, -32),
            (-11, -11),
            (-16, -20),
            (-26, -26),
            (-32, -30),
            (-19, -14),
            (-24, -12),
            (-41, -52),
            (-52, -48),
            (-55, -56),
            (-42, -45),
            (-49, -66),
            (-78, -54),
            (-79, -212),
            (-36, -48),
            (-16, -16),
            (-46, -54),
            (-23, -15),
            (-54, -54),
            (-22, -16),
            (-32, -32),
            (-45, -56),
        ];
        let kind = usize::from(r[1]);
        if kind >= 26 {
            return self.stop(0x41559a, "AffDyingEngin unit type");
        }
        // The intact renderer explicitly returns for these flying types;
        // their death effects come from SetDyingEngin, independently.
        if (11..=17).contains(&kind) {
            return Ok(());
        }
        // AffDyingEngin reads neither the camera nor +0x99; the effects of
        // +0xdc are AffCorpsEngin's (owners 0..4 and the neutral grey).
        if r[0] > 4 && r[0] != 200 {
            return self.stop(0x411147, "AffDyingEngin owner");
        }
        if !(1..=16).contains(&r[0x91]) {
            return self.stop(0x415647, "AffDyingEngin direction");
        }
        let timer = w(r, 0xc2);
        let id = BASE[kind]
            + (u32::from(r[0x91]).div_ceil(2) - 1) * u32::from(r[0xdb])
            + u32::from(r[0xa4]);
        let (ox, oy) = OFF[kind];
        let (px, py) = (
            i16(x + i32::from(r[0x86] as i8) + ox),
            i16(y + i32::from(r[0x87] as i8) + oy),
        );
        if timer <= 315 {
            // 0x41569c: AffCorpsEngin.
            self.corps_unit(px, py, id, r)?;
        }
        let copies = if (300..315).contains(&timer) {
            (timer - 300) / 5
        } else if timer >= 315 {
            3i32.saturating_sub((timer as i32 - 315) / 5).max(0) as u32
        } else {
            0
        };
        for _ in 0..copies {
            self.shadow(px, py, id, false)?;
            // 0x4157b4 / 0x4158e7: a neutral corpse also gets PasteGrisCmp
            // 0x45bcb0 after each PasteOmbreCmp copy.
            if r[0] == 200 {
                self.observe("PasteGrisCmp", 0x45bcb0, &[px, py, id as i32, 1]);
                self.remap_sprite(px, py, id, 0x4e24c4)?;
            }
        }
        Ok(())
    }

    pub fn unit(&mut self, x: i32, y: i32, index: u32, r: &[u8]) -> Result<()> {
        let kind = r[1];
        let state = r[2];
        if state == 4 {
            return self.dying_unit(x, y, index, r);
        }
        if crate::transport_unit_render::supports(r) {
            return crate::transport_unit_render::draw(self, x, y, index, r);
        }
        if crate::unit_experience_render::supports(r) {
            return crate::unit_experience_render::draw(self, x, y, index, r);
        }
        if self.late_unit(x, y, index, r)? {
            return Ok(());
        }
        // AffEngin 0x4159ce: states 11 and 15 draw AffConstruireBatiment 0x414495
        // for every kind; only kinds 6 and 7 have a body there.
        if matches!(state, 11 | 15) {
            return self.construction_builder(r, x, y, index);
        }
        let base_type = matches!(kind, 0 | 2 | 5 | 6 | 7 | 9);
        // AffEngin41591f sends state 13 (walking into a structure or vehicle)
        // to the common body 412390 for every kind. Any +0x99 jitter or +0xa5
        // move count takes the shared AffDeplaceEngin provider, which reads
        // both (native differential in the private research repository, 6876/6876).
        if matches!(kind, 0 | 1 | 2 | 5 | 6 | 7 | 8 | 9)
            && state == 13
            && (r[0x99] != 0 || r[0xa5] != if kind == 8 { 10 } else { 8 })
        {
            return self.unit_display_extra(x, y, index, r);
        }
        let route = match state {
            1 | 22 | 24 => {
                if r[0x8a] == 0 {
                    1
                } else if r[0x8a] <= if state == 22 { 1 } else { 3 } {
                    2
                } else {
                    0
                }
            }
            2 | 3 | 6 | 7 | 8 | 9 | 13 | 16 | 17 | 18 | 19 | 20 | 21 | 23 | 30 => 1,
            4 | 5 | 11 | 15 => 2,
            _ => 0,
        };
        if route == 0 {
            // AffEngin41591f returns without drawing for these states, whatever
            // the kind (dispatch on the state byte only).
            return Ok(());
        }
        if kind == 7 && state == 5 && matches!(r[0x89], 1 | 2) {
            return self.work_body(r, x, y, index, false);
        }
        let overlay =
            r[0xba] != 0 || r[0xbd] != 0 || self.u16(0x496ac2)? == index || self.u8(0x508f8f)? != 0;
        // AffEngin 0x415a0a: state 5 calls AffWorkEngin 0x414a8a for every
        // kind, not only the workers (an AI soldier in state 5, MONDE10A).
        if state == 5 && !matches!(kind, 5 | 7 | 13) {
            return self.work_body(r, x, y, index, overlay);
        }
        let dx = r[0x86] as i8 as i32;
        let dy = r[0x87] as i8 as i32;
        let (px, py) = (x + dx, y + dy);
        // 415990 / 4159a6: states 1 and 24 with phase 1..3, and state 22 with
        // phase 1, take AffAttaqueEngin (kind 16 never does).
        if kind != 16
            && (matches!(state, 1 | 24) && (1..=3).contains(&r[0x8a])
                || state == 22 && r[0x8a] == 1)
        {
            return self.attack_body(r, x, y, index, overlay);
        }
        // AffWorkEngin414a8a draws markers only for the camera's units; other
        // owners go straight to the body and the shared info/blink tail.
        if matches!(kind, 5 | 7 | 13) && state == 5 && r[0] as u32 != self.camera {
            return self.work_body(r, x, y, index, overlay);
        }
        // AffConstruireBatiment414495: foreign builders go straight to the body and
        // the shared info tail (hover/info over an enemy builder).
        if overlay {
            if !(matches!(kind, 0 | 1 | 2 | 5 | 6 | 7 | 8 | 9) || kind == 13 && state == 5)
                || r[0] != 0
                || self.camera != 0
                || !(matches!(state, 2 | 7 | 21)
                    || state == 1 && r[0x8a] == 0
                    || matches!(kind, 7 | 13) && state == 5
                    || kind == 5 && state == 5
                    || state == 13
                    || kind == 7 && state == 19
                    || matches!(kind, 5..=7) && matches!(state, 16 | 17 | 18 | 20))
                || r[0x99] != 0 && !(kind == 13 && state == 5)
            {
                return self.stop(
                    0x41591f,
                    "input overlay outside captured owner-0 normal field-unit domain",
                );
            }
            // AffWorkEngin 0x414a8a: kind 13 at work reads neither the +0x99
            // jitter nor a fixed body count of 8.
            if r[0xa5] != if kind == 8 { 10 } else { 8 } && !(kind == 13 && state == 5) {
                return self.stop(
                    0x41591f,
                    "input-overlay body count differs from initialized count",
                );
            }
            // Target modes 1 and 2 (any depot structure, BatimentGetCase)
            // and the marker-less modes >= 3 are drawn by order_markers.
            if r[0xbd] != 0 && r[0] as u32 == self.camera {
                // 44106c (table 441004): kind 13 uses the large marker.
                if kind == 13 {
                    self.bob(i16(px - 32), i16(py - 32), 0x2a1f, true, None)?;
                } else {
                    self.bob(i16(px), i16(py), 0x2a1e, true, None)?;
                }
            }
            // Native 0x412611 skips order markers only for state17.
            if matches!(state, 2 | 7 | 13 | 16 | 18 | 20 | 21)
                || kind == 7 && state == 19
                || state == 1 && r[0x8a] == 0
            {
                self.order_markers(r)?;
            }
        }
        if matches!(kind, 5 | 7 | 13) && state == 5 {
            return self.work_body(r, x, y, index, overlay);
        }
        if matches!(kind, 1 | 8) {
            let count = if kind == 1 { 8 } else { 10 };
            if r[0] != 0
                || self.camera != 0
                || !(matches!(state, 2 | 7 | 13 | 21) || state == 1 && r[0x8a] == 0)
                || r[0x99] != 0
                || r[0xa5] != count
                || !(1..=16).contains(&r[0x91])
                || r[0xa4] >= count
            {
                return self.stop(
                    0x41591f,
                    format!(
                        "AffEngin unit {index} type {kind} outside captured type-1/8 body domain"
                    ),
                );
            }
            let (base, ax, ay) = if kind == 1 {
                (2290, 23, 9)
            } else {
                (2450, 32, 30)
            };
            let id = base + ((r[0x91] as u32).div_ceil(2) - 1) * count as u32 + r[0xa4] as u32;
            self.shadow(i16(px + 2), i16(py + 26), 10784, false)?;
            self.corps_unit(i16(px - ax), i16(py - ay), id, r)?;
        } else {
            if !base_type {
                return self.stop(
                    0x41591f,
                    format!("AffEngin unit {index} type {kind} has no committed body"),
                );
            }
            if route != 1
                || !(matches!(state, 2 | 7 | 13 | 21)
                    || state == 1 && r[0x8a] == 0
                    || kind == 7 && state == 19
                    || matches!(kind, 5..=7) && matches!(state, 16 | 17 | 18 | 20))
                || r[0] != 0
                || self.camera != 0
                || !(1..=16).contains(&r[0x91])
                || r[0xa4] > 7
            {
                return self.stop(0x41591f,format!("AffEngin unit {index} type {kind} state {state} outside validated body domain"));
            }
            let bag = w(r, 10) != 0;
            let (base, ax, ay) = match kind {
                0 => (2082, 40, 22),
                // Intact0x412ece: body0xc82, offsets0x410ff8 (-32,-16).
                2 => (3202, 32, 16),
                5 => (if bag { 1698 } else { 1602 }, 11, 11),
                6 => (if bag { 3042 } else { 2978 }, 16, 20),
                7 => (if bag { 1474 } else { 1346 }, 26, 26),
                9 => (1826, 19, 14),
                _ => unreachable!(),
            };
            self.shadow(px + 2, py + 26, 10784, false)?;
            // AffDeplaceEngin: the body goes through AffCorpsEngin (effects of +0xdc).
            self.corps_unit(
                px - ax,
                py - ay,
                base + ((r[0x91] as u32 - 1) / 2) * 8 + r[0xa4] as u32,
                r,
            )?;
        }
        self.unit_tail(r, x, y, index, overlay)
    }
    /// AffDeplaceEngin/AffAttaqueEngin late-S1 army bodies. Owner 4
    /// draws twice through AffCorpsEngin0x411c9f then0x41121b.
    fn late_unit(&mut self, x: i32, y: i32, index: u32, r: &[u8]) -> Result<bool> {
        let (owner, kind, state) = (r[0], r[1], r[2]);
        let routed = owner == 4 && kind == 0 && matches!(state, 1 | 2 | 7)
            || owner == 0 && matches!(kind, 0 | 1 | 5 | 8) && state == 19;
        if !routed {
            return Ok(false);
        }
        let attack = state == 1 && r[0x8a] != 0;
        // The effects of +0xdc are AffCorpsEngin's. The movement body
        // (AffDeplaceEngin) also reads +0x99, still outside this port.
        if !attack && (self.camera != 0 || r[0x99] != 0) {
            return self.stop(0x411147, "late-S1 movement body +0x99/camera");
        }
        if state == 1 && r[0x8a] > 3 {
            return Ok(true);
        }
        let count = r[if attack { 0xa6 } else { 0xa5 }];
        if !(1..=16).contains(&r[0x91])
            || count != if attack || kind == 8 { 10 } else { 8 }
            || r[0xa4] >= count
        {
            return self.stop(0x412390, "late-S1 direction/animation count");
        }
        let (base, ax, ay) = match kind {
            0 => (if attack { 2146 } else { 2082 }, 40, 22),
            1 => (2290, 23, 9),
            5 => (if w(r, 10) != 0 { 1698 } else { 1602 }, 11, 11),
            8 => (2450, 32, 30),
            _ => unreachable!(),
        };
        let (px, py) = (x + r[0x86] as i8 as i32, y + r[0x87] as i8 as i32);
        // AffDeplaceEngin0x4124a9: own selection precedes order markers,
        // shadow and body. The supported late armies have one-cell selection.
        if r[0xbd] != 0 && u32::from(owner) == self.camera {
            self.bob(i16(px), i16(py), 0x2a1e, true, None)?;
            if !attack {
                self.order_markers(r)?;
            }
        }
        self.shadow(i16(px + 2), i16(py + 26), 10784, false)?;
        let id =
            base + (u32::from(r[0x91]).div_ceil(2) - 1) * u32::from(count) + u32::from(r[0xa4]);
        self.corps_unit(i16(px - ax), i16(py - ay), id, r)?;
        self.unit_tail(r, x, y, index, true)?;
        Ok(true)
    }
    /// AffAttaqueEngin0x41374c: small troop bodies, selection then shadow/body/info.
    /// AffAttaqueEngin 0x41374c for kinds 3, 11, 12, 14, 15, 16 and 25: selection
    /// marker, then per-kind shadow and body from the dispatch table at
    /// 0x4136e4 (413aa5, 413d7f, 413cf9, 413f73, 413ff5, 413e01, 413b56).
    fn vehicle_attack_body(
        &mut self,
        r: &[u8],
        x: i32,
        y: i32,
        index: u32,
        overlay: bool,
    ) -> Result<()> {
        let kind = r[1];
        if r[0] > 4 && r[0] != 200 || !(1..=16).contains(&r[0x91]) {
            return self.stop(0x41374c, "vehicle attack owner/direction");
        }
        let (px, py) = (
            i16(x + i32::from(r[0x86] as i8)),
            i16(y + i32::from(r[0x87] as i8)),
        );
        if r[0xbd] != 0 && u32::from(r[0]) == self.camera {
            self.bob(i16(px - 32), i16(py - 32), 0x2a1f, true, None)?;
            if kind == 14 {
                // 41383d..413924: the bomb queue markers, as on the movement path.
                self.bomb_queue(r, index)?;
            }
        }
        let dir = i32::from(r[0x91]);
        let a4 = i32::from(r[0xa4]);
        let a5 = i32::from(r[0xa5]);
        let a6 = i32::from(r[0xa6]);
        let n = (dir / 2) * a5 + (dir - 1) / 2 + a4 * (dir % 2);
        let ceil = (dir + 1) / 2;
        let halved = (dir - 1) / 2;
        let (shadow, body, ax, ay) = match kind {
            3 => (0xe5a + ceil * 8 + a4, 0xda2 + (ceil - 1) * a6 + a4, 45, 56),
            25 => (0xfda + ceil * 8 + a4, 0xf22 + (ceil - 1) * a6 + a4, 45, 56),
            11 => (0x10ba + halved * a6 + a4, 0x1082 + halved * a6 + a4, 41, 52),
            12 => (0x1272 + n, 0x1212 + n, 52, 48),
            14 => (0x1322 + halved * a6 + a4, 0x12f2 + halved * a6 + a4, 42, 45),
            _ => (0x11b2 + n, 0x1152 + n, 49, 66),
        };
        self.shadow(i16(px - ax), i16(py - ay), (shadow as u32) & 65535, false)?;
        self.corps_unit(i16(px - ax), i16(py - ay), (body as u32) & 65535, r)?;
        self.unit_tail(
            r,
            px - i32::from(r[0x86] as i8),
            py - i32::from(r[0x87] as i8),
            index,
            overlay,
        )
    }
    fn attack_body(&mut self, r: &[u8], x: i32, y: i32, index: u32, overlay: bool) -> Result<()> {
        let kind = r[1];
        if matches!(kind, 5..=7 | 9 | 10 | 13 | 17 | 18 | 19) {
            // 0x4137c0: the own-selection marker precedes the kind switch
            // (EnginGetTaille 0x44106c: large 0x2a1f for kind 13, small 0x2a1e
            // otherwise). These kinds draw no attack body: 0x4139b8 goes to the
            // tail with the prelude's coordinates, still (0, 0); kind 17
            // (0x4140b8 -> 0x413f6b) saves (px - 32, py - 9).
            let (px, py) = (x + r[0x86] as i8 as i32, y + r[0x87] as i8 as i32);
            if r[0xbd] != 0 && r[0] as u32 == self.camera {
                if kind == 13 {
                    self.bob(i16(px - 32), i16(py - 32), 0x2a1f, true, None)?;
                } else {
                    self.bob(i16(px), i16(py), 0x2a1e, true, None)?;
                }
            }
            let info = overlay
                && ((r[0] as u32 == self.camera && r[0xbd] != 0)
                    || self.u16(0x496ac2)? == index
                    || self.u8(0x508f8f)? != 0);
            let (ax, ay) = if kind == 17 {
                (px - 32, py - 9)
            } else {
                (0, 0)
            };
            return self.tail_at(r, ax, ay, index, overlay, info);
        }
        if matches!(kind, 20..=24) {
            // AffAttaqueEngin41374c ->4140c4/41413c/4142a4/4141b4: directional shadow, then
            // the common AffCorpsEngin body (owner palettes, owner4 twice, neutral
            // grey). The own-selection marker (4137c0, small 2a1e) precedes them.
            if r[0] > 4 && r[0] != 200 || !(1..=16).contains(&r[0x91]) {
                return self.stop(0x41374c, "type20..24 attack owner/direction");
            }
            let (px, py) = (
                i16(x + i32::from(r[0x86] as i8)),
                i16(y + i32::from(r[0x87] as i8)),
            );
            if r[0xbd] != 0 && r[0] as u32 == self.camera {
                self.bob(px, py, 0x2a1e, true, None)?;
            }
            let frame = u32::from((r[0x91] - 1) / 2) * u32::from(r[0xa6]) + u32::from(r[0xa4]);
            let (ax, ay, body, shadow) = match kind {
                20 => (46, 54, 0x15b5, 0x1675),
                21 => (23, 15, 0x1435, 0x14f5),
                22 => (54, 54, 0x1735, 0x17f5),
                // 0x41422c: shadow 0x1af5, body 0x1a35; AffCorpsEnginDecalage 0x411089.
                24 => (32, 32, 0x1a35, 0x1af5),
                _ => (22, 16, 0x18b5, 0x1975),
            };
            self.shadow(i16(px - ax), i16(py - ay), (shadow + frame) & 65535, false)?;
            self.corps_unit(i16(px - ax), i16(py - ay), (body + frame) & 65535, r)?;
            return self.unit_tail(r, x, y, index, overlay);
        }
        if kind == 8 && r[0] == 4 {
            // Intact 413929..4139b8: fixed ground shadow then AffCorpsEngin.
            // Native uses the raw animation byte, including phase >= count.
            if !(1..=16).contains(&r[0x91]) {
                return self.stop(0x41374c, "type8 owner4 attack direction");
            }
            let (px, py) = (
                i16(x + i32::from(r[0x86] as i8)),
                i16(y + i32::from(r[0x87] as i8)),
            );
            self.shadow(i16(px + 2), i16(py + 26), 0x2a20, false)?;
            let frame = u32::from((r[0x91] - 1) / 2) * u32::from(r[0xa6]) + u32::from(r[0xa4]);
            self.corps_unit(i16(px - 32), i16(py - 30), (0x9e2 + frame) & 65535, r)?;
            return self.unit_tail(r, x, y, index, true);
        }
        if matches!(kind, 3 | 25 | 11 | 12 | 14 | 15) {
            return self.vehicle_attack_body(r, x, y, index, overlay);
        }
        if kind == 4 {
            // 0x413c14: the caster's shadow, then the state-1 (0xab2) or
            // state-24 (0xb52) casting body; other states draw no body.
            if r[0] > 4 && r[0] != 200 || !(1..=16).contains(&r[0x91]) {
                return self.stop(
                    0x413c14,
                    "caster attack body outside owner/effect/direction",
                );
            }
            let (px, py) = (x + r[0x86] as i8 as i32, y + r[0x87] as i8 as i32);
            if r[0xbd] != 0 && r[0] as u32 == self.camera {
                self.bob(i16(px), i16(py), 0x2a1e, true, None)?;
            }
            self.shadow(i16(px + 2), i16(py + 26), 10784, false)?;
            let base = match r[2] {
                1 => Some(0xab2),
                24 => Some(0xb52),
                _ => None,
            };
            if let Some(base) = base {
                let id = base
                    + (u32::from(r[0x91]).div_ceil(2) - 1) * u32::from(r[0xa6])
                    + u32::from(r[0xa4]);
                self.corps_unit(i16(px - 40), i16(py - 32), id & 65535, r)?;
            }
            return self.unit_tail(r, x, y, index, overlay);
        }
        // Kinds0/1/2/8 reach AffCorpsEngin411147 (owners0..4, neutral200).
        if !matches!(kind, 0 | 1 | 2 | 8)
            || r[0] > 4 && r[0] != 200
            || !(1..=16).contains(&r[0x91])
            || r[0xa6] == 0
            || r[0xa4] >= r[0xa6]
        {
            return self.stop(0x41374c, "attack body outside small owner-0 troop domain");
        }
        let (px, py) = (x + r[0x86] as i8 as i32, y + r[0x87] as i8 as i32);
        if r[0xbd] != 0 && r[0] as u32 == self.camera {
            self.bob(i16(px), i16(py), 0x2a1e, true, None)?;
        }
        let (base, ax, ay) = match kind {
            0 => (2146, 40, 22),
            1 => (2354, 23, 9),
            2 => (3266, 32, 16),
            8 => (2530, 32, 30),
            _ => unreachable!(),
        };
        self.shadow(i16(px + 2), i16(py + 26), 10784, false)?;
        let id =
            base + (u32::from(r[0x91]).div_ceil(2) - 1) * u32::from(r[0xa6]) + u32::from(r[0xa4]);
        self.corps_unit(i16(px - ax), i16(py - ay), id, r)?;
        self.unit_tail(r, x, y, index, overlay)
    }
    pub(crate) fn unit_tail(
        &mut self,
        r: &[u8],
        x: i32,
        y: i32,
        index: u32,
        overlay: bool,
    ) -> Result<()> {
        let dx = r[0x86] as i8 as i32;
        let dy = r[0x87] as i8 as i32;
        let (px, py) = (x + dx, y + dy);
        let kind = r[1];
        let info = overlay
            && ((r[0] as u32 == self.camera && r[0xbd] != 0)
                || self.u16(0x496ac2)? == index
                || self.u8(0x508f8f)? != 0);
        // AffDeplaceEngin saves the tail coordinates after the body: x, y-9 for
        // most kinds; 0x412e45 kinds 3 and 25 (x-15, y-24); 0x4130b0 kind 16 (x-32, y-41).
        let (tx, ty) = match kind {
            3 | 25 => (15, 24),
            16 => (32, 41),
            _ => (0, 9),
        };
        self.tail_at(r, px - tx, py - ty, index, overlay, info)
    }
    /// The shared info/blink/experience tail at the coordinates the body saved.
    pub(crate) fn tail_at(
        &mut self,
        r: &[u8],
        ax: i32,
        ay: i32,
        index: u32,
        overlay: bool,
        info: bool,
    ) -> Result<()> {
        let _ = index;
        let kind = r[1];
        if info {
            self.unit_info(r, ax, ay)?;
        } else {
            let life = w(r, 0x74);
            let tired = w(r, 4) == w(r, 0x76) && life < 100;
            if tired && (life / 5).is_multiple_of(2) && (kind <= 10 || kind == 25) {
                let owner = if r[0] == 200 {
                    None
                } else if r[0] < 5 {
                    Some(r[0] as u32)
                } else {
                    return self.stop(0x413650, "owner has no player table");
                };
                // AffDeplaceEngin0x412390 saves shifted x and shifted y-9
                // at +1c/+20; 0x4135fa subtracts 8/2, then sign-extends
                // the low coordinate words. AvanceEngin writes the signed
                // subcell shifts at +86/+87 and clears them on cell arrival.
                self.bob(
                    i16(ax - 8),
                    i16(ay - 2),
                    (w(r, 4) + 0x1e2b) & 0xffff,
                    true,
                    owner,
                )?;
            }
            if overlay {
                self.experience(r, ax, ay, false)?;
            }
        }
        Ok(())
    }
    fn experience(&mut self, r: &[u8], x: i32, y: i32, info: bool) -> Result<()> {
        // TypeEnginGagneXP 0x4422b0 admits kinds 0..10 and 25 only; vehicles,
        // flyers and monsters (11..24) never draw the level stars.
        if !(r[1] <= 10 || r[1] == 25) {
            return Ok(());
        }
        let level = r[0xaf] >> 5;
        if level != 0 && (info || r[0xba] != 0) && (r[0xba] == 0 || self.u32(0x508f7c)? % 5 == 0) {
            self.bob(i16(x - 8), i16(y - 2), 0x1e46 + level as u32, true, None)?;
        }
        Ok(())
    }
    /// AffContenanceEngin 0x410a2c: one dot per bag slot, top to bottom, eight
    /// pixels apart. Capacity 1 draws nothing. A resident (tag 1) is 0x2a3c in
    /// the player's palette; any tag other than 2 is the empty dot 0x2a3b. Tag
    /// 2 sums the loaded goods (0x4471b8) and was never reached.
    fn unit_bag_dots(&mut self, r: &[u8], x: i32, y: i32) -> Result<()> {
        let capacity = w(r, 12);
        if capacity == 1 {
            return Ok(());
        }
        let (mut i, mut py) = (0usize, y);
        while i < capacity as usize {
            match r[0x0a + 0x42 + i] {
                1 => {
                    self.bob(i16(x), i16(py), 0x2a3c, true, Some(u32::from(r[0])))?;
                    py += 8;
                }
                2 => {
                    // 0x410a63: BagContient(2, 1) + BagContient(2, 2) sack icons
                    // at this slot; the slot index skips past them.
                    let goods = (0..capacity as usize)
                        .filter(|&j| {
                            r[0x0a + 0x42 + j] == 2 && matches!(w(r, 0x0a + 6 + 2 * j), 1 | 2)
                        })
                        .count();
                    if goods == 0 {
                        // Native adds n - 1 to the index, then 1: it never advances.
                        return self.stop(0x410ab8, "AffContenanceEngin goods slot with no sack");
                    }
                    for _ in 0..goods {
                        self.bob(i16(x), i16(py), 0x2a3d, true, None)?;
                        py += 8;
                    }
                    i += goods - 1;
                }
                _ => {
                    self.bob(i16(x), i16(py), 0x2a3b, true, None)?;
                    py += 8;
                }
            }
            i += 1;
        }
        Ok(())
    }
    fn unit_info(&mut self, r: &[u8], x: i32, y: i32) -> Result<()> {
        if r[0] as u32 != self.camera {
            return Ok(());
        }
        let maximum = self.u16(0x4965b6 + 50 * r[1] as u32)?;
        let hp = w(r, 0x9a);
        if maximum != 100 || hp > maximum {
            return self.stop(0x410be4, "unit health outside captured 0..100 / max 100");
        }
        let percent = hp;
        let height = (15 * percent / 100) as i32;
        self.filled(x - 11, y, x - 8, y + 15, 2)?;
        self.filled(
            x - 11,
            y + 15 - height,
            x - 9,
            y + 14,
            if percent > 75 {
                48
            } else if percent > 35 {
                180
            } else {
                145
            },
        )?;
        let life = w(r, 0x74);
        // AffInfoSelection0x410c6c: equal provision words and life<100
        // blink every five counts; unequal words draw the current stock icon
        // continuously. SeFatigue0x442c34 (table0x442bcc) admits only kinds
        // 0..10 and25; flyers and vehicles11..24 never show the stock icon.
        let fatigue_kind = r[1] <= 10 || r[1] == 25;
        if fatigue_kind && (!(w(r, 4) == w(r, 0x76) && life < 100) || (life / 5).is_multiple_of(2))
        {
            self.bob(
                i16(x - 8),
                i16(y - 2),
                (0x1e2b + w(r, 4)) & 0xffff,
                true,
                Some(r[0] as u32),
            )?;
        }
        self.experience(r, x, y, true)?;
        self.unit_bag_dots(r, x, y + 10)?;
        // AffInfoSelection0x410eee..0x410f5b: kinds14/16 show five mana dots
        // (0x2a3d filled below EnginGetMana0x44828b = +0xda, 0x2a3b empty)
        // eight pixels apart at y+30, after AffContenanceEngin.
        if matches!(r[1], 14 | 16) {
            let mana = u32::from(r[0xda]);
            for i in 0..5u32 {
                self.bob(
                    i16(x + 8 * i as i32),
                    i16(y + 30),
                    if i < mana { 0x2a3d } else { 0x2a3b },
                    true,
                    None,
                )?;
            }
        }
        Ok(())
    }
    pub fn structure_overlay(
        &mut self,
        r: &[u8],
        cell: u32,
        tile: u32,
        frame: bool,
        gauge: bool,
    ) -> Result<()> {
        // The origin is the facade body's (the terrain word), the frame sprite,
        // gauge offset and slots are the record type's.
        let Some(origin) = crate::structure_overlay_render::facade_origin(tile) else {
            return self.stop(0x40f36a, "structure overlay outside a facade word");
        };
        if r[8] == 4 {
            return self.type4_overlay(r, cell, origin, frame, gauge);
        }
        if matches!(r[8], 2 | 3) {
            // Sanctuary and CAVE (any mode, occupancy and bag capacity): the
            // API 1.8 body with the generic AffJaugeBatiment bar and slots.
            return self.structure_overlay18(r, cell, origin, frame, gauge);
        }
        // Every other inhabited type: the generic tail (per-type frame
        // sprite sizes, gauge offsets and bag slots).
        self.structure_overlay_generic(r, cell, origin, frame, gauge)
    }

    /// AffCadreBatiment4022c8 and AffJaugeBatiment4020ec for the type4
    /// facade. Coordinates match its original -47/-44 caller adjustment.
    fn type4_overlay(
        &mut self,
        r: &[u8],
        cell: u32,
        origin: (i32, i32),
        frame: bool,
        gauge: bool,
    ) -> Result<()> {
        if r[4] > 4 || self.camera > 3 {
            return self.stop(0x4020ec, "type4 overlay owner/camera");
        }
        let ox = self.u16(0x508720)?;
        let oy = self.u16(0x508722)?;
        let x = i16((cell as i32 % 256 - ox as i32) * 32 - origin.0);
        let y = i16((cell as i32 / 256 - oy as i32) * 32 - origin.1);
        if frame {
            let sprite = self.assets.sprite(0x1c78)?;
            let width = sprite.width as i32;
            let height = sprite.height as i32;
            let phase = (self.u32(0x508f80)? & 7) as i32;
            for (id, px, py) in [
                (0x2a79, x + phase - 20, y + phase),
                (0x2a78, x + phase - 20, y + height - phase),
                (0x2a7a, x + width + 3 - phase, y + phase),
                (0x2a77, x + width + 3 - phase, y + height - phase),
            ] {
                self.bob(i16(px), i16(py), id, true, Some(u32::from(r[4])))?;
            }
        }
        if !gauge {
            return Ok(());
        }
        let mut gx = i16(x - 23);
        let gy = i16(y + 14); // 402094[4] ->402167
        if ox == 0 && gx < 0 {
            gx = 0;
        }
        self.bob(i16(gx + 9), gy, 0x2a7b, true, None)?;
        let maximum = self.u16(0x495d90 + 52 * 4)?;
        if maximum == 0 {
            return self.stop(0x41a93b, "type4 gauge zero maximum health");
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
        let capacity = w(r, 14) as usize;
        if capacity > 30 {
            return self.stop(0x401a70, "type4 bag exceeds inline slots");
        }
        let mut bx = gx;
        let mut by = gy;
        for slot in 0..capacity {
            if slot == 8 {
                bx = i16(bx - 9);
                by = gy;
            }
            let troop = r[0x4e + slot] == 1;
            self.bob(
                bx,
                i16(by),
                if troop { 0x2a3c } else { 0x2a3b },
                true,
                troop.then_some(u32::from(r[4])),
            )?;
            by += 8;
        }
        // 401c16..401cbe: colonist capacity bonus and the queued count at+78.
        let count =
            4 - (crate::frame_economy::bonus(self.m, u32::from(r[4]), 9, 2, 0)? as u16 as i32);
        bx = i16(bx + 32);
        by = i16(by + 32);
        for slot in 0..count {
            let queued = w(r, 0x78) != 65535 && (slot as u32) < w(r, 0x78);
            self.bob(
                bx,
                by,
                if queued { 0x2a3d } else { 0x2a3b },
                true,
                queued.then_some(u32::from(r[4])),
            )?;
            bx = i16(bx + 8);
        }
        Ok(())
    }
}
