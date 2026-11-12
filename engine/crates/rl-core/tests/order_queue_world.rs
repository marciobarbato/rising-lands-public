//! Shift+click order queue (`RULE_ORDER_QUEUE`) on real mission states, driven
//! by declared player input exactly as a host delivers it: Shift held (VK 0x10),
//! a left press and a release on a map pixel. Receipt for
//! the order-queue receipt in the private research repository.
//!
//! Needs saves made by the engine (the stage-start autosave is enough):
//! * `RL_ORDER_QUEUE_SAVE`: MONDE01 (rule off no-op, 3 units x 3 waypoints,
//!   determinism, save round trip with section 9, a unit dying mid-queue);
//! * `RL_ORDER_QUEUE_SAVE_2X2` (optional): a stage where the local player owns
//!   a 2x2 machine (kinds 11..=15), e.g. MONDE13, for a mixed group.
//!
//! Run: `cargo test -p rl-core --test order_queue_world -- --ignored --nocapture`
use rl_core::{
    player::{DeclaredInput, PlayerEvent},
    save::{decode, encode, ResumeState},
    FrameInput, MemoriaJogo, World, RULE_ORDER_QUEUE,
};
use std::collections::{BTreeMap, VecDeque};

type R<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[path = "support/legacy_save.rs"]
mod legacy;

const VIEW: u32 = 0x508720;
const UNITS: u32 = 0x4e19a0;
const RECORD: u32 = 276;
const SHIFT: u32 = 0x10;
const MK_LBUTTON: u32 = 1;
const MK_SHIFT: u32 = 4;

struct Game {
    world: World,
    input: FrameInput,
    /// Every order packet (player event kind 7) seen: (frame, player, tag, word).
    orders: Vec<(u64, u32, u32, u32)>,
    frames: u64,
}
impl Game {
    fn load(path: &str, rules: u32) -> R<Self> {
        let bytes = std::fs::read(path)?;
        // Engine saves load as they are; schema-1 app saves (before the render
        // receipt) through the test reader, with the clock restarted at tick 0.
        let (mut world, input) = match decode(&bytes) {
            Ok((w, resume)) => (
                World::load_with_rules(w.project()?, w.clock.clone(), rules)?,
                resume.input,
            ),
            Err(_) => {
                let (memory, input) = legacy::load(&bytes)?;
                (
                    World::load_with_rules(memory, rl_core::TickClock::new(20, 0)?, rules)?,
                    input,
                )
            }
        };
        world.set_execution_mode(1)?;
        Ok(Self {
            world,
            input,
            orders: Vec::new(),
            frames: 0,
        })
    }
    fn memory(&self) -> R<MemoriaJogo> {
        Ok(self.world.project()?)
    }
    fn edit(&mut self, f: impl FnOnce(&mut MemoriaJogo) -> R<()>) -> R<()> {
        let mut m = self.world.project()?;
        f(&mut m)?;
        self.world.replace_memory(m)?;
        Ok(())
    }
    fn collect(&mut self, events: &[PlayerEvent]) {
        for e in events.iter().filter(|e| e.kind == 7) {
            self.orders
                .push((self.frames, e.values[0], e.values[1], e.values[2]));
        }
    }
    fn frame(&mut self, phase2: DeclaredInput, phase3: DeclaredInput) -> R<()> {
        let i = self.world.input_step(2, self.input, phase2)?;
        let events = self.world.player_events.clone();
        self.collect(&events);
        self.input = self.world.input_step(3, i, phase3)?;
        let events = self.world.player_events.clone();
        self.collect(&events);
        self.frames += 1;
        Ok(())
    }
    fn idle_frame(&mut self) -> R<()> {
        self.frame(DeclaredInput::default(), DeclaredInput::default())
    }
    fn save(&self) -> R<Vec<u8>> {
        Ok(encode(
            &self.world,
            &ResumeState {
                input: self.input,
                pending: VecDeque::new(),
            },
        )?)
    }
    /// The map pixel of `cell` in the current view, if it is clickable.
    fn pixel(&self, cell: u16) -> R<Option<(i32, i32)>> {
        let m = self.memory()?;
        let (vx, vy, vw, vh) = (
            i32::from(m.u16(VIEW)?),
            i32::from(m.u16(VIEW + 2)?),
            i32::from(m.u16(VIEW + 4)?),
            i32::from(m.u16(VIEW + 6)?),
        );
        let margin = i32::from(m.u8(0x508f90)? as i8) * 32;
        let (x, y) = (
            (i32::from(cell & 255) - vx) * 32 + 16,
            (i32::from(cell >> 8) - vy) * 32 + 16,
        );
        Ok(
            (x >= margin && y >= margin && x < vw * 32 - margin && y < vh * 32 - margin)
                .then_some((x, y)),
        )
    }
    /// A host's Shift+click: frame 1 moves the pointer and presses with Shift
    /// held, frame 2 releases (the order is issued), frame 3 lets Shift go.
    fn shift_click(&mut self, (x, y): (i32, i32), press: bool) -> R<()> {
        let lp = ((y as u32) << 16) | (x as u32 & 0xffff);
        let shift = || {
            let mut keys = BTreeMap::new();
            keys.insert(SHIFT, 0x8000);
            DeclaredInput {
                keys,
                cursor: Some((x, y)),
                ..Default::default()
            }
        };
        let mut down = shift();
        down.messages.push([0x200, MK_SHIFT, lp]);
        if press {
            down.messages.push([0x201, MK_LBUTTON | MK_SHIFT, lp]);
        }
        self.frame(down, shift())?;
        let mut up = shift();
        if press {
            up.messages.push([0x202, MK_SHIFT, lp]);
        }
        self.frame(up, shift())?;
        self.frame(
            DeclaredInput {
                cursor: Some((x, y)),
                ..Default::default()
            },
            DeclaredInput {
                cursor: Some((x, y)),
                ..Default::default()
            },
        )
    }
}

