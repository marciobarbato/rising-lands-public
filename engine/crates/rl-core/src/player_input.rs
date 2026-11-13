//! InputGame 0x429278 and its literal leaf/packet flow (AI/runtime input revision).
use crate::{
    frame::stop,
    player::{
        self, alive, count, first, selected_structure as sel_structure, selections, unit, Memory,
    },
    MemoriaJogo, Result,
};
#[path = "player_pages.rs"]
mod pages;
/// IconeScroll 0x45da13: the original tests and additions use different widths.
pub(crate) fn icon_scroll(m: &mut MemoriaJogo, bag: u32, delta: u32) -> Result<()> {
    let delta = delta as u16;
    if m.w(bag + 4)? != 0 && (delta as i16) < 0 {
        m.sw(bag + 4, m.w(bag + 4)?.wrapping_add(u32::from(delta)))?;
    }
    if m.w(bag + 4)? + 4 < m.w(bag)? && (delta as i16) > 0 {
        m.sw(bag + 4, m.w(bag + 4)?.wrapping_add(u32::from(delta)))?;
    }
    Ok(())
}
pub(crate) fn selection_leaf(m: &MemoriaJogo, name: &str, a: u32, d: u32, b: u32) -> Result<u32> {
    pages::leaf(m, name, a, d, b)
}

const X: u32 = 0x5099a8;
const Y: u32 = 0x5099aa;
const Z: u32 = 0x5099ac;
const AE: u32 = 0x5099ae;
const AF: u32 = 0x5099af;
const B0: u32 = 0x5099b0;
const B1: u32 = 0x5099b1;
const PAGE: u32 = 0x4e05ae;
const MODE: u32 = 0x496207;
const DRAG: u32 = 0x4e19b0;
const VIEW: u32 = 0x508720;
fn yes(v: bool) -> u32 {
    u32::from(v)
}
fn kind(m: &MemoriaJogo, u: u32) -> Result<u32> {
    m.b(unit(m, u)? + 1)
}
fn owner(m: &MemoriaJogo, u: u32) -> Result<u32> {
    m.b(unit(m, u)?)
}
fn mirror(m: &MemoriaJogo, u: u32) -> Result<bool> {
    Ok(m.b(unit(m, u)? + 0xdc)? & 4 != 0)
}
fn obj(m: &MemoriaJogo, c: u32) -> Result<u32> {
    m.b(m.d(0x507d74)? + (c & 65535))
}
fn tile(m: &MemoriaJogo, c: u32) -> Result<u32> {
    m.w(m.d(0x507d70)? + 2 * (c & 65535))
}
fn discovered(m: &MemoriaJogo, c: u32, p: u32) -> Result<bool> {
    Ok(m.b(m.d(0x507d78 + 4 * (p & 255))? + (c & 65535))? & 16 == 0)
}
fn lit(m: &MemoriaJogo, c: u32, p: u32) -> Result<bool> {
    Ok(m.b(m.d(0x507d88 + 4 * (p & 255))? + (c & 65535))? & 16 == 0)
}
fn sack(m: &MemoriaJogo, c: u32) -> Result<bool> {
    Ok((0x42..=0x44).contains(&obj(m, c)?))
}
fn sack_kind(m: &MemoriaJogo, c: u32) -> Result<u32> {
    let o = obj(m, c)?;
    Ok(if (0x42..=0x44).contains(&o) {
        o - 0x42
    } else {
        255
    })
}
fn field(m: &MemoriaJogo, c: u32) -> Result<bool> {
    Ok((0x1d..0x35).contains(&obj(m, c)?))
}
fn mine(m: &MemoriaJogo, c: u32) -> Result<bool> {
    Ok(matches!(tile(m, c)?, 0x251 | 0x252 | 0x255 | 0x256))
}
fn collision(m: &MemoriaJogo, c: u32, mask: u32) -> Result<bool> {
    Ok(m.b(m.d(0x507da8)? + (c & 65535))? & mask & 255 != 0)
}
fn obstacle(m: &MemoriaJogo, c: u32) -> Result<bool> {
    let t = tile(m, c)?;
    Ok((t < 0x2be && m.b(m.d(0x507db0)? + t)? != 0) || (1..=11).contains(&obj(m, c)?))
}
fn army(t: u32) -> bool {
    t <= 3 || t == 8 || (11..=12).contains(&t) || (15..=16).contains(&t) || (18..=25).contains(&t)
}
fn land(m: &MemoriaJogo, p: u32) -> Result<bool> {
    for u in selections(m, p)? {
        if alive(m, u)? && (16..=17).contains(&kind(m, u)?) {
            return Ok(false);
        }
    }
    Ok(true)
}
fn can_attack(m: &MemoriaJogo, p: u32, mode: u32, target: u32) -> Result<bool> {
    Ok(pages::leaf(m, "EnginSelectPeutAttaquer", p, mode, target)? & 255 != 0)
}
fn can_enter(m: &MemoriaJogo, p: u32, mode: u32, target: u32) -> Result<bool> {
    for u in selections(m, p)? {
        if alive(m, u)? && mirror(m, u)? {
            return Ok(false);
        }
    }
    if mode == 1 {
        let t = kind(m, target)?;
        if t == 16 || !(11..=17).contains(&t) {
            return Ok(false);
        }
        return crate::chain_entry::can_enter(m, p, mode, target);
    }
    if mode == 3 {
        return crate::chain_entry::can_enter(m, p, mode, target);
    }
    Ok(false)
}
fn find(m: &mut MemoriaJogo, c: u32) -> Result<u32> {
    crate::frame_movement::call(m, "ChercheIndiceEngin", &[c])
}
pub fn zone(m: &MemoriaJogo) -> Result<u32> {
    let (x, y) = (m.w(X)? as i32, m.w(Y)? as i32);
    let mut z = m.w(0x50a9b4)?;
    while z <= m.w(0x50a9b6)? {
        let r = 0x5099b4 + 16 * z;
        if m.d(r)? as i32 <= x
            && m.d(r + 4)? as i32 <= y
            && x <= m.d(r + 8)? as i32
            && y <= m.d(r + 12)? as i32
        {
            return Ok(z);
        }
        z = (z + 1) & 65535;
        if z == 0 {
            return stop("MouseZone wrapping zone range", 0x479cac);
        }
    }
    Ok(0)
}
pub fn scroll(m: &mut MemoriaJogo, z: u32, steps: u32) -> Result<()> {
    for _ in 0..(steps & 65535) {
        for (a, dir, limit) in [
            (
                VIEW,
                match z {
                    1 | 7 | 8 => -1,
                    3..=5 => 1,
                    _ => 0,
                },
                0x497168,
            ),
            (
                VIEW + 2,
                match z {
                    1..=3 => -1,
                    5..=7 => 1,
                    _ => 0,
                },
                0x49716a,
            ),
        ] {
            let v = m.w(a)?;
            if dir < 0 && v != 0 {
                m.sw(a, v - 1)?
            } else if dir > 0 && m.w(a + 4)? + v < m.w(limit)? {
                m.sw(a, v + 1)?
            }
        }
    }
    Ok(())
}
/// WndProc numeric pad `-` (0x4183fc..0x418483) and `+` (0x418374..0x4183f7):
/// the margin at 0x508f90/91 grows or shrinks one step, then InitGameZone and
/// SetClipping for the new playfield. `-` also recentres the minimap frame
/// and raises 0x508f92, which makes the next AffGame clear both surfaces.
fn margin_key(m: &mut MemoriaJogo, grow: bool) -> Result<()> {
    if grow {
        m.sb(0x508f92, 1)?;
    }
    let step: u32 = if grow { 1 } else { 255 };
    let margin = m.b(0x508f90)?.wrapping_add(step) & 255;
    m.sb(0x508f90, margin)?;
    m.sb(0x508f91, margin)?;
    if grow {
        crate::chain_vision::extra(m, "MiniMapRecentre", Default::default())?;
    }
    crate::frame_core::call(m, "InitGameZone", &[1, 0])?;
    let inset = (margin as u8 as i8 as i32 as u32).wrapping_shl(5) & 65535;
    let right = (m.w(0x508724)? << 5).wrapping_sub(1).wrapping_sub(inset) & 65535;
    let bottom = (m.w(0x508726)? << 5).wrapping_sub(1).wrapping_sub(inset) & 65535;
    crate::frame_core::call(m, "SetClipping", &[inset, inset, right, bottom])?;
    Ok(())
}

