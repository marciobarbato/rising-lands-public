//! P4 save extension to ABI 1.6. No foreign pointer is retained.
use super::*;
use rl_core::{
    player::DeclaredInput,
    save::{self, PendingInput, ResumeState, SaveError, StageAutosave},
    FrameInput,
};
use std::{collections::VecDeque, path::Path};

#[derive(Default)]
pub(super) struct SaveSession {
    pending: VecDeque<PendingInput>,
    autosave: StageAutosave,
}
impl From<SaveError> for AbiError {
    fn from(error: SaveError) -> Self {
        match error {
            SaveError::Engine(e) => e.into(),
            SaveError::Limit => Self::ResourceLimit,
            other => Self::Save(other),
        }
    }
}
fn snapshot(r: &Registry, handle: u64, input: FrameInput) -> Result<Vec<u8>> {
    let w = r.worlds.get(&handle).ok_or(AbiError::InvalidHandle)?;
    let pending = r
        .saves
        .get(&handle)
        .map(|s| s.pending.clone())
        .unwrap_or_default();
    Ok(save::encode(w, &ResumeState { input, pending })?)
}
fn publish(world: World, resume: ResumeState) -> Result<(u64, FrameInput)> {
    with_registry(|r| {
        let handle = r.next;
        r.next = handle.checked_add(1).ok_or(AbiError::ResourceLimit)?;
        r.worlds.insert(handle, world);
        r.saves.entry(handle).or_default().pending = resume.pending;
        Ok((handle, resume.input))
    })
}
unsafe fn path<'a>(bytes: *const u8, length: u32) -> Result<&'a Path> {
    if bytes.is_null() || length == 0 || length > 32768 {
        return Err(AbiError::InvalidArgument);
    }
    let bytes = unsafe { slice::from_raw_parts(bytes, length as usize) };
    if bytes.contains(&0) {
        return Err(AbiError::InvalidArgument);
    }
    Ok(Path::new(
        std::str::from_utf8(bytes).map_err(|_| AbiError::InvalidArgument)?,
    ))
}
unsafe fn copy_bytes(
    bytes: &[u8],
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> Result<()> {
    if required.is_null() {
        return Err(AbiError::InvalidArgument);
    }
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
}

/// Version of the independently versioned P4 save extension; ABI 1.6 stays compatible.
#[no_mangle]
pub extern "C" fn rl_salvar_versao() -> u32 {
    save::FORMAT_VERSION
}

/// # Safety
/// input/required are aligned readable/writable respectively; buffer is writable
/// for capacity bytes, disjoint from both. NULL/zero queries the required size.
#[no_mangle]
pub unsafe extern "C" fn rl_salvar_bytes(
    handle: u64,
    input: *const FrameInput,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    status(|| {
        if input.is_null() || required.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let input = unsafe { input.read() };
        let bytes = with_registry(|r| snapshot(r, handle, input))?;
        unsafe { copy_bytes(&bytes, buffer, capacity, required) }
    })
}
/// # Safety
/// bytes is readable for length bytes; output is aligned/writable and disjoint.
/// Failure leaves world, queued input and output untouched.
#[no_mangle]
pub unsafe extern "C" fn rl_carregar_bytes(
    bytes: *const u8,
    length: u32,
    handle_out: *mut u64,
    output: *mut FrameInput,
) -> i32 {
    status(|| {
        if bytes.is_null() || output.is_null() || handle_out.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        if length as usize > save::MAX_SAVE_BYTES {
            return Err(AbiError::ResourceLimit);
        }
        let bytes = unsafe { slice::from_raw_parts(bytes, length as usize) };
        let (world, resume) = save::decode_for_play(bytes)?;
        let (handle, input) = publish(world, resume)?;
        unsafe {
            handle_out.write(handle);
            output.write(input)
        };
        Ok(())
    })
}
/// # Safety
/// path is a readable UTF-8 span without NUL; input is aligned/readable.
/// Writes a synced temporary file and renames it over the chosen slot.
#[no_mangle]
pub unsafe extern "C" fn rl_salvar_arquivo(
    handle: u64,
    input: *const FrameInput,
    path_bytes: *const u8,
    path_length: u32,
) -> i32 {
    status(|| {
        if input.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let path = unsafe { path(path_bytes, path_length) }?;
        let input = unsafe { input.read() };
        let bytes = with_registry(|r| snapshot(r, handle, input))?;
        Ok(save::write_file(path, &bytes)?)
    })
}
/// # Safety
/// path is a readable UTF-8 span without NUL; output is aligned/writable,
/// disjoint from path. Failure leaves output and the running world untouched.
#[no_mangle]
pub unsafe extern "C" fn rl_carregar_arquivo(
    path_bytes: *const u8,
    path_length: u32,
    handle_out: *mut u64,
    output: *mut FrameInput,
) -> i32 {
    status(|| {
        if output.is_null() || handle_out.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let path = unsafe { path(path_bytes, path_length) }?;
        let (world, resume) = save::load_file_for_play(path)?;
        let (handle, input) = publish(world, resume)?;
        unsafe {
            handle_out.write(handle);
            output.write(input)
        };
        Ok(())
    })
}

// Same copied-input contract as rl_passo_quadro_entrada; future execution is owned.
unsafe fn answers(
    phase: u32,
    declared: *const player_abi::RlDeclaredInput,
) -> Result<DeclaredInput> {
    if declared.is_null() {
        return Err(AbiError::InvalidArgument);
    }
    let d = unsafe { declared.read() };
    if d.size != std::mem::size_of::<player_abi::RlDeclaredInput>() as u32
        || d.flags & !7 != 0
        || !matches!(phase, 2 | 3)
        || d.message_count > 65536
        || d.key_count > 256
        || (d.message_count != 0 && d.messages.is_null())
        || (d.key_count != 0 && d.keys.is_null())
        || (phase == 3 && d.message_count != 0)
    {
        return Err(AbiError::InvalidArgument);
    }
    let cursor = if d.flags & 1 != 0 {
        if !(0..800).contains(&d.cursor_x) || !(0..600).contains(&d.cursor_y) {
            return Err(AbiError::InvalidArgument);
        }
        Some((d.cursor_x, d.cursor_y))
    } else {
        None
    };
    let mut a = DeclaredInput {
        cursor,
        display_text_boundary: d.flags & 2 != 0,
        display_sound_boundary: d.flags & 4 != 0,
        ..Default::default()
    };
    if d.message_count != 0 {
        for m in unsafe { slice::from_raw_parts(d.messages, d.message_count as usize) } {
            if !matches!(m.message, 0x100 | 0x101 | 0x200..=0x205) {
                return Err(AbiError::InvalidArgument);
            }
            a.messages.push([m.message, m.wparam, m.lparam]);
        }
    }
    if d.key_count != 0 {
        for k in unsafe { slice::from_raw_parts(d.keys, d.key_count as usize) } {
            if k.vk > 255 || a.keys.insert(k.vk, k.value as u32).is_some() {
                return Err(AbiError::InvalidArgument);
            }
        }
    }
    Ok(a)
}
/// # Safety
/// declared and its spans satisfy rl_passo_quadro_entrada's readable input contract.
/// Copies an accepted phase to the FIFO; it does not execute or consume input.
#[no_mangle]
pub unsafe extern "C" fn rl_entrada_enfileirar(
    handle: u64,
    phase: u32,
    declared: *const player_abi::RlDeclaredInput,
) -> i32 {
    status(|| {
        let answers = unsafe { answers(phase, declared) }?;
        with_registry(|r| {
            if !r.worlds.contains_key(&handle) {
                return Err(AbiError::InvalidHandle);
            }
            let queue = &mut r.saves.entry(handle).or_default().pending;
            let size = |a: &DeclaredInput| 32 + a.messages.len() * 12 + a.keys.len() * 8;
            let total: usize = queue.iter().map(|p| size(&p.answers)).sum();
            if queue.len() >= 65536 || total + size(&answers) > MAX_BYTES {
                return Err(AbiError::ResourceLimit);
            }
            queue.push_back(PendingInput { phase, answers });
            Ok(())
        })
    })
}
/// # Safety
/// input is aligned/readable/writable. Executes one queued phase. An empty FIFO
/// succeeds without mutation. Failed execution retains the queued input.
#[no_mangle]
pub unsafe extern "C" fn rl_entrada_passo_pendente(handle: u64, input: *mut FrameInput) -> i32 {
    status(|| {
        if input.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let original = unsafe { input.read() };
        let out = with_registry(|r| {
            let w = r.worlds.get_mut(&handle).ok_or(AbiError::InvalidHandle)?;
            let s = r.saves.entry(handle).or_default();
            let Some(next) = s.pending.front() else {
                return Ok(original);
            };
            let out = w.input_step(next.phase, original, next.answers.clone())?;
            s.pending.pop_front();
            Ok(out)
        })?;
        unsafe { input.write(out) };
        Ok(())
    })
}
/// # Safety
/// output is an aligned writable u32. Reports the number of queued phases.
#[no_mangle]
pub unsafe extern "C" fn rl_entrada_quantidade_pendente(handle: u64, output: *mut u32) -> i32 {
    status(|| {
        if output.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let count = with_registry(|r| {
            if !r.worlds.contains_key(&handle) {
                return Err(AbiError::InvalidHandle);
            }
            Ok(r.saves.get(&handle).map_or(0, |s| s.pending.len() as u32))
        })?;
        unsafe { output.write(count) };
        Ok(())
    })
}
/// # Safety
/// input is aligned/readable. Call from the stage loader after load-tail and
/// preparing carried registers. The previous slot survives any capture failure.
#[no_mangle]
pub unsafe extern "C" fn rl_autosalvar_inicio_fase(handle: u64, input: *const FrameInput) -> i32 {
    status(|| {
        if input.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        let input = unsafe { input.read() };
        with_registry(|r| {
            let world = r.worlds.get(&handle).ok_or(AbiError::InvalidHandle)?;
            let s = r.saves.entry(handle).or_default();
            let resume = ResumeState {
                input,
                pending: s.pending.clone(),
            };
            Ok(s.autosave.capture(world, &resume)?)
        })
    })
}
/// # Safety
/// Same copied-buffer/size-query contract as rl_salvar_bytes. This reads the
/// stage-start slot, not the current world, without consuming the slot.
#[no_mangle]
pub unsafe extern "C" fn rl_autosalvar_bytes(
    handle: u64,
    buffer: *mut u8,
    capacity: u32,
    required: *mut u32,
) -> i32 {
    status(|| {
        if required.is_null() {
            return Err(AbiError::InvalidArgument);
        }
        with_registry(|r| {
            if !r.worlds.contains_key(&handle) {
                return Err(AbiError::InvalidHandle);
            }
            let bytes = r
                .saves
                .get(&handle)
                .and_then(|s| s.autosave.bytes())
                .ok_or(AbiError::NoAutosave)?;
            unsafe { copy_bytes(bytes, buffer, capacity, required) }
        })
    })
}
/// # Safety
/// path is a readable UTF-8 span without NUL. Persists the captured stage-start slot.
#[no_mangle]
pub unsafe extern "C" fn rl_autosalvar_arquivo(
    handle: u64,
    path_bytes: *const u8,
    path_length: u32,
) -> i32 {
    status(|| {
        let path = unsafe { path(path_bytes, path_length) }?;
        let bytes = with_registry(|r| {
            if !r.worlds.contains_key(&handle) {
                return Err(AbiError::InvalidHandle);
            }
            r.saves
                .get(&handle)
                .and_then(|s| s.autosave.bytes())
                .map(<[u8]>::to_vec)
                .ok_or(AbiError::NoAutosave)
        })?;
        Ok(save::write_file(path, &bytes)?)
    })
}

#[cfg(test)]
#[path = "save_abi_tests.rs"]
mod tests;

pub(super) fn has_pending(registry: &Registry, handle: u64) -> bool {
    registry
        .saves
        .get(&handle)
        .is_some_and(|session| !session.pending.is_empty())
}
