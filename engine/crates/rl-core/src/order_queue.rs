//! Optional Shift+click order queue (`RULE_ORDER_QUEUE`, "Atalhos modernos").
//!
//! The original has no order queue: a Shift+click on the map that does not hit
//! a selectable unit is a no-op (`group_page`, page 12). Under the rule that
//! gesture issues player order tag 40 `[cell]`, which appends a move waypoint
//! to every alive own unit of the selection. Once per tick, a unit whose queue
//! is not empty and that is idle (state 7) starts its next waypoint exactly as
//! the original move order starts one unit (SetDeplaceEnginSelect 0x45efd8:
//! `frame_spawn::order` plus the tick stamp at +0x110).
//!
//! Queues live Rust-side (`MemoriaJogo::order_queue`, mirrored by `World`),
//! never in game memory, and are walked in ascending unit index. No RNG is
//! used. With the rule off nothing here runs; `process` sends tag 40 to the original default 0x45f2de.
use crate::{
    frame::stop,
    player::{alive, selections, unit, Memory},
    MemoriaJogo, Result,
};
use std::collections::BTreeMap;

/// Shift+click order queue. Off reproduces the original.
pub const RULE_ORDER_QUEUE: u32 = 2;
/// Player order tag issued by a queued Shift+click (the original uses 1..=39).
pub const TAG_QUEUE: u32 = 40;
/// Waypoints kept per unit; a click beyond this is ignored for that unit.
pub const MAX_WAYPOINTS: usize = 8;
/// Unit records the original allocates (EnginAlloc 0x35e80 / 276).
pub const MAX_UNITS: u16 = 800;

/// The waypoints of one unit, in the order they will be walked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Waypoints {
    /// The player that queued them; a unit that changes owner drops its queue.
    pub player: u8,
    pub cells: Vec<u16>,
}
/// Every unit's pending waypoints, keyed by unit index (deterministic order).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OrderQueue(pub(crate) BTreeMap<u16, Waypoints>);
impl OrderQueue {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// The pending waypoints of `unit` (empty when it has none).
    pub fn cells(&self, unit: u16) -> &[u16] {
        self.0.get(&unit).map_or(&[], |w| &w.cells)
    }
    pub fn iter(&self) -> impl Iterator<Item = (u16, &Waypoints)> {
        self.0.iter().map(|(u, w)| (*u, w))
    }
    /// Rebuilds a queue from saved entries; refuses what this module never writes.
    pub fn from_entries(
        entries: impl IntoIterator<Item = (u16, Waypoints)>,
    ) -> std::result::Result<Self, &'static str> {
        let mut map = BTreeMap::new();
        let mut previous = None;
        for (u, w) in entries {
            if u >= MAX_UNITS || previous.is_some_and(|p| u <= p) {
                return Err("order queue unit");
            }
            if w.cells.is_empty() || w.cells.len() > MAX_WAYPOINTS || w.player > 4 {
                return Err("order queue entry");
            }
            previous = Some(u);
            map.insert(u, w);
        }
        Ok(Self(map))
    }
}

impl MemoriaJogo {
    /// True when the world plays with the optional Shift+click order queue.
    pub fn order_queue_rule(&self) -> bool {
        self.rules & RULE_ORDER_QUEUE != 0
    }
}

/// The original move order for one unit (SetDeplaceEnginSelect's loop body).
fn start(m: &mut MemoriaJogo, u: u32, cell: u16) -> Result<()> {
    crate::frame_spawn::order(m, u, cell)?;
    let rec = unit(m, u)?;
    m.sd(rec + 0x110, m.d(0x508f7c)?)
}

fn idle(m: &MemoriaJogo, u: u32) -> Result<bool> {
    Ok(m.b(unit(m, u)? + 2)? == 7)
}

/// Whether `u` can still walk `player`'s queue: alive, still owned, on the map.
fn keeps(m: &MemoriaJogo, u: u32, player: u8) -> Result<bool> {
    let r = unit(m, u)?;
    Ok(alive(m, u)? && m.b(r)? == u32::from(player) && m.b(r + 0x72)? != 0)
}

