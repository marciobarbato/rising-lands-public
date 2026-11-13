//! Original mission-end gameplay preparation and explicit shell host boundary.
//! File/video/device ownership stays with the host; no game rule runs there.
use crate::{
    frame::stop,
    shell,
    shell_briefing::{self, cstring, Files},
    shell_input::Keys,
    shell_menu, shell_state, text, FrameRegisters, MemoriaJogo, Result,
};

/// PlaySnd_StopAllSound 48aac5 -> 48a4a8, in the engine's unopened audio domain.
/// Active device and queued sample destruction remain explicit dependencies.
/// The call is published to the host first as a kind-10 player event whose
/// first value is 2 (stop all; 1 is a post, 0 a stop of one handle) at the
/// original call `site`: 0x429335 InputGame, 0x42d4c8 CalculGame,
/// 0x4580b7 EvenementEnd, 0x45837a / 0x458390 EvenementAffiche, 0x47c83a PlayVideo.
pub fn stop_sounds(m: &mut MemoriaJogo, site: u32) -> Result<()> {
    crate::player::event(m, 10, site, &[2, 0, 0, 0, 0, 0, 0]);
    for i in 0..50 {
        let p = 0x52137c + 44 * i;
        let state = m.u8(p + 8)?;
        if state == 0 {
            continue;
        }
        if state == 1 {
            return stop("StopAllSound active device", 0x489e35);
        }
        let reference = m.u32(p + 40)?;
        if m.u32(reference)? == u32::MAX {
            continue;
        }
        if state == 2 {
            return stop("StopAllSound queued sample", 0x48af1b);
        }
        m.set_u32(reference, u32::MAX)?;
        m.set_u8(p + 8, 0)?;
        m.set_u32(p + 32, 0)?;
    }
    Ok(())
}

/// MenuInit common tail 468c60..468cf3. Its one DDFlipPage gameplay prefix
/// runs here, before publication, and must not be consumed again by drawing.
fn finish_menu(m: &mut MemoriaJogo, mode: u8) -> Result<()> {
    m.set_u8(shell_state::MODE, mode)?;
    let margin = m.u8(0x508f90)?;
    if margin != 0 {
        m.set_u16(0x4e2f7a, margin as i8 as i16 as u16)?;
        m.set_u8(0x508f90, 0)?;
    }
    crate::load_tail_input::menu_flip_effects_v1(m)?;
    crate::shell_menu::reset(m)
}

/// CalculGame 42d4c8 and MenuInit mode19. Caption/background pixels are drawn
/// by the shell presenter; this routine preserves the actual result latch.
pub fn result(m: &mut MemoriaJogo) -> Result<()> {
    stop_sounds(m, 0x42d4c8)?;
    if m.u16(0x49a018)? == 640 {
        return stop("MenuInit result 640-wide device transition", 0x468acf);
    }
    finish_menu(m, 19)
}

/// CalculGame 0x42ca6e: MissionNext with the host FileExists answer. Success
/// stores the carry-over and enters the briefing (mode17); an absent next stage
/// enters the original campaign-end screen MenuInit(24), whose movie request the
/// shell raises on the next frame. Returns whether a next stage exists.
pub fn next(
    m: &mut MemoriaJogo,
    exists: impl FnMut(&[u8]) -> Result<bool>,
    files: &mut impl crate::shell_briefing::Files,
) -> Result<bool> {
    if !crate::mission_campaign::advance_after_victory(m, exists)? {
        // MenuInit(local player,24) 0x42ca9d: generic picture branch, plain tail.
        free_credits(m)?;
        if m.u16(0x49a018)? == 640 {
            return stop("MenuInit 640-wide device transition", 0x468bff);
        }
        cd_stop(m)?;
        finish_menu(m, 24)?;
        return Ok(false);
    }
    m.set_u8(0x4e1d84, 0)?;
    crate::shell_briefing::load(m, m.u8(0x496fe4)?, files)?;
    shell_state::zones(
        m,
        FrameRegisters {
            eax: 2,
            ..Default::default()
        },
    )?;
    crate::mission_research::recalculate(m)?;
    finish_menu(m, 17)?;
    Ok(true)
}

