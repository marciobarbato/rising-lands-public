//! Portable, bounded, deterministic Rust state snapshots. This is not GameSave.
//! Snapshot only between public engine calls, including between phases 2 and 3.
use crate::{player::DeclaredInput, render_assets::sha256, *};
use std::{collections::VecDeque, fs, io::Write, path::Path};

pub const FORMAT_VERSION: u32 = 1;
// Schema 1 has no authoritative render receipt: its boundary consumption is
// unknowable. Refuse it explicitly; never infer "not yet drawn" on load.
pub const STATE_SCHEMA: u32 = 2;
/// Schema 3 = schema 2 plus section 8, the world rule set. Only a world with a
/// non-zero rule set writes it, so original-rules saves stay schema 2 byte for
/// byte, and an engine without rules refuses a modern save.
pub const RULES_SCHEMA: u32 = 3;
pub const MAX_SAVE_BYTES: usize = 160 * 1024 * 1024;
const MAX_RAW: usize = 128 * 1024 * 1024;
const MAX_MEMORY: usize = 64 * 1024 * 1024;
const MAX_RECORDS: usize = 65536;
const MAGIC: &[u8; 8] = b"RLSAVE\r\n";
const HEADER: usize = 64;
const SECTIONS: u32 = 7;
const RULES_SECTIONS: u32 = 8;
/// Schema 3 with section 9, the Shift+click order queue (only when not empty).
const QUEUE_SECTIONS: u32 = 9;

#[derive(Debug)]
pub enum SaveError {
    Invalid(&'static str),
    Version { format: u32, schema: u32 },
    Checksum,
    Limit,
    Busy,
    Engine(EngineError),
    Io(std::io::Error),
}
impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SaveError {}
impl From<EngineError> for SaveError {
    fn from(e: EngineError) -> Self {
        Self::Engine(e)
    }
}
impl From<std::io::Error> for SaveError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
pub type SaveResult<T> = std::result::Result<T, SaveError>;

/// Owned inputs accepted for a later phase. FIFO order is part of the save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInput {
    pub phase: u32,
    pub answers: DeclaredInput,
}
/// The old phase API carries these registers in the caller. Saving them is mandatory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResumeState {
    pub input: FrameInput,
    pub pending: VecDeque<PendingInput>,
}
impl ResumeState {
    /// Executes the oldest accepted input; an engine error leaves the queue/input intact.
    pub fn step_pending(&mut self, world: &mut World) -> crate::Result<bool> {
        let Some(next) = self.pending.front() else {
            return Ok(false);
        };
        self.input = world.input_step(next.phase, self.input, next.answers.clone())?;
        self.pending.pop_front();
        Ok(true)
    }
}

