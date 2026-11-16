//! No-key InputGame/MenuInit and published display logic effects, translated
//! from entrada_quadro_ia.py and efeitos_logicos_exibicao.py. Pixels omitted.
use crate::{
    frame::{binding, boundary, stop, EventLog, FrameInput},
    frame_core, MemoriaJogo, Result,
};
fn guard(m: &MemoriaJogo, a: u32, site: u32, name: &'static str) -> Result<()> {
    if m.u8(a)? != 0 {
        stop(name, site)
    } else {
        Ok(())
    }
}
pub fn icons(m: &mut MemoriaJogo) -> Result<u32> {
    let local = u32::from(m.u8(0x508041)?);
    if m.u16(0x508728 + (local << 9))? == 1 {
        return stop("CalculIconeAction selected unit", 0x45e962);
    }
    if m.u16(0x495be0 + 2 * local)? != 0xffff {
        return stop("CalculIconeAction selected structure", 0x45e9bf);
    }
    for i in 0..10 {
        let r = 0x4e2410 + 6 * i;
        if m.u16(r)? != 0x29b1 || m.u16(r + 2)? != 1 || m.u16(r + 4)? != 0 {
            return stop("CalculIconeAction icon slot", 0x45ea64);
        }
        m.set_u16(r, 0x29b1)?;
        m.set_u16(r + 2, 1)?;
        m.set_u16(r + 4, 0)?;
    }
    m.set_u8(0x496d48, 255)?;
    Ok(10)
}
fn zone(m: &MemoriaJogo) -> Result<u16> {
    let x = i64::from(m.u16(0x5099a8)?);
    let y = i64::from(m.u16(0x5099aa)?);
    let mut z = m.u16(0x50a9b4)?;
    while z <= m.u16(0x50a9b6)? {
        let r = 0x5099b4 + 16 * u32::from(z);
        if i64::from(m.u32(r)? as i32) <= x
            && i64::from(m.u32(r + 4)? as i32) <= y
            && x <= i64::from(m.u32(r + 8)? as i32)
            && y <= i64::from(m.u32(r + 12)? as i32)
        {
            return Ok(z);
        }
        z = z.wrapping_add(1);
        if z == 0 {
            return stop("MouseZone wrapping zone range", 0x479cac);
        }
    }
    Ok(0)
}
pub(crate) fn rain(m: &mut MemoriaJogo) -> Result<()> {
    let wind = m.u16(0x496d2e)? as i16;
    if wind >= 0 {
        return stop("NextGouttes non-negative wind", 0x45ac56);
    }
    let strength = m.u8(0x496d2e)?.wrapping_neg();
    let mode = m.u8(0x496d2c)?;
    if mode == 2 {
        return stop("NextGouttes mode 2", 0x45ac6b);
    }
    if mode != 1 {
        return Ok(());
    }
    let denominator = 5 - i32::from(strength);
    if denominator == 0 {
        return stop("NextGouttes division zero", 0x45ad5e);
    }
    for i in 0..120 {
        let r = 0x4e1da0 + 6 * i;
        let y = (i32::from(m.u16(r + 2)? as i16) + 20) % 446;
        m.set_u16(r + 2, y as u16)?;
        let x = (i32::from(m.u16(r)? as i16) - 20 / denominator + 510) % 510;
        m.set_u16(r, x as u16)?;
        m.set_u8(r + 4, m.u8(r + 4)?.wrapping_add(1) % 4)?;
    }
    Ok(())
}
fn hover(
    m: &mut MemoriaJogo,
    player: u8,
    info: &mut u32,
    info2: &mut u32,
    delayed: &mut bool,
) -> Result<()> {
    let y = u32::from(m.u16(0x5099aa)?);
    let x = u32::from(m.u16(0x5099a8)?);
    let c =
        (((y >> 5) + u32::from(m.u16(0x508722)?)) << 8) + (x >> 5) + u32::from(m.u16(0x508720)?);
    let margin = i64::from(m.u8(0x508f90)? as i8) * 32;
    if i64::from(x) >= i64::from(m.u16(0x508724)?) * 32 - margin
        || i64::from(y) >= i64::from(m.u16(0x508726)?) * 32 - margin
        || i64::from(x) < margin
        || i64::from(y) < margin
    {
        return Ok(());
    }
    let c = c & 65535;
    if crate::input_hover_undiscovered::route(m, c, player)?
        == crate::input_hover_undiscovered::Route::Finish
    {
        return Ok(());
    }
    let obj = m.u8(m.u32(0x507d74)?.wrapping_add(c))?;
    if obj >= 0x42 {
        return stop("TerrainEstSac bag", 0x4766c9);
    }
    if let Some(message) = crate::terrain_gc_hover::lookup(m, c)? {
        *info = message;
    } else if let Some(mine) = crate::terrain_mine::lookup(m, c)? {
        *info = mine.message;
        *info2 = mine.capacity;
        *delayed = false;
    } else if obj == 0x46 {
        // TerrainEstPiege 0x4762de: the owner's own trap shows message 0x33.
        if m.u8(m.u32(0x507dac)?.wrapping_add(c))? == player {
            *info = crate::player::text(m, 0x33, 0x429a46)?;
            *info2 = 65535;
            *delayed = false;
        }
    }
    if frame_core::call(m, "CaseEstIlluminee", &[c, u32::from(player)])? as u8 != 0 {
        if m.u8(m.u32(0x4e0f38)?.wrapping_add(2 * c + 1))? & 128 != 0 {
            return stop("InputGame structure hover", 0x429a8c);
        }
        m.set_u16(0x496ac4, 65535)?;
        let pool = m.u32(0x4e19a0)?;
        for i in 0..800 {
            let r = pool.wrapping_add(276 * i);
            if [0, 4].contains(&m.u8(r + 2)?) || m.u8(r + 0x72)? == 0 {
                continue;
            }
            let filter = 1 + u32::from(m.u8(r + 0xab)? != 0);
            if crate::frame_units::call(m, "EnginOccupeCase", &[i, c, filter])? as u8 != 0 {
                return stop("ChercheIndiceEngin unit found", 0x44671c);
            }
        }
        m.set_u16(0x496ac2, 65535)?;
    }
    if [9, 12, 11, 3, 13, 5, 10, 1, 2, 7].contains(&m.u16(0x4e05ae)?) {
        return stop("InputGame hover help page", 0x429b8c);
    }
    let selected = 0x508728 + (u32::from(player) << 9);
    if m.u16(selected)? == 0
        && m.u16(0x495be0 + 2 * u32::from(player))? == 65535
        && m.u8(0x4e19b0)? == 0
        && m.u8(0x5099ae)? != 0
    {
        return stop("InputGame hover held button", 0x42a6fd);
    }
    guard(m, 0x4e19b0, 0x42b85d, "InputGame hover flag")?;
    if m.u16(selected)? != 0 {
        return stop("InputGame selected troops hover", 0x42a762);
    }
    if m.u16(0x496ac2)? != 65535 {
        return stop("InputGame unit info", 0x42a793);
    }
    if m.u16(0x496ac4)? != 65535 {
        return stop("InputGame structure info", 0x42ac1b);
    }
    Ok(())
}
pub fn input(m: &mut MemoriaJogo, player: u8, events: &mut EventLog) -> Result<()> {
    let mut info = 0;
    let mut info2 = 65535;
    let mut delayed = true;
    m.set_u16(0x49a010, 0x1dd8)?;
    m.set_u16(0x4e0612, 0)?;
    m.set_u16(0x4e0614, 0)?;
    m.set_u32(0x49a014, 0)?;
    m.set_u16(0x4e0616, 0)?;
    m.set_u8(0x508f8f, 0)?;
    m.set_u8(0x4e17ac, 0)?;
    m.set_u16(0x4e0618, 0)?;
    for a in [0x496d4a, 0x496ac2, 0x496ac4] {
        m.set_u16(a, 65535)?;
    }
    binding(events, 2, 0x429300, &[0x1b], 0);
    for (a, site) in [
        (0x5099ae, 0x42934d),
        (0x5099ae, 0x42935e),
        (0x4e05a8, 0x42937b),
        (0x5099b1, 0x4293a1),
    ] {
        guard(m, a, site, "InputGame mouse/button guard")?;
    }
    m.set_u8(0x4e19b0, 0)?;
    let z = zone(m)?;
    m.set_u16(0x5099ac, z)?;
    guard(m, 0x49a05a, 0x4293dc, "InputGame focus flag")?;
    binding(events, 2, 0x429556, &[0x10], 0);
    if m.u16(0x4e05ae)? == 12 {
        return stop("InputGame help page12", 0x429573);
    }
    guard(m, 0x5099b1, 0x429585, "InputGame mouse button")?;
    guard(m, 0x5099b1, 0x4295aa, "InputGame mouse button")?;
    binding(events, 2, 0x4295fc, &[0x11], 0);
    if [8, 9].contains(&m.u16(0x4e05ae)?) {
        return stop("InputGame help page8/9", 0x42966c);
    }
    binding(events, 2, 0x42967e, &[0x2e], 0);
    if m.u16(0x4e05ae)? == 7 {
        return stop("InputGame help page7", 0x42969b);
    }
    binding(events, 2, 0x4296ad, &[0x12], 0);
    if m.u16(0x4e05ae)? == 2 {
        return stop("InputGame help page2", 0x4296ca);
    }
    for (vk, site) in [
        (0x20, 0x4296dc),
        (0x26, 0x4296f1),
        (0x28, 0x42970b),
        (0x27, 0x429732),
        (0x25, 0x42974c),
        (0x0d, 0x429773),
        (0x58, 0x429781),
    ] {
        binding(events, 2, site, &[vk], 0);
    }
    if m.u16(0x4e05ae)? == 13 {
        return stop("InputGame help page13", 0x4297a2);
    }
    if z >= 10 {
        return stop("InputGame zone>=10", 0x4297cd);
    }
    if z == 0 {
        hover(m, player, &mut info, &mut info2, &mut delayed)?;
    } else if z > 8 {
        return stop("InputGame zone9", 0x42c29f);
    } else {
        guard(m, 0x5099ae, 0x42ba8c, "InputGame scroll button")?;
        m.set_u32(0x49a014, 0)?;
        m.set_u16(0x49a010, z.wrapping_add(0x1dd7))?;
        if z != 1 {
            return stop("ScrollTerrain direction", 0x472048 + 4 * u32::from(z - 1));
        }
        for a in [0x508720, 0x508722] {
            let v = m.u16(a)?;
            if v > 0 {
                m.set_u16(a, v - 1)?;
            }
        }
    }
    crate::terrain_gc_hover::publish(m, info, info2, delayed)?;
    if m.u8(0x496d2c)? != 0 {
        rain(m)?;
    }
    Ok(())
}
fn cstring(m: &MemoriaJogo, a: u32) -> Result<Vec<u8>> {
    let mut v = Vec::new();
    let mut p = a;
    loop {
        let b = m.u8(p)?;
        if b == 0 {
            return Ok(v);
        }
        v.push(b);
        p = p.wrapping_add(1);
        if p == a {
            return stop("MenuInit unterminated path", a);
        }
    }
}
pub fn menu(
    m: &mut MemoriaJogo,
    input: &FrameInput,
    events: &mut EventLog,
    mut delete: impl FnMut(&mut MemoriaJogo, &mut EventLog, u32) -> Result<()>,
) -> Result<()> {
    let current = m.u8(0x4e2f78)?;
    if current == 0x12 {
        return Ok(());
    }
    if [6, 7, 10, 1, 19, 24].contains(&current) {
        return stop("MenuInit source mode", 0x4682f5);
    }
    binding(events, 1, 0x46837a, &[0], 0);
    m.set_u32(0x508f70, input.offscreen)?;
    let bank = m.u32(0x508f64)?;
    if bank != 0 {
        delete(m, events, bank)?;
        m.set_u32(0x508f64, 0)?;
    }
    if m.u32(0x508f68)? != 0 {
        return stop("MenuInit bank2", 0x4683c3);
    }
    // 0x4683d2..0x4683e4: with the cursor-outside flag 0x49a054 set (AffGame,
    // cursor in the viewport margin) MenuInit calls USER32 ShowCursor(TRUE).
    // That is host presentation only: no game memory is written and the flag
    // itself stays as it is (receipt menu_cursor-v1).
    // FilledBox writes pixels only. DDLock/Unlock bookkeeping restores itself.
    guard(m, 0x4e2f79, 0x46840e, "MenuInit asset mode")?;
    let path = cstring(m, 0x509494)?;
    let mut text = path.clone();
    text.extend(cstring(m, 0x494a4d)?);
    text.push(0);
    let mut stack = [0u32; 4];
    let mut mask = 0;
    for (k, word) in stack.iter_mut().enumerate() {
        if 4 * k < text.len() {
            let mut b = [0; 4];
            let n = (text.len() - 4 * k).min(4);
            b[..n].copy_from_slice(&text[4 * k..4 * k + n]);
            *word = u32::from_le_bytes(b);
            mask |= 1 << k;
        }
    }
    let ecx = 0u32
        .wrapping_sub(2)
        .wrapping_sub(u32::try_from(path.len()).map_err(|_| crate::EngineError::AddressOverflow)?);
    boundary(
        m,
        events,
        101,
        0x46847b,
        [
            input.registers.esp.wrapping_sub(4 + 0x14 + 0x138),
            m.u32(0x508f70)?,
            0,
            ecx,
        ],
        stack,
        mask,
    )?;
    let ebx = frame_core::call(m, "InitGameZone", &[1, 0])?;
    let i = m.u8(0x4e1d18)?;
    let track = 2 + u32::from(m.u8(0x4e1c18 + u32::from(i))? % 2);
    m.set_u8(0x4e1d18, i.wrapping_add(1))?;
    boundary(
        m,
        events,
        102,
        0x4684bd,
        [track, 0, (ebx & !255) | u32::from(i.wrapping_add(1)), 2],
        stack,
        mask,
    )?;
    m.set_u8(0x4e2f78, 0x12)?;
    guard(m, 0x508f90, 0x468c76, "MenuInit viewport margin")?;
    m.set_u32(0x4e2f70, 0)?;
    m.set_u16(0x4e2f40, 0)?;
    m.set_u8(0x508f90, m.u8(0x4e2f7a)?)?;
    if m.player.as_ref().is_some_and(|runtime| runtime.phase == 1) {
        crate::load_tail_input::menu_flip_effects_v1(m)?;
    }
    Ok(())
}
// Exact sampled-origin domain of the accepted Graphics effect provider.
const ORIGINS: &[(u16, u16)] = &[
    (0, 0),
    (0, 1),
    (0, 2),
    (0, 6),
    (0, 7),
    (0, 12),
    (0, 14),
    (0, 18),
    (0, 21),
    (0, 24),
    (0, 28),
    (0, 30),
    (0, 35),
    (0, 36),
    (0, 42),
    (0, 47),
    (4, 6),
    (4, 30),
    (1, 3),
    (2, 4),
    (3, 5),
    (4, 44),
    (6, 0),
    (6, 7),
    (6, 14),
    (6, 21),
    (6, 28),
    (6, 35),
    (6, 42),
    (8, 0),
    (8, 6),
    (8, 12),
    (8, 18),
    (8, 24),
    (8, 30),
    (8, 36),
    (8, 42),
    (8, 47),
    (12, 0),
    (12, 7),
    (12, 14),
    (12, 21),
    (12, 28),
    (12, 35),
    (12, 40),
    (12, 42),
    (16, 0),
    (16, 6),
    (16, 12),
    (16, 18),
    (16, 24),
    (16, 30),
    (16, 36),
    (16, 42),
    (16, 47),
    (18, 0),
    (18, 7),
    (18, 14),
    (18, 21),
    (18, 28),
    (18, 35),
    (18, 42),
    (24, 0),
    (24, 6),
    (24, 7),
    (24, 12),
    (24, 14),
    (24, 18),
    (24, 21),
    (24, 24),
    (24, 28),
    (24, 30),
    (24, 35),
    (24, 36),
    (24, 42),
    (24, 47),
    (30, 0),
    (30, 7),
    (30, 14),
    (30, 21),
    (30, 28),
    (30, 35),
    (30, 42),
    (32, 0),
    (32, 6),
    (32, 12),
    (32, 18),
    (32, 24),
    (32, 30),
    (32, 36),
    (32, 42),
    (32, 47),
    (36, 0),
    (36, 7),
    (36, 14),
    (36, 21),
    (36, 28),
    (36, 35),
    (36, 42),
    (39, 0),
    (39, 6),
    (39, 12),
    (39, 18),
    (39, 24),
    (39, 30),
    (39, 36),
    (39, 42),
    (39, 47),
];
const AFFGAME_ORIGINS: &[(u16, u16)] = &[
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 0),
    (7, 0),
    (9, 0),
    (10, 0),
    (10, 16),
    (11, 0),
    (13, 0),
    (14, 0),
    (15, 0),
    (17, 0),
    (19, 0),
    (19, 1),
    (19, 2),
    (19, 3),
    (19, 4),
    (19, 5),
    (19, 6),
    (19, 7),
    (19, 8),
    (19, 9),
    (19, 10),
    (19, 11),
    (19, 12),
    (19, 13),
    (19, 14),
    (19, 15),
    (19, 16),
    (19, 17),
    (19, 18),
    (19, 19),
    (31, 0),
];
pub fn display_effects(m: &mut MemoriaJogo, flips: u32) -> Result<()> {
    // RestoreAll474b08 draws terrain twice, never AffSprite. Physical map,
    // viewport, camera, cache and backing checks apply independently of mission.
    // Facades, units and effects still meet their own real renderer boundaries.
    if flips == 2 {
        crate::display_stage_domain::restore_terrain_flip_effects(m)?;
        return Ok(());
    }
    display_effects_for(m, flips, false)
}
pub(crate) fn display_effects_for(m: &mut MemoriaJogo, flips: u32, affgame: bool) -> Result<()> {
    if (m.u16(0x508724)?, m.u16(0x508726)?) != (25, 17) {
        return stop("Display effect viewport", 0x4178f7);
    }
    let origin = (m.u16(0x508720)?, m.u16(0x508722)?);
    let prior = ORIGINS.contains(&origin) || affgame && AFFGAME_ORIGINS.contains(&origin);
    let extended = m.player.is_some()
        && (if affgame {
            crate::chain_display_origins::AFFGAME
        } else {
            crate::chain_display_origins::RESTOREALL
        })
        .contains(&origin)
        && (m.u16(0x497168)?, m.u16(0x49716a)?) == (64, 64)
        && m.u8(0x508041)? == 0
        && m.u8(0x4e2f78)? == 18;
    // AffGame selects terrain by viewport geometry, never by mission number.
    // AffSprite scans the physical 256-stride backing with u16 truncation;
    // each object body still has its own addressed renderer boundary.
    let scene_profile = affgame && m.player.is_some() && m.u8(0x4e2f78)? == 18;
    let scene_buildings = if scene_profile {
        Some(crate::display_stage_domain::scene_building_indices(m)?)
    } else {
        None
    };
    if !(prior || extended || scene_profile) {
        return stop(
            if m.player.is_some() {
                "AffGame"
            } else {
                "Display effect sampled origin"
            },
            0x415b77,
        );
    }
    let mut occupied = 0;
    let mut single = true;
    let mut pair_indices = true;
    let mut static_type1 = true;
    let mut has_type1 = false;
    let mut single_cave = true;
    let mut scene_parts = true;
    for i in 0..1000 {
        // Native AffSprite never visits off-camera or undiscovered facades.
        // Legacy finite display profiles retain their original full-pool guard.
        if scene_buildings
            .as_ref()
            .is_some_and(|indices| !indices[i as usize])
        {
            continue;
        }
        let r = 0x49a0c8 + 276 * i;
        let state = m.u8(r + 5)?;
        // Display states above 7 take every facade's construction branch (the
        // bodies test 5, 3 and 4, then jne), which draws no permutation jitter.
        if state == 0 {
            continue;
        }
        occupied += 1;
        single &= m.u8(r + 8)? == 2;
        pair_indices &= i < 2;
        let kind = m.u8(r + 8)?;
        has_type1 |= kind == 1;
        let collapsing_cave = crate::dismantle_scene::admits(m, i)?;
        // A damaged CAVE (+6 != 0) draws the same body followed by the pure
        // AffFlamme type-3 tail in modes 3..5 (409991..4099a1); the mode-3
        // collapse jitters of an uninhabited CAVE are published by the renderer
        // like the other facades. Owned CAVEs (owners 0..4) in modes 3..5 draw the
        // inhabited body or the uninhabited one, both with their own renderer stops.
        let checked_cave = collapsing_cave
            || kind == 3
                && (state != 3 || scene_profile)
                && (m.u8(r + 4)? == 200 && m.u16(r + 10)? <= 100
                    || m.u8(r + 4)? <= 4 && matches!(state, 3..=5));
        single_cave &= checked_cave;
        // AffBatiments dispatches each facade independently. These existing
        // non-collapse bodies have no gameplay writes; their pixels/clip and
        // remaining consumers are checked at the actual renderer call. Pool
        // indices, building count and combinations are not original rules.
        // Native-proven terrain3f8/406/436 (types18/10/11) non-collapse draws
        // are pure here; pixel/clip and fire dependencies stay renderer-owned.
        // Terrain390 (type17) has no owner palette, RNG or world write outside
        // mode3; its modes/health/owners are native-proven on the MONDE23
        // consumer-path world (facade390). Terrain3ce (types15/20; the type-20 record
        // adds AffUpgradeMonastere) is native-proven on the MONDE17/17A
        // consumer-path worlds (facade3ce); both only clip and draw.
        // AffFlamme0x40323c (the shared fire tail0x40d5e2 after these facades)
        // only pastes flames: no RNG or world write. Burning facades whose
        // renderer runs that tail are admitted, types1/2 included (their
        // modes3..5 push 1/2 into the same tail; construction modes skip it).
        // The type-9 wall branch (407ea2..407fa2) never reads +6 and has no
        // flame tail, so a damaged wall draws exactly like an intact one.
        // Mode3 (collapse) of these facades is drawn by their renderers with
        // the three permutation jitters published as render effects; it is
        // native-proven per facade (facade_native/flame_native matrices).
        // Terrain3b0 (type8 hangar, facade3b0) is native-proven on the MONDE09A/
        // MONDE10C worlds: clip, draw, mode-3 jitters and the type-8 flame tail.
        // Terrain343 (type5 workshop, facade343) and terrain326 (type13,
        // facade326): clip, draw, jitters and flame tail like the others; the
        // workshop's inhabited smoke also draws LongRand (CRT rand), whose seed
        // store the renderer publishes with the draw (Outcome::crt_seed).
        // The wall branch (407ea2..407fa2) never reads the display state, so a
        // collapsing wall draws no jitter either.
        let renderer_collapse = scene_profile && matches!(kind, 0..=11 | 13..=21);
        scene_parts &= checked_cave
            || (m.u16(r + 6)? == 0 || matches!(kind, 0 | 1 | 2 | 4..=11 | 13..=21))
                && (state != 3 || renderer_collapse)
                && (matches!(kind, 0 | 1 | 2 | 4..=8 | 10 | 11 | 13 | 14 | 15 | 17 | 18 | 19 | 20 | 21)
                    // 0x407ea2: the wall branch never reads the display state.
                    || kind == 9
                    // facade37a (type16): modes4/5 native-proven for owners0..4 and
                    // every health word on the MONDE17C worlds; the mode-3 collapse
                    // and the construction modes (facade37a_modes.rs) likewise.
                    || kind == 16);
        // Native AffBatiments0x407dd3 dispatches each facade independently.
        // The type1 non-collapse port and completed CAVE have no RNG/world
        // effects beyond clipping, which the renderer publishes explicitly.
        // The native-proved inhabited CAVE fire/gauge draws are pure; all
        // other fire and collapse domains retain their existing boundaries.
        static_type1 &= checked_cave
            || m.u16(r + 6)? == 0
                && (matches!(kind, 1 | 2) && state != 3
                    || kind == 4 && state != 3 && m.u8(r + 4)? == 200);
        if state == 3 && !collapsing_cave && !renderer_collapse {
            return stop("BatimentEstHabiter display mode3", 0x40863e);
        }
    }
    let cave = 0x49a0c8 + 276;
    let pair = (affgame || m.player.is_some())
        && occupied == 2
        && pair_indices
        && m.u8(0x508041)? == 0
        && m.u8(0x49a0c8 + 8)? == 2
        && matches!(m.u8(0x49a0c8 + 5)?, 4 | 5)
        && m.u8(0x49a0c8 + 4)? == 0
        && m.u16(0x49a0c8 + 6)? == 0
        && m.u16(0x49a0c8 + 10)? == 600
        && m.u8(cave + 8)? == 3
        && (m.u16(cave + 6)? == 0
            || m.u8(cave + 4)? == 0
                && m.u8(cave + 5)? == 5
                && m.u16(cave + 10)? == 100
                && m.u16(cave + 6)? <= 100)
        && ((m.u8(cave + 5)? == 2 && m.u8(cave + 4)? == 200 && m.u16(cave + 10)? <= 100)
            || (m.u8(cave + 5)? == 5
                && matches!(m.u8(cave + 4)?, 0 | 200)
                && m.u16(cave + 10)? == 100));
    // The new terrain0x2cb facade has no logic-side display effects in these
    // non-collapse modes. Its actual pixels/clipping are checked by rendering.
    if !(occupied <= 1 && single
        || matches!(occupied, 1 | 2) && single_cave
        || pair
        || scene_profile && scene_parts
        || has_type1 && static_type1)
    {
        return stop("AffGame", 0x407dd3);
    }
    // Monster count does not drive display effects. Each live body is
    // dispatched and domain-checked by the renderer, including late-S1 armies.
    let pool = m.u32(0x5080f0)?;
    for i in 0..100 {
        let p = pool.wrapping_add(56 * i);
        if m.u8(p + 2)? != 0 {
            // These native hit/spawn draws have no world/RNG effects, and the
            // renderer draws every active entry, not only slot zero.
            let kind = m.u8(p)?;
            // Native 0x4703fb/0x47044d uses both 32-frame effect banks.
            // ProjectileAvance clears kinds31/32 at phase32, not phase16.
            // Kinds18/19 share the stationary-explosion body of kinds6..8
            // (0x46fb9e -> 0x46fbb6, cleared at phase32): their 32-sprite
            // families 0x2115/0x2135 + phase are drawn for phases16..31 too.
            let limit = if matches!(kind, 6..=8 | 18 | 19 | 24 | 25 | 31 | 32) {
                32
            } else {
                16
            };
            // Kinds17..22 are plain PasteBobCmp sprites (0x2355 family) with
            // the phase added to the base; native-proven on the MONDE17B
            // turret-shot world for phases0..31 (projectile_native.py).
            // Kinds1..5 (arrows/missiles, 4702..: 0x2a7c/0x2a80/0x2a84 + phase,
            // kinds2..5 raised by +0x24>>8) are proven on the MONDE12A/MONDE17
            // worlds of main d283fc5d.
            // Kinds 26..30, 33 and 34 (0x470076, 0x4701dc, 0x470371, 0x4703da,
            // 0x4700f2): 28 and 33 draw nothing natively, the others are plain
            // sprites whose families the renderer draws from the phase.
            let supported = matches!(kind, 9..=13 | 23 | 28 | 33)
                || matches!(kind, 26 | 27 | 29 | 30 | 34)
                || matches!(kind, 1..=8 | 14..=22 | 24 | 25 | 31 | 32) && m.u8(p + 0x18)? < limit;
            if !supported {
                return stop(
                    if m.player.is_some() {
                        "AffGame"
                    } else {
                        "Display projectile domain"
                    },
                    0x46ff70,
                );
            }
        }
    }
    for _ in 0..flips {
        let n = m.u8(0x4dfb44)?;
        if n != 0 {
            m.set_u8(0x4e1d18, m.u8(0x4e1d18)?.wrapping_add(1))?;
            if m.u8(0x508f7c)? & 1 == 0 {
                m.set_u8(0x4dfb44, n - 1)?;
            }
        }
    }
    Ok(())
}
pub fn affgame(m: &mut MemoriaJogo) -> Result<u32> {
    if m.u8(0x4e2f78)? == 0x16 {
        return affgame_event(m);
    }
    if m.u8(0x4e2f78)? != 0x12 {
        return stop("AffGame mode", 0x4176a8);
    }
    // 0x4178fc: an event running at entry draws its first EvenementAffiche
    // inline (0x417c8a) and switches to mode 22 at the end (0x417ec7).
    let event = m.u8(0x4e1b0e)? != 0;
    let flag = m.u8(0x4e05a2)?;
    let counter = m.u32(0x508f7c)?;
    if flag == 0 {
        let divisor = u32::from(m.u8(0x497284)?);
        if divisor == 0 {
            return stop("AffGame divisor zero", 0x417921);
        }
        if counter % divisor != 0 {
            return Ok(counter / divisor);
        }
    }
    // 0x41792b..0x41794b: the clear itself is drawn by the renderer, which
    // reads the flag from the pre-AffGame boundary memory.
    if m.u8(0x508f92)? != 0 && m.u16(0x49a018)? == 800 {
        m.set_u8(0x508f92, 0)?;
    }
    if m.u16(0x496204)? != 65535 {
        return stop("AffGame marker animation", 0x417ba2);
    }
    display_effects_for(m, u32::from(m.u8(0x49a05b)?) + 1, true)?;
    if event {
        crate::event_flc::affiche(m)?;
    }
    // MessageBoxAffiche follows EvenementAffiche in AffGame's tail; its
    // InitGameZone(1) is game memory (mouse zone 27, the notification icon).
    crate::event_flc::message_box_zones(m)?;
    if m.player.is_some() {
        // AffGame's post-flip cursor handoff, 0x417db2..0x417e78.
        // The software cursor uses this flag on the *following* draw. ShowCursor
        // itself belongs to host presentation; no OS callback runs in the engine.
        let margin = i32::from(m.u8(0x508f90)? as i8) * 32;
        let x = i32::from(m.u16(0x5099a8)?);
        let y = i32::from(m.u16(0x5099aa)?);
        let right = i32::from(m.u16(0x508724)?) * 32 - margin;
        let bottom = i32::from(m.u16(0x508726)?) * 32 - margin;
        match m.u8(0x49a054)? {
            0 if x < margin || x > right || y < margin || y > bottom => {
                m.set_u8(0x49a054, 1)?;
            }
            1 if x > margin && x < right && y > margin && y < bottom => {
                m.set_u8(0x49a054, 0)?;
            }
            _ => {}
        }
    }
    m.set_u8(0x4e05a2, 0)?;
    // 0x417e7e..0x417ec7: the periodic palette pass is skipped while an event
    // that was running at entry still runs; one that ended takes it at once.
    let running = m.u8(0x4e1b0e)? != 0;
    let palette = if event { !running } else { counter % 5 == 0 };
    let eax = if palette {
        if m.u32(0x4e179c)? == 0 {
            return stop("DDSetPalette CreatePalette", 0x43ee95);
        }
        m.set_u8(0x4e05a2, 1)?;
        0
    } else {
        counter / 5
    };
    if running {
        m.set_u8(0x4e2f78, 0x16)?;
    }
    Ok((eax & !255) | u32::from(m.u8(0x4e1b0e)?))
}
/// AffGame mode 22 (0x4178b3): the modal event. Surface lock/unlock and the
/// FLC frames are host presentation; EvenementAffiche's engine part, the
/// DDFlipPage gameplay prefix, then back to mode 18 with DDSetPalette
/// (0x43ee31, palette 0x4e24b0) once the event has ended. EAX is the
/// EvenementEnCours byte (the caller consumes no other bits).
fn affgame_event(m: &mut MemoriaJogo) -> Result<u32> {
    crate::event_flc::affiche(m)?;
    crate::load_tail_input::menu_flip_effects_v1(m)?;
    let running = m.u8(0x4e1b0e)?;
    if running == 0 {
        m.set_u8(0x4e2f78, 0x12)?;
        if m.u32(0x4e179c)? == 0 {
            return stop("DDSetPalette CreatePalette", 0x43ee95);
        }
    }
    Ok(u32::from(running))
}
