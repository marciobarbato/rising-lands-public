//! Persistent indexed rendering; World owns accepted permutation-cursor effects.
use crate::render_assets::{Assets, Sprite};
use crate::{EngineError, MemoriaJogo};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Debug)]
pub enum Error {
    Memory(EngineError),
    Invalid(&'static str),
    Stop(u32, String),
}
impl From<EngineError> for Error {
    fn from(e: EngineError) -> Self {
        Self::Memory(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
/// The esp - 52 word AffChrono finds (None: not modeled) and the writes.
pub type HudChronoFixture = (Option<u16>, Vec<(u32, u8)>);
#[derive(Clone)]
pub struct Cache {
    pub keys: Vec<u8>,
    pub raw: Vec<u8>,
    pub cursor: u16,
}
impl Cache {
    pub fn validate(&self) -> Result<()> {
        if self.keys.len() != 1024 || self.raw.len() != 270336 || self.cursor > 255 {
            Err(Error::Invalid("terrain cache dimensions/cursor"))
        } else {
            Ok(())
        }
    }
}
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub kind: u32,
    pub address: u32,
    pub name: [u8; 48],
    pub count: u32,
    pub args: [i32; 8],
    pub before: [u8; 8],
    pub after: [u8; 8],
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedBackground {
    pub address: u32,
    pub pixels: Vec<u8>,
}
/// Fixture-only attestation of the original probe's non-executed AffText boundary.
/// The C ABI never installs these; production rendering draws original bitmap glyphs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedTextBoundary {
    pub text: Vec<u8>,
    pub rectangle: [i32; 4],
    pub font: u32,
    pub flags: [u32; 2],
    pub surface_sha256: [u8; 32],
}
#[derive(Clone, Debug)]
pub struct Outcome {
    /// Exact render input, distinct from the published world cursor.
    pub consumption_input: Option<(bool, u8)>,
    pub text_boundaries_checked: u32,
    pub disposition: u32,
    pub stop: Option<(u32, String)>,
    pub observations: Vec<Observation>,
    /// Original byte-permutation cursor, including an addressed draw prefix.
    /// The caller publishes this to the world once when accepting the draw.
    pub permutation_cursor: Option<u8>,
    /// CRT `rand` seed consumed by the draw (LongRand 0x45841b -> 0x48c51d,
    /// the logic's own generator): (seed address, render input, final seed).
    /// Published with the cursor, including an addressed draw prefix.
    pub crt_seed: Option<(u32, u32, u32)>,
}

impl crate::World {
    /// Reconstruct the accepted boundary's render input without changing the
    /// authoritative world. A new renderer must replay it, not consume it again.
    pub fn render_boundary_memory(
        &self,
        hud: bool,
    ) -> crate::Result<std::borrow::Cow<'_, MemoriaJogo>> {
        let mut memory = self.render_memory()?;
        if let Some(receipt) = &self.render_receipt {
            self.validate_render_receipt()?;
            if receipt.hud != hud {
                return Err(EngineError::ExecutionProjectionMismatch);
            }
            if receipt.before != receipt.after {
                memory.to_mut().set_u8(0x4e1d18, receipt.before)?;
            }
            if let Some((address, before, after)) = receipt.crt {
                if before != after {
                    memory.to_mut().set_u32(address, before)?;
                }
            }
        }
        Ok(memory)
    }
    pub(crate) fn validate_render_receipt(&self) -> crate::Result<()> {
        if let Some(receipt) = &self.render_receipt {
            let memory = self.render_memory()?;
            if receipt.generation != self.generation
                || memory.u8(0x4e1d18)? != receipt.after
                || receipt.crt.is_some_and(|(address, _, after)| {
                    memory.optional_u32(address).ok() != Some(Some(after))
                })
            {
                return Err(EngineError::ExecutionProjectionMismatch);
            }
        }
        Ok(())
    }
    /// Publish the renderer's declared gameplay effect before another draw or
    /// logic call. This is constant work, independent of audit observations.
    /// Render publication does not tick the clock or change logic generation.
    pub fn publish_render_effects(&mut self, outcome: &Outcome) -> crate::Result<()> {
        if outcome.permutation_cursor.is_none()
            && outcome.crt_seed.is_none()
            && self.render_receipt.is_none()
        {
            return Ok(());
        }
        let Some((hud, before)) = outcome.consumption_input else {
            return Err(EngineError::ExecutionProjectionMismatch);
        };
        let after = outcome.permutation_cursor.unwrap_or(before);
        let receipt = ConsumptionReceipt {
            generation: self.generation,
            hud,
            before,
            after,
            crt: outcome.crt_seed,
        };
        if let Some(previous) = &self.render_receipt {
            self.validate_render_receipt()?;
            if previous != &receipt {
                return Err(EngineError::ExecutionProjectionMismatch);
            }
            return Ok(());
        }
        if self.render_memory()?.u8(0x4e1d18)? != before {
            return Err(EngineError::ExecutionProjectionMismatch);
        }
        if let Some((address, seed, after)) = outcome.crt_seed {
            // The render consumed the logic's CRT generator: the seed it read
            // must be the world's, at the bound thread-data address.
            if self.projection.crt_seed_address != Some(address)
                || self.state.crt_rng.is_none()
                || self.render_memory()?.u32(address)? != seed
            {
                return Err(EngineError::ExecutionProjectionMismatch);
            }
            self.projection.template.set_u32(address, after)?;
            self.state.crt_rng = Some(crate::CrtRng { seed: after });
        }
        if let Some(cursor) = outcome.permutation_cursor {
            let frame = self
                .state
                .frame
                .as_mut()
                .ok_or(crate::EngineError::IncompleteField { address: 0x4e1d18 })?;
            self.projection.template.u8(0x4e1d18)?;
            self.projection.template.set_u8(0x4e1d18, cursor)?;
            frame.globals.permutation_cursor = cursor;
        }
        self.render_receipt = Some(receipt);
        Ok(())
    }
}
/// World-owned, generation-scoped record of accepted rendering. This is saved
/// with the world, never inferred from Renderer.last or a wrapping frame count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConsumptionReceipt {
    pub generation: u64,
    pub hud: bool,
    pub before: u8,
    pub after: u8,
    /// CRT seed address, seed before and after the accepted draw.
    pub crt: Option<(u32, u32, u32)>,
}
/// The CRT seed a draw consumed: (address, render input, final), when the
/// draw wrote any byte of the bound seed. No write, no effect.
pub(crate) fn crt_effect(
    m: &MemoriaJogo,
    overrides: &BTreeMap<u32, u8>,
) -> Result<Option<(u32, u32, u32)>> {
    if overrides.is_empty() || m.optional_u32(0x4990cc)? != Some(0x48e7c7) {
        return Ok(None);
    }
    let Some(thread) = m.optional_u32(0x522244)? else {
        return Ok(None);
    };
    let address = thread.wrapping_add(12);
    if address == 0 || !(0..4).any(|i| overrides.contains_key(&(address + i))) {
        return Ok(None);
    }
    let before = m.u32(address)?;
    let mut after = before.to_le_bytes();
    for (i, b) in after.iter_mut().enumerate() {
        if let Some(&v) = overrides.get(&(address + i as u32)) {
            *b = v;
        }
    }
    Ok(Some((address, before, u32::from_le_bytes(after))))
}
#[derive(Clone)]
pub struct Renderer {
    pub audit: bool,
    pub saved_background: Option<SavedBackground>,
    // Display-owned tooltip state travels with its saved pixels, not the logic world.
    display_state: BTreeMap<u32, u8>,
    pub recorded_text_boundaries: Option<std::collections::VecDeque<RecordedTextBoundary>>,
    pub tick: Option<u32>,
    pub assets: Arc<Assets>,
    pub surface: Vec<u8>,
    pub cache: Cache,
    pub cache_generation: u64,
    pub last: Option<(u64, u32, Outcome)>,
}
impl Renderer {
    pub fn new(assets: Arc<Assets>, surface: &[u8], cache: Cache) -> Result<Self> {
        cache.validate()?;
        if surface.len() != 480000 {
            return Err(Error::Invalid("initial surface must have 480000 indices"));
        }
        Ok(Self {
            audit: true,
            saved_background: None,
            display_state: BTreeMap::new(),
            recorded_text_boundaries: None,
            tick: None,
            assets,
            surface: surface.to_vec(),
            cache,
            cache_generation: 0,
            last: None,
        })
    }
    /// Reuse the atomic render transaction's buffers. Previous pixels/cache are
    /// copied because skipped drawing and unexpected errors must preserve them.
    pub fn prepare_from(&mut self, previous: &Self) {
        self.audit = previous.audit;
        self.display_state.clone_from(&previous.display_state);
        self.surface.clone_from(&previous.surface);
        self.cache.keys.clone_from(&previous.cache.keys);
        self.cache.raw.clone_from(&previous.cache.raw);
        self.cache.cursor = previous.cache.cursor;
        self.assets = Arc::clone(&previous.assets);
        self.tick = previous.tick;
        self.cache_generation = previous.cache_generation;
        self.last.clone_from(&previous.last);
        self.recorded_text_boundaries
            .clone_from(&previous.recorded_text_boundaries);
        match (&mut self.saved_background, &previous.saved_background) {
            (Some(next), Some(old)) => {
                next.address = old.address;
                next.pixels.clone_from(&old.pixels);
            }
            _ => self.saved_background.clone_from(&previous.saved_background),
        }
    }
    pub fn reset(&mut self, surface: &[u8], cache: Cache) -> Result<()> {
        cache.validate()?;
        if surface.len() != 480000 {
            return Err(Error::Invalid("surface size"));
        }
        self.surface.copy_from_slice(surface);
        self.cache = cache;
        self.cache_generation = self.cache_generation.wrapping_add(1);
        self.last = None;
        self.saved_background = None;
        self.display_state.clear();
        self.recorded_text_boundaries = None;
        Ok(())
    }
    #[doc(hidden)]
    pub fn render_building_fixture(
        &mut self,
        m: &MemoriaJogo,
        x: i32,
        y: i32,
        cell: u32,
    ) -> Result<Outcome> {
        let assets = Arc::clone(&self.assets);
        let mut c = Context {
            background: &mut self.saved_background,
            text_boundaries: &mut self.recorded_text_boundaries,
            text_checked: 0,
            tick: self.tick,
            m,
            overrides: BTreeMap::new(),
            surface: &mut self.surface,
            cache: &mut self.cache,
            assets: &assets,
            observations: Vec::new(),
            audit: self.audit,
            clip: [
                m.u16(0x49a020)? as i32,
                m.u16(0x49a01c)? as i32,
                m.u16(0x49a022)? as i32,
                m.u16(0x49a01e)? as i32,
            ],
            camera: m.u8(0x508041)? as u32,
        };
        let tile = m.u16(m.u32(0x507d70)? + cell * 2)? as u32;
        let code = m.u8(m.u32(0x507d74)? + cell)?;
        let discovery = m.u8(m.u32(0x507d78 + 4 * c.camera)? + cell)?;
        let result = c.building(x, y, cell, tile, code, discovery);
        let stop = match result {
            Ok(()) => None,
            Err(Error::Stop(a, s)) => Some((a, s)),
            Err(e) => return Err(e),
        };
        Ok(Outcome {
            consumption_input: Some((false, m.u8(0x4e1d18)?)),
            disposition: if stop.is_some() { 2 } else { 0 },
            permutation_cursor: c.overrides.get(&0x4e1d18).copied(),
            crt_seed: crt_effect(m, &c.overrides)?,
            stop,
            observations: c.observations,
            text_boundaries_checked: 0,
        })
    }
    fn fixture_context<'a>(&'a mut self, m: &'a MemoriaJogo) -> Result<Context<'a>> {
        Ok(Context {
            background: &mut self.saved_background,
            text_boundaries: &mut self.recorded_text_boundaries,
            text_checked: 0,
            tick: self.tick,
            m,
            overrides: BTreeMap::new(),
            surface: &mut self.surface,
            cache: &mut self.cache,
            assets: &self.assets,
            observations: Vec::new(),
            audit: self.audit,
            clip: [
                m.u16(0x49a020)? as i32,
                m.u16(0x49a01c)? as i32,
                m.u16(0x49a022)? as i32,
                m.u16(0x49a01e)? as i32,
            ],
            camera: 0,
        })
    }
    /// Isolated display effects API; scratch registers are outside this contract.
    #[doc(hidden)]
    pub fn message_box_fixture(&mut self, m: &MemoriaJogo) -> Result<Vec<(u32, u8)>> {
        let mut c = self.fixture_context(m)?;
        c.message_box()?;
        Ok(c.overrides.into_iter().collect())
    }
    /// Isolated display effects API; scratch registers are outside this contract.
    #[doc(hidden)]
    pub fn object_fixture(
        &mut self,
        m: &MemoriaJogo,
        r: crate::FrameRegisters,
        sprites: bool,
    ) -> Result<()> {
        let mut c = self.fixture_context(m)?;
        let camera = u32::from(m.u8(0x508041)?);
        let cell = r.ebx & 65535;
        c.object(
            r.eax as i16 as i32,
            r.edx as i16 as i32,
            m.u8(m.u32(0x507d74)? + cell)?,
            m.u8(m.u32(0x507dac)? + cell)?,
            m.u8(m.u32(0x507d78 + 4 * camera)? + cell)?,
            m.u8(m.u32(0x507d88 + 4 * camera)? + cell)?,
            sprites,
        )
    }
    /// Isolated display effects API; scratch registers are outside this contract.
    #[doc(hidden)]
    pub fn projectile_fixture(&mut self, m: &MemoriaJogo, index: u32) -> Result<()> {
        let mut c = self.fixture_context(m)?;
        let camera = u32::from(m.u8(0x508041)?);
        let record = m.read(m.u32(0x5080f0)? + 56 * index, 56)?;
        let cell = ((w(record, 14) / 32) * 256 + w(record, 12) / 32) & 65535;
        let light = m.u8(m.u32(0x507d88 + camera * 4)? + cell)?;
        c.projectile(
            record,
            u32::from(m.u16(0x508720)?),
            u32::from(m.u16(0x508722)?),
            light,
        )
    }
    /// Isolated display effects API; scratch registers are outside this contract.
    #[doc(hidden)]
    pub fn overlay_fixture(
        &mut self,
        m: &MemoriaJogo,
        r: crate::FrameRegisters,
        structure: bool,
    ) -> Result<()> {
        let mut c = self.fixture_context(m)?;
        if structure {
            let record = m.read(0x49a0c8 + 276 * r.ebx, 276)?;
            let tile = m.u16(m.u32(0x507d70)? + 2 * r.eax)?;
            c.structure_overlay(
                record,
                r.eax,
                u32::from(tile),
                r.ecx & 1 != 0,
                r.ecx & 2 != 0,
            )
        } else {
            let record = m.read(m.u32(0x4e19a0)? + 276 * r.ebx, 276)?;
            c.unit(
                r.eax as u16 as i16 as i32,
                r.edx as u16 as i16 as i32,
                r.ebx,
                record,
            )
        }
    }
    /// Exercise the production unit dispatcher and preserve its display prefix.
    #[doc(hidden)]
    pub fn unit_wiring_fixture(
        &mut self,
        m: &MemoriaJogo,
        r: crate::FrameRegisters,
    ) -> Result<Outcome> {
        let mut c = self.fixture_context(m)?;
        c.camera = u32::from(m.u8(0x508041)?);
        let index = r.ebx & 65535;
        let record = m.read(m.u32(0x4e19a0)? + 276 * index, 276)?;
        let result = c.unit(i16(r.eax as i32), i16(r.edx as i32), index, record);
        let stop = match result {
            Ok(()) => None,
            Err(Error::Stop(a, reason)) => Some((a, reason)),
            Err(e) => return Err(e),
        };
        Ok(Outcome {
            consumption_input: Some((false, m.u8(0x4e1d18)?)),
            disposition: if stop.is_some() { 2 } else { 0 },
            permutation_cursor: c.overrides.get(&0x4e1d18).copied(),
            crt_seed: crt_effect(m, &c.overrides)?,
            stop,
            observations: c.observations,
            text_boundaries_checked: 0,
        })
    }
    /// AffZoneIconeAction 0x416f29 alone (the four action icon slots).
    #[doc(hidden)]
    pub fn action_icons_fixture(&mut self, m: &MemoriaJogo) -> Result<Outcome> {
        let mut c = self.fixture_context(m)?;
        c.camera = u32::from(m.u8(0x508041)?);
        let result = c.action_icons();
        let stop = match result {
            Ok(()) => None,
            Err(Error::Stop(a, reason)) => Some((a, reason)),
            Err(e) => return Err(e),
        };
        Ok(Outcome {
            consumption_input: Some((false, m.u8(0x4e1d18)?)),
            disposition: if stop.is_some() { 2 } else { 0 },
            permutation_cursor: c.overrides.get(&0x4e1d18).copied(),
            crt_seed: crt_effect(m, &c.overrides)?,
            stop,
            observations: c.observations,
            text_boundaries_checked: 0,
        })
    }
    /// AffMarche 0x46677d: the Outcome and the writes it would leave.
    #[doc(hidden)]
    pub fn market_fixture(&mut self, m: &MemoriaJogo, p: u32) -> Result<(Outcome, Vec<(u32, u8)>)> {
        let mut c = self.fixture_context(m)?;
        c.camera = u32::from(m.u8(0x508041)?);
        let result = c.market(p);
        let stop = match result {
            Ok(()) => None,
            Err(Error::Stop(a, reason)) => Some((a, reason)),
            Err(e) => return Err(e),
        };
        let outcome = Outcome {
            consumption_input: Some((false, m.u8(0x4e1d18)?)),
            disposition: if stop.is_some() { 2 } else { 0 },
            permutation_cursor: c.overrides.get(&0x4e1d18).copied(),
            crt_seed: crt_effect(m, &c.overrides)?,
            stop,
            observations: c.observations,
            text_boundaries_checked: 0,
        };
        Ok((outcome, c.overrides.into_iter().collect()))
    }
    /// Isolated AffChrono effects transport for the live local player.
    #[doc(hidden)]
    pub fn chrono_fixture(&mut self, m: &MemoriaJogo) -> Result<()> {
        let mut c = self.fixture_context(m)?;
        c.camera = u32::from(m.u8(0x508041)?);
        c.chrono(None)
    }
    /// AffGame 0x417b32..0x417c5b (counters through AffChrono) for the live
    /// local player: the esp - 52 word AffChrono finds (None: not modeled)
    /// and the writes, the draw's own plus the AffMarche writes the port
    /// applies through `market::display_tick`.
    #[doc(hidden)]
    pub fn hud_chrono_fixture(&mut self, m: &MemoriaJogo) -> Result<HudChronoFixture> {
        let mut c = self.fixture_context(m)?;
        c.camera = u32::from(m.u8(0x508041)?);
        let slot = c.hud_to_chrono()?;
        let mut writes = c.overrides;
        let mut after = m.clone();
        crate::market::display_tick(&mut after)
            .map_err(|_| Error::Invalid("market display tick"))?;
        for ((base, old), (_, new)) in m.blocks().zip(after.blocks()) {
            for (i, (x, y)) in old.iter().zip(new).enumerate() {
                if x != y {
                    writes.insert(base + i as u32, *y);
                }
            }
        }
        Ok((slot, writes.into_iter().collect()))
    }
    /// AffTrame placement entry0x41721f, including viewport/page exits.
    #[doc(hidden)]
    pub fn placement_fixture(
        &mut self,
        m: &MemoriaJogo,
        mut r: crate::FrameRegisters,
    ) -> Result<crate::FrameRegisters> {
        let mut c = self.fixture_context(m)?;
        let width = u32::from(m.u16(0x508724)?) * 32;
        let height = u32::from(m.u16(0x508726)?) * 32;
        r.eax = if u32::from(m.u16(0x5099a8)?) > width {
            width
        } else if u32::from(m.u16(0x5099aa)?) > height {
            height
        } else if m.u16(0x4e05ae)? != 3 {
            u32::from(m.u16(0x4e05ae)?)
        } else {
            c.placement()?;
            u32::from(m.u16(0x495d8e + 52 * u32::from(m.u8(0x496207)?))?)
        };
        r.esp = r.esp.wrapping_add(4);
        Ok(r)
    }
    /// Native AffInfoAide entry0x40f8aa (the old stop was its interior0x40f8c0).
    #[doc(hidden)]
    pub fn tooltip_fixture(
        &mut self,
        m: &MemoriaJogo,
        mut r: crate::FrameRegisters,
    ) -> Result<(crate::FrameRegisters, Vec<(u32, u8)>)> {
        let mut c = self.fixture_context(m)?;
        let length = c.tooltip()?;
        if let Some((length, terminator)) = length {
            r.eax = terminator;
            r.edx = r.esp.wrapping_sub(0x44).wrapping_add(length as u32);
        } else if m.u32(0x4e060c)? != 0 {
            r.eax = (r.eax & 0xffff0000) | m.u16(0x4e0610)? as u32;
        }
        r.esp = r.esp.wrapping_add(4);
        Ok((r, c.overrides.into_iter().collect()))
    }
    /// Isolated original-address routine entry for differential validation.
    #[doc(hidden)]
    pub fn afftext_fixture(
        &mut self,
        m: &MemoriaJogo,
        ptr: u32,
        font: u32,
        flags: [u32; 2],
        mut r: crate::FrameRegisters,
    ) -> Result<crate::FrameRegisters> {
        let mut text = Vec::new();
        for i in 0..129 {
            let c = m.u8(ptr + i)?;
            text.push(c);
            if c == 0 || c == 10 {
                break;
            }
        }
        let local = r.esp.wrapping_sub(0x54);
        let rectangle = [r.eax as i32, r.edx as i32, r.ebx as i32, r.ecx as i32];
        let mut c = self.fixture_context(m)?;
        c.afftext(&text, rectangle, font, flags)?;
        let line = text.split(|&v| v == 0 || v == 10).next().unwrap();
        let base = [0x23d5, 0x2595, 0x2755][((font - 1) / 2) as usize];
        r.eax = text[line.len()] as u32;
        r.edx = ptr + line.len() as u32;
        if let Some(&last) = line.last() {
            r.ebx = self
                .assets
                .sprite(
                    base + if last == b' ' {
                        b'l' as u32
                    } else {
                        last as u32
                    } - 32,
                )?
                .width as u32;
            r.ecx = ((local + 0x1c) & !255) | line.len() as u32;
        } else {
            r.ebx = if rectangle[0] as i16 == rectangle[2] as i16 {
                local + 0x24
            } else {
                rectangle[1] as i16 as i32 as u32
            };
            r.ecx = self.assets.sprite(base + b'j' as u32 - 32)?.height as u32;
        }
        r.esp = r.esp.wrapping_add(20);
        Ok(r)
    }
    pub fn render(&mut self, m: &MemoriaJogo, hud: bool) -> Result<Outcome> {
        let assets = Arc::clone(&self.assets);
        let mut c = Context {
            background: &mut self.saved_background,
            text_boundaries: &mut self.recorded_text_boundaries,
            text_checked: 0,
            tick: self.tick,
            m,
            overrides: self.display_state.clone(),
            surface: &mut self.surface,
            cache: &mut self.cache,
            assets: &assets,
            observations: Vec::new(),
            audit: self.audit,
            clip: [0, 0, 799, 599],
            camera: 0,
        };
        let result = c.compose(hud);
        if result.as_ref().is_ok_and(|d| *d == 0)
            && c.text_boundaries.as_ref().is_some_and(|b| !b.is_empty())
        {
            return Err(Error::Invalid("Unconsumed AffText probe boundary"));
        }
        let (disposition, stop) = match result {
            Ok(d) => (d, None),
            Err(Error::Stop(a, s)) => (2, Some((a, s))),
            Err(e) => return Err(e),
        };
        for (&address, &value) in &c.overrides {
            if address == 0x49a041 || (0x49a046..0x49a04e).contains(&address) {
                self.display_state.insert(address, value);
            }
        }
        if disposition != 1 {
            self.cache_generation = self.cache_generation.wrapping_add(1);
        }
        Ok(Outcome {
            consumption_input: Some((hud, m.u8(0x4e1d18)?)),
            text_boundaries_checked: c.text_checked,
            permutation_cursor: c.overrides.get(&0x4e1d18).copied(),
            crt_seed: crt_effect(m, &c.overrides)?,
            disposition,
            stop,
            observations: c.observations,
        })
    }
}
pub(crate) struct Context<'a> {
    pub background: &'a mut Option<SavedBackground>,
    pub text_boundaries: &'a mut Option<std::collections::VecDeque<RecordedTextBoundary>>,
    pub text_checked: u32,
    pub tick: Option<u32>,
    pub m: &'a MemoriaJogo,
    pub overrides: BTreeMap<u32, u8>,
    pub surface: &'a mut Vec<u8>,
    pub cache: &'a mut Cache,
    pub assets: &'a Assets,
    pub observations: Vec<Observation>,
    pub audit: bool,
    pub clip: [i32; 4],
    pub camera: u32,
}
pub(crate) fn w(r: &[u8], o: usize) -> u32 {
    u16::from_le_bytes(r[o..o + 2].try_into().unwrap()) as u32
}
pub(crate) fn dw(r: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(r[o..o + 4].try_into().unwrap())
}
pub(crate) fn i16(v: i32) -> i32 {
    v as i16 as i32
}
impl Context<'_> {
    pub fn u8(&self, a: u32) -> Result<u32> {
        Ok(if let Some(v) = self.overrides.get(&a) {
            *v
        } else if let Some(background) = self
            .background
            .as_ref()
            .filter(|b| a >= b.address && (a - b.address) < b.pixels.len() as u32)
        {
            background.pixels[(a - background.address) as usize]
        } else {
            self.m.u8(a)?
        } as u32)
    }
    /// True when no override or saved-background byte lies in `[a, a + n)`, so a
    /// multi-byte read can take one memory lookup instead of `n` calls to `u8`.
    /// Any failure of that lookup falls back to the bytewise path (same result/error).
    #[inline]
    fn plain(&self, a: u32, n: usize) -> bool {
        let start = u64::from(a);
        let end = start + n as u64;
        end <= 1u64 << 32
            && self
                .overrides
                .range(a..)
                .next()
                .is_none_or(|(&k, _)| u64::from(k) >= end)
            && self.background.as_ref().is_none_or(|b| {
                end <= u64::from(b.address) || start >= u64::from(b.address) + b.pixels.len() as u64
            })
    }
    pub fn u16(&self, a: u32) -> Result<u32> {
        if self.plain(a, 2) {
            if let Ok(v) = self.m.u16(a) {
                return Ok(v as u32);
            }
        }
        Ok(self.u8(a)? | self.u8(a + 1)? << 8)
    }
    pub fn u32(&self, a: u32) -> Result<u32> {
        if self.plain(a, 4) {
            if let Ok(v) = self.m.u32(a) {
                return Ok(v);
            }
        }
        Ok(self.u16(a)? | self.u16(a + 2)? << 16)
    }
    pub fn read(&self, a: u32, n: usize) -> Result<Vec<u8>> {
        if n > 0 && self.plain(a, n) {
            if let Ok(v) = self.m.read(a, n) {
                return Ok(v.to_vec());
            }
        }
        (0..n)
            .map(|i| self.u8(a + i as u32).map(|v| v as u8))
            .collect()
    }
    pub fn stop<T>(&self, a: u32, s: impl Into<String>) -> Result<T> {
        Err(Error::Stop(a, s.into()))
    }
    /// LongRand 0x45841b -> CRT rand 0x48c51d on the single-thread data the
    /// logic also uses (frame_core::crt_random): the seed store is a render
    /// effect published to the world with the draw (Outcome::crt_seed).
    pub(crate) fn crt_rand(&mut self) -> Result<u32> {
        if self.u32(0x4990cc)? != 0x48e7c7 {
            return self.stop(0x48c513, "rand thread-data getter");
        }
        let address = self.u32(0x522244)?.wrapping_add(12);
        if address == 0 {
            return Ok(0);
        }
        let mut rng = crate::CrtRng {
            seed: self.u32(address)?,
        };
        let value = rng.draw();
        self.put32("crt_seed", address, rng.seed)?;
        Ok(value)
    }
    pub fn observe(&mut self, name: &str, a: u32, args: &[i32]) {
        if !self.audit {
            return;
        }
        let mut n = [0; 48];
        let len = name.len().min(47);
        n[..len].copy_from_slice(&name.as_bytes()[..len]);
        let mut values = [0; 8];
        values[..args.len()].copy_from_slice(args);
        self.observations.push(Observation {
            kind: 2,
            address: a,
            name: n,
            count: args.len() as u32,
            args: values,
            before: [0; 8],
            after: [0; 8],
        });
    }
    pub fn put(&mut self, name: &str, a: u32, b: &[u8]) -> Result<()> {
        let old = self.read(a, b.len())?;
        if old != b && self.audit {
            self.observe(name, a, &[]);
            let e = self.observations.last_mut().unwrap();
            e.kind = 1;
            e.count = b.len() as u32;
            e.before[..b.len()].copy_from_slice(&old);
            e.after[..b.len()].copy_from_slice(b);
        }
        for (i, v) in b.iter().enumerate() {
            self.overrides.insert(a + i as u32, *v);
        }
        Ok(())
    }
    pub fn put16(&mut self, n: &str, a: u32, v: u32) -> Result<()> {
        self.put(n, a, &(v as u16).to_le_bytes())
    }
    pub fn put32(&mut self, n: &str, a: u32, v: u32) -> Result<()> {
        self.put(n, a, &v.to_le_bytes())
    }
    pub fn screen_clip(&self) -> [i32; 4] {
        [0, 0, 799, 599]
    }
    pub fn raster(
        &mut self,
        id: u32,
        x: i32,
        y: i32,
        op: u32,
        owner: u32,
        clip: [i32; 4],
    ) -> Result<()> {
        if op == 3 && owner > 4 {
            return Err(Error::Invalid("player table owner outside 0..4"));
        }
        let q = self.assets.sprite(id)?;
        // The collapse clip set by AffBatiments (SetClipping with the view width
        // 800 as its right edge) lets the native pasters write column 800, the
        // first pixel of the next row of the linear 800-pitch surface. On the
        // last row that byte lies past the surface: keep that an explicit stop.
        let [_, _, r, b] = clip;
        let (sx, sy) = (800 - x, 599 - y);
        if r >= 800
            && b >= 599
            && (0..q.width as i32).contains(&sx)
            && (0..q.height as i32).contains(&sy)
            && q.mask[sy as usize * q.width + sx as usize] != 0
        {
            return self.stop(0x45b247, "sprite column 800 on the last surface row");
        }
        self.raster_sprite(q, x, y, op, owner, clip);
        Ok(())
    }
    fn raster_sprite(&mut self, q: &Sprite, x: i32, y: i32, op: u32, owner: u32, clip: [i32; 4]) {
        let [l, t, r, b] = clip;
        let l = l.max(0);
        let t = t.max(0);
        let r = r.min(800);
        let b = b.min(599);
        for sy in 0.max(t - y)..(q.height as i32).min(b - y + 1) {
            for sx in 0.max(l - x)..(q.width as i32).min(r - x + 1) {
                let src = sy as usize * q.width + sx as usize;
                if q.mask[src] == 0 {
                    continue;
                }
                let dest = (y + sy) as usize * 800 + (x + sx) as usize;
                let v = self.surface[dest];
                self.surface[dest] = match op {
                    1 => self.assets.shadow[v as usize],
                    2 => self.assets.light[v as usize],
                    3 => player(q.pixels[src], owner),
                    _ => q.pixels[src],
                };
            }
        }
    }
    pub fn bob(
        &mut self,
        x: i32,
        y: i32,
        id: u32,
        clipped: bool,
        owner: Option<u32>,
    ) -> Result<()> {
        let name = if owner.is_some() {
            "PastePlayerCmp"
        } else {
            "PasteBobCmp"
        };
        let mut args = vec![x, y, id as i32, clipped as i32];
        if let Some(o) = owner {
            args.push(o as i32);
        }
        self.observe(
            name,
            if owner.is_some() { 0x45c455 } else { 0x45b247 },
            &args,
        );
        self.raster(
            id,
            x,
            y,
            if owner.is_some() { 3 } else { 0 },
            owner.unwrap_or(0),
            if clipped {
                self.clip
            } else {
                self.screen_clip()
            },
        )
    }
    pub fn shadow(&mut self, x: i32, y: i32, id: u32, light: bool) -> Result<()> {
        self.observe(
            if light {
                "PasteEclairCmp"
            } else {
                "PasteOmbreCmp"
            },
            if light { 0x45ba23 } else { 0x45b796 },
            &[x, y, id as i32, 1],
        );
        self.raster(id, x, y, if light { 2 } else { 1 }, 0, self.clip)
    }
    pub fn filled(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: u8) -> Result<()> {
        let (x0, y0, x1, y1) = (i16(x0), i16(y0), i16(x1), i16(y1));
        self.observe("FilledBox", 0x45cacd, &[x0, y0, x1, y1, color as i32, 1]);
        let [l, t, r, b] = self.clip;
        let (x0, y0, x1, y1) = (
            x0.max(l).max(0),
            y0.max(t).max(0),
            x1.min(r).min(799),
            y1.min(b).min(599),
        );
        if x1 < x0 || y1 < y0 {
            return Ok(());
        }
        for y in y0..=y1 {
            self.surface[y as usize * 800 + x0 as usize..y as usize * 800 + x1 as usize + 1]
                .fill(color);
        }
        Ok(())
    }
    pub fn outline(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        color: u8,
        clipped: bool,
    ) -> Result<()> {
        self.observe(
            "Box",
            0x45c912,
            &[x0, y0, x1, y1, color as i32, clipped as i32],
        );
        let [l, t, r, b] = if clipped {
            self.clip
        } else {
            [
                0,
                0,
                self.u16(0x49a018)? as i32 - 1,
                self.u16(0x49a01a)? as i32 - 1,
            ]
        };
        if x0 > r || y0 > b || x1 < l || y1 < t {
            return Ok(());
        }
        let (mut x0, mut y0, mut x1, mut y1) = (x0.max(l), y0.max(t), x1.min(r), y1.min(b));
        if x1 < x0 {
            std::mem::swap(&mut x0, &mut x1);
        }
        if y1 < y0 {
            std::mem::swap(&mut y0, &mut y1);
        }
        if x0 < 0 || y0 < 0 || x1 > 799 || y1 > 599 {
            return Err(Error::Invalid("Box outside physical surface"));
        }
        for y in [y0, y1] {
            self.surface[y as usize * 800 + x0 as usize..y as usize * 800 + x1 as usize + 1]
                .fill(color);
        }
        for y in y0..=y1 {
            self.surface[y as usize * 800 + x0 as usize] = color;
            self.surface[y as usize * 800 + x1 as usize] = color;
        }
        Ok(())
    }
}
fn player(v: u8, owner: u32) -> u8 {
    if !(160..168).contains(&v) {
        return v;
    }
    let i = (v - 160) as usize;
    match owner {
        1 => [144, 145, 146, 148, 149, 150, 253, 254][i],
        2 => [136, 137, 138, 139, 140, 141, 142, 143][i],
        3 => [48, 50, 52, 54, 56, 58, 59, 61][i],
        4 => 47,
        0 => v,
        _ => unreachable!("validated player table owner"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_zero_skips_signed_clipping_and_destination_shadow() {
        // Synthetic fixture: literal zero is opaque, the upper-right pixel is a skip.
        let assets = Assets {
            sprites: vec![Sprite {
                width: 2,
                height: 2,
                pixels: vec![0, 8, 3, 4],
                mask: vec![255, 0, 255, 255],
                raw: vec![],
            }],
            palette: [0; 768],
            shadow: std::array::from_fn(|i| 255 - i as u8),
            light: std::array::from_fn(|i| i as u8),
        };
        let m = MemoriaJogo::default();
        let mut pixels = vec![7; 480000];
        let mut cache = Cache {
            keys: vec![0; 1024],
            raw: vec![0; 270336],
            cursor: 0,
        };
        let mut background = None;
        let mut text_boundaries = None;
        let mut c = Context {
            background: &mut background,
            text_boundaries: &mut text_boundaries,
            text_checked: 0,
            tick: None,
            m: &m,
            overrides: BTreeMap::new(),
            surface: &mut pixels,
            cache: &mut cache,
            assets: &assets,
            observations: vec![],
            audit: true,
            clip: [0, 0, 799, 599],
            camera: 0,
        };
        c.raster(1, 0, 0, 0, 0, c.clip).unwrap();
        assert_eq!(&c.surface[..2], &[0, 7]);
        assert_eq!(&c.surface[800..802], &[3, 4]);
        c.raster(1, -1, 0, 1, 0, c.clip).unwrap();
        assert_eq!(c.surface[0], 0);
        assert_eq!(c.surface[800], 252);
        c.raster(1, 0, -1, 0, 0, [1, 0, 799, 599]).unwrap();
        assert_eq!(&c.surface[..2], &[0, 4]);
    }
}
