//! Prepared-shell host policy and explicit campaign suspension; no platform services.
use super::*;
use rl_core::{
    render::{Cache, Error, Renderer},
    render_assets::Assets,
    shell::{self, Banks, Request, Screen},
    shell_briefing::Files,
    shell_input::Keys,
    FrameInput,
};
use std::{
    mem::{align_of, size_of},
    sync::Arc,
};

const VERSION: u32 = 1;
const PIXELS: usize = 480000;
const LIMIT: usize = 64 * 1024 * 1024;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RlShellBytes {
    pub data: *const u8,
    pub length: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RlShellFile {
    pub path: RlShellBytes,
    pub bytes: RlShellBytes,
}
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct RlShellInput {
    pub version: u32,
    pub keys: u32,
    pub mouse_x: u32,
    pub mouse_y: u32,
    pub mouse_ae: u32,
    pub mouse_af: u32,
    pub mouse_b0: u32,
    pub clock_7c: u32,
    pub animation_84: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RlShellEvent {
    pub generation: u64,
    pub kind: u32,
    pub mode: u32,
    pub path_length: u32,
    pub reserved: u32,
    pub path: [u8; 512],
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RlShellCredits {
    pub generation: u64,
    pub state: u32,
    pub frame: u32,
    pub frames: u32,
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
    pub reserved: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RlShellPlan {
    pub generation: u64,
    pub mode: u32,
    pub same_mode: u32,
    pub required_count: u32,
    pub required_mask: u32,
    pub required_calls: [u32; 4],
    pub background: [u8; 32],
}
impl Default for RlShellEvent {
    fn default() -> Self {
        Self {
            generation: 0,
            kind: 0,
            mode: 0,
            path_length: 0,
            reserved: 0,
            path: [0; 512],
        }
    }
}
thread_local! { static STOP_DETAIL: RefCell<Option<String>> = const { RefCell::new(None) }; }
fn status(f: impl FnOnce() -> Result<()>) -> i32 {
    STOP_DETAIL.with(|d| *d.borrow_mut() = None);
    let code = super::status(f);
    if code != 0 {
        STOP_DETAIL.with(|d| {
            if let Some(detail) = d.borrow_mut().take() {
                LAST_ERROR.with(|e| *e.borrow_mut() = detail);
            }
        });
    }
    code
}
struct Session {
    memory: MemoriaJogo,
    source: World,
    renderer: Renderer,
    banks: Banks,
    generation: u64,
    request: Request,
}
struct Sessions {
    entries: BTreeMap<u64, Session>,
}
static SESSIONS: OnceLock<Mutex<Sessions>> = OnceLock::new();
fn sessions<T>(f: impl FnOnce(&mut Sessions) -> Result<T>) -> Result<T> {
    let mut s = SESSIONS
        .get_or_init(|| {
            Mutex::new(Sessions {
                entries: BTreeMap::new(),
            })
        })
        .lock()
        .map_err(|_| AbiError::Poisoned)?;
    f(&mut s)
}
fn session<T>(
    handle: u64,
    generation: u64,
    f: impl FnOnce(&mut Session) -> Result<T>,
) -> Result<T> {
    sessions(|s| {
        let s = s.entries.get_mut(&handle).ok_or(AbiError::InvalidHandle)?;
        if s.generation != generation {
            return Err(AbiError::StaleGeneration);
        }
        f(s)
    })
}
/// Read-only view of a prepared session for the port's options screen
/// (options_abi.rs): its memory, renderer and banks, and whether a request is
/// pending. Never changes the session or its generation.
pub(crate) fn session_view<T>(
    handle: u64,
    f: impl FnOnce(&MemoriaJogo, &Renderer, &Banks, bool) -> Result<T>,
) -> Result<T> {
    sessions(|s| {
        let s = s.entries.get(&handle).ok_or(AbiError::InvalidHandle)?;
        f(&s.memory, &s.renderer, &s.banks, s.request != Request::None)
    })
}
pub(crate) fn render_error(e: Error) -> AbiError {
    error(e)
}
fn screen(mode: u32) -> Result<Screen> {
    match mode {
        2 => Ok(Screen::Main),
        3 => Ok(Screen::SinglePlayer),
        12 => Ok(Screen::LoadList),
        13 => Ok(Screen::LoadMenu),
        14 => Ok(Screen::SaveList),
        16 => Ok(Screen::Pause),
        17 => Ok(Screen::Briefing),
        19 => Ok(Screen::Result),
        23 => Ok(Screen::Objectives),
        24 => Ok(Screen::Final),
        10 => Ok(Screen::Credits),
        _ => Err(AbiError::InvalidArgument),
    }
}
fn error(e: Error) -> AbiError {
    match e {
        Error::Memory(e) => e.into(),
        Error::Invalid(s) => AbiError::RenderInput(s),
        Error::Stop(address, reason) => {
            // Preserve the original addressed diagnostic in the existing TLS channel.
            STOP_DETAIL.with(|d| *d.borrow_mut() = Some(format!("{reason} at 0x{address:08x}")));
            AbiError::UnsupportedPath(EngineError::UnportedCallee {
                name: "prepared shell",
                address,
            })
        }
    }
}
fn aligned<T>(p: *const T) -> Result<()> {
    if p.is_null() || !(p as usize).is_multiple_of(align_of::<T>()) {
        Err(AbiError::InvalidArgument)
    } else {
        Ok(())
    }
}
fn span(p: *const u8, n: usize) -> Result<(usize, usize)> {
    if n != 0 && p.is_null() {
        return Err(AbiError::InvalidArgument);
    }
    let a = p as usize;
    Ok((a, a.checked_add(n).ok_or(AbiError::InvalidArgument)?))
}
fn disjoint(spans: &[(usize, usize)]) -> Result<()> {
    for (i, &(a, b)) in spans.iter().enumerate() {
        for &(c, d) in &spans[..i] {
            if a < d && c < b && a != b && c != d {
                return Err(AbiError::InvalidArgument);
            }
        }
    }
    Ok(())
}
unsafe fn bytes<'a>(b: RlShellBytes, maximum: usize) -> Result<&'a [u8]> {
    if b.length as usize > maximum {
        return Err(AbiError::ResourceLimit);
    }
    span(b.data, b.length as usize)?;
    if b.length == 0 {
        Ok(&[])
    } else {
        Ok(unsafe { slice::from_raw_parts(b.data, b.length as usize) })
    }
}
fn next(s: &Session) -> Result<u64> {
    s.generation.checked_add(1).ok_or(AbiError::ResourceLimit)
}
fn validate_briefing_template(m: &MemoriaJogo) -> Result<()> {
    // Host boundary only: the original filename template has writable digit
    // positions12/13. Reject malformed projected input before native API indexing.
    if m.read(0x498430, 19)?
        .iter()
        .position(|&b| b == 0)
        .is_some_and(|n| n < 14)
    {
        return Err(AbiError::RenderInput(
            "shell briefing filename template is shorter than14 bytes",
        ));
    }
    Ok(())
}
fn event(request: Request, generation: u64) -> Result<RlShellEvent> {
    let mut e = RlShellEvent {
        generation,
        ..RlShellEvent::default()
    };
    match request {
        Request::None => {}
        Request::Show(s) => {
            e.kind = 1;
            e.mode = s as u32;
        }
        Request::ResetCampaign { next_mode } => {
            e.kind = 2;
            e.mode = next_mode as u32;
        }
        Request::StartMission => e.kind = 3,
        Request::MissionNext => e.kind = 4,
        Request::ResumeGame => e.kind = 5,
        Request::SaveLoadMenu { mode } => {
            e.kind = 6;
            e.mode = mode as u32;
        }
        Request::Introduction { path } => {
            if path.len() > e.path.len() {
                return Err(AbiError::ResourceLimit);
            }
            e.kind = 7;
            e.mode = 1;
            e.path_length = path.len() as u32;
            e.path[..path.len()].copy_from_slice(&path);
        }
        Request::Credits => e.kind = 8,
        Request::Multiplayer => e.kind = 9,
        Request::Quit => e.kind = 10,
        Request::DefeatMovie { path } => {
            if path.len() > e.path.len() {
                return Err(AbiError::ResourceLimit);
            }
            e.kind = 11;
            e.path_length = path.len() as u32;
            e.path[..path.len()].copy_from_slice(&path);
        }
        Request::LoadSlot {
            slot,
            from_menu,
            path,
        } => {
            if path.len() > e.path.len() {
                return Err(AbiError::ResourceLimit);
            }
            e.kind = 12;
            e.mode = if from_menu { 13 } else { 12 };
            e.reserved = u32::from(slot);
            e.path_length = path.len() as u32;
            e.path[..path.len()].copy_from_slice(&path);
        }
        Request::SaveSlot { slot, path } => {
            if path.len() > e.path.len() {
                return Err(AbiError::ResourceLimit);
            }
            e.kind = 13;
            e.mode = 14;
            e.reserved = u32::from(slot);
            e.path_length = path.len() as u32;
            e.path[..path.len()].copy_from_slice(&path);
        }
        Request::CampaignMovie { path } => {
            if path.len() > e.path.len() {
                return Err(AbiError::ResourceLimit);
            }
            e.kind = 14;
            e.mode = 24;
            e.path_length = path.len() as u32;
            e.path[..path.len()].copy_from_slice(&path);
        }
    }
    Ok(e)
}
// Immutable host VFS: exact raw path lookup, actual bounded binary read, close.
// A missing entry means an explicitly absent file, never a suppressed I/O error.
struct HostFiles {
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    active: Option<Vec<u8>>,
    position: usize,
}
impl Files for HostFiles {
    fn open(&mut self, path: &[u8], mode: &[u8]) -> rl_core::Result<bool> {
        if mode != b"rb" {
            return Err(EngineError::UnportedCallee {
                name: "shell host file mode",
                address: 0x458420,
            });
        }
        self.active = self.files.get(path).cloned();
        self.position = 0;
        Ok(self.active.is_some())
    }
    // Sequential reads continue from the file position, as the original fread does.
    fn read(&mut self, maximum: usize) -> rl_core::Result<Vec<u8>> {
        let data = self
            .active
            .as_ref()
            .map(|v| v[self.position.min(v.len())..(self.position + maximum).min(v.len())].to_vec())
            .ok_or(EngineError::UnportedCallee {
                name: "shell host read without open",
                address: 0x458420,
            })?;
        self.position += data.len();
        Ok(data)
    }
    fn close(&mut self) -> rl_core::Result<()> {
        self.active
            .take()
            .map(|_| ())
            .ok_or(EngineError::UnportedCallee {
                name: "shell host close without open",
                address: 0x458420,
            })
    }
    fn seek(&mut self, offset: u32, origin: u8) -> rl_core::Result<()> {
        if self.active.is_none() || origin != 0 {
            return Err(EngineError::UnportedCallee {
                name: "shell host seek outside the save-slot rewind",
                address: 0x45848b,
            });
        }
        self.position = offset as usize;
        Ok(())
    }
}
unsafe fn files(p: *const RlShellFile, count: u32) -> Result<HostFiles> {
    if count > 64 {
        return Err(AbiError::ResourceLimit);
    }
    let records = if count == 0 {
        &[]
    } else {
        aligned(p)?;
        unsafe { slice::from_raw_parts(p, count as usize) }
    };
    let mut files = BTreeMap::new();
    let mut total = 0;
    for record in records {
        let path = unsafe { bytes(record.path, 512) }?;
        let data = unsafe { bytes(record.bytes, LIMIT) }?;
        total += path.len() + data.len();
        if total > LIMIT {
            return Err(AbiError::ResourceLimit);
        }
        if path.is_empty()
            || path.contains(&0)
            || files.insert(path.to_vec(), data.to_vec()).is_some()
        {
            return Err(AbiError::InvalidArgument);
        }
    }
    Ok(HostFiles {
        files,
        active: None,
        position: 0,
    })
}
#[no_mangle]
pub extern "C" fn rl_shell_version() -> u32 {
    VERSION
}
/// # Safety
/// All pointers are aligned/valid for their declared spans. Outputs are disjoint.
/// Eleven blobs: existing nine render assets, MENU.CMP, INVENTE.CMP. Copied once.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_create(
    version: u32,
    world: u64,
    assets: *const RlShellBytes,
    count: u32,
    surface: RlShellBytes,
    output: *mut u64,
) -> i32 {
    status(|| {
        if version != VERSION {
            return Err(AbiError::VersionMismatch);
        }
        aligned(output)?;
        aligned(assets)?;
        if count != 11 || surface.length as usize != PIXELS {
            return Err(AbiError::InvalidArgument);
        }
        let a = unsafe { slice::from_raw_parts(assets, 11) };
        let mut total = 0;
        let blobs = a
            .iter()
            .map(|&b| {
                total += b.length as usize;
                if total > LIMIT {
                    return Err(AbiError::ResourceLimit);
                }
                unsafe { bytes(b, LIMIT) }
            })
            .collect::<Result<Vec<_>>>()?;
        let surface = unsafe { bytes(surface, PIXELS) }?;
        let source = with_world(world, |w| Ok(w.clone()))?;
        let memory = source.render_memory()?.into_owned();
        // Actual text initialization belongs to rl_carregar_textos on the source.
        if memory.u32(rl_core::text::TABLE)? == 0 {
            return Err(AbiError::RenderInput(
                "shell requires original TEXTES.TXT; call rl_carregar_textos",
            ));
        }
        let assets = Arc::new(Assets::load(&blobs[..9]).map_err(error)?);
        // Terrain cache is not read by prepared shell drawing; this is private render
        // scratch, not a fabricated game world or callback.
        let renderer = Renderer::new(
            assets,
            surface,
            Cache {
                keys: vec![0; 1024],
                raw: vec![0; 270336],
                cursor: 0,
            },
        )
        .map_err(error)?;
        let banks = Banks::load(blobs[9], blobs[10], &renderer).map_err(error)?;
        let handle = with_registry(|r| {
            let h = r.next;
            r.next = h.checked_add(1).ok_or(AbiError::ResourceLimit)?;
            Ok(h)
        })?;
        let handle = sessions(|r| {
            r.entries.insert(
                handle,
                Session {
                    memory,
                    source,
                    renderer,
                    banks,
                    generation: 0,
                    request: Request::None,
                },
            );
            Ok(handle)
        })?;
        unsafe {
            output.write(handle);
        }
        Ok(())
    })
}
/// # Safety
/// output is aligned/writable. Query only; required addresses retain original order.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_plan(handle: u64, mode: u32, output: *mut RlShellPlan) -> i32 {
    status(|| {
        aligned(output)?;
        let screen = screen(mode)?;
        let plan = sessions(|r| {
            let s = r.entries.get(&handle).ok_or(AbiError::InvalidHandle)?;
            let same = s.memory.u8(rl_core::shell_state::MODE)? == mode as u8;
            let calls = screen.required_game_calls();
            let mut p = RlShellPlan {
                generation: s.generation,
                mode,
                same_mode: u32::from(same),
                required_count: calls.len() as u32,
                required_mask: (1 << calls.len()) - 1,
                ..RlShellPlan::default()
            };
            for (out, call) in p.required_calls.iter_mut().zip(calls) {
                *out = call.address;
            }
            let path = screen.background(s.memory.u8(0x496fe4)?);
            p.background[..path.len()].copy_from_slice(path);
            Ok(p)
        })?;
        unsafe {
            output.write(plan);
        }
        Ok(())
    })
}
/// # Safety
/// Inputs are readable, output aligned/writable and disjoint; files are an exact
/// binary VFS. prepared_mask attests actual completion of plan.required_calls.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_enter(
    handle: u64,
    generation: u64,
    mode: u32,
    prepared_mask: u32,
    background: RlShellBytes,
    file_entries: *const RlShellFile,
    count: u32,
    output: *mut u64,
) -> i32 {
    status(|| {
        aligned(output)?;
        let screen = screen(mode)?;
        let background = unsafe { bytes(background, PIXELS) }?;
        let mut files = unsafe { files(file_entries, count) }?;
        let g = session(handle, generation, |s| {
            if s.memory.u8(rl_core::shell_state::MODE)? == mode as u8 {
                return Ok(s.generation);
            }
            if prepared_mask != (1 << screen.required_game_calls().len()) - 1 {
                return Err(AbiError::UnsupportedPath(EngineError::UnportedCallee {
                    name: "shell host must complete plan.required_calls in order",
                    address: 0x4682bb,
                }));
            }
            if matches!(screen, Screen::Briefing | Screen::Objectives) {
                validate_briefing_template(&s.memory)?;
            }
            let g = next(s)?;
            shell::enter_prepared(
                &mut s.memory,
                &mut s.renderer,
                &s.banks,
                screen,
                background,
                &mut files,
            )
            .map_err(error)?;
            s.generation = g;
            s.request = Request::None;
            Ok(g)
        })?;
        unsafe {
            output.write(g);
        }
        Ok(())
    })
}
/// # Safety
/// input/output are aligned, valid and disjoint. Keys are completed logical actions.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_input(
    handle: u64,
    generation: u64,
    input: *const RlShellInput,
    output: *mut RlShellEvent,
) -> i32 {
    status(|| {
        aligned(input)?;
        aligned(output)?;
        disjoint(&[
            span(input.cast(), size_of::<RlShellInput>())?,
            span(output.cast(), size_of::<RlShellEvent>())?,
        ])?;
        let i = unsafe { input.read() };
        if i.version != VERSION {
            return Err(AbiError::VersionMismatch);
        }
        if i.keys > 15
            || i.mouse_x > 65535
            || i.mouse_y > 65535
            || i.mouse_ae > 255
            || i.mouse_af > 255
            || i.mouse_b0 > 255
            || i.reserved != 0
        {
            return Err(AbiError::InvalidArgument);
        }
        let e = session(handle, generation, |s| {
            let g = next(s)?;
            let mut memory = s.memory.clone();
            memory.set_u16(0x5099a8, i.mouse_x as u16)?;
            memory.set_u16(0x5099aa, i.mouse_y as u16)?;
            memory.set_u8(0x5099ae, i.mouse_ae as u8)?;
            memory.set_u8(0x5099af, i.mouse_af as u8)?;
            memory.set_u8(0x5099b0, i.mouse_b0 as u8)?;
            memory.set_u32(0x508f7c, i.clock_7c)?;
            memory.set_u32(0x508f84, i.animation_84)?;
            let request = shell::input(
                &mut memory,
                Keys {
                    enter: i.keys & 1 != 0,
                    up: i.keys & 2 != 0,
                    down: i.keys & 4 != 0,
                    escape: i.keys & 8 != 0,
                },
            )
            .map_err(error)?;
            let e = event(request.clone(), g)?;
            // Prepared API semantics: common-tail 0x5099b0 clear is host-owned.
            s.memory = memory;
            s.generation = g;
            s.request = request;
            Ok(e)
        })?;
        unsafe {
            output.write(e);
        }
        Ok(())
    })
}
/// # Safety
/// output is aligned/writable. Draw only; copying/querying never advances animation.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_draw(
    handle: u64,
    generation: u64,
    animation_84: u32,
    output: *mut u64,
) -> i32 {
    status(|| {
        aligned(output)?;
        let g = session(handle, generation, |s| {
            let g = next(s)?;
            // Preserve the caller counter on failure, including draw's inner rollback.
            let old = s.memory.u32(0x508f84)?;
            s.memory.set_u32(0x508f84, animation_84)?;
            if let Err(e) = shell::draw(&mut s.memory, &mut s.renderer, &s.banks) {
                s.memory.set_u32(0x508f84, old)?;
                return Err(error(e));
            }
            s.generation = g;
            Ok(g)
        })?;
        unsafe {
            output.write(g);
        }
        Ok(())
    })
}