#[derive(Default)]
struct Writer(Vec<u8>);
impl Writer {
    fn u32(&mut self, n: u32) {
        self.0.extend(n.to_le_bytes());
    }
    fn u64(&mut self, n: u64) {
        self.0.extend(n.to_le_bytes());
    }
    fn count(&mut self, n: usize) -> SaveResult<()> {
        if n > MAX_RECORDS {
            return Err(SaveError::Limit);
        }
        self.u32(n as u32);
        Ok(())
    }
    fn memory(&mut self, m: &MemoriaJogo) -> SaveResult<()> {
        if !m.save_boundary() {
            return Err(SaveError::Busy);
        }
        self.count(m.blocks().count())?;
        let mut total = 0usize;
        for (address, bytes) in m.blocks() {
            total = total.checked_add(bytes.len()).ok_or(SaveError::Limit)?;
            if total > MAX_MEMORY {
                return Err(SaveError::Limit);
            }
            self.u32(address);
            self.u32(bytes.len() as u32);
            self.0.extend(bytes);
        }
        Ok(())
    }
    fn section(&mut self, id: u32, body: Writer) -> SaveResult<()> {
        if self.0.len() + body.0.len() + 8 > MAX_RAW {
            return Err(SaveError::Limit);
        }
        self.u32(id);
        self.u32(body.0.len() as u32);
        self.0.extend(body.0);
        Ok(())
    }
}
pub(crate) struct Reader<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) at: usize,
}
impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    pub(crate) fn take(&mut self, n: usize) -> SaveResult<&'a [u8]> {
        let end = self.at.checked_add(n).ok_or(SaveError::Limit)?;
        let out = self
            .bytes
            .get(self.at..end)
            .ok_or(SaveError::Invalid("truncated"))?;
        self.at = end;
        Ok(out)
    }
    pub(crate) fn u32(&mut self) -> SaveResult<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four bytes"),
        ))
    }
    fn u64(&mut self) -> SaveResult<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }
    fn count(&mut self) -> SaveResult<usize> {
        let n = self.u32()? as usize;
        if n > MAX_RECORDS {
            return Err(SaveError::Limit);
        }
        Ok(n)
    }
    pub(crate) fn memory(&mut self) -> SaveResult<MemoriaJogo> {
        let mut m = MemoriaJogo::default();
        let mut total = 0usize;
        let count = self.count()?;
        let mut previous_end = 0u64;
        for _ in 0..count {
            let address = self.u32()?;
            let len = self.u32()? as usize;
            total = total.checked_add(len).ok_or(SaveError::Limit)?;
            if total > MAX_MEMORY {
                return Err(SaveError::Limit);
            }
            if u64::from(address) < previous_end {
                return Err(SaveError::Invalid("memory order"));
            }
            previous_end = u64::from(address) + len as u64;
            m.declare(address, self.take(len)?.to_vec())?;
        }
        Ok(m)
    }
    pub(crate) fn finish(&self) -> SaveResult<()> {
        if self.at != self.bytes.len() {
            return Err(SaveError::Invalid("trailing bytes"));
        }
        Ok(())
    }
}
fn write_input(w: &mut Writer, input: &FrameInput) {
    w.u32(input.cursor_x as u32);
    w.u32(input.cursor_y as u32);
    w.u32(input.offscreen);
    let r = input.registers;
    for v in [r.eax, r.ebx, r.ecx, r.edx, r.esi, r.edi, r.ebp, r.esp] {
        w.u32(v);
    }
    for v in input.saved.into_iter().chain(input.msg) {
        w.u32(v);
    }
}
pub(crate) fn read_input(r: &mut Reader<'_>) -> SaveResult<FrameInput> {
    Ok(FrameInput {
        cursor_x: r.u32()? as i32,
        cursor_y: r.u32()? as i32,
        offscreen: r.u32()?,
        registers: FrameRegisters {
            eax: r.u32()?,
            ebx: r.u32()?,
            ecx: r.u32()?,
            edx: r.u32()?,
            esi: r.u32()?,
            edi: r.u32()?,
            ebp: r.u32()?,
            esp: r.u32()?,
        },
        saved: [r.u32()?, r.u32()?, r.u32()?],
        msg: [r.u32()?, r.u32()?, r.u32()?, r.u32()?],
    })
}
fn write_answers(w: &mut Writer, a: &DeclaredInput) -> SaveResult<()> {
    let size = a
        .messages
        .len()
        .checked_mul(12)
        .and_then(|n| a.keys.len().checked_mul(8).and_then(|k| n.checked_add(k)))
        .and_then(|n| n.checked_add(20))
        .ok_or(SaveError::Limit)?;
    if w.0.len().checked_add(size).is_none_or(|n| n > MAX_MEMORY) {
        return Err(SaveError::Limit);
    }
    if a.keys.len() > 256
        || a.keys.keys().any(|v| *v > 255)
        || a.messages
            .iter()
            .any(|m| !matches!(m[0], 0x100 | 0x101 | 0x200..=0x205))
        || a.cursor
            .is_some_and(|(x, y)| !(0..800).contains(&x) || !(0..600).contains(&y))
    {
        return Err(SaveError::Invalid("declared input"));
    }
    let (x, y) = a.cursor.unwrap_or_default();
    w.u32(
        u32::from(a.cursor.is_some())
            | (u32::from(a.display_text_boundary) << 1)
            | (u32::from(a.display_sound_boundary) << 2),
    );
    w.u32(x as u32);
    w.u32(y as u32);
    w.count(a.messages.len())?;
    for msg in &a.messages {
        for v in msg {
            w.u32(*v);
        }
    }
    w.count(a.keys.len())?;
    for (k, v) in &a.keys {
        w.u32(*k);
        w.u32(*v);
    }
    Ok(())
}
fn read_answers(r: &mut Reader<'_>) -> SaveResult<DeclaredInput> {
    let flags = r.u32()?;
    if flags & !7 != 0 {
        return Err(SaveError::Invalid("input flags"));
    }
    let xy = (r.u32()? as i32, r.u32()? as i32);
    if flags & 1 == 0 && xy != (0, 0) {
        return Err(SaveError::Invalid("absent cursor"));
    }
    let mut a = DeclaredInput {
        cursor: (flags & 1 != 0).then_some(xy),
        display_text_boundary: flags & 2 != 0,
        display_sound_boundary: flags & 4 != 0,
        ..Default::default()
    };
    for _ in 0..r.count()? {
        a.messages.push([r.u32()?, r.u32()?, r.u32()?]);
    }
    let count = r.count()?;
    if count > 256 {
        return Err(SaveError::Limit);
    }
    for _ in 0..count {
        if a.keys.insert(r.u32()?, r.u32()?).is_some() {
            return Err(SaveError::Invalid("duplicate key"));
        }
    }
    write_answers(&mut Writer::default(), &a)?;
    Ok(a)
}

