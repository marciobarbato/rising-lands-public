//! Intact retained HANGAR / MONASTERE / ATELIER continuation.
//! Native scheduling and world publication stay in CalculBatimentNextEtape.
use crate::{frame::stop, FrameRegisters, MemoriaJogo, Result};

fn record(index: u16) -> u32 {
    0x49a0c8 + 276 * u32::from(index)
}
fn scientists(m: &mut MemoriaJogo, index: u16) -> Result<u16> {
    Ok(crate::chain_bags::body(
        m,
        "BagContient",
        FrameRegisters {
            eax: record(index) + 12,
            edx: 1,
            ebx: 9,
            ..Default::default()
        },
    )?
    .eax as u16)
}

/// Full selected caller, void effects. All occupancy scans use native capacity.
pub fn next(m: &mut MemoriaJogo, index: u16) -> Result<()> {
    let r = record(index);
    let kind = m.u8(r + 8)?;
    if !matches!(kind, 5 | 8 | 15 | 19) {
        return stop("Building loop completion type", 0x426910);
    }
    let inhabited = scientists(m, index)? != 0;
    match kind {
        5 => {
            if inhabited {
                production(m, index)?;
            }
            crate::building_support::animate_door(m, index)
        }
        8 => {
            if !inhabited {
                return Ok(());
            }
            production(m, index)?;
            animate_hangar(m, index)?;
            if m.u8(r + 0x74)? == 0 {
                arrival(m, index)?;
            }
            publish_hangar(m, m.u8(r + 4)?, index).map(|_| ())
        }
        15 => {
            if inhabited {
                // The original type15 branch is the existing unit producer,
                // unlike ATELIER's trajectory or HANGAR's entry/exit machines.
                crate::frame_economy::production(m, u32::from(index), false, true)?;
            }
            Ok(())
        }
        19 => {
            if inhabited {
                return stop("BatimentSoignePerso", 0x424c2e);
            }
            Ok(())
        }
        _ => unreachable!(),
    }
}

/// CalculTroupe 0x4259e6, selected type5/8 branches. Uncovered active entry,
/// exit and trajectory bodies remain actual callee-entry dependencies.
pub fn production(m: &mut MemoriaJogo, index: u16) -> Result<()> {
    let r = record(index);
    match m.u8(r + 8)? {
        8 => {
            if m.u8(r + 0x74)? != 0 {
                if (m.u32(r + 0x7c)? as i32 >> 16) < 128 {
                    hangar_enter(m, index)?;
                } else {
                    hangar_leave(m, index)?;
                }
            }
        }
        5 => {
            if m.u32(r + 0x8c)? != 0 && m.u8(r + 0x74)? != 0 {
                let requested = u32::from(m.u8(r + 0x7b)?);
                let time = 100 * u32::from(m.u16(0x4965ae + 50 * requested)?);
                if m.u32(r + 0x8c)? < time {
                    let qty = scientists(m, index)?;
                    let capacity = m.u8(0x495d8c + 52 * 5 + 18)?;
                    let coefficient =
                        f64::from_le_bytes(m.read(0x49443a, 8)?.try_into().expect("f64 width"));
                    let value = (f64::from(qty) / f64::from(capacity)) * coefficient;
                    // __CHP truncates, then FISTP stores integer-indefinite for
                    // a nonfinite/out-of-i32 result. The caller consumes AX.
                    let converted = if value.is_finite()
                        && value.trunc() >= f64::from(i32::MIN)
                        && value.trunc() <= f64::from(i32::MAX)
                    {
                        value.trunc() as i32 as u32
                    } else {
                        0x80000000
                    };
                    m.set_u32(
                        r + 0x8c,
                        m.u32(r + 0x8c)?.wrapping_add(100 + (converted & 65535)),
                    )?;
                }
                // Native recounts time after the progress store.
                let requested = u32::from(m.u8(r + 0x7b)?);
                let time = 100 * u32::from(m.u16(0x4965ae + 50 * requested)?);
                if m.u32(r + 0x8c)? >= time {
                    crate::chain_vision::position(
                        m,
                        FrameRegisters {
                            eax: u32::from(index),
                            ..Default::default()
                        },
                    )?;
                    m.set_u8(r + 0x75, 1)?;
                    m.set_u8(r + 0x74, 1)?;
                }
            }
            if m.u8(r + 0x74)? != 0 && m.u8(r + 0x76)? == 7 && m.u8(r + 0x75)? != 0 {
                trajectory(m, index)?;
            }
        }
        _ => return stop("CalculTroupe completion type", 0x4259e6),
    }
    Ok(())
}

