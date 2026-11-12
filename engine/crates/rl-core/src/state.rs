use crate::{EngineError, MemoriaJogo, Result, TickClock};

pub const WORLD_SEED: u32 = 0x497164;
pub const CRT_GETTER: u32 = 0x4990cc;
pub const THREAD_DATA: u32 = 0x522244;
pub const SINGLE_THREAD_GETTER: u32 = 0x48e7c7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldRng {
    pub seed: u32,
}
impl WorldRng {
    pub fn draw(&mut self) -> u32 {
        let quotient = self.seed / 127773;
        let remainder = self.seed % 127773;
        let mut next = remainder
            .wrapping_mul(16807)
            .wrapping_sub(quotient.wrapping_mul(2836));
        if next == 0 || next & 0x80000000 != 0 {
            next = next.wrapping_add(0x7fffffff);
        }
        self.seed = next;
        next
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrtRng {
    pub seed: u32,
}
impl CrtRng {
    pub fn draw(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(0x41c64e6d).wrapping_add(0x3039);
        self.seed.wrapping_shr(16) & 0x7fff
    }
}
/// Four unpacked bytes, units first. The thousands byte may exceed9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BcdDigits(pub [u8; 4]);
impl BcdDigits {
    pub fn encode(value: u16) -> Self {
        let mut remaining = u32::from(value);
        let mut digits = [0; 4];
        for (index, divisor) in [(3, 1000), (2, 100), (1, 10), (0, 1)] {
            // Each quotient is <=65; intentional original-byte width.
            digits[index] = (remaining / divisor) as u8;
            remaining %= divisor;
        }
        Self(digits)
    }
    pub fn load(memory: &MemoriaJogo, address: u32) -> Result<Self> {
        let mut digits = [0; 4];
        for (offset, digit) in digits.iter_mut().enumerate() {
            *digit = memory.u8(address.wrapping_add(u32::try_from(offset).expect("four digits")))?;
        }
        Ok(Self(digits))
    }
    pub fn project(&self, memory: &mut MemoriaJogo, address: u32) -> Result<()> {
        // Original store order: thousands, hundreds, tens, units.
        for index in (0..4).rev() {
            memory.set_u8(
                address.wrapping_add(u32::try_from(index).expect("four digits")),
                self.0[index],
            )?;
        }
        Ok(())
    }
    pub fn decode_aliasing(&self, source: u32, destination: u32) -> u16 {
        let mut value = 0u16; // Original zeros destination before reading digits.
        let mut multiplier = 1u16;
        for (offset, original_digit) in self.0.iter().enumerate() {
            let address = source.wrapping_add(u32::try_from(offset).expect("four digits"));
            let digit = if address == destination {
                value.to_le_bytes()[0]
            } else if address == destination.wrapping_add(1) {
                value.to_le_bytes()[1]
            } else {
                *original_digit
            };
            value = value.wrapping_add(u16::from(digit).wrapping_mul(multiplier));
            multiplier = multiplier.wrapping_mul(10);
        }
        value
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub eax: u32,
    pub ebx: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esi: u32,
    pub edi: u32,
    pub ebp: u32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routine {
    Random,
    LongRand,
    CrtRand,
    IntToBcd,
    BcdToInt,
}
impl TryFrom<u32> for Routine {
    type Error = EngineError;
    fn try_from(id: u32) -> Result<Self> {
        match id {
            1 => Ok(Self::Random),
            2 => Ok(Self::LongRand),
            3 => Ok(Self::CrtRand),
            4 => Ok(Self::IntToBcd),
            5 => Ok(Self::BcdToInt),
            _ => Err(EngineError::UnknownRoutine(id)),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameState {
    pub world_rng: Option<WorldRng>,
    pub crt_rng: Option<CrtRng>,
    pub frame: Option<crate::FrameState>,
}
/// Address bindings and opaque, declared bytes belong to projection, not logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    pub(crate) template: MemoriaJogo,
    pub(crate) crt_seed_address: Option<u32>,
    pub(crate) frame_bound: bool,
}
impl GameState {
    pub fn load(memory: MemoriaJogo) -> Result<(Self, Projection)> {
        let (state, crt_seed_address, frame_bound) = Self::inspect(&memory)?;
        Ok((
            state,
            Projection {
                template: memory,
                crt_seed_address,
                frame_bound,
            },
        ))
    }
    pub(crate) fn inspect(memory: &MemoriaJogo) -> Result<(Self, Option<u32>, bool)> {
        let world_rng = memory
            .optional_u32(WORLD_SEED)?
            .map(|seed| WorldRng { seed });
        let getter = memory.optional_u32(CRT_GETTER)?;
        let thread = memory.optional_u32(THREAD_DATA)?;
        let crt_seed_address = if getter == Some(SINGLE_THREAD_GETTER) {
            thread
                .map(|address| address.wrapping_add(12))
                .filter(|address| *address != 0)
        } else {
            None
        };
        let crt_rng = match crt_seed_address {
            Some(address) => memory.optional_u32(address)?.map(|seed| CrtRng { seed }),
            None => None,
        };
        let frame = crate::FrameState::load(memory)?;
        let frame_bound = frame.is_some();
        Ok((
            Self {
                world_rng,
                crt_rng,
                frame,
            },
            crt_seed_address,
            frame_bound,
        ))
    }
    pub fn project(&self, layout: &Projection) -> Result<MemoriaJogo> {
        // Presence belongs to the captured layout. Dropping a typed value must
        // not silently publish its stale opaque template bytes.
        let world_declared = layout.template.optional_u32(WORLD_SEED)?.is_some();
        if self.world_rng.is_some() != world_declared {
            return Err(EngineError::ProjectionFieldMismatch {
                address: WORLD_SEED,
            });
        }
        let crt_declared = match layout.crt_seed_address {
            Some(address) => layout.template.optional_u32(address)?.is_some(),
            None => false,
        };
        if self.crt_rng.is_some() != crt_declared {
            return Err(EngineError::ProjectionFieldMismatch {
                address: layout.crt_seed_address.unwrap_or(THREAD_DATA),
            });
        }
        if self.frame.is_some() != layout.frame_bound {
            return Err(EngineError::ProjectionFieldMismatch { address: 0x4e19a0 });
        }
        let mut memory = layout.template.clone();
        if let Some(frame) = &self.frame {
            frame.project(&mut memory)?;
        }
        if let Some(rng) = self.world_rng {
            memory.set_u32(WORLD_SEED, rng.seed)?;
        }
        if let Some(rng) = self.crt_rng {
            let address = layout
                .crt_seed_address
                .ok_or(EngineError::IncompleteField {
                    address: THREAD_DATA,
                })?;
            memory.set_u32(address, rng.seed)?;
        }
        Ok(memory)
    }
}
#[derive(Debug, Clone)]
pub struct World {
    pub(crate) render_receipt: Option<crate::render::ConsumptionReceipt>,
    // Host-only stage-start capability: the clock tick at which a fresh stage
    // boundary granted it. Decoded/imported worlds default closed; never
    // serialize this permission or infer it from a wrapping clock.
    pub(crate) difficulty_setup: Option<u64>,
    pub generation: u64,
    pub frame_phase: u32,
    pub(crate) state: GameState,
    pub(crate) projection: Projection,
    pub(crate) execution_mode: u32,
    pub(crate) delta: Option<crate::frame_state::FrameDelta>,
    pub clock: TickClock,
    pub frame_events: Vec<crate::FrameEvent>,
    pub frame_freed: MemoriaJogo,
    pub player_events: Vec<crate::player::PlayerEvent>,
    pub player_cursor: (i32, i32),
    /// Rule set of this world (`RULE_*` bits): fixed before the load tail,
    /// inherited by the next stage, recorded by saves. 0 is the original game.
    pub(crate) rules: u32,
    /// Shift+click waypoints (`RULE_ORDER_QUEUE`), Rust-side, never game memory.
    pub(crate) order_queue: crate::OrderQueue,
}
/// Optional modern unit paths ("Caminhos melhores", `crate::modern_path`).
/// Off reproduces the original CheminCalcul movement byte for byte.
pub const RULE_MODERN_PATHS: u32 = 1;
/// Every rule bit this engine knows; any other bit is refused. Each optional
/// feature adds its `RULE_*` bit here.
pub const RULES_KNOWN: u32 = RULE_MODERN_PATHS | crate::order_queue::RULE_ORDER_QUEUE;
/// Why a rule change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulesError {
    /// A bit outside `RULES_KNOWN`.
    Unknown(u32),
    /// The world already ran its load tail (or any later phase).
    Locked,
}
impl World {
    pub fn new(rate: u32, epoch: u32) -> Result<Self> {
        let mut world = Self::load(MemoriaJogo::default(), TickClock::new(rate, epoch)?)?;
        world.difficulty_setup = Some(world.clock.tick());
        Ok(world)
    }
    pub fn load(memory: MemoriaJogo, clock: TickClock) -> Result<Self> {
        let (state, projection) = GameState::load(memory)?;
        Ok(Self {
            render_receipt: None,
            difficulty_setup: None,
            generation: 0,
            frame_phase: 0,
            state,
            projection,
            execution_mode: 0,
            delta: None,
            clock,
            frame_events: Vec::new(),
            frame_freed: MemoriaJogo::default(),
            player_events: Vec::new(),
            player_cursor: (0, 0),
            rules: 0,
            order_queue: crate::OrderQueue::default(),
        })
    }
    /// `load` under an explicit rule set, as a save that records it decodes.
    pub fn load_with_rules(memory: MemoriaJogo, clock: TickClock, rules: u32) -> Result<Self> {
        if rules & !RULES_KNOWN != 0 {
            return Err(EngineError::InvalidRules(rules));
        }
        let mut world = Self::load(memory, clock)?;
        world.rules = rules;
        Ok(world)
    }
    pub fn rules(&self) -> u32 {
        self.rules
    }
    /// Pending Shift+click waypoints (always empty without `RULE_ORDER_QUEUE`).
    pub fn order_queue(&self) -> &crate::OrderQueue {
        &self.order_queue
    }
    /// Rules change only before the load tail: a menu world or a stage world
    /// that has not run phase 1 yet. The next stage inherits them.
    pub fn set_rules(&mut self, rules: u32) -> std::result::Result<(), RulesError> {
        if rules & !RULES_KNOWN != 0 {
            return Err(RulesError::Unknown(rules));
        }
        if self.frame_phase != 0 {
            return Err(RulesError::Locked);
        }
        self.rules = rules;
        Ok(())
    }
    pub fn state(&self) -> &GameState {
        &self.state
    }
    /// Explicit Rust edits invalidate the play view. C ABI mutations synchronize it automatically.
    pub fn state_mut(&mut self) -> &mut GameState {
        self.render_receipt = None;
        self.difficulty_setup = None;
        self.execution_mode = 0;
        self.delta = None;
        self.projection.template.audit = true;
        &mut self.state
    }
    pub fn project(&self) -> Result<MemoriaJogo> {
        let mut memory = self.state.project(&self.projection)?;
        memory.rules = self.rules;
        memory.order_queue = self.order_queue.clone();
        memory.path_searches = 0;
        if self.execution_mode != 0 && memory != self.projection.template {
            return Err(EngineError::ExecutionProjectionMismatch);
        }
        Ok(memory)
    }
    pub fn replace_memory(&mut self, memory: MemoriaJogo) -> Result<()> {
        let generation = self.generation.wrapping_add(1);
        let player_cursor = self.player_cursor;
        let mode = self.execution_mode;
        let mut next = Self::load(memory, self.clock.clone())?;
        next.difficulty_setup = self.difficulty_setup;
        next.player_cursor = player_cursor;
        next.generation = generation;
        next.rules = self.rules;
        next.order_queue = self.order_queue.clone();
        if mode != 0 && next.state.frame.is_some() {
            next.set_execution_mode(mode)?;
        }
        *self = next;
        Ok(())
    }
    /// Each successful CalculGame body advances one deterministic tick. Display phases do not.
    pub fn frame_step(
        &mut self,
        phase: u32,
        input: crate::FrameInput,
    ) -> Result<crate::FrameInput> {
        if self.execution_mode != 0 && phase != 1 {
            let result = self.play_phase(phase, input, None);
            if result.is_ok() {
                self.difficulty_setup = None;
            }
            return result;
        }
        let mut memory = self.project()?;
        memory.audit = self.execution_mode != 1;
        let (output, events, freed) = crate::frame::phase(&mut memory, phase, input)?;
        let queue = std::mem::take(&mut memory.order_queue);
        self.replace_memory(memory)?;
        self.order_queue = queue;
        self.difficulty_setup = None;
        self.frame_phase = phase;
        self.frame_events = events;
        self.frame_freed = freed;
        if phase == 1 || phase == 3 {
            self.clock.advance(1);
        }
        Ok(output)
    }
    /// Declared input phases are transactional. Failed attempts expose diagnostics
    /// separately, while game bytes, clock, cursor hold and generation stay unchanged.
    pub fn input_step(
        &mut self,
        phase: u32,
        input: crate::FrameInput,
        mut answers: crate::player::DeclaredInput,
    ) -> Result<crate::FrameInput> {
        if self.execution_mode != 0 {
            let result = self.play_phase(phase, input, Some(answers));
            if result.is_ok() {
                self.difficulty_setup = None;
            }
            return result;
        }
        let xy = answers.cursor.unwrap_or(self.player_cursor);
        answers.cursor = Some(xy);
        let mut memory = self.project()?;
        let result = crate::player::phase(&mut memory, phase, input, answers);
        match result {
            Ok((output, events, consumed)) => {
                let queue = std::mem::take(&mut memory.order_queue);
                self.replace_memory(memory)?;
                self.order_queue = queue;
                self.difficulty_setup = None;
                self.frame_phase = phase;
                self.frame_events = events;
                self.player_events = consumed;
                self.player_cursor = xy;
                if phase == 3 {
                    self.clock.advance(1);
                }
                Ok(output)
            }
            Err(error) => {
                self.player_events = memory.player.take().map(|r| r.events).unwrap_or_default();
                Err(error)
            }
        }
    }
    /// An isolated routine call, NOT a game frame. Errors publish no state/register change.
    pub fn step(&mut self, routine: Routine, input: Registers) -> Result<Registers> {
        let mut next = self.clone();
        let mode = next.execution_mode;
        next.execution_mode = 0;
        next.delta = None;
        let mut memory = next.project()?;
        let mut output = input;
        match routine {
            Routine::Random => {
                let rng = next
                    .state
                    .world_rng
                    .as_mut()
                    .ok_or(EngineError::UndeclaredAccess {
                        address: WORLD_SEED,
                        size: 4,
                    })?;
                output.eax = rng.draw();
                memory = next.project()?;
            }
            Routine::LongRand | Routine::CrtRand => {
                let getter = memory.u32(CRT_GETTER)?;
                if getter != SINGLE_THREAD_GETTER {
                    return Err(EngineError::UnsupportedThreadGetter {
                        address: 0x48c513,
                        value: getter,
                    });
                }
                let address = memory.u32(THREAD_DATA)?.wrapping_add(12);
                output.eax = if address == 0 {
                    0
                } else {
                    next.state
                        .crt_rng
                        .as_mut()
                        .ok_or(EngineError::UndeclaredAccess { address, size: 4 })?
                        .draw()
                };
                memory = next.project()?;
            }
            Routine::IntToBcd => {
                // Intentional low16 of original DX; semantic value then projection.
                let digits = BcdDigits::encode(input.edx as u16);
                digits.project(&mut memory, input.eax)?;
                output.eax = 0;
                output.edx = 1; // Original final div bx: AX=0, DX=1.
            }
            Routine::BcdToInt => {
                memory.read(input.eax, 2)?; // Validate destination before original zero/store.
                let digits = BcdDigits::load(&memory, input.edx)?;
                let value = digits.decode_aliasing(input.edx, input.eax);
                memory.set_u16(input.eax, value)?;
                output.edx = 10000; // Original final multiplier, not the converted value.
            }
        }
        // Refresh bindings after writes: BCD buffers may alias an RNG/global.
        next.replace_memory(memory)?;
        next.difficulty_setup = None;
        if mode != 0 && next.state.frame.is_some() {
            next.set_execution_mode(mode)?;
        }
        *self = next;
        Ok(output)
    }
}
