//! Independently versioned P2 entry extension to ABI 1.6.
use super::*;
use rl_core::{
    mission_boot::{StartupAssets, StartupInput},
    mission_entry::{self, Stage},
    FrameInput, TickClock,
};
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RlMissaoInicio {
    pub version: u32,
    pub allocation_fill: u32,
    pub ai_clock_ms: u32,
    pub thread_id: u32,
    pub ticks_per_second: u32,
    pub epoch_ms: u32,
    pub execution_mode: u32,
    /// Rule set (`RL_REGRA_*`), formerly `reserved`: 0 is the original game.
    pub regras: u32,
    pub iterations: [u32; 3],
    pub input: FrameInput,
}
pub(super) unsafe fn bytes<'a>(p: *const u8, n: u32) -> Result<&'a [u8]> {
    if p.is_null() || n == 0 {
        return Err(AbiError::InvalidArgument);
    }
    if n as usize > MAX_BYTES {
        return Err(AbiError::ResourceLimit);
    }
    Ok(unsafe { slice::from_raw_parts(p, n as usize) })
}
pub(super) fn publish(world: World) -> Result<u64> {
    with_registry(|r| {
        let handle = r.next;
        r.next = handle.checked_add(1).ok_or(AbiError::ResourceLimit)?;
        r.worlds.insert(handle, world);
        Ok(handle)
    })
}
#[no_mangle]
pub extern "C" fn rl_missao_versao() -> u32 {
    1
}
/// Read the current original screen without projecting/copying the whole world.
/// # Safety
/// `output` is writable and aligned for one u32 for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn rl_missao_tela(handle: u64, output: *mut u32) -> i32 {
    status(|| {
        if output.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let mode = with_world(handle, |w| Ok(u32::from(w.render_memory()?.u8(0x4e2f78)?)))?;
        unsafe {
            output.write(mode);
        }
        Ok(())
    })
}
/// # Safety
/// Ten readable ordered blobs, readable UTF-8 filename and aligned config;
/// writable aligned disjoint output/input_out. All spans live for this call.
#[no_mangle]
pub unsafe extern "C" fn rl_missao_iniciar(
    assets: *const RlBloco,
    count: u32,
    name: *const u8,
    name_len: u32,
    config: *const RlMissaoInicio,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    unsafe {
        mission_start(
            assets,
            count,
            name,
            name_len,
            config,
            ptr::null(),
            (output, input_out),
        )
    }
}
/// # Safety
/// Same readable/aligned/disjoint buffers as rl_missao_iniciar, plus host spans.
#[no_mangle]
pub unsafe extern "C" fn rl_missao_iniciar_eventos_v1(
    assets: *const RlBloco,
    count: u32,
    name: *const u8,
    name_len: u32,
    config: *const RlMissaoInicio,
    host: *const super::events_host_abi::RlEventsHostV1,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    if host.is_null() {
        return status(|| Err(AbiError::InvalidArgument));
    }
    unsafe {
        mission_start(
            assets,
            count,
            name,
            name_len,
            config,
            host,
            (output, input_out),
        )
    }
}
unsafe fn mission_start(
    assets: *const RlBloco,
    count: u32,
    name: *const u8,
    name_len: u32,
    config: *const RlMissaoInicio,
    host: *const super::events_host_abi::RlEventsHostV1,
    outputs: (*mut u64, *mut FrameInput),
) -> i32 {
    let (output, input_out) = outputs;
    status(|| {
        let events = if host.is_null() {
            None
        } else {
            Some(unsafe { super::events_host_abi::input(host) }?)
        };
        if count != 10
            || assets.is_null()
            || config.is_null()
            || output.is_null()
            || input_out.is_null()
        {
            return Err(AbiError::InvalidArgument);
        }
        let c = unsafe { config.read() };
        if c.version != 1 {
            return Err(AbiError::VersionMismatch);
        }
        if c.allocation_fill > 255 || c.regras & !rl_core::RULES_KNOWN != 0 || c.execution_mode > 2
        {
            return Err(AbiError::InvalidArgument);
        }
        let name = std::str::from_utf8(unsafe { bytes(name, name_len) }?)
            .map_err(|_| AbiError::InvalidArgument)?;
        let stage = Stage::parse(name).map_err(|_| AbiError::InvalidArgument)?;
        let blocks = unsafe { slice::from_raw_parts(assets, 10) };
        let mut total = 0usize;
        let mut blobs = Vec::new();
        for block in blocks {
            total = total
                .checked_add(block.length as usize)
                .ok_or(AbiError::ResourceLimit)?;
            if total > MAX_BYTES {
                return Err(AbiError::ResourceLimit);
            }
            blobs.push(unsafe { bytes(block.data, block.length) }?);
        }
        let a = StartupAssets {
            executable: blobs[0],
            rising_ini: blobs[1],
            rules: blobs[2],
            symbols: blobs[3],
            obstacles: blobs[4],
            terrain_colors: blobs[5],
            palette_pie: blobs[6],
        };
        let mut memory = if let Some(events) = &events {
            mission_entry::load_stage_with_events(
                &a,
                StartupInput {
                    allocation_fill: c.allocation_fill as u8,
                    ai_clock_ms: c.ai_clock_ms,
                    thread_id: c.thread_id,
                },
                stage,
                blobs[9],
                blobs[7],
                blobs[8],
                events,
            )?
        } else {
            mission_entry::load_stage(
                &a,
                StartupInput {
                    allocation_fill: c.allocation_fill as u8,
                    ai_clock_ms: c.ai_clock_ms,
                    thread_id: c.thread_id,
                },
                stage,
                blobs[9],
                blobs[7],
                blobs[8],
            )?
        };
        mission_entry::bind_frame_context(&mut memory, c.input, c.iterations)?;
        let mut world = rl_core::difficulty::load_fresh(
            memory,
            TickClock::new(c.ticks_per_second, c.epoch_ms)?,
        )?;
        world.player_cursor = (c.input.cursor_x, c.input.cursor_y);
        world.set_execution_mode(c.execution_mode)?;
        world
            .set_rules(c.regras)
            .map_err(|_| AbiError::InvalidArgument)?;
        let input = c.input;
        let handle = publish(world)?;
        unsafe {
            output.write(handle);
            input_out.write(input);
        }
        Ok(())
    })
}
/// # Safety
/// Nine readable ordered blobs (no ETA) and aligned config; writable aligned
/// disjoint output/input_out. All spans live for this call.
#[no_mangle]
pub unsafe extern "C" fn rl_missao_iniciar_menu(
    assets: *const RlBloco,
    count: u32,
    config: *const RlMissaoInicio,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    unsafe { menu_start(assets, count, config, ptr::null(), output, input_out) }
}
/// # Safety
/// Same buffers as rl_missao_iniciar_menu, plus the EVENTS host spans.
#[no_mangle]
pub unsafe extern "C" fn rl_missao_iniciar_menu_eventos_v1(
    assets: *const RlBloco,
    count: u32,
    config: *const RlMissaoInicio,
    host: *const super::events_host_abi::RlEventsHostV1,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    if host.is_null() {
        return status(|| Err(AbiError::InvalidArgument));
    }
    unsafe { menu_start(assets, count, config, host, output, input_out) }
}
unsafe fn menu_start(
    assets: *const RlBloco,
    count: u32,
    config: *const RlMissaoInicio,
    host: *const super::events_host_abi::RlEventsHostV1,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    status(|| {
        let events = if host.is_null() {
            None
        } else {
            Some(unsafe { super::events_host_abi::input(host) }?)
        };
        if count != 9
            || assets.is_null()
            || config.is_null()
            || output.is_null()
            || input_out.is_null()
        {
            return Err(AbiError::InvalidArgument);
        }
        let c = unsafe { config.read() };
        if c.version != 1 {
            return Err(AbiError::VersionMismatch);
        }
        if c.allocation_fill > 255 || c.regras & !rl_core::RULES_KNOWN != 0 || c.execution_mode > 2
        {
            return Err(AbiError::InvalidArgument);
        }
        let blocks = unsafe { slice::from_raw_parts(assets, 9) };
        let mut total = 0usize;
        let mut blobs = Vec::new();
        for block in blocks {
            total = total
                .checked_add(block.length as usize)
                .ok_or(AbiError::ResourceLimit)?;
            if total > MAX_BYTES {
                return Err(AbiError::ResourceLimit);
            }
            blobs.push(unsafe { bytes(block.data, block.length) }?);
        }
        let a = StartupAssets {
            executable: blobs[0],
            rising_ini: blobs[1],
            rules: blobs[2],
            symbols: blobs[3],
            obstacles: blobs[4],
            terrain_colors: blobs[5],
            palette_pie: blobs[6],
        };
        let mut memory = mission_entry::boot_main_menu(
            &a,
            StartupInput {
                allocation_fill: c.allocation_fill as u8,
                ai_clock_ms: c.ai_clock_ms,
                thread_id: c.thread_id,
            },
            blobs[8],
            events.as_ref(),
        )?;
        mission_entry::bind_frame_context(&mut memory, c.input, c.iterations)?;
        // A menu world has no frame state: the requested execution mode applies
        // to the stage world the host loads from it (rl_execucao_definir).
        let mut world = World::load_with_rules(
            memory,
            TickClock::new(c.ticks_per_second, c.epoch_ms)?,
            c.regras,
        )?;
        world.player_cursor = (c.input.cursor_x, c.input.cursor_y);
        let input = c.input;
        let handle = publish(world)?;
        unsafe {
            output.write(handle);
            input_out.write(input);
        }
        Ok(())
    })
}
/// # Safety
/// required is aligned/writable; buffer is writable for capacity bytes and
/// disjoint. NULL/zero queries the byte length, without a NUL terminator.
#[no_mangle]
pub unsafe extern "C" fn rl_missao_nome_proxima(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    status(|| {
        if required.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let name = with_world(handle, |w| Ok(mission_entry::next_name(w)?))?;
        unsafe {
            required.write(name.len() as u32);
        }
        if capacity < (name.len() as u32) {
            return Err(AbiError::BufferTooSmall);
        }
        if buffer.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        unsafe {
            ptr::copy_nonoverlapping(name.as_ptr(), buffer, name.len());
        }
        Ok(())
    })
}
/// # Safety
/// Readable original path/ETA/MONSTRE spans and aligned input; aligned
/// writable disjoint outputs. Source world and all outputs survive every error.
#[no_mangle]
pub unsafe extern "C" fn rl_missao_continuar(
    handle: u64,
    name: *const u8,
    name_len: u32,
    eta: *const u8,
    eta_len: u32,
    monsters: *const u8,
    monsters_len: u32,
    input: *const FrameInput,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    status(|| {
        if input.is_null() || output.is_null() || input_out.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let name = unsafe { bytes(name, name_len) }?;
        let eta = unsafe { bytes(eta, eta_len) }?;
        let monsters = unsafe { bytes(monsters, monsters_len) }?;
        let (world, next_input) = with_world(handle, |w| {
            Ok(mission_entry::continue_victory(
                w,
                name,
                eta,
                monsters,
                unsafe { input.read() },
            )?)
        })?;
        let new_handle = publish(world)?;
        unsafe {
            output.write(new_handle);
            input_out.write(next_input);
        }
        Ok(())
    })
}
