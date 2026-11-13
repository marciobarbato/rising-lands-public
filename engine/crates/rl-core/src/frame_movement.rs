//! DeplaceEngin/CalculNextCase/AvanceEngin with shared original grid projections.
use crate::{frame_core, frame_path, frame_spawn, EngineError, MemoriaJogo, Result};
fn stop(name: &'static str, address: u32) -> EngineError {
    EngineError::UnportedCallee { name, address }
}
fn unit(m: &MemoriaJogo, i: u32) -> Result<u32> {
    frame_spawn::unit(m, i)
}
fn quad(t: u8) -> bool {
    matches!(t, 3 | 11 | 12 | 13 | 14 | 15 | 16 | 25)
}
fn occupy(m: &mut MemoriaJogo, i: u32, cell: u16, insert: bool) -> Result<()> {
    let u = unit(m, i)?;
    let t = m.u8(u + 1)?;
    if t >= 26 {
        return Ok(());
    }
    let mask = if matches!(t, 16 | 17) { 2 } else { 1 };
    let deltas = if quad(t) {
        &[0u16, 1, 257, 256][..]
    } else {
        &[0u16][..]
    };
    for (sub, d) in deltas.iter().enumerate() {
        let c = cell.wrapping_sub(*d);
        let a = m.u32(0x507da8)?.wrapping_add(u32::from(c));
        let old = m.u8(a)?;
        m.set_u8(a, if insert { old | mask } else { old & !mask })?;
        if matches!(t, 16 | 17) {
            continue;
        }
        let tag = 0x4000
            | ((i as u16) << 3)
            | sub as u16
            | if frame_path::CLASSES[t as usize] != 1 {
                4
            } else {
                0
            };
        let a = m.u32(0x4e0f38)?.wrapping_add(2 * u32::from(c));
        let old = m.u16(a)?;
        if insert && old < 0x4000 {
            m.set_u16(u + 0xfa + 2 * sub as u32, old)?;
            m.set_u16(a, tag)?;
        } else if !insert && old == tag {
            m.set_u16(a, m.u16(u + 0xfa + 2 * sub as u32)?)?;
        }
    }
    Ok(())
}
fn occupies(m: &MemoriaJogo, i: u32, cell: u16, mask: u8) -> Result<bool> {
    let u = unit(m, i)?;
    let ab = m.u8(u + 0xab)?;
    if (ab != 0 && mask == 1) || (ab == 0 && mask == 2) || m.u8(u + 2)? == 14 {
        return Ok(false);
    }
    for base in [m.u16(u + 8)?, m.u16(u + 0x7c)?] {
        for d in if quad(m.u8(u + 1)?) {
            &[0i32, 1, 257, 256][..]
        } else {
            &[0i32][..]
        } {
            if i32::from(base) - d == i32::from(cell) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn passage(m: &MemoriaJogo, i: u32, cell: u16) -> Result<(bool, [u16; 4])> {
    let u = unit(m, i)?;
    let t = m.u8(u + 1)?;
    if t >= 26 {
        return Err(stop("EnginPeutPasser type", 0x448b9f));
    }
    let mask = 1 + u8::from(m.u8(u + 0xab)? != 0);
    if quad(t) && (cell & 255 == 0 || cell >> 8 == 0) {
        return Ok((false, [65535; 4]));
    }
    let mut allowed = true;
    // 0x442668 output list: one entry per eligible cell, 0xffff when empty.
    let mut crushed = [65535u16; 4];
    let mut next = 0;
    for d in if quad(t) {
        &[0u16, 1, 257, 256][..]
    } else {
        &[0u16][..]
    } {
        let c = cell.wrapping_sub(*d);
        if occupies(m, i, c, mask)?
            || m.u8(m.u32(0x507da8)?.wrapping_add(u32::from(c)))? & mask == 0
        {
            continue;
        }
        let mut candidate = 65535;
        for other in 0..800 {
            let q = unit(m, other)?;
            if matches!(m.u8(q + 2)?, 0 | 4) || m.u8(q + 0x72)? == 0 {
                continue;
            }
            if !occupies(m, other, c, 1 + u8::from(m.u8(q + 0xab)? != 0))? {
                continue;
            }
            if m.u8(q + 1)? == 16 && m.u8(q + 0xab)? != 50 {
                continue;
            }
            candidate = other;
            if m.u8(q + 0xab)? != 0 {
                break;
            }
        }
        crushed[next] = candidate as u16;
        next += 1;
        let crush = if matches!(t, 12 | 15) && candidate != 65535 {
            let q = unit(m, candidate)?;
            m.u8(q)? != m.u8(u)?
                && matches!(
                    m.u8(q + 1)?,
                    0 | 1 | 2 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 18 | 19 | 22
                )
        } else {
            false
        };
        if !crush {
            allowed = false;
        }
    }
    Ok((allowed, crushed))
}
fn register_step(
    m: &mut MemoriaJogo,
    i: u32,
    cell: u16,
    d: u8,
    crushed: [u16; 4],
    direct: bool,
) -> Result<()> {
    let u = unit(m, i)?;
    // 0x448dc2..0x448dfe: kinds 12 and 15 run over the units on the cells
    // they enter; the list stops at its first empty entry.
    if matches!(m.u8(u + 1)?, 12 | 15) {
        for other in crushed.into_iter().take_while(|v| *v != 65535) {
            crate::chain_death::set(m, other)?;
        }
    }
    occupy(m, i, m.u16(u + 0x7c)?, false)?;
    occupy(m, i, m.u16(u + 8)?, true)?;
    occupy(m, i, cell, true)?;
    // EnginNextCase 0x448e32 resets after collision, before step/heading.
    // EnginNextCaseChemin 0x448cd4 does not reset the timer here.
    if direct {
        m.set_u8(u + 0x8b, 0)?;
    }
    m.set_u16(u + 0x7c, cell)?;
    m.set_u8(u + 0x90, d)?;
    Ok(())
}
pub fn choose_direct(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    let u = unit(m, i)?;
    let origin = m.u16(u + 8)?;
    let dest = m.u16(u + 0x7e)?;
    let mut d = frame_path::direction(origin, dest, false);
    for attempt in 0..3 {
        let c = frame_path::neighbor(m, origin, d)?;
        if c != 65535 {
            let (ok, crushed) = passage(m, i, c)?;
            if ok {
                register_step(m, i, c, d, crushed, true)?;
                return Ok(1);
            }
            if c == dest {
                m.set_u16(u + 0x7e, m.u16(u + 0x7c)?)?;
                frame_spawn::release(m, i)?;
                m.set_u8(u + 0x88, 0)?;
                return Ok(0);
            }
        }
        d = if attempt == 0 {
            ((i32::from(d) - 3).rem_euclid(16) + 1) as u8
        } else {
            ((u32::from(d) + 1) % 16 + 1) as u8
        };
    }
    m.set_u8(u + 0x88, 3)?;
    m.set_u8(u + 0x8b, 0)?;
    Ok(0)
}
pub fn choose_route(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    let u = unit(m, i)?;
    let route = frame_path::route(m, i)?;
    let n = m.u16(route)?;
    if n == 0 {
        m.set_u8(u + 0x88, 0)?;
        m.set_u8(u + 0x8b, 0)?;
        return Ok(0);
    }
    let cell = m.u16(route.wrapping_add(2 * u32::from(n)))?;
    let (ok, crushed) = passage(m, i, cell)?;
    if !ok {
        m.set_u8(u + 0x88, 0)?;
        m.set_u8(u + 0x8b, 0)?;
        return Ok(0);
    }
    let d = frame_path::direction(m.u16(u + 8)?, cell, false);
    register_step(m, i, cell, d, crushed, false)?;
    Ok(1)
}
fn rotate(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    let u = unit(m, i)?;
    // RotateEngin 0x448599 works on raw bytes: no orientation range check.
    let desired = m.u8(u + 0x90)?;
    let current = m.u8(u + 0x91)?;
    if matches!(m.u8(u + 1)?, 12 | 15) {
        m.set_u8(
            u + 0xda,
            desired.wrapping_sub(1).wrapping_mul(2).wrapping_add(1),
        )?;
    }
    if desired != current {
        let counter = m.u8(u + 0x92)?.wrapping_add(1);
        m.set_u8(u + 0x92, counter)?;
        if counter == m.u8(u + 0x93)? {
            // 0x448613: clockwise (current % 16 + 1) when desired is ahead by
            // less than 8 or behind by more than 8, else counter-clockwise.
            let (d, c) = (i32::from(desired), i32::from(current));
            let forward = (d > c && d - c < 8) || (c > d && c - d > 8);
            let next = if forward {
                current % 16 + 1
            } else if current == 1 {
                16
            } else {
                current.wrapping_sub(1)
            };
            m.set_u8(u + 0x91, next)?;
            m.set_u8(u + 0x92, 0)?;
        }
    }
    Ok(0)
}
fn can_advance(m: &MemoriaJogo, u: u32) -> Result<bool> {
    let desired = m.u8(u + 0x90)?;
    let current = m.u8(u + 0x91)?;
    if !(1..=16).contains(&desired) || !(1..=16).contains(&current) {
        return Err(stop("CalculNextCase orientation", 0x4490fd));
    }
    Ok(matches!(
        (i32::from(current) - i32::from(desired)).rem_euclid(16),
        0 | 1 | 2 | 14 | 15
    ))
}
fn animation(m: &mut MemoriaJogo, i: u32) -> Result<()> {
    let u = unit(m, i)?;
    let t = m.u8(u + 1)?;
    if t == 17 {
        return balloon_animation(m, i);
    }
    if t == 16 || t > 25 {
        return Ok(());
    }
    let n = m.u8(u + 0xa9)?.wrapping_add(1);
    m.set_u8(u + 0xa9, n)?;
    if n == m.u8(u + 0xa7)? {
        let frames = m.u8(u + 0xa5)?;
        if frames == 0 {
            return Err(stop("CalculNextCase animation division", 0x449039));
        }
        m.set_u8(
            u + 0xa4,
            ((u16::from(m.u8(u + 0xa4)?) + 1) % u16::from(frames)) as u8,
        )?;
        m.set_u8(u + 0xa9, 0)?;
    }
    Ok(())
}
pub fn advance(m: &mut MemoriaJogo, i: u32, discount: u8) -> Result<u32> {
    let u = unit(m, i)?;
    let t = m.u8(u + 1)?;
    let owner = m.u8(u)?;
    let dest = m.u16(u + 0x7c)?;
    let step = m.u8(u + 0x98)?.wrapping_sub(discount);
    let (dx, dy) = frame_path::vector(m.u8(u + 0x90)?);
    m.set_u8(
        u + 0x86,
        (i32::from(m.u8(u + 0x86)?) + dx * i32::from(step)) as u8,
    )?;
    m.set_u8(
        u + 0x87,
        (i32::from(m.u8(u + 0x87)?) + dy * i32::from(step)) as u8,
    )?;
    let progress = m.u8(u + 0x97)?.wrapping_add(step);
    m.set_u8(u + 0x97, progress)?;
    let arrived = progress > 31;
    if arrived {
        m.set_u8(u + 0x86, 0)?;
        m.set_u8(u + 0x87, 0)?;
        m.set_u8(u + 0x97, 0)?;
        let current = m.u16(u + 8)?;
        occupy(m, i, current, false)?;
        occupy(m, i, dest, true)?;
        frame_spawn::minimap(m, owner, current, 5)?;
        if !matches!(owner, 4 | 200) {
            crate::player_sight::area(
                m,
                current,
                u32::from(m.u8(u + 0xbb)?) + u32::from(m.u8(u + 0xf5)?),
                owner,
                2,
            )?;
        }
        m.set_u16(u + 8, dest)?;
        if !matches!(owner, 4 | 200) {
            let radius = u32::from(m.u8(u + 0xbb)?) + u32::from(m.u8(u + 0xf5)?);
            crate::player_sight::area(m, dest, radius + 2, owner, 0)?;
            crate::player_sight::area(m, dest, radius, owner, 1)?;
        }
        frame_spawn::minimap(m, owner, dest, 4)?;
        if m.u8(m.u32(0x507d74)?.wrapping_add(u32::from(dest)))? == 0x46
            && m.u8(m.u32(0x507dac)?.wrapping_add(u32::from(dest)))? != owner
            && !matches!(t, 16 | 17)
        {
            // 0x448a29: the trap goes off, a random explosion, and the cell
            // loses its mine.
            crate::chain_damage::impact_mode(m, dest, 8, 0, false)?;
            let random = frame_core::call(m, "Random", &[0x16])?;
            let cell = u32::from(dest);
            crate::frame_combat::call(
                m,
                "ProjectileInit",
                &[6 + (random & 1), cell, cell, 8, 0, 0, 0, 0, 0],
            )?;
            let objects = m.u32(0x507d74)?;
            m.set_u8(objects.wrapping_add(cell), 0)?;
            let aux = m.u32(0x507dac)?;
            m.set_u8(aux.wrapping_add(cell), 0)?;
        }
        let route = frame_path::route(m, i)?;
        let n = m.u16(route)?;
        if n != 0 {
            m.set_u16(route, n - 1)?;
        }
    }
    // Native0x448ac6 checks the state after movement/collision/sight and then
    // calls the same CalculFatigue body as stationary units. Starvation may
    // consume bag food or kill the unit here, after the step has committed.
    if !matches!(m.u8(u + 2)?, 0 | 4) && matches!(m.u8(u + 1)?, 0..=10 | 25) {
        frame_core::fatigue(m, i)?;
    }
    Ok(u32::from(arrived))
}
pub fn next_case(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    let u = unit(m, i)?;
    if m.u8(u + 0x86)? == 0 && m.u8(u + 0x87)? == 0 {
        match m.u8(u + 0x88)? {
            0 => {
                choose_direct(m, i)?;
            }
            5 => {
                choose_route(m, i)?;
            }
            _ => {}
        }
    }
    if m.u16(u + 0x7c)? == m.u16(u + 8)? {
        return Ok(0);
    }
    animation(m, i)?;
    if m.u8(u + 0x91)? != m.u8(u + 0x90)? {
        rotate(m, i)?;
    }
    let local = m.u8(0x508041)?;
    if m.u8(u)? == local
        && (11..=16).contains(&m.u8(u + 1)?)
        && m.u8(m
            .u32(0x507d78 + 4 * u32::from(local))?
            .wrapping_add(u32::from(m.u16(u + 8)?)))?
            & 31
            == 0
    {
        // 0x4490c1..0x4490f8: the local player's war unit (kinds 11..=16,
        // jump table 0x448f38) stepping where TerrainEstDecouvert answers 1.
        // EnginGetVolume 0x440c5d (read-only: unit cell, viewport, selection
        // mark) is the stack volume of PlaySnd_PostSound. The step flag +0xf9
        // is set before the handle test; sound 0x50 with the kind in the high
        // word posts at 0x4490f8 only while the handle +0x104 is free.
        let volume = crate::player_orders::unit_volume(m, i)?;
        let handle = m.u32(u + 0x104)?;
        m.set_u8(u + 0xf9, 1)?;
        if handle == u32::MAX {
            let kind = u32::from(m.u8(u + 1)?);
            crate::player::sound(
                m,
                0x4490f8,
                true,
                [0x50, (kind << 16) | 0x800, u + 0x104, 1],
                Some(volume),
            )?;
        }
    }
    if can_advance(m, u)? {
        advance(m, i, 0)?;
    }
    Ok(0)
}
pub fn displace(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    if m.modern_paths() && crate::modern_path::handles(m, i)? {
        return crate::modern_path::displace(m, i);
    }
    let u = unit(m, i)?;
    let state = m.u8(u + 0x88)?;
    if matches!(state, 0 | 5) || (state == 3 && (m.u8(u + 0x86)? != 0 || m.u8(u + 0x87)? != 0)) {
        return next_case(m, i);
    }
    match state {
        1 => m.set_u8(u + 0x88, 0)?,
        3 => {
            if m.u8(0x4e0f36)? == 0 {
                frame_path::begin(m, i)?;
                m.set_u8(u + 0x88, 4)?;
            }
        }
        4 => {
            if m.u8(0x496302)? == 0 {
                return Err(stop("DeplaceEngin unfinished continuation", 0x4493c8));
            }
            let status = m.u8(0x4e0f4c)?;
            if m.u8(0x4e0f37)? != 0 {
                if m.u8(u + 2)? == 2 && status == 2 {
                    m.set_u16(u + 0x7e, m.u16(0x4e0f4a)?)?;
                    frame_spawn::release(m, i)?;
                    m.set_u8(u + 0x88, 0)?;
                }
                m.set_u8(u + 0x88, 5)?;
            } else if status == 8 {
                m.set_u8(u + 0x88, 6)?;
                m.set_u8(u + 0x8b, 0)?;
            } else {
                if !crate::chain_unblock::unblock(m, i as u16)? {
                    m.set_u16(u + 0x7e, m.u16(u + 0x7c)?)?;
                    frame_spawn::release(m, i)?;
                }
                m.set_u8(u + 0x88, 0)?;
            }
            frame_spawn::release(m, i)?;
        }
        6 => {
            let n = m.u8(u + 0x8b)?.wrapping_add(1);
            if n == 20 {
                m.set_u8(u + 0x88, 3)?;
                m.set_u8(u + 0x8b, 0)?;
            } else {
                m.set_u8(u + 0x88, 6)?;
                m.set_u8(u + 0x8b, n)?;
            }
        }
        _ => {}
    }
    Ok(0)
}
pub fn isolated(id: u32) -> Option<(&'static str, usize)> {
    Some(match id {
        450 => ("GetFreeCell", 2),
        451 => ("CreerEngin", 4),
        452 => ("EnginSetPlayer", 2),
        453 => ("PlaceEngin", 1),
        454 => ("EnginGetCase", 1),
        455 => ("GetRandCaseChamps", 0),
        456 => ("GetRandCaseTypeBatPlayer", 2),
        457 => ("Chemin_Accessible", 2),
        458 => ("SetDeplaceEngin", 2),
        459 => ("EnginEnArret", 1),
        460 => ("DeplaceEngin", 1),
        461 => ("CalculNextCase", 1),
        462 => ("AvanceEngin", 2),
        463 => ("EnginNextCase", 1),
        464 => ("EnginNextCaseChemin", 1),
        465 => ("CheminCalcul", 0),
        466 => ("OptimiseChemin", 2),
        467 => ("Chemin_Verouille", 7),
        468 => ("RotateEngin", 1),
        469 => ("SetMiniMap", 3),
        470 => ("EnginSetType", 4),
        471 => ("EnginSetTypeCaract", 1),
        472 => ("TestLigneOptimise", 4),
        473 => ("ResetEnginCollision", 2),
        474 => ("EnginPeutPasser", 2),
        475 => ("ChercheIndiceEngin", 1),
        476 => ("CalculDirection", 2),
        477 => ("CalculDirCase", 2),
        478 => ("Distance2d", 4),
        479 => ("ResetOrdre", 1),
        480 => ("EnginEstVivant", 1),
        481 => ("TerrainEnCollision", 2),
        482 => ("ResetCollision", 2),
        483 => ("memset", 3),
        484 => ("GetNumIndiceTypeBatiment", 3),
        485 => ("GetCaseIndiceBatiment", 1),
        486 => ("PlayerGetColor", 1),
        487 => ("EnginTypeGetCapacite", 1),
        _ => return None,
    })
}
pub fn call(m: &mut MemoriaJogo, name: &str, a: &[u32]) -> Result<u32> {
    match name {
        "TestLigneOptimise" => frame_path::test_line(m, a[0], a[1] as u16, a[2] as u16, a[3]),
        "ResetEnginCollision" => {
            occupy(m, a[0], a[1] as u16, false)?;
            Ok(0)
        }
        "EnginPeutPasser" => Ok(u32::from(passage(m, a[0], a[1] as u16)?.0)),
        "ChercheIndiceEngin" => {
            let cell = a[0] as u16;
            let mut out = 65535;
            for i in 0..800 {
                let u = unit(m, i)?;
                if matches!(m.u8(u + 2)?, 0 | 4) || m.u8(u + 0x72)? == 0 {
                    continue;
                }
                let ab = m.u8(u + 0xab)?;
                if !occupies(m, i, cell, 1 + u8::from(ab != 0))? {
                    continue;
                }
                if m.u8(u + 1)? == 16 && ab != 50 {
                    continue;
                }
                out = i;
                if ab != 0 {
                    break;
                }
            }
            Ok(out)
        }
        "CalculDirection" => Ok(u32::from(frame_path::direction(
            a[0] as u16,
            a[1] as u16,
            false,
        ))),
        "CalculDirCase" => Ok(u32::from(frame_path::neighbor(m, a[0] as u16, a[1] as u8)?)),
        "Distance2d" => {
            let dx = (a[0] & 65535).wrapping_sub(a[2] & 65535);
            let dy = (a[1] & 65535).wrapping_sub(a[3] & 65535);
            Ok(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)))
        }
        "DeplaceEngin" => displace(m, a[0]),
        "CalculNextCase" => next_case(m, a[0]),
        "AvanceEngin" => advance(m, a[0], a[1] as u8),
        "EnginNextCase" => choose_direct(m, a[0]),
        "EnginNextCaseChemin" => choose_route(m, a[0]),
        "CheminCalcul" | "OptimiseChemin" | "Chemin_Verouille" => frame_path::call(m, name, a),
        "RotateEngin" => rotate(m, a[0]),
        _ => frame_spawn::call(m, name, a),
    }
}
/// AnimBallonStatic 0x447b2f: the type-17 balloon's drift (+0x94 heading
/// nudges, Random 0x11 % 40 and 0x12 % 3), bob (+0x95/+0x96, Random 0x13 %
/// 10), altitude (+0xab, Random 0x14 % 5 while static, 0x15 % 10 otherwise;
/// +0xa4 frame while static) and the static switch (+0xae, +0xa4 reset,
/// sound 0x50 posted when the unit's cell is discovered by the current
/// player and the sound handle +0x104 is free); altitude 50 ends the static
/// phase. Every caller (CalculNextCase 0x448fb9, CalculEnginStatic 0x4530a6,
/// AnimDeplaceEngin 0x44810e, AnimRechargeEngin 0x447dc8, CalculAttaqueEngin
/// 0x452a61/0x452b8d, EnginNextEtape 0x45695c) passes the unit in EAX.
pub(crate) fn balloon_animation(m: &mut MemoriaJogo, i: u32) -> Result<()> {
    let r = unit(m, i)?;
    if m.u8(r + 0xae)? == 0 && frame_core::random(m)? % 40 == 0 {
        if m.u8(r + 0xab)? > 40 {
            match m.u8(r + 0x94)? {
                0 | 2 => m.set_u8(r + 0x94, 1)?,
                heading => {
                    let delta = (frame_core::random(m)? % 3).wrapping_sub(1);
                    m.set_u8(r + 0x94, heading.wrapping_add(delta as u8))?;
                }
            }
        } else if m.u8(r + 0x94)? != 0 {
            m.set_u8(r + 0x94, m.u8(r + 0x94)? - 1)?;
        }
    }
    if m.u8(r + 0xae)? == 0 && frame_core::random(m)? % 10 == 0 {
        if m.u8(r + 0xab)? <= 40 {
            m.set_u8(r + 0x95, 0)?;
        }
        let bob = m.u8(r + 0x96)?;
        if m.u8(r + 0x95)? == 0 && bob != 0 {
            m.set_u8(r + 0x96, bob - 1)?;
        } else if bob < 16 {
            m.set_u8(r + 0x96, bob + 1)?;
        }
        if m.u8(r + 0x96)? == 0 {
            m.set_u8(r + 0x95, 1)?;
        }
        if m.u8(r + 0x96)? == 15 {
            m.set_u8(r + 0x95, 0)?;
        }
    }
    if m.u8(r + 0xae)? != 0 {
        if frame_core::random(m)? % 5 == 0 {
            m.set_u8(r + 0xab, m.u8(r + 0xab)?.wrapping_add(1))?;
        }
        m.set_u8(r + 0xa4, m.u8(r + 0xa4)?.wrapping_add(1))?;
    } else if frame_core::random(m)? % 10 == 0 {
        m.set_u8(r + 0xab, m.u8(r + 0xab)?.wrapping_sub(1))?;
    }
    if m.u8(r + 0xae)? == 0 && m.u8(r + 0xab)? < 40 && m.u8(r + 0x96)? == 0 && m.u8(r + 0x94)? == 0
    {
        m.set_u8(r + 0xae, 1)?;
        m.set_u8(r + 0xa4, 0)?;
        let cell = u32::from(m.u16(r + 8)?);
        let player = u32::from(m.u8(0x508041)?);
        if frame_core::call(m, "TerrainEstDecouvert", &[cell, player])? as u8 == 0 {
            return Ok(());
        }
        // EnginGetVolume 0x440c5d / EnginEstSelectionne 0x472174 only read the
        // unit cell, the viewport and the selection mark; the volume is the
        // stack argument of PlaySnd_PostSound 0x447d72.
        let volume = crate::player_orders::unit_volume(m, i)?;
        let handle = m.u32(r + 0x104)?;
        m.set_u8(r + 0xf9, 0)?;
        if handle == u32::MAX {
            let kind = u32::from(m.u8(r + 1)?);
            crate::player::sound(
                m,
                0x447d72,
                true,
                [0x50, (kind << 16) | 0x800, r + 0x104, 0],
                Some(volume),
            )?;
        }
        return Ok(());
    }
    if m.u8(r + 0xab)? == 50 {
        m.set_u8(r + 0xae, 0)?;
    }
    Ok(())
}