/// Absent/finished defeat movie -> MenuInit2. Disconnected mpClose performs
/// its buffer clears; network connections never become no-op callbacks.
pub fn main_menu(m: &mut MemoriaJogo) -> Result<()> {
    finish_menu(m, 2)?;
    for p in [0x50803c, 0x508038, 0x508034] {
        if m.u32(p)? != 0 {
            return stop("mpClose live network", 0x46ce60);
        }
    }
    for (p, n) in [
        (0x507e7c, 424),
        (0x507e6c, 16),
        (0x507e68, 4),
        (0x508024, 16),
    ] {
        m.write(p, &vec![0; n])?;
    }
    shell_state::configure(m, 2)?;
    shell_state::zones(
        m,
        FrameRegisters {
            eax: 3,
            ..Default::default()
        },
    )?;
    // RechercheReset 470b46, including BonusReset.
    for p in 0..4 {
        for o in 0..5 {
            m.set_u8(0x508134 + 16 * p + o, 0)?;
        }
        for o in [8, 12] {
            m.set_u32(0x508134 + 16 * p + o, 0)?;
        }
        m.set_u8(0x50871c + p, 0)?;
    }
    crate::chain_technology::Technology::default().entry(
        m,
        "BonusReset",
        FrameRegisters::default(),
    )?;
    for p in 0..4 {
        for i in 0..5 {
            m.set_u16(0x4e19ec + 10 * p + 2 * i, 255)?;
        }
    }
    crate::mission_boot::players(m)?;
    Ok(())
}

/// Execute only the original shell CalculGame block. The caller resolves the
/// returned host request before publishing this candidate and the next surface.
pub fn input(m: &mut MemoriaJogo, keys: Keys) -> crate::render::Result<shell::Request> {
    let request = shell::input(m, keys)?;
    m.set_u8(0x5099b0, 0)?;
    Ok(request)
}

/// A host without files for MenuInit targets that open none (modes2/3/16).
pub struct NoFiles;
impl Files for NoFiles {
    fn open(&mut self, _: &[u8], _: &[u8]) -> Result<bool> {
        stop("MenuInit target opens no file", 0x458420)
    }
    fn read(&mut self, _: usize) -> Result<Vec<u8>> {
        stop("MenuInit target reads no file", 0x458461)
    }
    fn close(&mut self) -> Result<()> {
        stop("MenuInit target closes no file", 0x45844c)
    }
}

/// GameSaveScanNames 0x46bb3b over the ten original slot paths at0x497128.
/// Version3+ "SAV" headers carry the32-byte name; older or foreign headers get
/// the original hard-coded French default; absent files get text0x72. Reads are
/// the host boundary; short header reads keep their explicit stop.
fn save_slot_names(m: &mut MemoriaJogo, files: &mut impl Files) -> Result<()> {
    let mode = cstring(m, 0x494b06, 4)?;
    for slot in 0..10u32 {
        let destination = 0x496fe8 + 32 * slot;
        let path = cstring(m, m.u32(0x497128 + 4 * slot)?, 256)?;
        if !files.open(&path, &mode)? {
            let mut name = cstring(m, text::get(m, 0x72)?, 256)?;
            name.push(0);
            m.write(destination, &name)?;
            continue;
        }
        let header = files.read(4)?;
        if header.len() != 4 {
            return stop("GameSaveScanNames short header read", 0x46bbf6);
        }
        let version = if &header[..3] == b"SAV" {
            i32::from(header[3]) - 0x30
        } else {
            files.seek(0, 0)?;
            0
        };
        if version >= 3 {
            let name = files.read(32)?;
            if name.len() > 32 {
                return stop("GameSaveScanNames oversized name read", 0x46bb6d);
            }
            m.write(destination, &name)?;
        } else {
            let mut name = format!("Sauvegarde {}", slot + 1).into_bytes();
            name.push(0);
            m.write(destination, &name)?;
        }
        files.close()?;
    }
    Ok(())
}

/// MCICD_Stop 0x467a4d: only an available, playing CD device has an effect. The
/// device command is a host boundary, so a live CD track remains an explicit stop.
pub(crate) fn cd_stop(m: &MemoriaJogo) -> Result<()> {
    if m.u8(0x4e2f29)? != 0 && m.u8(0x4e2f28)? != 0 {
        return stop("MenuInit CD audio stop", 0x467a66);
    }
    Ok(())
}

