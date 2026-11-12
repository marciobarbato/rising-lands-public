//! Live selection and packet setters. Original records remain shared with navigation.
use crate::{
    frame::stop,
    player::{self, alive, selections, structure, unit, Memory},
    MemoriaJogo, Result,
};
const DECAL: [u32; 22] = [
    0x201, 0x300, 0x101, 0x202, 0x100, 0x202, 0x100, 0x300, 0x202, 0x100, 0, 0, 0x100, 0x200,
    0x300, 0x100, 0x100, 1, 0x201, 0x100, 0x100, 0x100,
];
pub fn construction_offset(t: u32) -> u32 {
    DECAL.get(t as usize).copied().unwrap_or(0)
}
fn selectable(m: &MemoriaJogo, u: u32) -> Result<bool> {
    let r = unit(m, u)?;
    let state = m.b(r + 2)?;
    Ok(state != 14
        && !(m.b(r + 1)? == 16 && state == 13 && m.b(r + 0xab)? != 50)
        && m.b(r + 0x72)? != 0)
}
fn append(m: &mut MemoriaJogo, u: u32, p: u32, conditional: bool) -> Result<()> {
    if conditional {
        let r = unit(m, u)?;
        if !alive(m, u)? || !selectable(m, u)? || m.b(r)? != p || m.b(r + 0xbd)? != 0 {
            return Ok(());
        }
    }
    let a = 0x508728 + ((p & 255) << 9);
    let n = m.w(a)?;
    if n >= 255 {
        return Ok(());
    }
    m.sw(a, n + 1)?;
    m.sw(a + 2 * (n + 1), u)?;
    m.sb(unit(m, u)? + 0xbd, 1)
}
fn retire(m: &mut MemoriaJogo, u: u32, p: u32) -> Result<u32> {
    let a = 0x508728 + ((p & 255) << 9);
    let n = m.w(a)?;
    let mut found = false;
    for i in 1..=n {
        if !found {
            if m.w(a + 2 * i)? == u {
                found = true
            }
            m.sb(unit(m, u)? + 0xbd, 0)?
        } else {
            m.sw(a + 2 * i - 2, m.w(a + 2 * i)?)?
        }
    }
    if found {
        m.sw(a, m.w(a)?.wrapping_sub(1))?
    }
    Ok(n + 1)
}
fn deselect(m: &mut MemoriaJogo, p: u32) -> Result<u32> {
    let mut eax = 0;
    for u in 0..800 {
        eax = u32::from(alive(m, u)?);
        if eax == 0 {
            continue;
        }
        let r = unit(m, u)?;
        eax = r;
        if m.b(r + 0xbd)? == 0 || m.b(r)? != p {
            continue;
        }
        m.sb(r + 0xbd, 0)?;
        eax = retire(m, u, m.b(r)?)?
    }
    Ok(eax)
}
pub fn volume(m: &MemoriaJogo, cell: u32) -> Result<u32> {
    let x = (m.w(0x508724)? >> 1) + m.w(0x508720)?;
    let y = (m.w(0x508726)? >> 1) + m.w(0x508722)?;
    let dx = x.wrapping_sub(cell & 255);
    let dy = y.wrapping_sub(cell >> 8);
    let d = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy)) as i32;
    Ok(if d <= 0x91 {
        256
    } else if d <= 0x12c {
        180
    } else if d < 0x258 {
        100
    } else {
        0
    })
}
pub fn unit_volume(m: &MemoriaJogo, u: u32) -> Result<u32> {
    let r = unit(m, u)?;
    let mut v = volume(m, m.w(r + 8)?)?;
    if !(m.b(r + 0xbd)? != 0 && m.b(r)? == m.b(0x508041)?) {
        v >>= 1
    }
    Ok(v)
}
pub fn ask(m: &mut MemoriaJogo, u: u32, mode: u32, flag: u32, ecx: u32) -> Result<u32> {
    let r = unit(m, u)?;
    let c = m.w(r + 8)?;
    let p = m.b(0x508041)?;
    if m.b(m.d(0x507d78 + 4 * p)? + c)? != 0 {
        return Ok(0);
    }
    let v = if mode == 16 { 256 } else { unit_volume(m, u)? };
    let h = m.d(r + 0x104)?;
    m.sb(r + 0xf9, flag)?;
    if h != u32::MAX {
        return Ok(v);
    }
    let _ = ecx;
    player::sound(
        m,
        0x440de7,
        true,
        [0x50, (m.b(r + 1)? << 16) | (mode << 8), r + 0x104, flag],
        Some(v),
    )
}
fn in_zone(m: &MemoriaJogo, u: u32, xa: u32, ya: u32, xb: u32, yb: u32) -> Result<bool> {
    let r = unit(m, u)?;
    let t = m.b(r + 1)?;
    if t > 25 {
        return Ok(false);
    }
    let c = m.w(r + 8)?;
    let ds: &[u32] = if matches!(t, 3 | 11 | 12 | 13 | 14 | 15 | 16 | 25) {
        &[0, 1, 0x101, 0x100]
    } else {
        &[0]
    };
    for d in ds {
        let c = c.wrapping_sub(*d) as i32;
        let x = (c % 256) as u16 as u32;
        let y = (c / 256) as u16 as u32;
        if xa <= x && x <= xb && ya <= y && y <= yb {
            return Ok(true);
        }
    }
    Ok(false)
}
fn select(m: &mut MemoriaJogo, p: u32, a: u32, b: u32) -> Result<u32> {
    let (xa, xb) = ((a & 255).min(b & 255), (a & 255).max(b & 255));
    let (ya, yb) = ((a >> 8).min(b >> 8), (a >> 8).max(b >> 8));
    let mut spoke = false;
    let mut eax = 0;
    for u in 0..800 {
        eax = u32::from(alive(m, u)?);
        if eax == 0 {
            continue;
        }
        let r = unit(m, u)?;
        if !selectable(m, u)? {
            continue;
        }
        if m.b(r)? != p {
            continue;
        }
        if m.b(r + 0xbd)? != 0 {
            m.sb(r + 0xbd, 0)?;
            retire(m, u, m.b(r)?)?;
        }
        if !in_zone(m, u, xa, ya, xb, yb)? {
            continue;
        }
        append(m, u, p, true)?;
        eax = u32::from(spoke);
        if spoke {
            continue;
        }
        let t = m.b(r + 1)?;
        if t == 10 || p != m.b(0x508041)? || (11..=16).contains(&t) {
            continue;
        }
        spoke = true;
        if t == 7 && matches!(m.b(r + 2)?, 11 | 15) {
            player::sound(m, 0x474582, false, [r + 0x104, 0x16, 0, 0], None)?;
            eax = ask(m, u, 0x16, 0, 0)?
        } else {
            eax = ask(m, u, 8, 0, 0)?
        }
    }
    Ok(eax)
}
pub fn resources(m: &MemoriaJogo, p: u32, food: u32, stone: u32, parts: u32) -> Result<bool> {
    let a = 0x4e2ea8 + 32 * (p & 255);
    Ok(food & 65535 <= m.w(a + 10)?
        && stone & 65535 <= m.w(a + 18)?
        && parts & 65535 <= m.w(a + 2)?)
}
pub fn mine_capacity(m: &MemoriaJogo, c: u32) -> Result<u32> {
    for i in 0..20 {
        let r = 0x509954 + 4 * i;
        let origin = m.w(r)?;
        if origin != 0 && [origin, origin + 1, origin + 256, origin + 257].contains(&(c & 65535)) {
            return crate::chain_queries::mine(m, i);
        }
    }
    stop("MineGetCapacite invalid index", 0x475541)
}
fn bag_has(m: &MemoriaJogo, a: u32, troop: bool) -> Result<bool> {
    for i in 0..m.w(a + 2)? {
        if m.b(a + 0x42 + i)? != 1 {
            continue;
        }
        let t = m.b(unit(m, m.w(a + 6 + 2 * i)?)? + 1)?;
        if if troop {
            !matches!(t, 3 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 25)
        } else {
            t == 9
        } {
            return Ok(true);
        }
    }
    Ok(false)
}
fn select_structure(m: &mut MemoriaJogo, s: u32) -> Result<u32> {
    let r = structure(s);
    let t = m.b(r + 8)?;
    let p = m.b(r + 4)?;
    if t == 12 {
        m.sw(0x495be0 + 2 * p, s)?;
        return Ok(p);
    }
    if m.b(r + 5)? == 0 {
        return Ok(0);
    }
    let mut ok = matches!(t, 21 | 2) && bag_has(m, r + 12, true)?;
    if !ok && t != 21 {
        ok = bag_has(m, r + 12, false)?
    }
    if !ok {
        return Ok(0);
    }
    m.sw(0x495be0 + 2 * p, s)?;
    if p != m.b(0x508041)? {
        return Ok(((s * 276) & !255) | p);
    }
    let v = volume(m, m.w(r + 2)?)?;
    if m.d(r + 0x100)? != u32::MAX {
        return Ok(v);
    }
    player::sound(
        m,
        0x41b785,
        true,
        [0x42, (t << 16) | 0x800, r + 0x100, 0],
        Some(v),
    )
}
pub fn build_order(m: &mut MemoriaJogo, u: u32, t: u32, cell: u32) -> Result<u32> {
    let r = unit(m, u)?;
    let (w, h) = (m.w(0x495d8c + 52 * t)?, m.w(0x495d8e + 52 * t)?);
    if m.b(r + 2)? == 11 && m.b(r + 0xbe)? == 9 {
        let n = m.w(r + 0xd8)?;
        if n < 10 {
            m.sw(r + 0xd8, n + 1)?;
            m.sw(r + 0xc4 + 2 * n, cell)?
        }
        return Ok(r);
    }
    crate::frame_spawn::reset_order(m, u)?;
    let c = if matches!(t, 9 | 12) {
        cell
    } else {
        cell.wrapping_add((h - (h >> 1)) << 8)
            .wrapping_sub(256)
            .wrapping_add(w - (w >> 1))
            .wrapping_sub(1)
            .wrapping_sub(DECAL.get(t as usize).copied().unwrap_or(0))
    };
    m.sw(r + 0x7e, c)?;
    crate::frame_spawn::release(m, u)?;
    m.sb(r + 0x88, 0)?;
    m.sb(r + 0x88, 0)?;
    m.sb(r + 0x89, 0)?;
    m.sw(r + 0xc0, 65535)?;
    m.sb(r + 0xbe, t)?;
    m.sb(r + 2, 11)?;
    m.sw(r + 0xb2, c)?;
    if t == 9 {
        m.sw(r + 0xd8, 0)?
    }
    Ok(t)
}
fn harvest_order(m: &mut MemoriaJogo, u: u32, c: u32) -> Result<u32> {
    let r = unit(m, u)?;
    crate::frame_spawn::reset_order(m, u)?;
    m.sw(r + 0x7e, c)?;
    crate::frame_spawn::release(m, u)?;
    m.sb(r + 0x88, 0)?;
    m.sb(r + 0x89, 0)?;
    m.sb(r + 2, 5)?;
    m.sb(r + 0x82, 0)?;
    m.sw(r + 0xb2, c)?;
    Ok(r)
}
fn transport_order(m: &mut MemoriaJogo, u: u32, c: u32) -> Result<u32> {
    let r = unit(m, u)?;
    m.sb(r + 2, 18)?;
    m.sb(r + 0x89, 0)?;
    m.sw(r + 0xb2, c)?;
    m.sw(r + 0x7e, c)?;
    crate::frame_spawn::release(m, u)?;
    m.sb(r + 0x88, 0)?;
    Ok(r)
}
pub fn posable(m: &mut MemoriaJogo, t: u32, c: u32, u: u32) -> Result<u32> {
    let (w, h) = (m.w(0x495d8c + 52 * t)?, m.w(0x495d8e + 52 * t)?);
    let (x, y) = (c & 255, (c & 65535) >> 8);
    let (hw, hh) = (w >> 1, h >> 1);
    m.sb(0x4e05b8, 1)?;
    for i in 0..w * h {
        m.sb(0x4e05b9 + i, 1)?
    }
    if hw > x || y < hh || w + x - hw - 1 > 256 || h + y - hh - 1 > 256 {
        for i in 0..w * h {
            m.sb(0x4e05b9 + i, 0)?
        }
        m.sb(0x4e05b8, 0)?;
        return Ok(0);
    }
    if t == 12 {
        let mut cell = c.wrapping_sub(hw).wrapping_sub(hh << 8);
        for row in 0..h {
            for col in 0..w {
                let c = cell & 65535;
                let valid = m.b(m.d(0x507d74)? + c)? == m.b(0x495bf8)?
                    && crate::player_build::gate(m, c)? != 255;
                if !valid {
                    m.sb(0x4e05b9 + row * w + col, 0)?;
                    m.sb(0x4e05b8, 0)?
                }
                cell = cell.wrapping_add(1)
            }
            cell = cell.wrapping_add(256).wrapping_sub(w)
        }
        return m.b(0x4e05b8);
    }
    let mut cell = c.wrapping_sub(hw).wrapping_sub(hh << 8);
    for row in 0..h {
        for col in 0..w {
            let c = cell & 65535;
            let tile = m.w(m.d(0x507d70)? + 2 * c)?;
            let mut valid = false;
            if (0x15d..0x1a5).contains(&tile) {
                let p = m.b(unit(m, u)?)?;
                if m.b(m.d(0x507d78 + 4 * p)? + c)? == 0 {
                    valid =
                        m.b(m.d(0x507da8)? + c)? & 1 == 0 || crate::player_build::occupies(m, u, c)?
                }
            }
            if !valid {
                m.sb(0x4e05b9 + row * w + col, 0)?;
                m.sb(0x4e05b8, 0)?
            }
            cell = cell.wrapping_add(1)
        }
        cell = cell.wrapping_add(256).wrapping_sub(w)
    }
    m.b(0x4e05b8)
}
fn attack(m: &mut MemoriaJogo, u: u32, mode: u32, target: u32) -> Result<u32> {
    // Combat attaque_vivo.definir_ataque: target order, not an executed attack.
    let r = unit(m, u)?;
    let same = m.b(r + 2)? == 1 && m.b(r + 0xb7)? == mode && m.w(r + 0xb8)? == target;
    if same {
        return Ok(1);
    }
    crate::frame_spawn::reset_order(m, u)?;
    let t = m.b(r + 1)?;
    if !matches!(t, 5 | 6 | 7 | 9 | 10 | 13)
        && matches!(mode, 1 | 3 | 5)
        && !(mode == 1 && u == target)
    {
        // Native mode1 resets the attack phase before publishing state1.
        // Building/ground modes3/5 publish state1 first (445acd).
        if mode == 1 {
            m.sb(r + 0x8a, 0)?;
            m.sb(r + 2, 1)?;
        } else {
            m.sb(r + 2, 1)?;
            m.sb(r + 0x8a, 0)?;
        }
        m.sb(r + 0xb7, mode)?;
        m.sw(r + 0xb8, target)?;
    }
    Ok(t)
}
pub fn call(m: &mut MemoriaJogo, name: &'static str, r: [u32; 4], site: u32) -> Result<u32> {
    let [a, d, b, c] = r;
    match name {
        "EnginGetCentreGroupe" => Ok(crate::chain_groups::centre(
            m,
            crate::FrameRegisters {
                eax: a,
                edx: d,
                ebx: b,
                ecx: c,
                ..Default::default()
            },
        )?
        .eax),
        "SelectionAddEngin" => {
            append(m, a & 65535, d & 255, false)?;
            Ok(0)
        }
        "RetireSelectionEngin" => retire(m, a & 65535, d & 255),
        "EnginSelectionnable" => Ok(u32::from(selectable(m, a & 65535)?)),
        "EnginInZone" => Ok(u32::from(in_zone(
            m,
            a & 65535,
            d & 65535,
            b & 65535,
            c & 65535,
            site & 65535,
        )?)),
        "DeselectionEngin" => {
            let r = unit(m, a)?;
            if m.b(r + 0xbd)? != 0 {
                m.sb(r + 0xbd, 0)?;
                retire(m, a & 65535, m.b(r)?)?;
            }
            Ok(r)
        }
        "Deselection" => {
            let p = a & 255;
            deselect(m, p)?;
            m.sw(0x495be0 + 2 * p, 65535)?;
            m.sb(0x496207, 255)?;
            m.sb(0x4e19b0, 0)?;
            m.sw(0x4e05ae, 0)?;
            m.sb(0x4e05ab, 0)?;
            m.sb(0x496d48, 1)?;
            Ok(p)
        }
        "BatimentPeutAttaquer" | "BatimentInitAttaqueSelect" => Ok(crate::chain_turret::entry(
            m,
            name,
            crate::FrameRegisters {
                eax: a,
                edx: d,
                ebx: b,
                ecx: c,
                ..Default::default()
            },
        )?
        .eax),
        "SetDechargeCaveEnginSelect" => {
            crate::chain_restore::select_unload(m, a & 255, d & 65535)?;
            Ok(0)
        }
        "SetDechargeCaveEngin" => {
            crate::chain_restore::unload(m, a & 65535, d & 65535)?;
            crate::player::unit(m, a & 65535)
        }
        "SetAttaqueEngin" => attack(m, a & 65535, d & 255, b & 65535),
        "SetRecolteEngin" => harvest_order(m, a & 65535, d),
        "SetTransporteEngin" => transport_order(m, a & 65535, d),
        "DeSelectionGroupe" => deselect(m, a & 255),
        "BatimentDeselectionner" => {
            m.sw(0x495be0 + 2 * (a & 255), 65535)?;
            Ok(a & 255)
        }
        "SelectionEngin" => select(m, a & 255, d & 65535, b & 65535),
        "EnginAddSelection" => {
            append(m, a & 65535, d & 255, true)?;
            Ok(a & 65535)
        }
        "BatimentSelection" => select_structure(m, a & 65535),
        "EnginAsk" => ask(m, a & 65535, d & 65535, b & 255, c),
        "BatimentPosable" => posable(m, a & 255, d & 65535, b & 65535),
        "SetEnginConstruitBatiment" => build_order(m, a & 65535, d & 255, b & 65535),
        "EnginTravailSelectionne" => {
            let mut found = false;
            for u in selections(m, a & 255)? {
                if !alive(m, u)? {
                    continue;
                }
                if d & 255 > 2 {
                    continue;
                }
                let t = m.b(unit(m, u)? + 1)?;
                if !match d & 255 {
                    0 => t == 6,
                    1 => matches!(t, 5 | 13),
                    _ => t == 7,
                } {
                    return Ok(0);
                }
                found = true
            }
            Ok(u32::from(found))
        }
        "SetMineEnginSelect" | "SetRamasseEnginSelect" | "SetDepotEnginSelect" => {
            let p = a & 255;
            let base = 0x508728 + (p << 9);
            let mut i = 0;
            let mut spoke = false;
            let mut value = a;
            while (i & 65535) < m.w(base)? {
                let u = m.w(base + 2 + 2 * (i & 65535))?;
                i += 1;
                if !alive(m, u)? {
                    continue;
                }
                let rec = unit(m, u)?;
                if name == "SetDepotEnginSelect" {
                    m.sb(rec + 0x82, d)?;
                    m.sw(rec + 0x80, b)?;
                    m.sw(rec + 0x84, c & 255)?;
                    value = ((u * 276) & 0xffff0000) | (c & 255);
                    continue;
                }
                if m.b(rec)? != p {
                    continue;
                }
                crate::frame_spawn::reset_order(m, u)?;
                let cell = if name == "SetMineEnginSelect" {
                    d.wrapping_add(256) & 65535
                } else {
                    d & 65535
                };
                m.sw(rec + 0x7e, cell)?;
                crate::frame_spawn::release(m, u)?;
                m.sb(rec + 0x88, 0)?;
                m.sb(rec + 0x89, 0)?;
                m.sb(rec + 2, if name == "SetMineEnginSelect" { 5 } else { 16 })?;
                if name == "SetMineEnginSelect" {
                    m.sb(rec + 0x82, 0)?
                }
                m.sw(rec + 0xb2, cell)?;
                value = rec;
                if !spoke {
                    spoke = true;
                    if p == m.b(0x508041)? {
                        value = ask(m, u, 15, 0, c)?
                    }
                }
            }
            Ok(value)
        }
        "SetDeplaceEnginSelect"
        | "SetConstruitBatimentSelect"
        | "SetRecolteEnginSelect"
        | "SetTransporteEnginSelect"
        | "SetAttaqueEnginSelect" => {
            let p = a & 255;
            let mut spoke = false;
            let mut eax = a;
            // The original list count is read at every iteration.
            let base = 0x508728 + (p << 9);
            let mut i = 0;
            while i < m.w(base)? {
                let u = m.w(base + 2 + 2 * i)?;
                i += 1;
                if !alive(m, u)? {
                    continue;
                }
                let rec = unit(m, u)?;
                if name != "SetTransporteEnginSelect" && m.b(rec)? != p {
                    continue;
                }
                let t = m.b(rec + 1)?;
                if name == "SetConstruitBatimentSelect" && t != 7 {
                    continue;
                }
                eax = match name {
                    "SetDeplaceEnginSelect" => {
                        crate::frame_spawn::order(m, u, d as u16)?;
                        m.sd(rec + 0x110, m.d(0x508f7c)?)?;
                        rec
                    }
                    "SetConstruitBatimentSelect" => build_order(m, u, d & 255, b & 65535)?,
                    "SetRecolteEnginSelect" => harvest_order(m, u, d & 65535)?,
                    "SetTransporteEnginSelect" => transport_order(m, u, d & 65535)?,
                    _ => {
                        m.sd(rec + 0x110, m.d(0x508f7c)?)?;
                        attack(m, u, d & 255, b & 65535)?
                    }
                };
                if name == "SetDeplaceEnginSelect" && (11..=17).contains(&t) {
                    continue;
                }
                if !spoke {
                    spoke = true;
                    if p == m.b(0x508041)? {
                        eax = ask(
                            m,
                            u,
                            match name {
                                "SetDeplaceEnginSelect" => 11,
                                "SetConstruitBatimentSelect" => 17,
                                "SetAttaqueEnginSelect" => 23,
                                _ => 15,
                            },
                            0,
                            c,
                        )?
                    }
                }
            }
            Ok(eax)
        }
        _ => stop(name, site),
    }
}
/// SetEnginRepareBatiment 0x444fdb: ResetOrdre, BatimentGetCase of the
/// low-word building as the unit's destination (+0x7e), release of the
/// pathfinding slot when this unit holds it, then work state 15 with the
/// building (+0xc0) and its cell (+0xb2).
fn repair_building_order(m: &mut MemoriaJogo, u: u32, b: u32) -> Result<()> {
    let r = unit(m, u)?;
    crate::frame_spawn::reset_order(m, u)?;
    let cell = crate::frame_core::building_cell(m, b & 65535)?;
    m.sw(r + 0x7e, cell)?;
    crate::frame_spawn::release(m, u)?;
    m.sb(r + 0x88, 0)?;
    m.sb(r + 0x88, 0)?;
    m.sb(r + 0x89, 0)?;
    m.sb(r + 2, 15)?;
    m.sw(r + 0xc0, b)?;
    m.sw(r + 0xb2, cell)
}
/// SetEnginRepareEngin 0x445083: as the building order, with the target
/// unit's cell (+0x7c) as destination and the unit index in +0xc0.
fn repair_unit_order(m: &mut MemoriaJogo, u: u32, t: u32) -> Result<()> {
    let r = unit(m, u)?;
    crate::frame_spawn::reset_order(m, u)?;
    let cell = m.w(unit(m, t & 65535)? + 0x7c)?;
    m.sw(r + 0x7e, cell)?;
    crate::frame_spawn::release(m, u)?;
    m.sb(r + 0x88, 0)?;
    m.sb(r + 0x88, 0)?;
    m.sb(r + 0x89, 0)?;
    m.sb(r + 2, 15)?;
    m.sw(r + 0xb2, cell)?;
    m.sw(r + 0xc0, t)
}
/// SetRepareSelect 0x473bbc (ProcessInputGame tag 20 at 0x45ef42, called with
/// the record's bytes +2 and +4). Mode 3 gives every alive unit of player
/// `p`'s selection SetEnginRepareBatiment, mode 1 SetEnginRepareEngin; any
/// other mode returns at once. No owner or type filter; the list count is
/// re-read at every iteration. The first alive unit answers (EnginAsk 0x13)
/// when `p` is the local player.
pub fn repair_select(m: &mut MemoriaJogo, p: u32, mode: u32, target: u32) -> Result<()> {
    let (p, mode, target) = (p & 255, mode & 255, target & 255);
    if mode != 1 && mode != 3 {
        return Ok(());
    }
    let base = 0x508728 + (p << 9);
    let mut spoke = 0u16;
    let mut i = 0;
    while i < m.w(base)? {
        let u = m.w(base + 2 + 2 * i)?;
        i += 1;
        if !alive(m, u)? {
            continue;
        }
        if mode == 3 {
            repair_building_order(m, u, target)?;
        } else {
            repair_unit_order(m, u, target)?;
        }
        if spoke >= 1 {
            continue;
        }
        spoke += 1;
        if p == m.b(0x508041)? {
            ask(m, u, 0x13, 0, u)?;
        }
    }
    Ok(())
}
/// VendreBatiment 0x41eddf: only the owner sells. The city AI is told, the
/// structure enters collapse (state 3) and half of its food and stone costs are
/// credited to both words of each stock record.
fn sell_building(m: &mut MemoriaJogo, s: u32, p: u32) -> Result<()> {
    let r = player::structure(s);
    if m.b(r + 4)? != p {
        return Ok(());
    }
    crate::frame_ai::call(m, "iagVilleSendMessage", &[s, 2, 0])?;
    m.sb(r + 0x90, 1)?;
    let t = m.b(r + 8)?;
    m.sb(r + 5, 3)?;
    if !matches!(t, 9 | 12) {
        m.sb(0x4dfb44, 0x14)?;
    }
    if t == 20 {
        m.sw(r + 0x84, 0x113)?;
    }
    let (food, stone) = (m.w(0x495d98 + 52 * t)? >> 1, m.w(0x495d9a + 52 * t)? >> 1);
    let base = 0x4e2ea8 + 32 * p;
    for (offset, amount) in [(10, food), (8, food), (18, stone), (16, stone)] {
        m.sw(
            base + offset,
            m.w(base + offset)?.wrapping_add(amount) & 65535,
        )?;
    }
    Ok(())
}
/// SetEnginSelectRentreEngin 0x4740c2 (ProcessInputGame tag 16 mode 1): every
/// alive unit of player `p`'s selection owned by `p` boards the target unit
/// (SetEnginRentreEngin 0x445f2d); the list count is re-read per iteration.
/// Colonists (EnginGetType 9) never answer; the first other unit answers
/// through EnginAsk 0xe when `p` is the local player.
pub fn select_board(m: &mut MemoriaJogo, p: u32, target: u32) -> Result<()> {
    let p = p & 255;
    let base = 0x508728 + (p << 9);
    let mut spoke = 0u16;
    let mut i = 0;
    while i < m.w(base)? {
        let u = m.w(base + 2 + 2 * i)?;
        i += 1;
        if !alive(m, u)? || m.b(unit(m, u)?)? != p {
            continue;
        }
        crate::ai_rules::board(m, u, target & 65535)?;
        if m.b(unit(m, u)? + 1)? == 9 || spoke >= 1 {
            continue;
        }
        spoke += 1;
        if p == m.b(0x508041)? {
            ask(m, u, 0xe, 0, u)?;
        }
    }
    Ok(())
}
/// EnginEstBlesse 0x4420a6: current health differs from the maximum of its type.
fn wounded(m: &MemoriaJogo, r: u32) -> Result<bool> {
    Ok(m.w(0x4965b6 + 50 * m.b(r + 1)?)? != m.w(r + 0x9a)?)
}
/// UpdateBatiment 0x41a860(player): the selected type-15 structure becomes
/// type 20 and leaves the player's upgrade Bag.
fn upgrade_selected(m: &mut MemoriaJogo, p: u32) -> Result<()> {
    let s = m.w(0x495be0 + 2 * p)?;
    let r = 0x49a0c8 + 276 * s;
    let t = m.b(r + 8)?;
    if t != 15 {
        return Ok(());
    }
    crate::chain_bags::body(
        m,
        "BagRetire",
        crate::FrameRegisters {
            eax: 0x4dd6e8 + 2288 * p + 104 * t,
            edx: 3,
            ebx: 0x14,
            ..Default::default()
        },
    )?;
    m.sb(r + 8, 0x14)?;
    m.sb(r + 0x75, 0)?;
    m.sw(r + 0x80, 0x14)?;
    Ok(())
}
/// SetEnginPiege 0x445307: a unit already laying (state 22) queues the cell,
/// up to ten; otherwise the unit starts walking to it in state 22.
fn set_trap(m: &mut MemoriaJogo, u: u16, cell: u16) -> Result<()> {
    let r = player::unit(m, u32::from(u))?;
    if m.b(r + 2)? == 0x16 {
        let n = m.u16(r + 0xd8)?;
        if n < 10 {
            m.set_u16(r + 0xc4 + 2 * u32::from(n), cell)?;
            m.set_u16(r + 0xd8, n + 1)?;
        }
        return Ok(());
    }
    crate::frame_spawn::call(m, "ResetOrdre", &[u32::from(u)])?;
    m.set_u16(r + 0x7e, cell)?;
    crate::frame_spawn::release(m, u32::from(u))?;
    m.set_u8(r + 0x88, 0)?;
    m.set_u8(r + 0x89, 0)?;
    m.set_u8(r + 2, 0x16)?;
    m.set_u16(r + 0xd8, 0)?;
    m.set_u16(r + 0xb2, cell)
}