/// ResetMode 0x40f645, game-memory side. DDExit/DDInit, DDSetPalette, the two
/// surface clears, PLATGAME.PIE and the page flip belong to the host; DDInit's
/// effects on game memory (screen size, view rows and columns, 0x4e17a0) are
/// written here. Then InitGameZone(1, 0), the margin (0 at 640x480, the saved
/// 0x508f91 at 800x600), the view clip 0x49a01c..0x49a022 and the HUD latch.
fn reset_mode(m: &mut MemoriaJogo, high: bool) -> Result<()> {
    let (w, h, rows) = if high { (800, 600, 17) } else { (640, 480, 13) };
    m.sw(0x49a018, w)?;
    m.sw(0x49a01a, h)?;
    m.sw(0x508726, rows)?;
    m.sw(0x508724, w / 32)?;
    m.sd(0x4e17a0, 800)?;
    crate::frame_core::call(m, "InitGameZone", &[1, 0])?;
    let margin = if high { m.b(0x508f91)? } else { 0 };
    m.sb(0x508f90, margin)?;
    let inset = ((margin as u8 as i8 as i32) << 5) as u32 & 65535;
    let right = (m.w(0x508724)? << 5).wrapping_sub(1).wrapping_sub(inset) & 65535;
    let bottom = (m.w(0x508726)? << 5).wrapping_sub(1).wrapping_sub(inset) & 65535;
    m.sw(0x49a020, inset)?;
    m.sw(0x49a01c, inset)?;
    m.sw(0x49a022, right)?;
    m.sw(0x49a01e, bottom)?;
    m.sb(0x4e05a2, 1)
}

