//! Persistent execution projection, reusable rollback and incremental semantic refresh.
//! The same phase routines execute in all modes. No unknown provider is skipped.
use crate::{frame_state::FrameDelta, *};
use std::borrow::Cow;
use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

impl World {
    /// 0: legacy audit; 1: play; 2: persistent execution with complete audit traces.
    pub fn set_execution_mode(&mut self, mode: u32) -> Result<()> {
        if mode > 2 || mode != 0 && self.state.frame.is_none() {
            return Err(EngineError::InvalidExecutionMode(mode));
        }
        if mode != 0 {
            let memory = self.project()?;
            self.delta = self.state.frame.as_ref().map(FrameDelta::new);
            self.projection.template = memory;
        } else {
            self.delta = None;
        }
        self.execution_mode = mode;
        self.projection.template.audit = mode != 1;
        self.frame_events.clear();
        self.player_events.clear();
        self.frame_freed = MemoriaJogo::default();
        Ok(())
    }
    pub fn execution_mode(&self) -> u32 {
        self.execution_mode
    }
    pub fn render_memory(&self) -> Result<Cow<'_, MemoriaJogo>> {
        if self.execution_mode == 0 {
            Ok(Cow::Owned(self.project()?))
        } else {
            Ok(Cow::Borrowed(&self.projection.template))
        }
    }
    fn synchronize_play(&mut self) -> Result<()> {
        let m = &self.projection.template;
        let frame = self.state.frame.as_ref().expect("play frame state");
        let current_crt = if m.optional_u32(CRT_GETTER)? == Some(SINGLE_THREAD_GETTER) {
            m.optional_u32(THREAD_DATA)?
                .map(|p| p.wrapping_add(12))
                .filter(|p| *p != 0)
        } else {
            None
        };
        if m.structural_change()
            || m.u32(0x4e19a0)? != frame.unit_pool_base()
            || current_crt != self.projection.crt_seed_address
        {
            // Rebinding is rare and remains fully checked; ordinary ticks never reload the state.
            let (state, crt, bound) = GameState::inspect(m)?;
            self.delta = state.frame.as_ref().map(FrameDelta::new);
            if !bound {
                self.execution_mode = 0;
                self.projection.template.audit = true;
            }
            self.state = state;
            self.projection.crt_seed_address = crt;
            self.projection.frame_bound = bound;
            return Ok(());
        }
        let world_rng = self
            .state
            .world_rng
            .map(|_| m.u32(WORLD_SEED).map(|seed| WorldRng { seed }))
            .transpose()?;
        let crt_rng = match (self.state.crt_rng, self.projection.crt_seed_address) {
            (Some(_), Some(address)) => Some(CrtRng {
                seed: m.u32(address)?,
            }),
            _ => None,
        };
        let delta = self.delta.as_mut().expect("play semantic scratch");
        frame.prepare_delta(m, delta)?;
        // All fallible reads completed. Publish typed values together, preserving opaque bytes.
        self.state
            .frame
            .as_mut()
            .expect("play frame state")
            .apply_delta(delta);
        self.state.world_rng = world_rng;
        self.state.crt_rng = crt_rng;
        Ok(())
    }
    pub(crate) fn play_phase(
        &mut self,
        phase: u32,
        input: FrameInput,
        mut answers: Option<player::DeclaredInput>,
    ) -> Result<FrameInput> {
        if !matches!(phase, 2 | 3) {
            return Err(EngineError::InvalidFramePhase(phase));
        }
        let is_input = answers.is_some();
        let xy = answers
            .as_ref()
            .and_then(|a| a.cursor)
            .unwrap_or(self.player_cursor);
        if let Some(a) = &mut answers {
            a.cursor = Some(xy);
        }
        let m = &mut self.projection.template;
        m.audit = self.execution_mode != 1;
        m.rules = self.rules;
        m.order_queue = self.order_queue.clone();
        m.path_searches = 0;
        m.begin();
        let attempted = catch_unwind(AssertUnwindSafe(|| {
            if let Some(answers) = answers {
                player::phase(m, phase, input, answers)
                    .map(|(out, events, consumed)| (out, events, MemoriaJogo::default(), consumed))
            } else {
                frame::phase(m, phase, input)
                    .map(|(out, events, freed)| (out, events, freed, Vec::new()))
            }
        }));
        let result = match attempted {
            Ok(result) => result,
            Err(panic) => {
                m.rollback();
                m.player = None;
                resume_unwind(panic);
            }
        };
        match result {
            Ok((output, events, freed, consumed)) => {
                if let Err(error) = self.synchronize_play() {
                    self.projection.template.rollback();
                    return Err(error);
                }
                self.projection.template.commit();
                self.order_queue = self.projection.template.order_queue.clone();
                self.generation = self.generation.wrapping_add(1);
                self.render_receipt = None;
                self.frame_phase = phase;
                self.frame_events = events;
                self.frame_freed = freed;
                if is_input {
                    self.player_events = consumed;
                    self.player_cursor = xy;
                }
                if phase == 3 {
                    self.clock.advance(1);
                }
                Ok(output)
            }
            Err(error) => {
                let failed = self.projection.template.player.take();
                self.projection.template.rollback();
                if let Some(runtime) = failed {
                    if self.execution_mode == 1 {
                        // Diagnostics on a stopped input attempt retain the ABI1.5 contract.
                        // Re-execution has only owned deterministic inputs, no external callbacks.
                        let m = &mut self.projection.template;
                        m.audit = true;
                        // Rollback restores game bytes only: the Rust-side scratch the
                        // failed attempt consumed (modern-path search budget, order
                        // queue) must start again as the attempt saw it, or the replay
                        // takes another path and reports a mismatch instead of the stop.
                        m.path_searches = 0;
                        m.order_queue = self.order_queue.clone();
                        m.begin();
                        let replay = player::phase(m, phase, input, runtime.answers);
                        let diagnostics = m.player.take().map(|r| r.events).unwrap_or_default();
                        m.rollback();
                        m.audit = false;
                        if replay.err().as_ref() != Some(&error) {
                            return Err(EngineError::ExecutionReplayMismatch);
                        }
                        self.player_events = diagnostics;
                    } else {
                        self.player_events = runtime.events;
                    }
                }
                Err(error)
            }
        }
    }
}