/// # Safety
/// Original decoded background is readable; output is aligned/writable and
/// disjoint. Repaint the current prepared mode without repeating MenuInit.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_paint_prepared(
    handle: u64,
    generation: u64,
    background: RlShellBytes,
    output: *mut u64,
) -> i32 {
    status(|| {
        aligned(output)?;
        disjoint(&[
            span(background.data, background.length as usize)?,
            span(output.cast(), size_of::<u64>())?,
        ])?;
        let background = unsafe { bytes(background, PIXELS) }?;
        let g = session(handle, generation, |s| {
            let g = next(s)?;
            shell::paint_prepared(&s.memory, &mut s.renderer, &s.banks, background)
                .map_err(error)?;
            s.generation = g;
            Ok(g)
        })?;
        unsafe { output.write(g) };
        Ok(())
    })
}

/// # Safety
/// Exact binary VFS and next ETA path are readable, output aligned/writable and
/// disjoint. Completes an actual MISSION_NEXT request, stopping at mode17.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_campaign_prepare(
    handle: u64,
    generation: u64,
    name: RlShellBytes,
    file_entries: *const RlShellFile,
    count: u32,
    output: *mut u64,
) -> i32 {
    status(|| {
        aligned(output)?;
        let name = unsafe { bytes(name, 512) }?;
        let mut files = unsafe { files(file_entries, count) }?;
        if name.is_empty() || name.contains(&0) {
            return Err(AbiError::InvalidArgument);
        }
        let g = session(handle, generation, |s| {
            if s.request != Request::MissionNext
                || s.memory.u8(rl_core::shell_state::MODE)? != 19
                || s.memory.u8(0x4e05a0)? == 0
                || s.memory.u8(0x4e1d84)? == 0
            {
                return Err(AbiError::InvalidArgument);
            }
            validate_briefing_template(&s.memory)?;
            let g = next(s)?;
            let mut memory = s.memory.clone();
            // The name must be the engine's own MakeMissionName path; an absent
            // VFS entry for it means the campaign ended (original mode24).
            let expected = rl_core::mission_entry::next_name(&s.source)?;
            if name != expected.as_slice() {
                return Err(AbiError::UnsupportedPath(EngineError::UnportedCallee {
                    name: "Campaign next ETA path mismatch",
                    address: 0x46c08c,
                }));
            }
            let present = files.files.contains_key(name);
            rl_core::campaign_flow::next(
                &mut memory,
                |path| Ok(path == name && present),
                &mut files,
            )?;
            s.memory = memory;
            s.generation = g;
            s.request = Request::None;
            Ok(g)
        })?;
        unsafe { output.write(g) };
        Ok(())
    })
}

