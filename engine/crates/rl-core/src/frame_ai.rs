//! AI/runtime leaves translated from rotinas_quadro_ia.py, incursao_ia.py and ia.py.
//! Unsupported instruction and monster-spawn paths name their intact-image address.
use crate::{EngineError, MemoriaJogo, Result};

fn stop(name: &'static str, address: u32) -> EngineError {
    EngineError::UnportedCallee { name, address }
}

pub fn call(m: &mut MemoriaJogo, name: &str, args: &[u32]) -> Result<u32> {
    match name {
        "MessageBoxCalcul" => {
            let tail = m.u16(0x507d56)?;
            let head = m.u16(0x507d54)?;
            if tail != head {
                let address = 0x4e7858u32.wrapping_add(518 * u32::from(head));
                let remaining = m.u16(address)?.wrapping_sub(1);
                m.set_u16(address, remaining)?;
                if remaining == 0 {
                    m.set_u16(0x507d54, head.wrapping_add(1) % 256)?;
                    m.set_u8(0x507d58, 0)?;
                }
            }
            Ok(0)
        }
        "ProcessInputGame" => {
            let pointer = 0x4e2458 + 4 * u32::from(args[0] as u8);
            let command = m.u16(m.u32(pointer)?)?;
            let target = if command > 0x27 {
                0x45f2de
            } else {
                m.u32(0x45ed20 + 4 * u32::from(command))?
            };
            if target != 0x45f2de {
                return Err(stop("ProcessInputGame command", target));
            }
            for offset in [0, 2, 4, 6, 8] {
                let address = m.u32(pointer)?.wrapping_add(offset);
                m.set_u16(address, 0)?;
            }
            m.u32(pointer)
        }
        "AvanceMonstre" => advance_monsters(m),
        "iaSolver" => crate::frame_tasks::solve(m, args[0]),
        "CalculNextReflect" => {
            update_player(m, args[0] as u8, args[1] as u8)?;
            Ok(1)
        }
        "iagVilleSendMessage" if args.get(1).is_some_and(|message| message & 255 == 3) => {
            crate::ai_recruit_message::city(m, args[0])
        }
        "iagVilleSendMessage" => {
            let [structure, message, argument] = args[..] else {
                return Err(stop("iagVilleSendMessage arguments", 0x484808));
            };
            crate::chain_queries::city_message(m, structure, message, argument)
        }
        "iagUnitSendMessage" => send_message(m, args[0] as u16, args[1] as u8, args[2] as u16),
        "iagUnitNoyau" => Ok(0),
        "iagVilleNoyau" => crate::ai_kernels::city(m, args[0]),
        "iagRessNoyau" => crate::ai_kernels::resource(m, args[0]),
        "iagWarNoyau" => crate::ai_kernels::war(m, args[0]),
        "iagUnitDeleteAllGroups" => {
            crate::mission_ai::delete_all_groups(m, args[0])?;
            Ok(0)
        }
        "iaProbNew" | "iaProbPost" | "iaProbSleep" | "RaidRule" => {
            crate::frame_tasks::call(m, name, args)
        }
        "iaGroupNew"
        | "iagUnitAddToGroup"
        | "iagUnitGetFirstUnitInGroup"
        | "iagUnitGetNextUnitInGroup"
        | "StockeAction"
        | "ConstruitGroupeMonstre" => crate::frame_raid::call(m, name, args),
        _ => Err(stop("unknown AI frame provider", 0x42c868)),
    }
}

