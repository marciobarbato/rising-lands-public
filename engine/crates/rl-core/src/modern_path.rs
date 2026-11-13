//! Optional modern unit paths ("Caminhos melhores", `RULE_MODERN_PATHS`).
//!
//! Reached only through two entry branches taken when the world's rule bit is
//! set: DeplaceEngin (`frame_movement::displace`) and the group move order
//! (SetDeplaceEnginSelect in `player_orders`). With the bit clear none of this
//! runs and the original routines are unchanged.
//!
//! Only movement *decisions* change. Steps are still taken by the original
//! CalculNextCase/EnginNextCaseChemin/AvanceEngin through the per-unit route
//! buffer, and every step is still tested by the original EnginPeutPasser, so
//! occupancy rules are the original ones.
//!
//! Determinism: integer costs, fixed neighbour order, a heap ordered by
//! (f, h, insertion counter), no hashing containers, no floating point. This
//! module never draws a random value (game `Random`, CRT `rand` or the
//! permutation cursor); `tests::no_random_source` enforces it.
//!
//! Per-unit state lives in original fields, so saves carry it unchanged:
//! `+0x88` move state (5 follow route, 6 wait, 3 replan, anything else a fresh
//! order), `+0x8b` = consecutive hard blocks (bits 6..7), replan-after-wait
//! (bit 5) and remaining wait frames (bits 0..4), the route buffer, and
//! `+0x7e` the goal.
//!
//! Blocking: a unit in front that is stepping, or that still has an order of
//! its own, is traffic: wait 2 frames and try the same route again. A unit in
//! front that has no order, or that waits for us (head-on, a 2x2 footprint
//! included, or a longer wait cycle through other standing units, which
//! traffic waits would never break), is a hard block: shove
//! it if it is an idle friend, wait 4/8/16 frames and replan around it; the
//! fourth consecutive hard block, or any hard block next to the goal, ends the
//! order where the unit stands.
use crate::{frame_movement, frame_path, frame_spawn, MemoriaJogo, Result};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

/// Planner work per phase, in A* expansions; the plans past it wait for the
/// next frame, in unit order. A search is charged its expansions but at least
/// `SEARCH_MIN`, so cheap searches still run eight per phase; a search already
/// started always finishes (the budget is checked before it), which bounds a
/// phase at `PHASE_BUDGET + MAX_EXPANSIONS` expansions.
const PHASE_BUDGET: u32 = 24576;
const SEARCH_MIN: u32 = PHASE_BUDGET / 8;
/// Expansions per search; past it the closest expanded cell is used.
const MAX_EXPANSIONS: u32 = 16384;
const STEP: u32 = 10;
const TURN: u32 = 1;
/// Extra cost of crossing a stationary friendly unit (it can be shoved).
const STATIONARY_FRIEND: u32 = 40;
/// Consecutive blocks before the unit settles where it is.
const MAX_BLOCKS: u8 = 3;
const TRAFFIC_WAIT: u8 = 2;
const REPLAN_AFTER_WAIT: u8 = 0x20;
const WAIT_MASK: u8 = 0x1f;
const ROUTE_CAPACITY: usize = 255;
const DIRECTIONS: [u8; 8] = [1, 3, 5, 7, 9, 11, 13, 15];
const NONE: u16 = 65535;

fn unit(m: &MemoriaJogo, i: u32) -> Result<u32> {
    frame_spawn::unit(m, i)
}
fn cheb(a: u16, b: u16) -> u32 {
    let dx = (i32::from(a & 255) - i32::from(b & 255)).unsigned_abs();
    let dy = (i32::from(a >> 8) - i32::from(b >> 8)).unsigned_abs();
    dx.max(dy)
}
fn grid8(m: &MemoriaJogo, p: u32, c: u16) -> Result<u8> {
    m.u8(m.u32(p)?.wrapping_add(u32::from(c)))
}
fn grid16(m: &MemoriaJogo, p: u32, c: u16) -> Result<u16> {
    m.u16(m.u32(p)?.wrapping_add(2 * u32::from(c)))
}
/// The original `mask_cell`: occupation bit of layer `mask` (0xfd objects are ground).
fn mask_cell(m: &MemoriaJogo, c: u16, mask: u8) -> Result<bool> {
    Ok(if grid8(m, 0x507d74, c)? == 0xfd {
        mask & 1 != 0
    } else {
        grid8(m, 0x507da8, c)? & mask != 0
    })
}
/// Gate object code (the original `mask_cell` tests the same literal).
const GATE: u8 = 0xfd;
fn footprint(class: u8) -> &'static [u16] {
    if class == 1 {
        &[0]
    } else {
        &[0, 1, 257, 256]
    }
}

/// True when the modern policy drives this unit (the original otherwise).
pub(crate) fn handles(m: &MemoriaJogo, i: u32) -> Result<bool> {
    let t = m.u8(unit(m, i)? + 1)?;
    Ok(frame_path::CLASSES
        .get(usize::from(t))
        .is_some_and(|c| matches!(c, 1..=3)))
}

