//! Pilot ABI. Foreign pointers must reference valid caller-owned memory.
//! Supported hosts are64-bit; u32 lengths/indices widen to usize without loss.
#![deny(unsafe_op_in_unsafe_fn)]
use rl_core::{EngineError, MemoriaJogo, Registers, Routine, World};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr, slice,
    sync::{Mutex, OnceLock},
};

pub const ABI_VERSION: u32 = 0x00010006;
const MAX_BLOCKS: u32 = 65536;
const MAX_BYTES: usize = 64 * 1024 * 1024;
#[repr(C)]
pub struct RlBloco {
    pub address: u32,
    pub length: u32,
    pub data: *const u8,
}
mod event_flc_abi;
mod events_host_abi;
mod game_hud_background_abi;
mod mission_abi;
mod render_abi;
mod save_abi;
struct Registry {
    saves: BTreeMap<u64, save_abi::SaveSession>,
    renderers: BTreeMap<u64, render_abi::BoundRenderer>,
    next: u64,
    worlds: BTreeMap<u64, World>,
}
static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();
thread_local! { static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) }; }
#[derive(Debug)]
enum AbiError {
    InvalidArgument,
    InvalidHandle,
    Memory(EngineError),
    UnknownRoutine,
    UnsupportedPath(EngineError),
    BufferTooSmall,
    Panic,
    Poisoned,
    ResourceLimit,
    VersionMismatch,
    StaleGeneration,
    RenderInput(&'static str),
    Save(rl_core::save::SaveError),
    NoAutosave,
}
impl AbiError {
    fn code(&self) -> i32 {
        match self {
            Self::InvalidArgument | Self::RenderInput(_) => 1,
            Self::InvalidHandle => 2,
            Self::Memory(_) => 3,
            Self::UnknownRoutine => 4,
            Self::UnsupportedPath(_) => 5,
            Self::BufferTooSmall => 6,
            Self::Panic => 7,
            Self::Poisoned => 8,
            Self::ResourceLimit => 9,
            Self::VersionMismatch => 10,
            Self::StaleGeneration => 11,
            Self::Save(error) => match error {
                rl_core::save::SaveError::Invalid(_) => 12,
                rl_core::save::SaveError::Checksum => 13,
                rl_core::save::SaveError::Version { .. } => 14,
                rl_core::save::SaveError::Io(_) => 15,
                rl_core::save::SaveError::Busy => 16,
                rl_core::save::SaveError::Limit => 9,
                rl_core::save::SaveError::Engine(_) => 3,
            },
            Self::NoAutosave => 17,
        }
    }
    fn message(&self) -> String {
        match self {
            Self::Memory(e) | Self::UnsupportedPath(e) => e.to_string(),
            Self::RenderInput(s) => (*s).to_owned(),
            Self::Save(error) => error.to_string(),
            _ => format!("{self:?}"),
        }
    }
}
impl From<EngineError> for AbiError {
    fn from(e: EngineError) -> Self {
        match e {
            EngineError::UnknownRoutine(_) => Self::UnknownRoutine,
            EngineError::UnsupportedThreadGetter { .. } | EngineError::UnportedCallee { .. } => {
                Self::UnsupportedPath(e)
            }
            EngineError::InvalidTickRate
            | EngineError::InvalidFramePhase(_)
            | EngineError::InvalidExecutionMode(_)
            | EngineError::InvalidRules(_) => Self::InvalidArgument,
            _ => Self::Memory(e),
        }
    }
}
type Result<T> = std::result::Result<T, AbiError>;
fn status(f: impl FnOnce() -> Result<()>) -> i32 {
    let outcome = catch_unwind(AssertUnwindSafe(f)).unwrap_or(Err(AbiError::Panic));
    match outcome {
        Ok(()) => {
            LAST_ERROR.with(|e| e.borrow_mut().clear());
            0
        }
        Err(error) => {
            LAST_ERROR.with(|e| *e.borrow_mut() = error.message());
            error.code()
        }
    }
}
fn with_registry<T>(f: impl FnOnce(&mut Registry) -> Result<T>) -> Result<T> {
    let mut registry = REGISTRY
        .get_or_init(|| {
            Mutex::new(Registry {
                next: 1,
                worlds: BTreeMap::new(),
                renderers: BTreeMap::new(),
                saves: BTreeMap::new(),
            })
        })
        .lock()
        .map_err(|_| AbiError::Poisoned)?;
    f(&mut registry)
}
fn with_world<T>(handle: u64, f: impl FnOnce(&mut World) -> Result<T>) -> Result<T> {
    with_registry(|r| f(r.worlds.get_mut(&handle).ok_or(AbiError::InvalidHandle)?))
}
#[no_mangle]
pub extern "C" fn rl_versao() -> u32 {
    ABI_VERSION
}

/// # Safety
/// `output` is writable and properly aligned; outputs do not alias inputs.
#[no_mangle]
pub unsafe extern "C" fn rl_criar_mundo(
    version: u32,
    rate: u32,
    epoch_ms: u32,
    output: *mut u64,
) -> i32 {
    status(|| {
        if ![
            ABI_VERSION,
            0x00010000,
            0x00010001,
            0x00010002,
            0x00010003,
            0x00010004,
            0x00010005,
        ]
        .contains(&version)
        {
            return Err(AbiError::VersionMismatch);
        }
        if output.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let world = World::new(rate, epoch_ms)?;
        let handle = with_registry(|r| {
            let handle = r.next;
            r.next = r.next.checked_add(1).ok_or(AbiError::ResourceLimit)?;
            r.worlds.insert(handle, world);
            Ok(handle)
        })?;
        unsafe {
            output.write(handle);
        }
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn rl_liberar_mundo(handle: u64) -> i32 {
    status(|| {
        with_registry(|r| {
            r.worlds
                .remove(&handle)
                .map(|_| ())
                .ok_or(AbiError::InvalidHandle)?;
            r.saves.remove(&handle);
            Ok(())
        })
    })
}
/// # Safety
/// `blocks` references `count` aligned descriptors. Each data points to length
/// readable bytes for this call; no pointer is retained, and no overlap with outputs.
#[no_mangle]
pub unsafe extern "C" fn rl_carregar_blocos(
    handle: u64,
    blocks: *const RlBloco,
    count: u32,
) -> i32 {
    status(|| {
        if count > MAX_BLOCKS {
            return Err(AbiError::ResourceLimit);
        }
        if count != 0 && blocks.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let descriptors = if count == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(blocks, count as usize) }
        }; // u32 count fits supported64-bit usize.
        let mut total = 0usize;
        for block in descriptors {
            if block.length == 0 || block.data.is_null() {
                return Err(AbiError::InvalidArgument);
            }
            total = total
                .checked_add(block.length as usize)
                .ok_or(AbiError::ResourceLimit)?; // u32 length fits64-bit usize.
            if total > MAX_BYTES {
                return Err(AbiError::ResourceLimit);
            }
        }
        let mut memory = MemoriaJogo::default();
        for block in descriptors {
            let bytes =
                unsafe { slice::from_raw_parts(block.data, block.length as usize) }.to_vec();
            memory.declare(block.address, bytes)?;
        }
        with_registry(|r| {
            let world = r.worlds.get_mut(&handle).ok_or(AbiError::InvalidHandle)?;
            world.replace_memory(memory)?;
            world.player_cursor = (0, 0);
            r.saves.remove(&handle);
            Ok(())
        })
    })
}
/// Original text startup after S0; file bytes come from bundled CD assets.
/// # Safety
/// `bytes` is readable for `length` bytes for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn rl_carregar_textos(
    handle: u64,
    bytes: *const u8,
    length: u32,
    language: u32,
) -> i32 {
    status(|| {
        if bytes.is_null() || length == 0 || length > 1024 * 1024 || language > 255 {
            return Err(AbiError::InvalidArgument);
        }
        let file = unsafe { slice::from_raw_parts(bytes, length as usize) };
        with_world(handle, |world| Ok(world.load_texts(file, language as u8)?))
    })
}
/// Supply bundled EVENTS.INI for native notifications after S0 startup.
/// # Safety
/// `bytes` is readable for `length` bytes for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn rl_carregar_eventos(handle: u64, bytes: *const u8, length: u32) -> i32 {
    status(|| {
        if bytes.is_null() || length == 0 || length > 1024 * 1024 {
            return Err(AbiError::InvalidArgument);
        }
        let file = unsafe { slice::from_raw_parts(bytes, length as usize) };
        with_world(handle, |world| Ok(world.load_events(file)?))
    })
}
/// # Safety
/// `registers` is a writable aligned Registers; unchanged on failure.
#[no_mangle]
pub unsafe extern "C" fn rl_executar_rotina(
    handle: u64,
    routine: u32,
    registers: *mut Registers,
) -> i32 {
    status(|| {
        if registers.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let routine = Routine::try_from(routine)?;
        let input = unsafe { registers.read() };
        let output = with_world(handle, |world| Ok(world.step(routine, input)?))?;
        unsafe {
            registers.write(output);
        }
        Ok(())
    })
}
/// # Safety
/// `output` is writable and aligned.
#[no_mangle]
pub unsafe extern "C" fn rl_quantidade_blocos(handle: u64, output: *mut u32) -> i32 {
    status(|| {
        if output.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let count = with_world(handle, |world| {
            u32::try_from(world.project()?.blocks().count()).map_err(|_| AbiError::ResourceLimit)
        })?;
        unsafe {
            output.write(count);
        }
        Ok(())
    })
}
/// # Safety
/// address/required are distinct aligned writable u32 pointers. Buffer is
/// writable for capacity bytes when nonnull; no pointers alias. Metadata is
/// written on BufferTooSmall; byte buffer is untouched. Query with null/zero.
#[no_mangle]
pub unsafe extern "C" fn rl_projetar_bloco(
    handle: u64,
    index: u32,
    address: *mut u32,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    status(|| {
        if address.is_null() || required.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let memory = with_world(handle, |world| Ok(world.project()?))?;
        let (base, bytes) = memory
            .blocks()
            .nth(index as usize)
            .ok_or(AbiError::InvalidArgument)?;
        let length = u32::try_from(bytes.len()).map_err(|_| AbiError::ResourceLimit)?;
        unsafe {
            address.write(base);
            required.write(length);
        }
        if capacity < length {
            return Err(AbiError::BufferTooSmall);
        }
        if buffer.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
        }
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn rl_avancar_ticks(handle: u64, ticks: u64) -> i32 {
    status(|| {
        with_world(handle, |world| {
            world.clock.advance(ticks);
            Ok(())
        })
    })
}
/// # Safety
/// Outputs are distinct writable aligned u64/u32 pointers.
#[no_mangle]
pub unsafe extern "C" fn rl_relogio(handle: u64, tick: *mut u64, milliseconds: *mut u32) -> i32 {
    status(|| {
        if tick.is_null() || milliseconds.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let (t, ms) = with_world(handle, |world| {
            Ok((world.clock.tick(), world.clock.milliseconds()))
        })?;
        unsafe {
            tick.write(t);
            milliseconds.write(ms);
        }
        Ok(())
    })
}
/// # Safety
/// Required is writable/aligned. Buffer is writable for capacity bytes, disjoint
/// from required. Copies UTF8 plus NUL; query does not clear the previous error.
#[no_mangle]
pub unsafe extern "C" fn rl_erro_mensagem(
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    if required.is_null() {
        return 1;
    }
    LAST_ERROR.with(|error| {
        let text = error.borrow();
        let length = text.len() + 1;
        unsafe {
            required.write(u32::try_from(length).unwrap_or(u32::MAX));
        }
        if (capacity as usize) < length {
            return 6;
        }
        if buffer.is_null() {
            return 1;
        }
        unsafe {
            ptr::copy_nonoverlapping(text.as_ptr(), buffer, text.len());
            buffer.add(text.len()).write(0);
        }
        0
    })
}

/// # Safety
/// Input is an aligned, writable RlFrameInput. No foreign pointers are retained.
/// On failure the complete world, clock, previous events and input remain unchanged.
#[no_mangle]
pub unsafe extern "C" fn rl_passo_quadro(
    handle: u64,
    phase: u32,
    input: *mut rl_core::FrameInput,
) -> i32 {
    status(|| {
        if input.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let value = unsafe { input.read() };
        let output = with_world(handle, |world| Ok(world.frame_step(phase, value)?))?;
        unsafe { input.write(output) };
        Ok(())
    })
}
/// # Safety
/// Output/required are writable, aligned, disjoint; capacity is a record count.
/// NULL/zero queries required count. Buffers remain untouched on errors.
#[no_mangle]
pub unsafe extern "C" fn rl_eventos_quadro(
    handle: u64,
    buffer: *mut rl_core::FrameEvent,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    status(|| {
        if required.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        with_world(handle, |world| {
            let count =
                u32::try_from(world.frame_events.len()).map_err(|_| AbiError::ResourceLimit)?;
            unsafe { required.write(count) };
            if capacity < count {
                return Err(AbiError::BufferTooSmall);
            }
            if count != 0 && buffer.is_null() {
                return Err(AbiError::InvalidArgument);
            }
            if count != 0 {
                unsafe {
                    ptr::copy_nonoverlapping(world.frame_events.as_ptr(), buffer, count as usize)
                }
            }
            Ok(())
        })
    })
}
fn pack(memory: &MemoriaJogo) -> Result<Vec<u8>> {
    let blocks: Vec<_> = memory.blocks().collect();
    let mut bytes = Vec::new();
    bytes.extend(
        u32::try_from(blocks.len())
            .map_err(|_| AbiError::ResourceLimit)?
            .to_le_bytes(),
    );
    for (base, data) in blocks {
        bytes.extend(base.to_le_bytes());
        bytes.extend(
            u32::try_from(data.len())
                .map_err(|_| AbiError::ResourceLimit)?
                .to_le_bytes(),
        );
        bytes.extend(data);
    }
    Ok(bytes)
}
unsafe fn bulk(
    handle: u64,
    freed: bool,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    status(|| {
        if required.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let bytes = with_world(handle, |world| {
            if freed {
                pack(&world.frame_freed)
            } else {
                pack(&world.project()?)
            }
        })?;
        let count = u32::try_from(bytes.len()).map_err(|_| AbiError::ResourceLimit)?;
        unsafe { required.write(count) };
        if capacity < count {
            return Err(AbiError::BufferTooSmall);
        }
        if buffer.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len()) };
        Ok(())
    })
}
/// Copied little-endian count, then repeated address/length/data (no padding).
/// # Safety
/// buffer/required writable and disjoint; NULL/zero queries byte count.
#[no_mangle]
pub unsafe extern "C" fn rl_projetar_tudo(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    unsafe { bulk(handle, false, buffer, capacity, required) }
}
/// Actual bytes at each free in the last successful phase, sorted by address.
/// # Safety
/// Same pointer and size-query contract as rl_projetar_tudo.
#[no_mangle]
pub unsafe extern "C" fn rl_projetar_liberados(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    unsafe { bulk(handle, true, buffer, capacity, required) }
}
/// Isolated frame-provider effects for differential testing; does not tick.
/// # Safety
/// args contains count readable aligned u32s; output is writable, disjoint.
#[no_mangle]
pub unsafe extern "C" fn rl_executar_rotina_quadro(
    handle: u64,
    id: u32,
    args: *const u32,
    count: u32,
    output: *mut u32,
) -> i32 {
    status(|| {
        let (name, arity) = rl_core::frame_ai::isolated(id)
            .or_else(|| rl_core::frame_units::isolated(id))
            .or_else(|| rl_core::frame_core::isolated(id))
            .or_else(|| rl_core::frame_economy::isolated(id))
            .or_else(|| rl_core::frame::isolated(id))
            .or_else(|| rl_core::frame_movement::isolated(id))
            .or_else(|| rl_core::frame_combat::isolated(id))
            .ok_or(AbiError::UnknownRoutine)?;
        if count as usize != arity || output.is_null() || (count != 0 && args.is_null()) {
            return Err(AbiError::InvalidArgument);
        }
        let values = if count == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(args, count as usize) }
        };
        let result = with_world(handle, |world| {
            let mut memory = world.project()?;
            let result = match id {
                100..=149 | 400..=449 => rl_core::frame_ai::call(&mut memory, name, values)?,
                150..=199 => rl_core::frame_units::call(&mut memory, name, values)?,
                200..=249 => rl_core::frame_core::call(&mut memory, name, values)?,
                250..=299 => rl_core::frame_economy::call(&mut memory, name, values)?,
                450..=499 => rl_core::frame_movement::call(&mut memory, name, values)?,
                550..=599 => rl_core::frame_combat::call(&mut memory, name, values)?,
                _ => rl_core::frame::call(&mut memory, name, values)?,
            };
            world.replace_memory(memory)?;
            Ok(result)
        })?;
        unsafe { output.write(result) };
        Ok(())
    })
}