/// PackBits: high bit = repeated byte, low seven bits + 1 = length (1..128).
fn compress(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < raw.len() {
        let run = raw[at..]
            .iter()
            .take(128)
            .take_while(|b| **b == raw[at])
            .count();
        if run >= 3 {
            out.extend([0x80 | (run as u8 - 1), raw[at]]);
            at += run;
        } else {
            let start = at;
            at += run;
            while at < raw.len() && at - start < 128 {
                if at + 2 < raw.len() && raw[at] == raw[at + 1] && raw[at] == raw[at + 2] {
                    break;
                }
                at += 1;
            }
            out.push((at - start - 1) as u8);
            out.extend(&raw[start..at]);
        }
    }
    out
}
fn decompress(packed: &[u8], length: usize) -> SaveResult<Vec<u8>> {
    let mut raw = Vec::with_capacity(length);
    let mut r = Reader::new(packed);
    while r.at < packed.len() {
        let tag = r.take(1)?[0];
        let n = usize::from(tag & 127) + 1;
        if raw.len() + n > length {
            return Err(SaveError::Invalid("compression length"));
        }
        if tag & 128 != 0 {
            let byte = r.take(1)?[0];
            raw.resize(raw.len() + n, byte);
        } else {
            raw.extend(r.take(n)?);
        }
    }
    if raw.len() != length {
        return Err(SaveError::Invalid("compression length"));
    }
    Ok(raw)
}