#[derive(Clone, Copy)]
struct Mover {
    unit: u32,
    kind: u8,
    owner: u8,
    class: u8,
    mask: u8,
}
fn mover(m: &MemoriaJogo, i: u32) -> Result<Mover> {
    let u = unit(m, i)?;
    let kind = m.u8(u + 1)?;
    Ok(Mover {
        unit: i,
        kind,
        owner: m.u8(u)?,
        class: frame_path::CLASSES[usize::from(kind)],
        mask: 1 + u8::from(m.u8(u + 0xab)? != 0),
    })
}
/// A unit standing on its cell this frame (no sub-step, no committed step).
fn stationary(m: &MemoriaJogo, q: u32) -> Result<bool> {
    Ok(m.u8(q + 0x86)? == 0 && m.u8(q + 0x87)? == 0 && m.u16(q + 0x7c)? == m.u16(q + 8)?)
}
/// Occupant kinds a crusher (12/15) runs over, as EnginPeutPasser (0x4428fe).
fn yielding(kind: u8) -> bool {
    matches!(kind, 0..=2 | 4..=10 | 18 | 19 | 22)
}
/// Read-only views of the planning grids and the map size for one search.
/// Every read is the one `grid8`/`grid16`/`frame_path::neighbor` would make
/// (the search writes nothing); a cell outside a view falls back to them, so
/// results and errors are unchanged. This only avoids a slot lookup per read.
struct Grids<'a> {
    w: i32,
    h: i32,
    mask: Option<&'a [u8]>,
    object: Option<&'a [u8]>,
    tags: Option<&'a [u8]>,
}
impl<'a> Grids<'a> {
    fn new(m: &'a MemoriaJogo) -> Result<Self> {
        let w = m.u16(0x497168)?;
        let h = m.u16(0x49716a)?;
        let cells = if w == 0 || h == 0 {
            0
        } else {
            256 * (usize::from(h) - 1) + usize::from(w)
        };
        let view = |p: u32, n: usize| m.u32(p).ok().and_then(|a| m.read(a, n).ok());
        Ok(Self {
            w: i32::from(w),
            h: i32::from(h),
            mask: view(0x507da8, cells),
            object: view(0x507d74, cells),
            tags: view(0x4e0f38, 2 * cells),
        })
    }
    fn mask(&self, m: &MemoriaJogo, c: u16) -> Result<u8> {
        match self.mask.and_then(|s| s.get(usize::from(c))) {
            Some(&b) => Ok(b),
            None => grid8(m, 0x507da8, c),
        }
    }
    fn object(&self, m: &MemoriaJogo, c: u16) -> Result<u8> {
        match self.object.and_then(|s| s.get(usize::from(c))) {
            Some(&b) => Ok(b),
            None => grid8(m, 0x507d74, c),
        }
    }
    fn tag(&self, m: &MemoriaJogo, c: u16) -> Result<u16> {
        let i = 2 * usize::from(c);
        match self.tags.and_then(|s| s.get(i..i + 2)) {
            Some(b) => Ok(u16::from_le_bytes([b[0], b[1]])),
            None => grid16(m, 0x4e0f38, c),
        }
    }
    /// Planning occupancy of layer `mask`, as EnginPeutPasser reads it when a unit
    /// steps. The original planner (`mask_cell`) counts every gate cell as occupied
    /// for ground units, open or closed, so CheminCalcul never routes through a
    /// gate and only the greedy step crosses an open one. Here an open gate
    /// (occupation bit 0 cleared by BatimentNextEtape at phase 15) is passable and
    /// a closed gate blocks, exactly as for the step.
    fn occupied(&self, m: &MemoriaJogo, c: u16, mask: u8) -> Result<bool> {
        let flag = self.mask(m, c)?;
        Ok(if self.object(m, c)? == GATE {
            mask & 1 != 0 && flag & 1 != 0
        } else {
            flag & mask != 0
        })
    }
    /// `frame_path::neighbor` with the map size read once.
    fn neighbor(&self, c: u16, d: u8) -> u16 {
        let (dx, dy) = frame_path::vector(d);
        let x = i32::from(c & 255) + dx;
        let y = i32::from(c >> 8) + dy;
        if x >= 0 && x < self.w && y >= 0 && y < self.h {
            (x + 256 * y) as u16
        } else {
            NONE
        }
    }
}
/// Search buffers kept between searches (planner scratch, never game state).
/// A cell's g/parent/heading are valid when `seen` holds the current stamp,
/// and it is closed when `closed` does, so nothing is cleared per search.
struct Scratch {
    stamp: u32,
    seen: Vec<u32>,
    closed: Vec<u32>,
    g: Vec<u32>,
    parent: Vec<u16>,
    heading: Vec<u8>,
    /// `cell_cost` per cell, valid when `costed` holds the stamp (the search
    /// writes nothing, so a cell's cost cannot change within one search).
    costed: Vec<u32>,
    cost: Vec<Option<u32>>,
    heap: BinaryHeap<Reverse<(u32, u32, u32, u16)>>,
}
std::thread_local! {
    static SCRATCH: std::cell::RefCell<Scratch> = const {
        std::cell::RefCell::new(Scratch {
            stamp: 0,
            seen: Vec::new(),
            closed: Vec::new(),
            g: Vec::new(),
            parent: Vec::new(),
            heading: Vec::new(),
            costed: Vec::new(),
            cost: Vec::new(),
            heap: BinaryHeap::new(),
        })
    };
}
/// Planning cost of standing on `c`: None when blocked; Some(extra) otherwise.
/// Terrain, structures and closed gates block; open gates do not; moving units are ignored; stationary enemies
/// block (except what a crusher runs over); stationary friends cost extra.
fn cell_cost(m: &MemoriaJogo, grids: &Grids, v: &Mover, c: u16) -> Result<Option<u32>> {
    if v.class != 1 && (c & 255 == 0 || c >> 8 == 0) {
        return Ok(None);
    }
    let mut extra = 0;
    for d in footprint(v.class) {
        let sc = c.wrapping_sub(*d);
        if !grids.occupied(m, sc, v.mask)? {
            continue;
        }
        let tag = grids.tag(m, sc)?;
        if tag & 0xc000 != 0x4000 {
            return Ok(None);
        }
        let other = u32::from((tag & 0x3ff8) >> 3);
        if other == v.unit {
            continue;
        }
        let q = unit(m, other)?;
        if !stationary(m, q)? {
            continue;
        }
        if frame_path::allied(m, v.owner, m.u8(q)?)? {
            extra = extra.max(STATIONARY_FRIEND);
        } else if !(matches!(v.kind, 12 | 15) && yielding(m.u8(q + 1)?)) {
            return Ok(None);
        }
    }
    Ok(Some(extra))
}

