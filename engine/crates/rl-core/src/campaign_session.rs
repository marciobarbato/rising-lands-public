//! Headless/host campaign orchestration over the production World transactions.
//! Host resources are immutable inputs, like renderer assets, not saved game state.
use crate::{
    campaign_flow, mission_boot,
    mission_ini::MissionIni,
    mission_load,
    player::DeclaredInput,
    render::{self, Renderer},
    shell::{self, Banks, Request, Screen},
    shell_input::Keys,
    shell_picture, FrameInput, MemoriaJogo, World,
};
use std::collections::BTreeMap;

pub trait Files {
    /// Resolve the exact original logical path. Absence is distinct from I/O failure.
    fn read(&mut self, path: &[u8]) -> crate::Result<Option<Vec<u8>>>;
}
struct Briefing<'a, F> {
    host: &'a mut F,
    opened: Option<Vec<u8>>,
    position: usize,
}
impl<F: Files> crate::shell_briefing::Files for Briefing<'_, F> {
    fn open(&mut self, p: &[u8], _: &[u8]) -> crate::Result<bool> {
        self.opened = self.host.read(p)?;
        self.position = 0;
        Ok(self.opened.is_some())
    }
    fn read(&mut self, n: usize) -> crate::Result<Vec<u8>> {
        let b = self
            .opened
            .as_ref()
            .ok_or(crate::EngineError::InvalidFramePhase(3))?;
        let data = b[self.position.min(b.len())..(self.position + n).min(b.len())].to_vec();
        self.position += data.len();
        Ok(data)
    }
    fn seek(&mut self, offset: u32, origin: u8) -> crate::Result<()> {
        if self.opened.is_none() || origin != 0 {
            return Err(crate::EngineError::UnportedCallee {
                name: "campaign host seek outside the save-slot rewind",
                address: 0x45848b,
            });
        }
        self.position = offset as usize;
        Ok(())
    }
    fn close(&mut self) -> crate::Result<()> {
        self.opened = None;
        Ok(())
    }
}
fn path(m: &MemoriaJogo, root: u32, relative: &[u8]) -> crate::Result<Vec<u8>> {
    let mut p = crate::mission_startup::string(m, root)?;
    p.extend_from_slice(relative);
    Ok(p)
}
fn required(files: &mut impl Files, p: &[u8], pc: u32) -> crate::Result<Vec<u8>> {
    files.read(p)?.ok_or(crate::EngineError::UnportedCallee {
        name: "Campaign required original file missing",
        address: pc,
    })
}

