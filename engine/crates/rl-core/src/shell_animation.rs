//! AffGame's menu/briefing raster bodies, with original MENU/INVENTE CMP banks.
//! Surface locking/flipping stays in the platform adapter. This module uses the
//! same Context/Bob and text APIs as game rendering, without changing them.
use crate::{
    render::{Context, Error, Renderer, Result},
    render_assets::{Assets, Sprite},
    shell_draw, FrameRegisters, MemoriaJogo,
};
use std::collections::BTreeMap;

pub struct Bank {
    assets: Assets,
}
impl Bank {
    /// The decoded original sprites (port-only screens draw with them).
    pub fn assets(&self) -> &Assets {
        &self.assets
    }
    pub fn load(raw: &[u8], base: &Assets) -> Result<Self> {
        let digest = crate::render_assets::sha256(raw);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        if !matches!(
            hex.as_str(),
            "9a0a407e8eb2be4fc24f1f7ca57f1b0aad67e7e49e0ca09b667e2d0ea4d7e866"
                | "a1c2e7155bec34c52d249d4c3aec7e3ce45759d806f880168dd8d8b4b05101aa"
        ) {
            return Err(Error::Invalid("original MENU/INVENTE CMP identity"));
        }
        let mut at = 2;
        let count = u16::from_le_bytes(raw[..2].try_into().unwrap());
        let mut sprites = Vec::new();
        for _ in 0..count {
            let header = raw
                .get(at..at + 8)
                .ok_or(Error::Invalid("shell CMP header"))?;
            let width = usize::from(u16::from_le_bytes(header[..2].try_into().unwrap()));
            let height = usize::from(u16::from_le_bytes(header[2..4].try_into().unwrap()));
            let size = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
            at += 8;
            let payload = raw
                .get(at..at + size)
                .ok_or(Error::Invalid("shell CMP payload"))?;
            at += size;
            let mut pixels = vec![0; width * height];
            let mut mask = vec![0; width * height];
            let mut p = 0;
            for y in 0..height {
                let mut x = 0;
                while x < width {
                    let token = *payload.get(p).ok_or(Error::Invalid("shell CMP command"))?;
                    p += 1;
                    if token == 128 {
                        return Err(Error::Invalid("shell CMP0x80"));
                    }
                    let n = if token > 128 {
                        256 - usize::from(token)
                    } else {
                        usize::from(token)
                    };
                    if x + n > width {
                        return Err(Error::Invalid("shell CMP row overflow"));
                    }
                    if token > 128 {
                        pixels[y * width + x..y * width + x + n].copy_from_slice(
                            payload
                                .get(p..p + n)
                                .ok_or(Error::Invalid("shell CMP literal"))?,
                        );
                        mask[y * width + x..y * width + x + n].fill(255);
                        p += n;
                    }
                    x += n;
                }
            }
            if payload[p..].iter().any(|&b| b != 0) {
                return Err(Error::Invalid("shell CMP trailing bytes"));
            }
            sprites.push(Sprite {
                width,
                height,
                pixels,
                mask,
                raw: payload.to_vec(),
            });
        }
        if at != raw.len() {
            return Err(Error::Invalid("shell CMP trailing bank bytes"));
        }
        Ok(Self {
            assets: Assets {
                sprites,
                palette: base.palette,
                shadow: base.shadow,
                light: base.light,
            },
        })
    }
}
fn context<'a>(
    renderer: &'a mut Renderer,
    m: &'a MemoriaJogo,
    bank: &'a Bank,
) -> Result<Context<'a>> {
    Ok(Context {
        background: &mut renderer.saved_background,
        text_boundaries: &mut renderer.recorded_text_boundaries,
        text_checked: 0,
        tick: renderer.tick,
        m,
        overrides: BTreeMap::new(),
        surface: &mut renderer.surface,
        cache: &mut renderer.cache,
        assets: &bank.assets,
        observations: Vec::new(),
        audit: renderer.audit,
        clip: [
            i32::from(m.u16(0x49a020)?),
            i32::from(m.u16(0x49a01c)?),
            i32::from(m.u16(0x49a022)?),
            i32::from(m.u16(0x49a01e)?),
        ],
        camera: 0,
    })
}
fn set_bank(m: &mut MemoriaJogo, slot: u32) -> crate::Result<u32> {
    set_bank_header(m, m.u32(slot)?)
}
fn set_bank_header(m: &mut MemoriaJogo, header: u32) -> crate::Result<u32> {
    let table = m.u32(header.wrapping_add(4))?;
    m.set_u32(0x508f6c, table)?;
    Ok(table)
}
/// Literal SetBank0x418590 projection, including its returned table pointer.
pub fn set_bank_registers(
    m: &mut MemoriaJogo,
    mut r: FrameRegisters,
) -> crate::Result<FrameRegisters> {
    r.eax = set_bank_header(m, r.eax)?;
    r.esp = r.esp.wrapping_add(4);
    Ok(r)
}
fn menu_sprites(renderer: &mut Renderer, m: &MemoriaJogo, bank: &Bank) -> Result<()> {
    let frame = m.u32(0x508f84)?;
    let mut c = context(renderer, m, bank)?;
    c.bob(35, 134, ((frame >> 1) & 15) + 1, false, None)?;
    c.bob(702, 0, ((frame >> 1) & 15) + 17, false, None)?;
    c.bob(528, 315, ((frame >> 1) & 7) + 33, false, None)
}
fn briefing_sprites(renderer: &mut Renderer, m: &MemoriaJogo, bank: &Bank) -> Result<()> {
    let branch = m.u8(0x5080f8 + 16 * u32::from(m.u8(0x508041)?))?;
    let phase = u32::from(m.u16(0x508f84)?) & 15;
    if branch <= 3 {
        let (x, y, id) = match branch {
            0 => (397, 484, 1),
            1 => (610, 475, 33),
            2 => (507, 473, 17),
            _ => (716, 455, 49),
        };
        context(renderer, m, bank)?.bob(x, y, id + phase, false, None)?;
    }
    Ok(())
}
/// Semantic renderer APIs select owned CMP banks directly. They need no legacy
/// heap headers or pointer-table allocations; register entries below retain
/// that explicit projection for the original routine differential.
pub fn menu_pixels(renderer: &mut Renderer, m: &MemoriaJogo, bank: &Bank) -> Result<()> {
    menu_sprites(renderer, m, bank)?;
    shell_draw::menu(renderer, m, FrameRegisters::default())?;
    Ok(())
}
pub fn briefing_pixels(renderer: &mut Renderer, m: &mut MemoriaJogo, bank: &Bank) -> Result<()> {
    briefing_sprites(renderer, m, bank)?;
    shell_draw::objectives(renderer, m, FrameRegisters::default())?;
    Ok(())
}