struct Plan {
    /// Cells after the start, in walking order.
    cells: Vec<u16>,
    /// Last cell: the goal, or the closest reachable cell to it.
    end: u16,
    /// Cells expanded (the planner's work, charged to the phase budget).
    expansions: u32,
}
/// A* from `start` toward `goal`. A cell where another unit stands can never
/// be the end; when the goal is not reached the end is the expanded cell
/// (other than a gate) minimising (Chebyshev distance to goal, path cost, cell
/// index).
fn search(m: &MemoriaJogo, v: &Mover, start: u16, goal: u16) -> Result<Plan> {
    SCRATCH.with(|scratch| search_with(m, v, start, goal, &mut scratch.borrow_mut()))
}
fn search_with(m: &MemoriaJogo, v: &Mover, start: u16, goal: u16, s: &mut Scratch) -> Result<Plan> {
    let grids = Grids::new(m)?;
    let w = grids.w as u32;
    let index = |c: u16| (u32::from(c & 255) + w * u32::from(c >> 8)) as usize;
    let size = (w * grids.h as u32) as usize;
    if s.seen.len() < size {
        s.seen.resize(size, 0);
        s.closed.resize(size, 0);
        s.g.resize(size, 0);
        s.parent.resize(size, 0);
        s.heading.resize(size, 0);
        s.costed.resize(size, 0);
        s.cost.resize(size, None);
    }
    s.stamp = s.stamp.wrapping_add(1);
    if s.stamp == 0 {
        s.seen.fill(0);
        s.closed.fill(0);
        s.costed.fill(0);
        s.stamp = 1;
    }
    let stamp = s.stamp;
    s.heap.clear();
    let Scratch {
        seen,
        closed,
        g,
        parent,
        heading,
        costed,
        cost: costs,
        heap,
        ..
    } = s;
    let mut cost_at = |c: u16| -> Result<Option<u32>> {
        let i = index(c);
        if costed[i] != stamp {
            costs[i] = cell_cost(m, &grids, v, c)?;
            costed[i] = stamp;
        }
        Ok(costs[i])
    };
    // Unvisited cells read as g = MAX, parent = NONE, heading = 0.
    let cost_of =
        |seen: &[u32], g: &[u32], i: usize| if seen[i] == stamp { g[i] } else { u32::MAX };
    let mut counter = 0u32;
    let si = index(start);
    seen[si] = stamp;
    g[si] = 0;
    parent[si] = NONE;
    heading[si] = 0;
    heap.push(Reverse((
        STEP * cheb(start, goal),
        cheb(start, goal),
        counter,
        start,
    )));
    let mut best = (cheb(start, goal), 0u32, start);
    let mut found = false;
    let mut expansions = 0;
    // A goal that can never be an end (an attack approach aims at the target's
    // own cell) is never found, so the search used to run to MAX_EXPANSIONS.
    // The best fallback is then at distance 1; once it is, and the popped f
    // exceeds its f, no later cell can beat it (consistent heuristic: every
    // later f is at least the popped one, and a distance-1 cell has
    // g = f - STEP). Stopping there returns the same plan.
    let unreachable_goal = goal != start && cost_at(goal)? != Some(0);
    while let Some(Reverse((f, _, _, c))) = heap.pop() {
        if unreachable_goal && best.0 == 1 && f > best.1 + STEP {
            break;
        }
        let ci = index(c);
        if closed[ci] == stamp {
            continue;
        }
        closed[ci] = stamp;
        expansions += 1;
        let gc = cost_of(seen, g, ci);
        let free = c == start || cost_at(c)? == Some(0);
        if c == goal && free {
            found = true;
            break;
        }
        // A fallback end is never a gate cell: a unit parked there would stop
        // the gate from closing on it.
        if free && grids.object(m, c)? != GATE && (cheb(c, goal), gc, c) < best {
            best = (cheb(c, goal), gc, c);
        }
        if expansions >= MAX_EXPANSIONS {
            break;
        }
        for d in DIRECTIONS {
            let n = grids.neighbor(c, d);
            if n == NONE || closed[index(n)] == stamp {
                continue;
            }
            let Some(extra) = cost_at(n)? else {
                continue;
            };
            let turn = if c != start && heading[ci] != d {
                TURN
            } else {
                0
            };
            let cost = gc + STEP + extra + turn;
            let ni = index(n);
            if cost < cost_of(seen, g, ni) {
                seen[ni] = stamp;
                g[ni] = cost;
                parent[ni] = c;
                heading[ni] = d;
                counter += 1;
                let hn = cheb(n, goal);
                heap.push(Reverse((cost + STEP * hn, hn, counter, n)));
            }
        }
    }
    let end = if found { goal } else { best.2 };
    let mut cells = Vec::new();
    let mut at = end;
    while at != start {
        cells.push(at);
        at = parent[index(at)];
    }
    cells.reverse();
    Ok(Plan {
        cells,
        end,
        expansions,
    })
}

fn route(m: &MemoriaJogo, i: u32) -> Result<u32> {
    frame_path::route(m, i)
}
/// Same layout as CheminCalcul: count, then cells last-to-first.
fn write_route(m: &mut MemoriaJogo, i: u32, cells: &[u16]) -> Result<()> {
    let r = route(m, i)?;
    let n = cells.len().min(ROUTE_CAPACITY);
    m.set_u16(r, n as u16)?;
    for j in 0..n {
        m.set_u16(r.wrapping_add(2 + 2 * j as u32), cells[n - 1 - j])?;
    }
    Ok(())
}
/// End the order on the current cell, as the original's give-up signal
/// (destination = current cell) so callers see the same outcome.
fn settle(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    let u = unit(m, i)?;
    m.set_u16(u + 0x7e, m.u16(u + 8)?)?;
    m.set_u16(route(m, i)?, 0)?;
    m.set_u8(u + 0x88, 0)?;
    m.set_u8(u + 0x8b, 0)?;
    frame_spawn::release(m, i)?;
    Ok(0)
}

