use crate::{EngineError, Result};
use std::cell::Cell;
use std::collections::BTreeMap;

/// Sparse, owned blocks, ordered by original 32-bit base. No implicit regions.
#[derive(Debug, Clone)]
pub struct MemoriaJogo {
    // Slot arena plus an ordered base -> slot index. `hint` caches the slot of the
    // last located block; a hit is only taken when the address lies inside that live
    // block, which (blocks never overlap) is exactly the block `index.range(..=a)`
    // would pick. Freed slots keep an empty Vec and so can never hit.
    index: BTreeMap<u32, usize>,
    slots: Vec<(u32, Vec<u8>)>,
    free: Vec<usize>,
    hint: Cell<usize>,
    /// Transaction-local declared input, never part of projected game bytes.
    pub(crate) player: Option<crate::player::Runtime>,
    journal: Journal,
    pub(crate) audit: bool,
    /// World rule set (`crate::RULE_*` bits), stamped by `World` on every
    /// projection; never part of projected game bytes or equality.
    pub(crate) rules: u32,
    /// Shift+click waypoints (`crate::order_queue`), stamped and collected by
    /// `World` around every phase; never part of projected game bytes or equality.
    pub(crate) order_queue: crate::OrderQueue,
    #[cfg(feature = "validation-trace")]
    validation_trace: Option<Vec<(u32, Vec<u8>)>>,
}