/// MenuInit 0x4682bb gameplay effects for a shell host. `player` is the original
/// AL argument: a mismatch with the local player0x508041 or a same-mode target
/// returns Ok(false) without any effect, as the original does. Cursor, surface
/// lock, pictures, palette, bank handles and the research/caption pixels belong
/// to the host presenter; the DDFlipPage gameplay prefix runs here. Targets
/// outside the prepared shell keep their original addressed stop.
pub fn menu_init(
    m: &mut MemoriaJogo,
    player: u8,
    mode: u8,
    files: &mut impl Files,
) -> Result<bool> {
    let current = m.u8(shell_state::MODE)?;
    if player != m.u8(0x508041)? || current == mode {
        return Ok(false);
    }
    if matches!(current, 6 | 7) {
        return stop("MenuInit multiplayer source bitmaps", 0x4682f5);
    }
    match mode {
        17 | 23 => {
            m.set_u8(0x4e1d84, 0)?;
            shell_briefing::load(m, m.u8(0x496fe4)?, files)?;
            shell_state::zones(
                m,
                FrameRegisters {
                    eax: 2,
                    ..Default::default()
                },
            )?;
            if mode == 17 {
                crate::mission_research::recalculate(m)?;
            }
            finish_menu(m, mode)?;
        }
        // 0x468b52: the introduction target takes the plain tail.
        1 => finish_menu(m, 1)?,
        // 0x468826: DeleteBank/LoadPie(credits.pie) are presentation; CreateCredits.
        10 => {
            create_credits(m, files)?;
            finish_menu(m, 10)?;
        }
        2 | 3 | 12 | 13 | 14 | 16 => {
            // 0x468b64 FreeCredits, then the bank/picture/device presentation.
            free_credits(m)?;
            if m.u16(0x49a018)? == 640 {
                return stop("MenuInit 640-wide device transition", 0x468bff);
            }
            cd_stop(m)?;
            match mode {
                2 => main_menu(m)?,
                3 | 16 => {
                    finish_menu(m, mode)?;
                    shell_state::configure(m, mode)?;
                    shell_state::zones(
                        m,
                        FrameRegisters {
                            eax: 3,
                            ..Default::default()
                        },
                    )?;
                }
                _ => {
                    finish_menu(m, mode)?;
                    save_slot_names(m, files)?;
                    m.set_u32(shell_menu::TITLE, text::get(m, 0x1f)?)?;
                    for slot in 0..10 {
                        shell_menu::add_item(m, 0x496fe8 + 32 * slot)?;
                    }
                    m.set_u8(0x4e2f74, 5)?;
                    m.set_u8(0x4e2f75, 1)?;
                    m.set_u8(0x4e2f76, 2)?;
                    shell_state::zones(
                        m,
                        FrameRegisters {
                            eax: 3,
                            ..Default::default()
                        },
                    )?;
                }
            }
        }
        _ => return stop("MenuInit unported target mode", 0x4682bb),
    }
    Ok(true)
}

/// CreateCredits 0x43ec48: OpenFli(<CD root>\Events\CREDITS.FLC) into 0x4e1780;
/// an opened animation stops the CD track and zeroes the frame counter 0x4e1784.
/// The host VFS answers the CRT open(path, O_BINARY) and the 128-byte header read;
/// the descriptor and later frame reads stay the host's (FLC decoding is
/// presentation, as for the in-game events). A file shorter than the header leaves
/// uninitialised GlobalAlloc bytes in the original: explicit stop.
fn create_credits(m: &mut MemoriaJogo, files: &mut impl Files) -> Result<()> {
    let mut path = cstring(m, 0x509594, 256)?;
    path.extend(cstring(m, 0x49448c, 256)?);
    path.extend(cstring(m, 0x494495, 256)?);
    // CRT open(path, 0x200 O_BINARY): a binary read-only open, the VFS "rb".
    let header = if files.open(&path, b"rb")? {
        let header = files.read(128)?;
        files.close()?;
        if header.len() != 128 {
            return stop("OpenFli short FLC header read", 0x45a659);
        }
        Some(header)
    } else {
        None
    };
    let handle = crate::event_flc::open_header(m, header.as_deref())?;
    m.set_u32(0x4e1780, handle)?;
    if handle != 0 {
        cd_stop(m)?;
        m.set_u16(0x4e1784, 0)?;
    }
    Ok(())
}

/// FreeCredits 0x43ecf0: CloseFli on the credits animation, then 0x4e1780 = 0.
pub(crate) fn free_credits(m: &mut MemoriaJogo) -> Result<()> {
    let handle = m.u32(0x4e1780)?;
    if handle == 0 {
        return Ok(());
    }
    if handle != crate::event_flc::HANDLE {
        return stop("FreeCredits handle outside the FLC allocation", 0x43ecfc);
    }
    crate::event_flc::close_handle(m)?;
    m.set_u32(0x4e1780, 0)
}