fn unit_base(m: &MemoriaJogo) -> R<u32> {
    Ok(m.u32(UNITS)?)
}
fn rec(m: &MemoriaJogo, u: u16) -> R<u32> {
    Ok(unit_base(m)? + RECORD * u32::from(u))
}
fn kind(m: &MemoriaJogo, u: u16) -> R<u8> {
    Ok(m.u8(rec(m, u)? + 1)?)
}
fn state(m: &MemoriaJogo, u: u16) -> R<u8> {
    Ok(m.u8(rec(m, u)? + 2)?)
}
fn cell(m: &MemoriaJogo, u: u16) -> R<u16> {
    Ok(m.u16(rec(m, u)? + 8)?)
}
fn dest(m: &MemoriaJogo, u: u16) -> R<u16> {
    Ok(m.u16(rec(m, u)? + 0x7e)?)
}
fn big(k: u8) -> bool {
    (11..=15).contains(&k)
}
/// Idle, on-map, own units of the local player, 1x1 land kinds or 2x2 machines.
fn own_idle(m: &MemoriaJogo) -> R<Vec<u16>> {
    let local = m.u8(0x508041)?;
    let mut out = Vec::new();
    for u in 0..800u16 {
        let r = rec(m, u)?;
        let k = m.u8(r + 1)?;
        if m.u8(r)? == local
            && m.u8(r + 2)? == 7
            && m.u8(r + 0x72)? != 0
            && (matches!(k, 0..=2 | 5..=8 | 18..=24) || big(k))
        {
            out.push(u);
        }
    }
    Ok(out)
}
fn cheb(a: u16, b: u16) -> u16 {
    ((a & 255).abs_diff(b & 255)).max((a >> 8).abs_diff(b >> 8))
}
/// No unit or structure occupies the cell (hover grid bits 15/14 clear), so a
/// Shift+click there cannot change the selection.
fn free(m: &MemoriaJogo, c: u16) -> R<bool> {
    let word = m.u16(m.u32(0x4e0f38)?.wrapping_add(2 * u32::from(c)))?;
    Ok(word & 0xc000 == 0)
}