#[cfg(test)]
mod transactions {
    use super::*;
    #[test]
    fn overlapping_inline_and_large_writes_roll_back_in_reverse_order() {
        let mut m = MemoriaJogo::from_blocks([(100, (0..96).collect())]).unwrap();
        let before = m.clone();
        m.begin();
        m.set_u32(104, 0xdeadbeef).unwrap();
        m.write(106, &[0xa5; 24]).unwrap();
        m.set_u16(105, 0x4321).unwrap();
        assert!(m.write(190, &[0; 8]).is_err());
        m.rollback();
        assert_eq!(m, before);
        assert_eq!(m.changes().count(), 0);
        let capacity = m.journal.undo.capacity();
        m.begin();
        m.set_u8(100, 9).unwrap();
        m.commit();
        assert_eq!(m.u8(100).unwrap(), 9);
        assert_eq!(m.journal.undo.capacity(), capacity);
    }
    #[test]
    fn declarations_removals_and_reused_addresses_restore_exact_layout() {
        let mut m = MemoriaJogo::from_blocks([(100, vec![7; 16]), (200, vec![9; 4])]).unwrap();
        let before = m.clone();
        m.begin();
        m.set_u8(102, 3).unwrap();
        m.remove(100).unwrap();
        m.declare(100, vec![1; 12]).unwrap();
        m.set_u32(104, 77).unwrap();
        m.declare(300, vec![2; 40]).unwrap();
        assert!(m.structural_change());
        m.rollback();
        assert_eq!(m, before);
    }
    #[test]
    fn last_hit_slot_never_outlives_its_block() {
        let mut m = MemoriaJogo::from_blocks([(100, vec![7; 16]), (200, vec![9; 4])]).unwrap();
        assert_eq!(m.u8(105).unwrap(), 7);
        m.remove(100).unwrap();
        assert!(m.u8(105).is_err());
        m.declare(104, vec![5; 2]).unwrap();
        assert_eq!(m.u8(105).unwrap(), 5);
        assert!(m.u8(106).is_err());
        assert!(m.u8(100).is_err());
        assert!(m.read(105, 2).is_err());
        m.declare(300, vec![1; 8]).unwrap();
        assert_eq!(m.u8(201).unwrap(), 9);
        assert_eq!(m.u8(307).unwrap(), 1);
        assert!(m.u8(308).is_err());
        assert_eq!(
            m.blocks().map(|(b, d)| (b, d.len())).collect::<Vec<_>>(),
            [(104, 2), (200, 4), (300, 8)]
        );
    }
}
#[derive(Debug, Clone, Default)]
struct Journal {
    active: bool,
    structural: bool,
    undo: Vec<Undo>,
}
#[derive(Debug, Clone)]
enum Undo {
    Small {
        address: u32,
        bytes: [u8; 8],
        length: u8,
    },
    Large {
        address: u32,
        bytes: Vec<u8>,
    },
    Added(u32),
    Removed(u32, Vec<u8>),
}
impl Default for MemoriaJogo {
    fn default() -> Self {
        Self {
            index: BTreeMap::new(),
            slots: Vec::new(),
            free: Vec::new(),
            hint: Cell::new(0),
            player: None,
            journal: Journal::default(),
            audit: true,
            rules: 0,
            order_queue: crate::OrderQueue::default(),
            #[cfg(feature = "validation-trace")]
            validation_trace: None,
        }
    }
}
impl PartialEq for MemoriaJogo {
    fn eq(&self, other: &Self) -> bool {
        self.blocks().eq(other.blocks()) && self.player == other.player
    }
}
impl Eq for MemoriaJogo {}
impl MemoriaJogo {
    #[cfg(feature = "validation-trace")]
    pub fn start_validation_trace(&mut self) {
        self.validation_trace = Some(Vec::new());
    }
    #[cfg(feature = "validation-trace")]
    pub fn take_validation_trace(&mut self) -> Vec<(u32, Vec<u8>)> {
        self.validation_trace.take().unwrap_or_default()
    }
    /// Reused undo storage: game bytes are changed in place, never speculatively published.
    pub(crate) fn begin(&mut self) {
        assert!(!self.journal.active, "nested memory transaction");
        if self.journal.undo.capacity() == 0 {
            self.journal.undo.reserve(4096);
        }
        self.journal.active = true;
        self.journal.structural = false;
    }
    pub(crate) fn structural_change(&self) -> bool {
        self.journal.structural
    }
    pub(crate) fn changes(&self) -> impl Iterator<Item = (u32, usize)> + '_ {
        self.journal.undo.iter().filter_map(|u| match u {
            Undo::Small {
                address, length, ..
            } => Some((*address, *length as usize)),
            Undo::Large { address, bytes } => Some((*address, bytes.len())),
            _ => None,
        })
    }
    pub(crate) fn commit(&mut self) {
        self.journal.active = false;
        self.journal.undo.clear();
        self.journal.structural = false;
    }
    pub(crate) fn rollback(&mut self) {
        self.journal.active = false;
        while let Some(undo) = self.journal.undo.pop() {
            match undo {
                Undo::Small {
                    address,
                    bytes,
                    length,
                } => self
                    .write(address, &bytes[..length as usize])
                    .expect("undo write"),
                Undo::Large { address, bytes } => self.write(address, &bytes).expect("undo span"),
                Undo::Added(base) => {
                    let slot = self.index.remove(&base).expect("undo declaration");
                    self.slots[slot].1 = Vec::new();
                    self.free.push(slot);
                }
                Undo::Removed(base, bytes) => {
                    self.insert_slot(base, bytes);
                }
            }
        }
        self.journal.structural = false;
    }
    pub fn from_blocks(blocks: impl IntoIterator<Item = (u32, Vec<u8>)>) -> Result<Self> {
        let mut result = Self::default();
        for (base, bytes) in blocks {
            result.declare(base, bytes)?;
        }
        Ok(result)
    }
    pub fn blocks(&self) -> impl Iterator<Item = (u32, &[u8])> {
        self.index
            .iter()
            .map(|(&base, &slot)| (base, self.slots[slot].1.as_slice()))
    }
    fn insert_slot(&mut self, base: u32, bytes: Vec<u8>) {
        let slot = if let Some(slot) = self.free.pop() {
            self.slots[slot] = (base, bytes);
            slot
        } else {
            self.slots.push((base, bytes));
            self.slots.len() - 1
        };
        self.index.insert(base, slot);
    }
    /// Slot of the block with the greatest base <= address that contains it, if any
    /// block has base <= address; exactly the original range(..=address) choice.
    #[inline]
    fn locate(&self, address: u32) -> Option<usize> {
        let h = self.hint.get();
        if let Some((base, data)) = self.slots.get(h) {
            if address >= *base && ((address - *base) as usize) < data.len() {
                return Some(h);
            }
        }
        let (_, &slot) = self.index.range(..=address).next_back()?;
        self.hint.set(slot);
        Some(slot)
    }
    pub fn declare(&mut self, base: u32, bytes: Vec<u8>) -> Result<()> {
        if bytes.is_empty() {
            return Err(EngineError::EmptyRegion);
        }
        let end = u64::from(base)
            .checked_add(u64::try_from(bytes.len()).map_err(|_| EngineError::AddressOverflow)?)
            .ok_or(EngineError::AddressOverflow)?;
        if end > 1u64 << 32 {
            return Err(EngineError::AddressOverflow);
        }
        if let Some((&previous, &slot)) = self.index.range(..=base).next_back() {
            let data = &self.slots[slot].1;
            let previous_end = u64::from(previous)
                + u64::try_from(data.len()).map_err(|_| EngineError::AddressOverflow)?;
            if u64::from(base) < previous_end {
                return Err(EngineError::OverlappingRegions);
            }
        }
        if let Some((&next, _)) = self.index.range(base..).next() {
            if u64::from(next) < end {
                return Err(EngineError::OverlappingRegions);
            }
        }
        self.insert_slot(base, bytes);
        if self.journal.active {
            self.journal.structural = true;
            self.journal.undo.push(Undo::Added(base));
        }
        Ok(())
    }
    pub fn remove(&mut self, base: u32) -> Result<Vec<u8>> {
        let slot = self
            .index
            .remove(&base)
            .ok_or(EngineError::UndeclaredAccess {
                address: base,
                size: 1,
            })?;
        let bytes = std::mem::take(&mut self.slots[slot].1);
        self.free.push(slot);
        if self.journal.active {
            self.journal.structural = true;
            self.journal.undo.push(Undo::Removed(base, bytes.clone()));
        }
        Ok(bytes)
    }
    pub fn read(&self, address: u32, size: usize) -> Result<&[u8]> {
        if let Some(slot) = self.locate(address) {
            let (base, data) = (&self.slots[slot].0, &self.slots[slot].1);
            let base = *base;
            let offset =
                usize::try_from(address - base).map_err(|_| EngineError::AddressOverflow)?;
            if offset
                .checked_add(size)
                .is_some_and(|end| end <= data.len())
            {
                return Ok(&data[offset..offset + size]);
            }
        }
        Err(EngineError::UndeclaredAccess { address, size })
    }
    pub fn write(&mut self, address: u32, bytes: &[u8]) -> Result<()> {
        let Some(slot) = self.locate(address) else {
            return Err(EngineError::UndeclaredAccess {
                address,
                size: bytes.len(),
            });
        };
        let (base, data) = {
            let entry = &mut self.slots[slot];
            (entry.0, &mut entry.1)
        };
        let offset = (address - base) as usize;
        let Some(target) = data.get_mut(
            offset
                ..offset
                    .checked_add(bytes.len())
                    .ok_or(EngineError::AddressOverflow)?,
        ) else {
            return Err(EngineError::UndeclaredAccess {
                address,
                size: bytes.len(),
            });
        };
        #[cfg(feature = "validation-trace")]
        if let Some(trace) = &mut self.validation_trace {
            trace.push((address, bytes.to_vec()));
        }
        if target == bytes {
            return Ok(());
        }
        if self.journal.active {
            if bytes.len() <= 8 {
                let mut old = [0; 8];
                old[..bytes.len()].copy_from_slice(target);
                self.journal.undo.push(Undo::Small {
                    address,
                    bytes: old,
                    length: bytes.len() as u8,
                });
            } else {
                self.journal.undo.push(Undo::Large {
                    address,
                    bytes: target.to_vec(),
                });
            }
        }
        target.copy_from_slice(bytes);
        Ok(())
    }
    pub fn u8(&self, address: u32) -> Result<u8> {
        Ok(self.read(address, 1)?[0])
    }
    pub fn u16(&self, address: u32) -> Result<u16> {
        Ok(u16::from_le_bytes(
            self.read(address, 2)?.try_into().expect("width"),
        ))
    }
    pub fn u32(&self, address: u32) -> Result<u32> {
        Ok(u32::from_le_bytes(
            self.read(address, 4)?.try_into().expect("width"),
        ))
    }
    pub fn set_u8(&mut self, address: u32, value: u8) -> Result<()> {
        self.write(address, &[value])
    }
    pub fn set_u16(&mut self, address: u32, value: u16) -> Result<()> {
        self.write(address, &value.to_le_bytes())
    }
    pub fn set_u32(&mut self, address: u32, value: u32) -> Result<()> {
        self.write(address, &value.to_le_bytes())
    }
    /// A missing whole field is absent; a partially captured field is an error.
    pub fn optional_u32(&self, address: u32) -> Result<Option<u32>> {
        if let Ok(value) = self.u32(address) {
            return Ok(Some(value));
        }
        if (0..4).any(|offset| self.u8(address.wrapping_add(offset)).is_ok()) {
            return Err(EngineError::IncompleteField { address });
        }
        Ok(None)
    }
}
