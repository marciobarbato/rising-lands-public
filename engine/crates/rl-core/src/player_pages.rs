//! InputGame round 3: deposit/help pages and structure-hover dispatch.
//! This child shares the parent's input transaction and exact packet writer.
use super::*;

fn troop(t: u32) -> bool {
    !matches!(t, 3 | 11..=17 | 25)
}
fn wounded(m: &MemoriaJogo, u: u32) -> Result<bool> {
    Ok(m.w(unit(m, u)? + 0x9a)? != m.w(0x4965b6 + 50 * kind(m, u)?)?)
}
pub(super) fn all_type(m: &MemoriaJogo, p: u32, t: u32) -> Result<bool> {
    all_live(m, p, |m, u| Ok(kind(m, u)? == t))
}
fn all_live(
    m: &MemoriaJogo,
    p: u32,
    test: impl Fn(&MemoriaJogo, u32) -> Result<bool>,
) -> Result<bool> {
    let mut found = false;
    for u in selections(m, p)? {
        if alive(m, u)? {
            if !test(m, u)? {
                return Ok(false);
            }
            found = true;
        }
    }
    Ok(found)
}
fn structure_at(m: &MemoriaJogo, c: u32) -> Result<u32> {
    let word = m.w(m.d(0x4e0f38)? + 2 * (c & 65535))?;
    Ok(if word & 0x8000 != 0 {
        (word & 0x7fe0) >> 5
    } else {
        65535
    })
}
fn built(m: &MemoriaJogo, s: u32) -> Result<bool> {
    let r = player::structure(s);
    Ok(m.w(r + 10)? == m.w(0x495d90 + 52 * m.b(r + 8)?)?)
}
fn ally(m: &MemoriaJogo, a: u32, b: u32) -> Result<bool> {
    let (a, b) = (a & 255, b & 255);
    if a == b {
        return Ok(true);
    }
    if a > 4 || b > 4 {
        return Ok(false);
    }
    Ok(m.b(m.d(0x5080a8)? + 20 * a + 6)? & (1 << b) != 0)
}