pub fn encode(world: &World, resume: &ResumeState) -> SaveResult<Vec<u8>> {
    world.validate_render_receipt()?;
    if !world.projection.template.save_boundary() {
        return Err(SaveError::Busy);
    }
    let mut raw = Writer::default();
    let mut memory = Writer::default();
    memory.memory(&world.project()?)?;
    raw.section(1, memory)?;
    let mut meta = Writer::default();
    meta.u64(world.generation);
    meta.u32(world.frame_phase);
    meta.u32(world.execution_mode());
    let (tick, rate, epoch) = world.clock.save_parts();
    meta.u64(tick);
    meta.u32(rate);
    meta.u32(epoch);
    meta.u32(world.player_cursor.0 as u32);
    meta.u32(world.player_cursor.1 as u32);
    raw.section(2, meta)?;
    let mut input = Writer::default();
    write_input(&mut input, &resume.input);
    raw.section(3, input)?;
    let mut queue = Writer::default();
    queue.count(resume.pending.len())?;
    for p in &resume.pending {
        if !matches!(p.phase, 2 | 3) || p.phase == 3 && !p.answers.messages.is_empty() {
            return Err(SaveError::Invalid("queued phase"));
        }
        queue.u32(p.phase);
        write_answers(&mut queue, &p.answers)?;
    }
    raw.section(4, queue)?;
    let mut events = Writer::default();
    events.count(world.frame_events.len())?;
    for e in &world.frame_events {
        for v in [e.kind, e.id, e.site]
            .into_iter()
            .chain(e.args)
            .chain([e.result])
            .chain(e.registers)
            .chain(e.stack)
            .chain([e.stack_mask])
        {
            events.u32(v);
        }
    }
    events.count(world.player_events.len())?;
    for e in &world.player_events {
        for v in [e.kind, e.phase, e.site, e.frame]
            .into_iter()
            .chain(e.values)
        {
            events.u32(v);
        }
    }
    raw.section(5, events)?;
    let mut freed = Writer::default();
    freed.memory(&world.frame_freed)?;
    raw.section(6, freed)?;
    let mut receipt = Writer::default();
    // Tag 0: none; 1: cursor; 2: cursor and the consumed CRT seed.
    receipt.u32(match &world.render_receipt {
        None => 0,
        Some(r) if r.crt.is_none() => 1,
        Some(_) => 2,
    });
    if let Some(r) = &world.render_receipt {
        receipt.u64(r.generation);
        receipt.u32(u32::from(r.hud));
        receipt.u32(u32::from(r.before));
        receipt.u32(u32::from(r.after));
        if let Some((address, before, after)) = r.crt {
            receipt.u32(address);
            receipt.u32(before);
            receipt.u32(after);
        }
    }
    raw.section(7, receipt)?;
    if world.rules != 0 {
        let mut rules = Writer::default();
        rules.u32(world.rules);
        raw.section(8, rules)?;
    }
    let queued = !world.order_queue.is_empty();
    if queued {
        if world.rules & crate::RULE_ORDER_QUEUE == 0 {
            return Err(SaveError::Invalid("order queue without its rule"));
        }
        let mut queue = Writer::default();
        queue.count(world.order_queue.0.len())?;
        for (unit, w) in world.order_queue.iter() {
            queue.u32(u32::from(unit));
            queue.u32(u32::from(w.player));
            queue.u32(w.cells.len() as u32);
            for cell in &w.cells {
                queue.u32(u32::from(*cell));
            }
        }
        raw.section(9, queue)?;
    }
    let packed = compress(&raw.0);
    if packed.len() + HEADER > MAX_SAVE_BYTES {
        return Err(SaveError::Limit);
    }
    let mut header = Writer(MAGIC.to_vec());
    for v in [
        FORMAT_VERSION,
        if world.rules != 0 {
            RULES_SCHEMA
        } else {
            STATE_SCHEMA
        },
        1,
        raw.0.len() as u32,
        packed.len() as u32,
        if queued {
            QUEUE_SECTIONS
        } else if world.rules != 0 {
            RULES_SECTIONS
        } else {
            SECTIONS
        },
    ] {
        header.u32(v);
    }
    let mut checked = header.0.clone();
    checked.extend(&raw.0);
    header.0.extend(sha256(&checked));
    header.0.extend(packed);
    Ok(header.0)
}

