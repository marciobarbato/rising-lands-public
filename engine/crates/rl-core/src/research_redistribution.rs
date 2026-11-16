//! "Redistribuir descobertas": a port-only option (RISING.EXE has none) on the
//! briefing screen (MenuInit 17), where the original shows the four research
//! pillars (`shell_draw::research_block`, 0x468614..0x468821).
//!
//! Each pillar gets a − under its base and a + over its top. − returns the
//! pillar's last discovery as a free point; + spends a free point on the
//! pillar's next discovery, always in the branch's original order. Limits:
//! - a pillar never drops below the discoveries this mission grants by itself
//!   (the leading entries whose APPARITION is at or below the mission, the ones
//!   RechercheCalculNew 0x47150f grants);
//! - a pillar never grows past the entries the mission makes available (it stops
//!   at the first entry with no effect, kind 255, or with MISSION above the
//!   mission: before mission 7 the religious pillar stays empty);
//! - the total never changes. Free points left when the briefing is left
//!   return to the pillars they came from, newest first.
//!
//! The result is always a distribution the original reaches by researching
//! other branches, so worlds and saves keep the original format. Each step
//! writes the local player's stored record 0x508134 (levels plus the current
//! branch choice and progress) and runs the original RechercheCalculNew
//! (`mission_research::recalculate`) again: technology and bonus tables are
//! rebuilt from the levels exactly as at the start of a mission. Because the
//! stored levels then already hold this mission's free discoveries, no
//! completion notification repeats.
use crate::{
    render::{Cache, Context, Result as DrawResult},
    render_assets::Assets,
    MemoriaJogo, Result,
};
use std::collections::BTreeMap;

const TABLE: u32 = 0x50817c;
const CURRENT: u32 = 0x5080f4;
const STORED: u32 = 0x508134;
const MISSION: u32 = 0x496fe4;
const LOCAL: u32 = 0x508041;
/// Briefing mode (MenuInit 17).
pub const BRIEFING: u8 = 17;
/// Branch shown in each screen column, as research_block draws them:
/// agriculture, civil engineering, military, religious.
pub const COLUMN_BRANCH: [u8; 4] = [0, 2, 1, 3];
/// Bar column x (research_block: 442 + 103 * column, 5 px wide) plus 2.
fn centre(column: usize) -> i32 {
    444 + 103 * column as i32
}
/// The + sits over the pillar's top, the − under its base, above the icons
/// (the original zones 1..4 start at y 468).
const PLUS_TOP: i32 = 107;
const MINUS_TOP: i32 = 451;
const HALF: i32 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pillar {
    pub level: u8,
    pub minimum: u8,
    pub maximum: u8,
}

/// What a step or hit test acts on: a branch and +1 or −1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub branch: u8,
    pub delta: i8,
}

/// One briefing's edit: the branch of every − not yet spent by a +, oldest
/// first. A + undoes a − of its own pillar when there is one, otherwise it
/// spends the newest −; what is left returns to its pillars on leaving.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Redistribution {
    removed: Vec<u8>,
}

fn local(m: &MemoriaJogo) -> Result<u32> {
    Ok(u32::from(m.u8(LOCAL)?))
}
fn levels(m: &MemoriaJogo) -> Result<[u8; 4]> {
    let p = local(m)?;
    let mut out = [0; 4];
    for (b, level) in out.iter_mut().enumerate() {
        *level = m.u8(CURRENT + 16 * p + b as u32)?;
    }
    Ok(out)
}
/// The pillar of one branch: its level and the mission's limits.
pub fn pillar(m: &MemoriaJogo, branch: u8) -> Result<Pillar> {
    let mission = m.u8(MISSION)?;
    let (mut minimum, mut maximum, mut granted) = (0u8, 0u8, true);
    for item in 0..30u32 {
        let a = TABLE + 360 * u32::from(branch) + 12 * item;
        if m.u8(a + 6)? == 255 || m.u8(a + 5)? > mission {
            break;
        }
        maximum = item as u8 + 1;
        if granted && m.u8(a + 4)? <= mission {
            minimum = item as u8 + 1;
        } else {
            granted = false;
        }
    }
    let level = levels(m)?[usize::from(branch)];
    Ok(Pillar {
        level,
        minimum,
        maximum,
    })
}
pub fn pillars(m: &MemoriaJogo) -> Result<[Pillar; 4]> {
    Ok([pillar(m, 0)?, pillar(m, 1)?, pillar(m, 2)?, pillar(m, 3)?])
}