/// EAX of the thirteen captured leaves. Other register leftovers are not an API.
pub(super) fn leaf(m: &MemoriaJogo, name: &str, a: u32, d: u32, b: u32) -> Result<u32> {
    let p = a & 255;
    match name {
        "PlayerEstAllie" => Ok(if p == d & 255 {
            (a & !255) | 1
        } else {
            yes(ally(m, a, d)?)
        }),
        "ChercheIndiceBatiment" => structure_at(m, a),
        "BatimentEstDepot" => Ok(yes(matches!(m.b(player::structure(a) + 8)?, 3 | 4))),
        "BatimentEstNeutre" => Ok(yes(m.b(player::structure(a) + 4)? == 200)),
        "BatimentEstEnnemi" => {
            let owner = m.b(player::structure(a) + 4)?;
            Ok(yes(owner != 200 && !ally(m, owner, d)?))
        }
        "BatimentPeutSeVendre" => {
            let r = player::structure(a);
            Ok(if m.b(r + 8)? == 17 {
                ((a & 65535) * 276) & !255
            } else {
                yes(m.b(r + 0x74)? == 0)
            })
        }
        "EnginSelectPeutAttaquer" => {
            let mode = d & 255;
            let target = if mode == 1 { kind(m, b)? } else { 0 };
            if mode == 1 && target == 17 {
                return Ok(0);
            }
            if mode == 5 {
                return Ok((a & !255) | 1);
            }
            let mut value = if mode == 1 { target } else { a };
            for u in selections(m, p)? {
                value = yes(alive(m, u)?);
                if value == 0 {
                    continue;
                }
                let t = kind(m, u)?;
                if !army(t)
                    || (mode == 1
                        && (matches!(target, 17 | 10) || target == 16 && !matches!(t, 2 | 8)))
                {
                    return Ok(0);
                }
                value = if matches!(t, 14 | 16) {
                    m.b(unit(m, u)? + 0xda)?
                } else {
                    0
                };
                if matches!(t, 14 | 16) && value == 0 {
                    return Ok(0);
                }
            }
            Ok((value & !255) | 1)
        }
        "EnginSelectEstTroupe" | "EnginSelectEstEngin" => {
            let mut value = a;
            let mut found = 0;
            for u in selections(m, p)? {
                value = yes(alive(m, u)?);
                if value == 0 {
                    continue;
                }
                let t = kind(m, u)?;
                value = if name == "EnginSelectEstTroupe" {
                    if troop(t) {
                        0x101
                    } else {
                        0
                    }
                } else {
                    yes((11..=17).contains(&t))
                };
                if value == 0 {
                    return Ok(0);
                }
                found = 1;
            }
            Ok((value & !255) | found)
        }
        "EnginSelectContientMiroir" => {
            let mut value = a;
            for u in selections(m, p)? {
                value = yes(alive(m, u)?);
                if value != 0 {
                    value = yes(mirror(m, u)?);
                    if value != 0 {
                        return Ok(1);
                    }
                }
            }
            Ok(value & !255)
        }
        "EnginSelectPeuventTransporter" => {
            let mut value = a;
            for u in selections(m, p)? {
                value = yes(alive(m, u)?);
                if value != 0 && !matches!(kind(m, u)?, 5 | 6 | 7 | 13) {
                    return Ok(0);
                }
            }
            Ok((value & !255) | 1)
        }
        "EnginSelectEstBlesse" => {
            let mut n = 0u32;
            for u in selections(m, p)? {
                if alive(m, u)? && wounded(m, u)? {
                    n = n.wrapping_add(1);
                }
            }
            n &= 65535;
            Ok((n & 0xff00) | yes(n >= 1))
        }
        "EnginSelectPeutDeposer" => {
            let t = m.b(player::structure(d) + 8)?;
            let mut found = false;
            for u in selections(m, p)? {
                if !alive(m, u)? {
                    continue;
                }
                let ut = kind(m, u)?;
                if !(3..=4).contains(&t)
                    || t == 3 && !matches!(ut, 5 | 6 | 7 | 13)
                    || t == 4 && ut != 5
                {
                    return Ok(0);
                }
                found = true;
            }
            Ok(((p << 9) & !255) | yes(found))
        }
        _ => stop("unported input selection leaf", 0x472355),
    }
}

/// Boolean projection of Core's EnginSelectPeutEntrer mode 3, now bound upstream.
/// State-13 execution remains the composition's separate named boundary.
fn enter_structure(m: &MemoriaJogo, p: u32, s: u32) -> Result<bool> {
    for u in selections(m, p)? {
        if alive(m, u)? && mirror(m, u)? {
            return Ok(false);
        }
    }
    let r = player::structure(s);
    let (t, owner) = (m.b(r + 8)?, m.b(r + 4)?);
    if (owner == p || owner == 200) && all_type(m, p, 9)? && t != 9 {
        return Ok(true);
    }
    if owner != p {
        return Ok(t == 16 && all_type(m, p, 10)?);
    }
    if t == 21 {
        let mut all = true;
        for u in selections(m, p)? {
            if alive(m, u)? && matches!(kind(m, u)?, 3 | 11..=17) {
                all = false;
                break;
            }
        }
        if all {
            return Ok(true);
        }
    }
    if all_live(m, p, |m, u| Ok(troop(kind(m, u)?)))? {
        if t == 2 {
            return Ok(true);
        }
        if t == 19 && leaf(m, "EnginSelectEstBlesse", p, 0, 0)? & 255 != 0 {
            return Ok(true);
        }
    }
    if t == 14 && all_live(m, p, |m, u| Ok((11..=17).contains(&kind(m, u)?)))? {
        if leaf(m, "EnginSelectEstBlesse", p, 0, 0)? & 255 != 0 {
            return Ok(true);
        }
        for u in selections(m, p)? {
            if alive(m, u)? && m.b(unit(m, u)? + 0xda)? != 5 {
                return Ok(true);
            }
        }
    }
    if t == 18 && all_type(m, p, 16)? {
        return Ok(true);
    }
    Ok(t == 8)
}