pub fn wnd(m: &mut MemoriaJogo, msg: [u32; 3]) -> Result<(u32, u32)> {
    let [message, mut wp, lp] = msg;
    match message {
        0x200 => {
            m.sw(X, lp)?;
            m.sw(Y, lp >> 16)?
        }
        0x201 => m.sb(AE, 1)?,
        0x202 => m.sb(AF, 1)?,
        0x203 => {
            wp = m.w(Y)?;
            if wp < m.w(VIEW + 6)? * 32 {
                m.sb(B0, 1)?
            } else {
                m.sb(AE, 1)?
            }
        }
        0x204 => m.sb(B1, 1)?,
        0x205 => m.sb(0x5099b2, 1)?,
        0x100 => {
            if m.b(0x4e2f78)? == 0x12 {
                match wp {
                    0x31..=0x39 => {
                        m.sb(0x49a05a, wp - 0x30)?;
                        wp = (wp & !255) | ((wp - 0x30) & 255)
                    }
                    0x70..=0x73 => m.sb(0x497284, wp - 0x6f)?,
                    0x6b => {
                        if m.b(0x508f90)? as u8 as i8 > 0 && m.w(0x49a018)? == 800 {
                            margin_key(m, false)?;
                        }
                    }
                    0x6d => {
                        if (m.b(0x508f90)? as u8 as i8) < 3 && m.w(0x49a018)? == 800 {
                            margin_key(m, true)?;
                        }
                    }
                    0x7a => {
                        // 0x418488 F11: ResetMode(hwnd, width == 640), then MiniMapRecentre.
                        let high = m.w(0x49a018)? == 640;
                        reset_mode(m, high)?;
                        crate::chain_vision::extra(m, "MiniMapRecentre", Default::default())?;
                    }
                    _ => {}
                }
            }
        }
        0x101 => {
            if wp == 0x74 && m.b(0x4e0730)? == 0 {
                wp = (m.b(0x49a05b)? + 1) % 4;
                m.sb(0x49a05b, wp)?
            } else if wp == 9 {
                m.sb(0x497285, yes(m.b(0x497285)? == 0))?;
                crate::frame_core::init_zone(m, 1)?;
                return Ok((0x1f, m.w(0x49a018)?));
            }
        }
        _ => return stop("WndProc message", 0x4181b5),
    }
    Ok((wp, lp))
}
pub fn bag(m: &MemoriaJogo, u: u32) -> Result<u32> {
    let r = unit(m, u)?;
    Ok(m.d(m.d(0x4e19b8)? + 4 * m.b(r)?)?
        .wrapping_add(104 * m.b(r + 1)?))
}
pub fn icon_zone(m: &MemoriaJogo, x: u32, y: u32) -> Result<u32> {
    let b = if m.w(0x49a018)? == 640 { 420 } else { 548 };
    let (x, y) = (x & 65535, y & 65535);
    for i in 0..4 {
        let a = 31 + 60 * i;
        if x >= a && a + 58 >= x && y >= b && b + 50 >= y {
            return Ok(i);
        }
    }
    if (8..=30).contains(&x) && y >= b && y <= b + 52 {
        return Ok(8);
    }
    if (272..=294).contains(&x) && y >= b && y <= b + 52 {
        return Ok(9);
    }
    Ok(65535)
}
fn icon(m: &mut MemoriaJogo, s: u32, id: u32, active: u32, extra: u32) -> Result<()> {
    let a = 0x4e2410 + 6 * s;
    if [m.w(a)?, m.w(a + 2)?, m.w(a + 4)?] != [id & 65535, active & 65535, extra & 65535] {
        m.sb(0x4e05a2, 1)?
    }
    m.sw(a, id)?;
    m.sw(a + 2, active)?;
    m.sw(a + 4, extra)
}
fn cost(m: &MemoriaJogo, t: u32) -> Result<[u32; 3]> {
    let a = 0x495d98 + 52 * (t & 255);
    Ok([m.w(a)?, m.w(a + 2)?, m.w(a + 4)?])
}
pub fn icons(m: &mut MemoriaJogo) -> Result<u32> {
    let p = m.b(0x508041)?;
    if count(m, p)? == 1 {
        m.sb(0x496d48, 0)?;
        let a = bag(m, first(m, p)?)?;
        fill_icons(m, a, 1)?;
        return Ok(0);
    }
    let selected = sel_structure(m, p)?;
    if selected != 65535 {
        let t = m.b(player::structure(selected) + 8)?;
        fill_icons(m, 0x4dd6e8 + 0x8f0 * p + 104 * t, 3)?;
        m.sb(0x496d48, 1)?;
        return Ok(0);
    }
    for i in 0..10 {
        icon(m, i, 0x29b1, 1, 0)?
    }
    m.sb(0x496d48, 255)?;
    Ok(10)
}
pub fn fill_icons(m: &mut MemoriaJogo, a: u32, mode: u32) -> Result<()> {
    let (scroll, total, cap) = (m.w(a + 4)?, m.w(a)?, m.w(a + 2)?);
    icon(m, 8, if scroll == 0 { 0x29b1 } else { 0x29cf }, 1, 0)?;
    icon(
        m,
        9,
        if scroll + 4 >= total { 0x29b1 } else { 0x29cf },
        1,
        0,
    )?;
    let (mut slot, mut k) = (0, scroll);
    while k & 65535 < cap {
        let t = m.b(a + 2 * (k & 65535) + 6)?;
        match m.b(a + 0x42 + (k & 65535))? {
            0 => icon(m, slot, 0x29b1, 1, 0)?,
            2 => icon(m, slot, m.w(a + 2 * (k & 65535) + 6)? + 0x29f0, 1, 0)?,
            3 => {
                // 0x45dec6: in a selected structure's panel (mode 3) a structure
                // already producing (BatimentEstOccupe 0x422fa5) shows it inactive.
                let p = m.b(0x508041)?;
                let producing = mode == 3 && {
                    let s = sel_structure(m, p)?;
                    s != 65535 && m.d(player::structure(s) + 0x8c)? != 0
                };
                let ok = if producing {
                    false
                } else {
                    let c = cost(m, t)?;
                    crate::player_orders::resources(m, p, c[0], c[1], 0)?
                };
                icon(m, slot, 0x29b9 + t, yes(ok), 0)?
            }
            4 if mode == 1 => {
                crate::spell_order_acceptance::caster_icon(
                    m,
                    first(m, m.b(0x508041)?)? as u16,
                    t as u8,
                    slot as u16,
                )?;
            }
            4 => {
                // 0x45e081: research entry of a selected structure: the running
                // item shows its progress, another running item blocks, otherwise
                // the cost (GetCoutApprentissage 0x4746fe) decides.
                let p = m.b(0x508041)?;
                let s = sel_structure(m, p)?;
                let a = player::structure(s);
                let id = 0x29ea + t;
                if s != 65535 && m.b(a + 0x7b)? == t {
                    icon(m, slot, id, 0, crate::chain_recruit::status(m, s, t)?)?;
                } else if s != 65535 && m.d(a + 0x8c)? != 0 {
                    icon(m, slot, id, 0, 0)?;
                } else {
                    let c = 0x497176 + 46 * t;
                    let ok = crate::player_orders::resources(m, p, m.w(c + 2)?, m.w(c)?, 0)?;
                    icon(m, slot, id, yes(ok), 0)?;
                }
            }
            6 => {
                let p = m.b(0x508041)?;
                let selected = sel_structure(m, p)?;
                let id = 0x29cf + if t == 25 { 18 } else { t };
                let (active, status) =
                    if selected != 65535 && m.b(player::structure(selected) + 0x7b)? == t {
                        (0, crate::chain_recruit::status(m, selected, t)? & 65535)
                    } else if selected != 65535 && m.d(player::structure(selected) + 0x8c)? != 0 {
                        (0, 0)
                    } else {
                        let r = 0x4965ae + 50 * t;
                        (
                            yes(crate::player_orders::resources(
                                m,
                                p,
                                m.w(r + 2)?,
                                m.w(r + 6)?,
                                m.w(r + 4)?,
                            )?),
                            0,
                        )
                    };
                icon(m, slot, id, active, status)?;
            }
            // 0x45e696: item entry. A selected kind-14 unit with no mana
            // (EnginGetType0x442025 / EnginGetMana0x44828b on the first selected
            // unit, unit panel only) shows it inactive; otherwise active.
            7 => {
                let inactive = mode == 1 && {
                    let r = unit(m, first(m, m.b(0x508041)?)?)?;
                    m.b(r + 1)? == 14 && m.b(r + 0xda)? == 0
                };
                icon(m, slot, 0x29e2 + t, yes(!inactive), 0)?
            }
            _ => {}
        }
        slot += 1;
        if slot == 4 {
            return Ok(());
        }
        k = k.wrapping_add(1)
    }
    for s in slot..4 {
        icon(m, s, 0x29b1, 1, 0)?
    }
    Ok(())
}
struct Input {
    info: u32,
    info2: u32,
    flag: bool,
    spell_tooltip_costs: Option<[u16; 3]>,
}
impl Input {
    fn new(spell_tooltip_costs: Option<[u16; 3]>) -> Self {
        Self {
            info: 0,
            info2: 65535,
            flag: true,
            spell_tooltip_costs,
        }
    }
    fn ord(&self, m: &mut MemoriaJogo, p: u32, tag: u32, words: [u32; 4], site: u32) -> Result<()> {
        player::order(m, p, tag, words, site)?;
        Ok(())
    }
    fn core(
        &self,
        m: &mut MemoriaJogo,
        name: &'static str,
        regs: [u32; 4],
        site: u32,
    ) -> Result<u32> {
        crate::player_orders::call(m, name, regs, site)
    }

