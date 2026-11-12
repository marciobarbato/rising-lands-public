//! Original-width unit and structure frame dispatch, from the accepted Python chain.
use crate::{EngineError, MemoriaJogo, Result};

fn stop(name: &'static str, address: u32) -> EngineError {
    EngineError::UnportedCallee { name, address }
}
fn record(m: &MemoriaJogo, unit: u32) -> Result<u32> {
    Ok(m.u32(0x4e19a0)?.wrapping_add(276 * u32::from(unit as u16)))
}
fn economy(m: &mut MemoriaJogo, name: &str, args: &[u32]) -> Result<u32> {
    crate::frame_economy::call(m, name, args)
}
fn core(m: &mut MemoriaJogo, name: &str, args: &[u32]) -> Result<u32> {
    crate::frame_core::call(m, name, args)
}

pub fn call(m: &mut MemoriaJogo, name: &str, args: &[u32]) -> Result<u32> {
    match name {
        "CalculEnginNextEtape" => next_units(m),
        "CalculBatimentNextEtape" => next_structures(m),
        "EnginNextEtape" => next_unit(m, args[0] as u16),
        "BatimentNextEtape" => next_structure(m, args[0] as u16),
        "EnginTrouveCible" => find_target(m, args[0] as u16),
        "IATestUtilite" => {
            if args[0] as u8 == 1 {
                test_message(m, args[1] as u16)?;
            }
            Ok(0)
        }
        "iagMessTestSendMessageEngin" => {
            test_message(m, args[0] as u16)?;
            Ok(0)
        }
        "EnginOccupeCase" => occupies(m, args[0] as u16, args[1] as u16, args[2] as u8),
        _ => Err(stop("unknown unit frame provider", 0x45442c)),
    }
}
fn test_message(m: &mut MemoriaJogo, unit: u16) -> Result<()> {
    let r = record(m, u32::from(unit))?;
    if m.u8(r + 2)? == m.u8(r + 0x10a)? && m.u8(r + 0x89)? == m.u8(r + 0x10b)? {
        return Ok(());
    }
    m.set_u8(r + 0x10a, m.u8(r + 2)?)?;
    m.set_u8(r + 0x10b, m.u8(r + 0x89)?)?;
    let owner = m.u8(r)?;
    if owner >= 4
        || economy(m, "PlayerGetType", &[u32::from(owner)])? as u8 != 2
        || m.u32(r + 0x10c)? == u32::MAX
    {
        return Ok(());
    }
    crate::ai_group_message::continue_descriptor_with_city(m, unit, crate::ai_war_enemy::descriptor)
}
fn next_units(m: &mut MemoriaJogo) -> Result<u32> {
    m.set_u16(0x4e1a38, 0)?;
    m.set_u16(0x4e1a3a, 0)?;
    for owner in 0..4 {
        for kind in 0..26 {
            m.set_u16(0x4e1a3c + owner * 0x34 + kind * 2, 0)?;
        }
        m.set_u16(0x4e1928 + owner * 2, 0)?;
    }
    m.set_u16(0x4e1930, 0)?;
    for i in 0..800 {
        let r = record(m, i)?;
        let state = m.u8(r + 2)?;
        if state == 0 {
            continue;
        }
        if state != 4 {
            let owner = m.u8(r)?;
            let kind = m.u8(r + 1)?;
            if owner != 200 && owner != 4 {
                let a = 0x4e1a3c + u32::from(owner) * 0x34 + u32::from(kind) * 2;
                m.set_u16(a, m.u16(a)?.wrapping_add(1))?;
            }
            test_message(m, i as u16)?;
            if kind != 17 {
                let a = if owner == m.u8(0x508041)? {
                    0x4e1a3a
                } else {
                    0x4e1a38
                };
                m.set_u16(a, m.u16(a)?.wrapping_add(1))?;
            }
            if owner != 200 && kind != 5 {
                let a = 0x4e1928 + u32::from(owner) * 2;
                m.set_u16(a, m.u16(a)?.wrapping_add(1))?;
            }
        }
        next_unit(m, i as u16)?;
    }
    Ok(0)
}
fn next_structures(m: &mut MemoriaJogo) -> Result<u32> {
    economy(m, "ResetTableauHangar", &[])?;
    let period = m.u16(0x495c02)?;
    if period == 0 {
        return Err(stop("CalculBatimentNextEtape divide by zero", 0x4281e5));
    }
    if m.u32(0x508f7c)? % u32::from(period) == 0 {
        m.set_u8(0x4dfb9b, 0)?;
    }
    for owner in 0..4 {
        m.set_u32(0x4dfaa8 + 4 * owner, 0)?;
    }
    for i in 0..1000 {
        let r = 0x49a0c8 + 276 * i;
        if m.u8(r + 5)? == 0 {
            continue;
        }
        let owner = m.u8(r + 4)?;
        let kind = m.u8(r + 8)?;
        if owner < 4 && ![9, 12, 17].contains(&kind) {
            let a = 0x4dfaa8 + 4 * u32::from(owner);
            m.set_u32(a, m.u32(a)?.wrapping_add(1))?;
        }
        if m.u8(r + 5)? == 3 {
            crate::building_dismantle::dismantle(m, i as u16)?;
            continue;
        }
        next_structure(m, i as u16)?;
        let cell = core(m, "BatimentGetCase", &[i])?;
        for player in 0..4 {
            if u32::from(m.u8(r + 4)?) == player {
                continue;
            }
            if core(m, "CaseEstDecouverte", &[cell & 0xffff, player])? as u8 != 0 {
                continue;
            }
            if core(m, "BatimentEstDecouvert", &[i, player])? as u8 == 0 {
                continue;
            }
            core(m, "BatimentDecouvre", &[i, player])?;
        }
        for player in 0..4 {
            if u32::from(m.u8(r + 4)?) == player {
                continue;
            }
            let flag = r + 0x110 + player;
            if core(m, "BatimentContourEstIllumine", &[i, player])? as u8 != 0 {
                if m.u8(flag)? == 0 {
                    core(m, "BatimentIllumine", &[i, player, 1])?;
                    m.set_u8(flag, 1)?;
                }
            } else if m.u8(flag)? != 0 {
                core(m, "BatimentIllumine", &[i, player, 0])?;
                m.set_u8(flag, 0)?;
            }
        }
    }
    m.set_u8(0x4dfb50, 0)?;
    Ok(0)
}
fn next_unit(m: &mut MemoriaJogo, unit: u16) -> Result<u32> {
    let n = u32::from(unit);
    let r = record(m, n)?;
    let mut owner = 0;
    let mut kind = 0;
    if m.u8(r + 2)? != 0 {
        owner = m.u8(r)?;
        kind = m.u8(r + 1)?;
        if m.u32(r + 0x104)? != u32::MAX {
            return Err(stop("EnginNextEtape sound handle", 0x454479));
        }
        m.set_u8(r + 0xf9, 0)?;
    }
    let state = m.u8(record(m, n)? + 2)?;
    if state == 0 || state == 4 {
        return switch_state(m, unit, r);
    }
    if m.u8(r + 1)? == 0x10 {
        // 0x4544d7: +0xa9 counts up to +0xa7, then the phase +0xa4 advances
        // modulo the count +0xa5 (idiv: a zero count faults in the original)
        // and +0xa9 restarts at 0.
        let tick = m.u8(r + 0xa9)?.wrapping_add(1);
        m.set_u8(r + 0xa9, tick)?;
        if tick == m.u8(r + 0xa7)? {
            let count = u32::from(m.u8(r + 0xa5)?);
            if count == 0 {
                return Err(stop(
                    "EnginNextEtape type-16 zero animation count",
                    0x454521,
                ));
            }
            let phase = (u32::from(m.u8(r + 0xa4)?) + 1) % count;
            m.set_u8(r + 0xa9, 0)?;
            m.set_u8(r + 0xa4, phase as u8)?;
        }
    }
    reload(m, unit)?;
    if matches!(kind, 20..=24) || (11..=15).contains(&kind) {
        crate::stage_unit_progression::after_reload(m, r, kind)?;
    } else if kind == 0x10 && m.u32(r + 0x104)? == u32::MAX {
        // 0x454602..0x454680: with the sound handle free, a type-16 unit on a
        // cell TerrainEstDecouvert reports for the local player posts sound
        // 0x50 with the kind in the high word (EnginGetVolume is read-only;
        // its result is the stack argument outside the typed request). The
        // step flag +0xf9 is set either way.
        let local = u32::from(m.u8(0x508041)?);
        let cell = u32::from(m.u16(r + 8)?);
        if m.u8(m.u32(0x507d78 + 4 * local)?.wrapping_add(cell))? & 31 == 0 {
            m.set_u8(r + 0xf9, 1)?;
            crate::player_audio::request(m, true, [0x50, (0x10 << 16) | 0x800, r + 0x104, 1])?;
        }
        m.set_u8(r + 0xf9, 1)?;
    }
    // Kinds 0x11 and above (the type-17 balloon included) take 0x4545a7 ->
    // 0x4545f3 -> 0x454687 with no effect: straight into the periodic tail.

    next_unit_tail(m, unit, r, owner, kind)
}
/// EnginNextEtape0x454530..0x454591, signed threshold and wrapping u16 store.
pub fn reload(m: &mut MemoriaJogo, unit: u16) -> Result<()> {
    let rr = record(m, u32::from(unit))?;
    let current = m.u8(rr + 1)?;
    let limit = m.u16(0x4965de + 50 * u32::from(current))?;
    let current_owner = m.u8(rr)?;
    let bonus = economy(
        m,
        "BonusGetValEngin",
        &[u32::from(current_owner), u32::from(current), 3, rr],
    )? as u16;
    if i32::from(m.u16(rr + 0x102)?) < i32::from(limit) - i32::from(bonus) {
        m.set_u16(rr + 0x102, m.u16(rr + 0x102)?.wrapping_add(1))?;
    }
    Ok(())
}
/// Native periodic acquisition block0x454687..0x4547e0, before fatigue.
pub(crate) fn automatic_attack(
    m: &mut MemoriaJogo,
    unit: u16,
    r: u32,
    owner: u8,
    kind: u8,
) -> Result<()> {
    let n = u32::from(unit);
    if m.u8(r + 2)? != 14 && m.u32(0x508f7c)?.wrapping_add(n) & 31 == 0 {
        let rr = record(m, n)?;
        let current = m.u8(rr + 1)?;
        if owner != 200
            && [
                0, 1, 2, 3, 8, 11, 12, 15, 16, 18, 19, 20, 21, 22, 23, 24, 25,
            ]
            .contains(&current)
        {
            let player_type = economy(m, "PlayerGetType", &[u32::from(owner)])? as u8;
            if ![1, 19].contains(&m.u8(r + 2)?)
                && (![14, 16].contains(&kind) || m.u8(record(m, n)? + 0xda)? != 0)
            {
                let target = find_target(m, unit)? as u16;
                if target != 0xffff {
                    if player_type == 2 {
                        crate::frame_ai::call(
                            m,
                            "iagUnitSendMessage",
                            &[n, 0x49, u32::from(target)],
                        )?;
                    } else if m.u8(r + 2)? == 7 {
                        crate::player_orders::call(
                            m,
                            "SetAttaqueEngin",
                            [n, 1, u32::from(target), 0],
                            0x4547db,
                        )?;
                    }
                }
            }
        }
    }
    Ok(())
}
fn next_unit_tail(m: &mut MemoriaJogo, unit: u16, r: u32, owner: u8, kind: u8) -> Result<u32> {
    let n = u32::from(unit);
    automatic_attack(m, unit, r, owner, kind)?;
    if m.u8(record(m, n)? + 0xdc)? & 4 != 0 {
        // 0x454811: a unit with status bit 2 (an ESPELHO copy) loses one hit
        // point whenever the frame counter is a multiple of
        // SortilegeGetTempsFin(2); at zero it dies (SetDyingEngin) and the step ends.
        let period = crate::spell_queries::entry(
            m,
            crate::spell_queries::GET_TEMPS_FIN,
            crate::FrameRegisters {
                eax: 2,
                ..Default::default()
            },
        )?
        .eax & 65535;
        if period == 0 {
            return Err(stop("EnginNextEtape spell timer zero divisor", 0x454827));
        }
        if m.u32(0x508f7c)? % period == 0 {
            let hp = m.u16(r + 0x9a)?.wrapping_sub(1);
            m.set_u16(r + 0x9a, hp)?;
            if hp == 0 {
                crate::chain_death::set(m, unit)?;
                return Ok(0x454ccd);
            }
        }
    }
    let t = m.u8(r + 1)?;
    if t > 25 {
        return Err(stop("EnginNextEtape fatigue type > 25", 0x454869));
    }
    let target = m.u32(0x454344 + 4 * u32::from(t))?;
    let fatigue = match target {
        0x454851 => true,
        0x454869 => false,
        _ => return Err(stop("EnginNextEtape fatigue table", target)),
    };
    if fatigue && m.u8(record(m, n)? + 0xdc)? & 4 == 0 {
        core(m, "CalculFatigue", &[n])?;
        if [0, 4].contains(&m.u8(record(m, n)? + 2)?) {
            return Ok(0x454ccd);
        }
        if m.u16(r + 4)? != 4 {
            let rr = record(m, n)?;
            if ![0, 4].contains(&m.u8(rr + 2)?)
                && m.u8(rr + 0x72)? != 0
                && m.u16(rr + 4)? == 0
                && m.u8(r + 0x86)? == 0
                && m.u8(r + 0x87)? == 0
            {
                let owner = m.u8(rr)?;
                if economy(m, "PlayerGetType", &[u32::from(owner)])? as u8 == 2 {
                    crate::frame_ai::call(m, "iagUnitSendMessage", &[n, 0x4b, 0])?;
                } else {
                    crate::chain_fatigue::set(m, n)?;
                }
            }
        }
    }
    if m.u8(record(m, n)? + 1)? == 17 {
        balloon_step(m, unit)?;
    }
    if m.u8(record(m, n)? + 0x72)? != 0 && m.u8(r + 2)? != 4 {
        let cell = m.u16(r + 8)?;
        core(m, "SetEnginCollision", &[n, u32::from(cell)])?;
    }
    // 0x454ba5 / 0x454c0e: status bits 1 (PETRIFICAÇÃO) and 3 (ARMADURA) count
    // down their spell slot (+0xe0 + 4 * bit, a dword decremented whole, its
    // low word tested) and clear the bit at zero. A petrified unit always ends
    // the step here; armour continues.
    for (bit, ends) in [(1u32, true), (3u32, false)] {
        let rr = record(m, n)?;
        if m.u8(rr + 0xdc)? & (1 << bit) == 0 {
            continue;
        }
        let slot = rr + 0xe0 + 4 * bit;
        let left = m.u32(slot)?.wrapping_sub(1);
        m.set_u32(slot, left)?;
        if left & 65535 == 0 {
            m.set_u8(rr + 0xdc, m.u8(rr + 0xdc)? & !(1u8 << bit))?;
        }
        if ends {
            return Ok(0x454ccd);
        }
    }
    // 0x454c4f..0x454c91: every kind reaches the rotation gate; only kinds 12
    // and 15 rotate (RotateTourEngin), the others continue to the state switch.
    if m.u8(record(m, n)? + 1)? >= 11 {
        crate::stage_unit_progression::before_state(m, unit)?;
    }
    switch_state(m, unit, r)
}
/// EnginNextEtape 0x4549c2..0x454b3c: the type-17 balloon of a player whose
/// balloon count (0x4e1a14 + 8p) is still zero. A live balloon takes state 30
/// (selection released), is sent to the recorded column (0x4e1a1a + 8p) and,
/// once there or on row 0, is retired (RetireEngin, state 0, selection
/// released) and the player's balloon unit slot (0x4e1a18 + 8p) is cleared.
pub(crate) fn balloon_step(m: &mut MemoriaJogo, unit: u16) -> Result<()> {
    let n = u32::from(unit);
    let r = record(m, n)?;
    let p = u32::from(m.u8(r)?);
    if m.u16(0x4e1a14 + 8 * p)? != 0 {
        return Ok(());
    }
    if ![0, 4].contains(&m.u8(r + 2)?) {
        m.set_u8(r + 2, 30)?;
        release_selection(m, unit)?;
    }
    let dest = m.u16(0x4e1a1a + 8 * p)?;
    crate::frame_spawn::call(m, "SetDeplaceEngin", &[n, u32::from(dest)])?;
    let pos = m.u16(record(m, n)? + 0x7c)?;
    if pos == dest || pos >> 8 == 0 {
        crate::chain_entry::retire(m, unit)?;
        let rr = record(m, n)?;
        if ![0, 4].contains(&m.u8(rr + 2)?) {
            m.set_u8(rr + 2, 0)?;
            release_selection(m, unit)?;
        }
        m.set_u16(0x4e1a18 + 8 * p, 0xffff)?;
    }
    Ok(())
}
/// 0x454a2b / 0x454b0a: selection release in the balloon branch's store order.
fn release_selection(m: &mut MemoriaJogo, unit: u16) -> Result<()> {
    if unit == m.u16(0x496304)? {
        m.set_u8(0x4e0f36, 0)?;
        m.set_u8(0x4e0f37, 0)?;
        m.set_u16(0x496304, 65535)?;
        m.set_u32(0x4e0f3c, 0)?;
    }
    Ok(())
}
/// EnginNextEtape state 5 (0x456600): the work structure check with its
/// destroyed/foreign branch 0x456639, then CalculWorkEngin.
pub fn work_state(m: &mut MemoriaJogo, unit: u16) -> Result<()> {
    let r = record(m, u32::from(unit))?;
    if m.u8(r + 0x82)? == 2 {
        let a = 0x49a0c8 + 276 * u32::from(m.u16(r + 0x80)?);
        if m.u8(a + 5)? == 3 || m.u8(a + 4)? != m.u8(r)? {
            // 0x456639: the work structure is destroyed or foreign. An
            // off-map unit is placed back (MineEndWork for type 7,
            // SetEnginCollision, discovery/illumination unless neutral
            // or player 4, SetMiniMap 4), then the depot is reset.
            if m.u8(r + 0x72)? == 0 {
                if m.u8(r + 1)? == 7 {
                    crate::chain_work::mine_call(m, "MineEndWork", u32::from(m.u16(r + 0xb2)?))?;
                }
                let cell = m.u16(r + 8)?;
                crate::frame_core::call(
                    m,
                    "SetEnginCollision",
                    &[u32::from(unit), u32::from(cell)],
                )?;
                m.set_u8(r + 0x72, 1)?;
                let owner = m.u8(r)?;
                if !matches!(owner, 200 | 4) {
                    let sight = u32::from(m.u8(r + 0xbb)?) + u32::from(m.u8(r + 0xf5)?);
                    crate::player_sight::area(m, cell, (sight + 2) & 0xffff, owner, 0)?;
                    crate::player_sight::area(m, cell, sight & 0xffff, owner, 1)?;
                }
                crate::frame_spawn::minimap(m, owner, cell, 4)?;
            }
            crate::frame_spawn::call(
                m,
                "SetDeplaceEngin",
                &[u32::from(unit), u32::from(m.u16(r + 0x7e)?)],
            )?;
            m.set_u8(r + 0x82, 0)?;
            m.set_u16(r + 0x80, 0xffff)?;
            return Ok(());
        }
    }
    crate::chain_work::work(m, u32::from(unit))?;
    Ok(())
}
fn switch_state(m: &mut MemoriaJogo, unit: u16, r: u32) -> Result<u32> {
    let state = m.u8(r + 2)?;
    if state > 24 {
        return Ok(0x454ccd);
    }
    let target = m.u32(0x4543c8 + 4 * u32::from(state))?;
    match target {
        0x45652f if m.player.is_some() => {
            crate::chain_repair::state15(m, unit)?;
            Ok(0x454ccd)
        }
        0x455a61 if m.player.is_some() => {
            crate::unit_park::state3(m, unit)?;
            Ok(0x454ccd)
        }
        0x4568ad if m.player.is_some() => {
            crate::unit_park::state6(m, unit)?;
            Ok(0x454ccd)
        }
        0x455fe2 if m.player.is_some() => {
            crate::player_spy::state23(m, unit)?;
            Ok(0x454ccd)
        }
        0x456ae5 if m.player.is_some() => {
            crate::chain_death::state4(m, unit)?;
            Ok(0x454ccd)
        }
        0x454ccd => Ok(0x454ccd),
        0x4558b4 if m.player.is_some() => {
            crate::chain_entry::state21(m, unit, r)?;
            Ok(0x454ccd)
        }
        0x456520 if m.player.is_some() => {
            crate::player_orders::calcul_piege(m, unit)?;
            Ok(0x454ccd)
        }
        0x456886 if m.player.is_some() && m.u8(r + 2)? == 24 => {
            crate::spell_order_acceptance::attack_state24(m, unit, r)?;
            Ok(0x454ccd)
        }
        0x456886 if m.player.is_some() => {
            crate::chain_attack::state1(m, unit, r)?;
            Ok(0x454ccd)
        }
        0x455b28 if m.player.is_some() => {
            crate::transport_delivery_next::state20(m, unit)?;
            Ok(0x454ccd)
        }
        0x455d26 if m.player.is_some() => {
            crate::transport_delivery_next::state18(m, unit)?;
            Ok(0x454ccd)
        }
        0x455e32 if m.player.is_some() => {
            crate::chain_work::pickup(m, u32::from(unit))?;
            Ok(0x454ccd)
        }
        0x456966 => {
            economy(m, "EnginNextEtape:state7", &[u32::from(unit)])?;
            Ok(0x454ccd)
        }
        0x456600 if m.player.is_some() => {
            work_state(m, unit)?;
            Ok(0x454ccd)
        }
        0x4554db if m.player.is_some() => {
            crate::chain_restore::hunger(m, u32::from(unit))?;
            Ok(0x454ccd)
        }
        0x4558e4 if m.player.is_some() => {
            crate::chain_entry::state13(m, unit, r)?;
            Ok(0x454ccd)
        }
        0x4564ef if m.player.is_some() => crate::player_build::builder(m, u32::from(unit)),
        0x4563da => {
            arrival(m, unit)?;
            Ok(0x454ccd)
        }
        _ => Err(stop(
            if m.player.is_some() {
                const NAMES: [&str; 25] = [
                    "EnginNextEtape state 0",
                    "EnginNextEtape state 1",
                    "EnginNextEtape state 2",
                    "EnginNextEtape state 3",
                    "EnginNextEtape state 4",
                    "EnginNextEtape state 5",
                    "EnginNextEtape state 6",
                    "EnginNextEtape state 7",
                    "EnginNextEtape state 8",
                    "EnginNextEtape state 9",
                    "EnginNextEtape state 10",
                    "EnginNextEtape state 11",
                    "EnginNextEtape state 12",
                    "EnginNextEtape state 13",
                    "EnginNextEtape state 14",
                    "EnginNextEtape state 15",
                    "EnginNextEtape state 16",
                    "EnginNextEtape state 17",
                    "EnginNextEtape state 18",
                    "EnginNextEtape state 19",
                    "EnginNextEtape state 20",
                    "EnginNextEtape state 21",
                    "EnginNextEtape state 22",
                    "EnginNextEtape state 23",
                    "EnginNextEtape state 24",
                ];
                NAMES[usize::from(state)]
            } else {
                "EnginNextEtape state switch"
            },
            target,
        )),
    }
}
/// EnginNextEtape state2/3 body, 4563da..4564ea. EnginEnArret is
/// the native frame setter returning AL, not a test of the current animation.
pub(crate) fn arrival(m: &mut MemoriaJogo, unit: u16) -> Result<()> {
    let r = record(m, u32::from(unit))?;
    if m.u16(r + 0x7e)? != m.u16(r + 8)? {
        core(m, "DeplaceEngin", &[u32::from(unit)])?;
        return Ok(());
    }
    if m.u8(r + 0x91)? != m.u8(r + 0x90)? {
        crate::frame_movement::call(m, "RotateEngin", &[u32::from(unit)])?;
        return Ok(());
    }
    if crate::frame_spawn::en_arret(m, u32::from(unit))? as u8 == 0 {
        let divisor = u16::from(m.u8(r + 0xa5)?);
        if divisor == 0 {
            return Err(stop(
                "EnginNextEtape arrival animation divide by zero",
                0x45643a,
            ));
        }
        m.set_u8(r + 0xa4, ((u16::from(m.u8(r + 0xa4)?) + 1) % divisor) as u8)?;
        return Ok(());
    }
    if ![0, 4].contains(&m.u8(r + 2)?) {
        m.set_u8(r + 2, 7)?;
        if unit == m.u16(0x496304)? {
            m.set_u8(0x4e0f36, 0)?;
            m.set_u8(0x4e0f37, 0)?;
            m.set_u32(0x4e0f3c, 0)?;
            m.set_u16(0x496304, 65535)?;
        }
    }
    Ok(())
}
fn next_structure(m: &mut MemoriaJogo, structure: u16) -> Result<u32> {
    let kind = m.u8(0x49a0c8 + 276 * u32::from(structure) + 8)?;
    let index = kind.wrapping_sub(1);
    if index > 20 {
        return Ok(0);
    }
    let target = m.u32(0x426880 + 4 * u32::from(index))?;
    // The original type table explicitly selects BatimentNextEtape's RET
    // epilogue for types such as21. This is a native return, not a fallback.
    if target == 0x4281c0 {
        return Ok(0);
    }
    if target == 0x426c10 {
        crate::chain_population::step(m, structure)?;
        return Ok(0);
    }
    if target == 0x426aff {
        crate::player_cave::body(m, u32::from(structure))?;
        return Ok(0);
    }
    match target {
        0x426d61 => {
            crate::research_building::next(m, structure)?;
        }
        0x428095 => crate::building_loop_tails::next(m, structure)?,
        0x426e43 | 0x427549 => crate::turret_structure_chain::next(
            m,
            structure,
            crate::turret_structure_chain::Helpers {
                enemy_unit: crate::chain_battle::enemy_unit,
                angle: crate::chain_attack::angle,
            },
        )?,
        0x42803c => crate::hospital_structure_chain::next(m, structure)?,
        0x427bb7 => crate::building_gate::next(m, structure)?,
        0x427da4 => crate::building_type20::next(m, structure)?,
        0x427eec => crate::building_support::next_type18(m, structure)?,
        0x426a08 | 0x427d5d => crate::building_loop_completion::next(m, structure)?,
        0x42698d if kind == 5 => crate::building_loop_completion::next(m, structure)?,
        0x42698d | 0x427ed9 => crate::building_support::next(m, structure)?,
        0x426918 => {
            economy(m, "BatimentNextEtape:production", &[u32::from(structure)])?;
        }
        _ => {
            return Err(stop(
                if kind == 3 {
                    "BatimentNextEtape type 3"
                } else {
                    "BatimentNextEtape type switch"
                },
                target,
            ));
        }
    }
    Ok(0)
}
fn allied(m: &MemoriaJogo, a: u8, b: u8) -> Result<bool> {
    if a == b {
        return Ok(true);
    }
    if a > 4 || b > 4 {
        return Ok(false);
    }
    Ok(m.u8(m.u32(0x5080a8)?.wrapping_add(20 * u32::from(a) + 6))? & (1u8 << b) != 0)
}
fn attack_types(a: u8, b: u8) -> bool {
    ![10, 17].contains(&b) && (b != 16 || [2, 8].contains(&a))
}
fn distance(a: u16, b: u16) -> u32 {
    let dx = i32::from(a & 255) - i32::from(b & 255);
    let dy = i32::from(a >> 8) - i32::from(b >> 8);
    (dx * dx + dy * dy) as u32
}
fn find_target(m: &MemoriaJogo, unit: u16) -> Result<u32> {
    let r = record(m, u32::from(unit))?;
    let kind = m.u8(r + 1)?;
    let owner = m.u8(r)?;
    let cell = m.u16(r + 8)?;
    let radius = u32::from(m.u8(r + 0xbb)?);
    if [2, 8].contains(&kind) {
        for i in 0..800 {
            let t = record(m, i)?;
            if [0, 4].contains(&m.u8(t + 2)?) || m.u8(t + 0x72)? == 0 {
                continue;
            }
            let other = m.u8(t)?;
            let enemy = if owner != other && (owner == 4 || other == 4) {
                true
            } else {
                owner != 200 && other != 200 && owner != other && !allied(m, owner, other)?
            };
            if !enemy || other == 200 {
                continue;
            }
            if attack_types(kind, m.u8(t + 1)?) && distance(cell, m.u16(t + 8)?) <= radius * radius
            {
                return Ok(i);
            }
        }
    }
    let x = u32::from(cell & 255);
    let y = u32::from(cell >> 8);
    let mut best = 0xffff;
    let mut least = 0xffff;
    let grid = m.u32(0x4e0f38)?;
    for cy in y.saturating_sub(radius)..256.min(y + radius + 1) {
        for cx in x.saturating_sub(radius)..256.min(x + radius + 1) {
            let c = cx + cy * 256;
            let occupation = m.u16(grid.wrapping_add(2 * c))?;
            if occupation & 0xc000 != 0x4000 {
                continue;
            }
            let i = u32::from((occupation & 0x3ff8) >> 3);
            if i >= 800 {
                return Err(stop(
                    "EnginTrouveCible occupant outside unit table",
                    0x453f74,
                ));
            }
            let t = record(m, i)?;
            let other = m.u8(t)?;
            if other == 200 || allied(m, other, owner)? || !attack_types(kind, m.u8(t + 1)?) {
                continue;
            }
            let d = distance(cell, c as u16) & 0xffff;
            if d <= radius * radius && d < least {
                best = i;
                least = d;
            }
        }
    }
    Ok(best)
}
pub(crate) fn occupies(m: &MemoriaJogo, unit: u16, cell: u16, filter: u8) -> Result<u32> {
    let r = record(m, u32::from(unit))?;
    let layer = m.u8(r + 0xab)?;
    if (layer != 0 && filter == 1) || (layer == 0 && filter == 2) || m.u8(r + 2)? == 14 {
        return Ok(0);
    }
    let kind = m.u8(r + 1)?;
    if kind > 25 {
        return Err(stop("EnginOccupeCase type outside domain", 0x44246e));
    }
    let offsets: &[u32] = if [3, 11, 12, 13, 14, 15, 16, 25].contains(&kind) {
        &[0, 1, 0x101, 0x100]
    } else {
        &[0]
    };
    for offset in [8, 0x7c] {
        for &d in offsets {
            if i64::from(m.u16(r + offset)?) - i64::from(d) == i64::from(cell) {
                return Ok(1);
            }
        }
    }
    Ok(0)
}

/// Isolated unit/structure validation entries; no clock tick is advanced.
pub fn isolated(id: u32) -> Option<(&'static str, usize)> {
    match id {
        150 => Some(("CalculEnginNextEtape", 0)),
        151 => Some(("CalculBatimentNextEtape", 0)),
        152 => Some(("EnginNextEtape", 1)),
        153 => Some(("BatimentNextEtape", 1)),
        154 => Some(("EnginTrouveCible", 1)),
        155 => Some(("IATestUtilite", 2)),
        156 => Some(("iagMessTestSendMessageEngin", 1)),
        157 => Some(("EnginOccupeCase", 3)),
        _ => None,
    }
}