/// Modern DeplaceEngin.
pub(crate) fn displace(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    let u = unit(m, i)?;
    let state = m.u8(u + 0x88)?;
    let here = m.u16(u + 8)?;
    let mid_step = m.u8(u + 0x86)? != 0 || m.u8(u + 0x87)? != 0;
    if mid_step || (state == 5 && m.u16(u + 0x7c)? != here) {
        // Inside a step (or committed to one while turning): the original
        // CalculNextCase finishes it.
        return frame_movement::next_case(m, i);
    }
    if here == m.u16(u + 0x7e)? {
        m.set_u16(route(m, i)?, 0)?;
        m.set_u8(u + 0x88, 0)?;
        return Ok(0);
    }
    match state {
        6 => {
            let timer = m.u8(u + 0x8b)?;
            if timer & WAIT_MASK > 1 {
                m.set_u8(u + 0x8b, timer - 1)?;
                return Ok(0);
            }
            m.set_u8(u + 0x8b, timer & 0xc0)?;
            let r = route(m, i)?;
            let n = m.u16(r)?;
            if timer & REPLAN_AFTER_WAIT == 0
                && n != 0
                && cheb(here, m.u16(r.wrapping_add(2 * u32::from(n)))?) == 1
            {
                follow(m, i)
            } else {
                plan(m, i)
            }
        }
        5 => {
            let r = route(m, i)?;
            let n = m.u16(r)?;
            if n != 0 && cheb(here, m.u16(r.wrapping_add(2 * u32::from(n)))?) == 1 {
                follow(m, i)
            } else {
                plan(m, i)
            }
        }
        3 => plan(m, i),
        _ => {
            m.set_u8(u + 0x8b, 0)?;
            plan(m, i)
        }
    }
}
fn plan(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    let u = unit(m, i)?;
    if m.path_searches >= PHASE_BUDGET {
        m.set_u8(u + 0x88, 3)?;
        return Ok(0);
    }
    let v = mover(m, i)?;
    let p = search(m, &v, m.u16(u + 8)?, m.u16(u + 0x7e)?)?;
    m.path_searches += p.expansions.max(SEARCH_MIN);
    if p.cells.is_empty() {
        return settle(m, i);
    }
    // Unreachable goal: walk to the closest reachable cell instead.
    m.set_u16(u + 0x7e, p.end)?;
    write_route(m, i, &p.cells)?;
    m.set_u8(u + 0x88, 5)?;
    follow(m, i)
}
fn follow(m: &mut MemoriaJogo, i: u32) -> Result<u32> {
    let u = unit(m, i)?;
    let r = route(m, i)?;
    let n = m.u16(r)?;
    let cell = m.u16(r.wrapping_add(2 * u32::from(n)))?;
    if frame_movement::call(m, "EnginPeutPasser", &[i, u32::from(cell)])? != 0 {
        m.set_u8(u + 0x8b, 0)?;
        m.set_u8(u + 0x88, 5)?;
        return frame_movement::next_case(m, i);
    }
    blocked(m, i, cell)
}
fn blocked(m: &mut MemoriaJogo, i: u32, cell: u16) -> Result<u32> {
    let u = unit(m, i)?;
    let here = m.u16(u + 8)?;
    let goal = m.u16(u + 0x7e)?;
    let timer = m.u8(u + 0x8b)?;
    let other = blocker(m, i, cell)?;
    if other != u32::from(NONE) {
        let q = unit(m, other)?;
        let ordered = m.u16(q + 0x7e)? != m.u16(q + 8)?;
        if !stationary(m, q)? || (ordered && !waits_for(m, other, i)?) {
            // Traffic: it will move on; keep the route.
            m.set_u8(u + 0x8b, (timer & 0xc0) | TRAFFIC_WAIT)?;
            m.set_u8(u + 0x88, 6)?;
            return Ok(0);
        }
        // Hard block: no order of its own, or a wait cycle back to us
        // (head-on, also across a 2x2 footprint, or through other units).
        if cheb(here, goal) <= 1 {
            // Arrival tolerance: the goal area is held by units that stay put.
            return settle(m, i);
        }
        if !ordered && m.u8(q)? == m.u8(u)? && m.u8(q + 2)? == 7 {
            shove(m, i, other)?;
        }
    }
    let blocks = timer >> 6;
    if blocks >= MAX_BLOCKS {
        return settle(m, i);
    }
    m.set_u8(
        u + 0x8b,
        ((blocks + 1) << 6) | REPLAN_AFTER_WAIT | (4u8 << blocks),
    )?;
    m.set_u8(u + 0x88, 6)?;
    Ok(0)
}
/// The unit in the way of `i` stepping to `cell`, or NONE. A 1x1 unit asks
/// the original ChercheIndiceEngin about the cell. A 2x2 unit checks the four
/// cells of its footprint there (west, north-west and north of the origin
/// too) through the occupant words, skipping itself, so a blocker on any of
/// them is seen and can be waited for or shoved.
fn blocker(m: &mut MemoriaJogo, i: u32, cell: u16) -> Result<u32> {
    let class = frame_path::CLASSES[usize::from(m.u8(unit(m, i)? + 1)?)];
    if class == 1 {
        return frame_movement::call(m, "ChercheIndiceEngin", &[u32::from(cell)]);
    }
    for d in footprint(class) {
        let tag = grid16(m, 0x4e0f38, cell.wrapping_sub(*d))?;
        if tag & 0xc000 == 0x4000 {
            let other = u32::from((tag & 0x3ff8) >> 3);
            if other != i {
                return Ok(other);
            }
        }
    }
    Ok(u32::from(NONE))
}
/// Every other unit standing on the footprint `i` would take at `cell`, in
/// footprint order (tag grid, no game routine called).
fn footprint_occupants(m: &MemoriaJogo, i: u32, cell: u16) -> Result<Vec<u32>> {
    let class = frame_path::CLASSES[usize::from(m.u8(unit(m, i)? + 1)?)];
    let mut found = Vec::new();
    for d in footprint(class) {
        let tag = grid16(m, 0x4e0f38, cell.wrapping_sub(*d))?;
        if tag & 0xc000 == 0x4000 {
            let other = u32::from((tag & 0x3ff8) >> 3);
            if other != i && !found.contains(&other) {
                found.push(other);
            }
        }
    }
    Ok(found)
}
/// Units a stationary unit with an order of its own is waiting for: the
/// occupants of the footprint at its next route cell. A moving unit, one
/// without an order, or one without a route waits for nobody.
fn waiting_on(m: &MemoriaJogo, a: u32) -> Result<Vec<u32>> {
    let q = unit(m, a)?;
    if !stationary(m, q)? || m.u16(q + 0x7e)? == m.u16(q + 8)? {
        return Ok(Vec::new());
    }
    match next_cell(m, a)? {
        NONE => Ok(Vec::new()),
        c => footprint_occupants(m, a, c),
    }
}
/// Units followed at most along a wait chain before giving up the search.
const WAIT_CHAIN: usize = 32;
/// True when `other` (directly or through other waiting units) waits for
/// `i`: a wait cycle that traffic waits would never break. Depth-first in
/// footprint order over at most `WAIT_CHAIN` units; reads only.
fn waits_for(m: &MemoriaJogo, other: u32, i: u32) -> Result<bool> {
    let mut seen = vec![other];
    let mut stack = vec![other];
    while let Some(a) = stack.pop() {
        for b in waiting_on(m, a)? {
            if b == i {
                return Ok(true);
            }
            if !seen.contains(&b) && seen.len() < WAIT_CHAIN {
                seen.push(b);
                stack.push(b);
            }
        }
    }
    Ok(false)
}
/// The next cell of a unit's route, or NONE. A unit whose route was never
/// written (an order still waiting for CheminCalcul) holds the allocation fill
/// as its count (0xa7a7 with the gate fill): any count above the route
/// capacity is no route, never an index past the buffer.
fn next_cell(m: &MemoriaJogo, i: u32) -> Result<u16> {
    let r = route(m, i)?;
    let n = m.u16(r)?;
    Ok(if n == 0 || usize::from(n) > ROUTE_CAPACITY {
        NONE
    } else {
        m.u16(r.wrapping_add(2 * u32::from(n)))?
    })
}
/// Ask an idle same-owner blocker to step to a free neighbour that is neither
/// its own cell, the mover's cell, nor one of the mover's next two cells.
fn shove(m: &mut MemoriaJogo, i: u32, other: u32) -> Result<()> {
    let u = unit(m, i)?;
    let q = unit(m, other)?;
    let at = m.u16(q + 8)?;
    let here = m.u16(u + 8)?;
    let r = route(m, i)?;
    let n = m.u16(r)?;
    let mut next = [NONE; 2];
    for (k, slot) in next.iter_mut().enumerate() {
        if u32::from(n) > k as u32 {
            *slot = m.u16(r.wrapping_add(2 * (u32::from(n) - k as u32)))?;
        }
    }
    for d in DIRECTIONS {
        let c = frame_path::neighbor(m, at, d)?;
        if c == NONE || c == at || c == here || next.contains(&c) {
            continue;
        }
        if frame_movement::call(m, "EnginPeutPasser", &[other, u32::from(c)])? != 0 {
            frame_spawn::order(m, other, c)?;
            return Ok(());
        }
    }
    Ok(())
}

