//! Test-only reader for schema-1 app saves (`RisingLands-ptbr/Partidas/*.rls`),
//! copied from a pathing probe in the private research repository.
//! The engine refuses schema 1 because it lacks render-receipt provenance; the
//! probe only needs the memory image and the carried FrameInput, never writes
//! a save, and restarts the virtual clock at tick 0 (20 Hz).
use rl_core::{FrameInput, FrameRegisters, MemoriaJogo};
type R<T> = std::result::Result<T, Box<dyn std::error::Error>>;
struct Rd<'a>(&'a [u8], usize);
impl Rd<'_> {
    fn take(&mut self, n: usize) -> R<&[u8]> {
        let s = self.0.get(self.1..self.1 + n).ok_or("truncated")?;
        self.1 += n;
        Ok(s)
    }
    fn u32(&mut self) -> R<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
}
pub fn load(bytes: &[u8]) -> R<(MemoriaJogo, FrameInput)> {
    let mut h = Rd(bytes, 0);
    if h.take(8)? != b"RLSAVE\r\n" {
        return Err("magic".into());
    }
    let (_format, _schema, _codec) = (h.u32()?, h.u32()?, h.u32()?);
    let length = h.u32()? as usize;
    let packed = h.u32()? as usize;
    let sections = h.u32()?;
    h.take(32)?;
    let src = h.take(packed)?;
    let mut raw = Vec::with_capacity(length);
    let mut i = 0;
    while i < src.len() {
        let tag = src[i];
        let n = usize::from(tag & 127) + 1;
        if tag & 128 != 0 {
            raw.resize(raw.len() + n, src[i + 1]);
            i += 2;
        } else {
            raw.extend_from_slice(&src[i + 1..i + 1 + n]);
            i += 1 + n;
        }
    }
    let mut r = Rd(&raw, 0);
    let mut sec = Vec::new();
    for _ in 0..sections {
        let _id = r.u32()?;
        let n = r.u32()? as usize;
        sec.push(r.take(n)?.to_vec());
    }
    let mut s = Rd(&sec[0], 0);
    let mut m = MemoriaJogo::default();
    for _ in 0..s.u32()? {
        let a = s.u32()?;
        let n = s.u32()? as usize;
        m.declare(a, s.take(n)?.to_vec())?;
    }
    let mut s = Rd(&sec[2], 0);
    let input = FrameInput {
        cursor_x: s.u32()? as i32,
        cursor_y: s.u32()? as i32,
        offscreen: s.u32()?,
        registers: FrameRegisters {
            eax: s.u32()?,
            ebx: s.u32()?,
            ecx: s.u32()?,
            edx: s.u32()?,
            esi: s.u32()?,
            edi: s.u32()?,
            ebp: s.u32()?,
            esp: s.u32()?,
        },
        saved: [s.u32()?, s.u32()?, s.u32()?],
        msg: [s.u32()?, s.u32()?, s.u32()?, s.u32()?],
    };
    Ok((m, input))
}