fn update_player(m: &mut MemoriaJogo, player: u8, mission: u8) -> Result<u32> {
    if player > 4 {
        return Err(stop("iaCalculPlayer invalid player", 0x47f927));
    }
    let context = 0x5148b0 + 16 * u32::from(player);
    if player == 4 {
        advance_monsters(m)?;
        return call(m, "iaSolver", &[context]);
    }
    let state = 0x51a98c + 16 * u32::from(player);
    if m.u8(state + 4)? != 0 {
        return Ok(0);
    }
    if m.u32(0x4dfaa8 + 4 * u32::from(player))? == 0
        && m.u16(0x4e1928 + 2 * u32::from(player))? == 0
    {
        let count = m.u8(state + 9)?.wrapping_add(1);
        m.set_u8(state + 9, count)?;
        if count == 10 {
            call(m, "iagUnitDeleteAllGroups", &[u32::from(player)])?;
            m.set_u8(state + 4, 1)?;
            return Ok(0);
        }
    } else {
        m.set_u8(state + 9, 0)?;
    }
    for routine in [
        "iagUnitNoyau",
        "iagVilleNoyau",
        "iagRessNoyau",
        "iagWarNoyau",
        "iaSolver",
    ] {
        let result = if routine == "iaSolver" {
            call(m, routine, &[context])?
        } else {
            call(m, routine, &[u32::from(player), u32::from(mission)])?
        };
        if result != 0 {
            return Ok(result);
        }
    }
    Ok(0)
}

fn advance_monsters(m: &mut MemoriaJogo) -> Result<u32> {
    crate::frame_raid::advance(m)
}

fn send_message(m: &mut MemoriaJogo, unit: u16, message: u8, target: u16) -> Result<u32> {
    let record = m.u32(0x4e19a0)?.wrapping_add(276 * u32::from(unit));
    let owner = m.u8(record)?;
    if crate::frame_economy::call(m, "PlayerGetType", &[u32::from(owner)])? as u8 != 2 {
        return Ok(0);
    }
    let group = m.u32(record + 0x10c)?;
    let g = if group == u32::MAX {
        0
    } else {
        0x51a9e0u32.wrapping_add(group.wrapping_shl(5))
    };
    if message.wrapping_sub(0x41) > 11 {
        return Ok(0);
    }
    // Native message42 does not depend on the unit's current group pointer.
    if message == 0x42 {
        crate::ai_stage25::released_colonist(m, unit)?;
        return Ok(0);
    }
    if matches!(message, 0x45 | 0x46) {
        crate::ai_recruit_message::available(m, unit, message == 0x46)?;
        return Ok(0);
    }
    if message == 0x47 {
        // 0x482eb5: message 0x47 has no handler body.
        return Ok(0);
    }
    if message == 0x43 {
        return reserve_message(m, unit, record);
    }
    if !matches!(message, 0x44 | 0x48 | 0x49 | 0x4a | 0x4b | 0x4c) {
        return Err(stop("iagUnitSendMessage message handler", 0x482a49));
    }
    if g == 0 {
        return Ok(0);
    }
    if message == 0x44 {
        crate::chain_battle::dead_message(m, unit, group)?;
        return Ok(0);
    }
    let behaviour = m.u8(g + 0x1b)?;
    if behaviour == 0xff {
        if message == 0x4b {
            crate::chain_fatigue::set(m, u32::from(unit))?;
        }
    } else {
        let offset = match message {
            0x48 => 0,
            0x4a => 4,
            0x49 => 8,
            0x4c => 12,
            _ => 16,
        };
        let handler = m.u32(
            m.u32(0x498e94 + 4 * u32::from(behaviour))?
                .wrapping_add(offset),
        )?;
        if handler != 0 {
            if (message == 0x49 && handler == 0x489b5a) || (message == 0x48 && handler == 0x489bc8)
            {
                crate::chain_battle::detection(m, unit, target)?;
            } else if message == 0x4a && handler == 0x489ca2 {
                crate::chain_battle::ended(m, unit)?;
            } else if message == 0x4b && handler == 0x4897c8 {
                // Intact raid hunger handler: literal RET; caller progression remains live.
            } else if matches!(message, 0x48 | 0x49)
                && matches!(handler, 0x48986f | 0x489957 | 0x489a0f)
            {
                let order = match handler {
                    0x48986f => 0x1f,
                    0x489957 => 0x1e,
                    _ => 0x20,
                };
                crate::ai_behaviour_battle::battle(m, unit, target, order)?;
            } else if message == 0x49 && handler == 0x489ace {
                crate::ai_behaviour_battle::filtered_detection(m, unit, target)?;
            } else if message == 0x4c && handler == 0x489c36 {
                crate::ai_behaviour_battle::structure_detection(m, unit, target)?;
            } else if message == 0x4a && handler == 0x489d6c {
                crate::ai_behaviour_battle::ended(m, unit)?;
            } else if message == 0x4b && handler == 0x4897c9 {
                crate::ai_behaviour_battle::hunger(m, unit)?;
            } else {
                return Err(stop("iagUnitSendMessage behaviour handler", handler));
            }
        }
    }
    if message == 0x4a {
        crate::chain_battle::ended_notification(m, unit)?;
    }
    Ok(0)
}