mod player_abi;

/// ABI1.6 execution policy. Modes change audit work, never the game routine chain.
/// # Safety
/// Output is a writable aligned u32. Queries do not change world generation.
#[no_mangle]
pub unsafe extern "C" fn rl_regras(handle: u64, output: *mut u32) -> i32 {
    status(|| {
        if output.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let rules = with_world(handle, |world| Ok(world.rules()))?;
        unsafe {
            output.write(rules);
        }
        Ok(())
    })
}
/// Rules change only before the load tail (a menu world, or a stage world that
/// has not run phase 1); the next stage inherits them. Unknown bits or a world
/// past its load tail: RL_INVALID_ARGUMENT, world unchanged.
#[no_mangle]
pub extern "C" fn rl_regras_definir(handle: u64, rules: u32) -> i32 {
    status(|| {
        with_world(handle, |world| {
            world
                .set_rules(rules)
                .map_err(|_| AbiError::InvalidArgument)
        })
    })
}
/// Pending Shift+click waypoints of one unit (`RL_REGRA_FILA_ORDENS`), oldest
/// first. `count` receives the total; at most `capacity` cells are written.
/// Without the rule, or for a unit with no waypoints, the count is 0.
/// # Safety
/// `count` is a writable aligned u32; `cells` holds `capacity` u16 (may be null
/// when `capacity` is 0). Queries do not change world generation.
#[no_mangle]
pub unsafe extern "C" fn rl_fila_ordens(
    handle: u64,
    unit: u32,
    cells: *mut u16,
    capacity: u32,
    count: *mut u32,
) -> i32 {
    status(|| {
        if count.is_null() || (cells.is_null() && capacity != 0) || unit > u32::from(u16::MAX) {
            return Err(AbiError::InvalidArgument);
        }
        let queued = with_world(handle, |world| {
            Ok(world.order_queue().cells(unit as u16).to_vec())
        })?;
        unsafe {
            for (i, cell) in queued.iter().take(capacity as usize).enumerate() {
                cells.add(i).write(*cell);
            }
            count.write(queued.len() as u32);
        }
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn rl_execucao_definir(handle: u64, mode: u32) -> i32 {
    status(|| with_world(handle, |world| Ok(world.set_execution_mode(mode)?)))
}
/// # Safety
/// Output is a writable aligned u32. Queries do not change world generation.
#[no_mangle]
pub unsafe extern "C" fn rl_execucao_modo(handle: u64, output: *mut u32) -> i32 {
    status(|| {
        if output.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let mode = with_world(handle, |world| Ok(world.execution_mode()))?;
        unsafe {
            output.write(mode);
        }
        Ok(())
    })
}

mod difficulty_abi;

mod shell_abi;

mod options_abi;

mod load_tail_input_abi;
#[cfg(test)]
#[path = "rules_abi_tests.rs"]
mod rules_abi_tests;