/// Builds and validates a replacement. No running world is mutated on failure.
pub fn decode(bytes: &[u8]) -> SaveResult<(World, ResumeState)> {
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(SaveError::Limit);
    }
    let mut h = Reader::new(bytes);
    if h.take(8)? != MAGIC {
        return Err(SaveError::Invalid("magic"));
    }
    let format = h.u32()?;
    let schema = h.u32()?;
    if format != FORMAT_VERSION || !matches!(schema, 1 | STATE_SCHEMA | RULES_SCHEMA) {
        return Err(SaveError::Version { format, schema });
    }
    if h.u32()? != 1 {
        return Err(SaveError::Invalid("codec"));
    }
    let length = h.u32()? as usize;
    let packed = h.u32()? as usize;
    if length > MAX_RAW || packed > MAX_SAVE_BYTES - HEADER {
        return Err(SaveError::Limit);
    }
    // Validate the complete known legacy body before returning typed refusal.
    let declared = h.u32()?;
    let section_count = match schema {
        1 => 6,
        STATE_SCHEMA => SECTIONS,
        // Section 9 (the order queue) is present only when the queue is not empty.
        _ if declared == QUEUE_SECTIONS => QUEUE_SECTIONS,
        _ => RULES_SECTIONS,
    };
    if declared != section_count {
        return Err(SaveError::Invalid("section count"));
    }
    let digest = h.take(32)?;
    let raw = decompress(h.take(packed)?, length)?;
    h.finish()?;
    let mut checked = bytes[..32].to_vec();
    checked.extend(&raw);
    if sha256(&checked) != digest {
        return Err(SaveError::Checksum);
    }
    let mut r = Reader::new(&raw);
    let mut sections = Vec::new();
    for id in 1..=section_count {
        if r.u32()? != id {
            return Err(SaveError::Invalid("section order"));
        }
        let n = r.u32()? as usize;
        sections.push(r.take(n)?);
    }
    r.finish()?;
    let mut m = Reader::new(sections[0]);
    let memory = m.memory()?;
    m.finish()?;
    let mut m = Reader::new(sections[1]);
    let generation = m.u64()?;
    let phase = m.u32()?;
    let mode = m.u32()?;
    if phase > 3 || mode > 2 {
        return Err(SaveError::Invalid("phase/mode"));
    }
    let clock = TickClock::from_save_parts(m.u64()?, m.u32()?, m.u32()?)?;
    let cursor = (m.u32()? as i32, m.u32()? as i32);
    m.finish()?;
    let mut world = World::load(memory, clock)?;
    world.set_execution_mode(mode)?;
    world.generation = generation;
    world.frame_phase = phase;
    world.player_cursor = cursor;
    let mut m = Reader::new(sections[2]);
    let input = read_input(&mut m)?;
    m.finish()?;
    if sections[3].len() > MAX_MEMORY {
        return Err(SaveError::Limit);
    }
    let mut m = Reader::new(sections[3]);
    let mut pending = VecDeque::new();
    for _ in 0..m.count()? {
        let phase = m.u32()?;
        let answers = read_answers(&mut m)?;
        if !matches!(phase, 2 | 3) || phase == 3 && !answers.messages.is_empty() {
            return Err(SaveError::Invalid("queued phase"));
        }
        pending.push_back(PendingInput { phase, answers });
    }
    m.finish()?;
    let mut m = Reader::new(sections[4]);
    for _ in 0..m.count()? {
        let mut e = FrameEvent {
            kind: m.u32()?,
            id: m.u32()?,
            site: m.u32()?,
            ..Default::default()
        };
        for v in &mut e.args {
            *v = m.u32()?;
        }
        e.result = m.u32()?;
        for v in e.registers.iter_mut().chain(&mut e.stack) {
            *v = m.u32()?;
        }
        e.stack_mask = m.u32()?;
        world.frame_events.push(e);
    }
    for _ in 0..m.count()? {
        let mut e = player::PlayerEvent {
            kind: m.u32()?,
            phase: m.u32()?,
            site: m.u32()?,
            frame: m.u32()?,
            ..Default::default()
        };
        for v in &mut e.values {
            *v = m.u32()?;
        }
        world.player_events.push(e);
    }
    m.finish()?;
    let mut m = Reader::new(sections[5]);
    world.frame_freed = m.memory()?;
    m.finish()?;
    if schema >= STATE_SCHEMA {
        let mut m = Reader::new(sections[6]);
        world.render_receipt = match m.u32()? {
            0 => None,
            tag @ (1 | 2) => {
                let generation = m.u64()?;
                let hud = match m.u32()? {
                    0 => false,
                    1 => true,
                    _ => return Err(SaveError::Invalid("render receipt flags")),
                };
                let before = u8::try_from(m.u32()?)
                    .map_err(|_| SaveError::Invalid("render receipt cursor"))?;
                let after = u8::try_from(m.u32()?)
                    .map_err(|_| SaveError::Invalid("render receipt cursor"))?;
                let crt = if tag == 2 {
                    Some((m.u32()?, m.u32()?, m.u32()?))
                } else {
                    None
                };
                Some(crate::render::ConsumptionReceipt {
                    generation,
                    hud,
                    before,
                    after,
                    crt,
                })
            }
            _ => return Err(SaveError::Invalid("render receipt tag")),
        };
        m.finish()?;
        world.validate_render_receipt()?;
    }
    if schema == RULES_SCHEMA {
        let mut m = Reader::new(sections[7]);
        let rules = m.u32()?;
        m.finish()?;
        // Schema 3 exists only for a non-zero rule set; unknown bits are refused.
        if rules == 0 || rules & !RULES_KNOWN != 0 {
            return Err(SaveError::Invalid("rules"));
        }
        world.rules = rules;
    }
    if section_count == QUEUE_SECTIONS {
        if world.rules & crate::RULE_ORDER_QUEUE == 0 {
            return Err(SaveError::Invalid("order queue without its rule"));
        }
        let mut m = Reader::new(sections[8]);
        let mut entries = Vec::new();
        for _ in 0..m.count()? {
            let unit = m.u32()?;
            let player = m.u32()?;
            let n = m.u32()?;
            if unit > u32::from(u16::MAX)
                || player > 255
                || n as usize > crate::order_queue::MAX_WAYPOINTS
            {
                return Err(SaveError::Invalid("order queue entry"));
            }
            let mut cells = Vec::new();
            for _ in 0..n {
                let cell = m.u32()?;
                if cell > u32::from(u16::MAX) {
                    return Err(SaveError::Invalid("order queue cell"));
                }
                cells.push(cell as u16);
            }
            entries.push((
                unit as u16,
                crate::order_queue::Waypoints {
                    player: player as u8,
                    cells,
                },
            ));
        }
        m.finish()?;
        world.order_queue = crate::OrderQueue::from_entries(entries).map_err(SaveError::Invalid)?;
        if world.order_queue.is_empty() {
            return Err(SaveError::Invalid("empty order queue section"));
        }
    }
    // Semantic state and address bindings must be reconstructible without changing bytes.
    if world.project()? != world.projection.template {
        return Err(SaveError::Invalid("projection"));
    }
    // Never publish the temporary legacy World: absent provenance is not an
    // absent receipt. No migration, cursor inference or default is authorized.
    if schema == 1 {
        return Err(SaveError::Version { format, schema });
    }
    Ok((world, ResumeState { input, pending }))
}