/// The door bytes as the hangar and repair machines read them.
fn door_open(m: &MemoriaJogo, r: u32) -> Result<bool> {
    Ok(m.u8(r + 0x76)? == 7 && m.u8(r + 0x75)? != 0)
}
fn door_shut(m: &MemoriaJogo, r: u32) -> Result<bool> {
    Ok(m.u8(r + 0x76)? == 0 && m.u8(r + 0x75)? == 0)
}
fn step(m: &mut MemoriaJogo, r: u32) -> Result<()> {
    m.set_u16(r + 0x7e, m.u16(r + 0x7e)?.wrapping_add(1))
}

/// AnimDeplaceEngin 0x448088, the movement animation step of a flying unit:
/// +0xa9 counts to +0xa7, then +0xa4 advances modulo +0xa5. A balloon (kind 17)
/// only takes AnimBallonStatic 0x447b2f (0x44810e) and keeps its counters.
fn flight_anim(m: &mut MemoriaJogo, unit: u32) -> Result<()> {
    let u = crate::player::unit(m, unit)?;
    let kind = m.u8(u + 1)?;
    if kind == 17 {
        return crate::frame_movement::balloon_animation(m, unit & 65535);
    }
    if !(kind <= 15 || (18..=25).contains(&kind)) {
        return Ok(());
    }
    let count = m.u8(u + 0xa9)?.wrapping_add(1);
    m.set_u8(u + 0xa9, count)?;
    if count != m.u8(u + 0xa7)? {
        return Ok(());
    }
    let frames = m.u8(u + 0xa5)?;
    if frames == 0 {
        return stop("AnimDeplaceEngin division", 0x4480f9);
    }
    let next = (u32::from(m.u8(u + 0xa4)?) + 1) % u32::from(frames);
    m.set_u8(u + 0xa9, 0)?;
    m.set_u8(u + 0xa4, next as u8)
}

/// 0x4212f5: the hangar's order fields after the unit has left or landed.
fn hangar_reset(m: &mut MemoriaJogo, r: u32) -> Result<()> {
    m.set_u16(r + 0x78, 0xffff)?;
    m.set_u8(r + 0x7b, 0xff)?;
    m.set_u16(r + 0x7c, 0)?;
    m.set_u32(r + 0x8c, 0)?;
    m.set_u16(r + 0x86, 0xffff)?;
    m.set_u16(r + 0x88, 0xffff)?;
    m.set_u16(r + 0x7e, 0)?;
    m.set_u16(r + 0x80, 0)?;
    m.set_u16(r + 0x82, 0)?;
    m.set_u8(r + 0x74, 0)?;
    if m.u8(r + 5)? != 3 {
        m.set_u8(r + 5, 5)?;
    }
    Ok(())
}

/// RentreTroupeHangar 0x421387: the door opens, the unit descends to the floor
/// (0xffab for kinds 3 and 11..17, else 0xffcc) along +0x82, the door shuts and
/// the unit is lodged.
fn hangar_enter(m: &mut MemoriaJogo, index: u16) -> Result<()> {
    let r = record(index);
    let unit = u32::from(m.u16(r + 0x88)?);
    if unit == 65535 {
        return Ok(());
    }
    let u = crate::player::unit(m, unit)?;
    let kind = m.u8(u + 1)?;
    let floor: i16 = if kind == 3 || (11..=17).contains(&kind) {
        -0x55
    } else {
        -0x34
    };
    match m.u16(r + 0x7e)? {
        0 => {
            m.set_u8(r + 0x75, 1)?;
            if m.u8(r + 0x76)? == 7 {
                step(m, r)?;
            }
        }
        1 => {
            if !door_open(m, r)? {
                return Ok(());
            }
            let offset = m.u16(r + 0x82)?;
            if floor >= offset as i16 {
                m.set_u8(r + 0x75, 0)?;
                step(m, r)?;
            } else {
                m.set_u16(r + 0x82, offset.wrapping_sub(u16::from(m.u8(u + 0x98)?)))?;
                flight_anim(m, unit)?;
            }
        }
        2 if door_shut(m, r)? => {
            crate::chain_entry::lodge(m, index, unit as u16)?;
            hangar_reset(m, r)?;
        }
        _ => {}
    }
    Ok(())
}

