//! Intact turret searches/validator and type10/11 structure continuation.
//! Scheduling remains caller-owned. The launch (0x427201/0x4272df and the
//! type-11 mirror 0x4278fa/0x4279da) is RechercheDecalageProjectile 0x425100 ->
//! ProjectileInit, discovery for the target's player and the local-player shot sound.
use crate::{frame::stop, player, FrameRegisters, MemoriaJogo, Result};

/// Proved helper implementations, passed in by the live dispatcher
/// (frame_units, type-table targets 0x426e43 / 0x427549). No host answers or
/// substitute targeting rules are accepted.
#[derive(Clone, Copy)]
pub struct Helpers {
    pub enemy_unit: fn(&MemoriaJogo, u16, u8) -> Result<bool>,
    pub angle: fn(u16, u16, u8) -> u8,
}
fn record(index: u16) -> u32 {
    player::structure(u32::from(index))
}
fn distance(a: u16, b: u16) -> u32 {
    let dx = u32::from((a & 255).abs_diff(b & 255));
    let dy = u32::from((a >> 8).abs_diff(b >> 8));
    dx * dx + dy * dy
}
fn occupied(m: &mut MemoriaJogo, r: u32) -> Result<bool> {
    Ok(crate::chain_bags::body(
        m,
        "BagContient",
        FrameRegisters {
            eax: r + 12,
            edx: 1,
            ebx: 9,
            ..Default::default()
        },
    )?
    .eax as u16
        != 0)
}
/// TourelleTrouveEngin423fb9: first eligible index, not nearest, no RNG.
pub fn find_unit(m: &mut MemoriaJogo, index: u16, h: Helpers) -> Result<u16> {
    let r = record(index);
    let owner = m.u8(r + 4)?;
    let origin = m.u16(r + 2)?;
    for i in 0..800u16 {
        if !player::alive(m, u32::from(i))? || !(h.enemy_unit)(m, i, owner)? {
            continue;
        }
        let u = player::unit(m, u32::from(i))?;
        if m.u8(u + 2)? == 14 {
            continue;
        }
        let kind = m.u8(u + 1)?;
        if matches!(kind, 10 | 17) || kind == 16 && m.u8(r + 8)? == 11 {
            continue;
        }
        let cell = crate::frame_spawn::call(m, "EnginGetCase", &[u32::from(i)])? as u16;
        let range = u32::from(m.u16(0x495c00)?);
        if distance(origin, cell) <= range * range {
            return Ok(i);
        }
    }
    Ok(u16::MAX)
}
/// TourelleTrouveBatiment424123: capacity-based habitation precedes state/owner.
pub fn find_building(m: &mut MemoriaJogo, index: u16) -> Result<u16> {
    let r = record(index);
    let owner = m.u8(r + 4)?;
    let origin = m.u16(r + 2)?;
    for i in 0..1000u16 {
        let b = record(i);
        let kind = m.u8(b + 8)?;
        let troop = if matches!(kind, 2 | 21) {
            crate::chain_bags::body(
                m,
                "BagContientTroupe",
                FrameRegisters {
                    eax: b + 12,
                    ..Default::default()
                },
            )?
            .eax as u8
                != 0
        } else {
            false
        };
        let inhabited = if kind == 21 {
            troop
        } else {
            troop || occupied(m, b)?
        };
        if !inhabited || m.u8(b + 5)? == 0 {
            continue;
        }
        let other = m.u8(b + 4)?;
        if other == 200
            || crate::frame_economy::call(
                m,
                "PlayerEstAllie",
                &[u32::from(other), u32::from(owner)],
            )? as u8
                != 0
            || kind == 17
        {
            continue;
        }
        let cell = crate::frame_core::building_cell(m, u32::from(i))? as u16;
        let range = u32::from(m.u16(0x495bfe)?);
        if distance(origin, cell) <= range * range {
            return Ok(i);
        }
    }
    Ok(u16::MAX)
}
/// CibleTourelleValide424624. Validation deliberately does not test alliances.
pub fn valid(m: &mut MemoriaJogo, index: u16) -> Result<bool> {
    let r = record(index);
    let target = m.u16(r + 0x86)?;
    if target == u16::MAX {
        return Ok(false);
    }
    let origin = m.u16(r + 2)?;
    let kind = m.u8(r + 8)?;
    let range = match kind {
        0 => 20,
        2 => 10,
        4 => 5,
        10 | 11 => 7,
        17 | 19 | 21 => 4,
        9 | 12 | 22..=255 => return stop("CibleTourelleValide incoming ECX", 0x42482c),
        _ => 8,
    };
    match m.u8(r + 0x7b)? {
        1 => {
            if !player::alive(m, u32::from(target))? {
                return Ok(false);
            }
            let u = player::unit(m, u32::from(target))?;
            if m.u8(u)? == 200 || m.u8(u + 2)? == 14 {
                return Ok(false);
            }
            let cell = crate::frame_spawn::call(m, "EnginGetCase", &[u32::from(target)])? as u16;
            Ok(distance(origin, cell) <= range * range)
        }
        3 => {
            if m.u8(record(target) + 5)? == 0 || m.u8(r + 4)? == 200 {
                return Ok(false);
            }
            let cell = crate::frame_core::building_cell(m, u32::from(target))? as u16;
            Ok(distance(origin, cell) <= 100)
        }
        _ => Ok(false),
    }
}
fn rotate(m: &mut MemoriaJogo, r: u32, tick: u32) -> Result<()> {
    let desired = m.u8(r + 0x8b)?;
    let current = m.u8(r + 0x8a)?;
    if desired != current && tick & 1 == 0 {
        let increasing = desired > current && i32::from(desired) - i32::from(current) < 16
            || current > desired && i32::from(current) - i32::from(desired) > 16;
        let angle = if increasing {
            (current % 32).wrapping_add(1)
        } else if current == 1 {
            32
        } else {
            current.wrapping_sub(1)
        };
        m.set_u8(r + 0x8a, angle)?;
    }
    Ok(())
}
/// Actual BatimentNextEtape type10/11 body up to the owned projectile caller.
pub fn next(m: &mut MemoriaJogo, index: u16, h: Helpers) -> Result<()> {
    let r = record(index);
    let kind = m.u8(r + 8)?;
    if !matches!(kind, 10 | 11) {
        return stop("Turret structure type", 0x426910);
    }
    if occupied(m, r)? {
        let tick = m.u32(0x508f7c)?;
        if !valid(m, index)? {
            m.set_u8(r + 0x76, 0)?;
            m.set_u8(r + 0x74, 0)?;
            m.set_u16(r + 0x78, 0)?;
            m.set_u16(r + 0x86, u16::MAX)?;
            let mut target = u16::MAX;
            if tick.wrapping_add(u32::from(index)) as u8 & 31 == 0 {
                target = find_unit(m, index, h)?;
                let mode = if target != u16::MAX {
                    1
                } else {
                    target = find_building(m, index)?;
                    3
                };
                if target != u16::MAX {
                    m.set_u16(r + 0x88, 0)?;
                    m.set_u8(r + 0x7b, mode)?;
                }
            }
            m.set_u16(r + 0x86, target)?;
        }
        let target = m.u16(r + 0x86)?;
        if target == u16::MAX {
            if kind == 10 && matches!(m.u8(r + 0x76)?, 1..=3) && tick % 5 == 0 {
                m.set_u8(r + 0x76, m.u8(r + 0x76)?.wrapping_add(1))?;
            }
            if crate::frame_core::random(m)? % 100 == 0 && m.u8(r + 0x8b)? == m.u8(r + 0x8a)? {
                let desired = ((crate::frame_core::random(m)? as u8) & 31).wrapping_add(1);
                m.set_u8(r + 0x8b, desired)?;
            }
        } else {
            let mode = m.u8(r + 0x7b)?;
            m.set_u8(r + 0x74, 1)?;
            if matches!(mode, 1 | 3) {
                let origin = m.u16(r + 2)?;
                // Existing proved AngleCalcul accepts byte coordinates. Preserve
                // the real extended-coordinate boundary instead of truncating it.
                if kind == 10 && origin < 256 {
                    return stop("AngleCalcul extended origin", 0x4796cb);
                }
                let from = if kind == 10 { origin - 256 } else { origin };
                let to = if mode == 1 {
                    m.u16(player::unit(m, u32::from(target))? + 8)?
                } else {
                    crate::chain_entry::central_offset(m, u32::from(target))? as u16
                };
                m.set_u8(r + 0x8b, (h.angle)(from, to, 32))?;
            }
            let period = u32::from(m.u16(if kind == 10 { 0x495bfc } else { 0x495bfa })?);
            if period == 0 {
                return stop(
                    "Turret firing division",
                    if kind == 10 { 0x4271b5 } else { 0x4278ae },
                );
            }
            if tick % period == 0 && m.u8(r + 0x8b)? == m.u8(r + 0x8a)? {
                m.set_u16(r + 0x78, 100)?;
                if matches!(mode, 1 | 3) && m.u8(r + 0x74)? != 0 {
                    m.set_u8(r + 0x76, 1)?;
                    fire(m, index, r, kind, mode, target)?;
                }
            } else {
                let timer = m.u16(r + 0x78)?;
                if timer != 0 && timer != u16::MAX {
                    if kind == 11 {
                        m.set_u8(r + 0x76, 0)?;
                    }
                    m.set_u16(r + 0x78, timer.wrapping_sub(1))?;
                }
                if kind == 10 && matches!(m.u8(r + 0x76)?, 1..=3) && tick % 5 == 0 {
                    m.set_u8(r + 0x76, m.u8(r + 0x76)?.wrapping_add(1))?;
                }
            }
            let alive = match mode {
                1 => player::alive(m, u32::from(target))?,
                3 => m.u8(record(target) + 5)? != 0,
                _ => true,
            };
            if !alive {
                m.set_u16(r + 0x86, u16::MAX)?;
                m.set_u8(r + 0x7b, 255)?;
                if kind == 10 {
                    m.set_u8(r + 0x76, 0)?;
                }
                m.set_u8(r + 0x74, 0)?;
            }
        }
        rotate(m, r, tick)?;
    }
    if kind == 10 && m.u8(r + 0x76)? == 4 {
        m.set_u8(r + 0x76, 0)?;
    }
    Ok(())
}
/// Muzzle offsets of RechercheDecalageProjectile 0x425100: jump table 0x425000
/// (type 11, projectile 0x14) indexed by angle - 1, entries (EDI, ESI) in pixels.
const MUZZLE_TYPE11: [(i16, i16); 32] = [
    (-11, 3),
    (-10, -2),
    (-9, -7),
    (-7, -11),
    (-2, -17),
    (2, -20),
    (9, -23),
    (17, -26),
    (24, -26),
    (32, -25),
    (38, -23),
    (45, -21),
    (50, -16),
    (53, -13),
    (58, -7),
    (58, -2),
    (59, 4),
    (59, 7),
    (58, 12),
    (54, 16),
    (49, 22),
    (44, 26),
    (38, 27),
    (31, 27),
    (25, 28),
    (18, 28),
    (10, 25),
    (5, 23),
    (0, 19),
    (-5, 16),
    (-8, 11),
    (-10, 8),
];
/// Jump table 0x425080 (type 10, projectile 0x15, aerial flag from the target type).
const MUZZLE_TYPE10: [(i16, i16); 32] = [
    (-9, -15),
    (-9, -24),
    (-7, -29),
    (-3, -35),
    (1, -37),
    (7, -41),
    (13, -43),
    (17, -43),
    (23, -43),
    (30, -43),
    (36, -42),
    (43, -40),
    (46, -37),
    (52, -33),
    (54, -29),
    (56, -24),
    (56, -19),
    (56, -12),
    (55, -8),
    (51, -4),
    (47, 0),
    (41, 3),
    (35, 5),
    (30, 6),
    (25, 6),
    (18, 6),
    (13, 5),
    (7, 2),
    (2, 0),
    (-2, -3),
    (-5, -7),
    (-9, -13),
];
/// RechercheDecalageProjectile 0x425100 for kinds 10/11: `angle` (+0x8a) picks the
/// muzzle offset (an angle outside 1..=32 is an explicit stop). Type 10 then asks EnginGetType/TypeEnginEstAerien 0x4422cf about the
/// target word (even for a structure target) and raises the projectile flag for
/// unit types 16/17. ProjectileInit(kind, structure cell, target cell, 3; structure,
/// 0, dx, dy, flag).
fn launch(
    m: &mut MemoriaJogo,
    index: u16,
    kind: u8,
    angle: u8,
    from: u16,
    to: u16,
    target: u16,
) -> Result<()> {
    let table = if kind == 11 {
        &MUZZLE_TYPE11
    } else {
        &MUZZLE_TYPE10
    };
    // Outside 1..=32 the original keeps the caller's EDI/ESI registers as the
    // offsets; no checkpoint turret fires with such an angle (the desired
    // angle is recomputed from the geometry every frame and the rotation keeps
    // 1..=32), so that path stays an explicit stop instead of a formula.
    let Some(&(dx, dy)) = table.get(usize::from(angle.wrapping_sub(1))) else {
        return stop(
            "RechercheDecalageProjectile angle outside the muzzle tables",
            if kind == 11 { 0x425129 } else { 0x425306 },
        );
    };
    let (projectile, flag) = if kind == 11 {
        (0x14, 0)
    } else {
        let t = crate::frame_core::call(m, "EnginGetType", &[u32::from(target)])? & 255;
        (0x15, u32::from(matches!(t, 16 | 17)))
    };
    crate::frame_combat::call(
        m,
        "ProjectileInit",
        &[
            projectile,
            u32::from(from),
            u32::from(to),
            3,
            u32::from(index),
            0,
            i32::from(dx) as u32,
            i32::from(dy) as u32,
            flag,
        ],
    )?;
    Ok(())
}
/// Shot tail 0x427201..0x4272a8 (type 10) and 0x4278fa..0x4279a3 (type 11): the
/// target cell (EnginGetCase for mode 1, BatimentGetCase for mode 3), the launch,
/// BatimentDecouvre for the target's player (EnginGetPlayer or the structure owner),
/// then for the local player's turret BatimentGetVolume and PlaySnd_PostSound 0x42
/// with the type in the high word, only while the sound handle +0x100 is free.
fn fire(m: &mut MemoriaJogo, index: u16, r: u32, kind: u8, mode: u8, target: u16) -> Result<()> {
    let to = if mode == 1 {
        crate::frame_spawn::call(m, "EnginGetCase", &[u32::from(target)])? as u16
    } else {
        crate::frame_core::call(m, "BatimentGetCase", &[u32::from(target)])? as u16
    };
    let angle = m.u8(r + 0x8a)?;
    let from = m.u16(r + 2)?;
    launch(m, index, kind, angle, from, to, target)?;
    let player = if mode == 1 {
        crate::frame_core::call(m, "EnginGetPlayer", &[u32::from(target)])? & 255
    } else {
        u32::from(m.u8(record(target) + 4)?)
    };
    crate::frame_core::call(m, "BatimentDecouvre", &[u32::from(index), player])?;
    if m.u8(r + 4)? == m.u8(0x508041)? {
        // BatimentGetVolume 0x41a0ab reads the viewport only; its result is the
        // stack volume of PlaySnd_PostSound (0x4272a3 type 10, 0x42799e type 11).
        let volume = crate::projectile_building_impact::volume(m, index)?;
        if m.u32(r + 0x100)? == u32::MAX {
            let site = if kind == 11 { 0x42799e } else { 0x4272a3 };
            crate::player::sound(
                m,
                site,
                true,
                [0x42, (u32::from(kind) << 16) | 0x2300, r + 0x100, 0],
                Some(volume),
            )?;
        }
    }
    Ok(())
}
