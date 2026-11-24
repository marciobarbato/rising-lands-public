//! Intact RULE leaves used by the city, resource and war schedulers.
//! See validation/ai-rules/HANDOFF.md for native contracts and frontier.
use crate::{EngineError, FrameRegisters as R, MemoriaJogo, Result};

fn stop(name: &'static str, address: u32) -> EngineError {
    EngineError::UnportedCallee { name, address }
}

fn group(index: u32) -> u32 {
    0x51a9e0u32.wrapping_add(index.wrapping_shl(5))
}

/// iagUnitReturnGroupToPoolDownToMax 0x4831a5: while the group holds more
/// members (+0x12) than its maximum (+0x14), the head member leaves the group
/// (iagUnitRemoveFromGroup) and joins the player's pool group (index at
/// 0x51b664 + 96 * player: links +0x108/+0x10c, head +0x10, count +0x12). The
/// scratch globals 0x51b844/0x51b848 are written as the original does. Returns
/// the number of returned units, 0 when the head link is 0xffff, 0x1a1a0010
/// for the wrapped-zero record.
fn return_to_pool(m: &mut MemoriaJogo, player: u32, index: u32) -> Result<u32> {
    let g = group(index);
    if g == 0 {
        return Ok(0x1a1a0010);
    }
    let mut count = 0u32;
    loop {
        if m.u16(g.wrapping_add(0x12))? <= m.u16(g.wrapping_add(0x14))? {
            return Ok(count);
        }
        m.set_u32(0x51b844, g)?;
        let head = m.u16(g.wrapping_add(0x10))?;
        m.set_u16(0x51b848, head)?;
        if head == 0xffff {
            return Ok(0);
        }
        crate::frame_raid::remove_unit(m, index, head)?;
        let pool = m.u32(0x51b664u32.wrapping_add(player.wrapping_mul(96)))?;
        if pool != u32::MAX {
            let r = crate::player::unit(m, u32::from(head))?;
            let p = group(pool);
            if p != 0 {
                m.set_u16(r.wrapping_add(0x108), m.u16(p.wrapping_add(0x10))?)?;
                m.set_u32(r.wrapping_add(0x10c), pool)?;
                m.set_u16(p.wrapping_add(0x10), head)?;
                m.set_u16(
                    p.wrapping_add(0x12),
                    m.u16(p.wrapping_add(0x12))?.wrapping_add(1),
                )?;
            }
        }
        count = count.wrapping_add(1);
    }
}

/// SetEnginRentreEngin 0x445f2d: the unit heads for the target unit's cell
/// (+0x7e), loses the current selection when it is the selected unit
/// (0x496304 -> 0xffff, 0x4e0f36/37/3c cleared), clears +0x88, drops its
/// structure link (+0xc0 = 0xffff), enters state 13 and records the target
/// in +0xb8.
pub(crate) fn board(m: &mut MemoriaJogo, unit: u32, target: u32) -> Result<()> {
    let r = crate::player::unit(m, unit)?;
    let t = crate::player::unit(m, target)?;
    m.set_u16(r.wrapping_add(0x7e), m.u16(t.wrapping_add(8))?)?;
    if unit as u16 == m.u16(0x496304)? {
        m.set_u8(0x4e0f36, 0)?;
        m.set_u8(0x4e0f37, 0)?;
        m.set_u16(0x496304, 0xffff)?;
        m.set_u32(0x4e0f3c, 0)?;
    }
    m.set_u8(r.wrapping_add(0x88), 0)?;
    m.set_u16(r.wrapping_add(0xc0), 0xffff)?;
    m.set_u8(r.wrapping_add(2), 13)?;
    m.set_u16(r.wrapping_add(0xb8), target as u16)?;
    Ok(())
}

/// SetRamasseEngin 0x445947: ResetOrdre, then the unit heads for the cell
/// (+0x7e and +0xb2), loses the hover/selection link when it is the tracked
/// unit (0x496304 -> 0xffff, 0x4e0f36/37/3c cleared), clears +0x88 and +0x89
/// and enters state 0x10.
fn pick_up(m: &mut MemoriaJogo, unit: u32, cell: u16) -> Result<()> {
    crate::frame_spawn::reset_order(m, unit)?;
    let r = crate::player::unit(m, unit)?;
    m.set_u16(r.wrapping_add(0x7e), cell)?;
    if unit as u16 == m.u16(0x496304)? {
        m.set_u8(0x4e0f36, 0)?;
        m.set_u8(0x4e0f37, 0)?;
        m.set_u16(0x496304, 0xffff)?;
        m.set_u32(0x4e0f3c, 0)?;
    }
    m.set_u8(r.wrapping_add(0x88), 0)?;
    m.set_u8(r.wrapping_add(2), 0x10)?;
    m.set_u8(r.wrapping_add(0x89), 0)?;
    m.set_u16(r.wrapping_add(0xb2), cell)
}

/// InventionFutureEstRealisable 0x488fa4(player, code, value): the next item
/// (level + 1) of the player's current research branch, read through
/// RechercheEffetEnCours 0x470e6a (table byte +6, 0 when it is 0xff),
/// RechercheCodeEnCours 0x470ed8 and RechercheTypeEnCours 0x470f1d (bytes +7
/// and +8, but only when byte +6 is 0, else 0) and InventionRealisable
/// 0x470cb0 (byte +6 not 0xff and byte +5 <= MissionGetNum). The effect test
/// needs byte +6 non-zero and the code test needs it zero, so the original
/// never answers 1; the reads are kept as they are.
fn invention_future(m: &MemoriaJogo, player: u32, code: u32, value: u32) -> Result<bool> {
    const RESEARCH: u32 = 0x5080f4;
    let p = player & 255;
    let branch = u32::from(m.u8(RESEARCH + 16 * p + 4)?);
    let level = u32::from(m.u8(RESEARCH + 16 * p + branch)?);
    let item = (level + 1) & 255;
    let at = |field: u32| 0x50817cu32 + 360 * branch + 12 * item + field;
    let kind = m.u8(at(6))?;
    let effect = if kind != 0xff { kind } else { 0 };
    if effect == 0 {
        return Ok(false);
    }
    let code_now = if kind == 0 { m.u8(at(7))? } else { 0 };
    if u32::from(code_now) != code {
        return Ok(false);
    }
    let type_now = if kind == 0 { m.u8(at(8))? } else { 0 };
    if u32::from(type_now) != value {
        return Ok(false);
    }
    if m.u8(at(6))? == 0xff {
        return Ok(false);
    }
    Ok(u32::from(m.u8(at(5))?) <= u32::from(m.u8(0x496fe4)?))
}

/// CaseGetZoneInfluence 0x47bf66: the first zone rectangle (0x50c614 + 10 *
/// zone: x1, y1, x2, y2 words, inclusive) of the 400 that holds the cell,
/// skipping zone 255; 0xff when none.
fn zone_influence(m: &MemoriaJogo, cell: u32) -> Result<u32> {
    let (x, y) = (cell & 0xff, (cell & 0xffff) >> 8);
    for zone in 0..400u32 {
        if zone == 0xff {
            continue;
        }
        let r = 0x50c614 + 10 * zone;
        if x >= u32::from(m.u16(r)?)
            && x <= u32::from(m.u16(r + 4)?)
            && y >= u32::from(m.u16(r + 2)?)
            && y <= u32::from(m.u16(r + 6)?)
        {
            return Ok(zone);
        }
    }
    Ok(0xff)
}

/// GetFreeCaseZoneInfluenceRayon 0x489628 with EDX = EBX = radius: columns
/// x - r ..= x + r outside, rows y - r ..= y + r inside; the first in-map cell
/// inside a zone whose grid word (0x4e0f38) carries no structure tag (bits
/// 15..14 == 01 -> index) is returned, else 0xffff.
fn free_cell_in_zone(m: &MemoriaJogo, cell: u32, radius: u32) -> Result<u32> {
    let (x, y, r) = ((cell & 0xff) as i32, (cell >> 8) as i32, radius as i32);
    let grid = m.u32(0x4e0f38)?;
    for cx in x - r..=x + r {
        for cy in y - r..=y + r {
            if !(0..256).contains(&cx) || !(0..256).contains(&cy) {
                continue;
            }
            let c = ((cy as u32) << 8) | cx as u32;
            if zone_influence(m, c)? == 0xff {
                continue;
            }
            let word = u32::from(m.u16(grid.wrapping_add(2 * c))?);
            let tag = if word & 0xc000 == 0x4000 {
                (word & 0x3ff8) >> 3
            } else {
                0xffff
            };
            if tag == 0xffff {
                return Ok(c);
            }
        }
    }
    Ok(0xffff)
}

fn construction_counter(player: u32, kind: u32) -> u32 {
    0x51b878u32
        .wrapping_add(player.wrapping_mul(124))
        .wrapping_add(kind.wrapping_mul(2))
}

/// GetTypeBatimentFromTypeUnite 0x41cb9a. Unassociated types leave DL
/// untouched in the original; the typed RULE API has no incoming DL.
fn unit_building_type(kind: u8) -> Option<u32> {
    match kind {
        0..=2 | 8 => Some(1),
        3 | 25 => Some(13),
        4 => Some(15),
        5..=7 => Some(2),
        9 => Some(4),
        10 => Some(16),
        11..=15 => Some(5),
        16..=17 => Some(18),
        _ => None,
    }
}