/// # Safety
/// Exact binary VFS readable, output aligned/writable. Completes the pending
/// SHOW request with the original MenuInit(local player, mode) gameplay effects.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_menu_init(
    handle: u64,
    generation: u64,
    file_entries: *const RlShellFile,
    count: u32,
    output: *mut u64,
) -> i32 {
    status(|| {
        aligned(output)?;
        let mut files = unsafe { files(file_entries, count) }?;
        let g = session(handle, generation, |s| {
            let screen = match s.request {
                Request::Show(screen) => screen,
                // Save/load list requests from the pause menu enter the original lists.
                Request::SaveLoadMenu { mode } => screen(u32::from(mode))?,
                // Main-menu "Credits": MenuInit(10) with CREDITS.FLC from the VFS.
                Request::Credits if s.memory.u8(rl_core::shell_state::MODE)? == 2 => {
                    Screen::Credits
                }
                _ => return Err(AbiError::InvalidArgument),
            };
            if matches!(screen, Screen::Briefing | Screen::Objectives) {
                validate_briefing_template(&s.memory)?;
            }
            let g = next(s)?;
            let mut memory = s.memory.clone();
            let player = memory.u8(0x508041)?;
            rl_core::campaign_flow::menu_init(&mut memory, player, screen as u8, &mut files)?;
            s.memory = memory;
            s.generation = g;
            s.request = Request::None;
            Ok(g)
        })?;
        unsafe { output.write(g) };
        Ok(())
    })
}

