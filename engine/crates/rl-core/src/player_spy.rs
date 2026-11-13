//! Orders tags 34, 37 and 39 and the spy state 23, transcribed from intact x86.
//! EnginSelectSetFatigue 0x473cc5, SetVolEnginSelect 0x4733b8, SetVolEngin
//! 0x4456c9, EnginNextEtape state 23 0x455fe2, BatimentLanceNova 0x4266e1,
//! BatimentRecentreSurCase 0x4265fd.
use crate::player::{alive, structure, unit, Memory};
use crate::{FrameRegisters, MemoriaJogo, Result};

const LOCAL: u32 = 0x508041;
const CHANCE: u32 = 0x4e19bc;
const RESEARCH: u32 = 0x5080f4;

/// The selection list is reread on every iteration, as the native loop does.
fn selected(m: &MemoriaJogo, p: u32, i: u32) -> Result<Option<u32>> {
    let a = 0x508728 + ((p & 255) << 9);
    if i >= m.w(a)? {
        return Ok(None);
    }
    Ok(Some(m.w(a + 2 + 2 * i)?))
}

/// The focused-unit trio cleared when the unit at 0x496304 changes state.
fn release_focus(m: &mut MemoriaJogo, u: u32) -> Result<()> {
    if u == m.w(0x496304)? {
        m.sb(0x4e0f36, 0)?;
        m.sb(0x4e0f37, 0)?;
        m.sd(0x4e0f3c, 0)?;
        m.sw(0x496304, 0xffff)?;
    }
    Ok(())
}

/// EnginSelectSetFatigue 0x473cc5 (tag 37): every live selected unit of the
/// player whose kind tires (TypeEnginSeFatigue) gets EnginSetFatigue.
pub fn fatigue_select(m: &mut MemoriaJogo, p: u32) -> Result<()> {
    let p = p & 255;
    let mut i = 0;
    while let Some(u) = selected(m, p, i)? {
        i += 1;
        if !alive(m, u)? {
            continue;
        }
        let r = unit(m, u)?;
        if m.b(r)? != p {
            continue;
        }
        let kind = m.b(r + 1)?;
        if crate::frame_core::call(m, "TypeEnginSeFatigue", &[kind])? & 255 == 0 {
            continue;
        }
        crate::chain_fatigue::set(m, u)?;
    }
    Ok(())
}

/// SetVolEngin 0x4456c9(AX unit, BX structure, CL mode). EDX is not read.
pub fn set_vol(m: &mut MemoriaJogo, u: u32, s: u32, mode: u32) -> Result<()> {
    let u = u & 65535;
    let r = unit(m, u)?;
    if alive(m, u)? {
        m.sb(r + 2, 23)?;
        release_focus(m, u)?;
    }
    m.sw(r + 0xc4, mode & 255)?;
    m.sw(r + 0xc0, s & 65535)?;
    let cell = crate::frame_core::building_cell(m, s & 65535)?;
    m.sw(r + 0x7e, cell & 65535)?;
    release_focus(m, u)?;
    m.sb(r + 0x88, 0)
}

/// SetVolEnginSelect 0x4733b8(AL player, EBX structure, CL mode), reached
/// from ProcessInputGame tag 34 with BL = the low byte of packet word +4:
/// the native order truncates the structure index to eight bits.
pub fn vol_select(m: &mut MemoriaJogo, p: u32, s: u32, mode: u32) -> Result<()> {
    let p = p & 255;
    let mut asked = 0u32;
    let mut i = 0;
    while let Some(u) = selected(m, p, i)? {
        i += 1;
        if !alive(m, u)? || m.b(unit(m, u)?)? != p {
            continue;
        }
        set_vol(m, u, s, mode)?;
        if p != m.b(LOCAL)? || asked >= 1 {
            continue;
        }
        asked += 1;
        let voice = if mode & 255 <= 2 { 0x1c } else { 0x1b };
        crate::player_orders::call(m, "EnginAsk", [u, voice, 0, asked], 0x4733db)?;
    }
    Ok(())
}

/// RechercheTypeBatiment 0x461222: the first live structure of the player and
/// type, or 0xffff.
pub fn find_structure(m: &MemoriaJogo, p: u32, kind: u32) -> Result<u32> {
    for s in 0..1000 {
        let a = structure(s);
        if m.b(a + 5)? != 0 && m.b(a + 4)? == p & 255 && m.b(a + 8)? == kind & 255 {
            return Ok(s);
        }
    }
    Ok(0xffff)
}

fn message(m: &mut MemoriaJogo, text: u32) -> Result<()> {
    crate::messagebox_state::entry(
        m,
        "MessageBoxSet",
        FrameRegisters {
            eax: 0xa,
            edx: text,
            ..Default::default()
        },
    )
    .map(|_| ())
}

/// The unit leaves the map after the attempt (0x456132 and 0x45634d).
fn spent(m: &mut MemoriaJogo, u: u32) -> Result<()> {
    crate::chain_entry::retire(m, u as u16)?;
    let r = unit(m, u)?;
    if alive(m, u)? {
        m.sb(r + 2, 0)?;
        release_focus(m, u)?;
    }
    Ok(())
}

