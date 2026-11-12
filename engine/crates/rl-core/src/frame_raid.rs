//! MONDE01 monster spawning, group actions and battle-task rules.
//! Mechanical original-width port of incursao_ia.py/regras_incursao_ia.py.
use crate::{EngineError, MemoriaJogo, Result};
const GROUPS: u32 = 0x51a9e0;
const ACTIONS: u32 = 0x50ae80;
const ACTION_ENTRIES: u32 = 0x50ae84;
const PENDING: u32 = 0x50b134;
const LIVE: u32 = 0x50b178;
const ITER_UNIT: u32 = 0x51b848;
fn err(name: &'static str, address: u32) -> EngineError {
    EngineError::UnportedCallee { name, address }
}
fn group(index: u32) -> u32 {
    GROUPS.wrapping_add(index.wrapping_shl(5))
}
fn unit(m: &MemoriaJogo, index: u16) -> Result<u32> {
    Ok(m.u32(0x4e19a0)?.wrapping_add(u32::from(index) * 276))
}
fn provider(m: &mut MemoriaJogo, name: &str, a: &[u32]) -> Result<u32> {
    match name {
        "Random" => crate::frame_core::call(m, name, a),
        "PlayerGetType" => crate::frame_economy::call(m, name, a),
        "ProjectileInit" => crate::frame::call(m, name, a),
        "iaProbPost" => crate::frame_tasks::call(m, name, a),
        _ => crate::frame_movement::call(m, name, a),
    }
}
pub fn new_group(m: &mut MemoriaJogo, player: u32, kind: u32) -> Result<u32> {
    let cursor = m.u32(0x51b660)?;
    let mut index = cursor;
    let mut offset = cursor.wrapping_shl(5);
    let found = loop {
        if (offset as i32) >= 0xc80 {
            break None;
        }
        if m.u8(GROUPS.wrapping_add(offset).wrapping_add(0x18))? == 0 {
            break Some(index);
        }
        offset = offset.wrapping_add(32);
        index = index.wrapping_add(1);
    };
    let Some(index) = found else {
        return Ok(u32::MAX);
    };
    let g = GROUPS.wrapping_add(offset);
    m.set_u32(g, u32::MAX)?;
    m.set_u32(g.wrapping_add(4), u32::MAX)?;
    m.set_u16(g.wrapping_add(0x12), 0)?;
    m.set_u16(g.wrapping_add(0x14), 0x7fff)?;
    m.set_u32(g.wrapping_add(8), u32::MAX)?;
    m.set_u32(g.wrapping_add(12), u32::MAX)?;
    m.set_u16(g.wrapping_add(0x10), 0xffff)?;
    m.set_u8(g.wrapping_add(0x18), 1)?;
    m.set_u8(g.wrapping_add(0x19), kind as u8)?;
    m.set_u8(g.wrapping_add(0x1a), 255)?;
    m.set_u16(g.wrapping_add(0x16), 0xffff)?;
    m.set_u32(g.wrapping_add(0x1c), 0)?;
    m.set_u8(
        g.wrapping_add(0x1b),
        m.u8(0x498ec4u32.wrapping_add(kind.wrapping_mul(4)))?,
    )?;
    let c = m.u32(0x51b660)?;
    m.set_u32(
        0x51b660,
        if (c as i32) < 99 {
            c.wrapping_add(1)
        } else {
            0
        },
    )?;
    let head = 0x51b664u32
        .wrapping_add(player.wrapping_mul(0x60))
        .wrapping_add(kind.wrapping_mul(4));
    let previous = m.u32(head)?;
    m.set_u32(head, index)?;
    m.set_u32(group(index), previous)?;
    Ok(index)
}
pub fn add_unit(m: &mut MemoriaJogo, index: u32, member: u16) -> Result<u32> {
    let r = unit(m, member)?;
    let g = group(index);
    if g == 0 {
        return Ok(0x1a1a0010);
    }
    m.set_u16(r.wrapping_add(0x108), m.u16(g.wrapping_add(0x10))?)?;
    m.set_u32(r.wrapping_add(0x10c), index)?;
    m.set_u16(g.wrapping_add(0x10), member)?;
    m.set_u16(
        g.wrapping_add(0x12),
        m.u16(g.wrapping_add(0x12))?.wrapping_add(1),
    )?;
    Ok(0)
}
pub fn first_unit(m: &mut MemoriaJogo, index: u32) -> Result<u16> {
    let g = group(index);
    m.set_u32(0x51b844, g)?;
    if g == 0 {
        return Ok(0xffff);
    }
    m.set_u16(ITER_UNIT, m.u16(g.wrapping_add(0x10))?)?;
    m.u16(ITER_UNIT)
}
pub fn next_unit(m: &mut MemoriaJogo) -> Result<u16> {
    let current = m.u16(ITER_UNIT)?;
    m.set_u16(ITER_UNIT, m.u16(unit(m, current)?.wrapping_add(0x108))?)?;
    m.u16(ITER_UNIT)
}
/// iagUnitRemoveFromGroup 0x4832b1. The removed node keeps its own link.
pub(crate) fn remove_unit(m: &mut MemoriaJogo, index: u32, member: u16) -> Result<u32> {
    let g = group(index);
    if g == 0 {
        return Ok(0x1a1a0010);
    }
    let mut previous = None;
    let mut current = m.u16(g + 0x10)?;
    while current != 65535 {
        let r = unit(m, current)?;
        let next = m.u16(r + 0x108)?;
        if current == member {
            let link = if let Some(p) = previous {
                unit(m, p)? + 0x108
            } else {
                g + 0x10
            };
            m.set_u16(link, next)?;
            m.set_u16(g + 0x12, m.u16(g + 0x12)?.wrapping_sub(1))?;
            return Ok(0);
        }
        previous = Some(current);
        current = next;
    }
    Ok(0x1a1a0012)
}
pub fn spawn_group(m: &mut MemoriaJogo, cell: u16, kind: u8) -> Result<u32> {
    if kind == 255 || (m.u32(LIVE)? as i32) >= 50 || m.u8(PENDING)? >= 16 {
        return Ok(0);
    }
    let g = new_group(m, 4, 16)?;
    if g == u32::MAX {
        return Ok(0);
    }
    for unit_kind in 0..26 {
        let count = m.u8(0x50af84 + u32::from(kind) * 26 + unit_kind)?;
        for _ in 0..count {
            let free = provider(m, "GetFreeCell", &[u32::from(cell), unit_kind])? as u16;
            if free == 0xffff {
                continue;
            }
            let member = provider(m, "CreerEngin", &[u32::from(free), 1, unit_kind, 4])? as u16;
            if member == 0xffff {
                continue;
            }
            m.set_u32(LIVE, m.u32(LIVE)?.wrapping_add(1))?;
            provider(m, "EnginSetPlayer", &[u32::from(member), 4])?;
            provider(m, "PlaceEngin", &[u32::from(member)])?;
            let r = unit(m, member)?;
            let effect = if provider(m, "Random", &[])? & 1 == 0 {
                0x20
            } else {
                0x1f
            };
            let cell = u32::from(m.u16(r.wrapping_add(8))?);
            provider(m, "ProjectileInit", &[effect, cell, cell, 0, 0, 0, 0, 0, 0])?;
            add_unit(m, g, member)?;
        }
    }
    let n = m.u8(PENDING)?;
    m.set_u32(PENDING + 4 + 4 * u32::from(n), g)?;
    m.set_u8(PENDING, n.wrapping_add(1))?;
    Ok(0)
}
pub fn stock_action(
    m: &mut MemoriaJogo,
    action: u32,
    index: u32,
    cell: u16,
    origin: u16,
) -> Result<u32> {
    let g = group(index);
    if g == 0 {
        return Ok(0);
    }
    let count = m.u32(ACTIONS)? as i32;
    for k in 0..count.max(0) {
        let e = ACTION_ENTRIES.wrapping_add(16u32.wrapping_mul(k as u32));
        if m.u16(e.wrapping_add(12))? == cell && u32::from(m.u16(e.wrapping_add(14))?) == action {
            m.set_u32(e, m.u32(e)?.wrapping_add(u32::from(m.u16(g + 0x12)?)))?;
            while m.u16(g + 0x12)? != 0 {
                let member = first_unit(m, index)?;
                remove_unit(m, index, member)?;
                add_unit(m, m.u32(e + 4)?, member)?;
                if action == 0x30 {
                    let reference = m.u16(m.u32(0x4e0f38)? + 2 * u32::from(cell))?;
                    let structure = if reference & 0x8000 != 0 {
                        (reference & 0x7fe0) >> 5
                    } else {
                        65535
                    };
                    crate::player_orders::call(
                        m,
                        "SetAttaqueEngin",
                        [u32::from(member), 3, u32::from(structure), 0],
                        0x47aa93,
                    )?;
                } else {
                    crate::frame_spawn::call(
                        m,
                        "SetDeplaceEngin",
                        &[u32::from(member), u32::from(m.u16(e + 12)?)],
                    )?;
                }
            }
            // iagGroupSendMessage(4, group's task slot, 1), 0x488cea.
            let slot = m.u32(g + 12)?;
            let task = m.u32(
                0x51bd2cu32
                    .wrapping_add(4 * 4420)
                    .wrapping_add(slot.wrapping_mul(272)),
            )?;
            if task != u32::MAX {
                crate::frame_tasks::wake(m, task, 1)?;
            }
            return Ok(1);
        }
    }
    let n = m.u32(ACTIONS)?;
    if (n as i32) >= 50 {
        return Ok(0);
    }
    let mut e = ACTION_ENTRIES.wrapping_add(n.wrapping_mul(16));
    if action == 0x31 {
        m.set_u32(e, u32::from(m.u16(g.wrapping_add(0x12))?))?;
        m.set_u32(e.wrapping_add(4), index)?;
        m.set_u32(e.wrapping_add(8), 0x1c2)?;
        m.set_u16(e.wrapping_add(12), cell)?;
        m.set_u32(ACTIONS, n.wrapping_add(1))?;
        m.set_u16(e.wrapping_add(14), action as u16)?;
        return Ok(1);
    }
    let task = provider(
        m,
        "iaProbPost",
        &[
            4,
            action,
            index,
            u32::from(cell),
            u32::from(origin),
            0,
            0,
            0,
        ],
    )?;
    // Native DL is zero-extended before comparison with -1, so its failure
    // check never fires. Preserve the task's low byte, including 0xff.
    e = ACTION_ENTRIES.wrapping_add(m.u32(ACTIONS)?.wrapping_mul(16));
    m.set_u32(e, u32::from(m.u16(g.wrapping_add(0x12))?))?;
    m.set_u32(e.wrapping_add(4), index)?;
    m.set_u32(e.wrapping_add(8), task & 255)?;
    m.set_u16(e.wrapping_add(12), cell)?;
    m.set_u32(ACTIONS, m.u32(ACTIONS)?.wrapping_add(1))?;
    m.set_u16(e.wrapping_add(14), action as u16)?;
    Ok(1)
}
fn try_action(m: &mut MemoriaJogo, action: u32, g: u32, cell: u16, origin: u16) -> Result<bool> {
    if provider(
        m,
        "Chemin_Accessible",
        &[u32::from(origin), u32::from(cell)],
    )? as u8
        == 0
    {
        return Ok(false);
    }
    Ok(stock_action(m, action, g, cell, origin)? as u8 != 0)
}
fn conclude(m: &mut MemoriaJogo) -> Result<u32> {
    m.set_u8(PENDING, m.u8(PENDING)?.wrapping_sub(1))?;
    Ok(0)
}
fn ambush_cell(m: &mut MemoriaJogo) -> Result<u16> {
    let mut attempts = 0;
    while attempts < u32::from(m.u16(0x50a9ce)?) {
        let count = u32::from(m.u16(0x50a9ce)?);
        let cursor = m.u32(0x5223d4)?.wrapping_add(1);
        m.set_u32(0x5223d4, cursor)?;
        if (count as i32) <= (cursor as i32) {
            m.set_u32(0x5223d4, 0)?;
        }
        let r = 0x50a9d0u32.wrapping_add(12u32.wrapping_mul(m.u32(0x5223d4)?));
        if m.u16(r.wrapping_add(4))? == 0 && m.u16(r)? != 0xffff {
            return m.u16(r);
        }
        attempts += 1;
    }
    Ok(0xffff)
}
fn pending_attack(m: &mut MemoriaJogo) -> Result<u32> {
    let mut player;
    loop {
        player = (provider(m, "Random", &[])? & 3) + 1;
        if player >= 4 {
            player = 0
        }
        if m.u8(0x50a9bc)? == 0 || provider(m, "PlayerGetType", &[player])? as u8 != 2 {
            break;
        }
    }
    let g = m.u32(PENDING + 4 * u32::from(m.u8(PENDING)?))?;
    let member = first_unit(m, g)?;
    if member == 0xffff {
        // 47b4a3: EAX=4, EDX=group; CALL483ce7, then JMP47b30e.
        crate::ai_rules::delete_group(m, 4, g)?;
        return conclude(m);
    }
    let origin = provider(m, "EnginGetCase", &[u32::from(member)])? as u16;
    let roll = provider(m, "Random", &[])? % 100;
    let kind = provider(m, "EnginGetType", &[u32::from(member)])? as u8;
    if kind == 24 {
        for _ in 0..20 {
            let cell = provider(m, "GetRandCaseChamps", &[])? as u16;
            if cell == 0xffff {
                break;
            }
            if try_action(m, 0x31, g, cell, origin)? {
                return conclude(m);
            }
        }
    }
    let p0 = m.u32(0x50a9c0)?;
    let p1 = m.u32(0x50a9c4)?;
    let p2 = m.u32(0x50a9c8)?;
    let mut mode = if (roll as i64) < i64::from(p0 as i32) {
        player = u32::from(m.u8(0x508041)?);
        0
    } else if (p0.wrapping_add(p1) as i32) > (roll as i32) {
        1
    } else if (p0.wrapping_add(p1).wrapping_add(p2) as i32) > (roll as i32) {
        2
    } else {
        3
    };
    if mode == 2 {
        for _ in 0..20 {
            let cell = ambush_cell(m)?;
            if cell == 0xffff {
                break;
            }
            if try_action(m, 0x2e, g, cell, origin)? {
                return conclude(m);
            }
        }
        mode = 1;
    }
    if mode <= 1 {
        for _ in 0..20 {
            let cell = provider(m, "GetRandCaseTypeBatPlayer", &[player, 3])? as u16;
            if cell == 0xffff {
                break;
            }
            if try_action(m, 0x30, g, cell, origin)? {
                return conclude(m);
            }
        }
    }
    for _ in 0..20 {
        let cell = provider(m, "GetRandCaseChamps", &[])? as u16;
        if cell == 0xffff {
            return Ok(0);
        }
        if try_action(m, 0x2f, g, cell, origin)? {
            return conclude(m);
        }
    }
    Ok(0)
}
pub fn advance(m: &mut MemoriaJogo) -> Result<u32> {
    if m.u8(0x50a9cc)? == 0 {
        m.set_u8(0x50a9cc, 1)?;
        for k in 0..u32::from(m.u16(0x50a9ce)?) {
            m.set_u32(0x50a9d0 + 12 * k + 8, m.u32(0x508f7c)?)?;
        }
    }
    if m.u8(0x496fe4)? == 0x17 {
        crate::monster_advance::pending(m)?;
        return Ok(0);
    }
    let now = m.u32(0x508f7c)?;
    for k in 0..u32::from(m.u16(0x50a9ce)?) {
        let r = 0x50a9d0 + 12 * k;
        let period = m.u16(r + 4)?;
        if period == 0 || (now.wrapping_sub(m.u32(r + 8)?) as i32) < i32::from(period) {
            continue;
        }
        let mask = m.u16(r + 2)?;
        let mut bit = m.u8(r + 6)?;
        let mut found = false;
        for _ in 0..16 {
            bit = bit.wrapping_add(1) & 15;
            if (1u16 << bit) & mask != 0 {
                found = true;
                break;
            }
        }
        if !found {
            bit = 255
        }
        m.set_u8(r + 6, bit)?;
        m.set_u32(r + 8, now)?;
        spawn_group(m, m.u16(r)?, m.u8(r + 6)?)?;
    }
    let mut i = 0u32;
    while (i as i32) < (m.u32(ACTIONS)? as i32) {
        let e = ACTION_ENTRIES.wrapping_add(i.wrapping_mul(16));
        if m.u16(e.wrapping_add(14))? == 0x31 {
            let time = m.u32(e.wrapping_add(8))?.wrapping_sub(1);
            m.set_u32(e.wrapping_add(8), time)?;
            if (time as i32) <= 0 {
                // 0x47b3d1: the ambush timer restarts at 450; an empty group
                // releases its action entry (same slot re-examined) and is
                // deleted for player 4, otherwise every member walks to the cell.
                let index = m.u32(e.wrapping_add(4))?;
                m.set_u32(e.wrapping_add(8), 0x1c2)?;
                let mut member = first_unit(m, index)?;
                if member == 0xffff {
                    rule(m, 0x47b6db, &[0, index], 4, u32::MAX)?;
                    crate::ai_rules::delete_group(m, 4, index)?;
                    i = i.wrapping_sub(1);
                } else {
                    let cell = m.u16(e.wrapping_add(12))?;
                    while member != 0xffff {
                        provider(m, "SetDeplaceEngin", &[u32::from(member), u32::from(cell)])?;
                        member = next_unit(m)?;
                    }
                }
            }
        }
        i = i.wrapping_add(1);
    }
    if m.u8(PENDING)? == 0 {
        Ok(0)
    } else {
        pending_attack(m)
    }
}
fn battle(player: u32, index: u32) -> u32 {
    0x51bd24u32
        .wrapping_add(player.wrapping_mul(0x1144))
        .wrapping_add(index.wrapping_mul(0x110))
}
fn search_battle(m: &MemoriaJogo, player: u32, position: u32) -> Result<u32> {
    let pos = position as i32;
    let x = pos % 256;
    let y = pos / 256;
    for k in 0..16 {
        let b = battle(player, k);
        let other = m.u32(b.wrapping_add(4))? as i32;
        if other == -1 {
            continue;
        }
        let bx = other % 256;
        let by = other / 256;
        let radius = i32::from(m.u16(b.wrapping_add(12))?);
        if x < bx - radius || x > bx + radius || y < by - radius || y > by + radius {
            continue;
        }
        return Ok(k);
    }
    Ok(u32::MAX)
}
/// iagWarNewBattle 0x487643: EAX=player, EDX=position, EBX=radius, ECX=group.
pub(crate) fn new_battle(
    m: &mut MemoriaJogo,
    player: u32,
    position: u32,
    radius: u16,
    index: u32,
) -> Result<u32> {
    for k in 0..16 {
        if m.u32(battle(player, k).wrapping_add(4))? == position {
            return Ok(k);
        }
    }
    for k in 0..16 {
        let b = battle(player, k);
        if m.u32(b.wrapping_add(4))? != u32::MAX {
            continue;
        }
        m.set_u16(b.wrapping_add(14), 0)?;
        m.set_u32(b.wrapping_add(4), position)?;
        m.set_u32(b, index)?;
        m.set_u16(b.wrapping_add(12), radius)?;
        if index != u32::MAX && group(index) != 0 {
            m.set_u32(group(index).wrapping_add(12), k)?;
        }
        return Ok(k);
    }
    Ok(u32::MAX)
}
fn delete_battle(m: &mut MemoriaJogo, player: u32, position: u32) -> Result<()> {
    for k in 0..16 {
        let b = battle(player, k);
        if m.u32(b.wrapping_add(4))? != position {
            continue;
        }
        m.set_u32(b.wrapping_add(4), u32::MAX)?;
        m.set_u32(b.wrapping_add(8), u32::MAX)?;
        m.set_u16(b.wrapping_add(14), 0)?;
        let index = m.u32(b)?;
        m.set_u16(b.wrapping_add(12), 0)?;
        if index != u32::MAX && group(index) != 0 {
            m.set_u32(group(index).wrapping_add(12), u32::MAX)?;
        }
        return Ok(());
    }
    Ok(())
}
pub fn rule(m: &mut MemoriaJogo, address: u32, a: &[u32], player: u32, index: u32) -> Result<u32> {
    let required = match address {
        0x48232f | 0x482550 | 0x47ff22 => 1,
        0x47ff4d | 0x481f4c | 0x47b6db => 2,
        0x47fee0 | 0x482354 | 0x47febf => 3,
        _ => 0,
    };
    if a.len() < required {
        return Err(err("iaSolver incomplete rule arguments", address));
    }
    match address {
        0x48232f => {
            let g = group(a[0]);
            Ok(if g != 0 && m.u16(g.wrapping_add(0x12))? > 0 {
                u32::MAX
            } else {
                1
            })
        }
        0x47fee0 => {
            if search_battle(m, player, a[0])? != u32::MAX {
                return Ok(u32::MAX);
            }
            Ok(
                if new_battle(m, player, a[0], a[1] as u16, a[2])? != u32::MAX {
                    1
                } else {
                    u32::MAX
                },
            )
        }
        0x47ff4d => {
            let g = group(a[0]);
            if g != 0 {
                let b = m.u32(g.wrapping_add(12))?;
                m.set_u32(
                    0x51bd2cu32
                        .wrapping_add(b.wrapping_mul(0x110))
                        .wrapping_add(player.wrapping_mul(0x1144)),
                    index,
                )?;
                m.set_u32(g.wrapping_add(8), index)?;
                if index != u32::MAX {
                    crate::frame_tasks::sleep(m, index, a[1])?;
                }
            }
            Ok(1)
        }
        0x482354 => {
            let g = group(a[0]);
            if g == 0 || m.u16(g.wrapping_add(0x12))? == 0 {
                return Ok(u32::MAX);
            }
            let mut slow = 256;
            let mut member = first_unit(m, a[0])?;
            while member != 0xffff {
                slow = slow.min(u32::from(m.u8(unit(m, member)?.wrapping_add(0x98))?));
                member = next_unit(m)?;
            }
            if slow == 0 {
                return Err(err("I_DetermineTempsTrajet unit speed zero", 0x482427));
            }
            let from = a[1] as i32;
            let to = a[2] as i32;
            let dx = (from % 256 - to % 256).unsigned_abs();
            let dy = (from / 256 - to / 256).unsigned_abs();
            Ok(40u32.wrapping_mul(dx.max(dy)) / slow)
        }
        0x481f4c => {
            let mut member = first_unit(m, a[0])?;
            while member != 0xffff {
                if provider(m, "EnginEstVivant", &[u32::from(member)])? as u8 != 0 {
                    let r = unit(m, member)?;
                    if !(m.u8(r.wrapping_add(2))? == 1 && m.u16(r.wrapping_add(0xb8))? != 0xffff) {
                        provider(m, "SetDeplaceEngin", &[u32::from(member), a[1] & 0xffff])?;
                    }
                }
                member = next_unit(m)?;
            }
            Ok(1)
        }
        0x482550 => {
            let member = first_unit(m, a[0])?;
            if member == 0xffff {
                Ok(u32::MAX)
            } else {
                Ok(provider(m, "EnginGetCase", &[u32::from(member)])? & 0xffff)
            }
        }
        0x47ff22 => {
            delete_battle(m, player, a[0])?;
            Ok(1)
        }
        0x47febf => {
            // B_InitConflit -> 0x4876da: the battle at this position takes the
            // group and radius; a real group record links back to the battle.
            for k in 0..16 {
                let b = battle(player, k);
                if m.u32(b.wrapping_add(4))? != a[0] {
                    continue;
                }
                m.set_u32(b, a[2])?;
                m.set_u16(b.wrapping_add(12), a[1] as u16)?;
                if a[2] != u32::MAX && group(a[2]) != 0 {
                    m.set_u32(group(a[2]).wrapping_add(12), k)?;
                }
                return Ok(1);
            }
            Ok(u32::MAX)
        }
        0x47b6db => {
            let mode = a[0];
            let index = a[1];
            if mode == 1 {
                let count = m.u8(PENDING)?;
                if count < 16 {
                    m.set_u32(PENDING + 4 + 4 * u32::from(count), index)?;
                    m.set_u8(PENDING, count.wrapping_add(1))?;
                }
            }
            let count = m.u32(ACTIONS)?;
            let mut k = 0u32;
            while (k.wrapping_mul(16) as i32) < (count.wrapping_shl(4) as i32)
                && m.u32(
                    ACTION_ENTRIES
                        .wrapping_add(4)
                        .wrapping_add(k.wrapping_mul(16)),
                )? != index
            {
                k = k.wrapping_add(1);
            }
            if (k as i32) < (m.u32(ACTIONS)? as i32) && mode == 0 {
                m.set_u32(
                    LIVE,
                    m.u32(LIVE)?
                        .wrapping_sub(m.u32(ACTION_ENTRIES.wrapping_add(k.wrapping_mul(16)))?),
                )?;
            }
            if (k as i32) < (m.u32(ACTIONS)?.wrapping_sub(1) as i32) {
                let from =
                    ACTION_ENTRIES.wrapping_add(m.u32(ACTIONS)?.wrapping_sub(1).wrapping_mul(16));
                for offset in [0, 4, 8, 12] {
                    m.set_u32(
                        ACTION_ENTRIES
                            .wrapping_add(k.wrapping_mul(16))
                            .wrapping_add(offset),
                        m.u32(from.wrapping_add(offset))?,
                    )?;
                }
            }
            if (k as i32) < (m.u32(ACTIONS)? as i32) {
                m.set_u32(ACTIONS, m.u32(ACTIONS)?.wrapping_sub(1))?;
            }
            Ok(1)
        }
        _ => Err(err("iaSolver unported rule", address)),
    }
}
pub fn call(m: &mut MemoriaJogo, name: &str, a: &[u32]) -> Result<u32> {
    match name {
        "AvanceMonstre" => advance(m),
        "iaGroupNew" => new_group(m, a[0], a[1]),
        "iagUnitAddToGroup" => add_unit(m, a[0], a[1] as u16),
        "iagUnitGetFirstUnitInGroup" => Ok(u32::from(first_unit(m, a[0])?)),
        "iagUnitGetNextUnitInGroup" => Ok(u32::from(next_unit(m)?)),
        "StockeAction" => stock_action(m, a[0], a[1], a[2] as u16, a[3] as u16),
        "ConstruitGroupeMonstre" => spawn_group(m, a[0] as u16, a[1] as u8),
        _ => Err(err("unknown raid provider", 0x47b283)),
    }
}