/// Selects `units` as the local player's selection and centres the view on the first.
fn prepare(g: &mut Game, units: &[u16]) -> R<()> {
    g.edit(|m| {
        let p = u32::from(m.u8(0x508041)?);
        let base = 0x508728 + (p << 9);
        m.set_u16(base, units.len() as u16)?;
        for (k, u) in units.iter().enumerate() {
            m.set_u16(base + 2 + 2 * k as u32, *u)?;
        }
        let c = cell(m, units[0])?;
        let (mw, mh) = (m.u16(0x497168)?, m.u16(0x49716a)?);
        let (vw, vh) = (m.u16(VIEW + 4)?, m.u16(VIEW + 6)?);
        let vx = (c & 255).saturating_sub(vw / 2).min(mw.saturating_sub(vw));
        let vy = (c >> 8).saturating_sub(vh / 2).min(mh.saturating_sub(vh));
        m.set_u16(VIEW, vx)?;
        m.set_u16(VIEW + 2, vy)?;
        Ok(())
    })
}
/// Candidate waypoint cells around `origin`, nearest rings first.
fn candidates(origin: u16) -> Vec<u16> {
    let (ox, oy) = (i32::from(origin & 255), i32::from(origin >> 8));
    let mut out = Vec::new();
    for r in [5, 4, 6, 3, 7] {
        for (dx, dy) in [
            (r, 0),
            (r, r),
            (0, r),
            (-r, r),
            (-r, 0),
            (-r, -r),
            (0, -r),
            (r, -r),
        ] {
            let (x, y) = (ox + dx, oy + dy);
            if (0..256).contains(&x) && (0..256).contains(&y) {
                out.push((x + 256 * y) as u16);
            }
        }
    }
    out
}
/// Shift+clicks free, clickable cells until `count` waypoints were accepted
/// (an order 40 was issued for each). Returns the accepted cells.
fn queue_waypoints(g: &mut Game, around: u16, count: usize) -> R<Vec<u16>> {
    let mut accepted = Vec::new();
    for c in candidates(around) {
        if accepted.len() == count {
            break;
        }
        if accepted.iter().any(|&a| cheb(a, c) < 3) || !free(&g.memory()?, c)? {
            continue;
        }
        let Some(px) = g.pixel(c)? else { continue };
        let before = g.orders.len();
        g.shift_click(px, true)?;
        if g.orders[before..]
            .iter()
            .any(|o| o.2 == 40 && o.3 == u32::from(c))
        {
            accepted.push(c);
        }
    }
    Ok(accepted)
}
/// Runs until every unit is idle with no waypoint left (or `limit` frames),
/// recording each unit's destination sequence. Returns the frames used.
fn run(g: &mut Game, units: &[u16], limit: u32, seen: &mut BTreeMap<u16, Vec<u16>>) -> R<u32> {
    for f in 0..limit {
        let m = g.memory()?;
        let mut done = true;
        for &u in units {
            let alive = !matches!(state(&m, u)?, 0 | 4);
            let d = dest(&m, u)?;
            let list = seen.entry(u).or_default();
            if list.last() != Some(&d) {
                list.push(d);
            }
            if alive && (state(&m, u)? != 7 || !g.world.order_queue().cells(u).is_empty()) {
                done = false;
            }
        }
        if done {
            return Ok(f);
        }
        g.idle_frame()?;
    }
    Ok(limit)
}
fn subsequence(needle: &[u16], hay: &[u16]) -> bool {
    let mut it = hay.iter();
    needle.iter().all(|n| it.any(|h| h == n))
}
fn path(var: &str) -> Option<String> {
    std::env::var(var).ok()
}

#[test]
#[ignore = "requires RL_ORDER_QUEUE_SAVE (an engine save of MONDE01)"]
fn rule_off_shift_click_on_ground_is_the_original_no_op() -> R<()> {
    let Some(save) = path("RL_ORDER_QUEUE_SAVE") else {
        eprintln!("RL_ORDER_QUEUE_SAVE unset: skipped");
        return Ok(());
    };
    let mut clicked = Game::load(&save, 0)?;
    let mut held = Game::load(&save, 0)?;
    let units: Vec<u16> = own_idle(&clicked.memory()?)?.into_iter().take(3).collect();
    assert!(!units.is_empty(), "no idle own unit in the save");
    prepare(&mut clicked, &units)?;
    prepare(&mut held, &units)?;
    let m = clicked.memory()?;
    let target = candidates(cell(&m, units[0])?)
        .into_iter()
        .find(|&c| free(&m, c).unwrap_or(false) && clicked.pixel(c).ok().flatten().is_some())
        .expect("a free clickable cell");
    let px = clicked.pixel(target)?.unwrap();
    // The same frames: one world gets the Shift+click, its twin only Shift and the pointer.
    clicked.shift_click(px, true)?;
    held.shift_click(px, false)?;
    for _ in 0..30 {
        clicked.idle_frame()?;
        held.idle_frame()?;
    }
    assert!(
        clicked.orders.iter().all(|o| o.2 != 40),
        "tag 40 issued without the rule"
    );
    assert_eq!(clicked.orders, held.orders, "the click issued an order");
    assert_eq!(
        clicked.memory()?.blocks().collect::<Vec<_>>(),
        held.memory()?.blocks().collect::<Vec<_>>(),
        "a Shift+click on the ground changed game memory"
    );
    assert!(clicked.world.order_queue().is_empty());
    // Under the rule the very same input issues order 40.
    let mut ruled = Game::load(&save, RULE_ORDER_QUEUE)?;
    prepare(&mut ruled, &units)?;
    ruled.shift_click(px, true)?;
    assert!(ruled
        .orders
        .iter()
        .any(|o| o.2 == 40 && o.3 == u32::from(target)));
    println!("rule off: Shift+click at cell {target:#06x} = no order, memory equal to Shift alone after 33 frames");
    Ok(())
}

