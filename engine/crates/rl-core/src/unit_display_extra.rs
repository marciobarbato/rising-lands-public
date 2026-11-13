//! Additional intact AffDeplaceEngin domains from the P1 a108ea8b census.
//! No scene dispatch: the caller supplies a visible, already ordered unit.
use crate::render::{i16, w, Context, Error, Observation, Renderer, Result};
use crate::{FrameRegisters, MemoriaJogo};
use std::collections::BTreeMap;

/// Full render effects, including the permutation cursor consumed by jitter.
/// Apply writes in order before the next render call; do not change logic RNGs.
#[derive(Debug)]
pub struct Effects {
    pub writes: Vec<(u32, u8)>,
    pub observations: Vec<Observation>,
    pub stop: Option<(u32, String)>,
}

/// Additional movement-body entry. EAX/EDX are signed low-word screen positions,
/// EBX is the low-word unit index, exactly as AffEngin41591f ->412390.
/// The typed display API has no scratch-register result. Native preserves
/// ECX/ESI/EDI/EBP and returns with ESP+4; the differential checks these registers.
pub fn display(renderer: &mut Renderer, m: &MemoriaJogo, r: FrameRegisters) -> Result<Effects> {
    let mut c = Context {
        background: &mut renderer.saved_background,
        text_boundaries: &mut renderer.recorded_text_boundaries,
        text_checked: 0,
        tick: renderer.tick,
        m,
        overrides: BTreeMap::new(),
        surface: &mut renderer.surface,
        cache: &mut renderer.cache,
        assets: &renderer.assets,
        observations: Vec::new(),
        audit: renderer.audit,
        clip: [
            m.u16(0x49a020)? as i32,
            m.u16(0x49a01c)? as i32,
            m.u16(0x49a022)? as i32,
            m.u16(0x49a01e)? as i32,
        ],
        camera: u32::from(m.u8(0x508041)?),
    };
    let index = r.ebx & 65535;
    let record = m.read(m.u32(0x4e19a0)?.wrapping_add(276 * index), 276)?;
    let result = c.unit_display_extra(i16(r.eax as i32), i16(r.edx as i32), index, record);
    let stop = match result {
        Ok(()) => None,
        Err(Error::Stop(a, reason)) => Some((a, reason)),
        Err(e) => return Err(e),
    };
    Ok(Effects {
        writes: c.overrides.into_iter().collect(),
        observations: c.observations,
        stop,
    })
}

/// Disjoint extension selector: existing proven body domains remain in unit().
/// Native has no phase<count check: state19 can inherit an attack phase.
/// AffEngin 0x41591f state dispatch: the states it hands to AffDeplaceEngin
/// 0x412390 whatever the kind and owner (1/22/24 only without an attack
/// phase: 0x415990 / 0x4159a6 / 0x4159fb; kind 16 never attacks in 1/24).
fn deplace_routed(r: &[u8]) -> bool {
    matches!(r[2], 2 | 3 | 6..=9 | 13 | 16..=21 | 23 | 30)
        || matches!(r[2], 1 | 22 | 24) && r[0x8a] == 0
        || r[1] == 16 && matches!(r[2], 1 | 24)
}