/// Order tag 40 (ProcessInputGame): append `cell` to every alive own unit of
/// `p`'s selection. An idle unit with no earlier waypoint starts at once, so a
/// single Shift+click on idle units behaves like the original move click.
pub fn order(m: &mut MemoriaJogo, p: u32, cell: u16) -> Result<()> {
    if !m.order_queue_rule() {
        return stop(
            "ProcessInputGame tag 40 without the order-queue rule",
            0x45f2de,
        );
    }
    let mut spoke = false;
    for u in selections(m, p)? {
        let r = unit(m, u)?;
        if !alive(m, u)? || m.b(r)? != p {
            continue;
        }
        let key = u as u16;
        let mut queue = m.order_queue.0.remove(&key).unwrap_or_default();
        if queue.player != p as u8 {
            queue = Waypoints {
                player: p as u8,
                cells: Vec::new(),
            };
        }
        if queue.cells.len() < MAX_WAYPOINTS {
            queue.cells.push(cell);
        }
        if queue.cells.len() == 1 && idle(m, u)? {
            queue.cells.clear();
            start(m, u, cell)?;
        }
        if !queue.cells.is_empty() {
            m.order_queue.0.insert(key, queue);
        }
        // The move order's acknowledgement (ask mode 11), first moving unit only.
        let t = m.b(r + 1)?;
        if !spoke && !(11..=17).contains(&t) {
            spoke = true;
            if p == m.b(0x508041)? {
                crate::player_orders::ask(m, u, 11, 0, 0)?;
            }
        }
    }
    Ok(())
}

/// A player order that commands the selected units replaces their queues.
pub fn replaced(m: &mut MemoriaJogo, p: u32, tag: u32, words: &[u32; 5]) -> Result<()> {
    if !m.order_queue_rule() || m.order_queue.is_empty() {
        return Ok(());
    }
    match tag {
        // SetEnginDepose: the one unit in word +2.
        9 => {
            m.order_queue.0.remove(&((words[1] & 65535) as u16));
        }
        4..=8 | 14 | 16 | 18..=22 | 32 | 34 | 37 => {
            for u in selections(m, p)? {
                m.order_queue.0.remove(&(u as u16));
            }
        }
        _ => {}
    }
    Ok(())
}

/// Once per tick, after the units moved: idle units start their next waypoint.
pub fn tick(m: &mut MemoriaJogo) -> Result<()> {
    if !m.order_queue_rule() || m.order_queue.is_empty() {
        return Ok(());
    }
    let units: Vec<u16> = m.order_queue.0.keys().copied().collect();
    for key in units {
        let u = u32::from(key);
        let player = m.order_queue.0[&key].player;
        if !keeps(m, u, player)? {
            m.order_queue.0.remove(&key);
            continue;
        }
        if !idle(m, u)? {
            continue;
        }
        let queue = m.order_queue.0.get_mut(&key).expect("queued unit");
        let cell = queue.cells.remove(0);
        if queue.cells.is_empty() {
            m.order_queue.0.remove(&key);
        }
        start(m, u, cell)?;
    }
    Ok(())
}