/// Explicit host acknowledgement. Unavailable includes a missing file or a host
/// without a decoder; the engine neither hides the file nor pretends to play it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MovieCompletion {
    Unavailable,
    Finished,
}
/// CalculGame/AffGame game modes: 0x12 play and 0x16 the modal in-game event
/// (FLC). Both take the gameplay path: AffGame draws them (0x4178b3 for 0x16,
/// render_event.rs) and CalculGame's game branch runs them (0x42d197 for 0x16,
/// frame.rs). Neither is a shell screen.
fn gameplay(mode: u8) -> bool {
    mode == 0x12 || mode == 0x16
}
struct PendingDefeat {
    request: Request,
    world: World,
    input: FrameInput,
    generation: u64,
}
pub struct Presenter {
    banks: Banks,
    backgrounds: BTreeMap<Vec<u8>, Vec<u8>>,
    pending_defeat: Option<PendingDefeat>,
}
impl Presenter {
    pub fn new(renderer: &Renderer, menu: &[u8], inventions: &[u8]) -> render::Result<Self> {
        Ok(Self {
            banks: Banks::load(menu, inventions, renderer)?,
            backgrounds: BTreeMap::new(),
            pending_defeat: None,
        })
    }
    /// Draws gameplay with the World consumption receipt, or the original shell.
    pub fn draw(
        &mut self,
        world: &mut World,
        renderer: &mut Renderer,
        files: &mut impl Files,
    ) -> render::Result<()> {
        let m = world.render_memory()?;
        let mode = m.u8(crate::shell_state::MODE)?;
        if gameplay(mode) {
            let outcome = renderer.render(world.render_boundary_memory(true)?.as_ref(), true)?;
            world.publish_render_effects(&outcome)?;
            if let Some((a, n)) = outcome.stop {
                return Err(render::Error::Stop(a, n));
            }
            return Ok(());
        }
        // mode0 is the native load request; keep the last briefing surface until load.
        if mode == 0 {
            return Ok(());
        }
        let screen = Screen::current(&m)?;
        let name = screen.background(m.u8(0x496fe4)?).to_vec();
        if !self.backgrounds.contains_key(&name) {
            let file = required(files, &path(&m, 0x509494, &name)?, 0x4799c4)?;
            let mut pixels = vec![0; 480000];
            if file.starts_with(b"Pie") {
                shell_picture::pie(&file, &mut pixels, 800)?;
            } else {
                if file.get(..4) != Some(b"LZSS") || file.get(780..784) != Some(b"FORM") {
                    return Err(render::Error::Invalid("original result LZS envelope"));
                }
                let size = u32::from_le_bytes(
                    file.get(784..788)
                        .ok_or(render::Error::Invalid("LZS size"))?
                        .try_into()
                        .unwrap(),
                );
                let data = file
                    .get(788..788 + size as usize)
                    .ok_or(render::Error::Invalid("LZS payload"))?;
                let mut decoded = MemoriaJogo::from_blocks([
                    (0x6000000, data.to_vec()),
                    (
                        shell_picture::RING,
                        m.read(shell_picture::RING, 4096)?.to_vec(),
                    ),
                    (0x7000000, pixels),
                ])?;
                if shell_picture::lzss(&mut decoded, 0x6000000, 0x7000000, size)? != 480000 {
                    return Err(render::Error::Invalid("LZS surface size"));
                }
                pixels = decoded.read(0x7000000, 480000)?.to_vec();
            }
            self.backgrounds.insert(name.clone(), pixels);
        }
        shell::paint_prepared(&m, renderer, &self.banks, &self.backgrounds[&name])
    }

