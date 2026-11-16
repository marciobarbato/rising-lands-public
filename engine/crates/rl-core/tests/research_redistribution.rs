//! "Redistribuir descobertas" on a real stage: every step must leave the world
//! exactly as the original's own path leaves it for the same distribution
//! (RechercheStore of those levels, then RechercheCalculNew 0x47150f at the
//! briefing). Runs when RL_CD_ROOT names the original CD.
use rl_core::{
    mission_boot::StartupInput,
    mission_campaign,
    mission_files::Files,
    mission_research,
    research_redistribution::{pillars, Redistribution, Target, BRIEFING},
    shell_state, MemoriaJogo,
};
use std::path::{Path, PathBuf};

const CURRENT: u32 = 0x5080f4;

/// MONDE07 at its briefing, after a campaign that researched three
/// agriculture entries past the free ones (the original's store-then-
/// recalculate at a mission start).
fn briefing(cd: &Path, agriculture: u8) -> MemoriaJogo {
    let mut m = Files::read(cd)
        .unwrap()
        .stage(
            cd,
            "MONDE07.ETA",
            StartupInput {
                allocation_fill: 0,
                ai_clock_ms: 0x123456,
                thread_id: 1,
            },
        )
        .unwrap();
    with_levels(&mut m, Some(agriculture), None);
    m.set_u8(shell_state::MODE, BRIEFING).unwrap();
    m
}
/// The original's own path to a distribution: store it, recalculate.
fn with_levels(m: &mut MemoriaJogo, agriculture: Option<u8>, military: Option<u8>) {
    let p = u32::from(m.u8(0x508041).unwrap());
    if let Some(v) = agriculture {
        m.set_u8(CURRENT + 16 * p, v).unwrap();
    }
    if let Some(v) = military {
        m.set_u8(CURRENT + 16 * p + 1, v).unwrap();
    }
    mission_campaign::store_research(m).unwrap();
    mission_research::recalculate(m).unwrap();
}
fn blocks(m: &MemoriaJogo) -> Vec<(u32, Vec<u8>)> {
    m.blocks().map(|(a, b)| (a, b.to_vec())).collect()
}

#[test]
fn steps_equal_the_original_path_and_free_points_return() {
    let Some(cd) = std::env::var_os("RL_CD_ROOT").map(PathBuf::from) else {
        eprintln!("RL_CD_ROOT unset: redistribution check skipped");
        return;
    };
    let mut m = briefing(&cd, 5);
    assert_eq!(m.u8(0x496fe4).unwrap(), 7, "MONDE07 is mission 7");
    let start = pillars(&m).unwrap();
    // Agriculture 5 (2 free), military 2 free, civil 7 free, religion 4 free.
    assert_eq!(
        start.map(|p| (p.level, p.minimum)),
        [(5, 2), (2, 2), (7, 7), (4, 4)]
    );
    assert!(start.iter().all(|p| p.maximum >= p.level));

    let mut r = Redistribution::default();
    // Free discoveries cannot go; + needs a free point.
    for target in [
        Target {
            branch: 1,
            delta: -1,
        },
        Target {
            branch: 2,
            delta: -1,
        },
        Target {
            branch: 3,
            delta: -1,
        },
        Target {
            branch: 1,
            delta: 1,
        },
    ] {
        assert!(!r.step(&mut m, target).unwrap(), "{target:?} acted");
    }
    // − agriculture, + military: one discovery moved.
    assert!(r
        .step(
            &mut m,
            Target {
                branch: 0,
                delta: -1
            }
        )
        .unwrap());
    assert_eq!(r.free(&m).unwrap(), 1);
    assert!(r
        .step(
            &mut m,
            Target {
                branch: 1,
                delta: 1
            }
        )
        .unwrap());
    assert_eq!(r.free(&m).unwrap(), 0);

    let mut original = briefing(&cd, 4);
    with_levels(&mut original, None, Some(3));
    assert_eq!(
        blocks(&m),
        blocks(&original),
        "world differs from the original path"
    );

    // A − left unspent returns when the briefing is left.
    assert!(r
        .step(
            &mut m,
            Target {
                branch: 1,
                delta: -1
            }
        )
        .unwrap());
    assert_eq!(r.free(&m).unwrap(), 1);
    r.give_back(&mut m).unwrap();
    assert_eq!(r.free(&m).unwrap(), 0);
    assert_eq!(blocks(&m), blocks(&original), "give_back did not restore");
}
