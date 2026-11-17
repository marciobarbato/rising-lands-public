//! IaReset 0x47f36e and its complete startup tails. Group primitives are shared.
use crate::{
    frame_raid, frame_spawn,
    mission_load::{fill, require},
    MemoriaJogo, Result, WorldRng,
};
const GROUPS: u32 = 0x51a9e0;
const POOLS: u32 = 0x51b664;
fn group(g: u32) -> u32 {
    GROUPS.wrapping_add(g.wrapping_mul(32))
}
fn unit(m: &MemoriaJogo, u: u16) -> Result<u32> {
    frame_spawn::unit(m, u32::from(u))
}
fn alive(m: &MemoriaJogo, u: u16) -> Result<bool> {
    Ok(!matches!(m.u8(unit(m, u)? + 2)?, 0 | 4))
}
fn player_type(m: &MemoriaJogo, p: u32) -> Result<u8> {
    if p == 4 {
        Ok(2)
    } else {
        m.u8(m.u32(0x5080a8)? + 20 * p + 1)
    }
}
fn random(m: &mut MemoriaJogo, n: u32) -> Result<u32> {
    let mut r = WorldRng {
        seed: m.u32(0x498db4)?,
    };
    let v = r.draw() % n;
    m.set_u32(0x498db4, r.seed)?;
    Ok(v)
}
fn remove(m: &mut MemoriaJogo, g: u32, u: u16) -> Result<()> {
    let a = group(g);
    let mut current = m.u16(a + 16)?;
    let mut previous = None;
    for _ in 0..800 {
        if current == 65535 {
            return Ok(());
        }
        let node = unit(m, current)?;
        let next = m.u16(node + 0x108)?;
        if current == u {
            m.set_u16(previous.unwrap_or(a + 16), next)?;
            m.set_u16(a + 18, m.u16(a + 18)?.wrapping_sub(1))?;
            return Ok(());
        }
        previous = Some(node + 0x108);
        current = next;
    }
    require(false, "IaReset cyclic unit group", 0x4832b1)
}
fn find(m: &MemoriaJogo, g: u32, t: u32, mask: bool) -> Result<u16> {
    if g == u32::MAX {
        return Ok(65535);
    }
    let mut u = m.u16(group(g) + 16)?;
    for _ in 0..800 {
        if u == 65535 {
            return Ok(u);
        }
        let r = unit(m, u)?;
        let kind = u32::from(m.u8(r + 1)?);
        if if mask {
            m.u32(0x4989f8 + 4 * kind)? & (1 << (t & 31)) != 0
        } else {
            kind == t
        } {
            return Ok(u);
        }
        u = m.u16(r + 0x108)?;
    }
    require(false, "IaReset cyclic unit group", 0x483416)?;
    Ok(65535)
}
fn delete(m: &mut MemoriaJogo, p: u32, g: u32) -> Result<()> {
    if g >= 100 {
        return Ok(());
    }
    let a = group(g);
    m.set_u8(a + 24, 0)?;
    if (g as i32) < (m.u32(0x51b660)? as i32) {
        m.set_u32(0x51b660, g)?;
    }
    let head = POOLS + 96 * p + 4 * u32::from(m.u8(a + 25)?);
    let mut cursor = m.u32(head)?;
    let mut previous = head;
    for _ in 0..100 {
        if cursor == u32::MAX {
            return Ok(());
        }
        if cursor == g {
            m.set_u32(previous, m.u32(a)?)?;
            return Ok(());
        }
        previous = group(cursor);
        cursor = m.u32(previous)?;
    }
    require(false, "IaReset cyclic group pool", 0x483d95)
}
/// iagUnitDeleteAllGroups 0x483d95 (EAX = player p). For the player's group
/// lists 1..=19 (heads 0x51b664 + 96p + 4k; list 0 is kept), the head group g of
/// each list in turn: while g's count +0x12 is non-zero, the inline
/// iagUnitGetFirstUnitInGroup stores g's address in 0x51b844 and its first unit
/// +0x10 in 0x51b848; a first unit 0xffff ends the loop, otherwise that unit is
/// removed (iagUnitRemoveFromGroup 0x4832b1) and, when list 0 has a head group
/// g0, pushed at g0's front (unit +0x108 = g0's old first, +0x10c = g0, g0's
/// count + 1). Then iaGroupDelete(p, g) unlinks g and the list head is read
/// again. A group address of zero (g * 32 + 0x51a9e0 wrapping) skips to the
/// delete. The original loops forever on a head iaGroupDelete cannot unlink or
/// on a cyclic unit list: both stay addressed stops.
pub(crate) fn delete_all_groups(m: &mut MemoriaJogo, p: u32) -> Result<()> {
    let pools = POOLS.wrapping_add(96u32.wrapping_mul(p));
    for k in 1..20u32 {
        let head = pools.wrapping_add(4 * k);
        let mut deleted = 0;
        loop {
            let g = m.u32(head)?;
            if g == u32::MAX {
                break;
            }
            deleted += 1;
            require(
                deleted <= 100,
                "iagUnitDeleteAllGroups list head not unlinked",
                0x483eba,
            )?;
            let a = group(g);
            // Each pass removes the head unit (found at step 0), so the count
            // drops by one unless g is list 0's head group g0 (remove then
            // re-add): the original ends after at most the initial count
            // passes even on a cyclic unit list, and loops forever only for
            // g == g0.
            let limit = if a != 0 {
                800.max(u32::from(m.u16(a + 0x12)?))
            } else {
                800
            };
            let mut moved = 0;
            while a != 0 && m.u16(a + 0x12)? != 0 {
                moved += 1;
                require(
                    moved <= limit,
                    "iagUnitDeleteAllGroups cyclic unit group",
                    0x483e11,
                )?;
                m.set_u32(0x51b844, a)?;
                let u = m.u16(a + 0x10)?;
                m.set_u16(0x51b848, u)?;
                if u == 65535 {
                    break;
                }
                remove(m, g, u)?;
                let g0 = m.u32(pools)?;
                if g0 == u32::MAX {
                    continue;
                }
                let a0 = group(g0);
                if a0 == 0 {
                    continue;
                }
                let r = m.u32(0x4e19a0)?.wrapping_add(276 * u32::from(u));
                m.set_u16(r + 0x108, m.u16(a0 + 0x10)?)?;
                m.set_u32(r + 0x10c, g0)?;
                m.set_u16(a0 + 0x10, u)?;
                m.set_u16(a0 + 0x12, m.u16(a0 + 0x12)?.wrapping_add(1))?;
            }
            delete(m, p, g)?;
        }
    }
    Ok(())
}
fn modes(m: &mut MemoriaJogo, p: u32) -> Result<()> {
    m.set_u32(0x51b8a4 + 124 * p, m.u32(0x498c10)?)?;
    m.set_u32(0x51b8b0 + 124 * p, 0)?;
    m.set_u32(0x51ba7c + 168 * p, m.u32(0x498c38)?)?;
    for k in 0..4 {
        m.set_u32(0x51ba6c + 168 * p + 4 * k, m.u32(0x498c60 + 4 * k)?)?;
    }
    m.set_u32(0x51ba84 + 168 * p, m.u32(0x498c4c)?)?;
    m.set_u32(0x51bcfc + 0x1144 * p, m.u32(0x498cb0)?)?;
    m.set_u32(0x51bd04 + 0x1144 * p, m.u32(0x498cc4)?)?;
    Ok(())
}
fn research_branch(m: &mut MemoriaJogo, p: u32, ctx: u32) -> Result<()> {
    if (0..4).all(|k| m.u8(ctx + 5 + k).is_ok_and(|v| v == 255)) {
        return Ok(());
    }
    for b in 0..4 {
        if m.u8(0x5080f4 + 16 * p + b)? >= m.u8(0x498464 + b)? {
            m.set_u8(ctx + 5 + b, 255)?;
        }
    }
    for _ in 0..100000 {
        let draw = random(m, 100)? as i32;
        let mut b = 0;
        while b < 4 && draw >= i32::from(m.u8(ctx + 5 + b)? as i8) {
            b += 1;
        }
        let mut chosen = b;
        if m.u8(ctx + 5 + b)? == 255 {
            chosen = u32::MAX;
            for k in 0..4 {
                if m.u8(ctx + 5 + k)? != 255 {
                    chosen = k;
                    break;
                }
            }
        }
        if chosen == 3 && m.u8(0x496fe4)? < 7 {
            continue;
        }
        m.set_u8(0x5080f4 + 16 * p + 4, chosen as u8)?;
        return Ok(());
    }
    require(false, "IaReset research branch nontermination", 0x47f44c)
}
fn contexts(m: &mut MemoriaJogo) -> Result<()> {
    for p in 0..4 {
        if player_type(m, p)? != 2 {
            continue;
        }
        let ctx = 0x51a98c + 16 * p;
        if (0..4).all(|k| m.u8(ctx + 5 + k).is_ok_and(|v| v == 0)) {
            let favorite = match p {
                3 => 0,
                1 => 1,
                _ => random(m, 4)?,
            };
            let weight = random(m, 20)? + 30;
            let other = (100 - weight) / 3;
            let mut total = 0;
            for b in 0..3 {
                total += if b == favorite { weight } else { other };
                m.set_u8(ctx + 5 + b, total as u8)?;
            }
            m.set_u8(ctx + 8, 100)?;
        }
        research_branch(m, p, ctx)?;
        let mut has = false;
        for u in 0..800u16 {
            if alive(m, u)? && u32::from(m.u8(unit(m, u)?)?) == p {
                has = true;
                break;
            }
        }
        if has {
            m.set_u8(ctx + 4, 0)?;
            m.set_u8(ctx, 0)?;
            modes(m, p)?;
            for (o, v) in [(1, 0), (2, 1), (3, 0), (9, 0)] {
                m.set_u8(ctx + o, v)?;
            }
        } else {
            m.set_u8(ctx + 4, 1)?;
        }
    }
    Ok(())
}
fn tasks(m: &mut MemoriaJogo) -> Result<()> {
    for i in 0..6 {
        let a = 0x5148a0 + 16 * i;
        m.set_u32(a + 4, 100)?;
        m.set_u32(a + 8, 96)?;
        m.set_u32(a + 12, 0x512320)?;
        m.set_u32(a, if i == 0 { 0 } else { u32::MAX })?;
    }
    for i in 0..100 {
        m.set_u32(0x512320 + 96 * i, if i == 99 { u32::MAX } else { i + 1 })?;
    }
    Ok(())
}
fn groups(m: &mut MemoriaJogo) -> Result<()> {
    for i in 0..100 {
        let a = group(i);
        for o in [0, 4, 8, 12] {
            m.set_u32(a + o, u32::MAX)?;
        }
        for (o, v) in [(18, 0), (20, 0x7fff), (22, 65535), (16, 65535)] {
            m.set_u16(a + o, v)?;
        }
        m.set_u8(a + 26, 255)?;
        m.set_u32(a + 28, 0)?;
        m.set_u8(a + 27, 255)?;
        m.set_u8(a + 24, 0)?;
        m.set_u8(a + 25, 0)?;
    }
    m.set_u32(0x51b660, 0)?;
    for p in 0..5 {
        if player_type(m, p)? != 2 {
            continue;
        }
        let pool = POOLS + 96 * p;
        for k in 0..21 {
            m.set_u32(pool + 4 * k, u32::MAX)?;
        }
        if p == 4 {
            continue;
        }
        let g = frame_raid::new_group(m, p, 0)?;
        if g == u32::MAX {
            continue;
        }
        for u in 0..800u16 {
            if alive(m, u)? && u32::from(m.u8(unit(m, u)?)?) == p {
                frame_raid::add_unit(m, g, u)?;
            }
        }
    }
    Ok(())
}
fn cities(m: &mut MemoriaJogo) -> Result<()> {
    for p in 0..4 {
        if player_type(m, p)? != 2 {
            continue;
        }
        let city = 0x51b84c + 124 * p;
        for k in 0..22 {
            m.set_u16(city + 2 * k, 0)?;
            m.set_u16(city + 44 + 2 * k, 0)?;
        }
        m.set_u32(city + 108, 0)?;
        for s in 0..1000 {
            let a = 0x49a0c8 + 276 * s;
            if m.u8(a + 5)? == 0 || u32::from(m.u8(a + 4)?) != p {
                continue;
            }
            let t = u32::from(m.u8(a + 8)?);
            let c = city + 2 * t;
            m.set_u16(c, m.u16(c)?.wrapping_add(1))?;
            m.set_u32(city + 108, m.u32(city + 108)?.wrapping_add(1))?;
            if t == 3 {
                for k in 0..8 {
                    let a = 0x51ba3c + 168 * p + 140 + 2 * k;
                    if m.u16(a)? == 65535 {
                        m.set_u16(a, s as u16)?;
                        break;
                    }
                }
            }
        }
        let g = frame_raid::new_group(m, p, 10)?;
        m.set_u32(city + 96, g)?;
        if g != u32::MAX {
            let g = frame_raid::new_group(m, p, 11)?;
            m.set_u32(city + 92, g)?;
            if g != u32::MAX {
                loop {
                    let pool = m.u32(POOLS + 96 * p)?;
                    let u = find(m, pool, 9, false)?;
                    if u == 65535 {
                        break;
                    }
                    remove(m, pool, u)?;
                    let state = m.u8(unit(m, u)? + 2)?;
                    if matches!(state, 14 | 7) {
                        frame_raid::add_unit(
                            m,
                            m.u32(city + if state == 14 { 96 } else { 92 })?,
                            u,
                        )?;
                    }
                }
            }
        }
        m.set_u32(city + 104, u32::MAX)?;
    }
    Ok(())
}
fn resources(m: &mut MemoriaJogo) -> Result<()> {
    fill(m, 0x51a9cc, 20, 0)?;
    for p in 0..4 {
        if player_type(m, p)? != 2 {
            continue;
        }
        let a = 0x51ba3c + 168 * p;
        for (k, r) in [(0, 1), (1, 2), (2, 0)] {
            let b = 0x4e2ea8 + 32 * p + 8 * r;
            let amount = m.u16(b + 2)?;
            m.set_u32(a + 4 * k, u32::from(amount))?;
        }
        for k in 0..4 {
            m.set_u32(a + 16 + 4 * k, m.u32(a + 4 * k)?)?;
        }
        for o in [32, 36, 40] {
            m.set_u32(a + o, 0)?;
        }
        m.set_u32(a + 68, u32::MAX)?;
        for k in 0..5 {
            m.set_u32(a + 88 + 8 * k, u32::MAX)?;
            m.set_u32(a + 84 + 8 * k, u32::MAX)?;
        }
        for o in [132, 136] {
            m.set_u32(a + o, 0)?;
        }
        for o in [128, 124] {
            m.set_u32(a + o, u32::MAX)?;
        }
        for k in 0..8 {
            m.set_u16(a + 140 + 2 * k, 65535)?;
        }
        require(
            frame_raid::new_group(m, p, 7)? != u32::MAX,
            "iagRessReset group allocation",
            0x485002,
        )?;
        require(
            frame_raid::new_group(m, p, 3)? != u32::MAX,
            "iagRessReset group allocation",
            0x485002,
        )?;
        m.set_u8(a + 157, 0)?;
        m.set_u8(a + 156, 0)?;
    }
    Ok(())
}
fn war(m: &mut MemoriaJogo) -> Result<()> {
    for p in 0..4 {
        if player_type(m, p)? != 2 {
            continue;
        }
        let a = 0x51bcfc + 0x1144 * p;
        for o in [12, 20, 16] {
            m.set_u32(a + o, 255)?;
        }
        let g = frame_raid::new_group(m, p, 9)?;
        m.set_u32(a + 4, g)?;
        if g != 0 {
            let mut strength = 0u32;
            loop {
                let pool = m.u32(POOLS + 96 * p)?;
                let u = find(m, pool, 9, true)?;
                if u == 65535 {
                    break;
                }
                let limit = m.u32(0x51b8b8 + 124 * p)?.wrapping_mul(m.u32(a + 8)?);
                if (strength as i32) >= (limit as i32) {
                    break;
                }
                remove(m, pool, u)?;
                frame_raid::add_unit(m, g, u)?;
                let t = u32::from(m.u8(unit(m, u)? + 1)?);
                strength = strength.wrapping_add(m.u32(0x498cf4 + 4 * t)?);
            }
        }
        for k in 0..16 {
            let e = a + 40 + 272 * k;
            for o in [0, 4, 8] {
                m.set_u32(e + o, u32::MAX)?;
            }
            m.set_u16(e + 12, 0)?;
            m.set_u16(e + 14, 0)?;
        }
    }
    for k in 0..16 {
        let e = 0x52020c + 40 + 272 * k;
        for o in [0, 4] {
            m.set_u32(e + o, u32::MAX)?;
        }
        m.set_u16(e + 12, 0)?;
        m.set_u16(e + 14, 0)?;
    }
    Ok(())
}
fn register(m: &mut MemoriaJogo, g: u32) -> Result<()> {
    let n = u32::from(m.u8(0x50b134)?);
    m.set_u32(0x50b138 + 4 * n, g)?;
    m.set_u8(0x50b134, (n as u8).wrapping_add(1))
}
fn monsters(m: &mut MemoriaJogo) -> Result<()> {
    m.set_u8(0x50a9cc, 0)?;
    m.set_u32(0x50ae80, 0)?;
    m.set_u8(0x50b134, 0)?;
    m.set_u32(0x50b178, 0)?;
    let reserve = frame_raid::new_group(m, 4, 16)?;
    if reserve == u32::MAX {
        return Ok(());
    }
    for u in 0..800u16 {
        if !alive(m, u)? {
            continue;
        }
        let a = unit(m, u)?;
        let t = m.u8(a + 1)?;
        if !(20..=24).contains(&t) {
            continue;
        }
        frame_raid::add_unit(m, reserve, u)?;
        frame_spawn::call(m, "EnginSetPlayer", &[u32::from(u), 4])?;
        m.set_u32(0x50b178, m.u32(0x50b178)?.wrapping_add(1))?;
    }
    if m.u8(0x496fe4)? == 23 {
        return register(m, reserve);
    }
    let mut kinds = Vec::new();
    let mut quantities = Vec::new();
    for g in 0..16 {
        let n = m.u8(0x50b124 + g)?;
        if n != 0 {
            kinds.push(g);
            quantities.push(n);
        }
    }
    for i in 0..kinds.len().saturating_sub(1) {
        for j in i + 1..kinds.len() {
            if quantities[j] > quantities[i] {
                quantities.swap(i, j);
                kinds.swap(i, j);
            }
        }
    }
    for spec in kinds {
        if m.u32(0x50b178)? as i32 >= 50 || m.u8(0x50b134)? >= 16 {
            break;
        }
        let mut more = true;
        while more && (m.u32(0x50b178)? as i32) < 50 && m.u8(0x50b134)? < 16 {
            let g = frame_raid::new_group(m, 4, 16)?;
            if g == u32::MAX {
                break;
            }
            for t in 0..26 {
                if m.u32(0x50b178)? as i32 >= 50 {
                    break;
                }
                let n = m.u8(0x50af84 + 26 * spec + t)?;
                for _ in 0..n {
                    let u = find(m, reserve, t, false)?;
                    if u == 65535 {
                        more = false;
                        break;
                    }
                    remove(m, reserve, u)?;
                    frame_raid::add_unit(m, g, u)?;
                }
            }
            if frame_raid::first_unit(m, g)? == 65535 {
                delete(m, 4, g)?;
                require(!more, "InitMissionMonstre empty group loop", 0x47b1f4)?;
                continue;
            }
            if m.u8(0x50b134)? >= 16 {
                break;
            }
            register(m, g)?;
        }
    }
    if frame_raid::first_unit(m, reserve)? == 65535 {
        delete(m, 4, reserve)?;
    } else if m.u8(0x50b134)? < 16 {
        register(m, reserve)?;
    }
    m.set_u32(0x50b180, 65535)?;
    m.set_u32(0x50b17c, 3)
}
pub fn reset(m: &mut MemoriaJogo) -> Result<()> {
    contexts(m)?;
    tasks(m)?;
    groups(m)?;
    cities(m)?;
    resources(m)?;
    war(m)?;
    monsters(m)
}