/// The game after the run, the waypoints, every unit's destinations, the
/// frames until all settled, and the save taken right after the clicks.
type Outcome = (Game, Vec<u16>, BTreeMap<u16, Vec<u16>>, u32, Vec<u8>);

fn scenario(save: &str, units: &[u16], kill: Option<u16>) -> R<Outcome> {
    let mut g = Game::load(save, RULE_ORDER_QUEUE)?;
    prepare(&mut g, units)?;
    let origin = cell(&g.memory()?, units[0])?;
    let waypoints = queue_waypoints(&mut g, origin, 3)?;
    assert_eq!(
        waypoints.len(),
        3,
        "could not place 3 waypoints near {origin:#06x}"
    );
    let mid = g.save()?;
    if let Some(u) = kill {
        g.edit(|m| Ok(rl_core::chain_death::set(m, u)?))?;
    }
    let mut seen = BTreeMap::new();
    let frames = run(&mut g, units, 6000, &mut seen)?;
    Ok((g, waypoints, seen, frames, mid))
}

#[test]
#[ignore = "requires RL_ORDER_QUEUE_SAVE (an engine save of MONDE01)"]
fn three_units_walk_three_waypoints_deterministically() -> R<()> {
    let Some(save) = path("RL_ORDER_QUEUE_SAVE") else {
        eprintln!("RL_ORDER_QUEUE_SAVE unset: skipped");
        return Ok(());
    };
    let base = Game::load(&save, RULE_ORDER_QUEUE)?;
    let m = base.memory()?;
    let units: Vec<u16> = own_idle(&m)?
        .into_iter()
        .filter(|&u| !big(kind(&m, u).unwrap()))
        .take(3)
        .collect();
    assert_eq!(units.len(), 3, "MONDE01 should give 3 idle own units");
    let (a, waypoints, seen, frames, mid) = scenario(&save, &units, None)?;
    assert!(frames < 6000, "units never settled");
    let end = a.memory()?;
    for &u in &units {
        let walked = &seen[&u];
        assert!(
            subsequence(&waypoints, walked),
            "unit {u}: {walked:x?} misses {waypoints:x?}"
        );
        assert_eq!(state(&end, u)?, 7);
        assert!(a.world.order_queue().cells(u).is_empty());
        println!(
            "unit {u} kind {}: destinations {walked:04x?}, ends at {:#06x} ({} from the last waypoint)",
            kind(&end, u)?,
            cell(&end, u)?,
            cheb(cell(&end, u)?, waypoints[2])
        );
    }
    println!(
        "mission {} (local player {}): waypoints {waypoints:04x?}; all idle after {frames} frames",
        end.u8(0x496fe4)?,
        end.u8(0x508041)?
    );

    // The mid-queue save carries section 9 and resumes to the same world.
    assert_eq!(
        &mid[28..32],
        &9u32.to_le_bytes(),
        "mid-queue save without section 9"
    );
    let (loaded, resume) = decode(&mid)?;
    assert!(!loaded.order_queue().is_empty());
    let mut resumed = Game {
        world: loaded,
        input: resume.input,
        orders: Vec::new(),
        frames: 0,
    };
    let mut again = BTreeMap::new();
    run(&mut resumed, &units, 6000, &mut again)?;
    // Twin determinism: the whole scenario again from the same save.
    let (b, waypoints_b, seen_b, frames_b, mid_b) = scenario(&save, &units, None)?;
    assert_eq!(
        (waypoints_b, frames_b, &seen_b),
        (waypoints.clone(), frames, &seen)
    );
    assert_eq!(mid_b, mid, "mid-queue saves differ between two runs");
    assert_eq!(b.save()?, a.save()?, "two runs ended in different worlds");
    assert_eq!(
        resumed.memory()?.blocks().collect::<Vec<_>>(),
        a.memory()?.blocks().collect::<Vec<_>>(),
        "a world resumed from the mid-queue save diverged"
    );
    println!("two runs byte-identical; resumed save equals the uninterrupted run");
    Ok(())
}

