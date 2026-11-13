//! Original in-game events: the FLC "event" a notification opens when the player
//! clicks it (MessageBoxRun 0x469baf -> EvenementInit 0x4580d4), its modal play
//! mode 22 and its two ends (EvenementAffiche 0x4581bc end path and EvenementStop
//! 0x458088).
//!
//! Host contract. The engine opens no file. Before the first frame the host
//! declares, for each `Events\NAME.FLC` of the CD root in use, whether the file
//! opens and its first 128 bytes (FlicOpen 0x45a60c reads exactly the 0x80-byte
//! header into its handle; only fopen/read failure makes it return 0). A name
//! outside the declared table is an explicit stop, never "absent". Frame
//! decoding and palette are host presentation, driven by the host's clock as the
//! original drives them by timeGetTime; the host reports the last frame with
//! [`finish`], after which the next AffGame runs the original end path.
use crate::{frame::stop, MemoriaJogo, Result, World};

/// Owned projection of the host's declared FLC table, outside game memory.
pub const TABLE: u32 = 0x06300000;
/// FlicOpen's GlobalAlloc blocks (handle 0x94, decode struct 8, palette 0x300,
/// pixels width*height). One event at a time (EvenementInit returns while one runs).
pub const HANDLE: u32 = 0x07100000;
pub const DECODE: u32 = 0x07101000;
pub const PALETTE: u32 = 0x07102000;
pub const PIXELS: u32 = 0x07110000;
/// CRT handle stored at handle+0x80: the intact open returns the first free
/// handle of the runtime's table, 0 in the game (no other CRT file is open while
/// playing; native differential). Never read by game logic.
pub const STREAM: u32 = 0;
const ENTRY: u32 = 16 + 4 + 128;
const MAX_ENTRIES: usize = 256;

/// One declared `Events\NAME.FLC`: `header` is the file's first 128 bytes, or
/// `None` when the file does not open (the original FlicOpen returns 0).
pub struct Declared<'a> {
    pub name: &'a [u8],
    pub header: Option<&'a [u8]>,
}

fn invalid(name: &'static str) -> crate::EngineError {
    crate::EngineError::UnportedCallee {
        name,
        address: 0x45a640,
    }
}