/// One AffCredits 0x43ed0e call (AffGame mode10 0x417ed0), engine part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreditsFrame {
    /// No animation (0x4e1780 == 0): AffCredits returns 0 and AffGame calls
    /// MenuInit(local player, 2) at 0x417ee5.
    Ended,
    /// The counter reached the frame count: nothing is drawn, AffCredits
    /// returns 1 and the last picture stays until Escape or a click.
    Hold { frame: u16, frames: u16 },
    /// Draw FLC frame `frame` (sequential decode) at x 464, y 155; DDFlipPage(1).
    Draw {
        frame: u16,
        frames: u16,
        width: u16,
        height: u16,
    },
}
/// Destination of the AffCredits row copy 0x43ed5a: surface + pitch*155 + 0x1d0.
pub const CREDITS_X: u32 = 0x1d0;
pub const CREDITS_Y: u32 = 155;

/// AffCredits 0x43ed0e. GetNextNbFrameEllapsedFli 0x45abb1 is
/// (timeGetTime - handle+0x90) / speed, and ReadFli stores timeGetTime at
/// handle+0x90 after every frame it reads (0x45aad7), so `elapsed_ms` is the
/// host's milliseconds since its last FLC frame read (since the open before the
/// first). The handle's read position/time fields stay the host decoder's.
pub fn credits_frame(m: &mut MemoriaJogo, elapsed_ms: u32) -> Result<CreditsFrame> {
    if m.u8(shell_state::MODE)? != 10 {
        return stop("AffCredits outside mode10", 0x417ed0);
    }
    let handle = m.u32(0x4e1780)?;
    if handle == 0 {
        return Ok(CreditsFrame::Ended);
    }
    if handle != crate::event_flc::HANDLE {
        return stop("AffCredits handle outside the FLC allocation", 0x43ed24);
    }
    let speed = m.u32(handle + 0x10)?;
    if speed == 0 {
        // div ecx with a zero FLC speed: integer-divide fault in the original.
        return stop("GetNextNbFrameEllapsedFli zero FLC speed", 0x45abc8);
    }
    let elapsed = elapsed_ms / speed;
    let frame = m.u16(0x4e1784)?.wrapping_add(elapsed as u16);
    m.set_u16(0x4e1784, frame)?;
    let frames = m.u16(handle + 6)?;
    if frame >= frames {
        return Ok(CreditsFrame::Hold { frame, frames });
    }
    // DDLockSurface/DisplayFliPict/row copy/DDUnlockSurface are presentation;
    // DDFlipPage(1) runs its gameplay prefix.
    crate::load_tail_input::menu_flip_effects_v1(m)?;
    Ok(CreditsFrame::Draw {
        frame,
        frames,
        width: m.u16(handle + 8)?,
        height: m.u16(handle + 10)?,
    })
}

/// CalculGame mode10 0x42c979: Escape (GetAsyncKeyState, the host's completed
/// key) or a click (0x5099ae and 0x5099af) calls MenuInit(local player, 2).
pub fn credits_input(m: &MemoriaJogo, escape: bool) -> Result<bool> {
    Ok(escape || (m.u8(0x5099ae)? != 0 && m.u8(0x5099af)? != 0))
}

/// PlayVideo 0x47c82c declared-memory effects, in the engine's unopened audio
/// domain (player_audio): StopAllSound; FreeSoundLib is a no-op while 0x521fc4
/// is clear; 0x47c844..0x47c9ff (DirectDraw display mode, MCI open/play/close,
/// the message loop that Escape key-up ends, DDSetPalette) is presentation;
/// InitSoundLib -> InitDirectSound clears 0x522224 when no DirectSound object
/// exists and calls DirectSoundCreate, the host's device boundary, which in this
/// domain fails exactly as at the program start that produced it.
fn play_video(m: &mut MemoriaJogo) -> Result<()> {
    stop_sounds(m, 0x47c83a)?;
    if m.u8(0x521fc4)? != 0 {
        return stop(
            "PlayVideo FreeSoundLib on an opened sound library",
            0x48aa0e,
        );
    }
    if m.u32(0x522228)? == 0 {
        m.set_u32(0x522224, 0)?;
    }
    Ok(())
}