/// # Safety
/// Exact binary VFS readable, output aligned/writable. Completes the pending
/// RESET_CAMPAIGN request: MissionReset, player types, MenuInit(17) or MenuInit(13).
#[no_mangle]
pub unsafe extern "C" fn rl_shell_campaign_reset(
    handle: u64,
    generation: u64,
    file_entries: *const RlShellFile,
    count: u32,
    output: *mut u64,
) -> i32 {
    status(|| {
        aligned(output)?;
        let mut files = unsafe { files(file_entries, count) }?;
        let g = session(handle, generation, |s| {
            let Request::ResetCampaign { next_mode } = s.request else {
                return Err(AbiError::InvalidArgument);
            };
            if s.memory.u8(rl_core::shell_state::MODE)? != 3 {
                return Err(AbiError::InvalidArgument);
            }
            if next_mode == 17 {
                validate_briefing_template(&s.memory)?;
            }
            let g = next(s)?;
            let mut memory = s.memory.clone();
            let player = memory.u8(0x508041)?;
            rl_core::campaign_flow::reset_campaign(&mut memory, player, next_mode, &mut files)?;
            s.memory = memory;
            s.generation = g;
            s.request = Request::None;
            Ok(g)
        })?;
        unsafe { output.write(g) };
        Ok(())
    })
}

