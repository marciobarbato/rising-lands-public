//! iabDetection0x489b5a and its battle insertion/assignment consumers.
//! Direct intact-asm port. Gameplay/RNG and native iteration order are preserved.
use crate::{frame::stop, player::unit, MemoriaJogo, Result};
fn group(index: u32) -> u32 {
    0x51a9e0u32.wrapping_add(index.wrapping_mul(32))
}
fn war(owner: u8, conflict: u32) -> u32 {
    0x51bd24u32
        .wrapping_add(u32::from(owner) * 4420)
        .wrapping_add(conflict.wrapping_mul(272))
}
fn reference(m: &MemoriaJogo, encoded: u16) -> Result<(u32, bool)> {
    let building = encoded & 0x8000 != 0;
    Ok((
        if building {
            0x49a0c8 + 276 * u32::from(encoded & 0x7fff)
        } else {
            unit(m, u32::from(encoded))?
        },
        building,
    ))
}
fn health(m: &MemoriaJogo, encoded: u16) -> Result<i32> {
    let (r, building) = reference(m, encoded)?;
    Ok(if building {
        i32::from(m.u16(r + 10)?) - i32::from(m.u16(r + 6)?)
    } else {
        i32::from(m.u16(r + 0x9a)?)
    })
}
fn priority(m: &MemoriaJogo, encoded: u16) -> Result<u8> {
    let (r, building) = reference(m, encoded)?;
    let kind = m.u8(r + if building { 8 } else { 1 })?;
    Ok(m.u32(if building { 0x498d5c } else { 0x498cf4 } + 4 * u32::from(kind))? as u8)
}
fn damage(m: &MemoriaJogo, member: u16, encoded: u16) -> Result<u8> {
    let source = m.u8(unit(m, u32::from(member))? + 1)?;
    let (r, building) = reference(m, encoded)?;
    let target = if building { 26 } else { m.u8(r + 1)? };
    m.u8(m.u32(m.u32(0x4e199c)? + 4 * u32::from(source))? + u32::from(target))
}
pub(crate) fn adjacent_units(m: &MemoriaJogo, r: u32, target: u16) -> Result<bool> {
    if m.u8(r + 0x86)? != 0 || m.u8(r + 0x87)? != 0 {
        return Ok(false);
    }
    let kind = m.u8(r + 1)?;
    if kind > 25 {
        return stop("EnginAdjacentEngin size dispatch", 0x446962);
    }
    let mut width = if matches!(kind, 3 | 11..=16 | 25) {
        2
    } else {
        1
    };
    let mut height = width;
    let mut cursor = m
        .u16(r + 8)?
        .wrapping_sub(((height - 1) * 256 + width - 1) as u16) as u32;
    if cursor & 255 != 0 {
        cursor = cursor.wrapping_sub(1);
    } else {
        width -= 1;
    }
    if cursor >> 8 != 0 {
        cursor = cursor.wrapping_sub(256);
    } else {
        height -= 1;
    }
    let x = cursor & 255;
    let y = (cursor & 65535) >> 8;
    let t = unit(m, u32::from(target))?;
    let filter = 1 + u32::from(m.u8(t + 0xab)? != 0);
    for dy in 0..height + 2 {
        if y + dy < 256 {
            for dx in 0..width + 2 {
                if x + dx < 256
                    && crate::frame_units::occupies(m, target, cursor as u16, filter as u8)? != 0
                {
                    return Ok(true);
                }
                cursor = cursor.wrapping_add(1);
            }
        }
        cursor = cursor.wrapping_add(256 - width - 2);
    }
    Ok(false)
}
/// EnginAPortee0x446cb5, unit/building target modes used by battle assignment.
pub(crate) fn in_range(m: &mut MemoriaJogo, member: u16, encoded: u16) -> Result<bool> {
    let r = unit(m, u32::from(member))?;
    let kind = m.u8(r + 1)?;
    let bonus = crate::frame_economy::bonus(m, u32::from(m.u8(r)?), u32::from(kind), 4, 0)?;
    let building = encoded & 0x8000 != 0;
    let target = encoded & 0x7fff;
    let cell = if building {
        crate::frame_core::building_cell(m, u32::from(target))? as u16
    } else {
        m.u16(unit(m, u32::from(target))? + 0x7c)?
    };
    if m.u8(r + 2)? == 24 {
        return crate::spell_range::in_range(m, member, if building { 3 } else { 1 }, target);
    }
    let origin = m.u16(r + 8)?;
    if kind == 16 {
        return Ok(origin == cell);
    }
    if (m.u8(r + 0x78)? == 0 || matches!(kind, 1 | 4 | 21))
        && if building {
            crate::chain_entry::adjacent(m, target, origin)?
        } else {
            adjacent_units(m, r, target)?
        }
    {
        return Ok(true);
    }
    let dx = i32::from(origin & 255) - i32::from(cell & 255);
    let dy = i32::from(origin >> 8) - i32::from(cell >> 8);
    Ok((dx * dx + dy * dy) as u32 <= (u32::from(m.u8(r + 0xb0)?).wrapping_add(bonus) & 65535))
}
/// iagWarAffecteEnnemy0x487eef: near pass followed by unrestricted pass.
///
/// The original walks the +0x108 links until 0xffff or until the assigned
/// damage reaches the target's health, with no cycle check. AI group lists can
/// become cyclic in the original (a unit prepended to a group it is still
/// linked in). A revisit is then harmless when damage was assigned since the
/// member's previous visit: members whose SetAttaqueEngin leaves +0xb8 free
/// (types 5/6/7/9/10/13) stay eligible and keep adding damage, so the native
/// loop ends on the health test. Only a revisit with no damage assigned since
/// the previous one is the original's endless loop: nothing between the two
/// visits wrote memory (in_range, damage and health are reads), so every later
/// lap repeats it. Port deviation (beta-6 player report, 0x488003): instead of
/// freezing, that pass ends there, as at the list end, since every further lap
/// would assign nothing. An index outside the 800-unit table still stops.
/// frame_raid::detach keeps new cycles from forming; this covers saves and
/// worlds whose lists are already cyclic.
fn assign(m: &mut MemoriaJogo, battle: u32, index: u32) -> Result<()> {
    let g = group(m.u32(battle)?);
    if g == 0 || m.u16(g + 0x12)? == 0 || m.u16(g + 0x10)? == 65535 {
        return Ok(());
    }
    let entry = battle.wrapping_add(16 + 8 * index);
    let encoded = m.u16(entry)?;
    let mut assigned = u32::from(m.u16(entry + 2)?);
    let hp = health(m, encoded)?;
    for near in [true, false] {
        if assigned as i32 >= hp {
            break;
        }
        let mut member = m.u16(g + 0x10)?;
        // Assigned damage at each member's previous visit in this pass.
        let mut last_visit: [Option<u32>; 800] = [None; 800];
        while member != 65535 && (assigned as i32) < hp {
            if member >= 800 {
                return stop("iagWarAffecteEnnemy invalid group list", 0x488003);
            }
            let slot = &mut last_visit[usize::from(member)];
            if *slot == Some(assigned) {
                break;
            }
            *slot = Some(assigned);
            let r = unit(m, u32::from(member))?;
            if m.u16(r + 0xb8)? == 65535 && (!near || in_range(m, member, encoded)?) {
                let amount = damage(m, member, encoded)?;
                if amount > 0 {
                    assigned = assigned.wrapping_add(u32::from(amount));
                    crate::player_orders::call(
                        m,
                        "SetAttaqueEngin",
                        [
                            u32::from(member),
                            if encoded & 0x8000 != 0 { 3 } else { 1 },
                            u32::from(encoded & 0x7fff),
                            0,
                        ],
                        0x4880dd,
                    )?;
                }
            }
            member = m.u16(r + 0x108)?;
        }
    }
    m.set_u16(entry + 2, assigned as u16)
}
/// iagWarAddEnnemyToBattle0x487b78; stable insertion and native duplicate path.
pub(crate) fn add_enemy(
    m: &mut MemoriaJogo,
    owner: u8,
    conflict: u32,
    encoded: u16,
) -> Result<bool> {
    let (r, building) = reference(m, encoded)?;
    if m.u8(r + if building { 4 } else { 0 })? == owner {
        return Ok(false);
    }
    let battle = war(owner, conflict);
    let count = u32::from(m.u16(battle + 14)?);
    if count >= 32 {
        return Ok(false);
    }
    let priority = priority(m, encoded)?;
    let mut position = None;
    let mut candidate = None;
    for i in 0..count {
        let e = battle + 16 + 8 * i;
        if m.u16(e)? == encoded {
            if m.u16(e + 2)? == 0 {
                assign(m, battle, i)?;
            }
            return Ok(false);
        }
        let old_priority = m.u16(e + 4)?;
        if old_priority == u16::from(priority) {
            if health(m, encoded)? < health(m, m.u16(e)?)? {
                candidate = Some(i);
            }
        } else if old_priority < u16::from(priority) {
            position = Some(candidate.unwrap_or(i));
            break;
        }
    }
    let position = position.unwrap_or(count);
    for i in (position..count).rev() {
        let e = battle + 16 + 8 * i;
        let bytes = m.read(e, 8)?.to_vec();
        m.write(e + 8, &bytes)?;
    }
    let e = battle + 16 + 8 * position;
    m.set_u16(e, encoded)?;
    m.set_u16(e + 2, 0)?;
    m.set_u16(e + 4, u16::from(priority))?;
    m.set_u16(battle + 14, count as u16 + 1)?;
    assign(m, battle, position)?;
    Ok(true)
}
/// iabDetection_AjouteEnnemiOuAlerteConflit0x489b5a.
pub(crate) fn detection(m: &mut MemoriaJogo, member: u16, encoded: u16) -> Result<()> {
    let r = unit(m, u32::from(member))?;
    let g = group(m.u32(r + 0x10c)?);
    if g == 0 {
        return Ok(());
    }
    let conflict = m.u32(g + 12)?;
    if conflict == u32::MAX {
        return Ok(());
    }
    if add_enemy(m, m.u8(r)?, conflict, encoded)? {
        // Reload the group field after assignment, exactly as at0x489bae.
        let task = m.u32(war(m.u8(r)?, m.u32(g + 12)?) + 8)?;
        if task != u32::MAX {
            crate::frame_tasks::wake(m, task, u32::MAX)?;
        }
    }
    Ok(())
}