/// iaGroupNew 0x483bd8. Keep the native failure check *after* initialization:
/// a corrupt cursor of -1 can initialize record -1, then skip the list insert.
fn allocate_group(m: &mut MemoriaJogo, player: u32, kind: u32) -> Result<u32> {
    let cursor = m.u32(0x51b660)?;
    // The existing provider covers the initialized allocator domain exactly.
    if cursor <= 100 {
        return crate::frame_raid::new_group(m, player, kind);
    }
    let mut index = cursor;
    let mut offset = cursor.wrapping_shl(5);
    loop {
        if (offset as i32) >= 0xc80 {
            return Ok(u32::MAX);
        }
        let g = 0x51a9e0u32.wrapping_add(offset);
        if m.u8(g.wrapping_add(0x18))? == 0 {
            m.set_u32(g, u32::MAX)?;
            m.set_u32(g.wrapping_add(4), u32::MAX)?;
            m.set_u16(g.wrapping_add(0x12), 0)?;
            m.set_u16(g.wrapping_add(0x14), 0x7fff)?;
            m.set_u32(g.wrapping_add(8), u32::MAX)?;
            m.set_u32(g.wrapping_add(12), u32::MAX)?;
            m.set_u16(g.wrapping_add(0x10), 65535)?;
            m.set_u8(g.wrapping_add(0x18), 1)?;
            m.set_u8(g.wrapping_add(0x19), kind as u8)?;
            m.set_u8(g.wrapping_add(0x1a), 255)?;
            m.set_u16(g.wrapping_add(0x16), 65535)?;
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
            if index != u32::MAX {
                let head = 0x51b664u32
                    .wrapping_add(player.wrapping_mul(96))
                    .wrapping_add(kind.wrapping_mul(4));
                let previous = m.u32(head)?;
                m.set_u32(head, index)?;
                m.set_u32(group(index), previous)?;
            }
            return Ok(index);
        }
        offset = offset.wrapping_add(32);
        index = index.wrapping_add(1);
    }
}

/// iaGroupDelete 0x483ce7. Despite the RULE name, no emptiness test occurs.
pub(crate) fn delete_group(m: &mut MemoriaJogo, player: u32, index: u32) -> Result<()> {
    if (index as i32) < 0 || (index as i32) >= 100 {
        return Ok(());
    }
    let g = group(index);
    m.set_u8(g + 0x18, 0)?;
    if (index as i32) < (m.u32(0x51b660)? as i32) {
        m.set_u32(0x51b660, index)?;
    }
    let kind = u32::from(m.u8(g + 0x19)?);
    let head = 0x51b664u32
        .wrapping_add(player.wrapping_mul(96))
        .wrapping_add(kind.wrapping_mul(4));
    let mut link = head;
    for _ in 0..10001 {
        let current = m.u32(link)?;
        if current == u32::MAX {
            return Ok(());
        }
        if current == index {
            return m.set_u32(link, m.u32(g)?);
        }
        link = group(current);
    }
    Err(stop("iaGroupDelete cyclic group list", 0x483d45))
}

fn available_building(m: &mut MemoriaJogo, kind: u32, player: u32) -> Result<u32> {
    let mut start = 0;
    loop {
        let index = crate::frame_spawn::call(
            m,
            "GetNumIndiceTypeBatiment",
            &[start, kind & 255, player & 255],
        )? & 65535;
        if index == 65535 {
            return Ok(u32::MAX);
        }
        if m.u8(0x49a0c8 + 276 * index + 5)? != 3 {
            return Ok(index);
        }
        start = index + 1;
    }
}
fn building_count(m: &MemoriaJogo, kind: u32, player: u32) -> Result<u32> {
    let mut count = 0;
    for i in 0..1000 {
        let b = 0x49a0c8 + 276 * i;
        if m.u8(b + 5)? != 0
            && u32::from(m.u8(b + 4)?) == player & 255
            && u32::from(m.u8(b + 8)?) == kind & 255
        {
            count += 1;
        }
    }
    Ok(count)
}
/// I_DonneBatimentAAttaquer 0x480aed. Priority categories include the final zero.
fn attack_building(m: &mut MemoriaJogo, a: &[u32]) -> Result<u32> {
    let table = 0x498468u32.wrapping_add(a[1].wrapping_mul(24));
    let mut priorities = Vec::new();
    let mut counts = [0u32; 10];
    let mut special = false;
    let mut eligible = false;
    for i in 0..10000 {
        let category = m.u32(table.wrapping_add(i * 4))?;
        if category >= counts.len() as u32 {
            return Err(stop(
                "I_DonneBatimentAAttaquer category outside local counters",
                0x480b31,
            ));
        }
        priorities.push(category);
        counts[category as usize] = 0;
        let list = 0x4984f8u32.wrapping_add(category.wrapping_mul(24));
        let n = m.u32(list)?;
        if n > 10000 {
            return Err(stop(
                "I_DonneBatimentAAttaquer malformed type table",
                0x480b49,
            ));
        }
        for j in 0..n {
            let kind = m.u32(list.wrapping_add(4 + j * 4))?;
            let number = building_count(m, kind, a[0])?;
            if number == 0 {
                continue;
            }
            if kind == 3 {
                special = true;
            }
            if kind != 3 || number < 3 {
                eligible = true;
            }
            counts[category as usize] += 1;
        }
        if category == 0 {
            break;
        }
        if i == 9999 {
            return Err(stop(
                "I_DonneBatimentAAttaquer unterminated priorities",
                0x480b0a,
            ));
        }
    }
    let special_only = special && !eligible;
    if !eligible && !special_only {
        return Ok(u32::MAX);
    }
    for category in priorities {
        if counts[category as usize] == 0 {
            continue;
        }
        let list = 0x4984f8u32.wrapping_add(category.wrapping_mul(24));
        let n = m.u32(list)?;
        if n == 0 {
            return Err(stop(
                "I_DonneBatimentAAttaquer x86 division fault",
                0x480c11,
            ));
        }
        let mut selected = None;
        for _ in 0..100000 {
            let roll = crate::frame_core::random(m)?;
            let kind = m.u32(list.wrapping_add(4 + (roll % n).wrapping_mul(4)))?;
            let number = building_count(m, kind, a[0])?;
            if number != 0 && (kind != 3 || number < 3 || special_only) {
                selected = Some(kind);
                break;
            }
        }
        let kind =
            selected.ok_or_else(|| stop("I_DonneBatimentAAttaquer RNG retry limit", 0x480c02))?;
        let mut start = 0;
        let mut best = 65535;
        let mut distance = u32::MAX;
        loop {
            let index = crate::frame_spawn::call(
                m,
                "GetNumIndiceTypeBatiment",
                &[start, kind & 255, a[0] & 255],
            )? & 65535;
            if index == 65535 {
                break;
            }
            let cell = crate::frame_spawn::call(m, "GetCaseIndiceBatiment", &[index])? & 65535;
            // 488dfc splits only the low X byte, but keeps 16 Y bits.
            let dx = (a[2] & 255).wrapping_sub(cell & 255);
            let dy = ((a[2] >> 8) & 65535).wrapping_sub(cell >> 8);
            let d = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
            if d <= distance {
                best = index;
                distance = d;
            }
            start = index + 1;
        }
        if best != 65535 {
            return Ok(best);
        }
    }
    Ok(u32::MAX)
}
fn unit_address(m: &MemoriaJogo, index: u16) -> Result<u32> {
    Ok(m.u32(0x4e19a0)?.wrapping_add(u32::from(index) * 276))
}
fn find_member(m: &MemoriaJogo, g: u32, kind: u32) -> Result<u16> {
    if group(g) == 0 {
        return Ok(65535);
    }
    let mut index = m.u16(group(g).wrapping_add(0x10))?;
    for _ in 0..10001 {
        if index == 65535 {
            return Ok(index);
        }
        let r = unit_address(m, index)?;
        if u32::from(m.u8(r.wrapping_add(1))?) == kind {
            return Ok(index);
        }
        index = m.u16(r.wrapping_add(0x108))?;
    }
    Err(stop("iagUnitTransfert cyclic unit list", 0x483490))
}
/// Port deviation (2026-11-23): whether group `index` is allocated and linked
/// in `player`'s list for its kind (head 0x51b664 + 96 * player + 4 * kind,
/// next at +0, as iaGroupNew/iaGroupDelete keep them). The walk is bounded by
/// the 100-entry table.
pub(crate) fn group_is_players(m: &MemoriaJogo, player: u32, index: u32) -> Result<bool> {
    if index >= 100 || player >= 5 {
        return Ok(false);
    }
    let g = group(index);
    if m.u8(g.wrapping_add(0x18))? == 0 {
        return Ok(false);
    }
    let kind = u32::from(m.u8(g.wrapping_add(0x19))?);
    if kind >= 24 {
        return Ok(false);
    }
    let mut current = m.u32(0x51b664 + 96 * player + 4 * kind)?;
    for _ in 0..100 {
        if current == index {
            return Ok(true);
        }
        if current >= 100 {
            return Ok(false);
        }
        current = m.u32(group(current))?;
    }
    Ok(false)
}