/// Main-menu "Introduction": MenuInit(player, 1); the mode-1 frame draws nothing
/// (AffGame 0x417f27); CalculGame mode1 0x42c8bf probes <CD root>\events\introfr.avi
/// (host). `present`: FileClose + PlayVideo (host playback finished or no
/// decoder); absent: DDSetPalette (presentation). Both join MenuInit(local, 2)
/// at 0x42c925, the defeat/campaign-end join.
pub fn introduction(m: &mut MemoriaJogo, player: u8, present: bool) -> Result<()> {
    if m.u8(shell_state::MODE)? != 2 {
        return stop("Introduction request outside the main menu", 0x4682bb);
    }
    if !menu_init(m, player, 1, &mut NoFiles)? {
        return stop("Introduction MenuInit(1) not applied", 0x4682da);
    }
    if present {
        play_video(m)?;
    }
    let local = m.u8(0x508041)?;
    menu_init(m, local, 2, &mut NoFiles)?;
    Ok(())
}

/// MenuNewSelect single-player items0/1 (0x42c474/0x42c4d3): MissionReset,
/// local player0, SetPlayerType human/computer/computer/computer, then the
/// original MenuInit(17) new-game briefing or MenuInit(13) load menu, and the
/// HUD refresh latch. `player` is the frame's AL argument (the local player at
/// selection time); item0 forwards it to MenuInit, item1 forwards the reset player.
pub fn reset_campaign(
    m: &mut MemoriaJogo,
    player: u8,
    next_mode: u8,
    files: &mut impl Files,
) -> Result<()> {
    if !matches!(next_mode, 13 | 17) {
        return stop("MenuNewSelect campaign target", 0x42c3e4);
    }
    m.set_u8(0x496fe4, 1)?;
    m.set_u8(0x508041, 0)?;
    let current = m.u32(0x5080a8)?;
    for (p, kind) in [(0u32, 1u8), (1, 2), (2, 2), (3, 2)] {
        m.set_u8(current + 20 * p, p as u8)?;
        m.set_u8(current + 20 * p + 1, kind)?;
    }
    menu_init(
        m,
        if next_mode == 17 { player } else { 0 },
        next_mode,
        files,
    )?;
    m.set_u8(0x4e05a2, 1)
}

/// Pause menu "continue" (MenuBefore16 or the sixth item): the original in-game
/// MenuInit(18) with its host boundaries (ShowCursor, picture, music) recorded as
/// frame events for the host, the bank release and the DDFlipPage gameplay
/// prefix. Returns the events the host must consume before the next frame.
pub fn resume_game(
    m: &mut MemoriaJogo,
    input: &crate::FrameInput,
) -> Result<Vec<crate::FrameEvent>> {
    if m.u8(shell_state::MODE)? != 16 {
        return stop("MenuInit resume source mode", 0x4682bb);
    }
    menu_init_in_game(m, input)
}

/// Save list14 slot activation, MenuNewSelect 0x42c58b: GameSave0x46bd45 on the
/// slot, then 0x42c570 MenuInit(local player,18) and the HUD refresh latch
/// 0x4e05a2 = 1. GameSave is the host's boundary (the host writes its own modern
/// save before this call); its only declared-memory effect, the slot-name rescan
/// into 0x496fe8, reproduces the scan MenuInit(14) made from the same files.
pub fn save_slot_resume(
    m: &mut MemoriaJogo,
    input: &crate::FrameInput,
) -> Result<Vec<crate::FrameEvent>> {
    if m.u8(shell_state::MODE)? != 14 {
        return stop("MenuNewSelect save-slot source mode", 0x42c58b);
    }
    let events = menu_init_in_game(m, input)?;
    m.set_u8(0x4e05a2, 1)?;
    Ok(events)
}

fn menu_init_in_game(
    m: &mut MemoriaJogo,
    input: &crate::FrameInput,
) -> Result<Vec<crate::FrameEvent>> {
    let mut events = crate::frame::EventLog::new(true);
    crate::frame_input::menu(m, input, &mut events, |_, _, bank| {
        if bank != 0 {
            return stop("MenuInit resume bank handle", 0x4683ae);
        }
        Ok(())
    })?;
    // Outside the declared runtime the in-game port leaves the flip prefix to
    // its caller; the original DDFlipPage runs inside MenuInit.
    if m.player.is_none() {
        crate::load_tail_input::menu_flip_effects_v1(m)?;
    }
    Ok(events.into_vec())
}

/// InputGame Escape in game (0x429305), for tools: the pause order and silence.
pub fn escape_pause(m: &mut MemoriaJogo, player: u32) -> Result<()> {
    crate::player_input::escape(m, player).map(|_| ())
}