impl Redistribution {
    /// Free points: discoveries taken off and not yet spent.
    pub fn free(&self, _m: &MemoriaJogo) -> Result<u32> {
        Ok(self.removed.len() as u32)
    }
    /// Whether this button acts now (shown and clickable).
    pub fn enabled(&self, m: &MemoriaJogo, target: Target) -> Result<bool> {
        if m.u8(crate::shell_state::MODE)? != BRIEFING || target.branch > 3 {
            return Ok(false);
        }
        let p = pillar(m, target.branch)?;
        Ok(match target.delta {
            -1 => p.level > p.minimum,
            1 => p.level < p.maximum && !self.removed.is_empty(),
            _ => false,
        })
    }
    /// The levels after one − or + (None: the button does not act). Updates
    /// the bookkeeping only; `step` also applies them.
    fn plan(&mut self, m: &MemoriaJogo, target: Target) -> Result<Option<[u8; 4]>> {
        if !self.enabled(m, target)? {
            return Ok(None);
        }
        let mut next = levels(m)?;
        let b = usize::from(target.branch);
        if target.delta < 0 {
            next[b] -= 1;
            self.removed.push(target.branch);
        } else {
            next[b] += 1;
            match self.removed.iter().rposition(|&r| r == target.branch) {
                Some(i) => {
                    self.removed.remove(i);
                }
                None => {
                    self.removed.pop();
                }
            }
        }
        Ok(Some(next))
    }
    /// One − or +. Ok(false) when the button does not act (nothing changes).
    pub fn step(&mut self, m: &mut MemoriaJogo, target: Target) -> Result<bool> {
        match self.plan(m, target)? {
            Some(next) => apply(m, next).map(|_| true),
            None => Ok(false),
        }
    }
    /// Leaving the briefing: free points go back to the pillars they came
    /// from, then the edit ends.
    pub fn give_back(&mut self, m: &mut MemoriaJogo) -> Result<()> {
        if let Some(next) = self.returned(m)? {
            apply(m, next)?;
        }
        *self = Self::default();
        Ok(())
    }
    /// The levels after free points return (None: nothing to return).
    fn returned(&self, m: &MemoriaJogo) -> Result<Option<[u8; 4]>> {
        if self.removed.is_empty() {
            return Ok(None);
        }
        let mut next = levels(m)?;
        for &b in &self.removed {
            next[usize::from(b)] += 1;
        }
        Ok(Some(next))
    }
    /// The button under (x, y), if it acts now.
    pub fn target_at(&self, m: &MemoriaJogo, x: i32, y: i32) -> Result<Option<Target>> {
        for (column, &branch) in COLUMN_BRANCH.iter().enumerate() {
            for (delta, top) in [(1i8, PLUS_TOP), (-1, MINUS_TOP)] {
                let c = centre(column);
                let inside = x >= c - HALF - 2
                    && x <= c + HALF + 2
                    && y >= top - 2
                    && y <= top + 2 * HALF + 2;
                let target = Target { branch, delta };
                if inside && self.enabled(m, target)? {
                    return Ok(Some(target));
                }
            }
        }
        Ok(None)
    }
}

/// Writes the local player's stored record (levels, then the current branch
/// choice and progress) and runs RechercheCalculNew again.
fn apply(m: &mut MemoriaJogo, next: [u8; 4]) -> Result<()> {
    let p = local(m)?;
    for offset in 4..16 {
        let v = m.u8(CURRENT + 16 * p + offset)?;
        m.set_u8(STORED + 16 * p + offset, v)?;
    }
    for (b, &level) in next.iter().enumerate() {
        m.set_u8(STORED + 16 * p + b as u32, level)?;
    }
    crate::mission_research::recalculate(m)
}

#[derive(Default)]
struct Scratch {
    background: Option<crate::render::SavedBackground>,
    text: Option<std::collections::VecDeque<crate::render::RecordedTextBoundary>>,
}