/// # Safety
/// input readable/aligned; outputs aligned/writable/disjoint. Completes a pending
/// RESUME request: original MenuInit(18) from the pause menu, returning the
/// resumed world and its host events (picture, music) for rl_eventos_quadro.
/// Also completes a pending SAVE_SLOT request from the in-game save list14 after
/// the host has written its own save: the original MenuInit(18) plus the HUD
/// refresh latch (MenuNewSelect 0x42c570), same world/events contract.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_resume(
    handle: u64,
    generation: u64,
    input: *const FrameInput,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    status(|| {
        aligned(input)?;
        aligned(output)?;
        aligned(input_out)?;
        disjoint(&[
            span(input.cast(), size_of::<FrameInput>())?,
            span(output.cast(), size_of::<u64>())?,
            span(input_out.cast(), size_of::<FrameInput>())?,
        ])?;
        let input = unsafe { input.read() };
        let world = session(handle, generation, |s| {
            let mode = s.memory.u8(rl_core::shell_state::MODE)?;
            let save = match s.request {
                Request::ResumeGame if mode == 16 => false,
                Request::SaveSlot { .. } if mode == 14 => true,
                _ => return Err(AbiError::InvalidArgument),
            };
            let g = next(s)?;
            let mut memory = s.memory.clone();
            let events = if save {
                rl_core::campaign_flow::save_slot_resume(&mut memory, &input)?
            } else {
                rl_core::campaign_flow::resume_game(&mut memory, &input)?
            };
            let mut world = s.source.clone();
            world.replace_memory(memory.clone())?;
            world.frame_events = events;
            s.memory = memory;
            s.generation = g;
            s.request = Request::None;
            Ok(world)
        })?;
        let handle = super::mission_abi::publish(world)?;
        unsafe {
            output.write(handle);
            input_out.write(input);
        }
        Ok(())
    })
}