#[test]
#[ignore = "requires RL_ORDER_QUEUE_SAVE (an engine save of MONDE01)"]
fn a_unit_dying_mid_queue_drops_its_waypoints() -> R<()> {
    let Some(save) = path("RL_ORDER_QUEUE_SAVE") else {
        eprintln!("RL_ORDER_QUEUE_SAVE unset: skipped");
        return Ok(());
    };
    let m = Game::load(&save, RULE_ORDER_QUEUE)?.memory()?;
    let units: Vec<u16> = own_idle(&m)?
        .into_iter()
        .filter(|&u| !big(kind(&m, u).unwrap()))
        .take(3)
        .collect();
    let victim = units[1];
    let (g, waypoints, seen, frames, _) = scenario(&save, &units, Some(victim))?;
    let end = g.memory()?;
    assert!(
        matches!(state(&end, victim)?, 0 | 4),
        "unit {victim} did not die"
    );
    assert!(
        g.world.order_queue().cells(victim).is_empty(),
        "a dead unit kept its queue"
    );
    assert!(
        !subsequence(&waypoints[1..], &seen[&victim]),
        "the dead unit kept walking"
    );
    for &u in units.iter().filter(|&&u| u != victim) {
        assert!(
            subsequence(&waypoints, &seen[&u]),
            "unit {u} stopped with the victim"
        );
        assert_eq!(state(&end, u)?, 7);
    }
    println!("unit {victim} died after the clicks: queue dropped; the others finished in {frames} frames");
    Ok(())
}

#[test]
#[ignore = "requires RL_ORDER_QUEUE_SAVE_2X2 (a save where the local player owns a 2x2 machine)"]
fn mixed_group_with_a_2x2_machine() -> R<()> {
    let Some(save) = path("RL_ORDER_QUEUE_SAVE_2X2") else {
        eprintln!("RL_ORDER_QUEUE_SAVE_2X2 unset: skipped");
        return Ok(());
    };
    let m = Game::load(&save, RULE_ORDER_QUEUE)?.memory()?;
    let idle = own_idle(&m)?;
    let machine = *idle
        .iter()
        .find(|&&u| big(kind(&m, u).unwrap()))
        .expect("no idle own 2x2 machine");
    let mut units = vec![machine];
    let near: Vec<u16> = {
        let mut small: Vec<u16> = idle
            .iter()
            .copied()
            .filter(|&u| !big(kind(&m, u).unwrap()))
            .collect();
        let mc = cell(&m, machine)?;
        small.sort_by_key(|&u| (cheb(cell(&m, u).unwrap(), mc), u));
        small.into_iter().take(2).collect()
    };
    units.extend(near);
    let (g, waypoints, seen, frames, mid) = scenario(&save, &units, None)?;
    assert_eq!(&mid[28..32], &9u32.to_le_bytes());
    let end = g.memory()?;
    for &u in &units {
        assert!(
            subsequence(&waypoints, &seen[&u]),
            "unit {u}: {:x?} misses {waypoints:x?}",
            seen[&u]
        );
        println!(
            "unit {u} kind {}: destinations {:04x?}, state {}, ends {} from the last waypoint",
            kind(&end, u)?,
            seen[&u],
            state(&end, u)?,
            cheb(cell(&end, u)?, waypoints[2])
        );
    }
    assert!(frames < 6000, "the mixed group never settled");
    println!(
        "mixed group (machine {machine} + {:?}): all idle after {frames} frames",
        &units[1..]
    );
    Ok(())
}
