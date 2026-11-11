//! Core construction terrain/colour stores and Economy's builder tick.
use crate::{
    frame::stop,
    player::{alive, structure, unit, Memory},
    player_orders, MemoriaJogo, Result,
};
const FIRST: [u32; 22] = [
    0x2fa, 0x2be, 0x2dc, 0x306, 0x393, 0x333, 0x415, 0x3d7, 0x39f, 0, 0x403, 0x433, 0, 0x31f,
    0x357, 0x3c3, 0x36f, 0x38f, 0x3ef, 0x383, 0x3c3, 0x409,
];
const DECAL: [u32; 22] = [
    0x201, 0x300, 0x101, 0x202, 0x100, 0x202, 0x100, 0x300, 0x202, 0x100, 0, 0, 0x100, 0x200,
    0x300, 0x100, 0x100, 1, 0x201, 0x100, 0x100, 0x100,
];
fn neighbors(m: &MemoriaJogo, c: u32) -> Result<[Option<u32>; 4]> {
    let (x, y) = ((c & 255) as i32, (c >> 8) as i32);
    let (w, h) = (m.w(0x497168)? as i32, m.w(0x49716a)? as i32);
    Ok(
        [(x - 1, y), (x, y - 1), (x + 1, y), (x, y + 1)].map(|(x, y)| {
            if x >= 0 && x < w && y >= 0 && y < h {
                Some((x + 256 * y) as u32)
            } else {
                None
            }
        }),
    )
}
fn allocate_color(m: &mut MemoriaJogo, removing: bool) -> Result<u32> {
    let n = m.b(0x4e1570)?;
    if n == 0 {
        return Ok(0);
    }
    let c = m.b(0x4e1570 + n)?;
    // MurRetire inlines allocation at 0x43e9fe: flag before count.
    // Standalone AlloueCouleur (0x43e5a0) has the reverse order.
    if removing {
        m.sb(0x4e1670 + c, 0)?;
        m.sb(0x4e1570, n - 1)?;
    } else {
        m.sb(0x4e1570, n - 1)?;
        m.sb(0x4e1670 + c, 0)?;
    }
    Ok(c)
}
fn free_color(m: &mut MemoriaJogo, c: u32) -> Result<()> {
    let n = m.b(0x4e1570)?;
    if n == 255 || m.b(0x4e1670 + c)? != 0 || c == 0 {
        return Ok(());
    }
    m.sb(0x4e1570, n + 1)?;
    m.sb(0x4e1570 + n + 1, c)?;
    m.sb(0x4e1670 + c, 1)
}
fn fill(m: &mut MemoriaJogo, start: u32, color: u32) -> Result<()> {
    if color == 0 {
        return Ok(());
    }
    let grid = m.d(0x4e156c)?;
    let old = m.b(grid + start)?;
    if old == color {
        let mut seen = vec![start];
        let mut border = vec![start];
        while !border.is_empty() && seen.len() < 3 {
            let c = border.pop().expect("nonempty");
            for v in neighbors(m, c)?.into_iter().flatten() {
                if !seen.contains(&v) && m.b(grid + v)? == old {
                    seen.push(v);
                    border.push(v)
                }
            }
        }
        if seen.len() == 2 {
            return stop("Remplit same-color pair does not terminate", 0x43e464);
        }
    }
    let mut top = 1;
    m.sw(0x4e0732 + 2, start)?;
    while top != 0 {
        let c = m.w(0x4e0732 + 2 * top)?;
        top -= 1;
        if m.b(grid + c)? == old {
            m.sb(grid + c, color)?
        }
        for v in neighbors(m, c)?.into_iter().flatten() {
            if m.b(grid + v)? == old && top < 1023 {
                top += 1;
                m.sw(0x4e0732 + 2 * top, v)?
            }
        }
        if top == 1023 {
            return Ok(());
        }
    }
    Ok(())
}
fn add_wall(m: &mut MemoriaJogo, c: u32) -> Result<()> {
    let n = neighbors(m, c)?;
    let grid = m.d(0x4e156c)?;
    let mut old = [0; 4];
    for (i, v) in n.iter().enumerate() {
        if let Some(v) = v {
            old[i] = m.b(grid + v)?
        }
    }
    let color = allocate_color(m, false)?;
    for (i, v) in n.into_iter().enumerate() {
        if let Some(v) = v {
            if m.b(grid + v)? != 0 && m.b(grid + v)? == old[i] {
                fill(m, v, color)?
            }
        }
    }
    m.sb(grid + c, color)?;
    for c in old {
        free_color(m, c)?
    }
    Ok(())
}
fn wall_object(m: &MemoriaJogo, c: u32) -> Result<bool> {
    let o = m.b(m.d(0x507d74)?.wrapping_add(c))?;
    Ok(o == m.b(0x495bf8)? || o == m.b(0x495bf9)?)
}
pub fn gate(m: &MemoriaJogo, c: u32) -> Result<u32> {
    let c = c & 65535;
    let (x, y) = (c & 255, c >> 8);
    if x > 0 && wall_object(m, c - 1)? && wall_object(m, c + 1)? {
        return Ok(0);
    }
    if y > 0 && wall_object(m, c - 256)? {
        return Ok(if wall_object(m, c + 256)? { 1 } else { 255 });
    }
    Ok(255)
}
/// Grid untag with zone merge (0x43e138 mode 1 then MurRetire 0x43e8b7): the
/// cell must still carry this structure's label; the saved word is restored.
pub(crate) fn untag_cell(m: &mut MemoriaJogo, s: u32, cell: u32, sub: u32) -> Result<bool> {
    let c = cell & 65535;
    let grid = m.d(0x4e0f38)?;
    if m.w(grid + 2 * c)? != (0x8000 | (s << 5) | sub) {
        return Ok(false);
    }
    m.sw(grid + 2 * c, m.w(structure(s) + 0xcc + 2 * sub)?)?;
    let colors = m.d(0x4e156c)?;
    if m.b(colors + c)? != 0 {
        let neighbors = neighbors(m, c)?;
        let mut old = [0; 4];
        for (i, n) in neighbors.iter().enumerate() {
            if let Some(n) = n {
                old[i] = m.b(colors + n)?
            }
        }
        m.sb(colors + c, 0)?;
        for (i, n) in neighbors.into_iter().enumerate() {
            if let Some(n) = n {
                if m.b(colors + n)? != 0 && m.b(colors + n)? == old[i] {
                    let color = allocate_color(m, true)?;
                    fill(m, n, color)?
                }
            }
        }
        for color in old {
            free_color(m, color)?
        }
    }
    Ok(true)
}
/// Grid tag with zone split (0x43e138 mode 3 then MurAjoute 0x43e616).
pub(crate) fn tag_cell(m: &mut MemoriaJogo, s: u32, cell: u32, sub: u32) -> Result<()> {
    let grid = m.d(0x4e0f38)?;
    let old = m.w(grid + 2 * cell)?;
    if old < 0x4000 {
        m.sw(structure(s) + 0xcc + 2 * sub, old)?;
        m.sw(m.d(0x4e0f38)? + 2 * cell, 0x8000 | (s << 5) | sub)?;
        add_wall(m, cell)?
    }
    Ok(())
}
pub fn place(m: &mut MemoriaJogo, s: u32) -> Result<u32> {
    if s >= 1000 {
        return stop("PlaceBatiment index", 0x41c4ac);
    }
    let r = structure(s);
    let t = m.b(r + 8)?;
    if t > 21 {
        return stop("PlaceBatiment type", 0x41c4ac);
    }
    if matches!(t, 9 | 12) {
        let cell = m.w(r + 2)?;
        if t == 9 {
            let mut bits: u32 = 0;
            for (d, bit) in [(-1i32, 8), (256, 4), (1, 2), (-256, 1)] {
                if wall_object(m, cell.wrapping_add(d as u32))? {
                    bits |= bit
                }
            }
            if m.b(m.d(0x507d74)? + cell)? == m.b(0x495bf9)? {
                bits = 0
            } else {
                bits = bits.saturating_sub(1);
            }
            m.sw(r + 0x7c, bits)?;
            m.sb(m.d(0x507d74)? + cell, m.b(0x495bf8)?)?;
        } else {
            let orientation = gate(m, cell)?;
            if orientation == 255 {
                return Ok(255);
            }
            m.sw(r + 0x7c, orientation)?;
            m.sb(m.d(0x507d74)? + cell, m.b(0x495bf9)?)?;
            m.sw(r + 0x86, 0)?;
        }
        let a = m.d(0x507da8)? + cell;
        m.sb(a, m.b(a)? | 1)?;
        tag_cell(m, s, cell, 0)?;
        return Ok(0);
    }
    let tdef = 0x495d8c + 52 * t;
    let (w, h) = (m.w(tdef)?, m.w(tdef + 2)?);
    let mask = m.d(tdef + 8)?;
    let mut c = m.w(r + 2)?;
    let mut tile = FIRST[t as usize];
    let mut sub = 0;
    for row in 0..h {
        for col in 0..w {
            let cell = c & 65535;
            m.sw(m.d(0x507d70)? + 2 * cell, tile)?;
            tile = (tile + 1) & 65535;
            if m.b(mask + row * w + col)? != 0 {
                let a = m.d(0x507da8)? + cell;
                m.sb(a, m.b(a)? | 1)?;
                let grid = m.d(0x4e0f38)?;
                let old = m.w(grid + 2 * cell)?;
                if old < 0x4000 {
                    m.sw(r + 0xcc + 2 * sub, old)?;
                    m.sw(m.d(0x4e0f38)? + 2 * cell, 0x8000 | (s << 5) | sub)?;
                    add_wall(m, cell)?
                }
                sub = (sub + 1) & 255;
            }
            m.sb(m.d(0x507d74)? + cell, 0)?;
            c = c.wrapping_add(1)
        }
        c = c.wrapping_add(256).wrapping_sub(w)
    }
    Ok(h)
}
pub fn occupies_mode(m: &mut MemoriaJogo, u: u32, c: u32, filter: u32) -> Result<bool> {
    let r = unit(m, u)?;
    let layer = m.b(r + 0xab)?;
    if (layer != 0 && filter == 1) || (layer == 0 && filter == 2) || m.b(r + 2)? == 14 {
        return Ok(false);
    }
    if m.b(r + 1)? > 25 {
        return Ok(true);
    }
    Ok(crate::frame_units::call(m, "EnginOccupeCase", &[u, c, filter])? & 255 != 0)
}
pub fn occupies(m: &mut MemoriaJogo, u: u32, c: u32) -> Result<bool> {
    occupies_mode(m, u, c, 1)
}
pub fn combination(m: &MemoriaJogo, c: u32) -> Result<u32> {
    let c = c & 65535;
    let mut bits: u32 = 0;
    for (d, b) in [(-1i32, 8), (256, 4), (1, 2), (-256, 1)] {
        if wall_object(m, c.wrapping_add(d as u32))? {
            bits |= b
        }
    }
    if m.b(m.d(0x507d74)? + c)? == m.b(0x495bf9)? {
        Ok(0)
    } else {
        Ok(bits.saturating_sub(1))
    }
}
/// SetMurAllentour 0x41c251 (EAX = cell, the completed wall's +2): for the
/// west, east, north and south neighbours, in that order, whose object byte is
/// the wall code 0x495bf8 (a gate code is not refreshed) and whose region word
/// holds a structure, store ChercheMurCombinaisons(neighbour) as that record's
/// word +0x7c (high byte zero). Object bytes are read at base + cell +/- d
/// unmasked, region words and the combination at the 16-bit neighbour.
pub(crate) fn wall_surround(m: &mut MemoriaJogo, c: u32) -> Result<()> {
    let c = c & 65535;
    for d in [-1i32, 1, -256, 256] {
        let n = c.wrapping_add(d as u32);
        if m.b(m.d(0x507d74)?.wrapping_add(n))? != m.b(0x495bf8)? {
            continue;
        }
        let n = n & 65535;
        let v = m.w(m.d(0x4e0f38)?.wrapping_add(2 * n))?;
        if v & 0x8000 == 0 {
            continue;
        }
        let index = (v & 0x7fe0) >> 5;
        if index >= 1000 {
            return stop("SetMurAllentour structure index beyond pool", 0x41c2c7);
        }
        let k = combination(m, n)? & 255;
        m.sw(structure(index) + 0x7c, k)?;
    }
    Ok(())
}
pub fn reset_terrain(m: &mut MemoriaJogo, s: u32) -> Result<u32> {
    let r = structure(s);
    let t = m.b(r + 8)?;
    if t > 21 {
        return stop("ResetCheminTerrainModify type", 0x41b78f);
    }
    let (w, h) = (m.w(0x495d8c + 52 * t)?, m.w(0x495d8e + 52 * t)?);
    let mut cell = m.w(r + 2)?;
    let mut sub = 0;
    for row in 0..h {
        for col in 0..w {
            if crate::frame_core::call(m, "BatimentCollision", &[t, row * w + col])? & 255 != 0 {
                let c = cell & 65535;
                let grid = m.d(0x4e0f38)?;
                if m.w(grid + 2 * c)? == (0x8000 | (s << 5) | sub) {
                    m.sw(grid + 2 * c, m.w(r + 0xcc + 2 * sub)?)?;
                    let colors = m.d(0x4e156c)?;
                    if m.b(colors + c)? != 0 {
                        let neighbors = neighbors(m, c)?;
                        let mut old = [0; 4];
                        for (i, n) in neighbors.iter().enumerate() {
                            if let Some(n) = n {
                                old[i] = m.b(colors + n)?
                            }
                        }
                        m.sb(colors + c, 0)?;
                        for (i, n) in neighbors.into_iter().enumerate() {
                            if let Some(n) = n {
                                if m.b(colors + n)? != 0 && m.b(colors + n)? == old[i] {
                                    let color = allocate_color(m, true)?;
                                    fill(m, n, color)?
                                }
                            }
                        }
                        for color in old {
                            free_color(m, color)?
                        }
                    }
                }
                sub = (sub + 1) & 255;
            }
            cell = cell.wrapping_add(1)
        }
        cell = cell.wrapping_add(256).wrapping_sub(w)
    }
    Ok(h)
}
pub fn constructible(m: &mut MemoriaJogo, t: u32, c: u32, u: u32) -> Result<bool> {
    if t == 12 {
        let cell = c & 65535;
        if m.b(m.d(0x507d74)? + cell)? != m.b(0x495bf8)? {
            return Ok(false);
        }
        let v = m.w(m.d(0x4e0f38)? + 2 * cell)?;
        if v & 0x8000 == 0 {
            return Ok(false);
        }
        let s = (v & 0x7fe0) >> 5;
        return Ok(m.b(unit(m, u)?)? == m.b(structure(s) + 4)?);
    }
    let (w, h) = (m.w(0x495d8c + 52 * t)?, m.w(0x495d8e + 52 * t)?);
    let mut c = c
        .wrapping_sub(w.wrapping_sub(1))
        .wrapping_sub(h.wrapping_sub(1) << 8);
    for _ in 0..h {
        for _ in 0..w {
            let cell = c & 65535;
            if m.b(m.d(0x507da8)? + cell)? & 1 != 0 && !occupies(m, u, cell)? {
                return Ok(false);
            }
            c = c.wrapping_add(1)
        }
        c = c.wrapping_add(256).wrapping_sub(w)
    }
    Ok(true)
}
pub fn debit(m: &mut MemoriaJogo, p: u32, rid: u32, cost: u32) -> Result<()> {
    let a = 0x4e2ea8 + (p * 4 + rid) * 8;
    let total = m.w(a + 2)?;
    m.sw(a + 2, total.saturating_sub(cost & 65535))?;
    let rmi = m.w(a)?;
    m.sw(a, rmi.saturating_sub(cost & 65535))
}
fn indicator(m: &mut MemoriaJogo, p: u32, divide_site: u32) -> Result<()> {
    let a = 0x4dfb2c + 6 * p;
    let d = m.w(a)?;
    if d == 0 {
        return Ok(());
    }
    let r = 0x4e2ea8 + 32 * p;
    let food = m.w(r + 10)?.wrapping_sub(m.w(r + 8)?) & 65535;
    let stone = m.w(r + 18)?.wrapping_sub(m.w(r + 16)?) & 65535;
    let d2 = m.w(a + 2)?;
    if d2 == 0 {
        return stop("CreerBatiment indicator division by zero", divide_site);
    }
    m.sb(a + 4, (((food + stone) & 65535) / d) * 7 / d2)
}
fn reset_bag(m: &mut MemoriaJogo, a: u32, cap: u32) -> Result<()> {
    for off in [0, 0x66, 4, 2] {
        m.sw(a + off, 0)?
    }
    for i in 0..30 {
        m.sw(a + 6 + 2 * i, 0)?;
        m.sb(a + 0x42 + i, 0)?
    }
    m.sw(a + 2, cap)
}
/// CreerBatiment gate branch 0x4200da..0x420253: the first live wall record
/// (type 9, in pool order, the new record included) whose footprint holds the
/// gate cell under BatimentCollision is cleared field by field and its terrain
/// path restored (ResetCheminTerrainModify 0x41b78f). No wall: nothing happens.
fn gate_replaces_wall(m: &mut MemoriaJogo, cell: u32) -> Result<()> {
    for s in 0..1000 {
        let q = structure(s);
        if m.b(q + 5)? == 0 || m.b(q + 8)? != 9 {
            continue;
        }
        let (w, h) = (m.w(0x495d8c + 52 * 9)?, m.w(0x495d8e + 52 * 9)?);
        let origin = m.w(q + 2)?;
        let mut found = false;
        'scan: for row in 0..h {
            for col in 0..w {
                // 0x420166: the 32-bit sum is compared with the zero-extended gate word.
                if (row << 8) + origin + col == cell
                    && crate::frame_core::call(
                        m,
                        "BatimentCollision",
                        &[9, (w * row + col) & 65535],
                    )? & 255
                        != 0
                {
                    found = true;
                    break 'scan;
                }
            }
        }
        if !found {
            continue;
        }
        m.sb(q + 5, 0)?;
        m.sw(q + 0x78, 0xffff)?;
        m.sw(q + 0x7c, 0)?;
        m.sw(q + 0x7e, 0)?;
        m.sw(q + 0x0a, 0)?;
        m.sw(q + 0x82, 0)?;
        m.sb(q, 0xff)?;
        m.sb(q + 0x74, 0)?;
        m.sb(q + 0x75, 0)?;
        m.sb(q + 0x76, 0)?;
        m.sb(q + 0x7b, 0xff)?;
        m.sw(q + 0x80, 0)?;
        m.sd(q + 0x8c, 0)?;
        reset_terrain(m, s)?;
        return Ok(());
    }
    Ok(())
}
pub fn create(m: &mut MemoriaJogo, c: u32, t: u32, p: u32) -> Result<u32> {
    let mut s = 0;
    while s < 1000 && m.b(structure(s) + 5)? != 0 {
        s += 1
    }
    if s == 1000 {
        return Ok(65535);
    }
    let def = 0x495d8c + 52 * t;
    let (food, stone) = (m.w(def + 12)?, m.w(def + 14)?);
    if !player_orders::resources(m, p, food, stone, 0)? {
        return Ok(65535);
    }
    debit(m, p, 1, food)?;
    indicator(m, p, 0x41ffd0)?;
    debit(m, p, 2, stone)?;
    indicator(m, p, 0x42004e)?;
    let (w, h) = (m.w(def)?, m.w(def + 2)?);
    let r = structure(s);
    m.sb(r + 5, 2)?;
    m.sb(r + 4, 200)?;
    m.sw(
        r + 2,
        c.wrapping_sub((h.wrapping_sub(1) << 8).wrapping_add(w).wrapping_sub(1)),
    )?;
    if t == 17 {
        m.sb(r + 4, p)?
    }
    if t == 12 {
        gate_replaces_wall(m, m.w(r + 2)?)?;
    }
    m.sb(r + 8, t)?;
    m.sw(r + 10, 1)?;
    m.sw(r + 6, 0)?;
    place(m, s)?;
    reset_bag(m, r + 12, 0)?;
    if matches!(t, 9 | 12) {
        m.sb(r + 4, p)?
    }
    reset_bag(m, r + 12, m.b(def + 18)?)?;
    Ok(s)
}
pub fn builder(m: &mut MemoriaJogo, u: u32) -> Result<u32> {
    let r = unit(m, u)?;
    if m.b(r + 0x89)? != 0 && m.b(structure(m.w(r + 0xc0)?) + 5)? == 3 {
        crate::frame_spawn::order(m, u, m.w(r + 0x7e)? as u16)?;
        return Ok(0x454ccd);
    }
    if m.b(r + 1)? != 7 {
        return Ok(0x454ccd);
    }
    let progress = m.b(r + 0x89)?;
    if progress == 1 {
        progress_tick(m, u)?;
        return Ok(0x454ccd);
    }
    if progress != 0 {
        return Ok(0x454ccd);
    }
    let state = m.b(r + 2)?;
    if state != 11 {
        if state == 15 {
            return stop("CalculConstruireBatiment", 0x44b15d);
        }
        return Ok(0x454ccd);
    }
    let t = m.b(r + 0xbe)?;
    if matches!(t, 9 | 12) {
        return wall_builder(m, u, r, t);
    }
    let pos = m.w(r + 8)?;
    if m.w(r + 0x7e)? != pos || m.b(r + 0x86)? != 0 || m.b(r + 0x87)? != 0 {
        crate::frame_movement::displace(m, u)?;
        return Ok(0x454ccd);
    }
    if pos != m.w(r + 0xb2)? {
        m.sb(r + 0xbe, 255)?;
        m.sw(r + 0xb2, 65535)?;
        m.sw(r + 0xc0, 65535)?;
        m.sb(r + 0x89, 0)?;
        if alive(m, u)? {
            m.sb(r + 2, 7)?;
            crate::frame_spawn::release(m, u)?
        }
        m.sb(r + 0x88, 0)?;
        return Ok(0x454ccd);
    }
    if t == 255 {
        return Ok(0x454ccd);
    }
    let c = (pos + DECAL.get(t as usize).copied().unwrap_or(0)) & 65535;
    if !constructible(m, t, c, u)? {
        return Ok(0x454ccd);
    }
    let s = create(m, c, t, m.b(r)?)?;
    m.sw(r + 0xc0, s)?;
    if s == 65535 {
        if alive(m, u)? {
            m.sb(r + 2, 7)?;
            crate::frame_spawn::release(m, u)?
        }
        return Ok(0x454ccd);
    }
    m.sb(r + 0x89, 1)?;
    m.sb(r + 0x8b, 20)?;
    m.sb(r + 0xa9, 0)?;
    m.sb(r + 0xa4, 0)?;
    m.sb(r + 0x90, if t == 17 { 7 } else { 3 })?;
    Ok(0x454ccd)
}
/// CalculConstruireBatiment 0x44abbf..0x44b0a7 for walls and gates (types 9
/// and 12), state 11, kind 7, work phase 0: the builder stands next to the
/// site. At the site it first steps to a free adjacent cell; once adjacent
/// and still, it starts construction at +0xb2, facing it. An unbuildable wall
/// takes the next waypoint of the queue +0xc4 (+0xd8 entries) or stops.
fn wall_builder(m: &mut MemoriaJogo, u: u32, r: u32, t: u32) -> Result<u32> {
    let pos = m.w(r + 8)?;
    let site = m.w(r + 0xb2)?;
    if pos == site && m.b(r + 0x86)? == 0 && m.b(r + 0x87)? == 0 {
        let c = free_adjacent(m, u, pos)?;
        if c != 0xffff {
            m.sw(r + 0x7e, c)?;
            crate::frame_spawn::release(m, u)?;
            m.sb(r + 0x88, 0)?;
        }
    }
    if pos == site || !builder_adjacent(m, r, site)? || m.b(r + 0x86)? != 0 || m.b(r + 0x87)? != 0 {
        crate::frame_movement::displace(m, u)?;
        return Ok(0x454ccd);
    }
    if !constructible(m, t, site, u)? {
        if t != 9 {
            return Ok(0x454ccd);
        }
        let n = m.w(r + 0xd8)?;
        if n != 0 {
            let q = m.w(r + 0xc4)?;
            m.sw(r + 0x7e, q)?;
            crate::frame_spawn::release(m, u)?;
            m.sb(r + 0x88, 0)?;
            m.sb(r + 0x89, 0)?;
            m.sw(r + 0xc0, 0xffff)?;
            m.sb(r + 2, 0xb)?;
            m.sw(r + 0xb2, q)?;
            for i in 1..n {
                m.sw(r + 0xc2 + 2 * i, m.w(r + 0xc4 + 2 * i)?)?;
            }
            m.sw(r + 0xd8, n.wrapping_sub(1))?;
            return Ok(0x454ccd);
        }
        m.sb(r + 0xbe, 255)?;
        m.sw(r + 0xb2, 0xffff)?;
        m.sw(r + 0xc0, 0xffff)?;
        m.sb(r + 0x89, 0)?;
        if alive(m, u)? {
            m.sb(r + 2, 7)?;
            crate::frame_spawn::release(m, u)?;
        }
        m.sb(r + 0x88, 0)?;
        return Ok(0x454ccd);
    }
    let s = create(m, site, t, m.b(r)?)?;
    m.sw(r + 0xc0, s)?;
    if s == 65535 {
        if alive(m, u)? {
            m.sb(r + 2, 7)?;
            crate::frame_spawn::release(m, u)?;
        }
        return Ok(0x454ccd);
    }
    m.sb(r + 0x89, 1)?;
    m.sb(r + 0x8b, 20)?;
    m.sb(r + 0xa9, 0)?;
    m.sb(r + 0xa4, 0)?;
    m.sb(
        r + 0x90,
        u32::from(crate::frame_path::direction(pos as u16, site as u16, false)),
    )?;
    Ok(0x454ccd)
}
/// EnginCaseAdjacent 0x4467a1 for a builder (kind 7): the 3x3 around `target`
/// (clipped to 0..=255 on each axis) holds the unit, tested by
/// EnginOccupeCaseEffective 0x442586 with BL = 1 (0x32 at +0xab or state 14
/// never occupy).
fn builder_adjacent(m: &MemoriaJogo, r: u32, target: u32) -> Result<bool> {
    if m.b(r + 0xab)? == 0x32 || m.b(r + 2)? == 14 {
        return Ok(false);
    }
    let (x, y) = ((target & 255) as i32, ((target >> 8) & 255) as i32);
    let pos = m.w(r + 8)? as i32;
    for dy in -1..=1 {
        if !(0..256).contains(&(y + dy)) {
            continue;
        }
        for dx in -1..=1 {
            if (0..256).contains(&(x + dx)) && pos == (x + dx) + ((y + dy) << 8) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
/// GetCaseLibreAdjacente 0x44512e: the first passable cell (EnginPeutPasser,
/// BL = 1, no occupant array) of the 3x3 around `c` (row by row from the
/// clamped corner, skipping the centre slot); 0xffff if none.
fn free_adjacent(m: &MemoriaJogo, u: u32, c: u32) -> Result<u32> {
    let (w, h) = (m.w(0x497168)?, m.w(0x49716a)?);
    let (mut x0, mut y0) = (c & 255, (c >> 8) & 255);
    x0 = x0.saturating_sub(1);
    y0 = y0.saturating_sub(1);
    for i in 0..9 {
        if i == 4 {
            continue;
        }
        let (x, y) = (x0 + i % 3, y0 + i / 3);
        if x >= w || y >= h {
            continue;
        }
        let cell = x + (y << 8);
        if crate::chain_search::passable(m, u, cell)? {
            return Ok(cell);
        }
    }
    Ok(0xffff)
}
pub(crate) fn anim(m: &mut MemoriaJogo, u: u32) -> Result<()> {
    let r = unit(m, u)?;
    let t = m.b(r + 1)?;
    if !matches!(t, 6 | 7 | 14) || m.b(r + 0x89)? != 1 {
        return Ok(());
    }
    let (period, count) = match t {
        7 => (m.b(r + 0xa7)?, m.b(r + 0xa5)?),
        6 => (m.b(r + 0xa7)?, 4),
        _ => (m.b(r + 0xa8)?, m.b(r + 0xa6)?),
    };
    let a = (m.b(r + 0xa9)? + 1) & 255;
    m.sb(r + 0xa9, a)?;
    if a != period {
        return Ok(());
    }
    if count == 0 {
        return stop("AnimConstruireBatiment division by zero", 0x447e12);
    }
    m.sb(r + 0xa9, 0)?;
    m.sb(r + 0xa4, (m.b(r + 0xa4)? + 1) % count)
}
fn builder_sound(m: &mut MemoriaJogo, u: u32, code: u32) -> Result<()> {
    let r = unit(m, u)?;
    let c = m.w(r + 8)?;
    let local = m.b(0x508041)?;
    if m.b(m.d(0x507d78 + 4 * local)? + c)? != 0 {
        return Ok(());
    }
    let v = player_orders::unit_volume(m, u)?;
    m.sb(r + 0xf9, 0)?;
    if m.d(r + 0x104)? == u32::MAX {
        crate::player::sound(
            m,
            if code == 0x25 { 0x44b33d } else { 0x44b483 },
            true,
            [0x50, (m.b(r + 1)? << 16) | (code << 8), r + 0x104, 0],
            Some(v),
        )?;
    }
    Ok(())
}
fn progress_tick(m: &mut MemoriaJogo, u: u32) -> Result<()> {
    let r = unit(m, u)?;
    if m.b(r + 0x90)? != m.b(r + 0x91)? {
        crate::frame_movement::call(m, "RotateEngin", &[u])?;
        return Ok(());
    }
    anim(m, u)?;
    let s = m.w(r + 0xc0)?;
    let sr = structure(s);
    let t = m.b(sr + 8)?;
    let limit = m.w(0x495d90 + 52 * t)?;
    if m.w(sr + 10)? == limit && m.w(sr + 6)? == 0 {
        let n = m.w(r + 0xd8)?;
        if m.b(r + 0xbe)? == 9 && n != 0 {
            let next = m.w(r + 0xc4)?;
            m.sw(r + 0x7e, next)?;
            crate::frame_spawn::release(m, u)?;
            m.sb(r + 0x88, 0)?;
            m.sb(r + 0x89, 0)?;
            m.sw(r + 0xc0, 65535)?;
            m.sb(r + 2, 11)?;
            m.sw(r + 0xb2, next)?;
            for i in 1..n {
                m.sw(r + 0xc2 + 2 * i, m.w(r + 0xc4 + 2 * i)?)?
            }
            m.sw(r + 0xd8, n - 1)?;
            return Ok(());
        }
        if m.b(0x508041)? == m.b(r)? {
            builder_sound(m, u, 18)?
        }
        m.sb(r + 0xbe, 255)?;
        m.sw(r + 0xb2, 65535)?;
        if alive(m, u)? {
            m.sb(r + 2, 7)?;
            crate::frame_spawn::release(m, u)?
        }
        m.sw(r + 0x7e, m.w(r + 0x7c)?)?;
        crate::frame_spawn::release(m, u)?;
        return m.sb(r + 0x88, 0);
    }
    let n = m.b(r + 0x8b)?;
    if n != 0 {
        m.sb(r + 0x8b, n - 1)?
    }
    if m.b(r + 0x8b)? != 0 || m.b(r + 0xa4)? != 2 {
        return Ok(());
    }
    let c = crate::frame_movement::call(m, "CalculDirCase", &[m.w(r + 8)?, m.b(r + 0x91)?])?;
    let seed = crate::frame_core::random(m)?;
    crate::frame_combat::call(
        m,
        "ProjectileInit",
        &[
            if seed & 1 != 0 { 16 } else { 15 },
            c & 65535,
            c & 65535,
            0,
            0,
            0,
            0,
            0,
            0,
        ],
    )?;
    if m.w(sr + 6)? != 0 {
        let d = m.w(sr + 6)?;
        m.sw(sr + 6, d.saturating_sub(2))?
    } else {
        m.sw(sr + 10, m.w(sr + 10)?.wrapping_add(2))?
    }
    if m.b(sr + 4)? == m.b(0x508041)? && m.d(sr + 0x100)? == u32::MAX {
        let v = player_orders::volume(m, m.w(sr + 2)?)?;
        crate::player::sound(
            m,
            0x41cfbc,
            true,
            [0x42, (t << 16) | 0x2500, sr + 0x100, 0],
            Some(v),
        )?;
    }
    if m.w(sr + 10)? >= limit {
        m.sb(sr + 5, 5)?;
        m.sw(sr + 10, limit)?;
        if t == 17 {
            // 0x41d000: BatimentHabitable (type 17: technology, +0x84 = 200).
            crate::chain_habitation::habitable(m, s)?;
        }
        if t == 9 {
            // 0x41d012: SetMurAllentour(+2), then BatimentModifyMap.
            wall_surround(m, m.w(sr + 2)?)?;
        }
        crate::chain_vision::structure_minimap(m, s as u16, 1)?;
    }
    m.sb(r + 0x8b, 20)?;
    if m.b(r)? == m.b(0x508041)? {
        builder_sound(m, u, 0x25)?
    }
    Ok(())
}