/// 0x44a7c1 and 0x44a9bc: head of the trap queue becomes the next target.
fn next_trap(m: &mut MemoriaJogo, u: u16, r: u32) -> Result<()> {
    let cell = m.u16(r + 0xc4)?;
    m.set_u16(r + 0x7e, cell)?;
    crate::frame_spawn::release(m, u32::from(u))?;
    m.set_u8(r + 0x88, 0)?;
    m.set_u8(r + 0x89, 0)?;
    m.set_u8(r + 2, 0x16)?;
    m.set_u16(r + 0xb2, cell)?;
    let n = m.u16(r + 0xd8)?;
    for i in 0..u32::from(n) {
        m.set_u16(r + 0xc4 + 2 * i, m.u16(r + 0xc6 + 2 * i)?)?;
    }
    m.set_u16(r + 0xd8, n.wrapping_sub(1))
}

fn set_idle(m: &mut MemoriaJogo, u: u16, r: u32) -> Result<()> {
    // EnginSetEtat 0x44115f with state 7.
    if !matches!(m.u8(r + 2)?, 0 | 4) {
        m.set_u8(r + 2, 7)?;
        crate::frame_spawn::release(m, u32::from(u))?;
    }
    Ok(())
}

/// CalculPiege 0x44a706, state 22 of a kind-14 unit: walk to +0xb2 (phase 0),
/// then a 20-tick laying phase (+0x8b) that puts the mine down on its first tick.
pub fn calcul_piege(m: &mut MemoriaJogo, u: u16) -> Result<()> {
    let r = player::unit(m, u32::from(u))?;
    if m.u8(r + 1)? != 14 {
        return Ok(());
    }
    match m.u8(r + 0x89)? {
        0 => {
            if m.u16(r + 8)? != m.u16(r + 0x7e)? || m.u8(r + 0x86)? != 0 || m.u8(r + 0x87)? != 0 {
                crate::frame_movement::call(m, "DeplaceEngin", &[u32::from(u)])?;
                return Ok(());
            }
            if m.u16(r + 8)? == m.u16(r + 0xb2)? {
                m.set_u8(r + 0x89, 1)?;
                m.set_u8(r + 0x8b, 0x14)?;
                m.set_u8(r + 0xa9, 0)?;
                return m.set_u8(r + 0xa4, 0);
            }
            if m.u16(r + 0xd8)? != 0 && m.u8(r + 0xda)? != 0 {
                return next_trap(m, u, r);
            }
            set_idle(m, u, r)?;
            m.set_u8(r + 0x88, 0)
        }
        1 => {
            crate::player_build::anim(m, u32::from(u))?;
            if m.u8(r + 0x8b)? == 0x14 {
                let cell = u32::from(m.u16(r + 8)?);
                // TerrainPoseMine 0x475431.
                let objects = m.u32(0x507d74)?;
                m.set_u8(objects.wrapping_add(cell), 0x46)?;
                let aux = m.u32(0x507dac)?;
                m.set_u8(aux.wrapping_add(cell), m.u8(r)?)?;
                let charge = m.u8(r + 0xda)?;
                if charge != 0 {
                    m.set_u8(r + 0xda, charge - 1)?;
                }
                let local = u32::from(m.u8(0x508041)?);
                if crate::frame_core::call(m, "TerrainEstDecouvert", &[cell, local])? & 255 != 0 {
                    let volume = unit_volume(m, u32::from(u))?;
                    m.set_u8(r + 0xf9, 0)?;
                    if m.u32(r + 0x104)? == u32::MAX {
                        player::sound(
                            m,
                            0x44a98e,
                            true,
                            [0x50, (u32::from(m.u8(r + 1)?) << 16) | 0x2300, r + 0x104, 0],
                            Some(volume),
                        )?;
                    }
                }
            }
            let left = m.u8(r + 0x8b)?.wrapping_sub(1);
            m.set_u8(r + 0x8b, left)?;
            if left != 0 {
                return Ok(());
            }
            if m.u16(r + 0xd8)? != 0 {
                return next_trap(m, u, r);
            }
            set_idle(m, u, r)?;
            m.set_u16(r + 0x7e, m.u16(r + 0x7c)?)?;
            crate::frame_spawn::release(m, u32::from(u))?;
            m.set_u8(r + 0x88, 0)
        }
        _ => Ok(()),
    }
}