pub fn install_table(m: &mut MemoriaJogo, entries: &[Declared<'_>]) -> Result<()> {
    if entries.is_empty() || entries.len() > MAX_ENTRIES {
        return Err(invalid("EvenementInit FLC table size"));
    }
    if m.blocks().any(|(a, _)| a == TABLE) {
        return Err(invalid("EvenementInit FLC table already declared"));
    }
    let mut bytes = (entries.len() as u32).to_le_bytes().to_vec();
    for (i, e) in entries.iter().enumerate() {
        if e.name.is_empty()
            || e.name.len() > 15
            || !e
                .name
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || *c == b'_')
            || entries[..i]
                .iter()
                .any(|o| o.name.eq_ignore_ascii_case(e.name))
        {
            return Err(invalid("EvenementInit FLC table name"));
        }
        let mut name = [0u8; 16];
        name[..e.name.len()].copy_from_slice(&e.name.to_ascii_uppercase());
        bytes.extend_from_slice(&name);
        match e.header {
            Some(h) if h.len() == 128 => {
                bytes.extend_from_slice(&1u32.to_le_bytes());
                bytes.extend_from_slice(h);
            }
            Some(_) => return Err(invalid("EvenementInit FLC header must be 128 bytes")),
            None => {
                bytes.extend_from_slice(&0u32.to_le_bytes());
                bytes.extend_from_slice(&[0; 128]);
            }
        }
    }
    m.declare(TABLE, bytes)
}
impl World {
    /// Declare the CD root's `Events\*.FLC` table before the first frame.
    pub fn load_event_flc(&mut self, entries: &[Declared<'_>]) -> Result<()> {
        let mut memory = self.project()?;
        install_table(&mut memory, entries)?;
        self.replace_memory(memory)
    }
}
fn cstring(m: &MemoriaJogo, mut a: u32, site: u32) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let c = m.u8(a)?;
        if c == 0 {
            return Ok(out);
        }
        out.push(c);
        if out.len() > 4096 {
            return stop("EvenementInit unterminated string", site);
        }
        a = a.wrapping_add(1);
    }
}
/// Table lookup of the composed path's file name: index and declared header.
/// An empty event name composes `\Events\.FLC`, which no CD file can match: the
/// file does not open and FlicOpen returns 0. Any other undeclared name is a
/// host-table error and stays a stop.
fn lookup(m: &MemoriaJogo, path: &[u8]) -> Result<Option<(u32, Option<Vec<u8>>)>> {
    if !m.blocks().any(|(a, _)| a == TABLE) {
        return stop("EvenementInit FLC table not declared by the host", 0x45a640);
    }
    let file = path.rsplit(|&c| c == b'\\').next().unwrap_or(path);
    let stem = file
        .strip_suffix(b".FLC")
        .ok_or(invalid("EvenementInit FLC path"))?;
    for i in 0..m.u32(TABLE)? {
        let e = TABLE + 4 + ENTRY * i;
        let name = m.read(e, 16)?;
        let name = &name[..name.iter().position(|&c| c == 0).unwrap_or(16)];
        if name.eq_ignore_ascii_case(stem) {
            let header = (m.u32(e + 16)? != 0).then(|| m.read(e + 20, 128).map(<[u8]>::to_vec));
            return Ok(Some((i, header.transpose()?)));
        }
    }
    if stem.is_empty() {
        return Ok(None);
    }
    stop(
        "EvenementInit FLC name outside the host's declared table",
        0x45a640,
    )
}
/// Host-visible "event started": index into the declared table.
pub fn started(m: &MemoriaJogo) -> Result<Option<u32>> {
    if m.u8(0x4e1b0e)? == 0 || m.u32(0x4e1b10)? != HANDLE {
        return Ok(None);
    }
    let path = cstring(m, 0x4e1b18, 0x45a640)?;
    Ok(lookup(m, &path)?.map(|(index, _)| index))
}
/// FlicOpen 0x45a60c(path, 0): handle with the 128-byte header, decode struct
/// {pixels, palette}, +0x88 = -1, +0x8c = 0, +0x90 = timeGetTime (host time base,
/// read only by host decoding; recorded as 0). Returns 0 when the file does not open.
fn flic_open(m: &mut MemoriaJogo, path: &[u8]) -> Result<u32> {
    let header = lookup(m, path)?.and_then(|(_, header)| header);
    open_header(m, header.as_deref())
}
/// FlicOpen's allocations for a file whose 128-byte header the host read
/// (`None`: the open failed and FlicOpen returns 0). One FLC at a time: the
/// in-game event (mode 18/22) and the credits (mode 10) never overlap.
pub(crate) fn open_header(m: &mut MemoriaJogo, header: Option<&[u8]>) -> Result<u32> {
    let Some(header) = header else {
        return Ok(0);
    };
    if header.len() != 128 {
        return Err(invalid("FlicOpen header must be 128 bytes"));
    }
    if m.blocks().any(|(a, _)| a == HANDLE) {
        return stop("FlicOpen allocation already in use", 0x45a61b);
    }
    let width = u32::from(u16::from_le_bytes([header[8], header[9]]));
    let height = u32::from(u16::from_le_bytes([header[10], header[11]]));
    let mut handle = header.to_vec();
    handle.extend_from_slice(&STREAM.to_le_bytes());
    handle.extend_from_slice(&DECODE.to_le_bytes());
    handle.extend_from_slice(&u32::MAX.to_le_bytes());
    handle.extend_from_slice(&0u32.to_le_bytes());
    handle.extend_from_slice(&0u32.to_le_bytes());
    m.declare(HANDLE, handle)?;
    let mut decode = PIXELS.to_le_bytes().to_vec();
    decode.extend_from_slice(&PALETTE.to_le_bytes());
    m.declare(DECODE, decode)?;
    m.declare(PALETTE, vec![0; 0x300])?;
    if width * height != 0 {
        m.declare(PIXELS, vec![0; (width * height) as usize])?;
    }
    Ok(HANDLE)
}
/// FlicClose 0x45aaeb: fclose and the three GlobalFree calls.
fn flic_close(m: &mut MemoriaJogo) -> Result<()> {
    if m.u32(0x4e1b10)? != HANDLE {
        return stop("FlicClose handle outside the event allocation", 0x45aaeb);
    }
    close_handle(m)
}
/// FlicClose 0x45aaeb on the single FLC allocation (the CRT close is the host's).
pub(crate) fn close_handle(m: &mut MemoriaJogo) -> Result<()> {
    let decode = m.u32(HANDLE + 0x84)?;
    let pixels = m.u32(decode)?;
    let palette = m.u32(decode + 4)?;
    m.remove(palette)?;
    if m.blocks().any(|(a, _)| a == pixels) {
        m.remove(pixels)?;
    }
    m.remove(decode)?;
    m.remove(HANDLE)?;
    Ok(())
}
/// EvenementInit 0x4580d4(name).
pub fn init(m: &mut MemoriaJogo, name: u32) -> Result<()> {
    if m.u8(0x4e1b0e)? != 0 {
        return Ok(());
    }
    if name == 0 {
        // The intact strcat reads the name at address 0 (0x45813d): an access
        // violation in the original. PlayerSetAllie's caller 0x46380c passes 0.
        return stop(
            "EvenementInit name pointer 0 (original access violation)",
            0x45813d,
        );
    }
    let mut path = cstring(m, 0x509594, 0x4580f2)?;
    path.extend_from_slice(b"\\Events\\");
    path.extend_from_slice(&cstring(m, name, 0x45813d)?);
    path.extend_from_slice(b".FLC");
    let mut terminated = path.clone();
    terminated.push(0);
    m.write(0x4e1b18, &terminated)?;
    let handle = flic_open(m, &path)?;
    m.set_u32(0x4e1b10, handle)?;
    if handle == 0 {
        return Ok(());
    }
    crate::campaign_flow::cd_stop(m)?;
    m.set_u16(0x4e1b0c, 0)?;
    m.set_u8(0x4e1b0e, 1)?;
    m.set_u8(0x4e05a2, 1)?;
    m.set_u8(0x4e1b14, 1)
}
/// MessageBoxRun 0x469baf: only a kind-10 head record opens its EVENEMENT name.
pub fn message_box_run(m: &mut MemoriaJogo) -> Result<()> {
    let record = 0x4e7754u32.wrapping_add(518 * u32::from(m.u16(0x507d54)?));
    if m.u8(record)? == 10 {
        init(m, record + 0x106)?;
    }
    Ok(())
}
/// MessageBoxNext 0x469b85: drop the head notification.
fn message_box_next(m: &mut MemoriaJogo) -> Result<()> {
    m.set_u16(0x507d54, ((u32::from(m.u16(0x507d54)?) + 1) % 256) as u16)?;
    m.set_u8(0x507d58, 0)
}
/// PlaySnd_PostSound 0x48aa79(0x41, EDX, EBX, ECX) with the stack volume word
/// 0x100 the three event calls push (0x4580bc, 0x45837f, 0x458395), published to
/// the host as a kind-10 player event at the call `site`.
fn post_sound(m: &mut MemoriaJogo, site: u32, edx: u32, ebx: u32, ecx: u32) -> Result<()> {
    crate::player::sound(m, site, true, [0x41, edx, ebx, ecx], Some(0x100)).map(|_| ())
}
/// The common tail of both ends: FlicClose, event off, HUD latch, next
/// notification, StopAllSound and PostSound(0x41, 0x40100, 0x496cd0), posted at
/// 0x4580cb by EvenementStop and at 0x4583ab by EvenementAffiche's end branch.
fn end(m: &mut MemoriaJogo, site: u32) -> Result<()> {
    flic_close(m)?;
    m.set_u8(0x4e1b0e, 0)?;
    m.set_u8(0x4e05a2, 1)?;
    m.set_u8(0x4e1b14, 0)?;
    message_box_next(m)?;
    crate::campaign_flow::stop_sounds(m)?;
    post_sound(m, site, 0x40100, 0x496cd0, 0)
}
/// EvenementStop 0x458088 (Escape in game, a click or Escape in mode 22).
pub fn stop_event(m: &mut MemoriaJogo) -> Result<()> {
    end(m, 0x4580cb)
}
/// EvenementAffiche 0x4581bc, the engine-side part. `0x4e1b14` set (just
/// opened, or the host reported the last frame): clear the view (presentation),
/// then either the end tail (frame >= count) or StopAllSound and the start
/// PostSound(0x41, 0x40000, 0x496cd0). Otherwise the host is decoding frames.
pub fn affiche(m: &mut MemoriaJogo) -> Result<()> {
    if m.u8(0x4e1b0e)? == 0 {
        return Ok(());
    }
    let last = m.u16(0x4e1b0c)? >= m.u16(m.u32(0x4e1b10)? + 6)?;
    // 0x45822f: without the flag the host steps frames (counter += elapsed/speed,
    // host-owned here); a counter at the frame count joins the 0x458318 path.
    if m.u8(0x4e1b14)? == 0 && !last {
        return Ok(());
    }
    m.set_u8(0x4e1b14, 0)?;
    // Both branches reach the one PostSound call at 0x4583ab (0x45838e jumps
    // there with EDX 0x40100). The per-frame PostSound(0x41, 0x40200, 0x496cd4, 1)
    // at 0x45827f needs the handle 0x496cd0 written by a sound driver; the engine
    // never writes it (the host plays from the events), so it stays -1.
    if last {
        end(m, 0x4583ab)
    } else {
        crate::campaign_flow::stop_sounds(m)?;
        post_sound(m, 0x4583ab, 0x40000, 0x496cd0, 0)
    }
}
/// Host report: the FLC's last frame has been shown. Leaves the state the
/// original player leaves after its last frame (counter = frame count, 0x4e1b14),
/// so the next AffGame (mode 22) runs the original end path.
pub fn finish(m: &mut MemoriaJogo) -> Result<()> {
    if m.u8(0x4e1b0e)? == 0 || m.u8(crate::shell_state::MODE)? != 22 || m.u32(0x4e1b10)? != HANDLE {
        return Err(invalid("event finish requires a running event in mode 22"));
    }
    m.set_u16(0x4e1b0c, m.u16(HANDLE + 6)?)?;
    m.set_u8(0x4e1b14, 1)
}
/// CalculGame mode 22 (0x42d197): a click, or Escape, ends the event and
/// returns to mode 18. The Escape release wait is the host's.
pub fn calcul_mode22(m: &mut MemoriaJogo) -> Result<()> {
    if (m.u8(0x5099ae)? == 0 || m.u8(0x5099af)? == 0) && crate::player::key(m, 0x1b, 0x42d1b2)? >= 0
    {
        return Ok(());
    }
    m.set_u8(crate::shell_state::MODE, 0x12)?;
    stop_event(m)
}
/// Host view of the event: `index` into the declared table, the FLC header's
/// frame count (+6), width (+8), height (+10) and speed (+0x10), and whether
/// the world is in the modal mode 22.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub running: bool,
    pub modal: bool,
    pub index: u32,
    pub frames: u16,
    pub width: u16,
    pub height: u16,
    pub speed: u32,
}
pub fn state(m: &MemoriaJogo) -> Result<State> {
    let Some(index) = started(m)? else {
        return Ok(State::default());
    };
    Ok(State {
        running: true,
        modal: m.u8(crate::shell_state::MODE)? == 0x16,
        index,
        frames: m.u16(HANDLE + 6)?,
        width: m.u16(HANDLE + 8)?,
        height: m.u16(HANDLE + 10)?,
        speed: m.u32(HANDLE + 0x10)?,
    })
}
impl World {
    pub fn event_state(&self) -> Result<State> {
        state(&*self.render_memory()?)
    }
    /// The host has shown the FLC's last frame (see [`finish`]).
    pub fn event_finish(&mut self) -> Result<()> {
        let mut memory = self.project()?;
        finish(&mut memory)?;
        self.replace_memory(memory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(frames: u16, width: u16, height: u16) -> Vec<u8> {
        let mut h = vec![0u8; 128];
        h[4..6].copy_from_slice(&0xaf12u16.to_le_bytes());
        h[6..8].copy_from_slice(&frames.to_le_bytes());
        h[8..10].copy_from_slice(&width.to_le_bytes());
        h[10..12].copy_from_slice(&height.to_le_bytes());
        h
    }
    fn world(available: bool) -> MemoriaJogo {
        let mut record = vec![0u8; 518 * 256];
        record[0] = 10;
        record[0x106..0x10c].copy_from_slice(b"LABO3\0");
        let mut m = MemoriaJogo::from_blocks([
            (0x4e7754, record),
            (0x507d54, vec![0, 0, 1, 0, 0]),
            (0x4e1b0c, vec![0; 0x200]),
            (0x509594, b"D:\0".to_vec()),
            (0x4e05a2, vec![0]),
            (0x4e2f28, vec![0, 0]),
            (crate::shell_state::MODE, vec![0x12]),
        ])
        .unwrap();
        let h = header(40, 320, 200);
        install_table(
            &mut m,
            &[
                Declared {
                    name: b"labo3",
                    header: available.then_some(&h[..]),
                },
                Declared {
                    name: b"OURS",
                    header: None,
                },
            ],
        )
        .unwrap();
        m
    }

    #[test]
    fn click_opens_declared_event_and_unavailable_does_nothing() {
        let mut m = world(true);
        message_box_run(&mut m).unwrap();
        assert_eq!(m.u8(0x4e1b0e).unwrap(), 1);
        assert_eq!(m.u8(0x4e1b14).unwrap(), 1);
        assert_eq!(m.u8(0x4e05a2).unwrap(), 1);
        assert_eq!(m.u32(0x4e1b10).unwrap(), HANDLE);
        assert_eq!(m.read(0x4e1b18, 21).unwrap(), b"D:\\Events\\LABO3.FLC\0\0");
        assert_eq!(m.u16(HANDLE + 6).unwrap(), 40);
        assert_eq!(m.u32(HANDLE + 0x84).unwrap(), DECODE);
        assert_eq!(m.read(PIXELS, 320 * 200).unwrap().len(), 64000);
        let s = state(&m).unwrap();
        assert_eq!(
            (s.running, s.index, s.frames, s.width, s.height),
            (true, 0, 40, 320, 200)
        );
        // A second click while it runs changes nothing.
        let before = m.clone();
        message_box_run(&mut m).unwrap();
        assert_eq!(m, before);

        let mut m = world(false);
        message_box_run(&mut m).unwrap();
        assert_eq!(m.u8(0x4e1b0e).unwrap(), 0);
        assert_eq!(m.u32(0x4e1b10).unwrap(), 0);
        assert!(!m.blocks().any(|(a, _)| a == HANDLE));
    }

    #[test]
    fn undeclared_name_and_double_table_are_refused() {
        let mut m = world(true);
        m.write(0x4e7754 + 0x106, b"MUR\0").unwrap();
        let before = m.clone();
        let e = message_box_run(&mut m).unwrap_err().to_string();
        assert!(e.contains("outside the host's declared table"), "{e}");
        let _ = before;
        assert!(install_table(
            &mut m,
            &[Declared {
                name: b"MUR",
                header: None
            }]
        )
        .is_err());
    }

    /// The event's PostSound calls reach the host as kind-10 player events with
    /// the original call sites and the stack volume 0x100.
    #[test]
    fn event_sounds_are_published_as_kind_10() {
        let sounds = |m: &MemoriaJogo| -> Vec<(u32, Vec<u32>)> {
            m.player
                .as_ref()
                .unwrap()
                .events
                .iter()
                .filter(|e| e.kind == 10)
                .map(|e| (e.site, e.values[..7].to_vec()))
                .collect()
        };
        let runtime = || crate::player::Runtime {
            answers: crate::player::DeclaredInput {
                display_sound_boundary: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut m = world(true);
        m.declare(0x52137c, vec![0; 44 * 50]).unwrap();
        message_box_run(&mut m).unwrap();
        m.player = Some(runtime());
        affiche(&mut m).unwrap();
        assert_eq!(
            sounds(&m),
            vec![(0x4583ab, vec![1, 0x41, 0x40000, 0x496cd0, 0, 1, 0x100])]
        );
        m.set_u8(crate::shell_state::MODE, 0x16).unwrap();
        finish(&mut m).unwrap();
        m.player = Some(runtime());
        affiche(&mut m).unwrap();
        assert_eq!(
            sounds(&m),
            vec![(0x4583ab, vec![1, 0x41, 0x40100, 0x496cd0, 0, 1, 0x100])]
        );
        assert_eq!(m.u8(0x4e1b0e).unwrap(), 0);

        let mut m = world(true);
        m.declare(0x52137c, vec![0; 44 * 50]).unwrap();
        message_box_run(&mut m).unwrap();
        m.player = Some(runtime());
        stop_event(&mut m).unwrap();
        assert_eq!(
            sounds(&m),
            vec![(0x4580cb, vec![1, 0x41, 0x40100, 0x496cd0, 0, 1, 0x100])]
        );
    }

    #[test]
    fn finish_requires_mode22_and_marks_last_frame() {
        let mut m = world(true);
        message_box_run(&mut m).unwrap();
        assert!(finish(&mut m).is_err());
        m.set_u8(crate::shell_state::MODE, 0x16).unwrap();
        m.set_u8(0x4e1b14, 0).unwrap();
        finish(&mut m).unwrap();
        assert_eq!(m.u16(0x4e1b0c).unwrap(), 40);
        assert_eq!(m.u8(0x4e1b14).unwrap(), 1);
    }
}