pub fn supports(r: &[u8]) -> bool {
    if r.len() != 276 || !matches!(r[1], 0..=17 | 20..=25) {
        return false;
    }
    // The routed states the per-kind lists below did not admit: spying
    // thieves (23), carrying loops (16..20) of non-workers, 3/6/8/9/30 and
    // 22/24 without a phase, for every owner; and neutral (owner 200)
    // units in any routed state. 412390 reads the state only for the order
    // markers (0x412611 skips state 17), so the body is the common one.
    // Owner-0 kinds 0..9 in their walking states keep the proven route in
    // render_units (AffEngin's overlay checks).
    if deplace_routed(r) {
        let uncommon = matches!(r[2], 3 | 6 | 8 | 9 | 23 | 30)
            || matches!(r[2], 22 | 24) && r[0x8a] == 0 && !(r[1] == 4 && r[2] == 24)
            || matches!(r[2], 16..=18 | 20) && !matches!(r[1], 5..=7);
        if uncommon || r[0] == 200 {
            return true;
        }
    }
    if r[1] == 4 {
        // 4159a6: states 1 and 24 without an attack phase reach 412390.
        return matches!(r[2], 2 | 7 | 13 | 19 | 21) || matches!(r[2], 1 | 24) && r[0x8a] == 0;
    }
    // Types11..15 (flyers/vehicles) have no other body; every routed movement
    // state reaches the412390 common path and their own kind block here.
    // Types 2 and 10 (412328 -> 412ece / 412c6a, shared 0x412cbc tail) draw
    // the same body in every state AffEngin routes to AffDeplaceEngin; the
    // only state test inside 412390 (0x412613) is the selected unit's order
    // markers.
    // Kind 17 (AffBallon via 412328[17]) has the same owner-independent routing:
    // 412390 never reads the owner before the body, and the selection marker
    // (table 441004[17] -> 44109f, small 0x2a1e) is gated on the camera only.
    if matches!(r[1], 2 | 3 | 10..=17 | 25) {
        // AffEngin41591f state dispatch: these states reach AffDeplaceEngin;
        // 1/22/24 only without an attack phase (415990/4159a6/4159fb).
        return matches!(r[2], 2 | 3 | 6..=9 | 13 | 16..=21 | 23 | 30)
            || matches!(r[2], 1 | 22 | 24) && r[0x8a] == 0
            // 4159a6: kind 16 in states 1 and 24 never takes the attack body.
            || r[1] == 16 && matches!(r[2], 1 | 24);
    }
    // AffCorpsEngin already handles the four player palettes, including the
    // double owner4 draw. Foreign normal units also reach the hover/info tail.
    if (1..=4).contains(&r[0]) && (matches!(r[2], 2 | 7 | 19) || r[2] == 1 && r[0x8a] == 0) {
        return true;
    }
    // Foreign field workers in the transport-loop states reach the same
    // AffDeplaceEngin body (AffEngin state dispatch 16..20 -> 412390).
    if (1..=4).contains(&r[0]) && matches!(r[1], 5..=7) && matches!(r[2], 16 | 17 | 18 | 20) {
        return true;
    }
    // Foreign kind-9 units in state13 also reach the412390 body (AffEngin
    // state dispatch), like the owner-0 state13 extension in unit().
    if (1..=4).contains(&r[0])
        && matches!(r[1], 0 | 1 | 2 | 5 | 6 | 7 | 8 | 9)
        && matches!(r[2], 13 | 21)
    {
        return true;
    }
    // Types6/9 state19 and the inherited move-frame state19 were missing bodies.
    // The normal non-builder experience tail already exists; only its callers
    // stopped. Keep existing state19/count-valid and builder work bodies routed
    // through their proven implementation.
    matches!(r[1], 6 | 9) && r[2] == 19
        || matches!(r[1], 0 | 1 | 5 | 8) && r[2] == 19 && r[0xa4] >= r[0xa5]
        || r[1] != 7 && r[0xaf] >> 5 != 0 && (matches!(r[2], 2 | 7) || r[2] == 1 && r[0x8a] == 0)
}