pub(crate) fn allied(m: &MemoriaJogo, a: u8, b: u8) -> Result<bool> {
    if a == b {
        return Ok(true);
    }
    if a > 4 || b > 4 {
        return Ok(false);
    }
    Ok(m.u8(m.u32(0x5080a8)? + 20 * u32::from(a) + 6)? & (1 << b) != 0)
}
/// EnginEstEnnemy0x446fbf, including its early player4-vs-neutral branch.
pub(crate) fn enemy_unit(m: &MemoriaJogo, target: u16, owner: u8) -> Result<bool> {
    let other = m.u8(unit(m, u32::from(target))?)?;
    if owner != other && (owner == 4 || other == 4) {
        return Ok(true);
    }
    Ok(owner != 200 && other != 200 && owner != other && !allied(m, owner, other)?)
}
fn wake_battle(m: &mut MemoriaJogo, owner: u8, conflict: u32, message: u32) -> Result<()> {
    let task = m.u32(war(owner, conflict) + 8)?;
    if task != u32::MAX {
        crate::frame_tasks::wake(m, task, message)?;
    }
    Ok(())
}
/// iagWarRemoveEnnemy0x487e2a: compact the first matching entry; tail retained.
fn remove_enemy(m: &mut MemoriaJogo, owner: u8, conflict: u32, encoded: u16) -> Result<()> {
    let (r, building) = reference(m, encoded)?;
    if m.u8(r + if building { 4 } else { 0 })? == owner || encoded == 65535 {
        return Ok(());
    }
    let b = war(owner, conflict);
    let count = m.u16(b + 14)?;
    if count > 32 {
        return stop("iagWarRemoveEnnemy list count", 0x487e2a);
    }
    for i in 0..u32::from(count) {
        if m.u16(b + 16 + 8 * i)? == encoded {
            for j in i..u32::from(count) - 1 {
                let next = m.read(b + 16 + 8 * (j + 1), 8)?.to_vec();
                m.write(b + 16 + 8 * j, &next)?;
            }
            m.set_u16(b + 14, count - 1)?;
            break;
        }
    }
    Ok(())
}
fn target_order(m: &mut MemoriaJogo, member: u16, encoded: u16, site: u32) -> Result<()> {
    crate::player_orders::call(
        m,
        "SetAttaqueEngin",
        [
            u32::from(member),
            if encoded & 0x8000 != 0 { 3 } else { 1 },
            u32::from(encoded & 0x7fff),
            0,
        ],
        site,
    )?;
    Ok(())
}
/// iagWarGetEnnemy0x48829d: scan when empty, near/free allocation then fallback.
fn select_enemy(m: &mut MemoriaJogo, member: u16, conflict: u32) -> Result<u16> {
    let r = unit(m, u32::from(member))?;
    let owner = m.u8(r)?;
    let b = war(owner, conflict);
    if m.u16(b + 14)? == 0 {
        let position = m.u32(b + 4)? as i32;
        let radius = i32::from(m.u16(b + 12)?);
        if radius > 255 {
            return stop("iagWarGetEnnemy scan radius", 0x488314);
        }
        let (cx, cy) = (position % 256, position / 256);
        let mut best = 0i32;
        let mut found = 65535u16;
        let grid = m.u32(0x4e0f38)?;
        for y in (cy - radius).max(0)..=(cy + radius).min(255) {
            for x in (cx - radius).max(0)..=(cx + radius).min(255) {
                let cell = (y * 256 + x) as u16;
                let raw = m.u16(grid + 2 * u32::from(cell))?;
                if raw & 0xc000 == 0x4000 {
                    let target = (raw & 0x3ff8) >> 3;
                    let t = unit(m, u32::from(target))?;
                    if !matches!(m.u8(t + 2)?, 0 | 4)
                        && m.u16(t + 0x7c)? == cell
                        && enemy_unit(m, target, owner)?
                    {
                        let priority = m.u32(0x498cf4 + 4 * u32::from(m.u8(t + 1)?))? as i32;
                        if priority > best {
                            best = priority;
                            found = target;
                        }
                    }
                }
                if raw & 0x8000 != 0 {
                    let target = (raw & 0x7fe0) >> 5;
                    let t = 0x49a0c8 + 276 * u32::from(target);
                    if !matches!(m.u8(t + 5)?, 0 | 3) {
                        let other = m.u8(t + 4)?;
                        // BatimentEstEnnemy uses the target-to-source alliance direction.
                        if other != 200 && !allied(m, other, owner)? {
                            let priority = m.u32(0x498d5c + 4 * u32::from(m.u8(t + 8)?))? as i32;
                            if priority > best {
                                best = priority;
                                found = target | 0x8000;
                            }
                        }
                    }
                }
            }
        }
        if found == 65535 {
            return Ok(65535);
        }
        add_enemy(m, owner, conflict, found)?;
    }
    let count = m.u16(b + 14)?;
    if count > 32 {
        return stop("iagWarGetEnnemy list count", 0x488518);
    }
    for near in [true, false] {
        for i in 0..u32::from(count) {
            let e = b + 16 + 8 * i;
            let encoded = m.u16(e)?;
            if health(m, encoded)? <= i32::from(m.u16(e + 2)?)
                || near && !in_range(m, member, encoded)?
            {
                continue;
            }
            let amount = damage(m, member, encoded)?;
            if amount == 0 {
                continue;
            }
            m.set_u16(e + 2, m.u16(e + 2)?.wrapping_add(u16::from(amount)))?;
            target_order(m, member, encoded, 0x488688)?;
            return m.u16(e);
        }
    }
    // Native fallback deliberately uses the first record even if its HP is
    // already fully allocated; it also preserves a zero-damage assignment.
    let encoded = m.u16(b + 16)?;
    let amount = damage(m, member, encoded)?;
    m.set_u16(b + 18, m.u16(b + 18)?.wrapping_add(u16::from(amount)))?;
    target_order(m, member, encoded, 0x48872d)?;
    m.u16(b + 16)
}
/// iab attack-ended handler0x489ca2, before the dispatcher's task notification.
pub(crate) fn ended(m: &mut MemoriaJogo, member: u16) -> Result<()> {
    let r = unit(m, u32::from(member))?;
    let g = group(m.u32(r + 0x10c)?);
    let conflict = m.u32(g + 12)?;
    if conflict != u32::MAX {
        match m.u8(r + 0xb7)? {
            1 => remove_enemy(m, m.u8(r)?, conflict, m.u16(r + 0xb8)?)?,
            3 => remove_enemy(m, m.u8(r)?, conflict, m.u16(r + 0xb8)? | 0x8000)?,
            _ => {}
        }
        if select_enemy(m, member, conflict)? != 65535 {
            return Ok(());
        }
    }
    crate::frame_spawn::call(
        m,
        "SetDeplaceEngin",
        &[u32::from(member), u32::from(m.u16(r + 0x7c)?)],
    )?;
    m.set_u16(r + 0xb8, 65535)
}
/// iagWarRemoveUnitContribution0x48873f. The member's target/link stay intact.
fn unassign(m: &mut MemoriaJogo, member: u16, conflict: u32) -> Result<()> {
    if member == 65535 {
        return Ok(());
    }
    let r = unit(m, u32::from(member))?;
    let encoded = m.u16(r + 0xb8)?;
    if encoded == 65535 {
        return Ok(());
    }
    let b = war(m.u8(r)?, conflict);
    let count = m.u16(b + 14)?;
    if count > 32 {
        return stop("iagWarRemoveUnitContribution list count", 0x48873f);
    }
    for i in 0..u32::from(count) {
        let e = b + 16 + 8 * i;
        if m.u16(e)? == encoded {
            let amount = damage(m, member, encoded)?;
            m.set_u16(e + 2, m.u16(e + 2)?.saturating_sub(u16::from(amount)))?;
            break;
        }
    }
    Ok(())
}
pub(crate) fn dead_message(m: &mut MemoriaJogo, member: u16, index: u32) -> Result<()> {
    let r = unit(m, u32::from(member))?;
    let owner = m.u8(r)?;
    let g = group(index);
    let conflict = m.u32(g + 12)?;
    if conflict != u32::MAX {
        unassign(m, member, conflict)?;
        wake_battle(m, owner, conflict, u32::MAX)?;
    }
    crate::frame_raid::remove_unit(m, index, member)?;
    let flags = m.u32(g + 0x1c)?;
    if flags != 0 {
        let task = m.u32(g + 8)?;
        if task != u32::MAX {
            crate::frame_tasks::wake(m, task, if flags & 1 != 0 { 1 } else { u32::MAX })?;
        }
    }
    Ok(())
}
pub(crate) fn ended_notification(m: &mut MemoriaJogo, member: u16) -> Result<()> {
    let r = unit(m, u32::from(member))?;
    let g = group(m.u32(r + 0x10c)?);
    let conflict = m.u32(g + 12)?;
    if conflict != u32::MAX {
        wake_battle(m, m.u8(r)?, conflict, 1)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNITS: u32 = 0x0100_0000;
    const DAMAGE_ROWS: u32 = 0x0200_0000;
    const BATTLE: u32 = 0x51bd24;
    const GROUP: u32 = 7;

    /// Two monster-owned type5 members (SetAttaqueEngin leaves +0xb8 free)
    /// linked in a cycle 1 -> 2 -> 1, attacking unit 3 with `hp` health.
    fn cyclic_world(hp: u16, damage: u8) -> MemoriaJogo {
        let mut m = MemoriaJogo::from_blocks([
            (0x494000, vec![0; 0x9c000]),
            (UNITS, vec![0; 800 * 276]),
            (DAMAGE_ROWS, vec![0; 0x1000]),
        ])
        .unwrap();
        m.set_u32(0x4e19a0, UNITS).unwrap();
        m.set_u32(0x4e199c, DAMAGE_ROWS).unwrap();
        m.set_u32(DAMAGE_ROWS + 4 * 5, DAMAGE_ROWS + 0x100).unwrap();
        m.set_u8(DAMAGE_ROWS + 0x100 + 2, damage).unwrap();
        m.set_u16(0x496304, 0xffff).unwrap();
        for (member, next) in [(1u32, 2u16), (2, 1)] {
            let r = UNITS + 276 * member;
            m.set_u8(r, 4).unwrap();
            m.set_u8(r + 1, 5).unwrap();
            m.set_u8(r + 2, 7).unwrap();
            m.set_u8(r + 0x78, 1).unwrap();
            m.set_u8(r + 0xb0, 255).unwrap();
            m.set_u16(r + 0xb8, 0xffff).unwrap();
            m.set_u16(r + 0xc0, 0xffff).unwrap();
            m.set_u16(r + 0x108, next).unwrap();
            m.set_u32(r + 0x10c, GROUP).unwrap();
        }
        let target = UNITS + 276 * 3;
        m.set_u8(target, 1).unwrap();
        m.set_u8(target + 1, 2).unwrap();
        m.set_u16(target + 0x9a, hp).unwrap();
        let g = group(GROUP);
        m.set_u16(g + 0x10, 1).unwrap();
        m.set_u16(g + 0x12, 2).unwrap();
        m.set_u32(BATTLE, GROUP).unwrap();
        m.set_u16(BATTLE + 16, 3).unwrap();
        m
    }

    #[test]
    fn cyclic_list_with_progress_ends_on_health_like_the_original() {
        let mut m = cyclic_world(10, 3);
        assign(&mut m, BATTLE, 0).unwrap();
        // 1, 2, 1, 2: four visits of 3 damage reach the 10 health.
        assert_eq!(m.u16(BATTLE + 16 + 2).unwrap(), 12);
    }

    #[test]
    fn cyclic_list_without_progress_ends_the_pass_instead_of_freezing() {
        // The original loops forever here (no member adds damage); the port
        // ends the near and the unrestricted pass at the first revisit.
        let mut m = cyclic_world(10, 0);
        assign(&mut m, BATTLE, 0).unwrap();
        assert_eq!(m.u16(BATTLE + 16 + 2).unwrap(), 0);
    }
}