    /// Page 0xd (0x42a080): aiming the Nova. A click, or the countdown at
    /// 0x4e062c reaching zero (decremented every 32 ticks), issues tag 39.
    fn nova_page(&self, m: &mut MemoriaJogo, p: u32, c: u32) -> Result<()> {
        self.cursor(m, 0x1e99, m.d(0x508f80)? & 7)?;
        if m.b(0x508f7c)? & 0x1f == 0 {
            m.sw(0x4e062c, m.w(0x4e062c)?.wrapping_sub(1) & 65535)?;
        }
        if m.b(AE)? == 0 && m.w(0x4e062c)? != 0 {
            return Ok(());
        }
        self.ord(m, p, 0x27, [c, 0, 0, 0], 0x42a0e3)?;
        m.sw(PAGE, 0)?;
        m.sb(MODE, 255)?;
        m.sw(0x4e062c, 0)?;
        m.sb(0x496d48, 1)
    }
    fn cursor(&self, m: &mut MemoriaJogo, id: u32, anim: u32) -> Result<()> {
        m.sw(0x49a010, id)?;
        m.sd(0x49a014, anim)
    }
    fn end(&self, m: &mut MemoriaJogo) -> Result<()> {
        if m.d(0x4e060c)? != self.info {
            m.sw(0x4e0610, if self.flag { 1 } else { m.w(0x49620a)? })?
        } else if m.w(0x4e0610)? < m.w(0x49620a)? {
            m.sw(0x4e0610, m.w(0x4e0610)? + 1)?
        }
        m.sd(0x4e060c, self.info)?;
        m.sw(0x49620c, self.info2)?;
        if m.b(0x496d2c)? != 0 {
            crate::frame_input::rain(m)?
        }
        Ok(())
    }
    fn group(&self, m: &mut MemoriaJogo, p: u32) -> Result<()> {
        let g = m.b(0x49a05a)?;
        if player::key(m, 0x11, 0x4293e9)? < 0 {
            self.ord(m, p, 11, [g, 0, 0, 0], 0x429540)?
        } else if player::key(m, 0x10, 0x429403)? >= 0 {
            self.ord(m, p, 12, [g, 0, 0, 0], 0x429540)?
        } else {
            self.ord(m, p, 12, [g, 0, 0, 0], 0x429425)?;
            self.core(m, "EnginGetCentreGroupe", [p, VIEW, VIEW + 2, g], 0x42943e)?;
            for (o, t, map) in [(0, 4, 0x497168), (2, 6, 0x49716a)] {
                let mid = m.w(VIEW + t)? / 2;
                let v = m.w(VIEW + o)?;
                m.sw(VIEW + o, v.saturating_sub(mid))?;
                if m.w(VIEW + o)? + m.w(VIEW + t)? >= m.w(map)? {
                    m.sw(VIEW + o, m.w(map)?.wrapping_sub(m.w(VIEW + t)?))?
                }
            }
        }
        m.sb(0x49a05a, 0)
    }
    fn drag(&self, m: &mut MemoriaJogo, x: u32, y: u32, c: u32) -> Result<()> {
        m.sw(0x49a024, x)?;
        m.sw(0x49a026, y)?;
        m.sw(0x4e19b2, c)?;
        m.sw(0x49a028, x + 1)?;
        m.sw(0x4e19b4, c)?;
        m.sb(DRAG, 1)?;
        m.sw(0x49a02a, y + 1)
    }
    fn minimap(&self, m: &mut MemoriaJogo, p: u32) -> Result<()> {
        if count(m, p)? != 0 {
            self.cursor(m, 0x1df5, m.d(0x508f80)? & 7)?
        }
        if m.b(B1)? != 0 {
            m.sw(0x49a010, 0x1e0b)?;
            if m.b(0x4e05a8)? == 0 {
                m.sb(0x4e05a8, 1)?;
                m.sw(0x507d6c, m.w(0x507d68)?.wrapping_sub(m.w(X)?))?;
                m.sw(0x507d6e, m.w(0x507d6a)?.wrapping_sub(m.w(Y)?))?
            } else {
                crate::chain_vision::extra(
                    m,
                    "MiniMapSetPos",
                    crate::FrameRegisters {
                        eax: m.w(X)?,
                        edx: m.w(Y)?,
                        ecx: 1,
                        ..Default::default()
                    },
                )?;
                crate::frame_core::init_zone(m, 1)?;
            }
        } else {
            m.sb(0x4e05a8, 0)?
        }
        if m.b(AE)? == 0 {
            return Ok(());
        }
        let dx = (m.w(X)?.wrapping_sub(m.w(0x507d68)?)) & 65535;
        let dy = (m.w(Y)?.wrapping_sub(m.w(0x507d6a)?)) & 65535;
        if count(m, p)? != 0 {
            let c = minimap_case(m, dx, dy)?;
            self.ord(m, p, 5, [c & 65535, 0, 0, 0], 0x42bbd6)?
        } else {
            minimap_center(m, dx, dy, VIEW, VIEW + 2)?
        }
        Ok(())
    }
    fn panel(&mut self, m: &mut MemoriaJogo, p: u32) -> Result<()> {
        let si = icon_zone(m, m.w(X)?, m.w(Y)?)?;
        let selected = sel_structure(m, p)?;
        let a = if selected != 65535 {
            let t = m.b(player::structure(selected) + 8)?;
            0x4dd6e8 + 0x8f0 * p + 104 * t
        } else {
            if count(m, p)? != 1 {
                return Ok(());
            }
            bag(m, first(m, p)?)?
        };
        let pressed = m.b(AE)? != 0 && m.b(AF)? != 0;
        if matches!(si, 8 | 9) {
            m.sb(0x4e05a2, 1)?;
            if pressed {
                icon_scroll(m, a, if si == 8 { 65535 } else { 1 })?;
            }
        } else if si < 4 {
            let i = m.w(a + 4)? + si;
            let code = m.b(a + 0x42 + i)?;
            let t = m.w(a + 6 + 2 * i)?;
            match code {
                2 => {
                    self.info = player::text(m, (t & 255) + 0x28, 0x42be16)?;
                    self.info2 = 65535;
                    self.flag = false;
                    for a in [0x4e0614, 0x4e0616, 0x4e0612, 0x4e0618] {
                        m.sw(a, 0)?
                    }
                    if pressed {
                        m.sb(0x496209, t)?;
                        m.sw(PAGE, 11)?
                    }
                }
                3 => {
                    let t = t & 255;
                    self.info2 = 65535;
                    self.info = 0x495d8c + 52 * t + 0x13;
                    let c = cost(m, t)?;
                    for (a, v) in [
                        (0x4e0612, c[0]),
                        (0x4e0614, c[1]),
                        (0x4e0616, c[2]),
                        (0x4e0618, 0),
                    ] {
                        m.sw(a, v)?
                    }
                    self.flag = false;
                    if pressed && crate::player_orders::resources(m, p, c[0], c[1], 0)? {
                        if t == 20 {
                            self.ord(m, p, 15, [0; 4], 0x42bf88)?
                        } else {
                            m.sw(PAGE, 3)?;
                            m.sb(MODE, t)?
                        }
                    }
                }
                7 => {
                    self.info = player::text(m, 0x61, 0x42c218)?;
                    self.info2 = 65535;
                    self.flag = false;
                    if pressed {
                        m.sb(MODE, t)?;
                        m.sw(PAGE, 10)?
                    }
                }
                4 => {
                    let info = crate::spell_order_acceptance::input_panel(
                        m,
                        p as u8,
                        t as u8,
                        // 0x42bffa publishes three InputGame stack words that nothing
                        // sets; they only feed the tooltip's cost digits. Assumed zero
                        // (see validation/claude-20261109/README.md, spell_tooltip).
                        self.spell_tooltip_costs.or(Some([0; 3])),
                    )?;
                    self.info = info.text;
                    self.info2 = info.secondary;
                    self.flag = info.default_delay;
                }
                6 => {
                    let t = t & 255;
                    let r = 0x4965ae + 50 * t;
                    let (food, parts, stone) = (m.w(r + 2)?, m.w(r + 4)?, m.w(r + 6)?);
                    self.info2 = 65535;
                    self.info = r + 12;
                    self.flag = false;
                    m.sw(0x4e0612, food)?;
                    m.sw(0x4e0614, stone)?;
                    m.sw(0x4e0616, parts)?;
                    m.sw(0x4e0618, 0)?;
                    if pressed && crate::player_orders::resources(m, p, food, stone, parts)? {
                        self.ord(m, p, 17, [t, 0, 0, 0], 0x42bf88)?;
                    }
                }
                _ => {}
            }
        }
        if si != 255 && m.b(AE)? != 0 {
            m.sw(0x496d4a, si)?;
            m.sb(0x4e05a2, 1)?
        }
        Ok(())
    }
    fn map(&mut self, m: &mut MemoriaJogo, p: u32) -> Result<()> {
        let (x, y) = (m.w(X)?, m.w(Y)?);
        let c = (((y / 32 + m.w(VIEW + 2)?) << 8) + x / 32 + m.w(VIEW)?) & 65535;
        let margin = i32::from(m.u8(0x508f90)? as i8) * 32;
        if x as i32 >= m.w(VIEW + 4)? as i32 * 32 - margin
            || y as i32 >= m.w(VIEW + 6)? as i32 * 32 - margin
            || (x as i32) < margin
            || (y as i32) < margin
        {
            return Ok(());
        }
        let page = m.w(PAGE)?;
        if !discovered(m, c, m.b(0x508041)?)? && page != 5 && m.b(0x496208)? != 0 && page != 13 {
            if count(m, p)? == 0 {
                return Ok(());
            }
            if page == 0 {
                self.cursor(m, 0x1df5, m.d(0x508f80)? & 7)?;
                if m.b(AE)? != 0 && m.b(AF)? != 0 {
                    self.ord(m, p, 5, [c, 0, 0, 0], 0x42aecf)?
                }
            } else if page == 12 && m.order_queue_rule() {
                // Optional order queue: a Shift+click into the unexplored queues
                // the move a plain click would give (page 0 above).
                self.cursor(m, 0x1df5, m.d(0x508f80)? & 7)?;
                if m.b(AE)? != 0 && m.b(AF)? != 0 {
                    self.ord(m, p, crate::order_queue::TAG_QUEUE, [c, 0, 0, 0], 0x42aecf)?
                }
            } else {
                self.cursor(m, 0x1e0e, 0)?
            }
            return Ok(());
        }
        if sack(m, c)? {
            self.info = player::text(m, sack_kind(m, c)? + 0x28, 0x4299d0)?;
            self.info2 = crate::chain_queries::sack(m, c)?;
        } else if field(m, c)? {
            self.info = player::text(m, 0x2c, 0x4299f4)?
        } else if mine(m, c)? {
            self.info = player::text(m, 0x5b, 0x429a0e)?;
            self.info2 = crate::player_orders::mine_capacity(m, c)?;
            self.flag = false
        } else if obj(m, c)? == 0x46 && m.b(m.d(0x507dac)? + c)? == p {
            self.info = player::text(m, 0x33, 0x429a46)?;
            self.info2 = 65535;
            self.flag = false
        }
        let (mut under_mode, mut under_object) = (5, c);
        if lit(m, c, p)? {
            let w = m.w(m.d(0x4e0f38)? + 2 * c)?;
            let s = if w & 0x8000 != 0 {
                (w & 0x7fe0) >> 5
            } else {
                65535
            };
            m.sw(0x496ac4, s)?;
            if s != 65535 {
                (under_mode, under_object) = (3, s);
                self.info = 0x495d8c + 52 * m.b(player::structure(s) + 8)? + 0x13;
                self.info2 = 65535
            }
            let u = find(m, c)?;
            m.sw(0x496ac2, u)?;
            if u != 65535 {
                (under_mode, under_object) = (1, u);
                self.info = 0x4965ae + 50 * kind(m, u)? + 12;
                self.info2 = 65535;
                let r = unit(m, u)?;
                if m.b(r + 0xbd)? != 0 && owner(m, u)? == m.b(0x508041)? && m.b(B0)? != 0 {
                    player::sound(m, 0x429b6a, false, [r + 0x104, 2, u * 276, u], None)?;
                    self.core(m, "EnginAsk", [u, 2, 0, u], 0x429b79)?;
                }
            }
        }
        match page {
            9 => return self.deposit(m, p, c),
            12 => return self.group_page(m, p, c),
            11 => return self.engineer_page(m, p),
            13 => return self.nova_page(m, p, c),
            5 => return crate::spell_order_acceptance::input_cast(m, p as u8, c as u16),
            10 => return self.bomb_page(m, p, c),
            2 => return self.attack_page(m, p, under_mode, under_object),
            7 => return self.sell_page(m, p, c),
            3 => return self.placement(m, p, c),
            1 => {
                self.cursor(m, 0x1df5, m.d(0x508f80)? & 7)?;
                if m.b(AE)? != 0 && m.b(AF)? != 0 {
                    self.ord(m, p, 5, [c, 0, 0, 0], 0x42a488)?;
                    m.sw(PAGE, 0)?
                }
                return Ok(());
            }
            _ => {}
        }
        self.general(m, p, c, x, y)
    }
    /// Page 0xa (0x42a373): a kind-14 selection places a bomb on the cell
    /// (order 0x20), unless the selection is out of bombs (0x4738e0).
    fn bomb_page(&self, m: &mut MemoriaJogo, p: u32, c: u32) -> Result<()> {
        let u = first(m, p)?;
        m.sw(0x4e0628, u)?;
        if u == 65535 || kind(m, u)? != 14 || bombs_spent(m, p)? {
            m.sb(DRAG, 0)?;
            m.sb(MODE, 255)?;
            m.sw(PAGE, 0)?;
            return m.sb(0x496d48, 1);
        }
        self.cursor(m, 0x1e40, m.d(0x508f84)? % 3)?;
        if self.clicked(m)? {
            self.ord(m, p, 0x20, [c, 0, 0, 0], 0x42aecf)?;
        }
        Ok(())
    }
    fn placement(&self, m: &mut MemoriaJogo, p: u32, c: u32) -> Result<()> {
        let u = first(m, p)?;
        m.sw(0x4e0628, u)?;
        if u == 65535 || kind(m, u)? != 7 {
            m.sb(DRAG, 0)?;
            m.sw(PAGE, 0)?;
            m.sb(0x496d48, 1)?;
            return m.sb(MODE, 255);
        }
        let t = m.b(MODE)?;
        // 0x429fb6 EnginSelectMurComplet 0x473999: a selected, alive own
        // engineer with +0xd8 = 10 walls laid ends the wall page.
        if t == 9 && wall_complete(m, p)? {
            m.sb(DRAG, 0)?;
            m.sb(MODE, 255)?;
            m.sw(PAGE, 0)?;
            return m.sb(0x496d48, 1);
        }
        let ok = self.core(m, "BatimentPosable", [t, c, u, c], 0x42a002)? & 255 != 0;
        if !ok || m.b(AE)? == 0 || m.b(AF)? == 0 {
            return Ok(());
        }
        self.ord(m, p, 4, [t, c, 0, 0], 0x42a03c)?;
        if m.b(MODE)? != 9 {
            m.sb(DRAG, 0)?;
            m.sw(PAGE, 0)?;
            m.sb(0x496d48, 1)?;
            m.sb(MODE, 255)?
        }
        Ok(())
    }
    fn general(&self, m: &mut MemoriaJogo, p: u32, c: u32, x: u32, y: u32) -> Result<()> {
        if count(m, p)? == 0 && sel_structure(m, p)? == 65535 && m.b(DRAG)? == 0 && m.b(AE)? != 0 {
            return self.drag(m, x, y, c);
        }
        if m.b(DRAG)? != 0 {
            return Ok(());
        }
        if count(m, p)? != 0 && player::key(m, 0x45, 0x42a76b)? < 0 {
            self.ord(m, p, 0x25, [0; 4], 0x42a77f)?
        }
        let u = m.w(0x496ac2)?;
        if u != 65535 {
            return self.over_unit(m, p, u, x, y, c);
        }
        let s = m.w(0x496ac4)?;
        if s != 65535 {
            return self.over_structure(m, p, s, x, y, c);
        }
        self.terrain(m, p, c)
    }
    fn over_structure(
        &self,
        m: &mut MemoriaJogo,
        p: u32,
        s: u32,
        x: u32,
        y: u32,
        c: u32,
    ) -> Result<()> {
        if count(m, p)? != 0 {
            return self.structure_troops(m, p, s, x, y, c);
        }
        if sel_structure(m, p)? == s && m.b(B0)? != 0 {
            return self.ord(m, p, 0x21, [s, 0, 0, 0], 0x42aecf);
        }
        let selected = sel_structure(m, p)?;
        if selected != 65535 {
            let ok =
                self.core(m, "BatimentPeutAttaquer", [selected, 3, s, s], 0x42b2a3)? & 255 != 0;
            let owner = m.b(player::structure(s) + 4)?;
            if ok && owner != p && owner != 200 {
                self.cursor(m, 0x1dfd, m.d(0x508f80)? % 6)?;
                if self.clicked(m)? {
                    self.ord(m, p, 14, [3, s, 0, 0], 0x42aecf)?;
                }
                return Ok(());
            }
        }
        self.own_structure(m, p, s, x, y, c)
    }
    fn over_unit(&self, m: &mut MemoriaJogo, p: u32, u: u32, x: u32, y: u32, c: u32) -> Result<()> {
        if m.w(PAGE)? == 8 {
            return Ok(());
        }
        if owner(m, u)? == p {
            self.cursor(m, 0x1df0, m.d(0x508f80)? % 5)?
        }
        if count(m, p)? != 0 && can_attack(m, p, 1, u)? && owner(m, u)? != p && owner(m, u)? != 200
        {
            self.cursor(m, 0x1dfd, m.d(0x508f80)? % 6)?;
            if m.b(AE)? != 0 && m.b(AF)? != 0 {
                self.ord(m, p, 14, [1, u, 0, 0], 0x42aecf)?
            }
            return Ok(());
        }
        if count(m, p)? != 0 && can_enter(m, p, 1, u)? && !mirror(m, u)? {
            // 0x42a8ce: an all-mechanic selection over a vehicle at no more
            // than half health repairs it (order 20 [1, unit]). The half is
            // EnginGetVieMax 0x441108 called with EnginGetType(unit) as its
            // unit index (the original reads the type's maximum of unit
            // record #type). Otherwise the selection boards it (order 16).
            if pages::all_type(m, p, 6)? {
                let t = kind(m, u)?;
                let half = u32::from(m.w(0x4965b6 + 50 * kind(m, t)?)? as u16) >> 1;
                if m.w(unit(m, u)? + 0x9a)? & 65535 <= half {
                    self.cursor(m, 0x1e6d, m.d(0x508f84)? & 1)?;
                    if m.b(AE)? != 0 && m.b(AF)? != 0 {
                        self.ord(m, p, 20, [1, u, 0, 0], 0x42aecf)?;
                    }
                    return Ok(());
                }
            }
            self.cursor(m, 0x1e16, m.d(0x508f84)? % 3)?;
            if m.b(AE)? != 0 && m.b(AF)? != 0 {
                self.ord(m, p, 16, [1, u, 0, 0], 0x42aecf)?;
            }
            return Ok(());
        }
        let selected = sel_structure(m, p)?;
        if selected != 65535
            && self.core(
                m,
                "BatimentPeutAttaquer",
                [selected, 1, m.w(0x496ac2)?, 0],
                0x42a9ed,
            )? & 255
                != 0
        {
            // 0x42a9f6: an own turret or tower selected over an enemy unit
            // (not the player's, not neutral 200) attacks it: order 14.
            let target = m.w(0x496ac2)?;
            let target_owner = owner(m, target)?;
            if target_owner != p && target_owner != 200 {
                self.cursor(m, 0x1dfd, m.d(0x508f80)? % 6)?;
                if self.clicked(m)? {
                    self.ord(m, p, 14, [1, target, 0, 0], 0x42aecf)?;
                }
                return Ok(());
            }
        }
        if m.b(B0)? != 0 && first(m, p)? == u && !mirror(m, u)? {
            let t = kind(m, u)?;
            let a = match t {
                // 0x42ab1f: a harvester with sacks drops them; empty, it unloads.
                13 => Some(
                    if crate::chain_bags::body(
                        m,
                        "BagContient",
                        crate::FrameRegisters {
                            eax: unit(m, u)? + 10,
                            edx: 2,
                            ebx: 1,
                            ..Default::default()
                        },
                    )?
                    .eax as u16
                        != 0
                    {
                        (2, 1)
                    } else {
                        (1, 0)
                    },
                ),
                7 => Some((2, 2)),
                5 => Some((2, 1)),
                6 => Some((2, 0)),
                11..=17 => Some((1, 0)),
                _ => None,
            };
            if let Some((a, b)) = a {
                self.ord(m, p, 9, [u, a, b, 0], 0x42ab7d)?
            }
        }
        if owner(m, u)? != p {
            return Ok(());
        }
        self.cursor(m, 0x1df0, m.d(0x508f80)? % 5)?;
        if m.b(AE)? != 0 {
            self.drag(m, x, y, c)?
        }
        Ok(())
    }
    fn terrain(&self, m: &mut MemoriaJogo, p: u32, c: u32) -> Result<()> {
        if count(m, p)? == 0 {
            return Ok(());
        }
        let s = m.w(0x496ac4)?;
        let cave = s != 65535 && m.b(player::structure(s) + 8)? == 3;
        if (field(m, c)? || sack_kind(m, c)? == 1 || cave)
            && self.core(m, "EnginTravailSelectionne", [p, 1, 0, 0], 0x42b375)? & 255 != 0
        {
            if m.w(PAGE)? == 8 {
                return self.arm_deposit(
                    m,
                    c,
                    if field(m, c)? {
                        1
                    } else if cave {
                        3
                    } else {
                        2
                    },
                    1,
                );
            }
            self.cursor(m, 0x1e10, m.d(0x508f80)? % 6)?;
            if m.b(AE)? != 0 && m.b(AF)? != 0 {
                self.ord(
                    m,
                    p,
                    if field(m, c)? { 7 } else { 18 },
                    [c, 0, 0, 0],
                    if field(m, c)? { 0x42b486 } else { 0x42af96 },
                )?;
                m.sw(0x496204, c)?;
                m.sw(0x4e05ac, 0x1e03)?
            }
            return Ok(());
        }
        if (mine(m, c)? || sack_kind(m, c)? == 2 || cave)
            && self.core(m, "EnginTravailSelectionne", [p, 2, 0, 0], 0x42b375)? & 255 != 0
        {
            if m.w(PAGE)? == 8 {
                return self.arm_deposit(
                    m,
                    c,
                    if mine(m, c)? {
                        0
                    } else if cave {
                        3
                    } else {
                        2
                    },
                    2,
                );
            }
            self.cursor(m, 0x1e10, m.d(0x508f80)? % 6)?;
            if m.b(AE)? != 0 && m.b(AF)? != 0 {
                if sack(m, c)? {
                    self.ord(m, p, 18, [c, 0, 0, 0], 0x42b61c)?
                } else {
                    self.ord(m, m.b(0x508041)?, 6, [c, 0, 0, 0], 0x42ad8e)?
                }
                m.sw(0x496204, c)?;
                m.sw(0x4e05ac, 0x1e03)?
            }
            return Ok(());
        }
        if (sack_kind(m, c)? == 0 || cave)
            && self.core(m, "EnginTravailSelectionne", [p, 0, 0, 0], 0x42b375)? & 255 != 0
        {
            if m.w(PAGE)? == 8 {
                return self.arm_deposit(m, c, 2, 0);
            }
            self.cursor(m, 0x1e10, m.d(0x508f80)? % 6)?;
            if m.b(AE)? != 0 && m.b(AF)? != 0 && sack(m, c)? {
                self.ord(m, p, 18, [c, 0, 0, 0], 0x42ad8e)?;
                m.sw(0x496204, c)?;
                m.sw(0x4e05ac, 0x1e03)?;
            }
            return Ok(());
        }
        if m.w(PAGE)? == 8 {
            return self.cursor(m, 0x1e0e, 0);
        }
        let bad = (lit(m, c, p)? && collision(m, c, 1)? && land(m, p)?)
            || (!lit(m, c, p)? && obstacle(m, c)? && land(m, p)?);
        if bad {
            return self.cursor(m, 0x1e0e, 0);
        }
        self.cursor(m, 0x1df5, m.d(0x508f80)? & 7)?;
        if m.b(AE)? != 0 && m.b(AF)? != 0 {
            self.ord(m, p, 5, [c, 0, 0, 0], 0x42b82a)?;
            m.sw(0x496204, c)?;
            m.sw(0x4e05ac, 0x1e03)?
        }
        Ok(())
    }
    fn box_selection(&self, m: &mut MemoriaJogo, p: u32) -> Result<()> {
        if m.b(DRAG)? == 0 || m.b(AE)? == 0 {
            return Ok(());
        }
        let (x, y) = (m.w(X)?, m.w(Y)?);
        let (w, h) = (m.w(VIEW + 4)? * 32, m.w(VIEW + 6)? * 32);
        m.sw(0x49a028, if x < w { x } else { w - 1 })?;
        m.sw(0x49a02a, if y < h { y } else { h - 1 })?;
        let line = m.w(0x49a02a)? as u16 as i16 as i32 / 32 + m.w(VIEW + 2)? as i32;
        m.sw(
            0x4e19b4,
            ((line << 8) + m.w(0x49a028)? as u16 as i16 as i32 / 32 + m.w(VIEW)? as i32) as u32,
        )?;
        if m.b(AF)? != 0 {
            self.core(m, "DeSelectionGroupe", [p, 0, 0, 0], 0x42b935)?;
            self.core(
                m,
                "SelectionEngin",
                [p, m.w(0x4e19b2)?, m.w(0x4e19b4)?, 0],
                0x42b94a,
            )?;
            if count(m, p)? == 0 {
                m.sb(0x4e17b8, 3)?;
                m.sw(0x4963c0, m.w(0x496ac4)?)?;
                let s = m.w(0x496ac4)?;
                if s != 65535 && m.b(player::structure(s) + 4)? == p {
                    self.ord(m, p, 2, [s, 0, 0, 0], 0x42b9aa)?;
                    self.core(m, "BatimentSelection", [s, 0, 0, 0], 0x42b9b9)?;
                    m.sw(0x4e244c, 0)?;
                    m.sw(0x4e244e, 0)?;
                    if m.b(player::structure(s) + 8)? == 16 {
                        m.sb(0x496fd8, 1)?;
                        crate::frame_core::init_zone(m, 1)?;
                    }
                }
            } else {
                self.core(m, "BatimentDeselectionner", [p, 1, 0, 0], 0x42ba03)?;
                self.ord(m, p, 1, [m.w(0x4e19b2)?, m.w(0x4e19b4)?, 0, 0], 0x42ba20)?;
                m.sb(0x4e17b8, 1)?;
                m.sw(0x4963c0, m.w(0x496ac2)?)?;
                m.sw(0x4e2454, 0)?
            }
            m.sb(DRAG, 0)?;
        }
        self.cursor(m, 0x1e0c, 0)?;
        m.sw(X, m.w(X)?.wrapping_sub(7))?;
        m.sw(Y, m.w(Y)?.wrapping_sub(7))
    }
}
/// EnginSelectMurComplet 0x473999: a selected alive own kind-7 unit with +0xd8 = 10.
fn wall_complete(m: &MemoriaJogo, p: u32) -> Result<bool> {
    for u in player::selections(m, p)? {
        let r = unit(m, u)?;
        if player::alive(m, u)? && m.b(r)? == p && m.b(r + 1)? == 7 && m.w(r + 0xd8)? == 10 {
            return Ok(true);
        }
    }
    Ok(false)
}
/// 0x4738e0: a selected alive own kind-14 unit whose bomb count +0xd8 is 10,
/// or whose count plus one in state 0x16 reaches its mana 0x44828b (+0xda).
fn bombs_spent(m: &MemoriaJogo, p: u32) -> Result<bool> {
    for u in player::selections(m, p)? {
        let r = unit(m, u)?;
        if !player::alive(m, u)? || m.b(r)? != p || m.b(r + 1)? != 14 {
            continue;
        }
        let spent = m.w(r + 0xd8)?;
        if spent == 10 || spent + u32::from(m.b(r + 2)? == 0x16) >= m.b(r + 0xda)? {
            return Ok(true);
        }
    }
    Ok(false)
}
pub fn minimap_case(m: &MemoriaJogo, x: u32, y: u32) -> Result<u32> {
    let (w, h, mw, mh) = (
        m.w(0x497168)?,
        m.w(0x49716a)?,
        m.w(0x496fe0)?,
        m.w(0x496fe2)?,
    );
    if mw == 0 || mh == 0 {
        return stop("MiniMapGetCase division by zero", 0x46a224);
    }
    Ok((((y & 65535) * h / mh) << 8).wrapping_add((x & 65535) * w / mw))
}
pub fn minimap_center(m: &mut MemoriaJogo, x: u32, y: u32, ax: u32, ay: u32) -> Result<()> {
    let (w, h, mw, mh, vw, vh) = (
        m.w(0x497168)? as i32,
        m.w(0x49716a)? as i32,
        m.w(0x496fe0)? as i32,
        m.w(0x496fe2)? as i32,
        m.w(VIEW + 4)? as i32,
        m.w(VIEW + 6)? as i32,
    );
    if [w, h, mw, mh].contains(&0) {
        return stop("MiniMapGetCenterSelect division by zero", 0x46a273);
    }
    let mut bx = ((x & 65535) as i32 - (vw * mw / w) / 2) * w / mw;
    let mut dy = ((y & 65535) as i32 - (vh * mh / h) / 2) * h / mh;
    if (bx as i16) < 0 {
        bx = 0
    }
    if (dy as i16) < 0 {
        dy = 0
    }
    if i32::from(bx as i16) + vw > w {
        bx = w - vw
    }
    if (vh + i32::from(dy as i16)) as u32 > h as u32 {
        dy = h - vh
    }
    m.sw(ax, bx as u32)?;
    m.sw(ay, dy as u32)
}
#[cfg(feature = "validation-trace")]
pub fn spell_panel_fragment(m: &mut MemoriaJogo, p: u32, costs: [u16; 3]) -> Result<()> {
    let mut e = Input::new(Some(costs));
    e.panel(m, p)?;
    e.end(m)
}
#[cfg(feature = "validation-trace")]
pub fn input_with_spell_costs(m: &mut MemoriaJogo, p: u32, costs: [u16; 3]) -> Result<()> {
    input_context(m, p, Some(costs))
}
pub fn input(m: &mut MemoriaJogo, p: u32) -> Result<()> {
    crate::market::display_tick(m)?;
    // Original inherited tooltip stack words have no ordinary host contract.
    input_context(m, p, None)
}
/// InputGame 0x429305: Escape held in game. With an event in progress the
/// original ends it (EvenementStop 0x458088 at 0x42933f) and goes on with the
/// ordinary input; otherwise it queues the pause order13 for the player
/// (processed by ProcessInputGame as MenuInit16), waits for the key release at
/// the host and silences the sound device, returning from InputGame at once.
/// Returns whether InputGame returns at once.
pub fn escape(m: &mut MemoriaJogo, p: u32) -> Result<bool> {
    if m.b(0x4e1b0e)? != 0 {
        crate::event_flc::stop_event(m)?;
        return Ok(false);
    }
    player::order(m, p, 13, [0, 0, 0, 0], 0x45ecd4)?;
    crate::campaign_flow::stop_sounds(m, 0x429335)?;
    Ok(true)
}
fn input_context(m: &mut MemoriaJogo, p: u32, costs: Option<[u16; 3]>) -> Result<()> {
    let mut e = Input::new(costs);
    m.sw(0x49a010, 0x1dd8)?;
    m.sw(0x4e0612, 0)?;
    m.sw(0x4e0614, 0)?;
    m.sd(0x49a014, 0)?;
    m.sw(0x4e0616, 0)?;
    m.sb(0x508f8f, 0)?;
    m.sb(0x4e17ac, 0)?;
    m.sw(0x4e0618, 0)?;
    for a in [0x496d4a, 0x496ac2, 0x496ac4] {
        m.sw(a, 65535)?
    }
    if player::key(m, 0x1b, 0x429300)? < 0 && escape(m, p)? {
        return Ok(());
    }
    if m.b(AE)? != 0 {
        m.sw(0x49a010, m.w(0x49a010)? + 8)?
    }
    if m.b(AE)? != 0 && m.b(DRAG)? != 0 {
        m.sw(Z, 0)?
    } else if m.b(0x4e05a8)? != 0 {
        if m.b(B1)? != 0 {
            m.sw(Z, 10)?
        } else {
            m.sb(0x4e05a8, 0)?
        }
    } else if m.b(B1)? != 0 && (m.b(0x4e05a9)? != 0 || m.b(0x4e05aa)? != 0) {
        m.sw(Z, 26)?
    } else {
        m.sb(DRAG, 0)?;
        m.sw(Z, zone(m)?)?
    }
    if m.b(0x49a05a)? != 0 {
        e.group(m, p)?
    }
    if player::key(m, 0x10, 0x429556)? < 0 {
        m.sw(PAGE, 12)?
    } else if m.w(PAGE)? == 12 {
        m.sw(PAGE, 0)?
    }
    if m.b(B1)? != 0 && m.w(0x4e062c)? != 0 {
        m.sw(0x4e062c, 0)?;
        m.sw(PAGE, 0)?
    }
    if m.b(B1)? != 0 && m.w(Z)? != 11 {
        e.core(m, "DeSelectionGroupe", [p, p, 0, 0], 0x4295c0)?;
        e.core(m, "BatimentDeselectionner", [p, p, 0, 0], 0x4295c9)?;
        m.sw(PAGE, 0)?;
        m.sb(0x4e05ab, 0)?;
        m.sb(MODE, 255)?;
        m.sb(DRAG, 0)?;
        m.sb(0x496d48, 1)?
    }
    if player::key(m, 0x11, 0x4295fc)? < 0 {
        let carriers = count(m, p)? != 0
            && pages::leaf(m, "EnginSelectPeuventTransporter", p, 0, 0)? & 255 != 0;
        if m.w(PAGE)? != 9 {
            if carriers {
                m.sw(PAGE, 8)?;
            }
        } else if !carriers {
            m.sw(PAGE, 0)?;
        }
    } else if matches!(m.w(PAGE)?, 8 | 9) {
        m.sw(PAGE, 0)?
    }
    if player::key(m, 0x2e, 0x42967e)? < 0 {
        m.sw(PAGE, 7)?
    } else if m.w(PAGE)? == 7 {
        m.sw(PAGE, 0)?
    }
    if player::key(m, 0x12, 0x4296ad)? < 0 {
        m.sw(PAGE, 2)?
    } else if m.w(PAGE)? == 2 {
        m.sw(PAGE, 0)?
    }
    if player::key(m, 0x20, 0x4296dc)? < 0 {
        m.sb(0x508f8f, 1)?
    }
    if player::key(m, 0x26, 0x4296f1)? < 0 {
        scroll(m, 2, 1)?
    } else if player::key(m, 0x28, 0x42970b)? < 0 {
        scroll(m, 6, 1)?
    }
    if player::key(m, 0x27, 0x429732)? < 0 {
        scroll(m, 4, 1)?
    } else if player::key(m, 0x25, 0x42974c)? < 0 {
        scroll(m, 8, 1)?
    }
    if player::key(m, 13, 0x429773)? & 65535 != 0 || player::key(m, 0x58, 0x429781)? & 65535 != 0 {
        last_attack(m, VIEW, VIEW + 2)?
    }
    if m.w(PAGE)? == 13 {
        // 0x4297a6: the Nova page voices the selected structure every pass.
        let s = player::selected_structure(m, p)? & 65535;
        crate::chain_s1::entry(
            m,
            "VoixOffAsk",
            crate::FrameRegisters {
                eax: s,
                edx: 2,
                ebx: 0,
                ..Default::default()
            },
        )?;
    }
    let z = m.w(Z)?;
    match z {
        0 => {
            e.map(m, p)?;
            e.box_selection(m, p)?
        }
        1..=8 => {
            e.cursor(m, z + 0x1dd7 + if m.b(AE)? != 0 { 8 } else { 0 }, 0)?;
            scroll(m, z, if m.b(AE)? != 0 { 2 } else { 1 })?
        }
        10 => e.minimap(m, p)?,
        11 => e.panel(m, p)?,
        26 => crate::market::input_zone(m, p)?,
        27 => {
            if m.b(AE)? != 0 && m.b(AF)? != 0 {
                crate::event_flc::message_box_run(m)?;
            }
        }
        28..=31 if m.b(AE)? != 0 && m.b(AF)? != 0 => {
            e.ord(m, p, 0x26, [p, z - 28, 0, 0], 0x42bbd6)?;
        }
        _ => {}
    }
    e.end(m)
}
pub fn last_attack(m: &mut MemoriaJogo, ax: u32, ay: u32) -> Result<()> {
    let dif = m.d(0x508f7c)?.wrapping_sub(m.d(0x4e19c0)?);
    let c = m.w(0x496acc)?;
    if dif > 1000 || c == 65535 {
        return Ok(());
    }
    let vw = m.w(VIEW + 4)? as i32;
    let vh = m.w(VIEW + 6)? as i32;
    let x = (c as i32 % 256 - vw / 2)
        .max(0)
        .min(m.w(0x497168)? as i32 - vw);
    let y = (c as i32 / 256 - vh / 2)
        .max(0)
        .min(m.w(0x49716a)? as i32 - vh);
    m.sw(ax, x as u32)?;
    m.sw(ay, y as u32)
}