/// iagUnitTransfert 0x483358. Pool first, then destination-kind priorities.
///
/// Port deviation (2026-11-23): a destination that is not one of `player`'s
/// groups transfers nothing (the "no unit" result). A task keeps a group
/// number after iaGroupDelete frees it, and iaGroupNew hands the number to
/// any player; a private research bot, MONDE16 seed 3 (human), frame
/// 190,399: player 1's B_CompleteGroupe 0x481c75 filled group 18, then player
/// 3's kind-8 group, with player 1's unit 28, so player 3's AI ordered player
/// 1's unit.
pub(crate) fn transfer_unit(
    m: &mut MemoriaJogo,
    player: u32,
    destination: u32,
    kind: u32,
) -> Result<u32> {
    let dest = group(destination);
    if dest == 0 {
        return Ok(u32::MAX);
    }
    if !group_is_players(m, player, destination)? {
        return Ok(u32::MAX);
    }
    let head = 0x51b664u32.wrapping_add(player.wrapping_mul(96));
    let pool = m.u32(head)?;
    let mut found = None;
    if pool != u32::MAX {
        let member = find_member(m, pool, kind)?;
        if member != 65535 {
            found = Some((pool, member));
        }
    }
    if found.is_none() {
        let table = 0x4988c0 + 16 * u32::from(m.u8(dest.wrapping_add(0x19))?);
        let pointer = m.u32(table + 4)?;
        if pointer != 0 {
            let n = m.u32(table)? as i32;
            if n > 10000 {
                return Err(stop("iagUnitTransfert malformed priority table", 0x483411));
            }
            for i in 0..n.max(0) as u32 {
                let row = pointer.wrapping_add(4 * i);
                if u32::from(m.u8(row.wrapping_add(1))?) != kind {
                    continue;
                }
                let group_kind = u32::from(m.u8(row)?);
                let threshold = i32::from(m.u8(row.wrapping_add(2))? as i8);
                let mut source = m.u32(head.wrapping_add(4 * group_kind))?;
                for step in 0..10001 {
                    if source == u32::MAX || group(source) == 0 {
                        break;
                    }
                    let s = group(source);
                    if i32::from(m.u16(s.wrapping_add(0x12))?) > threshold {
                        let member = find_member(m, source, kind)?;
                        if member != 65535 {
                            found = Some((source, member));
                            break;
                        }
                    }
                    source = m.u32(s)?;
                    if step == 10000 {
                        return Err(stop("iagUnitTransfert cyclic group list", 0x48345c));
                    }
                }
                if found.is_some() {
                    break;
                }
            }
        }
    }
    let Some((source, member)) = found else {
        return Ok(u32::MAX);
    };
    // Native remove keeps the removed unit's own link until add overwrites it.
    let s = group(source);
    let mut link = s.wrapping_add(0x10);
    for step in 0..10001 {
        let index = m.u16(link)?;
        if index == 65535 {
            return Ok(u32::MAX);
        }
        let r = unit_address(m, index)?;
        if index == member {
            m.set_u16(link, m.u16(r.wrapping_add(0x108))?)?;
            m.set_u16(
                s.wrapping_add(0x12),
                m.u16(s.wrapping_add(0x12))?.wrapping_sub(1),
            )?;
            crate::frame_raid::add_unit(m, destination, member)?;
            return Ok(1);
        }
        link = r.wrapping_add(0x108);
        if step == 10000 {
            return Err(stop("iagUnitTransfert cyclic remove list", 0x4832d4));
        }
    }
    unreachable!()
}

/// I_DonnePositionMaVille 0x480859. The building scan intentionally tests
/// owner/type only, including inactive records. Ghost fallback is global.
fn city_position(m: &mut MemoriaJogo, player: u32) -> Result<u32> {
    let building = crate::frame_spawn::call(m, "GetNumIndiceTypeBatiment", &[0, 2, player & 255])?;
    if building != 65535 {
        // Native calls GetCaseIndiceBatiment, not the collision-mask centre.
        return Ok(crate::frame_spawn::call(m, "GetCaseIndiceBatiment", &[building])? & 65535);
    }
    for i in 0..u32::from(m.u16(0x50b18c)?) {
        let cell = m.u16(0x50b18e + 2 * i)?;
        // GetNumIndiceTypeGhost(0, 2) selects a *clear* bit 2.
        if cell != 65535 && m.u32(0x50b76c + 4 * i)? & 4 == 0 {
            return Ok(u32::from(cell));
        }
    }
    Ok(u32::MAX)
}

/// I_ChercheWayPoint 0x4802fb: scan the rectangle between the target column
/// and the start column (from the target side) row by row from the target row
/// towards the start row, both ends exclusive, for the first cell that is
/// either discovered but dark and reachable from the start (Chemin_Accessible)
/// or undiscovered without a structure; otherwise the midpoint cell. Signed
/// 256-divisions and the original loop shape (an equal column or row scans
/// nothing).
fn waypoint(m: &mut MemoriaJogo, start: u32, target: u32, player: u32) -> Result<u32> {
    let (sx, sy) = ((start as i32) % 256, (start as i32) / 256);
    let (tx, ty) = ((target as i32) % 256, (target as i32) / 256);
    let mut mid = 0i32;
    let step_x = if sx < tx {
        mid = sx + (tx - sx) / 2;
        -1
    } else if sx > tx {
        mid = tx + (sx - tx) / 2;
        1
    } else {
        0
    };
    let step_y = if ty > sy {
        mid += (sy + (ty - sy) / 2) << 8;
        -1
    } else if ty < sy {
        mid += (ty + (sy - ty) / 2) << 8;
        1
    } else {
        0
    };
    let mut y = ty;
    loop {
        let mut x = tx;
        while x != sx {
            if (0..256).contains(&x) && (0..256).contains(&y) {
                let cell = (x + (y << 8)) as u32 & 65535;
                if crate::frame_core::call(m, "CaseEstDecouverte", &[cell, player])? as u8 == 1 {
                    if crate::frame_core::call(m, "CaseEstIlluminee", &[cell, player])? as u8 == 0
                        && crate::frame_movement::call(
                            m,
                            "Chemin_Accessible",
                            &[start & 65535, cell],
                        )? as u8
                            == 1
                    {
                        return Ok(cell);
                    }
                } else if crate::player_input::selection_leaf(
                    m,
                    "ChercheIndiceBatiment",
                    cell,
                    0,
                    0,
                )? & 65535
                    == 0xffff
                {
                    return Ok(cell);
                }
            }
            x += step_x;
        }
        y += step_y;
        if y == sy {
            return Ok(mid as u32);
        }
    }
}