    /// Render -> message pump/cursor -> CalculGame, matching the production host.
    /// Keys are completed key actions (the shell's existing release-wait contract).
    pub fn step(
        &mut self,
        world: &mut World,
        input: &mut FrameInput,
        mut answers: DeclaredInput,
        keys: Keys,
        renderer: &mut Renderer,
        files: &mut impl Files,
    ) -> render::Result<Request> {
        if let Some(pending) = &self.pending_defeat {
            return Ok(pending.request.clone());
        }
        self.draw(world, renderer, files)?;
        // Gameplay keeps its ordinary path. Only shell frames may suspend on
        // a host request; retain their completed source boundary for saving.
        let source = (!gameplay(world.render_memory()?.u8(crate::shell_state::MODE)?))
            .then(|| (world.clone(), *input));
        *input = world.input_step(2, *input, answers.clone())?;
        answers.messages.clear();
        let mode = world.render_memory()?.u8(crate::shell_state::MODE)?;
        if gameplay(mode) {
            *input = world.input_step(3, *input, answers)?;
            if world.render_memory()?.u8(crate::shell_state::MODE)? == 19 {
                self.draw(world, renderer, files)?;
            }
            return Ok(Request::None);
        }
        let mut m = world.project()?;
        if mode == 0 {
            let name = mission_load::mission_name(&m, m.u8(0x496fe4)?)?;
            let eta = required(files, &path(&m, 0x509594, &name)?, 0x46c2cf)?;
            let monsters = required(files, &path(&m, 0x509494, b"\\MONSTRE.INI")?, 0x47ad44)?;
            mission_boot::prepare_load_frame(&mut m)?;
            mission_load::load(&mut m, Some(&eta), &MissionIni::parse(&monsters))?;
            let carried = input.registers;
            let mut next = World::load(m, world.clock.clone())?;
            next.rules = world.rules;
            next.player_cursor = world.player_cursor;
            next.set_execution_mode(world.execution_mode())?;
            let mut output = next.load_tail_input_v1(*input, answers)?;
            output.registers = carried;
            // Complete load and first render before publishing the candidate.
            let mut pixels = renderer.clone();
            self.draw(&mut next, &mut pixels, files)?;
            *world = next;
            *input = output;
            *renderer = pixels;
            return Ok(Request::None);
        }
        let request = campaign_flow::input(&mut m, keys)?;
        match request {
            Request::None | Request::StartMission => {}
            Request::ResumeGame => {
                let events = campaign_flow::resume_game(&mut m, input)?;
                self.publish_shell(world, input, m, renderer, files)?;
                world.frame_events = events;
                return Ok(Request::None);
            }
            Request::MissionNext => {
                let expected = crate::mission_entry::next_name(world)?;
                let exists = files.read(&expected)?.is_some();
                let mut brief = Briefing {
                    host: files,
                    opened: None,
                    position: 0,
                };
                // An absent next stage publishes the original campaign-end
                // screen (mode24); its movie request is raised on the next frame.
                campaign_flow::next(&mut m, |p| Ok(p == expected && exists), &mut brief)?;
            }
            Request::DefeatMovie { path } | Request::CampaignMovie { path } => {
                let (prior, prior_input) = source.ok_or(render::Error::Invalid(
                    "defeat request outside a result-screen boundary",
                ))?;
                let request = if m.u8(crate::shell_state::MODE)? == 24 {
                    Request::CampaignMovie { path }
                } else {
                    Request::DefeatMovie { path }
                };
                self.pending_defeat = Some(PendingDefeat {
                    request: request.clone(),
                    world: world.clone(),
                    input: *input,
                    generation: prior.generation,
                });
                *world = prior;
                *input = prior_input;
                return Ok(request);
            }
            other => {
                return Err(render::Error::Stop(
                    0x42c3e4,
                    format!("Campaign shell request: {other:?}"),
                ))
            }
        }
        self.publish_shell(world, input, m, renderer, files)?;
        Ok(Request::None)
    }

    /// Resolve the pending original PlayVideo request exactly once. Repeated
    /// polling does not consume input, advance time or change a save. The same
    /// source world must be supplied; failures retain both source and request.
    pub fn finish_defeat_movie(
        &mut self,
        world: &mut World,
        input: &mut FrameInput,
        renderer: &mut Renderer,
        files: &mut impl Files,
        _completion: MovieCompletion,
    ) -> render::Result<()> {
        let (mut candidate, mut carried) = {
            let pending = self
                .pending_defeat
                .as_ref()
                .ok_or(render::Error::Invalid("no pending defeat movie request"))?;
            if world.generation != pending.generation
                || !matches!(
                    world.render_memory()?.u8(crate::shell_state::MODE)?,
                    19 | 24
                )
            {
                return Err(render::Error::Invalid("stale defeat movie source"));
            }
            (pending.world.clone(), pending.input)
        };
        let mut memory = candidate.project()?;
        // Both native branches join at42c925, then MenuInit2 and the common tail.
        campaign_flow::main_menu(&mut memory)?;
        memory.set_u8(0x5099b0, 0)?;
        self.publish_shell(&mut candidate, &mut carried, memory, renderer, files)?;
        *world = candidate;
        *input = carried;
        self.pending_defeat = None;
        Ok(())
    }

    fn publish_shell(
        &mut self,
        world: &mut World,
        input: &mut FrameInput,
        m: MemoriaJogo,
        renderer: &mut Renderer,
        files: &mut impl Files,
    ) -> render::Result<()> {
        let mut next = world.clone();
        next.replace_memory(m)?;
        next.frame_phase = 3;
        next.clock.advance(1);
        let mut pixels = renderer.clone();
        self.draw(&mut next, &mut pixels, files)?;
        input.registers.esp = input.registers.esp.wrapping_add(4);
        *world = next;
        *renderer = pixels;
        Ok(())
    }
}
