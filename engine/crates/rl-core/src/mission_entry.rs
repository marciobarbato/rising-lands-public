//! Host mission entry points. Gameplay is built from shipped assets, never S0.
use crate::{
    mission_boot::{self, StartupAssets, StartupInput},
    mission_campaign,
    mission_ini::MissionIni,
    mission_load, mission_research, FrameInput, FrameRegisters, MemoriaJogo, Result, TickClock,
    World,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage {
    pub number: u8,
    pub variant: Option<u8>,
}
impl Stage {
    pub fn parse(name: &str) -> Result<Self> {
        let name = name.to_ascii_uppercase();
        let b = name.as_bytes();
        let invalid = || crate::EngineError::UnportedCallee {
            name: "Mission entry requires an original MONDExx[variant].ETA filename",
            address: 0x46c1b8,
        };
        if !(b.len() == 11 || b.len() == 12)
            || !b.starts_with(b"MONDE")
            || !b.ends_with(b".ETA")
            || !b[5..7].iter().all(u8::is_ascii_digit)
        {
            return Err(invalid());
        }
        let number = (b[5] - b'0') * 10 + b[6] - b'0';
        let variant = (b.len() == 12).then(|| b[7].wrapping_sub(b'A'));
        let allowed = match number {
            9 => 4,
            10 | 12 => 3,
            17 => 3,
            1..=25 => 0,
            _ => return Err(invalid()),
        };
        if variant.is_some_and(|v| v >= allowed) {
            return Err(invalid());
        }
        Ok(Self { number, variant })
    }
}

/// Explicit virtual display binding: the original DDInit 800x600 prefix. The
/// renderer owns surfaces; this supplies only the dimensions read by game code.
/// InitInputPlayer 0x45f32b uses five 16-byte host allocations, with the selected
/// allocator fill. No queue packet is fabricated; InputSend owns its contents.
pub fn bind_host(m: &mut MemoriaJogo, allocation_fill: u8) -> Result<()> {
    m.set_u16(0x49a018, 800)?;
    m.set_u16(0x49a01a, 600)?;
    m.set_u16(0x508726, 17)?;
    m.set_u16(0x508724, 25)?;
    m.set_u32(0x4e17a0, 800)?;
    // Software host owns an installed palette and an active WinMain session.
    // These are explicit device/lifecycle bindings, never gameplay fallbacks.
    m.set_u32(0x4e179c, 1)?;
    m.set_u8(0x49a058, 1)?;
    for i in 0..5 {
        let p = 0x07000000 + 4096 * i;
        m.declare(p, vec![allocation_fill; 16])?;
        m.set_u32(0x4e246c + 4 * i, 16)?;
        m.set_u32(0x4e2458 + 4 * i, p)?;
        m.set_u32(0x4e2f3c, m.u32(0x4e2f3c)?.wrapping_add(16))?;
    }
    Ok(())
}

/// ProgIteration's explicit host cursor and prior presentation counters. These
/// are inputs to RestoreAll; menu frame count parity changes its flip effects.
pub fn bind_frame_context(
    m: &mut MemoriaJogo,
    input: FrameInput,
    iterations: [u32; 3],
) -> Result<()> {
    m.set_u16(0x5099a8, input.cursor_x as u16)?;
    m.set_u16(0x5099aa, input.cursor_y as u16)?;
    for (slot, value) in [0x508f7c, 0x508f80, 0x508f84].into_iter().zip(iterations) {
        m.set_u32(slot, value)?;
    }
    Ok(())
}

/// Returns at MissionLoad's successful return (the S0 boundary). `eta` is an
/// explicit file selection; base 09/10/12/17 files are allowed as direct aliases,
/// just as in the 38-file differential. Campaign continuation uses MakeMissionName.
pub fn load_stage(
    assets: &StartupAssets<'_>,
    startup: StartupInput,
    stage: Stage,
    eta: &[u8],
    monsters: &[u8],
    texts: &[u8],
) -> Result<MemoriaJogo> {
    load_stage_inner(assets, startup, stage, eta, monsters, texts, None)
}

pub fn load_stage_with_events(
    assets: &StartupAssets<'_>,
    startup: StartupInput,
    stage: Stage,
    eta: &[u8],
    monsters: &[u8],
    texts: &[u8],
    events: &crate::events_host::Input<'_>,
) -> Result<MemoriaJogo> {
    events.validate()?;
    load_stage_inner(assets, startup, stage, eta, monsters, texts, Some(events))
}
/// Research of a campaign that reaches `stage` without researching anything.
/// The original has no direct stage entry: stage N is reached through N
/// briefings, each MenuInit(17) RechercheCalculNew 0x47150f at mission k, with
/// RechercheStore 0x4714f0 at every victory (CalculGame 0x42ca87) before
/// MissionNext. `mission_boot::startup` already ran k = 1; this runs k = 2..N,
/// including the stored copy 0x508134. IaRechercheAjuste adds nothing here (it
/// repeats the local player's inventions of the stage, which are 0).
pub fn stage_research(m: &mut MemoriaJogo, stage: u8) -> Result<()> {
    for k in 2..=stage {
        mission_campaign::store_research(m)?;
        m.set_u8(0x496fe4, k)?;
        mission_research::recalculate(m)?;
    }
    Ok(())
}
fn load_stage_inner(
    assets: &StartupAssets<'_>,
    startup: StartupInput,
    stage: Stage,
    eta: &[u8],
    monsters: &[u8],
    texts: &[u8],
    events: Option<&crate::events_host::Input<'_>>,
) -> Result<MemoriaJogo> {
    let mut m = if let Some(host) = events {
        mission_boot::startup_with_events(assets, startup, host)?
    } else {
        mission_boot::startup(assets, startup)?
    };
    stage_research(&mut m, stage.number)?;
    bind_host(&mut m, startup.allocation_fill)?;
    m.set_u8(0x496fe4, stage.number)?;
    if let Some(v) = stage.variant {
        if stage.number == 9 {
            m.set_u8(0x4e1d85, v)?;
        } else {
            let players = m.u32(0x5080a8)?;
            for p in 1..4 {
                m.set_u16(
                    players + 20 * p + 12,
                    if p == u32::from(v) + 1 { 65516 } else { 50 },
                )?;
            }
        }
    }
    mission_load::load(&mut m, Some(eta), &MissionIni::parse(monsters))?;
    crate::text::init_langue(&mut m, texts, 0)?;
    Ok(m)
}

/// Program start into the original main menu: ProgInit's gameplay portion, the
/// host display binding, the original text table and MenuInit(local player,2).
/// CalculGame mode1 (0x42c8bf) first offers the intro movie to the host; that
/// presentation is the host's choice and is not a game rule, so it is not run
/// here. No campaign stores, RNG draws or stage loads happen before the menu.
pub fn boot_main_menu(
    assets: &StartupAssets<'_>,
    startup: StartupInput,
    texts: &[u8],
    events: Option<&crate::events_host::Input<'_>>,
) -> Result<MemoriaJogo> {
    let mut m = if let Some(host) = events {
        mission_boot::prog_init_with_events(assets, startup, host)?
    } else {
        mission_boot::prog_init(assets, startup)?
    };
    bind_host(&mut m, startup.allocation_fill)?;
    crate::text::init_langue(&mut m, texts, 0)?;
    let player = m.u8(0x508041)?;
    mission_load::require(
        crate::campaign_flow::menu_init(&mut m, player, 2, &mut crate::campaign_flow::NoFiles)?,
        "Program start main menu entry",
        0x42c931,
    )?;
    Ok(m)
}

/// Run the already-ported native load tail, then install the host's explicit
/// carried WinMain context. The clock advances exactly once for this load frame.
pub fn enter(
    memory: MemoriaJogo,
    clock: TickClock,
    input: FrameInput,
    carried: FrameRegisters,
    mode: u32,
) -> Result<(World, FrameInput)> {
    let mut world = World::load(memory, clock)?;
    world.player_cursor = (input.cursor_x, input.cursor_y);
    let mut input = if mode == 1 {
        world.set_execution_mode(mode)?;
        world.load_tail_input_v1(input, crate::player::DeclaredInput::default())?
    } else {
        world.frame_step(1, input)?
    };
    input.registers = carried;
    world.set_execution_mode(mode)?;
    Ok((world, input))
}

/// Preview the actual original path, without changing the world or consuming RNG.
pub fn next_name(world: &World) -> Result<Vec<u8>> {
    let mut m = world.project()?;
    let mut name = Vec::new();
    mission_campaign::next(&mut m, |path| {
        name = path.to_vec();
        Ok(true)
    })?;
    Ok(name)
}

/// Confirm victory and load the host-resolved next ETA. Failed loads publish no
/// world/input mutation. The UI has explicitly confirmed the research/briefing
/// screen; this performs its RechercheCalculNew before the next load prelude.
/// Missing files are an explicit host error, never a substituted stage/default.
pub fn continue_victory(
    world: &World,
    name: &[u8],
    eta: &[u8],
    monsters: &[u8],
    input: FrameInput,
) -> Result<(World, FrameInput)> {
    let mut m = world.project()?;
    mission_load::require(
        m.u8(0x4e05a0)? != 0 && m.u8(0x4e1d84)? != 0,
        "Mission continuation requires victory",
        0x42ca6e,
    )?;
    let mut matched = false;
    mission_campaign::advance_after_victory(&mut m, |path| {
        matched = path == name;
        Ok(matched)
    })?;
    mission_load::require(matched, "MissionNext host filename mismatch", 0x46c08c)?;
    // MenuInit(17) 0x4684d4 clears the victory latch; neither MissionNext nor
    // MissionLoad does, so a stale 1 would reach the next stage.
    m.set_u8(0x4e1d84, 0)?;
    mission_research::recalculate(&mut m)?;
    m.set_u8(0x4e2f78, 0)?; // explicit host confirmation of the between-stage UI
    mission_boot::prepare_load_frame(&mut m)?;
    mission_load::load(&mut m, Some(eta), &MissionIni::parse(monsters))?;
    let mut next = World::load(m, world.clock.clone())?;
    next.rules = world.rules;
    next.difficulty_setup = Some(next.clock.tick());
    next.player_cursor = world.player_cursor;
    next.set_execution_mode(world.execution_mode)?;
    Ok((next, input))
}

/// Load only after actual mode17 input requested mode0. MissionNext, research
/// preparation and the user's research choice already belong to this world;
/// repeating them here would overwrite the chosen branch and advance the stage.
/// Returns at MissionLoad's return. The host still executes the public load tail.
pub fn load_prepared(
    world: &World,
    name: &[u8],
    eta: &[u8],
    monsters: &[u8],
    input: FrameInput,
) -> Result<(World, FrameInput)> {
    let mut m = world.project()?;
    mission_load::require(
        m.u8(crate::shell_state::MODE)? == 0,
        "Prepared mission load requires briefing confirmation",
        0x42caa9,
    )?;
    let mut expected = crate::mission_startup::string(&m, 0x509594)?;
    expected.extend(mission_load::mission_name(&m, m.u8(0x496fe4)?)?);
    mission_load::require(
        name == expected,
        "Prepared mission filename mismatch",
        0x46c1b8,
    )?;
    mission_boot::prepare_load_frame(&mut m)?;
    mission_load::load(&mut m, Some(eta), &MissionIni::parse(monsters))?;
    let mut next = World::load(m, world.clock.clone())?;
    next.rules = world.rules;
    // Fresh MissionLoad boundary: one stage-start difficulty application is allowed.
    next.difficulty_setup = Some(next.clock.tick());
    next.player_cursor = (input.cursor_x, input.cursor_y);
    if world.execution_mode != 0 {
        next.set_execution_mode(world.execution_mode)?;
    }
    Ok((next, input))
}