/// BatimentGetCaseEntrer 0x41d057 on a hangar (its only caller here).
fn entry_cell(m: &MemoriaJogo, r: u32, u: u32) -> Result<u16> {
    let kind = m.u8(u + 1)?;
    let offset = match m.u8(r + 8)? {
        8 if kind == 3 || (11..=17).contains(&kind) => 0x403,
        8 => 0x303,
        14 => 0x402,
        _ => return Ok(0),
    };
    Ok(m.u16(r + 2)?.wrapping_add(offset))
}

/// SortTroupeHangar 0x4215fb: states 0x80..0x82, the mirror of the descent.
/// On reaching height 0 every Bag entry equal to the unit leaves at the exit
/// cell and is told so (message 0x45).
fn hangar_leave(m: &mut MemoriaJogo, index: u16) -> Result<()> {
    let r = record(index);
    let unit = u32::from(m.u16(r + 0x88)?);
    if unit == 65535 {
        return Ok(());
    }
    let u = crate::player::unit(m, unit)?;
    match m.u16(r + 0x7e)? {
        0x80 => {
            m.set_u8(r + 0x75, 1)?;
            if m.u8(r + 0x76)? == 7 {
                step(m, r)?;
            }
        }
        0x81 => {
            let cell = entry_cell(m, r, u)?;
            if !crate::chain_search::passable(m, unit, u32::from(cell))? {
                let cell = entry_cell(m, r, u)?;
                return free_cell(m, cell, unit as u16);
            }
            if !door_open(m, r)? {
                return Ok(());
            }
            let offset = m.u16(r + 0x82)?;
            if (offset as i16) < 0 {
                m.set_u16(r + 0x82, offset.wrapping_add(u16::from(m.u8(u + 0x98)?)))?;
                return flight_anim(m, unit);
            }
            m.set_u8(r + 0x75, 0)?;
            step(m, r)?;
            let bag = r + 12;
            let count = m.u16(bag)?;
            for slot in 0..count {
                if u32::from(m.u16(bag + 6 + 2 * u32::from(slot))?) != unit {
                    continue;
                }
                let removed = crate::chain_cargo::remove_index(m, bag, slot)?;
                let cell = entry_cell(m, r, u)?;
                crate::chain_recruit::set_sort(m, u32::from(removed), u32::from(cell))?;
                crate::frame_ai::call(m, "iagUnitSendMessage", &[unit, 0x45, 0])?;
            }
        }
        0x82 if door_shut(m, r)? => hangar_reset(m, r)?,
        _ => {}
    }
    Ok(())
}

/// The order-field reset repeated at 0x422bfb and 0x422cc8.
fn reset_order(m: &mut MemoriaJogo, r: u32) -> Result<()> {
    m.set_u16(r + 0x78, 0xffff)?;
    m.set_u8(r + 0x7b, 0xff)?;
    m.set_u16(r + 0x7c, 0)?;
    m.set_u16(r + 0x7e, 0)?;
    m.set_u16(r + 0x86, 0xffff)?;
    if m.u8(r + 8)? != 0x14 {
        m.set_u16(r + 0x80, 0)?;
        m.set_u16(r + 0x82, 0)?;
    }
    m.set_u8(r + 0x74, 0)?;
    m.set_u32(r + 0x8c, 0)?;
    if m.u8(r + 5)? != 3 {
        m.set_u8(r + 5, 5)?;
    }
    Ok(())
}

/// EnginLibereCase 0x443470 with no use of its AL result: a stationary
/// same-owner occupant of `cell` is sent to a free neighbouring cell.
pub(crate) fn free_cell(m: &mut MemoriaJogo, cell: u16, unit: u16) -> Result<()> {
    let other = crate::frame_movement::call(m, "ChercheIndiceEngin", &[u32::from(cell)])? as u16;
    m.set_u16(0x4e1a34, other)?;
    if other == 65535 {
        return Ok(());
    }
    let q = crate::player::unit(m, u32::from(other))?;
    let u = crate::player::unit(m, u32::from(unit))?;
    if m.u8(q)? != m.u8(u)? || m.u16(q + 8)? != m.u16(q + 0x7e)? {
        return Ok(());
    }
    let free = crate::chain_unblock::free_neighbor(m, cell, other)?;
    m.set_u16(0x4e1a36, free)?;
    if free != 65535 {
        crate::frame_spawn::order(m, u32::from(other), free)?;
    }
    Ok(())
}