/// EnginNextEtape state 23, 0x455fe2..0x4563d5: the spy walks to the target
/// structure, then rolls against the owner's chance byte at 0x4e19bc. Every
/// path continues at 0x454ccd.
pub fn state23(m: &mut MemoriaJogo, u: u16) -> Result<()> {
    let u = u32::from(u);
    let r = unit(m, u)?;
    let s = m.w(r + 0xc0)?;
    let a = structure(s);
    let owner = m.b(a + 4)?;
    if m.b(a + 5)? == 3 || owner == 200 || owner == m.b(r)? {
        // 0x454d97
        let back = m.w(r + 0x7e)?;
        crate::frame_spawn::call(m, "SetDeplaceEngin", &[u, back])?;
        return m.sw(r + 0xc0, 0xffff);
    }
    if !crate::chain_entry::adjacent(m, s as u16, m.w(r + 8)? as u16)? {
        if m.w(r + 0x7e)? == m.w(r + 8)? {
            let cell = crate::frame_core::building_cell(m, s)?;
            m.sw(r + 0x7e, cell & 65535)?;
            release_focus(m, u)?;
            m.sb(r + 0x88, 0)?;
        }
        // 0x4555a7
        crate::frame_movement::call(m, "DeplaceEngin", &[u])?;
        return Ok(());
    }
    let thief = m.b(r)?;
    let roll = crate::frame_core::random(m)? % 100;
    let chance = m.b(CHANCE + thief)?;
    if roll > chance {
        m.sb(CHANCE + thief, chance.saturating_sub(5))?;
        spent(m, u)?;
        if thief == m.b(LOCAL)? {
            message(m, 0x1f6)?;
        }
        return Ok(());
    }
    let mut halve = false;
    if m.w(r + 0xc4)? != 3 {
        crate::frame_economy::call(m, "MAJSympathie", &[owner, thief, 3])?;
        let resource = m.b(r + 0xc4)? % 3;
        let base = 0x4e2ea8 + 32 * owner + 8 * resource;
        let stock = m.w(base + 2)?.wrapping_sub(m.w(base)?) & 65535;
        let cave = find_structure(m, thief, 3)?;
        let amount = stock / 2;
        crate::chain_restore::debit(m, resource as u8, amount as u16, owner as u8)?;
        // Deliberate defensive modernization approved by the user for
        // RL-THEFT-001 (2026-11-13): when the thief owns no type-3 structure,
        // RechercheTypeBatiment returns 0xffff and the original still calls
        // BatimentAjouteMatiere 0x42340f with it, reading the type byte at
        // 0x15d9fbc (pc 0x42343f), beyond the structure table. Only that credit
        // is skipped; the victim's debit and everything after run as in the
        // original. Every thief with a cave keeps the exact native call.
        if cave != 0xffff {
            crate::chain_entry::credit(m, resource as u8, amount as u16, thief as u8, cave as u16)?;
        }
        halve = true;
        if thief == m.b(LOCAL)? {
            message(m, 0x1f5)?;
        }
    } else {
        crate::frame_economy::call(m, "MAJSympathie", &[owner, thief, 4])?;
        let branch = m.b(RESEARCH + 16 * owner + 4)?;
        let mine = m.b(RESEARCH + 16 * thief + branch)?;
        let theirs = m.b(RESEARCH + 16 * owner + branch)?;
        if mine < theirs {
            if thief == m.b(LOCAL)? {
                message(m, 0x1f5)?;
            }
            crate::mission_research::complete(m, thief)?;
            halve = true;
        }
    }
    if halve {
        let c = m.b(CHANCE + thief)?;
        m.sb(CHANCE + thief, c >> 1)?;
    }
    spent(m, u)
}

/// BatimentRecentreSurCase 0x4265fd(AX cell). The vertical clamp stores into
/// the horizontal scroll word 0x508720 and leaves 0x508722 unchanged, as the
/// native code does.
pub fn recentre(m: &mut MemoriaJogo, cell: u32) -> Result<()> {
    let cell = cell & 65535;
    let (x, y) = (cell % 256, cell / 256);
    let half = m.w(0x508724)? / 2;
    let nx = x.wrapping_sub(half);
    if (nx as u16 as i16) < 0 {
        m.sw(0x508720, 0)?;
    } else if i32::from(nx as u16 as i16) > 256 - half as i32 {
        m.sw(0x508720, 0x100u32.wrapping_sub(half))?;
    } else {
        m.sw(0x508720, nx & 65535)?;
    }
    let half = m.w(0x508726)? / 2;
    let ny = y.wrapping_sub(half);
    if (ny as u16 as i16) < 0 {
        m.sw(0x508722, 0)
    } else if i32::from(ny as u16 as i16) > 256 - half as i32 {
        m.sw(0x508720, 0x100u32.wrapping_sub(half))
    } else {
        m.sw(0x508722, ny & 65535)
    }
}

/// BatimentNovaPrete 0x4266bc: structure byte +0x104 equals 3.
pub fn nova_ready(m: &MemoriaJogo, s: u32) -> Result<bool> {
    Ok(m.b(structure(s) + 0x104)? == 3)
}

/// BatimentLanceNova 0x4266e1(AL player, DX target cell), tag 39.
pub fn launch_nova(m: &mut MemoriaJogo, p: u32, target: u32) -> Result<()> {
    let p = p & 255;
    let s = m.w(0x495be0 + 2 * p)?;
    if s == 0xffff || !nova_ready(m, s)? {
        return Ok(());
    }
    let a = structure(s);
    let origin = m.w(a + 2)?;
    crate::frame_combat::call(
        m,
        "ProjectileInit",
        &[0x1e, origin, target & 65535, 3, s, 0, 0x43, 0xffff_ffde, 0],
    )?;
    crate::frame_economy::call(m, "MAJSympathie", &[p, p, 8])?;
    let a = structure(m.w(0x495be0 + 2 * p)?);
    m.sb(a + 0x7a, 0)?;
    m.sb(a + 0x104, 0)?;
    m.sd(a + 0x108, 1)
}