/// Additive isolated validation entry identifiers; argument counts are part of ABI 1.1.
pub fn isolated(id: u32) -> Option<(&'static str, usize)> {
    match id {
        100 => Some(("MessageBoxCalcul", 0)),
        101 => Some(("ProcessInputGame", 1)),
        102 => Some(("AvanceMonstre", 0)),
        103 => Some(("iaSolver", 1)),
        104 => Some(("CalculNextReflect", 2)),
        105 => Some(("iagUnitSendMessage", 3)),
        400 => Some(("iaGroupNew", 2)),
        401 => Some(("iagUnitAddToGroup", 2)),
        402 => Some(("iagUnitGetFirstUnitInGroup", 1)),
        403 => Some(("iagUnitGetNextUnitInGroup", 0)),
        404 => Some(("StockeAction", 4)),
        405 => Some(("ConstruitGroupeMonstre", 2)),
        406 => Some(("iaProbNew", 1)),
        407 => Some(("iaProbPost", 8)),
        408 => Some(("iaProbSleep", 2)),
        409 => Some(("RaidRule", 10)),
        _ => None,
    }
}

/// iagUnitSendMessage message 0x43 (0x482b60..0x482c4b): the unit joins the
/// head of its owner's reserve group (index at 0x51b68c + 96 * owner; links
/// +0x108/+0x10c, head +0x10, count +0x12) when that group record exists, then
/// the owner's group list (head 0x51b6b4 + 96 * owner, next link at group + 4)
/// is searched for a group whose unit kind byte (+0x1a) equals the unit's
/// kind: problem 0x17 is posted when one exists, problem 9 otherwise, for the
/// unit's player (EnginGetPlayer) with EBX = unit, ECX = 0 and four zero stack
/// arguments.
fn reserve_message(m: &mut MemoriaJogo, unit: u16, record: u32) -> Result<u32> {
    let owner = u32::from(m.u8(record)?);
    let reserve = m.u32(0x51b68cu32.wrapping_add(owner.wrapping_mul(96)))?;
    let g = 0x51a9e0u32.wrapping_add(reserve.wrapping_shl(5));
    if g != 0 {
        // Port deviation: unlink from the recorded group first (frame_raid::detach).
        crate::frame_raid::detach(m, unit)?;
        m.set_u16(record + 0x108, m.u16(g + 0x10)?)?;
        m.set_u32(record + 0x10c, reserve)?;
        let count = m.u16(g + 0x12)?;
        m.set_u16(g + 0x10, unit)?;
        m.set_u16(g + 0x12, count.wrapping_add(1))?;
    }
    let kind = m.u8(record + 1)?;
    let mut index = m.u32(0x51b6b4u32.wrapping_add(owner.wrapping_mul(96)))?;
    while index != u32::MAX {
        let a = 0x51a9e0u32.wrapping_add(index.wrapping_shl(5));
        if m.u8(a + 0x1a)? == kind {
            break;
        }
        index = m.u32(a + 4)?;
    }
    let problem = if index != u32::MAX { 0x17 } else { 9 };
    let player = crate::frame_core::call(m, "EnginGetPlayer", &[u32::from(unit)])? & 255;
    // iaProbPost stores EBX (the unit) at +0x18, ECX (0) at +0x1c, then the four
    // zero stack arguments.
    crate::frame_tasks::post(m, &[player, problem, u32::from(unit), 0, 0, 0, 0, 0])?;
    Ok(0)
}
