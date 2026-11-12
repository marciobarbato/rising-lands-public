//! rl_regras / rl_regras_definir and the rule set in saves.
use super::*;
use crate::save_abi::{rl_carregar_bytes, rl_salvar_bytes};
use rl_core::FrameInput;

fn create() -> u64 {
    let mut handle = 0;
    assert_eq!(
        unsafe { rl_criar_mundo(ABI_VERSION, 20, 0, &mut handle) },
        0
    );
    let seed = 1234u32.to_le_bytes();
    let block = RlBloco {
        address: rl_core::WORLD_SEED,
        length: 4,
        data: seed.as_ptr(),
    };
    assert_eq!(unsafe { rl_carregar_blocos(handle, &block, 1) }, 0);
    handle
}
fn rules(handle: u64) -> u32 {
    let mut out = 0xa5a5_a5a5;
    assert_eq!(unsafe { rl_regras(handle, &mut out) }, 0);
    out
}
unsafe fn saved(handle: u64) -> Vec<u8> {
    let input = FrameInput::default();
    let mut required = 0;
    assert_eq!(
        unsafe { rl_salvar_bytes(handle, &input, ptr::null_mut(), 0, &mut required) },
        6
    );
    let mut bytes = vec![0; required as usize];
    assert_eq!(
        unsafe { rl_salvar_bytes(handle, &input, bytes.as_mut_ptr(), required, &mut required) },
        0
    );
    bytes
}

#[test]
fn rules_change_before_the_load_tail() {
    let handle = create();
    assert_eq!(rules(handle), 0, "a new world plays the original rules");
    // Unknown bits are refused and leave the world unchanged.
    assert_eq!(rl_regras_definir(handle, 1 << 31), 1);
    assert_eq!(rules(handle), 0);
    assert_eq!(rl_regras_definir(handle, 0), 0);
    assert_eq!(rules(handle), 0);
    assert_eq!(unsafe { rl_regras(handle, ptr::null_mut()) }, 1);
    assert_eq!(rl_liberar_mundo(handle), 0);
}

#[test]
fn rules_are_locked_after_the_load_tail() {
    let handle = create();
    // The load tail is phase 1; any phase marks the world as started.
    with_world(handle, |w| {
        w.frame_phase = 1;
        Ok(())
    })
    .unwrap();
    assert_eq!(rl_regras_definir(handle, 0), 1);
    assert_eq!(rules(handle), 0);
    assert_eq!(rl_liberar_mundo(handle), 0);
}

#[test]
fn original_saves_stay_schema_2() {
    let original = create();
    let a = unsafe { saved(original) };
    let schema = |bytes: &[u8]| u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    assert_eq!(schema(&a), rl_core::save::STATE_SCHEMA);
    let mut loaded = 0;
    let mut input = FrameInput::default();
    assert_eq!(
        unsafe { rl_carregar_bytes(a.as_ptr(), a.len() as u32, &mut loaded, &mut input) },
        0
    );
    assert_eq!(rules(loaded), 0);
    assert_eq!(unsafe { saved(loaded) }, a);
    assert_eq!(rl_liberar_mundo(loaded), 0);
    assert_eq!(rl_liberar_mundo(original), 0);
}

#[test]
fn order_queue_rule_is_set_saved_and_read() {
    let handle = create();
    assert_eq!(rl_regras_definir(handle, rl_core::RULE_ORDER_QUEUE), 0);
    assert_eq!(rules(handle), rl_core::RULE_ORDER_QUEUE);
    let bytes = unsafe { saved(handle) };
    assert_eq!(
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        rl_core::save::RULES_SCHEMA
    );
    let mut loaded = 0;
    let mut input = FrameInput::default();
    assert_eq!(
        unsafe { rl_carregar_bytes(bytes.as_ptr(), bytes.len() as u32, &mut loaded, &mut input) },
        0
    );
    assert_eq!(rules(loaded), rl_core::RULE_ORDER_QUEUE);
    // No unit has waypoints: count 0, nothing written; bad arguments refused.
    let mut cells = [0xa5a5u16; 4];
    let mut count = 99;
    assert_eq!(
        unsafe { rl_fila_ordens(loaded, 3, cells.as_mut_ptr(), 4, &mut count) },
        0
    );
    assert_eq!((count, cells), (0, [0xa5a5; 4]));
    assert_eq!(
        unsafe { rl_fila_ordens(loaded, 3, ptr::null_mut(), 0, &mut count) },
        0
    );
    assert_eq!(
        unsafe { rl_fila_ordens(loaded, 3, ptr::null_mut(), 1, &mut count) },
        1
    );
    assert_eq!(
        unsafe { rl_fila_ordens(loaded, 3, cells.as_mut_ptr(), 4, ptr::null_mut()) },
        1
    );
    assert_eq!(rl_liberar_mundo(loaded), 0);
    assert_eq!(rl_liberar_mundo(handle), 0);
}