impl Context<'_> {
    /// AffBallon0x41209d (kind17) at the AffCorpsEnginDecalage origin, then the
    /// shared AffDeplaceEngin tail (4130b7). Altitude +0xab raises the basket and
    /// the envelope; the ground shadow depends only on the altitude band.
    fn balloon(&mut self, x: i32, y: i32, index: u32, r: &[u8]) -> Result<()> {
        // AffBallon reads no spell effect (+0xdc).
        if r[0] > 4 {
            return self.stop(0x41209d, "AffBallon owner has no player palette");
        }
        let owner = Some(u32::from(r[0]));
        let h = i32::from(r[0xab]);
        let shadow = match h {
            0..=34 => 0x1361,
            35..=39 => 0x1362,
            40..=44 => 0x1363,
            _ => 0x1364,
        };
        self.shadow(i16(x + 30), i16(y + 199), shadow, false)?;
        let (top, body) = if r[0xae] != 0 {
            (
                0x1339 + (u32::from(r[0xa4]) / 3) % 4,
                0x1332 + u32::from(r[0xa4]) % 4,
            )
        } else {
            (
                (0x133d + u32::from(r[0x96])) & 0xffff,
                (0x1336 + u32::from(r[0x94])) & 0xffff,
            )
        };
        self.bob(i16(x + 74), i16(y + 184 - h), top, true, owner)?;
        self.bob(x, i16(y - h), body, true, None)?;
        let frame = self.u32(0x508f7c)? % 20;
        self.bob(i16(x + 96), i16(y + 15 - h), 0x134d + frame, true, owner)?;
        // 413274..4130b7: the tail (info/fatigue) runs at X-0x20 for the balloon.
        self.unit_tail(
            r,
            x + 47 - i32::from(r[0x86] as i8),
            y + 212 - i32::from(r[0x87] as i8),
            index,
            true,
        )
    }
    /// AffCorpsEngin 0x411147 as a whole for unit record `r`: the owner's
    /// passes (owner 0 plain, owners 1..3 in their palette, owner 4 a plain
    /// pass for kinds 20..24 or a palette pass otherwise, then the palette
    /// pass again, the neutral owner 200 plain then greyed), each with the
    /// spell effects of +0xdc (bit 2 mirror: a permutation byte at
    /// 0x4e1c18[0x4e1d18] chooses the flash on odd values; bit 1 terror:
    /// the pass, then greyed), the turret of kinds 15 (0x1172) and 12
    /// (0x1232) after the body in the same paint, then the armour overlay
    /// (bit 3, 0x411277) and the caster's mana (kind 4, 0x411f62), both at
    /// the body's origin shifted back by AffCorpsEnginDecalage when armoured.
    pub(crate) fn corps_unit(&mut self, x: i32, y: i32, id: u32, r: &[u8]) -> Result<()> {
        let owner = r[0];
        if owner > 4 && owner != 200 {
            return self.stop(0x411147, "AffCorpsEngin owner has no player palette");
        }
        let kind = r[1];
        match owner {
            200 => {
                // 0x4115ee: no effect checks.
                self.paint(Paint::Bob, x, y, id)?;
                self.paint(Paint::Grey, x, y, id)?;
                if let Some(t) = turret(r) {
                    self.paint(Paint::Bob, x, y, t)?;
                    self.paint(Paint::Grey, x, y, t)?;
                }
            }
            0 => self.corps_pass(Paint::Bob, x, y, id, r)?,
            4 => {
                // 0x411bf3: kinds 20..24 take the plain pass 0x4116ec first.
                let first = if (20..=24).contains(&kind) {
                    Paint::Bob
                } else {
                    Paint::Player(4)
                };
                self.corps_pass(first, x, y, id, r)?;
                self.corps_pass(Paint::Player(4), x, y, id, r)?;
            }
            p => self.corps_pass(Paint::Player(u32::from(p)), x, y, id, r)?,
        }
        let (mut bx, mut by) = (x, y);
        if r[0xdc] >> 3 & 1 != 0 {
            if kind > 25 {
                return self.stop(0x44106c, "AffCorpsEngin armour size of kind > 25");
            }
            let (ox, oy) = DECALAGE[usize::from(kind)];
            bx = i16(bx + ox);
            by = i16(by + oy);
            let phase = self.u32(0x508f7c)? & 0x3f;
            let (base, d) = if matches!(kind, 3 | 11..=16 | 25) {
                (0x2b3d, 0x2a)
            } else {
                (0x2b7d, 0xe)
            };
            self.shadow(i16(bx - d), i16(by - d), (base + phase) & 65535, true)?;
        }
        if kind == 4 {
            // 0x411ff8: the silhouettes at the body origin plus (38, 8).
            self.caster_mana(r, bx + 40, by + 32)?;
        }
        Ok(())
    }
    fn corps_pass(&mut self, paint: Paint, x: i32, y: i32, id: u32, r: &[u8]) -> Result<()> {
        let mirror = r[0xdc] >> 2 & 1 != 0;
        let terror = r[0xdc] >> 1 & 1 != 0;
        let mut effect = mirror || terror;
        if mirror {
            let cursor = self.u8(0x4e1d18)?;
            let odd = self.u8(0x4e1c18 + cursor)? % 2 != 0;
            self.put("rng_u8", 0x4e1d18, &[(cursor as u8).wrapping_add(1)])?;
            effect = odd;
        }
        let t = turret(r);
        if !effect {
            self.paint(paint, x, y, id)?;
            if let Some(t) = t {
                self.paint(paint, x, y, t)?;
            }
        } else if terror {
            self.paint(paint, x, y, id)?;
            if let Some(t) = t {
                self.paint(paint, x, y, t)?;
            }
            self.paint(Paint::Grey, x, y, id)?;
            if let Some(t) = t {
                self.paint(Paint::Grey, x, y, t)?;
            }
        } else {
            self.paint(Paint::Flash, x, y, id)?;
            if let Some(t) = t {
                self.paint(Paint::Flash, x, y, t)?;
            }
        }
        Ok(())
    }
    fn paint(&mut self, paint: Paint, x: i32, y: i32, id: u32) -> Result<()> {
        match paint {
            Paint::Bob => self.bob(x, y, id, true, None),
            Paint::Player(p) => self.bob(x, y, id, true, Some(p)),
            Paint::Grey => {
                self.observe("PasteGrisCmp", 0x45bcb0, &[x, y, id as i32, 1]);
                self.remap_sprite(x, y, id, 0x4e24c4)
            }
            Paint::Flash => self.shadow(x, y, id, true),
        }
    }
    /// Full412390 call for the small normal movement family. This is also the
    /// runtime wiring point: call only after supports(), before unit's guards.
    pub(crate) fn unit_display_extra(
        &mut self,
        x: i32,
        y: i32,
        index: u32,
        r: &[u8],
    ) -> Result<()> {
        let kind = r[1];
        if !matches!(kind, 0..=17 | 20..=25)
            || !(deplace_routed(r)
                || matches!(r[2], 2 | 7 | 16 | 17 | 19)
                || matches!(kind, 5..=7) && matches!(r[2], 18 | 20)
                || matches!(kind, 0 | 1 | 2 | 4 | 5 | 6 | 7 | 8 | 9) && matches!(r[2], 13 | 21)
                || r[2] == 1 && r[0x8a] == 0
                || kind == 16 && matches!(r[2], 1 | 24)
                || kind == 4 && r[2] == 24 && r[0x8a] == 0
                || matches!(kind, 2 | 3 | 10..=17 | 25)
                    && (matches!(r[2], 3 | 6 | 8 | 9 | 13 | 18 | 20 | 21 | 23 | 30)
                        || matches!(r[2], 22 | 24) && r[0x8a] == 0)
                || crate::transport_unit_render::supports(r))
        {
            return self.stop(
                0x41591f,
                "unit-display-extra requires a small normal movement body",
            );
        }
        self.deplace_body(x, y, index, r)
    }

    /// AffDeplaceEngin 0x412390 body without AffEngin's state dispatch, as a
    /// direct caller (AffRentrerHangar 0x402da2) reaches it: the body reads the
    /// state only for the selected unit's order markers.
    pub(crate) fn deplace_body(&mut self, x: i32, y: i32, index: u32, r: &[u8]) -> Result<()> {
        let kind = r[1];
        if !matches!(kind, 0..=17 | 20..=25) {
            return self.stop(0x412328, "AffDeplaceEngin kind outside the body table");
        }
        // Original410fcc low-word offsets;412328 dispatch -> matching sprite bases.
        let (base, ax, ay): (u32, i32, i32) = match kind {
            0 => (2082, 40, 22),
            1 => (2290, 23, 9),
            // Original412328[2] ->412ece and410f64[2] ->410ff8.
            2 => (3202, 32, 16),
            // Original412328[4] ->412f1c and410f64[4] ->410fef.
            4 => (2674, 40, 32),
            // Original412328[11..15] ->4130bf/412f69/413280/413144/4131ca and
            // 410f64[11..15] ->411049/41106e/411052/411040/411077.
            11 => (0x1062, 41, 52),
            12 => (0x1212, 52, 48),
            13 => (0x10d2, 55, 56),
            14 => (0x12d2, 42, 45),
            15 => (0x1152, 49, 66),
            5 => (if w(r, 10) == 0 { 1602 } else { 1698 }, 11, 11),
            6 => (if w(r, 10) == 0 { 2978 } else { 3042 }, 16, 20),
            7 => (if w(r, 10) == 0 { 1346 } else { 1474 }, 26, 26),
            8 => (2450, 32, 30),
            9 => (1826, 19, 14),
            // Original412328[10] ->412c6a (body 0x7a2, shadow 0x2a20) and
            // 410f64[10] ->41102e (0x18, 0x0c).
            10 => (0x7a2, 24, 12),
            // 412328[21] ->413381; offsets410f64[21] ->4110ad.
            // 412328[20] ->413306; offsets410f64[20] ->4110a4.
            20 => (0x1575, 46, 54),
            21 => (0x13f5, 23, 15),
            // 412328[3] ->412d99, 412328[25] ->412e54: shadow bank then body at
            // the offsets410f64[3/25] ->411037.
            3 => (0xd62, 45, 56),
            25 => (0xee2, 45, 56),
            // 412328[16] ->412fee; offsets410f64[16] ->41105b.
            16 => (0x1365, 78, 54),
            // 412328[17] ->413252 (AffBallon41209d); offsets410f64[17] ->411064.
            17 => (0, 79, 212),
            // 412328[22] ->413504; offsets410f64[22] ->4110b6.
            22 => (0x16f5, 54, 54),
            // 412328[23] ->41340f; offsets410f64[23] ->411092.
            23 => (0x1875, 22, 16),
            // 412328[24] ->41348a; offsets410f64[24] ->411089 (0x20, 0x20).
            24 => (0x19f5, 32, 32),
            _ => unreachable!(),
        };
        let (mut px, mut py) = (
            i16(x + i32::from(r[0x86] as i8)),
            i16(y + i32::from(r[0x87] as i8)),
        );
        // Native4123f7..4124a9 draws once from the byte permutation, not Random.
        // X changes only for facings5/13. The cursor store precedes the remainder.
        if w(r, 8) != w(r, 0x7e) && r[0x99] != 0 && r[0x86] != 0 && r[0x87] != 0 {
            let cursor = self.u8(0x4e1d18)?;
            let delta = self.u8(0x4e1c18 + cursor)? % u32::from(r[0x99]);
            self.put("rng_u8", 0x4e1d18, &[(cursor as u8).wrapping_add(1)])?;
            if matches!(r[0x91], 5 | 13) {
                px = i16(px + delta as i32);
            } else {
                py = i16(py + delta as i32);
            }
        }
        if r[0xbd] != 0 && u32::from(r[0]) == self.camera {
            // 44106c (table441004): kinds3/11..16/25 use the large marker2a1f
            // at(-32,-32); the others the small marker2a1e in place.
            if matches!(kind, 3 | 11..=16 | 25) {
                self.bob(i16(px - 32), i16(py - 32), 0x2a1f, true, None)?;
            } else {
                self.bob(px, py, 0x2a1e, true, None)?;
            }
            if kind == 14 {
                // 41252c..41260c: the bomb queue markers.
                self.bomb_queue(r, index)?;
            }
            self.order_markers(r)?;
        }
        // Arithmetic is deliberately not normalized/clamped to count or facing.
        // The actual caller can retain phase9 while state19 uses move count8.
        let frame = if (11..=16).contains(&kind) {
            // Intact413144..41318c (and the kind11/12/13/15 twins): even facings
            // hold one frame, odd facings animate; signed truncating halves.
            let dir = i32::from(r[0x91]);
            let n = (dir / 2) * i32::from(r[0xa5]) + (dir - 1) / 2 + i32::from(r[0xa4]) * (dir % 2);
            n as u32
        } else {
            u32::from(r[0x91])
                .div_ceil(2)
                .wrapping_sub(1)
                .wrapping_mul(u32::from(r[0xa5]))
                .wrapping_add(u32::from(r[0xa4]))
        };
        let id = base.wrapping_add(frame) & 65535;
        if kind == 17 {
            return self.balloon(i16(px - ax), i16(py - ay), index, r);
        }
        if kind == 16 {
            // 41303e..41309b: shadow bank 0x13ad at the ground, the body raised
            // by the flying layer +0xab plus 20.
            self.shadow(
                i16(px - ax),
                i16(py - ay),
                0x13ad_u32.wrapping_add(frame) & 65535,
                false,
            )?;
            self.corps_unit(i16(px - ax), i16(py - ay - 20 - i32::from(r[0xab])), id, r)?;
            return self.unit_tail(
                r,
                px - i32::from(r[0x86] as i8),
                py - i32::from(r[0x87] as i8),
                index,
                true,
            );
        }
        if matches!(kind, 3 | 25) {
            let bank: u32 = if kind == 3 { 0xe22 } else { 0xfa2 };
            self.shadow(
                i16(px - ax),
                i16(py - ay),
                bank.wrapping_add(frame) & 65535,
                false,
            )?;
        } else if (11..=15).contains(&kind) {
            // Intact413194..4131b5 and twins: per-frame shadow bank via
            // PasteOmbreCmp at the body position, before AffCorpsEngin.
            let bank: u32 = match kind {
                11 => 0x109a,
                12 => 0x1272,
                13 => 0x1112,
                14 => 0x1302,
                _ => 0x11b2,
            };
            self.shadow(
                i16(px - ax),
                i16(py - ay),
                bank.wrapping_add(frame) & 65535,
                false,
            )?;
        } else if matches!(kind, 20..=24) {
            // The monster has its own directional shadow bank; the original
            // draws it before the shared AffCorpsEngin owner/effect dispatch.
            self.shadow(
                i16(px - ax),
                i16(py - ay),
                (match kind {
                    20 => 0x1635u32,
                    21 => 0x14b5,
                    22 => 0x17b5,
                    23 => 0x1935,
                    // 41348a: PasteOmbreCmp bank 0x1ab5 + frame, body 0x19f5 + frame.
                    _ => 0x1ab5,
                })
                .wrapping_add(frame)
                    & 65535,
                false,
            )?;
        } else {
            self.shadow(i16(px + 2), i16(py + 26), 10784, false)?;
        }
        if r[0] > 4 && r[0] != 200 {
            return self.stop(0x411147, "unit-display-extra body owner dependency");
        }
        if self.assets.sprite(id).is_err() {
            return self.stop(
                0x411147,
                format!("unit-display-extra sprite {id} outside original catalog"),
            );
        }
        // AffCorpsEngin draws the turret of kinds 12/15 and the caster's mana.
        self.corps_unit(i16(px - ax), i16(py - ay), id, r)?;
        // Reuse the already implemented info/fatigue/experience tail. Its maxHP
        // and bag bounds remain addressed dependencies, never widened here.
        self.unit_tail(
            r,
            px - i32::from(r[0x86] as i8),
            py - i32::from(r[0x87] as i8),
            index,
            true,
        )
    }
}