/// Group move order: each unit of the selection that SetDeplaceEnginSelect
/// just sent to `target` gets its own goal. Goal cells are taken in
/// breadth-first order from `target` over free ground cells. 2x2 machines are
/// placed first, each on the first origin whose whole footprint is free and
/// not already given out; then the 1x1 units, in order of (distance to target,
/// unit index), take the first cells left, so with no 2x2 machine the nearest
/// unit keeps the clicked cell.
pub(crate) fn spread(m: &mut MemoriaJogo, player: u32, target: u16) -> Result<()> {
    let base = 0x508728 + (player << 9);
    let mut members = Vec::new();
    for k in 0..u32::from(m.u16(base)?) {
        let i = u32::from(m.u16(base + 2 + 2 * k)?);
        let u = unit(m, i)?;
        if matches!(m.u8(u + 2)?, 0 | 4) || m.u8(u + 0x72)? == 0 {
            continue;
        }
        if u32::from(m.u8(u)?) != player || m.u16(u + 0x7e)? != target || m.u8(u + 2)? != 2 {
            continue;
        }
        let v = mover(m, i)?;
        if v.mask != 1 || members.iter().any(|(_, _, j)| *j == i) {
            continue;
        }
        // 2x2 machines (classes 2/3) sort first.
        members.push((u8::from(v.class == 1), cheb(m.u16(u + 8)?, target), i));
    }
    if members.len() < 2 {
        return Ok(());
    }
    members.sort_unstable();
    let w = u32::from(m.u16(0x497168)?);
    let h = u32::from(m.u16(0x49716a)?);
    if u32::from(target & 255) >= w || u32::from(target >> 8) >= h {
        return Ok(());
    }
    let index = |c: u16| (u32::from(c & 255) + w * u32::from(c >> 8)) as usize;
    let ids: Vec<u32> = members.iter().map(|(_, _, i)| *i).collect();
    let quads = members.iter().filter(|(one, _, _)| *one == 0).count();
    // Free cells in breadth-first order. With 2x2 machines more are collected,
    // since some origins do not fit a whole footprint.
    let wanted = if quads == 0 {
        members.len()
    } else {
        members.len() * 16
    };
    let mut seen = vec![false; (w * h) as usize];
    let mut queue = VecDeque::from([target]);
    seen[index(target)] = true;
    let mut cells = Vec::new();
    while let Some(c) = queue.pop_front() {
        if cells.len() == wanted {
            break;
        }
        let tag = grid16(m, 0x4e0f38, c)?;
        let terrain = mask_cell(m, c, 1)? && tag & 0xc000 != 0x4000;
        if terrain && c != target {
            continue;
        }
        if goal_free(m, c, &ids)? {
            cells.push(c);
        }
        for d in DIRECTIONS {
            let n = frame_path::neighbor(m, c, d)?;
            if n != NONE && !seen[index(n)] {
                seen[index(n)] = true;
                queue.push_back(n);
            }
        }
    }
    let mut taken = vec![false; (w * h) as usize];
    for &(one, _, i) in &members {
        let goal = if one == 0 {
            let mut found = None;
            for &c in &cells {
                if c & 255 == 0 || c >> 8 == 0 {
                    continue;
                }
                let mut fits = true;
                for d in footprint(2) {
                    let f = c.wrapping_sub(*d);
                    if taken[index(f)] || !goal_free(m, f, &ids)? {
                        fits = false;
                        break;
                    }
                }
                if fits {
                    found = Some(c);
                    break;
                }
            }
            let Some(c) = found else {
                continue;
            };
            for d in footprint(2) {
                taken[index(c.wrapping_sub(*d))] = true;
            }
            c
        } else {
            let Some(&c) = cells.iter().find(|c| !taken[index(**c)]) else {
                continue;
            };
            taken[index(c)] = true;
            c
        };
        let u = unit(m, i)?;
        m.set_u16(u + 0x7e, goal)?;
    }
    Ok(())
}
/// A cell a group member may be sent to: ground terrain with no unit, or a
/// group member, or a unit that is moving.
fn goal_free(m: &MemoriaJogo, c: u16, members: &[u32]) -> Result<bool> {
    let tag = grid16(m, 0x4e0f38, c)?;
    if mask_cell(m, c, 1)? && tag & 0xc000 != 0x4000 {
        return Ok(false);
    }
    if tag & 0xc000 != 0x4000 {
        return Ok(true);
    }
    let other = u32::from((tag & 0x3ff8) >> 3);
    Ok(members.contains(&other) || !stationary(m, unit(m, other)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The module never names a random source.
    #[test]
    fn no_random_source() {
        let source = include_str!("modern_path.rs");
        let code = &source[..source.find("#[cfg(test)]").expect("test module")];
        for name in [
            "random(",
            "crt_random",
            "crt_rand",
            "\"Random\"",
            "LongRand",
            "perm(",
            "0x497164",
            "0x4e1d18",
        ] {
            assert!(!code.contains(name), "modern_path uses {name}");
        }
    }

    const W: u16 = 16;
    const UNITS: u32 = 0x0060_0000;
    const ROUTES: u32 = 0x0080_0000;
    const ROUTE_TABLE: u32 = 0x0090_0000;
    const OBJ: u32 = 0x00a0_0000;
    const MASK: u32 = 0x00b0_0000;
    const TAGS: u32 = 0x00c0_0000;
    const ALLIANCE: u32 = 0x00d0_0000;
    const SEED: [u8; 4] = [0x2a, 0, 0, 0];

    /// A 16x16 synthetic world: grids, unit table, route buffers, alliances
    /// and the RNG seeds the module must not touch. Cells in `rocks` block.
    fn world(rocks: &[(u16, u16)]) -> MemoriaJogo {
        let cells = 65536usize;
        let mut m = MemoriaJogo::from_blocks([
            (0x497164, SEED.to_vec()),
            (0x497168, [W as u8, 0, W as u8, 0].to_vec()),
            (0x4e0f36, vec![0; 2]),
            (0x4e0f3c, vec![0; 4]),
            (0x4e1924, ROUTE_TABLE.to_le_bytes().to_vec()),
            (0x4e19a0, UNITS.to_le_bytes().to_vec()),
            (0x4e1d18, vec![7]),
            (0x496304, vec![0xff, 0xff]),
            (0x507d74, OBJ.to_le_bytes().to_vec()),
            (0x507da8, MASK.to_le_bytes().to_vec()),
            (0x4e0f38, TAGS.to_le_bytes().to_vec()),
            (0x5080a8, ALLIANCE.to_le_bytes().to_vec()),
            (UNITS, vec![0; 800 * 276]),
            (ROUTES, vec![0; 800 * 512]),
            (OBJ, vec![0; cells]),
            (MASK, vec![0; cells]),
            (TAGS, vec![0; 2 * cells]),
            (ALLIANCE, vec![0; 100]),
        ])
        .unwrap();
        let mut table = Vec::new();
        for i in 0..800u32 {
            table.extend((ROUTES + 512 * i).to_le_bytes());
        }
        m.declare(ROUTE_TABLE, table).unwrap();
        for (x, y) in rocks {
            let c = u32::from(x + 256 * y);
            m.set_u8(MASK + c, 1).unwrap();
            m.set_u16(TAGS + 2 * c, 1).unwrap();
        }
        m
    }
    fn place(m: &mut MemoriaJogo, i: u32, owner: u8, (x, y): (u16, u16), dest: (u16, u16)) {
        let u = UNITS + 276 * i;
        let c = x + 256 * y;
        m.set_u8(u, owner).unwrap();
        m.set_u8(u + 1, 0).unwrap();
        m.set_u8(u + 2, 2).unwrap();
        m.set_u8(u + 0x72, 1).unwrap();
        m.set_u16(u + 8, c).unwrap();
        m.set_u16(u + 0x7c, c).unwrap();
        m.set_u16(u + 0x7e, dest.0 + 256 * dest.1).unwrap();
        m.set_u8(MASK + u32::from(c), 1).unwrap();
        m.set_u16(TAGS + 2 * u32::from(c), 0x4000 | ((i as u16) << 3))
            .unwrap();
    }
    fn wall() -> Vec<(u16, u16)> {
        // A vertical wall at x = 8 from y = 0 to y = 12, open below.
        (0..13).map(|y| (8, y)).collect()
    }

    #[test]
    fn search_goes_around_and_is_shortest() {
        let mut m = world(&wall());
        place(&mut m, 0, 0, (4, 2), (12, 2));
        let v = mover(&m, 0).unwrap();
        let p = search(&m, &v, 4 + 2 * 256, 12 + 2 * 256).unwrap();
        assert_eq!(p.end, 12 + 2 * 256);
        // Shortest 8-connected detour under the wall end at (8, 13).
        assert_eq!(p.cells.len(), 22);
        assert!(p.cells.contains(&(8 + 13 * 256)));
    }

    #[test]
    fn unreachable_goal_ends_at_closest_reachable_cell() {
        // (12, 2) enclosed by rock: the end is the reachable cell nearest it.
        let mut rocks = Vec::new();
        for x in 10..15 {
            rocks.push((x, 0));
            rocks.push((x, 4));
        }
        for y in 1..4 {
            rocks.push((10, y));
            rocks.push((14, y));
        }
        let mut m = world(&rocks);
        place(&mut m, 0, 0, (2, 2), (12, 2));
        let v = mover(&m, 0).unwrap();
        let p = search(&m, &v, 2 + 2 * 256, 12 + 2 * 256).unwrap();
        assert_eq!(cheb(p.end, 12 + 2 * 256), 3);
        assert_eq!(p.end, 9 + 2 * 256);
    }

    #[test]
    fn planner_and_spread_leave_rng_and_permutation_untouched() {
        let mut m = world(&wall());
        for i in 0..6u32 {
            place(&mut m, i, 0, (2 + i as u16, 6), (12, 6));
        }
        m.declare(0x508728, vec![0; 512]).unwrap();
        m.set_u16(0x508728, 6).unwrap();
        for i in 0..6u32 {
            m.set_u16(0x508728 + 2 + 2 * i, i as u16).unwrap();
        }
        spread(&mut m, 0, 12 + 6 * 256).unwrap();
        let mut goals = Vec::new();
        for i in 0..6u32 {
            goals.push(m.u16(UNITS + 276 * i + 0x7e).unwrap());
            let v = mover(&m, i).unwrap();
            let start = m.u16(UNITS + 276 * i + 8).unwrap();
            let p = search(&m, &v, start, goals[i as usize]).unwrap();
            write_route(&mut m, i, &p.cells).unwrap();
        }
        goals.sort_unstable();
        goals.dedup();
        assert_eq!(goals.len(), 6, "every member has its own goal");
        assert_eq!(m.read(0x497164, 4).unwrap(), SEED);
        assert_eq!(m.u8(0x4e1d18).unwrap(), 7);
    }

    /// A full-height wall at x = 8 with one gate at (8, 6), closed or open as
    /// BatimentNextEtape leaves it (phase 0: bit 0 set and structure tag;
    /// phase 15: bit 0 clear and the region word back).
    fn gated(open: bool) -> MemoriaJogo {
        let rocks: Vec<(u16, u16)> = (0..W).filter(|y| *y != 6).map(|y| (8, y)).collect();
        let mut m = world(&rocks);
        let c = 8 + 6 * 256;
        m.set_u8(OBJ + c, GATE).unwrap();
        if open {
            m.set_u16(TAGS + 2 * c, 1).unwrap();
        } else {
            m.set_u8(MASK + c, 1).unwrap();
            m.set_u16(TAGS + 2 * c, 0x8000 | (3 << 5)).unwrap();
        }
        m
    }

    #[test]
    fn closed_gate_blocks() {
        let mut m = gated(false);
        place(&mut m, 0, 0, (4, 6), (12, 6));
        let v = mover(&m, 0).unwrap();
        let p = search(&m, &v, 4 + 6 * 256, 12 + 6 * 256).unwrap();
        // The wall's west face, 5 cells away; three cells tie at cost 30 and
        // the lowest index wins: (7, 3).
        assert_eq!(p.end, 7 + 3 * 256);
        assert!(!p.cells.contains(&(8 + 6 * 256)));
    }

    #[test]
    fn open_gate_is_crossed() {
        let mut m = gated(true);
        place(&mut m, 0, 0, (4, 6), (12, 6));
        let v = mover(&m, 0).unwrap();
        let p = search(&m, &v, 4 + 6 * 256, 12 + 6 * 256).unwrap();
        assert_eq!(p.end, 12 + 6 * 256);
        assert_eq!(p.cells.len(), 8);
        assert!(p.cells.contains(&(8 + 6 * 256)));
        // An unreachable goal beyond an open gate never ends on the gate.
        let rocks_east: Vec<u16> = (0..W).map(|y| 10 + y * 256).collect();
        for c in rocks_east {
            m.set_u8(MASK + u32::from(c), 1).unwrap();
            m.set_u16(TAGS + 2 * u32::from(c), 1).unwrap();
        }
        let p = search(&m, &v, 4 + 6 * 256, 12 + 6 * 256).unwrap();
        assert_eq!(p.end, 9 + 6 * 256);
    }

    /// A 2x2 machine (kind 14) with its origin at `at`: four tagged cells.
    fn place_quad(m: &mut MemoriaJogo, i: u32, at: (u16, u16), dest: (u16, u16)) {
        place(m, i, 0, at, dest);
        let u = UNITS + 276 * i;
        m.set_u8(u + 1, 14).unwrap();
        let c = at.0 + 256 * at.1;
        for (sub, d) in [0u16, 1, 257, 256].iter().enumerate() {
            let f = u32::from(c - d);
            m.set_u8(MASK + f, 1).unwrap();
            m.set_u16(TAGS + 2 * f, 0x4000 | ((i as u16) << 3) | 4 | sub as u16)
                .unwrap();
        }
    }

    #[test]
    fn spread_gives_a_2x2_machine_a_whole_footprint() {
        let mut m = world(&[]);
        for i in 0..3u32 {
            place(&mut m, i, 0, (2 + i as u16, 12), (8, 6));
        }
        place_quad(&mut m, 3, (6, 13), (8, 6));
        m.declare(0x508728, vec![0; 512]).unwrap();
        m.set_u16(0x508728, 4).unwrap();
        for i in 0..4u32 {
            m.set_u16(0x508728 + 2 + 2 * i, i as u16).unwrap();
        }
        spread(&mut m, 0, 8 + 6 * 256).unwrap();
        let quad = m.u16(UNITS + 276 * 3 + 0x7e).unwrap();
        // The machine is placed first: the clicked cell fits its footprint.
        assert_eq!(quad, 8 + 6 * 256);
        let covered: Vec<u16> = [0u16, 1, 257, 256].iter().map(|d| quad - d).collect();
        let mut goals = Vec::new();
        for i in 0..3u32 {
            let g = m.u16(UNITS + 276 * i + 0x7e).unwrap();
            assert!(!covered.contains(&g), "1x1 goal inside the 2x2 footprint");
            goals.push(g);
        }
        goals.sort_unstable();
        goals.dedup();
        assert_eq!(goals.len(), 3);
    }

    #[test]
    fn a_2x2_machine_sees_a_blocker_on_any_footprint_cell() {
        let mut m = world(&[]);
        place_quad(&mut m, 0, (6, 6), (12, 6));
        // A unit on the north-west cell of the machine's next origin (7, 6).
        place(&mut m, 1, 0, (6, 5), (6, 5));
        assert_eq!(blocker(&mut m, 0, 7 + 6 * 256).unwrap(), 1);
        assert_eq!(blocker(&mut m, 0, 7 + 9 * 256).unwrap(), u32::from(NONE));
    }

    /// MONDE13 worst group case, reduced: a 2x2 machine one step from its
    /// goal and another player's unit each want a cell of the other's
    /// footprint. Neither next cell is the other's origin, so the old head-on
    /// test saw traffic on both sides and they waited forever.
    #[test]
    fn a_wait_cycle_across_a_2x2_footprint_is_a_hard_block() {
        let mut m = world(&[]);
        // Machine: origin (6, 6), footprint (6,6) (5,6) (5,5) (6,5); goal and
        // next origin (5, 5), whose footprint takes (4, 5).
        place_quad(&mut m, 0, (6, 6), (5, 5));
        write_route(&mut m, 0, &[5 + 5 * 256]).unwrap();
        // Player 1's unit on (4, 5), going east through (5, 5).
        place(&mut m, 1, 1, (4, 5), (14, 5));
        write_route(&mut m, 1, &[5 + 5 * 256, 6 + 4 * 256]).unwrap();
        assert_ne!(next_cell(&m, 1).unwrap(), 6 + 6 * 256);
        assert!(waits_for(&m, 1, 0).unwrap());
        assert!(waits_for(&m, 0, 1).unwrap());
        // A third unit queued behind the machine is not part of the cycle.
        place(&mut m, 2, 0, (7, 7), (5, 5));
        write_route(&mut m, 2, &[6 + 6 * 256]).unwrap();
        assert!(!waits_for(&m, 0, 2).unwrap());
        assert!(waits_for(&m, 2, 0).unwrap());
        // The far unit backs off and replans (first hard block, 4 frames).
        blocked(&mut m, 1, 5 + 5 * 256).unwrap();
        assert_eq!(m.u8(UNITS + 276 + 0x88).unwrap(), 6);
        assert_eq!(
            m.u8(UNITS + 276 + 0x8b).unwrap(),
            0x40 | REPLAN_AFTER_WAIT | 4
        );
        // The machine, next to its goal, settles where it stands.
        blocked(&mut m, 0, 5 + 5 * 256).unwrap();
        assert_eq!(m.u16(UNITS + 0x7e).unwrap(), 6 + 6 * 256);
        assert_eq!(m.u8(UNITS + 0x88).unwrap(), 0);
    }

    /// MONDE17 seed 4, frame 907 with the 0xa7 fill: a stationary unit with an
    /// order but no route yet (count 0xa7a7) stood on the blocker's next cell;
    /// the wait-cycle test read its route 85 KB past the buffer and stopped.
    #[test]
    fn an_unwritten_route_count_is_no_route() {
        let mut m = world(&[]);
        place(&mut m, 0, 0, (4, 4), (12, 4));
        write_route(&mut m, 0, &[5 + 4 * 256]).unwrap();
        // The unit on (5, 4) has an order (goal elsewhere) and the fill count.
        place(&mut m, 1, 0, (5, 4), (5, 12));
        m.set_u16(ROUTES + 512, 0xa7a7).unwrap();
        assert_eq!(next_cell(&m, 1).unwrap(), NONE);
        assert!(waiting_on(&m, 1).unwrap().is_empty());
        assert!(!waits_for(&m, 1, 0).unwrap());
        m.set_u16(ROUTES + 512, ROUTE_CAPACITY as u16 + 1).unwrap();
        assert_eq!(next_cell(&m, 1).unwrap(), NONE);
        m.set_u16(ROUTES + 512, ROUTE_CAPACITY as u16).unwrap();
        assert_eq!(next_cell(&m, 1).unwrap(), 0);
    }

    /// An attack approach aims at the target's own (occupied) cell, which is
    /// never an end: the search stops once its fallback is adjacent, with the
    /// plan a full flood would give, instead of expanding the whole region.
    #[test]
    fn an_occupied_goal_stops_the_search_at_the_adjacent_fallback() {
        let mut m = world(&wall());
        place(&mut m, 0, 0, (4, 2), (12, 2));
        place(&mut m, 1, 1, (12, 2), (12, 2));
        let v = mover(&m, 0).unwrap();
        let p = search(&m, &v, 4 + 2 * 256, 12 + 2 * 256).unwrap();
        assert_eq!(cheb(p.end, 12 + 2 * 256), 1);
        // The goal itself costs one step more than the adjacent end (22 cells
        // around the wall end, see search_goes_around_and_is_shortest).
        assert_eq!(p.cells.len(), 21);
        // The flood of the open 16x16 region (243 cells) is cut short.
        assert!(p.expansions < 200, "{} expansions", p.expansions);
        // Exact: the same end and route as when the goal cell is free and the
        // end is forced one cell short of it.
        let full = search(&m, &v, 4 + 2 * 256, p.end).unwrap();
        assert_eq!(full.cells, p.cells);
    }

    /// The planner work budget restarts with every projection; a plan asked
    /// for after it is spent waits for the next phase (state 3) untouched.
    #[test]
    fn a_spent_phase_budget_defers_the_plan() {
        let mut m = world(&wall());
        place(&mut m, 0, 0, (4, 2), (12, 2));
        m.path_searches = PHASE_BUDGET;
        assert_eq!(plan(&mut m, 0).unwrap(), 0);
        assert_eq!(m.u8(UNITS + 0x88).unwrap(), 3);
        assert_eq!(m.u16(ROUTES).unwrap(), 0);
        assert_eq!(m.path_searches, PHASE_BUDGET);
        assert_eq!(SEARCH_MIN * 8, PHASE_BUDGET);
    }

    #[test]
    fn search_is_reproducible() {
        let mut a = world(&wall());
        place(&mut a, 0, 0, (1, 1), (14, 1));
        place(&mut a, 1, 0, (6, 13), (6, 13));
        let b = a.clone();
        let va = mover(&a, 0).unwrap();
        let vb = mover(&b, 0).unwrap();
        let pa = search(&a, &va, 1 + 256, 14 + 256).unwrap();
        let pb = search(&b, &vb, 1 + 256, 14 + 256).unwrap();
        assert_eq!(pa.cells, pb.cells);
        assert_eq!(pa.end, pb.end);
    }
}