/// # Safety
/// output is aligned/writable. Completion0 means host-unavailable;1 means
/// finished playback. Both continue the actual pending defeat through MenuInit2.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_defeat_finish(
    handle: u64,
    generation: u64,
    completion: u32,
    output: *mut u64,
) -> i32 {
    status(|| {
        aligned(output)?;
        if completion > 1 {
            return Err(AbiError::InvalidArgument);
        }
        let g = session(handle, generation, |s| {
            let mode = s.memory.u8(rl_core::shell_state::MODE)?;
            let defeat = matches!(&s.request, Request::DefeatMovie { .. })
                && mode == 19
                && s.memory.u8(0x4e05a0)? != 0
                && s.memory.u8(0x4e1d84)? == 0;
            let campaign_end = matches!(&s.request, Request::CampaignMovie { .. }) && mode == 24;
            let introduction = matches!(&s.request, Request::Introduction { .. }) && mode == 2;
            if !defeat && !campaign_end && !introduction {
                return Err(AbiError::InvalidArgument);
            }
            let g = next(s)?;
            let mut memory = s.memory.clone();
            if introduction {
                // MenuInit(1), CalculGame mode1 (absent: DDSetPalette; present:
                // FileClose + PlayVideo), then MenuInit(2) at 0x42c925.
                let player = memory.u8(0x508041)?;
                rl_core::campaign_flow::introduction(&mut memory, player, completion == 1)?;
            } else {
                rl_core::campaign_flow::main_menu(&mut memory)?;
            }
            s.memory = memory;
            s.generation = g;
            s.request = Request::None;
            Ok(g)
        })?;
        unsafe { output.write(g) };
        Ok(())
    })
}