/// Draws the enabled buttons and, with free points, a line saying how many,
/// over the host's copy of the briefing picture. `palette` is its 768-byte
/// palette (the darkest and lightest entries outline and fill the signs).
pub fn draw(
    state: &Redistribution,
    m: &MemoriaJogo,
    fonts: &Assets,
    pixels: &mut Vec<u8>,
    palette: &[u8],
    english: bool,
) -> DrawResult<()> {
    if pixels.len() != 480000 || palette.len() != 768 {
        return Err(crate::render::Error::Invalid(
            "redistribution draw needs 800x600 pixels and a 768-byte palette",
        ));
    }
    if m.u8(crate::shell_state::MODE)? != BRIEFING {
        return Ok(());
    }
    let luma = |i: usize| {
        let c = &palette[3 * i..3 * i + 3];
        u32::from(c[0]) * 30 + u32::from(c[1]) * 59 + u32::from(c[2]) * 11
    };
    let dark = (0..256).min_by_key(|&i| luma(i)).unwrap_or(0) as u8;
    let light = (0..256).max_by_key(|&i| luma(i)).unwrap_or(255) as u8;
    let mut scratch = Scratch::default();
    let mut cache = Cache {
        keys: Vec::new(),
        raw: Vec::new(),
        cursor: 0,
    };
    let mut c = Context {
        background: &mut scratch.background,
        text_boundaries: &mut scratch.text,
        text_checked: 0,
        tick: None,
        m,
        overrides: BTreeMap::new(),
        surface: pixels,
        cache: &mut cache,
        assets: fonts,
        observations: Vec::new(),
        audit: false,
        clip: [0, 0, 799, 599],
        camera: 0,
    };
    for (column, &branch) in COLUMN_BRANCH.iter().enumerate() {
        for (delta, top) in [(1i8, PLUS_TOP), (-1, MINUS_TOP)] {
            if !state.enabled(m, Target { branch, delta })? {
                continue;
            }
            let x = centre(column);
            c.filled(x - HALF, top, x + HALF, top + 2 * HALF, dark)?;
            c.filled(x - 5, top + HALF - 1, x + 5, top + HALF + 1, light)?;
            if delta > 0 {
                c.filled(x - 1, top + 3, x + 1, top + 2 * HALF - 3, light)?;
            }
        }
    }
    let free = state.free(m)?;
    if free > 0 {
        let text = if english {
            format!("FREE DISCOVERIES: {free}")
        } else {
            format!("DESCOBERTAS LIVRES: {free}")
        };
        c.afftext(text.as_bytes(), [400, 572, 800, 592], 1, [0, 0])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mission 3; branch 0 has two free entries and four more, branch 1 one
    /// free and two more, branch 2 ends at once, branch 3 opens at mission 7.
    fn world() -> MemoriaJogo {
        let mut table = vec![0u8; 360 * 4];
        let mut entry = |branch: usize, item: usize, appear: u8, mission: u8, kind: u8| {
            let a = 360 * branch + 12 * item;
            table[a..a + 4].copy_from_slice(&1000u32.to_le_bytes());
            table[a + 4] = appear;
            table[a + 5] = mission;
            table[a + 6] = kind;
        };
        for item in 0..30 {
            for branch in 0..4 {
                entry(branch, item, 100, 1, 255);
            }
        }
        entry(0, 0, 1, 1, 0);
        entry(0, 1, 1, 1, 0);
        for item in 2..6 {
            entry(0, item, 100, 1, 1);
        }
        entry(1, 0, 1, 1, 0);
        entry(1, 1, 100, 1, 1);
        entry(1, 2, 100, 1, 1);
        for item in 0..4 {
            entry(3, item, 7, 7, 0);
        }
        let mut current = vec![0u8; 64];
        current[0] = 4;
        current[1] = 2;
        MemoriaJogo::from_blocks([
            (TABLE, table),
            (CURRENT, current),
            (STORED, vec![0u8; 64]),
            (MISSION, vec![3]),
            (LOCAL, vec![0]),
            (crate::shell_state::MODE, vec![BRIEFING]),
        ])
        .unwrap()
    }
    /// The bookkeeping and limits only (the real step also runs the original
    /// RechercheCalculNew; tests/research_redistribution.rs covers that).
    fn step(r: &mut Redistribution, m: &mut MemoriaJogo, branch: u8, delta: i8) -> bool {
        match r.plan(m, Target { branch, delta }).unwrap() {
            Some(next) => {
                for (b, v) in next.iter().enumerate() {
                    m.set_u8(CURRENT + b as u32, *v).unwrap();
                }
                true
            }
            None => false,
        }
    }

    #[test]
    fn limits_follow_the_mission_and_the_free_entries() {
        let m = world();
        assert_eq!(
            pillars(&m).unwrap(),
            [
                Pillar {
                    level: 4,
                    minimum: 2,
                    maximum: 6
                },
                Pillar {
                    level: 2,
                    minimum: 1,
                    maximum: 3
                },
                Pillar {
                    level: 0,
                    minimum: 0,
                    maximum: 0
                },
                Pillar {
                    level: 0,
                    minimum: 0,
                    maximum: 0
                },
            ]
        );
    }

    #[test]
    fn steps_keep_the_total_and_respect_the_limits() {
        let mut m = world();
        let mut r = Redistribution::default();
        // + needs a free point; religion is closed before mission 7.
        assert!(!step(&mut r, &mut m, 1, 1));
        assert!(!step(&mut r, &mut m, 3, -1));
        // Two − on agriculture stop at its two free entries.
        assert!(step(&mut r, &mut m, 0, -1));
        assert!(step(&mut r, &mut m, 0, -1));
        assert!(!step(&mut r, &mut m, 0, -1));
        assert_eq!(r.free(&m).unwrap(), 2);
        // One + on military fills it to the mission's last entry.
        assert!(step(&mut r, &mut m, 1, 1));
        assert!(!step(&mut r, &mut m, 1, 1));
        assert!(!step(&mut r, &mut m, 3, 1));
        assert_eq!(r.free(&m).unwrap(), 1);
        assert_eq!(levels(&m).unwrap(), [2, 3, 0, 0]);
        // The hit test finds only buttons that act.
        let plus = r.target_at(&m, centre(0), PLUS_TOP + HALF).unwrap();
        assert_eq!(
            plus,
            Some(Target {
                branch: 0,
                delta: 1
            })
        );
        assert_eq!(r.target_at(&m, centre(0), MINUS_TOP + HALF).unwrap(), None);
        assert_eq!(r.target_at(&m, centre(3), PLUS_TOP + HALF).unwrap(), None);
        assert_eq!(r.target_at(&m, 10, 10).unwrap(), None);
    }

    #[test]
    fn free_points_return_to_their_pillars_newest_first() {
        let mut m = world();
        let mut r = Redistribution::default();
        assert_eq!(r.returned(&m).unwrap(), None);
        // − agriculture, − military, + agriculture: the + undoes
        // agriculture's own −, so the free point left is military's.
        assert!(step(&mut r, &mut m, 0, -1));
        assert!(step(&mut r, &mut m, 1, -1));
        assert!(step(&mut r, &mut m, 0, 1));
        assert_eq!(levels(&m).unwrap(), [4, 1, 0, 0]);
        assert_eq!(r.returned(&m).unwrap(), Some([4, 2, 0, 0]));
        // − agriculture, + military, − military: the + spent agriculture's
        // −, so the free point left is military's again.
        let mut m = world();
        let mut r = Redistribution::default();
        assert!(step(&mut r, &mut m, 0, -1));
        assert!(step(&mut r, &mut m, 1, 1));
        assert!(step(&mut r, &mut m, 1, -1));
        assert_eq!(levels(&m).unwrap(), [3, 2, 0, 0]);
        assert_eq!(r.returned(&m).unwrap(), Some([3, 3, 0, 0]));
    }

    #[test]
    fn nothing_acts_outside_the_briefing() {
        let mut m = world();
        m.set_u8(crate::shell_state::MODE, 2).unwrap();
        let r = Redistribution::default();
        let target = Target {
            branch: 0,
            delta: -1,
        };
        assert!(!r.enabled(&m, target).unwrap());
    }
}