fn exit_cell(m: &MemoriaJogo, index: u16) -> Result<u32> {
    Ok(crate::chain_vision::position(
        m,
        FrameRegisters {
            eax: u32::from(index),
            ..Default::default()
        },
    )? & 65535)
}

/// CalculTrajectoire 0x422900: the vehicle leaving the workshop. +0x7e steps
/// through creation, the slide along +0x80/+0x82 towards the kind's limits,
/// and the hand-over, where the pilot boards and the workshop is reset.
fn trajectory(m: &mut MemoriaJogo, index: u16) -> Result<()> {
    let r = record(index);
    let kind = m.u8(r + 0x7b)?;
    // Jump table 0x422884: the +0x82 end and the +0x80 limit per kind.
    let limits = match kind {
        11 | 15 => Some((0x78, if kind == 11 { 0x42 } else { 0x4a })),
        12 => Some((0x86, 0x52)),
        13 => Some((0x74, 0x44)),
        14 => Some((0x7a, 0x4e)),
        _ => None,
    };
    let state = m.u16(r + 0x7e)?;
    if matches!(state, 1 | 2) && limits.is_none() {
        return stop(
            "CalculTrajectoire kind outside the 0x422884 table",
            0x42292d,
        );
    }
    let (end, limit) = limits.unwrap_or((0, 0));
    let advance = match state {
        0 => {
            let cell = u32::from(m.u16(r + 2)?).wrapping_add(0x203) & 65535;
            let owner = u32::from(m.u8(r + 4)?);
            let unit =
                crate::frame_spawn::call(m, "CreerEngin", &[cell, 0xd, u32::from(kind), owner])?
                    & 65535;
            if unit == 65535 {
                return stop("CalculTrajectoire with no free unit slot", 0x4229c4);
            }
            m.set_u16(r + 0x86, unit as u16)?;
            let u = crate::player::unit(m, unit)?;
            m.set_u8(u + 0x90, 0xc)?;
            m.set_u8(u + 0x91, 0xc)?;
            // EnginSetEtat 0x44115f.
            if !matches!(m.u8(u + 2)?, 0 | 4) {
                m.set_u8(u + 2, 0xe)?;
                crate::frame_spawn::release(m, unit)?;
            }
            true
        }
        1 => {
            let cell = exit_cell(m, index)?;
            let unit = m.u16(r + 0x86)?;
            if !crate::chain_search::passable(m, u32::from(unit), cell)? {
                free_cell(m, cell as u16, unit)?;
                false
            } else {
                let slide = m.u16(r + 0x80)?;
                if slide as i16 >= limit as i16 {
                    true
                } else {
                    m.set_u16(r + 0x80, slide.wrapping_add(2))?;
                    let other = m.u16(r + 0x82)?.wrapping_add(2);
                    m.set_u16(r + 0x82, other)?;
                    false
                }
            }
        }
        2 => {
            let slide = m.u16(r + 0x82)?.wrapping_add(2);
            m.set_u16(r + 0x82, slide)?;
            slide == end
        }
        3 => {
            hand_over(m, index, kind)?;
            false
        }
        _ => false,
    };
    if advance {
        m.set_u16(r + 0x7e, m.u16(r + 0x7e)?.wrapping_add(1))?;
    }
    Ok(())
}