impl Input {
    pub(super) fn clicked(&self, m: &MemoriaJogo) -> Result<bool> {
        Ok(m.b(AE)? != 0 && m.b(AF)? != 0)
    }
    fn point(&self, m: &mut MemoriaJogo, c: u32) -> Result<()> {
        m.sw(0x496204, c)?;
        m.sw(0x4e05ac, 0x1e03)
    }
    pub(super) fn deposit(&self, m: &mut MemoriaJogo, p: u32, c: u32) -> Result<()> {
        let s = m.w(0x496ac4)?;
        if s != 65535
            && !(leaf(m, "BatimentEstDepot", s, 0, 0)? != 0
                && leaf(m, "EnginSelectPeutDeposer", p, s, 0)? & 255 != 0
                && m.b(player::structure(s) + 4)? == p)
        {
            return self.cursor(m, 0x1e0e, 0);
        }
        self.cursor(m, 0x1e40, m.d(0x508f84)? % 3)?;
        if !self.clicked(m)? {
            return Ok(());
        }
        let mode = m.w(0x4e05b0)?;
        if mode <= 3 {
            self.ord(
                m,
                p,
                [6, 7, 8, 0x13][mode as usize],
                [
                    m.w(0x4e05b2)?,
                    if s != 65535 { 2 } else { 1 },
                    if s != 65535 { s } else { c },
                    m.b(0x4e05b4)?,
                ],
                0x429c88,
            )?;
        }
        m.sw(PAGE, 0)?;
        self.point(m, c)
    }
    pub(super) fn group_page(&self, m: &mut MemoriaJogo, p: u32, c: u32) -> Result<()> {
        let u = m.w(0x496ac2)?;
        if u == 65535 && m.order_queue_rule() {
            return self.queue_waypoint(m, p, c);
        }
        if u == 65535 || self.core(m, "EnginSelectionnable", [u, 0, 0, 0], 0x429dc5)? & 255 == 0 {
            return Ok(());
        }
        self.cursor(m, 0x1df0, m.d(0x508f80)? % 5)?;
        if m.b(AF)? != 0 {
            self.ord(m, p, 0x24, [u, 0, 0, 0], 0x42aecf)?;
        }
        Ok(())
    }
    /// Optional order queue (`RULE_ORDER_QUEUE`): a Shift+click on open ground
    /// with units selected queues a move to the cell (order tag 40). The cell
    /// test and cursors are `terrain()`'s for a plain move; a structure under
    /// the cursor, or no unit selection, stays the original no-op.
    fn queue_waypoint(&self, m: &mut MemoriaJogo, p: u32, c: u32) -> Result<()> {
        if count(m, p)? == 0 || m.w(0x496ac4)? != 65535 {
            return Ok(());
        }
        let bad = (lit(m, c, p)? && collision(m, c, 1)? && land(m, p)?)
            || (!lit(m, c, p)? && obstacle(m, c)? && land(m, p)?);
        if bad {
            return self.cursor(m, 0x1e0e, 0);
        }
        self.cursor(m, 0x1df5, m.d(0x508f80)? & 7)?;
        if self.clicked(m)? {
            self.ord(m, p, crate::order_queue::TAG_QUEUE, [c, 0, 0, 0], 0x42aecf)?;
        }
        Ok(())
    }
    pub(super) fn engineer_page(&self, m: &mut MemoriaJogo, p: u32) -> Result<()> {
        let u = first(m, p)?;
        m.sw(0x4e0628, u)?;
        if u == 65535 || !all_type(m, p, 8)? {
            m.sb(DRAG, 0)?;
            m.sb(MODE, 255)?;
            m.sw(PAGE, 0)?;
            return m.sb(0x496d48, 1);
        }
        self.cursor(m, 0x1e0e, 0)?;
        let s = m.w(0x496ac4)?;
        // 0x429ea2 reads BatimentGetType(0xffff) with no structure under the
        // cursor: an out-of-pool byte at 0x15d9fbc. Only a value of 3 or 7
        // would change the cursor or issue the order, so empty ground is
        // treated as neither (see docs/HANDOVER_20261109_CACA_PARADAS.md).
        if s == 65535 {
            return Ok(());
        }
        let (t, mode) = (m.b(player::structure(s) + 8)?, m.b(0x496209)?);
        if !(t == 3 && mode != 3 || t == 7 && mode == 3) {
            return Ok(());
        }
        self.cursor(m, 0x1ea9, m.d(0x508f80)? % 3)?;
        if self.clicked(m)? {
            self.ord(m, p, 0x22, [3, s, mode, 0], 0x429f2b)?;
        }
        Ok(())
    }
    pub(super) fn attack_page(
        &self,
        m: &mut MemoriaJogo,
        p: u32,
        mode: u32,
        target: u32,
    ) -> Result<()> {
        let s = sel_structure(m, p)?;
        let u = first(m, p)?;
        m.sw(0x4e0628, u)?;
        if s != 65535 {
            let ok = self.core(
                m,
                "BatimentPeutAttaquer",
                [s, mode, target, target],
                0x42a4dc,
            )? & 255
                != 0;
            if !ok {
                return self.cursor(m, 0x1e0e, 0);
            }
            self.cursor(m, 0x1dfd, m.d(0x508f80)? % 6)?;
            if self.clicked(m)? {
                self.ord(m, p, 14, [mode, target, 0, 0], 0x42a52c)?;
                m.sw(PAGE, 0)?;
            }
            return Ok(());
        }
        if u == 65535 {
            return Ok(());
        }
        if !can_attack(m, p, mode, target)? && kind(m, target)? != 10 {
            return self.cursor(m, 0x1e0e, 0);
        }
        self.cursor(m, 0x1dfd, m.d(0x508f80)? % 6)?;
        if self.clicked(m)? {
            self.ord(m, p, 14, [mode, target, 0, 0], 0x42a5e6)?;
            m.sw(PAGE, 0)?;
        }
        Ok(())
    }
    pub(super) fn sell_page(&self, m: &mut MemoriaJogo, p: u32, c: u32) -> Result<()> {
        let s = structure_at(m, c)?;
        m.sw(0x496ac4, s)?;
        self.cursor(m, 0x1e0e, 0)?;
        if s == 65535
            || m.b(player::structure(s) + 4)? != p
            || leaf(m, "BatimentPeutSeVendre", s, 0, 0)? & 255 == 0
        {
            return Ok(());
        }
        self.cursor(m, 0x1e1c, m.d(0x508f80)? % 6)?;
        if self.clicked(m)? {
            self.ord(m, p, 10, [s, 0, 0, 0], 0x42a6a3)?;
            self.core(m, "BatimentDeselectionner", [p, 0, 0, 0], 0x42a6aa)?;
        }
        Ok(())
    }
    pub(super) fn own_structure(
        &self,
        m: &mut MemoriaJogo,
        p: u32,
        s: u32,
        x: u32,
        y: u32,
        c: u32,
    ) -> Result<()> {
        if m.b(player::structure(s) + 4)? != p {
            return Ok(());
        }
        self.cursor(m, 0x1df0, m.d(0x508f80)? % 5)?;
        if m.b(AE)? != 0 {
            self.drag(m, x, y, c)?;
        }
        Ok(())
    }
    pub(super) fn structure_troops(
        &self,
        m: &mut MemoriaJogo,
        p: u32,
        s: u32,
        x: u32,
        y: u32,
        c: u32,
    ) -> Result<()> {
        let r = player::structure(s);
        if m.w(PAGE)? == 8 {
            if m.b(r + 4)? == p
                && m.b(r + 8)? == 3
                && (all_type(m, p, 5)? || all_type(m, p, 7)? || all_type(m, p, 6)?)
            {
                self.cursor(m, 0x1e3d, m.d(0x508f84)? % 3)?;
                if self.clicked(m)? {
                    m.sw(0x4e05b0, 3)?;
                    m.sw(0x4e05b2, s)?;
                    m.sw(PAGE, 9)?;
                    m.sb(
                        0x4e05b4,
                        if all_type(m, p, 7)? {
                            2
                        } else if all_type(m, p, 5)? {
                            1
                        } else {
                            0
                        },
                    )?;
                }
                return Ok(());
            }
            self.cursor(m, 0x1e0e, 0)?;
            if self.clicked(m)? {
                self.ord(m, p, 5, [c, 0, 0, 0], 0x42ad8e)?;
                self.point(m, c)?;
            }
            return Ok(());
        }
        if leaf(m, "BatimentEstEnnemi", s, p, 0)? & 255 != 0
            && leaf(m, "BatimentEstNeutre", s, 0, 0)? & 255 == 0
        {
            if matches!(m.b(r + 8)?, 16 | 2) && all_type(m, p, 10)? {
                self.cursor(m, 0x1e6f, m.d(0x508f80)? & 1)?;
                if self.clicked(m)? {
                    self.ord(m, p, 16, [3, s, 0, 0], 0x42aecf)?;
                }
                return Ok(());
            }
            if !can_attack(m, p, 3, s)? {
                return Ok(());
            }
            self.cursor(m, 0x1dfd, m.d(0x508f80)? % 6)?;
            if self.clicked(m)? {
                self.ord(m, p, 14, [3, s, 0, 0], 0x42aecf)?;
            }
            return Ok(());
        }
        self.cursor(m, 0x1df0, m.d(0x508f80)? % 5)?;
        if kind(m, first(m, p)?)? == 7 && (!built(m, s)? || m.w(r + 6)? != 0) {
            self.cursor(m, 0x1e10, m.d(0x508f80)? % 6)?;
            if self.clicked(m)? {
                self.ord(m, p, 20, [3, s, 0, 0], 0x42af96)?;
                self.point(m, c)?;
            }
            return Ok(());
        }
        if leaf(m, "EnginSelectEstBlesse", p, 0, 0)? & 255 != 0 && m.b(r + 4)? == p {
            let t = m.b(r + 8)?;
            let target = leaf(m, "EnginSelectEstTroupe", p, 0, 0)? & 255 != 0 && t == 19
                || leaf(m, "EnginSelectEstEngin", p, 0, 0)? & 255 != 0 && t == 14;
            if target && leaf(m, "EnginSelectContientMiroir", p, 0, 0)? & 255 == 0 {
                if !built(m, s)? {
                    return Ok(());
                }
                self.cursor(m, 0x1e3b, m.d(0x508f80)? & 1)?;
                if self.clicked(m)? {
                    self.ord(m, p, 21, [s, 0, 0, 0], 0x42aecf)?;
                }
                return Ok(());
            }
        }
        if enter_structure(m, p, s)? && leaf(m, "EnginSelectContientMiroir", p, 0, 0)? & 255 == 0 {
            if built(m, s)? {
                self.cursor(m, 0x1e16, m.d(0x508f80)? % 6)?;
                if m.b(r + 8)? == 21 {
                    self.cursor(m, 0x1eac, m.d(0x508f80)? % 9)?;
                }
                if self.clicked(m)? {
                    self.ord(m, p, 16, [3, s, 0, 0], 0x42aecf)?;
                }
                return Ok(());
            }
            self.cursor(m, 0x1e0e, 0)?;
            if self.clicked(m)? {
                self.ord(m, p, 5, [c, 0, 0, 0], 0x42af96)?;
                self.point(m, c)?;
            }
            return Ok(());
        }
        self.own_structure(m, p, s, x, y, c)
    }
    pub(super) fn arm_deposit(
        &self,
        m: &mut MemoriaJogo,
        c: u32,
        mode: u32,
        cargo: u32,
    ) -> Result<()> {
        self.cursor(m, 0x1e3d, m.d(0x508f84)? % 3)?;
        if !self.clicked(m)? {
            return Ok(());
        }
        m.sw(PAGE, 9)?;
        m.sw(0x4e05b0, mode)?;
        m.sw(0x4e05b2, c)?;
        m.sb(0x4e05b4, cargo)
    }
}
