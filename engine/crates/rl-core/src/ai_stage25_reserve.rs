//! Original MONDE25 group disposal: reserve, pool fallback, then group deletion.
use crate::{frame::stop, FrameRegisters, MemoriaJogo, Result};
fn group(index: u32) -> u32 {
    0x51a9e0u32.wrapping_add(index.wrapping_shl(5))
}
fn unit(m: &MemoriaJogo, member: u16) -> Result<u32> {
    Ok(m.u32(0x4e19a0)?.wrapping_add(276 * u32::from(member)))
}

/// iagWarComputeGroupStrength487508. Its native iterator writes are retained.
fn strength(m: &mut MemoriaJogo, index: u32) -> Result<u32> {
    let g = group(index);
    if g == 0 || m.u16(g.wrapping_add(0x12))? == 0 {
        return Ok(0);
    }
    let mut member = crate::frame_raid::first_unit(m, index)?;
    let mut total = 0u32;
    let mut traversed = 0;
    while member != u16::MAX {
        let r = unit(m, member)?;
        if !matches!(m.u8(r + 2)?, 0 | 4) {
            total = total.wrapping_add(m.u32(0x498cf4 + 4 * u32::from(m.u8(r + 1)?))?);
        }
        member = crate::frame_raid::next_unit(m)?;
        traversed += 1;
        if traversed > 10000 {
            return stop("iagWarComputeGroupStrength nonprogress group", 0x48752f);
        }
    }
    Ok(total)
}

/// iagUnitReturnGroupToReserve48300a. Only error0018 requests pool fallback.
/// Eligibility does not test alive state; reserve strength does. Both details
/// and the signed strength/limit comparison follow the original instructions.
fn to_reserve(m: &mut MemoriaJogo, player: u32, source: u32) -> Result<u32> {
    let g = group(source);
    if g == 0 {
        return Ok(0x1a1a0010);
    }
    let destination = m.u32(0x51b688u32.wrapping_add(player.wrapping_mul(96)))?;
    if destination == u32::MAX {
        return Ok(0x1a1a0010);
    }
    // iagWarCalculeForceNecessaireReserve487570 and built-total484b96.
    let limit = m
        .u32(0x51b8b8u32.wrapping_add(player.wrapping_mul(124)))?
        .wrapping_mul(m.u32(0x51bd04u32.wrapping_add(player.wrapping_mul(0x1144)))?);
    let mut transferred = 0;
    while m.u16(g.wrapping_add(0x12))? != 0 {
        if strength(m, destination)? as i32 >= limit as i32 {
            break;
        }
        let mut member = crate::frame_raid::first_unit(m, source)?;
        let mut found = false;
        let mut traversed = 0;
        while member != u16::MAX {
            let kind = u32::from(m.u8(unit(m, member)? + 1)?);
            if m.u8(0x4989f9 + 4 * kind)? & 2 != 0 {
                crate::frame_raid::remove_unit(m, source, member)?;
                crate::frame_raid::add_unit(m, destination, member)?;
                found = true;
                break;
            }
            member = crate::frame_raid::next_unit(m)?;
            traversed += 1;
            if traversed > 10000 {
                return stop("iagUnitReturnGroupToReserve nonprogress search", 0x4830c8);
            }
        }
        if !found {
            break;
        }
        transferred += 1;
        if transferred > 10000 {
            return stop("iagUnitReturnGroupToReserve nonprogress transfer", 0x48306d);
        }
    }
    Ok(if m.u16(g.wrapping_add(0x12))? == 0 {
        0
    } else {
        0x1a1a0018
    })
}

/// iagUnitReturnGroupToPool482f34. Remove the first source member each time;
/// a missing destination still leaves the native removal in effect.
fn to_pool(m: &mut MemoriaJogo, player: u32, source: u32) -> Result<u32> {
    let g = group(source);
    if g == 0 {
        return Ok(0x1a1a0010);
    }
    // Each lap removes the head (found at step 0), so the count drops by one
    // (wrapping, as the original's unchecked decrement) even on a cyclic unit
    // list; the original then ends on count 0 within 65535 laps. A lap that
    // leaves the count unchanged re-added the head to the source itself
    // (pool == source): the same state again, the original's endless loop.
    loop {
        let count = m.u16(g.wrapping_add(0x12))?;
        if count == 0 {
            break;
        }
        let member = crate::frame_raid::first_unit(m, source)?;
        if member == u16::MAX {
            break;
        }
        crate::frame_raid::remove_unit(m, source, member)?;
        let destination = m.u32(0x51b664u32.wrapping_add(player.wrapping_mul(96)))?;
        if destination != u32::MAX {
            crate::frame_raid::add_unit(m, destination, member)?;
        }
        if m.u16(g.wrapping_add(0x12))? == count {
            return stop("iagUnitReturnGroupToPool nonprogress transfer", 0x482f64);
        }
    }
    Ok(0)
}

/// V_MiseADisposition481cb8. Group deletion is unconditional even when the
/// reserve call returns0010. The RULE returns1 and does not consume RNG.
pub fn dispose(m: &mut MemoriaJogo, arguments: &[u32], player: u32) -> Result<u32> {
    let Some(&source) = arguments.first() else {
        return stop("V_MiseADisposition missing argument", 0x481cb8);
    };
    if to_reserve(m, player, source)? == 0x1a1a0018 {
        to_pool(m, player, source)?;
    }
    crate::ai_rules::delete_group(m, player, source)?;
    Ok(1)
}

pub fn entry(m: &mut MemoriaJogo, mut r: FrameRegisters) -> Result<FrameRegisters> {
    let arguments = [m.u32(r.eax)?];
    let player = m.u32(r.eax.wrapping_add(0x1c))?;
    r.eax = dispose(m, &arguments, player)?;
    r.esp = r.esp.wrapping_add(4);
    Ok(r)
}