/// Raster body0x4176e3..0x417795 and the original AffGame epilogue only.
/// Registers supplied here are the outer AffGame entry registers.
pub fn menu(
    renderer: &mut Renderer,
    m: &mut MemoriaJogo,
    bank: &Bank,
    mut r: FrameRegisters,
) -> Result<FrameRegisters> {
    set_bank(m, 0x508f68)?;
    menu_sprites(renderer, m, bank)?;
    let table = set_bank(m, 0x508f5c)?;
    r.eax = shell_draw::menu(renderer, m, FrameRegisters { eax: table, ..r })?.eax;
    r.esp = r.esp.wrapping_add(4);
    Ok(r)
}

/// Raster body0x41782b..0x417795 and original epilogue; objective text uses
/// the established bitmap API after the invention bank is restored to game.
pub fn briefing(
    renderer: &mut Renderer,
    m: &mut MemoriaJogo,
    bank: &Bank,
    mut r: FrameRegisters,
) -> Result<FrameRegisters> {
    set_bank(m, 0x508f64)?;
    briefing_sprites(renderer, m, bank)?;
    let table = set_bank(m, 0x508f5c)?;
    r.eax = shell_draw::objectives(renderer, m, FrameRegisters { eax: table, ..r })?.eax;
    r.esp = r.esp.wrapping_add(4);
    Ok(r)
}