/// Atomic replacement in the destination directory; sync before rename. Failed writes
/// keep the previous slot. A directory sync error after rename is reported to the caller.
pub fn write_file(path: &Path, bytes: &[u8]) -> SaveResult<()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if bytes.len() > MAX_SAVE_BYTES {
        return Err(SaveError::Limit);
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path.file_name().ok_or(SaveError::Invalid("file name"))?;
    let mut temporary = name.to_os_string();
    temporary.push(format!(
        ".tmp-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = parent.join(temporary);
    // Only clean up a file this call created; create_new can fail on a collision.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
pub fn save_file(path: &Path, world: &World, resume: &ResumeState) -> SaveResult<()> {
    write_file(path, &encode(world, resume)?)
}
pub fn load_file(path: &Path) -> SaveResult<(World, ResumeState)> {
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_SAVE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    decode(&bytes)
}

/// One engine-owned stage-start slot. The stage loader calls capture after load-tail
/// and after preparing the next frame's carried registers. UI and slot paths are host policy.
#[derive(Default)]
pub struct StageAutosave {
    bytes: Option<Vec<u8>>,
}
impl StageAutosave {
    pub fn capture(&mut self, world: &World, resume: &ResumeState) -> SaveResult<()> {
        let bytes = encode(world, resume)?;
        self.bytes = Some(bytes);
        Ok(())
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
}

#[cfg(test)]
mod tests;