pub fn operation(m: &mut MemoriaJogo, address: u32, a: &[u32], player: u32) -> Result<u32> {
    let required = match address {
        0x48068f | 0x481372 | 0x481399 | 0x481400 | 0x481c4a | 0x482010 | 0x485d7b | 0x481c75
        | 0x485e98 | 0x4806ed | 0x4808b4 | 0x480731 | 0x4897a5 | 0x47ff65 | 0x47fc58 | 0x481ece
        | 0x481e07 | 0x481146 | 0x4804a1 | 0x482433 | 0x4801d2 | 0x47ffa4 | 0x480d97 | 0x481049 => {
            2
        }
        0x480aed | 0x482020 | 0x48213c | 0x4820e7 | 0x47febf | 0x4802fb | 0x482085 | 0x47fead
        | 0x4821f0 | 0x480cee => 3,
        0x47ffd8 | 0x480644 | 0x481c28 | 0x48264a | 0x482667 | 0x4806db | 0x47fffb | 0x480803
        | 0x482579 | 0x48181a | 0x485ec2 | 0x48219e | 0x481133 | 0x485dfc | 0x481c9b | 0x481bcf
        | 0x4801a2 | 0x48094a | 0x481be8 | 0x47ff34 | 0x481c03 | 0x481e88 | 0x482501 | 0x481b9e
        | 0x481bb1 | 0x481b46 | 0x481034 | 0x480e4f | 0x480e72 | 0x480e1f | 0x481075 | 0x47fe4b
        | 0x48001e | 0x482265 | 0x46030a => 1,
        0x4822ab => 2,
        0x481fb4 | 0x481c38 | 0x480d56 | 0x481738 | 0x48179e | 0x4817f2 | 0x481806 => 1,
        0x480859 | 0x485ef3 | 0x481426 | 0x481833 | 0x485e53 | 0x485e66 | 0x485e7c | 0x485f20
        | 0x480951 | 0x4819d7 | 0x480f16 | 0x480ea6 | 0x480ee2 | 0x4810aa => 0,
        _ => return Err(stop("unknown AI RULE leaf", address)),
    };
    if a.len() < required {
        return Err(stop("iaSolver incomplete rule arguments", address));
    }
    match address {
        0x46030a => {
            // B_EtatMarche: 1 when either byte of the market matrix
            // 0x4e2918 (4 x 4 bytes, row * 4 + column) for the pair
            // (nation [+0], player [+0x1c]) is non-zero, else -1. market.rs
            // has no name for this table, so the address is used directly.
            let pair = m.u8(0x4e2918u32
                .wrapping_add(a[0])
                .wrapping_add(player.wrapping_mul(4)))?
                | m.u8(0x4e2918u32
                    .wrapping_add(player)
                    .wrapping_add(a[0].wrapping_mul(4)))?;
            Ok(if pair != 0 { 1 } else { u32::MAX })
        }
        0x48001e => {
            // B_AiJeConnaissanceFutureBatiment: InventionFutureEstRealisable
            // (player [+0x1c], code 3, value [+0]) == 1, else -1.
            Ok(if invention_future(m, player, 3, a[0])? {
                1
            } else {
                u32::MAX
            })
        }
        0x482265 => {
            // V_DeposerPiece: every live member (EnginEstVivant) of the group
            // [+0] drops its load (SetEnginDepose mode 2, resource 0), then
            // iagMessDispatchWith(group, 7). Always 1.
            let mut member = crate::frame_raid::first_unit(m, a[0])?;
            while member != 0xffff {
                if crate::frame_spawn::call(m, "EnginEstVivant", &[u32::from(member)])? as u8 != 0 {
                    crate::chain_unload::depose(m, member, 2, 0)?;
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            let g = group(a[0]);
            if g != 0 {
                m.set_u16(g.wrapping_add(0x16), 7)?;
            }
            Ok(1)
        }
        0x4822ab => {
            // V_PrendrePiece: every live member of the group [+4] picks up the
            // piece at cell WORD [+0] (SetRamasseEngin), then
            // iagMessDispatchWith(group, 6). Always 1.
            let mut member = crate::frame_raid::first_unit(m, a[1])?;
            while member != 0xffff {
                if crate::frame_spawn::call(m, "EnginEstVivant", &[u32::from(member)])? as u8 != 0 {
                    pick_up(m, u32::from(member), a[0] as u16)?;
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            let g = group(a[1]);
            if g != 0 {
                m.set_u16(g.wrapping_add(0x16), 6)?;
            }
            Ok(1)
        }
        0x482010 => {
            // V_SetMaxUnites -> iagUnitSetMaxUnitsForGroup483881. Native checks
            // the wrapped record address, not the group index or active flag.
            let base = group(a[0]);
            if base != 0 {
                m.set_u16(base.wrapping_add(0x14), a[1] as u16)?;
            }
            Ok(1)
        }
        0x481c75 => {
            // B_CompleteGroupe -> iagUnitTransfertUpToMax48354e. The native
            // loop fixes the initial WORD capacity gap, including self-transfer.
            let base = group(a[0]);
            if base == 0 {
                return Ok(u32::MAX);
            }
            let count = m.u16(base.wrapping_add(0x12))?;
            let maximum = m.u16(base.wrapping_add(0x14))?;
            let mut moved = false;
            for _ in count..maximum {
                if transfer_unit(m, player, a[0], a[1])? as u16 == u16::MAX {
                    break;
                }
                moved = true;
            }
            Ok(if moved { 1 } else { u32::MAX })
        }
        0x480731 => {
            // V_BatimentRecruteUnite: select, attempt recruitment, then deselect.
            // A rejected attempt still returns1; a provider stop propagates.
            crate::player_orders::call(m, "BatimentSelection", [a[0] & 65535, 0, 0, 0], 0x48073c)?;
            let owner = player & 255;
            crate::chain_recruit::init(m, a[1] & 255, owner)?;
            // Native41b663 writes only this WORD; no player-input UI tail.
            m.set_u16(0x495be0 + 2 * owner, 65535)?;
            Ok(1)
        }
        0x4808b4 => crate::ai_field_search::nearest_building(m, a[0], a[1], player),
        0x4806ed => {
            // BatimentGetColons41a4c4 counts tag1/unit-type9 over bag capacity.
            let count = crate::chain_bags::body(
                m,
                "BagContient",
                R {
                    eax: 0x49a0d4 + 276 * (a[0] & 65535),
                    edx: 1,
                    ebx: 9,
                    ..Default::default()
                },
            )?
            .eax & 65535;
            Ok(if (count as i32) >= a[1] as i32 {
                1
            } else {
                u32::MAX
            })
        }
        0x47fffb => {
            // TypeEnginEstRealisable44228e reads a byte; only exactly1 is true.
            let known = m.u8(0x4e1934 + 26 * (player & 255) + (a[0] & 255))?;
            Ok(if known == 1 { 1 } else { u32::MAX })
        }
        0x480803 => {
            Ok(crate::frame_spawn::call(m, "GetCaseIndiceBatiment", &[a[0] & 65535])? & 65535)
        }
        0x485d7b => crate::ai_field_search::set_exploited(m, a[0], a[1], player),
        0x485dfc => crate::ai_field_search::clear_exploited(m, a[0], player),
        0x485f20 => {
            // B_TestMaxRocheDansStock: GetRessourceStock's second word (the
            // player's 0x4e2eba - 0x4e2eb8) above caves (0x4dfb2c) * capacity
            // (0x4dfb2e) * [0x51bcf8] / ([0x51bcf4] + [0x51bcf8]) -> 1, else -1.
            let p = player & 0xff;
            let stock = i32::from(
                m.u16(0x4e2eba + 32 * p)?
                    .wrapping_sub(m.u16(0x4e2eb8 + 32 * p)?),
            );
            let caves = i32::from(m.u16(0x4dfb2cu32.wrapping_add(player.wrapping_mul(6)))?);
            let capacity = i32::from(m.u16(0x4dfb2eu32.wrapping_add(player.wrapping_mul(6)))?);
            let weight = m.u32(0x51bcf8)? as i32;
            let divisor = (m.u32(0x51bcf4)? as i32).wrapping_add(weight);
            let Some(threshold) = caves
                .wrapping_mul(capacity)
                .wrapping_mul(weight)
                .checked_div(divisor)
            else {
                return Err(stop("B_TestMaxRocheDansStock division", 0x485f82));
            };
            Ok(if stock > threshold { 1 } else { u32::MAX })
        }
        0x481146 => {
            // V_DeplacerUnite: a live unit [+4] receives SetDeplaceEngin(cell [+0]).
            let unit = a[1] & 0xffff;
            if crate::frame_spawn::call(m, "EnginEstVivant", &[unit])? as u8 != 0 {
                crate::frame_spawn::call(m, "SetDeplaceEngin", &[unit, a[0] & 0xffff])?;
            }
            Ok(1)
        }
        0x48094a => {
            // I_DonneBatimentOuEstUnite = GetIndiceBatimentFromIndEngin: the first
            // of the 1000 structures whose bag (+0xc) lists the unit with slot
            // kind byte (+0x42 + i) == 1; 0xffff when none.
            let unit = a[0] & 0xffff;
            for s in 0..1000u32 {
                let bag = 0x49a0c8 + 276 * s + 0xc;
                for i in 0..u32::from(m.u16(bag)?) {
                    if m.u8(bag + 0x42 + i)? == 1 && u32::from(m.u16(bag + 6 + 2 * i)?) == unit {
                        return Ok(s);
                    }
                }
            }
            Ok(0xffff)
        }
        0x481be8 => {
            // V_InterditCaseMine: the mine index of the cell is flagged in 0x51a9cc.
            let index = crate::terrain_mine::mine_index(m, a[0] & 0xffff)? & 0xffff;
            m.set_u8(0x51a9cc + index, 1)?;
            Ok(1)
        }
        0x480951 => crate::ai_building_room::not_full(m, player),
        0x481b46 => {
            // I_DonneCaveProchePiece: the player's cave (structure type 3)
            // whose GetCaseIndiceBatiment cell is nearest (Distance2dFromCases,
            // squared, ties go to the later cave) to the cell [+0]; -1 without
            // a cave.
            let target = a[0] & 0xffff;
            let mut best = u32::MAX;
            let mut chosen = 0u32;
            let mut start = 0u32;
            loop {
                let cave = crate::frame_spawn::call(
                    m,
                    "GetNumIndiceTypeBatiment",
                    &[start, 3, player & 0xff],
                )? & 0xffff;
                if cave == 0xffff {
                    break;
                }
                let c = crate::frame_spawn::call(m, "GetCaseIndiceBatiment", &[cave])? & 0xffff;
                let d = crate::frame_movement::call(
                    m,
                    "Distance2d",
                    &[target & 0xff, target >> 8, c & 0xff, c >> 8],
                )?;
                if d <= best {
                    best = d;
                    chosen = cave;
                }
                start = cave + 1;
            }
            Ok(if best == u32::MAX { u32::MAX } else { chosen })
        }
        0x47ffa4 => {
            // V_AttacheConflitGroupe: group [+4] (wrapped-zero record -> -1)
            // joins the player's battle around position [+0]
            // (iagWarSearchBattleFromPos 0x487594) at +0xc; no battle -> -1.
            let g = group(a[1]);
            if g == 0 {
                return Ok(u32::MAX);
            }
            let battle = crate::ai_war_position::search(m, player, a[0])?;
            if battle == u32::MAX {
                return Ok(u32::MAX);
            }
            m.set_u32(g + 0xc, battle)?;
            Ok(1)
        }
        0x481b9e => {
            // I_DonneValeurPiece: SacGetCapacite(cell) low word.
            Ok(crate::chain_queries::sack(m, a[0] & 0xffff)? & 0xffff)
        }
        0x481bb1 => {
            // B_EncorePiece: 1 while the cell still holds a sack quantity, else -1.
            Ok(
                if crate::chain_queries::sack(m, a[0] & 0xffff)? & 0xffff != 0 {
                    1
                } else {
                    u32::MAX
                },
            )
        }
        0x482501 => {
            // V_TricheToutLeMondeMange: every live member of the group is fed
            // (+4 = +0x76 maximum, +0x74 = 0). Returns 1.
            let mut member = crate::frame_raid::first_unit(m, a[0])?;
            while member != 0xffff {
                if crate::frame_spawn::call(m, "EnginEstVivant", &[u32::from(member)])? as u8 != 0 {
                    let r = crate::player::unit(m, u32::from(member))?;
                    m.set_u16(r + 4, m.u16(r + 0x76)?)?;
                    m.set_u16(r + 0x74, 0)?;
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            Ok(1)
        }
        0x4801d2 => {
            // I_DonneBatimentDansZone: rings of radius 0..=[+4] around the cell
            // [+0] (signed idiv by 256; pass 0: x runs -r..=r with y = -r then
            // +r; pass 1: the axes swapped); the first cell inside the map
            // whose grid word holds a structure (ChercheIndiceBatiment) not
            // owned by the player (dword compare with [+0x1c]) gives that
            // structure, else -1.
            let cell = a[0] as i32;
            let (x, y) = (cell % 256, cell / 256);
            let radius = a[1] as i32;
            let grid = m.u32(0x4e0f38)?;
            let mut r = 0i32;
            while r <= radius {
                for pass in 0..2 {
                    for i in -r..=r {
                        for j in [-r, r] {
                            let (dx, dy) = if pass == 0 { (i, j) } else { (j, i) };
                            let (cx, cy) = (dx.wrapping_add(x), dy.wrapping_add(y));
                            if !(0..256).contains(&cx) || !(0..256).contains(&cy) {
                                continue;
                            }
                            let c = ((cy << 8) + cx) as u32 & 0xffff;
                            let raw = m.u16(grid.wrapping_add(2 * c))?;
                            if raw & 0x8000 == 0 {
                                continue;
                            }
                            let building = u32::from((raw & 0x7fe0) >> 5);
                            if u32::from(m.u8(crate::player::structure(building) + 4)?) != player {
                                return Ok(building);
                            }
                        }
                    }
                }
                r += 1;
            }
            Ok(u32::MAX)
        }
        0x481e88 => {
            // V_VideEngin: every live member of the group unloads its troops
            // (SetEnginDepose mode 1). Returns 1.
            let mut member = crate::frame_raid::first_unit(m, a[0])?;
            while member != 0xffff {
                if crate::frame_spawn::call(m, "EnginEstVivant", &[u32::from(member)])? as u8 != 0 {
                    crate::chain_unload::unload_all(m, member)?;
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            Ok(1)
        }
        0x4819d7 => {
            // I_CherchePiece: for each cave (structure type 3) of the player in
            // index order, square rings of radius 1..[0x51bce4] around the
            // cave's GetCaseIndiceBatiment cell (pass 0: x runs -r..=r with
            // y = -r then +r; pass 1: the axes swapped); the first cell inside
            // the map holding a sack of type 0 (object 0x42) reachable from the
            // cave cell (Chemin_Accessible) gives the cell, else -1.
            let limit = m.u32(0x51bce4)? as i32;
            let mut start = 0u32;
            loop {
                let cave = crate::frame_spawn::call(
                    m,
                    "GetNumIndiceTypeBatiment",
                    &[start, 3, player & 0xff],
                )? & 0xffff;
                if cave == 0xffff {
                    return Ok(u32::MAX);
                }
                let origin =
                    crate::frame_spawn::call(m, "GetCaseIndiceBatiment", &[cave])? & 0xffff;
                let (x, y) = ((origin & 0xff) as i32, (origin >> 8) as i32);
                let objects = m.u32(0x507d74)?;
                let mut r = 1i32;
                while r < limit {
                    for pass in 0..2 {
                        for i in -r..=r {
                            for j in [-r, r] {
                                let (dx, dy) = if pass == 0 { (i, j) } else { (j, i) };
                                let (cx, cy) = (dx + x, dy + y);
                                if !(0..256).contains(&cx) || !(0..256).contains(&cy) {
                                    continue;
                                }
                                let c = (cy * 256 + cx) as u32;
                                if m.u8(objects.wrapping_add(c))? == 0x42
                                    && crate::frame_spawn::accessible(m, origin as u16, c as u16)?
                                {
                                    return Ok(c);
                                }
                            }
                        }
                    }
                    r += 1;
                }
                start = cave + 1;
            }
        }
        0x482433 => {
            // B_TransfertUniteFastest: in group [+4], the unit whose kind has
            // the bit [dest group +0x19] in 0x4989f8 and the strictly highest
            // speed byte +0x98 (first wins, liveness not tested) moves to group
            // [+0]: iagUnitRemoveFromGroup then iagUnitAddToGroup; 1, or -1 for
            // a wrapped-zero group record or no candidate.
            let (to, from) = (group(a[0]), group(a[1]));
            if from == 0 || to == 0 {
                return Ok(u32::MAX);
            }
            let bit = 1u32.wrapping_shl(u32::from(m.u8(to + 0x19)?));
            let mut best = 0xffffu16;
            let mut speed = 0u32;
            let mut member = crate::frame_raid::first_unit(m, a[1])?;
            while member != 0xffff {
                let r = crate::player::unit(m, u32::from(member))?;
                if m.u32(0x4989f8 + 4 * u32::from(m.u8(r + 1)?))? & bit != 0 {
                    let s = u32::from(m.u8(r + 0x98)?);
                    if s as i32 > speed as i32 {
                        best = member;
                        speed = s;
                    }
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            if best == 0xffff {
                return Ok(u32::MAX);
            }
            crate::frame_raid::remove_unit(m, a[1], best)?;
            crate::frame_raid::call(m, "iagUnitAddToGroup", &[a[0], u32::from(best)])?;
            Ok(1)
        }
        0x4804a1 => {
            // I_ChercheCivilProche: rings of radius 0..=255 around the cell
            // [+4] (signed idiv by 256); per ring two passes (pass 0: dx runs
            // -r..=r with dy = -r then +r; pass 1: the same with the axes
            // swapped); the first cell inside the map whose grid unit belongs
            // to player [+0] (dword compare) and whose kind is not an army
            // kind (TypeEnginEstArmee 0x44230c) gives the cell, else -1.
            let cell = a[1] as i32;
            let (x, y) = (cell % 256, cell / 256);
            let grid = m.u32(0x4e0f38)?;
            let pool = m.u32(0x4e19a0)?;
            for r in 0..256i32 {
                for pass in 0..2 {
                    for i in -r..=r {
                        for j in [-r, r] {
                            let (dx, dy) = if pass == 0 { (i, j) } else { (j, i) };
                            let (cx, cy) = (dx.wrapping_add(x), dy.wrapping_add(y));
                            if !(0..256).contains(&cx) || !(0..256).contains(&cy) {
                                continue;
                            }
                            let c = (cy << 8).wrapping_add(cx) as u32;
                            let raw = m.u16(grid.wrapping_add(2 * (c & 0xffff)))?;
                            if raw & 0xc000 != 0x4000 {
                                continue;
                            }
                            let u = u32::from((raw & 0x3ff8) >> 3);
                            let record = pool.wrapping_add(276 * u);
                            if u32::from(m.u8(record)?) != a[0] {
                                continue;
                            }
                            let army = matches!(m.u8(record + 1)?, 0..=3 | 8 | 11 | 12 | 15 | 16 | 18..=25);
                            if !army {
                                return Ok(c);
                            }
                        }
                    }
                }
            }
            Ok(u32::MAX)
        }
        0x47fead => {
            // I_DonneForceGroupeDansZone -> iagWarComputeGroupStrengthInZone
            // 0x48794a(group [+8], cell [+0], radius [+4]): the force table
            // 0x498cf4 summed over the live units standing (EnginGetCase) on
            // the clamped square of the grid whose group dword +0x10c is the
            // group argument. Signed bounds as the native idiv/sar.
            let (cell, radius, group) = (a[0] as i32, a[1] as i32, a[2]);
            let (x, y) = (cell % 256, cell / 256);
            let x0 = (x.wrapping_sub(radius)).max(0);
            let y0 = (y.wrapping_sub(radius)).max(0);
            let x1 = if x.wrapping_add(radius) >= 256 {
                255
            } else {
                x.wrapping_add(radius)
            };
            let y1 = if y.wrapping_add(radius) >= 256 {
                255
            } else {
                y.wrapping_add(radius)
            };
            let grid = m.u32(0x4e0f38)?;
            let mut force = 0u32;
            let mut row = y0.wrapping_shl(8);
            while row <= y1.wrapping_shl(8) {
                let mut column = x0;
                while column <= x1 {
                    let c = row.wrapping_add(column) as u32;
                    let raw = m.u16(grid.wrapping_add(2 * c))?;
                    if raw & 0xc000 == 0x4000 {
                        let u = u32::from((raw & 0x3ff8) >> 3);
                        if crate::frame_spawn::call(m, "EnginEstVivant", &[u])? as u8 != 0 {
                            let r = crate::player::unit(m, u)?;
                            if u32::from(m.u16(r + 0x7c)?) == c && m.u32(r + 0x10c)? == group {
                                force = force
                                    .wrapping_add(m.u32(0x498cf4 + 4 * u32::from(m.u8(r + 1)?))?);
                            }
                        }
                    }
                    column += 1;
                }
                row = row.wrapping_add(0x100);
            }
            Ok(force)
        }
        0x481c03 => {
            // V_RestoreCaseMine: clears the flag V_InterditCaseMine set.
            let index = crate::terrain_mine::mine_index(m, a[0] & 0xffff)? & 0xffff;
            m.set_u8(0x51a9cc + index, 0)?;
            Ok(1)
        }
        0x481bcf => {
            // I_DonneValeurMine: MineGetCapacite(MineGetIndice(cell)), low word.
            let index = crate::terrain_mine::mine_index(m, a[0] & 0xffff)? & 0xffff;
            Ok(crate::terrain_mine::mine_capacity(m, index)? & 0xffff)
        }
        0x4801a2 => {
            // I_DonneCaseLibreZoneProche: GetFreeCaseZoneInfluenceRayon with the
            // square radius 1..=255 around the cell; -1 when every radius fails.
            for radius in 1..256u32 {
                let cell = free_cell_in_zone(m, a[0], radius)?;
                if cell != 0xffff {
                    return Ok(cell);
                }
            }
            Ok(u32::MAX)
        }
        0x481e07 => {
            // V_EntreBatiment: every member of the group (dead ones too) receives
            // SetEnginRentreBatiment(structure), then iagMessDispatchWith(group, 2).
            let mut member = crate::frame_raid::first_unit(m, a[1])?;
            while member != 0xffff {
                crate::chain_entry::set_enter(m, member, a[0] as u16)?;
                member = crate::frame_raid::next_unit(m)?;
            }
            let g = group(a[1]);
            if g != 0 {
                m.set_u16(g.wrapping_add(0x16), 2)?;
            }
            Ok(1)
        }
        0x482085 => {
            // V_CueillirChamp: every live member of the group [+8] outside state 5
            // gets SetRecolteEngin(field [+0]) and SetDepotEngin(DL 1, BX [+4], CL 1).
            let mut member = crate::frame_raid::first_unit(m, a[2])?;
            while member != 0xffff {
                let r = crate::player::unit(m, u32::from(member))?;
                let state = m.u8(r.wrapping_add(2))?;
                if !matches!(state, 0 | 4 | 5) {
                    crate::player_orders::call(
                        m,
                        "SetRecolteEngin",
                        [u32::from(member), a[0] & 0xffff, 0, 1],
                        0x4820ce,
                    )?;
                    m.set_u8(r.wrapping_add(0x82), 1)?;
                    m.set_u16(r.wrapping_add(0x80), a[1] as u16)?;
                    m.set_u16(r.wrapping_add(0x84), 1)?;
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            Ok(1)
        }
        0x480f16 => {
            // I_CombienDAlliances: players 0..=3 other than the caller's player
            // DWORD with PlayerEstAllie(player, other) == 1, counted.
            let mut count = 0u32;
            for other in 0..4u32 {
                if other == player {
                    continue;
                }
                if crate::frame_economy::call(m, "PlayerEstAllie", &[player & 255, other])? & 255
                    == 1
                {
                    count += 1;
                }
            }
            Ok(count)
        }
        0x480e4f => {
            // B_Allie: 1 when PlayerEstAllie(player, other [+0]) & 0xff == 1, else -1.
            let allied =
                crate::frame_economy::call(m, "PlayerEstAllie", &[player & 255, a[0] & 255])? & 255;
            Ok(if allied == 1 { 1 } else { u32::MAX })
        }
        0x480e72 => {
            // V_DeclareGuerre: PlayerSetSympathie 0x46dc90 writes -50 into the
            // word players + 20 * a + 0xc + 2 * b for (player, other) and
            // (other, player); no bounds check in the original. Returns 1.
            let players = m.u32(0x5080a8)?;
            let other = a[0] & 255;
            for (x, y) in [(player & 255, other), (other, player & 255)] {
                m.set_u16(
                    players
                        .wrapping_add(20 * x)
                        .wrapping_add(0xc)
                        .wrapping_add(2 * y),
                    0xffce,
                )?;
            }
            Ok(1)
        }
        0x480e1f => {
            // V_RefuseAlliance: when the DWORD [+0] is the local player byte,
            // MessageBoxSet(10, player word [+0x1c] + 0x195). Returns 1.
            if a[0] == u32::from(m.u8(0x508041)?) {
                crate::messagebox_state::enqueue(m, 10, (player as u16).wrapping_add(0x195))?;
            }
            Ok(1)
        }
        0x480d97 => {
            // V_FixeAlliance: mode [+4] == 0 breaks an existing alliance
            // (PlayerEstAllie non-zero: PlayerResetAllie both ways), any other
            // mode makes one (PlayerSetAllie 0x46d877 both ways: a != b, both
            // <= 4, bit 1 << other in the alliance byte +6, MessageBoxSet(10,
            // other + 0x18f) for the local player; then MAJSympathie both ways
            // with event 0x10). Returns 1.
            let other = a[0] & 255;
            let p = player & 255;
            if a[1] == 0 {
                if crate::frame_economy::call(m, "PlayerEstAllie", &[p, other])? & 255 != 0 {
                    crate::messagebox_state::reset_allie(m, p as u8, other as u8)?;
                    crate::messagebox_state::reset_allie(m, other as u8, p as u8)?;
                }
                return Ok(1);
            }
            for (x, y) in [(p, other), (other, p)] {
                if x != y && x <= 4 && y <= 4 {
                    let at = m.u32(0x5080a8)?.wrapping_add(20 * x + 6);
                    m.set_u8(at, m.u8(at)? | (1 << y) as u8)?;
                    if x == u32::from(m.u8(0x508041)?) {
                        crate::messagebox_state::enqueue(m, 10, y as u16 + 0x18f)?;
                    }
                }
            }
            crate::frame_economy::call(m, "MAJSympathie", &[p, other, 0x10])?;
            crate::frame_economy::call(m, "MAJSympathie", &[other, p, 0x10])?;
            Ok(1)
        }
        0x480cee => {
            // I_DonneBatimentMarchandage: the structure of owner [+0] (BL) and
            // type [+4] (DL) whose GetCaseIndiceBatiment cell is nearest
            // (Distance2dFromCases: 8 x bits and 16 y bits of the target DWORD
            // [+8], squared, ties go to the later structure); -1 without one.
            let target = a[2];
            let mut best = u32::MAX;
            let mut chosen = 0u32;
            let mut start = 0u32;
            loop {
                let index = crate::frame_spawn::call(
                    m,
                    "GetNumIndiceTypeBatiment",
                    &[start, a[1] & 0xff, a[0] & 0xff],
                )? & 0xffff;
                if index == 0xffff {
                    break;
                }
                let c = crate::frame_spawn::call(m, "GetCaseIndiceBatiment", &[index])? & 0xffff;
                let d = crate::frame_movement::call(
                    m,
                    "Distance2d",
                    &[target & 0xff, (target >> 8) & 0xffff, c & 0xff, c >> 8],
                )?;
                if d <= best {
                    best = d;
                    chosen = index;
                }
                start = index + 1;
            }
            Ok(if best == u32::MAX { u32::MAX } else { chosen })
        }
        0x480ea6 | 0x480ee2 => {
            // I_DonneNationPlusHaute / I_DonneNationPlusBasse: the player 0..=3
            // other than the caller's player DWORD with the highest (ties:
            // the later player, "jl" skips) / lowest (ties: the earlier
            // player, "jge" skips) sign-extended PlayerGetSympathie word,
            // starting from -32768 / 32768.
            let players = m.u32(0x5080a8)?;
            let highest = address == 0x480ea6;
            let mut best: i32 = if highest { -32768 } else { 32768 };
            let mut found = 0u32;
            for other in 0..4u32 {
                if other == player {
                    continue;
                }
                let value = m.u16(
                    players
                        .wrapping_add(20 * (player & 255))
                        .wrapping_add(0xc)
                        .wrapping_add(2 * other),
                )? as i16 as i32;
                if (highest && value >= best) || (!highest && value < best) {
                    found = other;
                    best = value;
                }
            }
            Ok(found)
        }
        0x481034 => {
            // I_RecupereSympathie: PlayerGetSympathie 0x46dc6c, the word at
            // players + 20 * player + 0xc + 2 * other, sign-extended (cwde).
            let players = m.u32(0x5080a8)?;
            let other = a[0] & 255;
            let value = m.u16(
                players
                    .wrapping_add(20 * (player & 255))
                    .wrapping_add(0xc)
                    .wrapping_add(2 * other),
            )?;
            Ok(value as i16 as i32 as u32)
        }
        0x481049 => {
            // V_ModifieSympathie: PlayerGetSympathie(player byte, other byte [+0])
            // plus the word [+4], truncated to 16 bits (add + cwde), written
            // back by PlayerSetSympathie (tail jump into V_DeclareGuerre's
            // second call, 0x480e98) for (player, other) only. Returns 1.
            let players = m.u32(0x5080a8)?;
            let other = a[0] & 255;
            let cell = players
                .wrapping_add(20 * (player & 255))
                .wrapping_add(0xc)
                .wrapping_add(2 * other);
            let value = m.u16(cell)?.wrapping_add(a[1] as u16);
            m.set_u16(cell, value)?;
            Ok(1)
        }
        0x481075 => {
            // I_NombreAlliances: players 0..=3 other than the nation DWORD [+0]
            // with PlayerEstAllie(nation byte, other) & 0xff == 1, counted.
            let mut count = 0u32;
            for other in 0..4u32 {
                if other == a[0] {
                    continue;
                }
                if crate::frame_economy::call(m, "PlayerEstAllie", &[a[0] & 255, other])? & 255 == 1
                {
                    count += 1;
                }
            }
            Ok(count)
        }
        0x4810aa => {
            // I_ChercheNationPasAllie: among players 0..=3 other than the
            // caller's player DWORD and not allied to it (PlayerEstAllie & 0xff
            // == 0), the one with the highest sign-extended PlayerGetSympathie
            // word strictly above -50 ("jle" skips: ties keep the earlier
            // player); -1 when none.
            let players = m.u32(0x5080a8)?;
            let mut best: i32 = -50;
            let mut found = 0xffffu32;
            for other in 0..4u32 {
                if other == player {
                    continue;
                }
                if crate::frame_economy::call(m, "PlayerEstAllie", &[player & 255, other])? & 255
                    != 0
                {
                    continue;
                }
                let value = m.u16(
                    players
                        .wrapping_add(20 * (player & 255))
                        .wrapping_add(0xc)
                        .wrapping_add(2 * other),
                )? as i16 as i32;
                if value > best {
                    best = value;
                    found = other;
                }
            }
            Ok(if found == 0xffff { u32::MAX } else { found })
        }
        0x4821f0 => {
            // V_ExploitePiece: every live member of the group [+8] outside state 5
            // gets SetTransporteEngin(piece cell [+0]) and SetDepotEngin(DL 2,
            // BX cave [+4], CL 0), then iagMessDispatchWith(group, 5).
            let mut member = crate::frame_raid::first_unit(m, a[2])?;
            while member != 0xffff {
                let r = crate::player::unit(m, u32::from(member))?;
                let state = m.u8(r.wrapping_add(2))?;
                if !matches!(state, 0 | 4 | 5) {
                    crate::player_orders::call(
                        m,
                        "SetTransporteEngin",
                        [u32::from(member), a[0] & 0xffff, 0, 0],
                        0x482232,
                    )?;
                    m.set_u8(r.wrapping_add(0x82), 2)?;
                    m.set_u16(r.wrapping_add(0x80), a[1] as u16)?;
                    m.set_u16(r.wrapping_add(0x84), 0)?;
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            let g = group(a[2]);
            if g != 0 {
                m.set_u16(g.wrapping_add(0x16), 5)?;
            }
            Ok(1)
        }
        0x481ece => {
            // V_EntreDansEngin: every live member of the group boards the target
            // unit (SetEnginRentreEngin), then iagMessDispatchWith(group, 8).
            let target = a[1] & 0xffff;
            let mut member = crate::frame_raid::first_unit(m, a[0])?;
            while member != 0xffff {
                if crate::frame_spawn::call(m, "EnginEstVivant", &[u32::from(member)])? as u8 != 0 {
                    board(m, u32::from(member), target)?;
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            let g = group(a[0]);
            if g != 0 {
                m.set_u16(g.wrapping_add(0x16), 8)?;
            }
            Ok(1)
        }
        0x481c9b => {
            // B_AjusteNbUnitesGroupe: 1 when units went back to the pool, else -1.
            let returned = return_to_pool(m, player, a[0])?;
            Ok(if (returned as i32) > 0 { 1 } else { u32::MAX })
        }
        0x481133 => {
            // I_DonneTypeUnite: EnginGetType of the unit word, low byte.
            Ok(crate::frame_core::call(m, "EnginGetType", &[a[0] & 0xffff])? & 255)
        }
        0x482020 | 0x48213c => crate::ai_resource_orders::issue(m, a, address == 0x48213c),
        0x4820e7 => crate::ai_resource_orders::shuttle(m, a),
        0x482579 => crate::chain_population::cheat(m, a[0], player),
        0x47ff34 => {
            // I_CommandeForceDefense: iagWarStartBuildUnits(player, wanted
            // strength [+0]); 0xff (nothing picked) becomes -1.
            let r = crate::ai_war_build::start_build_units(m, player, a[0])?;
            Ok(if r == 0xff { u32::MAX } else { r })
        }
        0x47febf => crate::frame_raid::rule(m, address, a, player, u32::MAX),
        0x47ff65 => crate::ai_war_position::danger(m, player, a[0], a[1]),
        0x48219e => {
            // V_RienFaire: every live (EnginEstVivant), on-map (EnginVisible:
            // +0x72) member of the group walks to its own cell (+0x7c). Always 1.
            let mut member = crate::frame_raid::first_unit(m, a[0])?;
            while member != 0xffff {
                let u = u32::from(member);
                if crate::frame_spawn::call(m, "EnginEstVivant", &[u])? as u8 != 0
                    && m.u8(crate::player::unit(m, u)?.wrapping_add(0x72))? != 0
                {
                    let cell = u32::from(m.u16(crate::player::unit(m, u)?.wrapping_add(0x7c))?);
                    crate::frame_spawn::call(m, "SetDeplaceEngin", &[u, cell])?;
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            Ok(1)
        }
        0x4802fb => waypoint(m, a[0], a[1], a[2] & 255),
        0x47fe4b => {
            // I_DonneForceNation: the force table 0x498cf4 summed over every
            // unit 0..799 that is alive (EnginEstVivant 0x443dad) and whose
            // owner byte (EnginGetPlayer 0x442045) equals the DWORD [+0].
            let mut force = 0u32;
            for u in 0..800u32 {
                if crate::frame_spawn::call(m, "EnginEstVivant", &[u])? as u8 == 0 {
                    continue;
                }
                let r = crate::player::unit(m, u)?;
                if u32::from(m.u8(r)?) != a[0] {
                    continue;
                }
                force = force.wrapping_add(m.u32(0x498cf4 + 4 * u32::from(m.u8(r + 1)?))?);
            }
            Ok(force)
        }
        0x47fc58 => {
            // I_DonneForceGroupe: the force table 0x498cf4 summed over the live
            // members of the group whose unit-type mask 0x4989f8 has the bit of
            // the player argument (second argument, low byte, shift count).
            // Wrapped-zero record or empty group: 0.
            let g = group(a[0]);
            if g == 0 || m.u16(g.wrapping_add(0x12))? == 0 {
                return Ok(0);
            }
            let bit = 1u32.wrapping_shl(a[1] & 255);
            let mut force = 0u32;
            let mut member = crate::frame_raid::first_unit(m, a[0])?;
            while member != 0xffff {
                let kind =
                    u32::from(m.u8(crate::player::unit(m, u32::from(member))?.wrapping_add(1))?);
                if m.u32(0x4989f8 + 4 * kind)? & bit != 0
                    && crate::frame_spawn::call(m, "EnginEstVivant", &[u32::from(member)])? as u8
                        != 0
                {
                    force = force.wrapping_add(m.u32(0x498cf4 + 4 * kind)?);
                }
                member = crate::frame_raid::next_unit(m)?;
            }
            Ok(force)
        }
        0x485ec2 => {
            // I_DonneMineAssocieeGroupe: the player's mine association slot.
            let slot = 0x51bab8u32.wrapping_add(player.wrapping_mul(168));
            Ok(if m.u32(slot)? == a[0] {
                m.u32(slot.wrapping_add(4))?
            } else {
                u32::MAX
            })
        }
        0x48181a => {
            // I_DonneChampAssocieGroupe 0x485ba9: field associations of the
            // player (eight-byte records at 0x51ba90+168p, count 0x51bcf0).
            let count = m.u32(0x51bcf0)? as i32;
            let base = 0x51ba90u32.wrapping_add(player.wrapping_mul(168));
            let mut index = 0i32;
            while index < count {
                let record = base.wrapping_add((index as u32).wrapping_mul(8));
                if m.u32(record)? == a[0] {
                    return m.u32(record.wrapping_add(4));
                }
                index += 1;
            }
            Ok(u32::MAX)
        }
        0x485e53 | 0x485e66 => {
            // V_SetProductionColonsOn/Off: full-DWORD player, byte 0x51a98d + 16p.
            m.set_u8(
                0x51a98du32.wrapping_add(player.wrapping_shl(4)),
                u8::from(address == 0x485e53),
            )?;
            Ok(1)
        }
        0x485e7c => {
            // B_TesteProductionColons: exactly byte 1 succeeds.
            let flag = m.u8(0x51a98du32.wrapping_add(player.wrapping_shl(4)))?;
            Ok(if flag == 1 { 1 } else { u32::MAX })
        }
        0x4897a5 => {
            // V_AffecteComportement: wrapped group record address test, then
            // the low argument byte becomes the behaviour selector.
            let g = 0x51a9e0u32.wrapping_add(a[0].wrapping_shl(5));
            if g == 0 {
                return Ok(u32::MAX);
            }
            m.set_u8(g.wrapping_add(0x1b), a[1] as u8)?;
            Ok(1)
        }
        0x481426 => crate::ai_field_search::search(m, player),
        0x481833 => crate::ai_field_search::search_mine(m, player),
        0x485e98 => {
            // V_SetMineExploitee: mine then group, full DWORD owner arithmetic.
            let slot = 0x51bab8u32.wrapping_add(player.wrapping_mul(168));
            m.set_u32(slot.wrapping_add(4), a[0])?;
            m.set_u32(slot, a[1])?;
            Ok(1)
        }
        0x481738 | 0x48179e => crate::ai_field_search::nearest_cave(m, a[0], player),
        0x4817f2 => crate::ai_field_search::field_value(m, a[0]),
        0x481806 => crate::ai_field_search::max_workers(m, a[0]),
        0x485ef3 => {
            // B_TestMaxChampsExploites: signed DWORD comparison, full-width
            // player arithmetic; no argument operands or RNG are consumed.
            let count = m.u32(0x51bcf0)? as i32;
            let maximum = m.u32(0x51bac4u32.wrapping_add(player.wrapping_mul(168)))? as i32;
            Ok(if count <= maximum { 1 } else { u32::MAX })
        }
        0x48264a => {
            // iagVilleGetNbBatEnCoursType: full dword player/type indices,
            // wrapping scaled address, zero-extended word before signed test.
            let address = construction_counter(player, a[0]);
            Ok(if m.u16(address)? > 0 { 1 } else { u32::MAX })
        }
        0x482667 => {
            // V_MiseAJourBatimentEnCoursConstruction -> iagVilleIncBatEnCours.
            // This is a scheduling counter, not an invented completed building.
            let address = construction_counter(player, a[0]);
            m.set_u16(address, m.u16(address)?.wrapping_add(1))?;
            Ok(1)
        }
        0x4806db => unit_building_type(a[0] as u8).ok_or_else(|| {
            stop(
                "I_DonneTypeBatiment requires incoming DL for unassociated type",
                0x41cc11,
            )
        }),
        0x48068f => {
            let state = u32::from(m.u8(0x49a0c8 + 276 * (a[0] & 65535) + 5)?);
            let table = 0x498610u32.wrapping_add(a[1].wrapping_mul(28));
            let count = m.u32(table)?;
            // Bound malformed input without claiming a fabricated answer.
            if count > 10000 {
                return Err(stop("B_EtatBatiment malformed state table", 0x4806b6));
            }
            for i in 0..count {
                if state == m.u32(table.wrapping_add(4u32.wrapping_mul(i + 1)))? {
                    return Ok(1);
                }
            }
            Ok(u32::MAX)
        }
        0x481372 => {
            // The native scaled address wraps at 32 bits before indexing.
            let selector = a[0] & 0x7fffffff;
            if selector >= 3 {
                // Native indexes past its three initialized stack words.
                return Err(stop(
                    "I_CombienCouteUnite outside initialized costs",
                    0x481393,
                ));
            }
            Ok(u32::from(
                crate::ai_kernels::unit_costs(m, a[1])?[selector as usize],
            ))
        }
        0x481399 => {
            // BatimentGetRessource41aace fills exactly three local WORD costs.
            // The caller's scaled stack offset wraps before the selection.
            let selector = a[0] & 0x7fffffff;
            if selector >= 3 {
                return Err(stop(
                    "I_CombienCouteBatiment outside initialized costs",
                    0x481393,
                ));
            }
            Ok(u32::from(
                m.u16(0x495d98 + 52 * (a[1] & 255) + 2 * selector)?,
            ))
        }
        0x481fb4 => crate::ai_kernels::first_group(m, player, a[0]),
        0x47ffd8 => {
            let value = crate::chain_technology::Technology::default()
                .entry(
                    m,
                    "TypeBatimentEstRealisable",
                    R {
                        eax: player & 255,
                        edx: a[0] & 255,
                        ..R::default()
                    },
                )?
                .registers
                .eax
                & 255;
            Ok(if value == 1 { 1 } else { u32::MAX })
        }
        0x480644 => available_building(m, a[0], player),
        // I_DonneNationBatiment480d56: low-word index, zero-extended owner byte.
        0x480d56 => Ok(u32::from(m.u8(0x49a0cc + 276 * (a[0] & 65535))?)),
        0x480aed => attack_building(m, a),
        0x481400 => {
            let selector = a[0] & 0x7fffffff;
            let b = 0x4e2ea8 + 32 * (player & 255);
            // GetRessource reads all three words in this order and clears the
            // fourth output before the shared signed comparison.
            let resources = [m.u16(b + 10)?, m.u16(b + 18)?, m.u16(b + 2)?, 0];
            let value =
                u32::from(*resources.get(selector as usize).ok_or_else(|| {
                    stop("B_TestRessource outside initialized resources", 0x4813de)
                })?);
            Ok(if (value as i32) < (a[1] as i32) {
                u32::MAX
            } else {
                1
            })
        }
        0x481c28 => allocate_group(m, player, a[0]),
        0x481c4a => transfer_unit(m, player, a[0], a[1]),
        0x481c38 => {
            delete_group(m, player, a[0])?;
            Ok(1)
        }
        0x480859 => city_position(m, player),
        _ => unreachable!(),
    }
}

/// Native RULE ABI: EAX points to seven argument dwords followed by player
/// and task index. All seven non-EAX registers, including ESP, are preserved
/// except the single return-address pop. Local native stack scratch is private.
pub fn entry(m: &mut MemoriaJogo, name: &str, mut r: R) -> Result<R> {
    let (address, count) = match name {
        "B_EtatBatiment" => (0x48068f, 2),
        "I_CombienCouteUnite" => (0x481372, 2),
        "I_CombienCouteBatiment" => (0x481399, 2),
        "I_DonneNationBatiment" => (0x480d56, 1),
        "I_RecupereNoGroupe" => (0x481fb4, 1),
        "I_DonnePositionMaVille" => (0x480859, 0),
        "V_LibereGroupeVide" => (0x481c38, 1),
        "B_AiJeConnaissanceBatiment" => (0x47ffd8, 1),
        "I_AiJeBatiment" => (0x480644, 1),
        "I_DonneBatimentAAttaquer" => (0x480aed, 3),
        "B_TestRessource" => (0x481400, 2),
        "I_DonneNoGroupe" => (0x481c28, 1),
        "B_TransfertUnite" => (0x481c4a, 2),
        "B_AiJeBatimentEnCoursConstruction" => (0x48264a, 1),
        "V_MiseAJourBatimentEnCoursConstruction" => (0x482667, 1),
        "B_TestMaxChampsExploites" => (0x485ef3, 0),
        "I_ChercheChampNonExploite" => (0x481426, 0),
        "I_ChercheMine" => (0x481833, 0),
        "V_SetMineExploitee" => (0x485e98, 2),
        "B_OccupationColonMinBatiment" => (0x4806ed, 2),
        "I_DonneBatimentProche" => (0x4808b4, 2),
        "V_BatimentRecruteUnite" => (0x480731, 2),
        "B_AiJeConnaissanceUnitee" => (0x47fffb, 1),
        "I_DonnePositionBatiment" => (0x480803, 1),
        "V_Recolter" => (0x482020, 3),
        "V_ExploiteMine" => (0x48213c, 3),
        "V_SetNavetteCaveToCulture" => (0x4820e7, 3),
        "V_TricheColon" => (0x482579, 1),
        "V_AffecteComportement" => (0x4897a5, 2),
        "B_InitConflit" => (0x47febf, 3),
        "B_TestUniteDanger" => (0x47ff65, 2),
        "I_DonneForceGroupe" => (0x47fc58, 2),
        "I_DonneForceNation" => (0x47fe4b, 1),
        "V_RienFaire" => (0x48219e, 1),
        "I_ChercheWayPoint" => (0x4802fb, 3),
        "I_DonneChampAssocieGroupe" => (0x48181a, 1),
        "I_DonneMineAssocieeGroupe" => (0x485ec2, 1),
        "V_SetProductionColonsOn" => (0x485e53, 0),
        "V_SetProductionColonsOff" => (0x485e66, 0),
        "B_TesteProductionColons" => (0x485e7c, 0),
        "I_DonneCaveAssocieeChamp" => (0x481738, 1),
        "I_DonneCaveAssocieeMine" => (0x48179e, 1),
        "I_DonneValeurChamp" => (0x4817f2, 1),
        "I_DonneMaxExploitantsDansChamp" => (0x481806, 1),
        "V_SetMaxUnites" => (0x482010, 2),
        "V_SetChampExploite" => (0x485d7b, 2),
        "V_SetChampNonExploite" => (0x485dfc, 1),
        "B_AjusteNbUnitesGroupe" => (0x481c9b, 1),
        "V_EntreDansEngin" => (0x481ece, 2),
        "I_DonneValeurMine" => (0x481bcf, 1),
        "I_DonneCaseLibreZoneProche" => (0x4801a2, 1),
        "V_EntreBatiment" => (0x481e07, 2),
        "V_CueillirChamp" => (0x482085, 3),
        "B_TestMaxRocheDansStock" => (0x485f20, 0),
        "V_DeplacerUnite" => (0x481146, 2),
        "I_DonneBatimentOuEstUnite" => (0x48094a, 1),
        "V_InterditCaseMine" => (0x481be8, 1),
        "I_CommandeForceDefense" => (0x47ff34, 1),
        "V_RestoreCaseMine" => (0x481c03, 1),
        "I_DonneBatimentPasPlein" => (0x480951, 0),
        "I_DonneForceGroupeDansZone" => (0x47fead, 3),
        "I_ChercheCivilProche" => (0x4804a1, 2),
        "I_CherchePiece" => (0x4819d7, 0),
        "B_TransfertUniteFastest" => (0x482433, 2),
        "V_VideEngin" => (0x481e88, 1),
        "I_DonneBatimentDansZone" => (0x4801d2, 2),
        "V_TricheToutLeMondeMange" => (0x482501, 1),
        "I_DonneValeurPiece" => (0x481b9e, 1),
        "B_EncorePiece" => (0x481bb1, 1),
        "V_AttacheConflitGroupe" => (0x47ffa4, 2),
        "I_DonneCaveProchePiece" => (0x481b46, 1),
        "V_ExploitePiece" => (0x4821f0, 3),
        "I_CombienDAlliances" => (0x480f16, 0),
        "I_RecupereSympathie" => (0x481034, 1),
        "V_ModifieSympathie" => (0x481049, 2),
        "I_NombreAlliances" => (0x481075, 1),
        "I_ChercheNationPasAllie" => (0x4810aa, 0),
        "I_DonneNationPlusHaute" => (0x480ea6, 0),
        "I_DonneNationPlusBasse" => (0x480ee2, 0),
        "I_DonneBatimentMarchandage" => (0x480cee, 3),
        "B_Allie" => (0x480e4f, 1),
        "V_DeclareGuerre" => (0x480e72, 1),
        "V_RefuseAlliance" => (0x480e1f, 1),
        "V_FixeAlliance" => (0x480d97, 2),
        "I_DonneTypeUnite" => (0x481133, 1),
        "B_CompleteGroupe" => (0x481c75, 2),
        "I_DonneTypeBatiment" => (0x4806db, 1),
        _ => return Err(stop("unknown AI RULE leaf", 0x48068f)),
    };
    let a = if address == 0x4806db {
        vec![u32::from(m.u8(r.eax)?)]
    } else {
        (0..count)
            .map(|i| m.u32(r.eax.wrapping_add(4 * i)))
            .collect::<Result<Vec<_>>>()?
    };
    let player = if !matches!(
        address,
        0x48068f
            | 0x481372
            | 0x481399
            | 0x480aed
            | 0x4806db
            | 0x4817f2
            | 0x481806
            | 0x482010
            | 0x482020
            | 0x48213c
            | 0x4820e7
            | 0x4897a5
            | 0x480803
            | 0x4806ed
    ) {
        m.u32(r.eax.wrapping_add(28))?
    } else {
        0 // these native leaves never read the player argument
    };
    r.eax = if address == 0x4806db {
        unit_building_type(a[0] as u8).unwrap_or(r.edx & 255)
    } else {
        operation(m, address, &a, player)?
    };
    r.esp = r.esp.wrapping_add(4);
    Ok(r)
}

#[cfg(test)]
mod ownership_tests {
    use super::*;

    const UNITS: u32 = 0x0100_0000;

    fn world() -> MemoriaJogo {
        let mut m =
            MemoriaJogo::from_blocks([(0x494000, vec![0; 0x9c000]), (UNITS, vec![0; 800 * 276])])
                .unwrap();
        m.set_u32(0x4e19a0, UNITS).unwrap();
        for u in 0..800u32 {
            m.set_u32(UNITS + 276 * u + 0x10c, u32::MAX).unwrap();
        }
        for a in (0x51b664..0x51b664 + 96 * 5).step_by(4) {
            m.set_u32(a, u32::MAX).unwrap();
        }
        m
    }

    #[test]
    fn a_task_cannot_fill_a_group_number_now_owned_by_another_player() {
        // MONDE16 seed 3 shape: player 1 still holds group 18's number after
        // deleting it; player 3 allocated it again.
        let mut m = world();
        let pool = crate::frame_raid::new_group(&mut m, 1, 0).unwrap();
        m.set_u8(UNITS + 276 * 5 + 1, 9).unwrap();
        crate::frame_raid::add_unit(&mut m, pool, 5).unwrap();
        let foreign = crate::frame_raid::new_group(&mut m, 3, 8).unwrap();
        assert!(group_is_players(&m, 3, foreign).unwrap());
        assert!(!group_is_players(&m, 1, foreign).unwrap());
        assert_eq!(transfer_unit(&mut m, 1, foreign, 9).unwrap(), u32::MAX);
        assert_eq!(m.u16(group(foreign) + 0x12).unwrap(), 0);
        assert_eq!(m.u16(group(pool) + 0x10).unwrap(), 5);
        // Its own group still fills from its pool, as in the original.
        let own = crate::frame_raid::new_group(&mut m, 1, 8).unwrap();
        assert_eq!(transfer_unit(&mut m, 1, own, 9).unwrap(), 1);
        assert_eq!(m.u16(group(own) + 0x10).unwrap(), 5);
        assert_eq!(m.u16(group(pool) + 0x12).unwrap(), 0);
    }

    #[test]
    fn a_freed_or_out_of_table_group_is_nobodys() {
        let mut m = world();
        let g = crate::frame_raid::new_group(&mut m, 2, 8).unwrap();
        assert!(group_is_players(&m, 2, g).unwrap());
        delete_group(&mut m, 2, g).unwrap();
        assert!(!group_is_players(&m, 2, g).unwrap());
        assert!(!group_is_players(&m, 2, u32::MAX).unwrap());
        assert!(!group_is_players(&m, 2, 100).unwrap());
    }
}