impl Context<'_> {
    /// AffCorpsEngin 0x411f62..0x412093 for kind 4: four additive mana
    /// silhouettes at the body origin + (38, 8). EnginGetMaxMana 0x448270
    /// returns the low-byte bonus plus ten.
    pub(crate) fn caster_mana(&mut self, r: &[u8], px: i32, py: i32) -> Result<()> {
        let maximum = (crate::frame_economy::bonus(self.m, u32::from(r[0]), 4, 12, 0)? as u8)
            .wrapping_add(10);
        let mana = u32::from(r[0xda]);
        if mana != 0 {
            let divisor = u32::from(maximum) / 5;
            if divisor == 0 {
                return self.stop(0x411fc8, "type4 mana silhouette zero divisor");
            }
            let base = 0x2c7d + ((mana - 1) / divisor) * 128;
            let phase = self.u32(0x508f7c)? & 31;
            for offset in [0, 32, 64, 96] {
                self.shadow(
                    i16(px - 2),
                    i16(py - 24),
                    (base + phase + offset) & 65535,
                    true,
                )?;
            }
        }
        Ok(())
    }
}

impl Context<'_> {
    /// The own selected kind-14 bomb queue +0xc4 (+0xd8 entries): marker
    /// 0x1ccf on every queued cell inside the view. Movement path
    /// 41252c..41260c, attack path 41383d..413924.
    pub(crate) fn bomb_queue(&mut self, r: &[u8], index: u32) -> Result<()> {
        let (ox, oy, vw, vh) = (
            self.u16(0x508720)?,
            self.u16(0x508722)?,
            self.u16(0x508724)?,
            self.u16(0x508726)?,
        );
        let pool = self.u32(0x4e19a0)?;
        for i in 0..w(r, 0xd8) {
            let c = self.u16(pool.wrapping_add(276 * index + 0xc4 + 2 * i))?;
            let (cx, cy) = (c & 255, c >> 8);
            if cx >= ox && cx < ox + vw && cy >= oy && cy < oy + vh {
                self.bob(
                    i16(((cx - ox) as i32) * 32),
                    i16(((cy - oy) as i32) * 32),
                    0x1ccf,
                    true,
                    None,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Paint {
    Bob,
    Player(u32),
    Grey,
    Flash,
}

/// EnginGetTourDir 0x440e31 turret sprite of kinds 15 and 12.
fn turret(r: &[u8]) -> Option<u32> {
    let base: u32 = match r[1] {
        15 => 0x1172,
        12 => 0x1232,
        _ => return None,
    };
    Some(base.wrapping_add(u32::from(r[0x94]).wrapping_sub(1).wrapping_mul(2)) & 65535)
}

/// AffCorpsEnginDecalage 0x410fcc (table 0x410f64), as positive (x, y) shifts.
pub(crate) const DECALAGE: [(i32, i32); 26] = [
    (40, 22),
    (23, 9),
    (32, 16),
    (45, 56),
    (40, 32),
    (11, 11),
    (16, 20),
    (26, 26),
    (32, 30),
    (19, 14),
    (24, 12),
    (41, 52),
    (52, 48),
    (55, 56),
    (42, 45),
    (49, 66),
    (78, 54),
    (79, 212),
    (36, 48),
    (16, 16),
    (46, 54),
    (23, 15),
    (54, 54),
    (22, 16),
    (32, 32),
    (45, 56),
];