/// # Safety
/// output is aligned/writable. One AffCredits call in mode10 with the host's
/// milliseconds since its last FLC frame read (since the open before the first). ENDED leaves a pending SHOW main
/// request (complete it with rl_shell_menu_init); DRAW asks the host to show FLC
/// frame `frame` at (x, y) after the engine ran DDFlipPage's gameplay prefix;
/// HOLD draws nothing. Failure leaves the session unchanged.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_credits_frame(
    handle: u64,
    generation: u64,
    elapsed_ms: u32,
    output: *mut RlShellCredits,
) -> i32 {
    status(|| {
        aligned(output)?;
        let out = session(handle, generation, |s| {
            if s.request != Request::None {
                return Err(AbiError::InvalidArgument);
            }
            let g = next(s)?;
            let mut memory = s.memory.clone();
            let frame = rl_core::campaign_flow::credits_frame(&mut memory, elapsed_ms)?;
            let mut out = RlShellCredits {
                generation: g,
                x: rl_core::campaign_flow::CREDITS_X,
                y: rl_core::campaign_flow::CREDITS_Y,
                ..RlShellCredits::default()
            };
            match frame {
                rl_core::campaign_flow::CreditsFrame::Ended => {
                    out.state = 0;
                    s.request = Request::Show(Screen::Main);
                }
                rl_core::campaign_flow::CreditsFrame::Draw {
                    frame,
                    frames,
                    width,
                    height,
                } => {
                    out.state = 1;
                    out.frame = frame.into();
                    out.frames = frames.into();
                    out.width = width.into();
                    out.height = height.into();
                }
                rl_core::campaign_flow::CreditsFrame::Hold { frame, frames } => {
                    out.state = 2;
                    out.frame = frame.into();
                    out.frames = frames.into();
                }
            }
            s.memory = memory;
            s.generation = g;
            Ok(out)
        })?;
        unsafe { output.write(out) };
        Ok(())
    })
}

/// # Safety
/// As rl_shell_campaign_load. Same load, then the stage-start difficulty preset
/// inside the same transaction (Normal leaves the loaded bytes unchanged).
#[no_mangle]
pub unsafe extern "C" fn rl_shell_campaign_load_dificuldade(
    handle: u64,
    generation: u64,
    name: RlShellBytes,
    eta: RlShellBytes,
    monsters: RlShellBytes,
    input: *const FrameInput,
    preset: u32,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    status(|| {
        let preset =
            rl_core::difficulty::Preset::try_from(preset).map_err(|_| AbiError::InvalidArgument)?;
        unsafe {
            load_stage(
                handle,
                generation,
                name,
                eta,
                monsters,
                input,
                output,
                input_out,
                Some(preset),
            )
        }
    })
}