pub fn process(m: &mut MemoriaJogo, p: u32) -> Result<u32> {
    let p = p & 255;
    let a = m.d(0x4e2458 + 4 * p)?;
    let tag = m.w(a)?;
    let mut w = [0; 5];
    for (i, v) in w.iter_mut().enumerate() {
        *v = m.w(a + 2 * i as u32)?
    }
    let jump = if tag > 39 {
        0x45f2de
    } else {
        m.d(0x45ed20 + 4 * tag)?
    };
    if tag != 0 {
        let bytes = m.read(a, 10)?;
        let v = [
            p,
            tag,
            u32::from_le_bytes(bytes[0..4].try_into().expect("packet")),
            u32::from_le_bytes(bytes[4..8].try_into().expect("packet")),
            u32::from(u16::from_le_bytes(bytes[8..10].try_into().expect("packet"))),
            jump,
        ];
        player::event(m, 8, 0x45edc0, &v)
    }
    crate::order_queue::replaced(m, p, tag, &w)?;
    match tag {
        1..=3 => {
            deselect(m, p)?;
            m.sw(0x495be0 + 2 * p, 65535)?;
            if tag == 1 {
                select(m, p, w[1], w[2])?;
            } else if tag == 2 {
                select_structure(m, w[1])?;
            }
        }
        4 => {
            call(
                m,
                "SetConstruitBatimentSelect",
                [p, w[1] & 255, w[2], 0],
                0x45ef9b,
            )?;
        }
        5 => {
            call(m, "SetDeplaceEnginSelect", [p, w[1], p << 2, 0], 0x45efd8)?;
        }
        6 | 7 | 8 | 19 => {
            let (name, site) = match tag {
                6 => ("SetMineEnginSelect", 0x45eff9),
                7 => ("SetRecolteEnginSelect", 0x45f058),
                8 => ("SetTransporteEnginSelect", 0x45f076),
                _ => ("SetDechargeCaveEnginSelect", 0x45f085),
            };
            call(m, name, [p, w[1], p << 2, 0], site)?;
            call(
                m,
                "SetDepotEnginSelect",
                [p, w[2] & 255, w[3], w[4] & 255],
                0x45f037,
            )?;
        }
        14 => {
            if player::selected_structure(m, p)? == 65535 {
                call(
                    m,
                    "SetAttaqueEnginSelect",
                    [p, w[1] & 255, w[2], 0],
                    0x45f105,
                )?;
            } else {
                crate::chain_turret::entry(
                    m,
                    "BatimentInitAttaqueSelect",
                    crate::FrameRegisters {
                        eax: p,
                        edx: w[1] & 255,
                        ebx: w[2],
                        ..Default::default()
                    },
                )?;
            }
        }
        17 => {
            crate::chain_recruit::init(m, w[1] & 255, p)?;
        }
        18 => {
            call(m, "SetRamasseEnginSelect", [p, w[1], p << 2, 0], 0x45f217)?;
        }
        22 => {
            crate::spell_order_acceptance::selected(
                m,
                p as u8,
                w[1] as u8,
                w[2] as u8,
                w[3] as u16,
            )?;
        }
        36 => {
            m.sw(0x495be0 + 2 * p, 65535)?;
            append(m, w[1], p, true)?;
        }
        11 => {
            crate::chain_groups::entry(
                m,
                "SauveSelectionGroupe",
                crate::FrameRegisters {
                    eax: p,
                    edx: w[1] & 255,
                    ..Default::default()
                },
            )?;
        }
        12 => {
            m.sw(0x495be0 + 2 * p, 65535)?;
            deselect(m, p)?;
            crate::chain_groups::entry(
                m,
                "RestaureSelectionGroupe",
                crate::FrameRegisters {
                    eax: p,
                    edx: w[1] & 255,
                    ..Default::default()
                },
            )?;
        }
        16 => {
            if w[1] == 1 {
                // 0x45f17a: SetEnginSelectRentreEngin 0x4740c2(player, word +4).
                select_board(m, p, w[2])?;
            } else if w[1] == 3 {
                crate::chain_entry::select_enter(m, p as u8, w[2] as u16)?;
            }
        }
        9 => {
            // 0x45f1b3: SetEnginDepose 0x444533(word +2, byte +4, byte +6).
            crate::chain_unload::depose(m, w[1] as u16, (w[2] & 255) as u8, (w[3] & 255) as u16)?;
        }
        20 => {
            // 0x45ef42: SetRepareSelect 0x473bbc(player, byte +2, byte +4).
            repair_select(m, p, w[1] & 255, w[2] & 255)?;
        }
        32 => {
            // 0x45f234: SetEnginPiegeSelect 0x47348a(player, word +2).
            for u in player::selections(m, p)? {
                let r = player::unit(m, u)?;
                if player::alive(m, u)? && m.b(r)? == p && m.b(r + 1)? == 14 {
                    set_trap(m, u as u16, w[1] as u16)?;
                }
            }
        }
        33 => {
            crate::chain_recruit::retire_colonist(m, player::selected_structure(m, p)?)?;
        }
        34 => {
            // 0x45ef58: SetVolEnginSelect 0x4733b8(player, BL = byte +4, CL = byte +6).
            crate::player_spy::vol_select(m, p, w[2] & 255, w[3] & 255)?;
        }
        37 => {
            // 0x45ef36: EnginSelectSetFatigue 0x473cc5(player).
            crate::player_spy::fatigue_select(m, p)?;
        }
        39 => {
            // 0x45f257: BatimentLanceNova 0x4266e1(player, word +2).
            crate::player_spy::launch_nova(m, p, w[1] & 65535)?;
        }
        35 => {
            // 0x45f246: InitApprentissage 0x422d9f(player, byte +2).
            crate::spell_order_acceptance::init_apprentissage(m, p, w[1] & 255)?;
        }
        23..=31 => crate::market::order(m, p, tag, w)?,
        10 => {
            // 0x45f0a4: VendreBatiment 0x41eddf(AX = word +2, DL = player).
            sell_building(m, w[1] & 65535, p)?;
        }
        15 => upgrade_selected(m, p)?,
        21 => {
            // SetEnginSelectSoigneBatiment 0x474067(player, structure): selected
            // alive wounded units (EnginEstBlesse 0x4420a6) enter the building.
            let s = w[1] & 65535;
            for u in player::selections(m, p)? {
                let r = player::unit(m, u)?;
                if player::alive(m, u)? && wounded(m, r)? {
                    crate::chain_entry::set_enter(m, u as u16, s as u16)?;
                }
            }
        }
        38 => {
            // 0x45edf3: PlayerValideAlliance(byte +2, byte +4).
            crate::messagebox_state::valide_alliance(m, w[1] & 255, w[2] & 255)?;
        }
        13 => {
            // 0x45ee0a: only the local player's pause order opens the menu.
            if p == m.b(0x508041)? {
                crate::campaign_flow::menu_init(
                    m,
                    p as u8,
                    16,
                    &mut crate::campaign_flow::NoFiles,
                )?;
            }
        }
        // Shift+click waypoint (RULE_ORDER_QUEUE). Without the rule, tag 40 takes
        // the original default 0x45f2de like every tag above 39: no action, then
        // the packet is cleared below (corpus case process_tag40_clear).
        crate::order_queue::TAG_QUEUE if m.order_queue_rule() => {
            crate::order_queue::order(m, p, w[1] as u16)?
        }
        _ => {
            if jump != 0x45f2de {
                return stop("ProcessInputGame tag", jump);
            }
        }
    }
    for off in [0, 2, 4, 6, 8] {
        m.sw(m.d(0x4e2458 + 4 * p)? + off, 0)?;
    }
    m.d(0x4e2458 + 4 * p)
}
