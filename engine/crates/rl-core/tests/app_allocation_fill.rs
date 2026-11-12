//! The player app creates its world with `allocation_fill = 0`, the memory a
//! fresh Windows `GlobalAlloc(0, 0x35e80)` hands EnginAlloc in practice.
//! No instruction of RISING.EXE writes unit byte `+0xba` (CreerEngin 0x457e6c
//! sets +0xbc..+0xbe only), so a soldier recruited into a never-used slot keeps
//! whatever the allocation held. AffDeplaceEngin 0x413655 blinks the level icon
//! when that byte is non-zero (private research repository issue #4: with the gates' 0xa7 fill every
//! recruit's icon blinked). This checks the whole unit table after MONDE01
//! loads: zero everywhere with fill 0, the fill in free slots with 0xa7.
//! Runs when RL_CD_ROOT names the original CD.
use rl_core::{mission_boot::StartupInput, mission_files::Files};
use std::path::PathBuf;

const UNIT_TABLE: u32 = 0x4e19a0;
const RECORD: u32 = 276;
const SLOTS: u32 = 0x35e80 / RECORD;

fn bytes_ba(fill: u8) -> Option<Vec<u8>> {
    let cd = PathBuf::from(std::env::var_os("RL_CD_ROOT")?);
    let files = Files::read(&cd).unwrap();
    let m = files
        .stage(
            &cd,
            "MONDE01.ETA",
            StartupInput {
                allocation_fill: fill,
                ai_clock_ms: 0x123456,
                thread_id: 1,
            },
        )
        .unwrap();
    let base = m.u32(UNIT_TABLE).unwrap();
    Some(
        (0..SLOTS)
            .map(|i| m.u8(base + i * RECORD + 0xba).unwrap())
            .collect(),
    )
}

#[test]
fn app_fill_zero_leaves_every_unit_highlight_byte_clear() {
    let Some(zero) = bytes_ba(0) else {
        eprintln!("RL_CD_ROOT unset: allocation fill check skipped");
        return;
    };
    assert_eq!(SLOTS, 800);
    assert!(zero.iter().all(|&b| b == 0), "unit +0xba set with fill 0");

    // The gates' deliberate 0xa7 fill: ETA units still read 0, but free slots
    // (where recruits land) carry the fill, which is what made the icon blink.
    let gate = bytes_ba(0xa7).unwrap();
    assert!(gate.iter().all(|&b| b == 0 || b == 0xa7));
    assert!(gate.iter().filter(|&&b| b == 0xa7).count() > 0);
    assert!(gate.contains(&0), "ETA units should keep +0xba = 0");
}