/// # Safety
/// Original path/ETA/MONSTRE spans and carried input are readable. Outputs are
/// aligned/writable/disjoint. Session is unchanged; output is a new world at
/// MissionLoad's return, before the public load-tail transaction.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_campaign_load(
    handle: u64,
    generation: u64,
    name: RlShellBytes,
    eta: RlShellBytes,
    monsters: RlShellBytes,
    input: *const FrameInput,
    output: *mut u64,
    input_out: *mut FrameInput,
) -> i32 {
    status(|| unsafe {
        load_stage(
            handle, generation, name, eta, monsters, input, output, input_out, None,
        )
    })
}
#[allow(clippy::too_many_arguments)]
unsafe fn load_stage(
    handle: u64,
    generation: u64,
    name: RlShellBytes,
    eta: RlShellBytes,
    monsters: RlShellBytes,
    input: *const FrameInput,
    output: *mut u64,
    input_out: *mut FrameInput,
    preset: Option<rl_core::difficulty::Preset>,
) -> Result<()> {
    aligned(input)?;
    aligned(output)?;
    aligned(input_out)?;
    disjoint(&[
        span(name.data, name.length as usize)?,
        span(eta.data, eta.length as usize)?,
        span(monsters.data, monsters.length as usize)?,
        span(input.cast(), size_of::<FrameInput>())?,
        span(output.cast(), size_of::<u64>())?,
        span(input_out.cast(), size_of::<FrameInput>())?,
    ])?;
    let name = unsafe { bytes(name, 512) }?;
    let eta = unsafe { bytes(eta, LIMIT) }?;
    let monsters = unsafe { bytes(monsters, LIMIT) }?;
    let input = unsafe { input.read() };
    let (world, carried) = session(handle, generation, |s| {
        if s.request != Request::StartMission
            || input.cursor_x as u16 != s.memory.u16(0x5099a8)?
            || input.cursor_y as u16 != s.memory.u16(0x5099aa)?
        {
            return Err(AbiError::InvalidArgument);
        }
        let mut source = s.source.clone();
        source.replace_memory(s.memory.clone())?;
        Ok(match preset {
            Some(preset) => {
                rl_core::difficulty::load_prepared(&source, name, eta, monsters, input, preset)?
            }
            None => rl_core::mission_entry::load_prepared(&source, name, eta, monsters, input)?,
        })
    })?;
    let handle = super::mission_abi::publish(world)?;
    unsafe {
        output.write(handle);
        input_out.write(carried);
    }
    Ok(())
}
/// # Safety
/// Buffers are writable for capacities; output spans must be disjoint. No partial
/// copies, no metadata writes on error. Fixed sizes: 480000 indices +768 RGB bytes.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_copy(
    handle: u64,
    generation: u64,
    pixels: *mut u8,
    pixel_capacity: u32,
    palette: *mut u8,
    palette_capacity: u32,
) -> i32 {
    status(|| {
        session(handle, generation, |s| {
            if pixel_capacity < PIXELS as u32 || palette_capacity < 768 {
                return Err(AbiError::BufferTooSmall);
            }
            disjoint(&[span(pixels, PIXELS)?, span(palette, 768)?])?;
            unsafe {
                ptr::copy_nonoverlapping(s.renderer.surface.as_ptr(), pixels, PIXELS);
                ptr::copy_nonoverlapping(s.renderer.assets.palette.as_ptr(), palette, 768);
            }
            Ok(())
        })
    })
}
/// # Safety
/// output is aligned/writable. Fresh world snapshot, source/session unchanged.
/// This does not execute any returned request or perform mode18/start preparation.
#[no_mangle]
pub unsafe extern "C" fn rl_shell_export_world(
    handle: u64,
    generation: u64,
    output: *mut u64,
) -> i32 {
    status(|| {
        aligned(output)?;
        let handle = session(handle, generation, |s| {
            let mut world = s.source.clone();
            let phase = world.frame_phase;
            world.replace_memory(s.memory.clone())?;
            world.frame_phase = phase;
            with_registry(|r| {
                let handle = r.next;
                r.next = handle.checked_add(1).ok_or(AbiError::ResourceLimit)?;
                r.worlds.insert(handle, world);
                Ok(handle)
            })
        })?;
        unsafe {
            output.write(handle);
        }
        Ok(())
    })
}
#[no_mangle]
pub extern "C" fn rl_shell_free(handle: u64) -> i32 {
    status(|| {
        sessions(|s| {
            s.entries
                .remove(&handle)
                .map(|_| ())
                .ok_or(AbiError::InvalidHandle)
        })
    })
}

#[cfg(test)]
#[path = "shell_abi_tests.rs"]
mod shell_abi_tests;

#[cfg(test)]
#[path = "shell_campaign_tests.rs"]
mod shell_campaign_tests;

#[cfg(test)]
#[path = "shell_pause_save_tests.rs"]
mod shell_pause_save_tests;