/// For tools and tests: the units currently holding waypoints.
pub fn queued_units(m: &MemoriaJogo) -> Vec<u16> {
    m.order_queue.0.keys().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNITS: u32 = 0x0060_0000;
    const P: u32 = 0;

    /// Unit table, selection lists, tick counter and hover scratch; the local
    /// player is 1, so the acknowledgement voice (local player only) is skipped.
    fn world(rules: u32) -> MemoriaJogo {
        let mut m = MemoriaJogo::from_blocks([
            (0x4e19a0, UNITS.to_le_bytes().to_vec()),
            (0x496304, vec![0xff, 0xff]),
            (0x4e0f36, vec![0; 2]),
            (0x4e0f3c, vec![0; 4]),
            (0x508041, vec![1]),
            (0x508728, vec![0; 1024]),
            (0x508f7c, 1234u32.to_le_bytes().to_vec()),
            (UNITS, vec![0; 800 * 276]),
        ])
        .unwrap();
        m.rules = rules;
        m
    }
    /// An idle (state 7) land unit of kind 0 owned by `owner`, on the map at `cell`.
    fn place(m: &mut MemoriaJogo, i: u32, owner: u8, cell: u16) {
        let r = UNITS + 276 * i;
        m.set_u8(r, owner).unwrap();
        m.set_u8(r + 1, 0).unwrap();
        m.set_u8(r + 2, 7).unwrap();
        m.set_u8(r + 0x72, 1).unwrap();
        m.set_u16(r + 8, cell).unwrap();
        m.set_u16(r + 0x7c, cell).unwrap();
        m.set_u16(r + 0x7e, cell).unwrap();
    }
    fn select(m: &mut MemoriaJogo, units: &[u32]) {
        let base = 0x508728 + (P << 9);
        m.set_u16(base, units.len() as u16).unwrap();
        for (k, u) in units.iter().enumerate() {
            m.set_u16(base + 2 + 2 * k as u32, *u as u16).unwrap();
        }
    }
    fn state(m: &MemoriaJogo, i: u32) -> u8 {
        m.u8(UNITS + 276 * i + 2).unwrap()
    }
    fn dest(m: &MemoriaJogo, i: u32) -> u16 {
        m.u16(UNITS + 276 * i + 0x7e).unwrap()
    }
    /// The unit walked to its destination: EnginNextEtape sets state 7.
    fn arrive(m: &mut MemoriaJogo, i: u32) {
        let r = UNITS + 276 * i;
        let d = m.u16(r + 0x7e).unwrap();
        m.set_u16(r + 8, d).unwrap();
        m.set_u16(r + 0x7c, d).unwrap();
        m.set_u8(r + 2, 7).unwrap();
    }

    #[test]
    fn refused_without_the_rule() {
        let mut m = world(0);
        place(&mut m, 3, 0, 0x0505);
        select(&mut m, &[3]);
        let before = m.clone();
        assert!(order(&mut m, P, 0x0909).is_err());
        assert_eq!(m, before);
        assert!(m.order_queue.is_empty());
        // Without the rule the per-tick pass and the replacement never run.
        tick(&mut m).unwrap();
        replaced(&mut m, P, 5, &[5, 0x0909, 0, 0, 0]).unwrap();
        assert_eq!(m, before);
    }

    #[test]
    fn idle_units_start_at_once_exactly_like_the_move_order() {
        let mut queued = world(RULE_ORDER_QUEUE);
        place(&mut queued, 3, 0, 0x0505);
        select(&mut queued, &[3]);
        let mut plain = queued.clone();
        order(&mut queued, P, 0x0909).unwrap();
        crate::player_orders::call(
            &mut plain,
            "SetDeplaceEnginSelect",
            [P, 0x0909, P << 2, 0],
            0x45efd8,
        )
        .unwrap();
        assert_eq!(
            queued, plain,
            "game bytes differ from the original move order"
        );
        assert!(queued.order_queue.is_empty());
        assert_eq!(state(&queued, 3), 2);
        assert_eq!(dest(&queued, 3), 0x0909);
    }

    #[test]
    fn waypoints_are_walked_in_order_and_capped() {
        let mut m = world(RULE_ORDER_QUEUE);
        place(&mut m, 3, 0, 0x0505);
        place(&mut m, 9, 0, 0x0606);
        select(&mut m, &[3, 9]);
        let cells: Vec<u16> = (0..10).map(|k| 0x0a00 + k).collect();
        for c in &cells {
            order(&mut m, P, *c).unwrap();
        }
        // The first started at once; 8 more are kept, the 10th is dropped.
        assert_eq!(dest(&m, 3), cells[0]);
        assert_eq!(m.order_queue.cells(3), &cells[1..9]);
        assert_eq!(m.order_queue.cells(9), &cells[1..9]);
        // Moving units keep their waypoints.
        tick(&mut m).unwrap();
        assert_eq!(m.order_queue.cells(3).len(), 8);
        let mut walked = vec![dest(&m, 3)];
        for _ in 0..12 {
            arrive(&mut m, 3);
            tick(&mut m).unwrap();
            if dest(&m, 3) != *walked.last().unwrap() {
                walked.push(dest(&m, 3));
            }
        }
        assert_eq!(walked, cells[..9]);
        assert!(m.order_queue.cells(3).is_empty());
        assert_eq!(state(&m, 3), 7, "idle at the last waypoint");
        assert_eq!(
            m.order_queue.cells(9).len(),
            8,
            "unit 9 is still on its first move"
        );
    }

    #[test]
    fn a_plain_order_replaces_the_queue_and_selection_does_not() {
        let mut m = world(RULE_ORDER_QUEUE);
        place(&mut m, 3, 0, 0x0505);
        place(&mut m, 9, 0, 0x0606);
        select(&mut m, &[3, 9]);
        for c in [0x0a00, 0x0a01, 0x0a02] {
            order(&mut m, P, c).unwrap();
        }
        // Selection-only orders and other players' orders leave queues alone.
        for tag in [1, 2, 3, 11, 12, 36, 10, 13, 17, 23, 38, 39] {
            replaced(&mut m, P, tag, &[tag, 0, 0, 0, 0]).unwrap();
        }
        assert_eq!(m.order_queue.cells(3).len(), 2);
        // A drop order (tag 9) concerns its one unit only.
        replaced(&mut m, P, 9, &[9, 3, 1, 0, 0]).unwrap();
        assert!(m.order_queue.cells(3).is_empty());
        assert_eq!(m.order_queue.cells(9).len(), 2);
        // A move (tag 5) clears every selected unit's queue.
        order(&mut m, P, 0x0b00).unwrap();
        replaced(&mut m, P, 5, &[5, 0x0c00, 0, 0, 0]).unwrap();
        assert!(m.order_queue.is_empty());
    }

    #[test]
    fn dead_converted_or_boarded_units_drop_their_queue() {
        let mut m = world(RULE_ORDER_QUEUE);
        for (i, c) in [(3, 0x0505), (4, 0x0506), (5, 0x0507), (6, 0x0508)] {
            place(&mut m, i, 0, c);
        }
        select(&mut m, &[3, 4, 5, 6]);
        for c in [0x0a00, 0x0a01] {
            order(&mut m, P, c).unwrap();
        }
        assert_eq!(queued_units(&m), vec![3, 4, 5, 6]);
        m.set_u8(UNITS + 276 * 3 + 2, 0).unwrap(); // dead
        m.set_u8(UNITS + 276 * 4, 2).unwrap(); // converted to player 2
        m.set_u8(UNITS + 276 * 5 + 0x72, 0).unwrap(); // inside a building or vehicle
        for i in [3, 4, 5, 6] {
            arrive(&mut m, i);
        }
        m.set_u8(UNITS + 276 * 3 + 2, 0).unwrap();
        tick(&mut m).unwrap();
        assert_eq!(
            queued_units(&m),
            Vec::<u16>::new(),
            "6 started its last waypoint"
        );
        assert_eq!(dest(&m, 6), 0x0a01);
        assert_eq!(dest(&m, 3), 0x0a00, "a dead unit gets no new move");
    }

    #[test]
    fn queues_restore_only_from_well_formed_entries() {
        let w = |cells: Vec<u16>| Waypoints { player: 0, cells };
        assert!(OrderQueue::from_entries([(3, w(vec![1])), (2, w(vec![1]))]).is_err());
        assert!(OrderQueue::from_entries([(800, w(vec![1]))]).is_err());
        assert!(OrderQueue::from_entries([(3, w(vec![]))]).is_err());
        assert!(OrderQueue::from_entries([(3, w(vec![1; 9]))]).is_err());
        let q = OrderQueue::from_entries([(2, w(vec![7, 8])), (5, w(vec![9]))]).unwrap();
        assert_eq!(q.cells(2), &[7, 8]);
        assert_eq!(q.cells(4), &[] as &[u16]);
    }

    /// The module never names a random source.
    #[test]
    fn no_random_source() {
        let source = include_str!("order_queue.rs");
        let code = &source[..source.find("#[cfg(test)]").expect("test module")];
        for name in [
            "random(",
            "crt_rand",
            "\"Random\"",
            "LongRand",
            "perm(",
            "0x497164",
            "0x4e1d18",
        ] {
            assert!(!code.contains(name), "order_queue uses {name}");
        }
    }
}