/// 0x422a8a..0x422d3c: the pilot leaves the Bag and boards the new vehicle.
/// A structure left empty is abandoned to the neutral owner.
fn hand_over(m: &mut MemoriaJogo, index: u16, kind: u8) -> Result<()> {
    let r = record(index);
    let s = u32::from(index);
    let cell = exit_cell(m, index)?;
    let bag = r + 12;
    let pilot = crate::chain_bags::body(
        m,
        "BagRetire",
        FrameRegisters {
            eax: bag,
            edx: 1,
            ebx: 9,
            ..Default::default()
        },
    )?
    .eax & 65535;
    if pilot == 65535 {
        return stop("CalculTrajectoire with no pilot in the Bag", 0x422afb);
    }
    let unit = u32::from(m.u16(r + 0x86)?);
    crate::frame_spawn::call(m, "EnginSetType", &[unit, cell, 0xd, u32::from(kind)])?;
    crate::chain_recruit::entry(
        m,
        "EnginAddVie",
        FrameRegisters {
            eax: pilot,
            edx: 1000,
            ..Default::default()
        },
    )?;
    crate::spell_remaining::lodge_unit(m, unit as u16, pilot as u16)?;
    crate::frame_spawn::call(m, "PlaceEngin", &[unit])?;
    crate::frame_ai::call(m, "iagUnitSendMessage", &[unit, 0x45, 0])?;
    crate::chain_queries::city_message(m, s, 3, 0)?;
    m.set_u8(r + 0x75, 0)?;
    let t = m.u8(r + 8)?;
    let contains = |m: &mut MemoriaJogo, name: &str, edx: u32, ebx: u32| -> Result<bool> {
        Ok(crate::chain_bags::body(
            m,
            name,
            FrameRegisters {
                eax: bag,
                edx,
                ebx,
                ..Default::default()
            },
        )?
        .eax as u16
            != 0)
    };
    let occupied = if t == 0x15 {
        contains(m, "BagContientTroupe", 0, 0)?
    } else {
        (t == 2 && contains(m, "BagContientTroupe", 0, 0)?) || contains(m, "BagContient", 1, 9)?
    };
    if !occupied {
        if matches!(t, 1 | 2 | 5) {
            reset_order(m, r)?;
        }
        let owner = m.u8(r + 4)?;
        if !matches!(t, 9 | 12) {
            // Jump table 0x422898.
            let radius = match t {
                0 => 0x14,
                2 => 0xa,
                4 => 5,
                10 | 11 => 7,
                17 | 19 | 21 => 4,
                _ if t <= 21 => 8,
                _ => return stop("CalculTrajectoire structure type", 0x422d8c),
            };
            let centre = crate::frame_core::call(m, "BatimentGetCase", &[s])? as u16;
            crate::player_sight::area(m, centre, radius, owner, 2)?;
        }
        let selected = 0x495be0 + 2 * u32::from(owner);
        if u32::from(m.u16(selected)?) == s {
            m.set_u16(selected, 0xffff)?;
        }
        m.set_u8(r + 4, 200)?;
        crate::chain_vision::structure_minimap(m, index, 1)?;
    }
    reset_order(m, r)
}

/// AnimPorteBatiment 0x420d3a's HANGAR call (0x426ac0).
pub fn animate_hangar(m: &mut MemoriaJogo, index: u16) -> Result<()> {
    crate::building_support::anim_porte(m, index)
}

/// ControleArrivageHangar 0x424de4 for type8. Its count scan deliberately
/// ignores slot tags after the habitation test; even a second scientist can
/// start the native exit sequence. Neither count nor queue is normalized.
pub fn arrival(m: &mut MemoriaJogo, index: u16) -> Result<()> {
    let r = record(index);
    if m.u8(r + 8)? != 8 {
        return stop("ControleArrivageHangar type", 0x424e13);
    }
    if scientists(m, index)? == 0 || m.u8(0x4e2e7c + u32::from(m.u8(r + 4)?))? != 0 {
        return Ok(());
    }
    let mut seen = false;
    for slot in 0..u32::from(m.u16(r + 12)?) {
        let member = m.u16(r + 18 + 2 * slot)?;
        let unit = crate::frame_core::unit(m, u32::from(member))?;
        if m.u8(unit + 1)? != 9 {
            seen = true;
        }
        if seen {
            if matches!(m.u16(r + 0x88)?, 0 | 65535) {
                m.set_u8(r + 5, 4)?;
                m.set_u8(r + 0x74, 1)?;
                m.set_u16(r + 0x7e, 128)?;
                m.set_u16(r + 0x80, 0)?;
                m.set_u16(r + 0x82, 0xffab)?;
                m.set_u16(r + 0x88, member)?;
                // EnginSetDir 0x4570f4: both direction stores precede
                // the independent final EnginGetType query.
                m.set_u8(unit + 0x90, 13)?;
                m.set_u8(unit + 0x91, 13)?;
                m.set_u8(r + 0x7b, m.u8(unit + 1)?)?;
            }
            return Ok(());
        }
        seen = m.u8(unit + 1)? == 9;
    }
    Ok(())
}

/// SetTableauHangar 0x461e6e: first ffff among ten words; no deduplication.
/// Returns native AL, including the full-table XOR result (10 XOR 10 = 0).
pub fn publish_hangar(m: &mut MemoriaJogo, owner: u8, index: u16) -> Result<u8> {
    for slot in 0..10 {
        let p = 0x4e296c + 20 * u32::from(owner) + 2 * slot;
        if m.u16(p)? == 65535 {
            m.set_u16(p, index)?;
            return Ok(1);
        }
    }
    Ok(0)
}
