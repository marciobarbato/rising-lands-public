use super::*;

#[test]
fn campaign_extensions_reject_invalid_handles_without_outputs() {
    let empty = RlShellBytes {
        data: ptr::null(),
        length: 0,
    };
    let mut out = 123;
    let input = FrameInput::default();
    let mut carried = FrameInput::default();
    unsafe {
        assert_eq!(rl_shell_paint_prepared(0, 0, empty, &mut out), 2);
        assert_eq!(
            rl_shell_campaign_prepare(0, 0, empty, ptr::null(), 0, &mut out),
            1
        );
        assert_eq!(
            rl_shell_campaign_load(0, 0, empty, empty, empty, &input, &mut out, &mut carried),
            2
        );
    }
    assert_eq!(out, 123);
    assert_eq!(carried, input);
}

// Real production saves derived from the pinned original result17032; these are
// NOT fresh played victories. Native carry-over spans were independently checked
// at17043. No fixture mode/latch/research/units/RNG bytes are manufactured here.
#[test]
#[ignore = "requires RL_B_RESULT_SAVE, RL_B_BRIEFING_SAVE, RL_B_INPUTS, RL_CD_ROOT, RL_B_API_REPORT"]
fn campaign_extensions_real_result_and_research_choices() {
    use rl_core::{
        mission_files::Presentation, player::DeclaredInput, render_assets::sha256, save,
        shell_picture,
    };
    use std::{fs, path::PathBuf};
    let path = |name| PathBuf::from(std::env::var_os(name).expect(name));
    let digest = |b: &[u8]| {
        sha256(b)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>()
    };
    let source = fs::read(path("RL_B_RESULT_SAVE")).unwrap();
    assert_eq!(
        digest(&source),
        "9ab07b190ad47bebcd5cb942dbd7482a564b6d56c6afacca2128ec5464ad1c70"
    );
    let reference = fs::read(path("RL_B_BRIEFING_SAVE")).unwrap();
    assert_eq!(
        digest(&reference),
        "19c96e46db308997a7a997c9c5e59fd893dcd52e40267a78aa5caf19f7208982"
    );
    let (reference, _) = save::decode(&reference).unwrap();
    let reference = reference.project().unwrap();
    let inputs = path("RL_B_INPUTS");
    let cd = path("RL_CD_ROOT");
    let presentation = Presentation::read(&inputs).unwrap();
    let mut blobs = (0..9)
        .map(|i| fs::read(inputs.join(format!("asset{i}.bin"))).unwrap())
        .collect::<Vec<_>>();
    blobs.push(fs::read(cd.join("INSTALHD/GRAPHICS/MENU.CMP")).unwrap());
    blobs.push(fs::read(cd.join("INSTALHD/GRAPHICS/INVENTE.CMP")).unwrap());
    let desc = |b: &[u8]| RlShellBytes {
        data: b.as_ptr(),
        length: b.len() as u32,
    };
    let descriptors = blobs.iter().map(|b| desc(b)).collect::<Vec<_>>();
    let eta = fs::read(cd.join("WORLD/MONDE02.ETA")).unwrap();
    let monsters = fs::read(cd.join("INSTALHD/MONSTRE.INI")).unwrap();
    let briefing = fs::read(cd.join("WORLD/MONDE02.TXT")).unwrap();
    let mut briefing_pixels = vec![0; PIXELS];
    shell_picture::pie(
        &fs::read(cd.join("INSTALHD/PAGEMIS2.PIE")).unwrap(),
        &mut briefing_pixels,
        800,
    )
    .unwrap();
    let (base, _) = save::decode(&source).unwrap();
    let memory = base.project().unwrap();
    let lzs = fs::read(cd.join("INSTALHD/ECRANFIN.LZS")).unwrap();
    assert_eq!(&lzs[..4], b"LZSS");
    assert_eq!(&lzs[780..784], b"FORM");
    let size = u32::from_le_bytes(lzs[784..788].try_into().unwrap());
    let mut decoded = MemoriaJogo::from_blocks([
        (0x6000000, lzs[788..788 + size as usize].to_vec()),
        (
            shell_picture::RING,
            memory.read(shell_picture::RING, 4096).unwrap().to_vec(),
        ),
        (0x7000000, vec![0; PIXELS]),
    ])
    .unwrap();
    assert_eq!(
        shell_picture::lzss(&mut decoded, 0x6000000, 0x7000000, size).unwrap(),
        PIXELS as u32
    );
    let result_pixels = decoded.read(0x7000000, PIXELS).unwrap();
    let name = rl_core::mission_entry::next_name(&base).unwrap();
    let mut text_path = (0..512)
        .map(|i| memory.u8(0x509594 + i).unwrap())
        .take_while(|&b| b != 0)
        .collect::<Vec<_>>();
    text_path.extend(b"\\WORLD\\MONDE02.TXT");
    let vfs = [
        RlShellFile {
            path: desc(&name),
            bytes: desc(&eta),
        },
        RlShellFile {
            path: desc(&text_path),
            bytes: desc(&briefing),
        },
    ];
    let mut reports = Vec::new();
    // Original MONDE02 has only three selectable branches; zone4 is disabled.
    for (zone, branch) in [(1u32, 0u8), (2, 2), (3, 1)] {
        let (world, resume) = save::decode(&source).unwrap();
        let original = world.project().unwrap();
        let tick = world.clock.tick();
        let source_handle = super::super::mission_abi::publish(world).unwrap();
        let mut handle = 0;
        unsafe {
            assert_eq!(
                rl_shell_create(
                    1,
                    source_handle,
                    descriptors.as_ptr(),
                    11,
                    desc(&presentation.surface),
                    &mut handle
                ),
                0
            );
        }
        assert_eq!(rl_liberar_mundo(source_handle), 0);
        let mut g = 0;
        let mut untouched = 987;
        unsafe {
            assert_eq!(
                rl_shell_campaign_prepare(handle, g, desc(&name), vfs.as_ptr(), 2, &mut untouched),
                1
            );
            assert_eq!(
                rl_shell_paint_prepared(handle, g, desc(result_pixels), &mut g),
                0
            );
        }
        assert_eq!(untouched, 987);
        session(handle, g, |s| {
            assert_eq!(s.memory, original);
            assert_ne!(s.renderer.surface, presentation.surface);
            Ok(())
        })
        .unwrap();
        let mut event = RlShellEvent::default();
        let input = RlShellInput {
            version: 1,
            keys: 1,
            ..Default::default()
        };
        unsafe {
            assert_eq!(rl_shell_input(handle, g, &input, &mut event), 0);
        }
        assert_eq!(event.kind, 4);
        g = event.generation;
        let before = session(handle, g, |s| Ok(s.memory.clone())).unwrap();
        let wrong = b"wrong.eta";
        let wrong_vfs = [RlShellFile {
            path: desc(wrong),
            bytes: desc(&eta),
        }];
        unsafe {
            assert_eq!(
                rl_shell_campaign_prepare(
                    handle,
                    g - 1,
                    desc(&name),
                    vfs.as_ptr(),
                    2,
                    &mut untouched
                ),
                11
            );
            assert_eq!(
                rl_shell_campaign_prepare(
                    handle,
                    g,
                    desc(wrong),
                    wrong_vfs.as_ptr(),
                    1,
                    &mut untouched
                ),
                5
            );
        }
        assert_eq!(untouched, 987);
        session(handle, g, |s| {
            assert_eq!(s.memory, before);
            Ok(())
        })
        .unwrap();
        unsafe {
            assert_eq!(
                rl_shell_campaign_prepare(handle, g, desc(&name), vfs.as_ptr(), 2, &mut g),
                0
            );
        }
        let prepared = session(handle, g, |s| Ok(s.memory.clone())).unwrap();
        assert_eq!(prepared.u8(rl_core::shell_state::MODE).unwrap(), 17);
        for (p, n) in [
            (0x496fe4, 1),
            (0x4e2f78, 1),
            (0x4e19ec, 40),
            (0x508134, 64),
            (0x5080f4, 64),
            (0x50871c, 4),
        ] {
            assert_eq!(
                prepared.read(p, n).unwrap(),
                reference.read(p, n).unwrap(),
                "native carry span {p:x}"
            );
        }
        assert_eq!(
            prepared.read(prepared.u32(0x5080ac).unwrap(), 80).unwrap(),
            reference
                .read(reference.u32(0x5080ac).unwrap(), 80)
                .unwrap()
        );
        unsafe {
            assert_eq!(
                rl_shell_campaign_prepare(handle, g, desc(&name), vfs.as_ptr(), 2, &mut untouched),
                1
            );
            assert_eq!(
                rl_shell_paint_prepared(handle, g, desc(&briefing_pixels), &mut g),
                0
            );
        }
        session(handle, g, |s| {
            assert_eq!(s.memory, prepared);
            Ok(())
        })
        .unwrap();
        let rect = 0x5099b4 + 16 * zone;
        let x = (prepared.u32(rect).unwrap() + prepared.u32(rect + 8).unwrap()) / 2;
        let y = (prepared.u32(rect + 4).unwrap() + prepared.u32(rect + 12).unwrap()) / 2;
        let click = RlShellInput {
            version: 1,
            mouse_x: x,
            mouse_y: y,
            mouse_ae: 1,
            ..Default::default()
        };
        unsafe {
            assert_eq!(rl_shell_input(handle, g, &click, &mut event), 0);
        }
        assert_eq!(event.kind, 0);
        g = event.generation;
        session(handle, g, |s| {
            assert_eq!(s.memory.u8(0x5080f8)?, branch);
            Ok(())
        })
        .unwrap();
        let enter = RlShellInput {
            version: 1,
            keys: 1,
            mouse_x: x,
            mouse_y: y,
            ..Default::default()
        };
        unsafe {
            assert_eq!(rl_shell_input(handle, g, &enter, &mut event), 0);
        }
        assert_eq!(event.kind, 3);
        g = event.generation;
        let confirmed = session(handle, g, |s| Ok(s.memory.clone())).unwrap();
        let mut carried = resume.input;
        carried.cursor_x = x as i32;
        carried.cursor_y = y as i32;
        let mut next_input = FrameInput::default();
        let mut next_world = 987;
        unsafe {
            assert_eq!(
                rl_shell_campaign_load(
                    handle,
                    g,
                    desc(wrong),
                    desc(&eta),
                    desc(&monsters),
                    &carried,
                    &mut next_world,
                    &mut next_input
                ),
                5
            );
            assert_eq!(
                rl_shell_campaign_load(
                    handle,
                    g,
                    desc(&name),
                    desc(&eta[..16]),
                    desc(&monsters),
                    &carried,
                    &mut next_world,
                    &mut next_input
                ),
                5
            );
        }
        assert_eq!(next_world, 987);
        assert_eq!(next_input, FrameInput::default());
        session(handle, g, |s| {
            assert_eq!(s.memory, confirmed);
            Ok(())
        })
        .unwrap();
        unsafe {
            assert_eq!(
                rl_shell_campaign_load(
                    handle,
                    g,
                    desc(&name),
                    desc(&eta),
                    desc(&monsters),
                    &carried,
                    &mut next_world,
                    &mut next_input
                ),
                0
            );
        }
        assert_eq!(next_input, carried);
        let mut world = with_world(next_world, |w| Ok(w.clone())).unwrap();
        assert_eq!(rl_liberar_mundo(next_world), 0);
        assert_eq!(rl_shell_free(handle), 0);
        assert_eq!(world.clock.tick(), tick);
        assert_eq!(world.player_cursor, (x as i32, y as i32));
        assert_eq!(world.render_memory().unwrap().u8(0x496fe4).unwrap(), 2);
        assert_eq!(world.render_memory().unwrap().u8(0x5080f8).unwrap(), branch);
        // Independently spell out the existing native-proven load composition;
        // the new ABI must not run MissionNext/research a second time.
        let mut control_memory = confirmed;
        rl_core::mission_boot::prepare_load_frame(&mut control_memory).unwrap();
        rl_core::mission_load::load(
            &mut control_memory,
            Some(&eta),
            &rl_core::mission_ini::MissionIni::parse(&monsters),
        )
        .unwrap();
        assert_eq!(world.project().unwrap(), control_memory);
        next_input = world
            .load_tail_input_v1(next_input, DeclaredInput::default())
            .unwrap();
        next_input.registers = carried.registers;
        assert_eq!(world.clock.tick(), tick + 1);
        let bytes = save::encode(
            &world,
            &save::ResumeState {
                input: next_input,
                ..Default::default()
            },
        )
        .unwrap();
        let (mut other, loaded) = save::decode(&bytes).unwrap();
        let mut other_input = loaded.input;
        let mut a = presentation.renderer().unwrap();
        let mut b = presentation.renderer().unwrap();
        a.audit = false;
        b.audit = false;
        for _ in 0..600 {
            for (w, i, r) in [
                (&mut world, &mut next_input, &mut a),
                (&mut other, &mut other_input, &mut b),
            ] {
                let draw = r
                    .render(w.render_boundary_memory(true).unwrap().as_ref(), true)
                    .unwrap();
                assert!(draw.stop.is_none());
                w.publish_render_effects(&draw).unwrap();
                *i = w.input_step(2, *i, DeclaredInput::default()).unwrap();
                *i = w.input_step(3, *i, DeclaredInput::default()).unwrap();
            }
            assert_eq!(
                save::encode(
                    &world,
                    &save::ResumeState {
                        input: next_input,
                        ..Default::default()
                    }
                )
                .unwrap(),
                save::encode(
                    &other,
                    &save::ResumeState {
                        input: other_input,
                        ..Default::default()
                    }
                )
                .unwrap()
            );
            assert_eq!(a.surface, b.surface);
        }
        reports.push(format!(
            "{{\"zone\":{zone},\"branch\":{branch},\"future_equal\":600,\"stage\":2}}"
        ));
    }
    fs::write(path("RL_B_API_REPORT"), format!("{{\"scope\":\"Actual result-boundary save, three real research clicks, public ABI request/generation/atomicity checks,7 native-attested carry spans,600 saved future state/pixel pairs per branch; no played victory claim\",\"rows\":[{}]}}\n", reports.join(","))).unwrap();
}

#[test]
#[ignore = "requires RL_B_DEFEAT_SAVE, RL_B_INPUTS, RL_CD_ROOT, RL_B_DEFEAT_REPORT"]
fn defeat_movie_real_result_requires_explicit_host_completion() {
    use rl_core::{mission_files::Presentation, render_assets::sha256, save, shell_picture};
    use std::{fs, path::PathBuf};
    let path = |name| PathBuf::from(std::env::var_os(name).expect(name));
    let digest = |b: &[u8]| {
        sha256(b)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>()
    };
    let source = fs::read(path("RL_B_DEFEAT_SAVE")).unwrap();
    assert_eq!(
        digest(&source),
        "f0f3ba1498429b339057d699c5f63141fb2347cee7429b0c074efea4dd9b0bb6"
    );
    let inputs = path("RL_B_INPUTS");
    let cd = path("RL_CD_ROOT");
    let presentation = Presentation::read(&inputs).unwrap();
    let mut blobs = (0..9)
        .map(|i| fs::read(inputs.join(format!("asset{i}.bin"))).unwrap())
        .collect::<Vec<_>>();
    blobs.push(fs::read(cd.join("INSTALHD/GRAPHICS/MENU.CMP")).unwrap());
    blobs.push(fs::read(cd.join("INSTALHD/GRAPHICS/INVENTE.CMP")).unwrap());
    let desc = |b: &[u8]| RlShellBytes {
        data: b.as_ptr(),
        length: b.len() as u32,
    };
    let descriptors = blobs.iter().map(|b| desc(b)).collect::<Vec<_>>();
    let mut main_pixels = vec![0; PIXELS];
    shell_picture::pie(
        &fs::read(cd.join("INSTALHD/TITRE.PIE")).unwrap(),
        &mut main_pixels,
        800,
    )
    .unwrap();
    let mut reports = Vec::new();
    for completion in [0, 1] {
        let (world, _) = save::decode(&source).unwrap();
        let original = world.project().unwrap();
        assert_eq!(original.u8(rl_core::shell_state::MODE).unwrap(), 19);
        assert_eq!(original.u8(0x4e05a0).unwrap(), 1);
        assert_eq!(original.u8(0x4e1d84).unwrap(), 0);
        let tick = world.clock.tick();
        let source_handle = super::super::mission_abi::publish(world).unwrap();
        let mut handle = 0;
        unsafe {
            assert_eq!(
                rl_shell_create(
                    1,
                    source_handle,
                    descriptors.as_ptr(),
                    11,
                    desc(&presentation.surface),
                    &mut handle
                ),
                0
            );
        }
        assert_eq!(rl_liberar_mundo(source_handle), 0);
        let mut untouched = 987;
        unsafe {
            assert_eq!(
                rl_shell_defeat_finish(handle, 0, completion, &mut untouched),
                1
            );
        }
        assert_eq!(untouched, 987);
        let input = RlShellInput {
            version: 1,
            mouse_x: original.u16(0x5099a8).unwrap() as u32,
            mouse_y: original.u16(0x5099aa).unwrap() as u32,
            mouse_ae: original.u8(0x5099ae).unwrap() as u32,
            mouse_af: original.u8(0x5099af).unwrap() as u32,
            mouse_b0: original.u8(0x5099b0).unwrap() as u32,
            clock_7c: original.u32(0x508f7c).unwrap(),
            animation_84: original.u32(0x508f84).unwrap(),
            ..Default::default()
        };
        let mut event = RlShellEvent::default();
        unsafe {
            assert_eq!(rl_shell_input(handle, 0, &input, &mut event), 0);
        }
        assert_eq!(event.kind, 11);
        let movie_path = &event.path[..event.path_length as usize];
        assert!(movie_path
            .to_ascii_uppercase()
            .ends_with(b"\\EVENTS\\OVER_FR.AVI"));
        let mut g = event.generation;
        let before = session(handle, g, |s| Ok(s.memory.clone())).unwrap();
        assert_eq!(before, original);
        unsafe {
            assert_eq!(rl_shell_defeat_finish(handle, g, 2, &mut untouched), 1);
            assert_eq!(
                rl_shell_defeat_finish(handle, g - 1, completion, &mut untouched),
                11
            );
        }
        assert_eq!(untouched, 987);
        session(handle, g, |s| {
            assert_eq!(s.memory, before);
            Ok(())
        })
        .unwrap();
        // Explicit synthetic negative: an active network must remain an error,
        // even after MenuInit's earlier effects would have changed the candidate.
        session(handle, g, |s| {
            s.memory.set_u32(0x50803c, 1)?;
            Ok(())
        })
        .unwrap();
        let negative = session(handle, g, |s| Ok(s.memory.clone())).unwrap();
        unsafe {
            assert_eq!(
                rl_shell_defeat_finish(handle, g, completion, &mut untouched),
                5
            );
        }
        session(handle, g, |s| {
            assert_eq!(s.memory, negative);
            s.memory = before.clone();
            Ok(())
        })
        .unwrap();
        assert_eq!(untouched, 987);
        let mut expected = before.clone();
        rl_core::campaign_flow::main_menu(&mut expected).unwrap();
        unsafe {
            assert_eq!(rl_shell_defeat_finish(handle, g, completion, &mut g), 0);
        }
        session(handle, g, |s| {
            assert_eq!(s.memory, expected);
            assert_eq!(s.memory.u8(rl_core::shell_state::MODE)?, 2);
            assert_eq!(s.source.clock.tick(), tick);
            assert_eq!(s.request, Request::None);
            Ok(())
        })
        .unwrap();
        unsafe {
            assert_eq!(
                rl_shell_defeat_finish(handle, g, completion, &mut untouched),
                1
            );
            assert_eq!(
                rl_shell_paint_prepared(handle, g, desc(&main_pixels), &mut g),
                0
            );
        }
        assert_eq!(untouched, 987);
        session(handle, g, |s| {
            assert_eq!(s.memory, expected);
            reports.push(format!("{{\"completion\":{completion},\"mode\":2,\"path_hex\":\"{}\",\"pixels_sha256\":\"{}\",\"source_clock\":{tick}}}", movie_path.iter().map(|b| format!("{b:02x}")).collect::<String>(), digest(&s.renderer.surface)));
            Ok(())
        }).unwrap();
        assert_eq!(rl_shell_free(handle), 0);
    }
    fs::write(path("RL_B_DEFEAT_REPORT"), format!("{{\"scope\":\"Actual played D1 result; explicit unavailable and finished host acknowledgements. Finished is a protocol fixture, not real video playback. Menu effects use the separately native-validated routine.\",\"rows\":[{}]}}\n", reports.join(","))).unwrap();
}

#[test]
fn menu_preparation_rejects_invalid_handles_and_arguments_without_outputs() {
    let mut out = 77;
    unsafe {
        assert_eq!(rl_shell_menu_init(0, 0, ptr::null(), 0, &mut out), 2);
        assert_eq!(rl_shell_campaign_reset(0, 0, ptr::null(), 0, &mut out), 2);
        assert_eq!(rl_shell_menu_init(0, 0, ptr::null(), 65, &mut out), 9);
        assert_eq!(rl_shell_menu_init(0, 0, ptr::null(), 0, ptr::null_mut()), 1);
        let mut world = 0;
        let mut input = FrameInput::default();
        let config = crate::mission_abi::RlMissaoInicio {
            version: 1,
            allocation_fill: 0,
            ai_clock_ms: 0,
            thread_id: 1,
            ticks_per_second: 20,
            epoch_ms: 0,
            execution_mode: 0,
            regras: 0,
            iterations: [0; 3],
            input: FrameInput::default(),
        };
        // Ten descriptors are the stage entry; the menu entry takes nine.
        assert_eq!(
            crate::mission_abi::rl_missao_iniciar_menu(
                ptr::null(),
                10,
                &config,
                &mut world,
                &mut input
            ),
            1
        );
        assert_eq!(
            crate::mission_abi::rl_missao_iniciar_menu_eventos_v1(
                ptr::null(),
                9,
                &config,
                ptr::null(),
                &mut world,
                &mut input
            ),
            1
        );
    }
    assert_eq!(out, 77);
}

// Program start -> original main menu -> single player -> new game -> briefing
// -> MONDE01, entirely through the public ABI, compared with the direct stage
// entry of the same assets. Only menu/briefing/research-screen presentation
// state may differ; gameplay bytes, RNG seeds and the stage are identical.
#[test]
#[ignore = "requires RL_B_INPUTS, RL_CD_ROOT, RL_B_MENU_REPORT"]
fn menu_boot_new_game_reaches_monde01_like_direct_entry() {
    use rl_core::{render_assets::sha256, shell_picture};
    use std::{fs, path::PathBuf};
    let path = |name| PathBuf::from(std::env::var_os(name).expect(name));
    let inputs = path("RL_B_INPUTS");
    let cd = path("RL_CD_ROOT");
    let names = [
        "RISING.EXE",
        "RISING.INI",
        "RISING.RUL",
        "RISING.SBL",
        "TABLEAU.COL",
        "COULEUR.MAP",
        "PLATGAME.PIE",
        "MONSTRE.INI",
        "TEXTES.TXT",
    ];
    let mut blobs: Vec<Vec<u8>> = names
        .iter()
        .map(|n| fs::read(cd.join("INSTALHD").join(n)).unwrap())
        .collect();
    let eta = fs::read(cd.join("WORLD/MONDE01.ETA")).unwrap();
    blobs.push(eta.clone());
    let blocks: Vec<RlBloco> = blobs
        .iter()
        .map(|b| RlBloco {
            address: 0,
            length: b.len() as u32,
            data: b.as_ptr(),
        })
        .collect();
    let host_input = rl_core::mission_files::frame_input(&inputs.join("input1.bin")).unwrap();
    let config = crate::mission_abi::RlMissaoInicio {
        version: 1,
        allocation_fill: 0xa7,
        ai_clock_ms: 0x123456,
        thread_id: 1,
        ticks_per_second: 20,
        epoch_ms: 0,
        execution_mode: 0,
        regras: 0,
        iterations: [0; 3],
        input: host_input,
    };
    let (mut menu, mut direct, mut carried) = (0u64, 0u64, FrameInput::default());
    unsafe {
        assert_eq!(
            crate::mission_abi::rl_missao_iniciar_menu(
                blocks.as_ptr(),
                9,
                &config,
                &mut menu,
                &mut carried
            ),
            0,
            "{}",
            last_error()
        );
        let name = b"MONDE01.ETA";
        assert_eq!(
            crate::mission_abi::rl_missao_iniciar(
                blocks.as_ptr(),
                10,
                name.as_ptr(),
                name.len() as u32,
                &config,
                &mut direct,
                &mut carried
            ),
            0
        );
    }
    let menu_memory = with_world(menu, |w| Ok(w.project()?)).unwrap();
    assert_eq!(menu_memory.u8(0x4e2f78).unwrap(), 2);
    assert_eq!(menu_memory.u16(0x4e2f40).unwrap(), 5);
    // Prepared shell over the menu world.
    let mut assets = (0..9)
        .map(|i| fs::read(inputs.join(format!("asset{i}.bin"))).unwrap())
        .collect::<Vec<_>>();
    assets.push(fs::read(cd.join("INSTALHD/GRAPHICS/MENU.CMP")).unwrap());
    assets.push(fs::read(cd.join("INSTALHD/GRAPHICS/INVENTE.CMP")).unwrap());
    let desc = |b: &[u8]| RlShellBytes {
        data: b.as_ptr(),
        length: b.len() as u32,
    };
    let descriptors = assets.iter().map(|b| desc(b)).collect::<Vec<_>>();
    let surface = vec![0u8; PIXELS];
    let mut shell = 0;
    let mut g = 0;
    let pie = |name: &str| {
        let mut pixels = vec![0; PIXELS];
        shell_picture::pie(
            &fs::read(cd.join("INSTALHD").join(name)).unwrap(),
            &mut pixels,
            800,
        )
        .unwrap();
        pixels
    };
    let titre = pie("TITRE.PIE");
    let pagemis2 = pie("PAGEMIS2.PIE");
    let mut event = RlShellEvent::default();
    let mut input = RlShellInput {
        version: 1,
        keys: 1,
        mouse_x: u32::from(menu_memory.u16(0x5099a8).unwrap()),
        mouse_y: u32::from(menu_memory.u16(0x5099aa).unwrap()),
        ..RlShellInput::default()
    };
    let root = (0..256)
        .map(|i| menu_memory.u8(0x509594 + i).unwrap())
        .take_while(|&b| b != 0)
        .collect::<Vec<_>>();
    let mut relative = (0..19)
        .map(|i| menu_memory.u8(0x498430 + i).unwrap())
        .take_while(|&b| b != 0)
        .collect::<Vec<_>>();
    relative[12] = b'0';
    relative[13] = b'1';
    let mut text_path = root.clone();
    text_path.extend(&relative);
    let briefing = fs::read(cd.join("WORLD/MONDE01.TXT")).unwrap();
    let vfs = [RlShellFile {
        path: desc(&text_path),
        bytes: desc(&briefing),
    }];
    let mut stage_name = root.clone();
    stage_name.extend(rl_core::mission_load::mission_name(&menu_memory, 1).unwrap());
    let monsters = fs::read(cd.join("INSTALHD/MONSTRE.INI")).unwrap();
    let mut loaded = 0u64;
    let mut loaded_input = FrameInput::default();
    let mut pixel_hashes = Vec::new();
    unsafe {
        assert_eq!(
            rl_shell_create(
                1,
                menu,
                descriptors.as_ptr(),
                11,
                desc(&surface),
                &mut shell
            ),
            0
        );
        assert_eq!(rl_shell_paint_prepared(shell, g, desc(&titre), &mut g), 0);
        let mut pixels = vec![0u8; PIXELS];
        let mut palette = vec![0u8; 768];
        let mut copy = |shell, g| {
            assert_eq!(
                rl_shell_copy(
                    shell,
                    g,
                    pixels.as_mut_ptr(),
                    PIXELS as u32,
                    palette.as_mut_ptr(),
                    768
                ),
                0
            );
            pixel_hashes.push(
                sha256(&pixels)
                    .iter()
                    .map(|v| format!("{v:02x}"))
                    .collect::<String>(),
            );
        };
        copy(shell, g);
        // Enter on item0 "um jogador" -> SHOW single3, completed by the engine.
        assert_eq!(rl_shell_input(shell, g, &input, &mut event), 0);
        g = event.generation;
        assert_eq!((event.kind, event.mode), (1, 3));
        assert_eq!(rl_shell_campaign_reset(shell, g, ptr::null(), 0, &mut g), 1);
        assert_eq!(rl_shell_menu_init(shell, g, ptr::null(), 0, &mut g), 0);
        assert_eq!(rl_shell_paint_prepared(shell, g, desc(&titre), &mut g), 0);
        copy(shell, g);
        // Enter on item0 "novo jogo" -> RESET_CAMPAIGN 17, completed by the engine.
        input.keys = 1;
        assert_eq!(rl_shell_input(shell, g, &input, &mut event), 0);
        g = event.generation;
        assert_eq!((event.kind, event.mode), (2, 17));
        assert_eq!(rl_shell_menu_init(shell, g, ptr::null(), 0, &mut g), 1);
        assert_eq!(
            rl_shell_campaign_reset(shell, g, vfs.as_ptr(), 1, &mut g),
            0,
            "{}",
            last_error()
        );
        let mut plan = RlShellPlan::default();
        assert_eq!(rl_shell_plan(shell, 17, &mut plan), 0);
        assert_eq!(plan.same_mode, 1);
        assert_eq!(
            rl_shell_paint_prepared(shell, g, desc(&pagemis2), &mut g),
            0
        );
        copy(shell, g);
        // Enter on the briefing -> START_MISSION (mode0), then the real MONDE01 load.
        assert_eq!(rl_shell_input(shell, g, &input, &mut event), 0);
        g = event.generation;
        assert_eq!(event.kind, 3);
        let mut frame_input = carried;
        frame_input.cursor_x = input.mouse_x as i32;
        frame_input.cursor_y = input.mouse_y as i32;
        assert_eq!(
            rl_shell_campaign_load(
                shell,
                g,
                desc(&stage_name),
                desc(&eta),
                desc(&monsters),
                &frame_input,
                &mut loaded,
                &mut loaded_input
            ),
            0,
            "{}",
            last_error()
        );
    }
    let prepared = with_world(menu, |_| Ok(())).is_ok();
    assert!(prepared);
    let via_menu = with_world(loaded, |w| Ok(w.project()?)).unwrap();
    let via_direct = with_world(direct, |w| Ok(w.project()?)).unwrap();
    assert_eq!(via_menu.u8(0x496fe4).unwrap(), 1);
    assert_eq!(
        via_menu.u8(0x4e2f78).unwrap(),
        via_direct.u8(0x4e2f78).unwrap()
    );
    let mut differing = Vec::new();
    for (address, bytes) in via_menu.blocks() {
        let other = via_direct.read(address, bytes.len()).unwrap();
        let mut run: Option<(u32, u32)> = None;
        for (i, (a, b)) in bytes.iter().zip(other).enumerate() {
            let p = address + i as u32;
            if a != b {
                run = Some(run.map_or((p, p), |(s, _)| (s, p)));
            } else if let Some(r) = run.take() {
                differing.push(r);
            }
        }
        if let Some(r) = run {
            differing.push(r);
        }
    }
    let report = serde_free_json(&differing, &pixel_hashes, &via_menu, &via_direct);
    fs::write(path("RL_B_MENU_REPORT"), report).unwrap();
    // Gameplay identity: both RNGs, the unit pool, players, research records,
    // resources and the whole terrain/structure arena.
    for (label, address, n) in [
        ("rng", 0x497164u32, 4usize),
        ("ai_rng", 0x498db4, 4),
        ("players", via_menu.u32(0x5080a8).unwrap(), 100),
        ("saved_players", via_menu.u32(0x5080ac).unwrap(), 100),
        ("research", 0x508134, 64),
        ("stored_research", 0x5080f4, 64),
        ("resources", 0x4e2ea8, 4 * 4 * 8),
        ("units", via_menu.u32(0x4e19a0).unwrap(), 220800),
        ("terrain", via_menu.u32(0x507d70).unwrap(), 131072),
        ("objects", via_menu.u32(0x507d74).unwrap(), 65536),
    ] {
        assert_eq!(
            via_menu.read(address, n).unwrap(),
            via_direct.read(address, n).unwrap(),
            "{label}"
        );
    }
    // The fresh-stage difficulty capability survives the menu path.
    assert_eq!(
        crate::difficulty_abi::rl_aplicar_dificuldade(loaded, 2),
        0,
        "{}",
        last_error()
    );
    assert_eq!(rl_shell_free(shell), 0);
    assert_eq!(rl_liberar_mundo(loaded), 0);
    assert_eq!(rl_liberar_mundo(menu), 0);
    assert_eq!(rl_liberar_mundo(direct), 0);
}
// P4 requests E1-E3 through the public shell ABI on the real program-start menu:
// introduction completion, credits mode10 (original CREDITS.FLC), and the
// one-transaction difficulty load equal to load + rl_aplicar_dificuldade.
#[test]
#[ignore = "requires RL_B_INPUTS, RL_CD_ROOT"]
fn p4_introduction_credits_and_difficulty_load_through_the_shell() {
    use rl_core::shell_picture;
    use std::{fs, path::PathBuf};
    let path = |name| PathBuf::from(std::env::var_os(name).expect(name));
    let inputs = path("RL_B_INPUTS");
    let cd = path("RL_CD_ROOT");
    let names = [
        "RISING.EXE",
        "RISING.INI",
        "RISING.RUL",
        "RISING.SBL",
        "TABLEAU.COL",
        "COULEUR.MAP",
        "PLATGAME.PIE",
        "MONSTRE.INI",
        "TEXTES.TXT",
    ];
    let blobs: Vec<Vec<u8>> = names
        .iter()
        .map(|n| fs::read(cd.join("INSTALHD").join(n)).unwrap())
        .collect();
    let blocks: Vec<RlBloco> = blobs
        .iter()
        .map(|b| RlBloco {
            address: 0,
            length: b.len() as u32,
            data: b.as_ptr(),
        })
        .collect();
    let config = crate::mission_abi::RlMissaoInicio {
        version: 1,
        allocation_fill: 0xa7,
        ai_clock_ms: 0x123456,
        thread_id: 1,
        ticks_per_second: 20,
        epoch_ms: 0,
        execution_mode: 0,
        regras: 0,
        iterations: [0; 3],
        input: rl_core::mission_files::frame_input(&inputs.join("input1.bin")).unwrap(),
    };
    let (mut menu, mut carried) = (0u64, FrameInput::default());
    unsafe {
        assert_eq!(
            crate::mission_abi::rl_missao_iniciar_menu(
                blocks.as_ptr(),
                9,
                &config,
                &mut menu,
                &mut carried
            ),
            0,
            "{}",
            last_error()
        );
    }
    let menu_memory = with_world(menu, |w| Ok(w.project()?)).unwrap();
    let mut assets = (0..9)
        .map(|i| fs::read(inputs.join(format!("asset{i}.bin"))).unwrap())
        .collect::<Vec<_>>();
    assets.push(fs::read(cd.join("INSTALHD/GRAPHICS/MENU.CMP")).unwrap());
    assets.push(fs::read(cd.join("INSTALHD/GRAPHICS/INVENTE.CMP")).unwrap());
    let desc = |b: &[u8]| RlShellBytes {
        data: b.as_ptr(),
        length: b.len() as u32,
    };
    let descriptors = assets.iter().map(|b| desc(b)).collect::<Vec<_>>();
    let surface = vec![0u8; PIXELS];
    let root = (0..256)
        .map(|i| menu_memory.u8(0x509594 + i).unwrap())
        .take_while(|&b| b != 0)
        .collect::<Vec<_>>();
    let mut intro_path = root.clone();
    intro_path.extend(b"\\events\\introfr.avi");
    let mut flc_path = root.clone();
    flc_path.extend(b"\\Events\\CREDITS.FLC");
    let flc = fs::read(cd.join("EVENTS/CREDITS.FLC")).unwrap();
    let flc = &flc[..65536];
    let mut credits_pixels = vec![0; PIXELS];
    shell_picture::pie(
        &fs::read(cd.join("INSTALHD/CREDITS.PIE")).unwrap(),
        &mut credits_pixels,
        800,
    )
    .unwrap();
    let key = |keys: u32| RlShellInput {
        version: 1,
        keys,
        mouse_x: u32::from(menu_memory.u16(0x5099a8).unwrap()),
        mouse_y: u32::from(menu_memory.u16(0x5099aa).unwrap()),
        ..RlShellInput::default()
    };
    let mode = |shell: u64, mode: u32| {
        let mut plan = RlShellPlan::default();
        unsafe { assert_eq!(rl_shell_plan(shell, mode, &mut plan), 0) };
        plan
    };
    let (mut shell, mut g, mut event) = (0u64, 0u64, RlShellEvent::default());
    unsafe {
        assert_eq!(
            rl_shell_create(
                1,
                menu,
                descriptors.as_ptr(),
                11,
                desc(&surface),
                &mut shell
            ),
            0
        );
        // E1: Down x2 -> "Introdução" -> INTRODUCTION with the original CD-root path.
        for _ in 0..2 {
            assert_eq!(rl_shell_input(shell, g, &key(4), &mut event), 0);
            g = event.generation;
            assert_eq!(event.kind, 0);
        }
        assert_eq!(rl_shell_input(shell, g, &key(1), &mut event), 0);
        g = event.generation;
        assert_eq!((event.kind, event.mode), (7, 1));
        assert_eq!(
            &event.path[..event.path_length as usize],
            intro_path.as_slice()
        );
        let mut untouched = 55;
        assert_eq!(
            rl_shell_menu_init(shell, g, ptr::null(), 0, &mut untouched),
            1
        );
        assert_eq!(rl_shell_defeat_finish(shell, g, 2, &mut untouched), 1);
        assert_eq!(untouched, 55);
        assert_eq!(
            rl_shell_defeat_finish(shell, g, 1, &mut g),
            0,
            "{}",
            last_error()
        );
        assert_eq!(mode(shell, 2).same_mode, 1);
        // Main menu again (selection reset): Down x3 -> "Créditos" -> CREDITS.
        let credits = |g: &mut u64, files: &[RlShellFile]| {
            let mut event = RlShellEvent::default();
            for _ in 0..3 {
                assert_eq!(rl_shell_input(shell, *g, &key(4), &mut event), 0);
                *g = event.generation;
            }
            assert_eq!(rl_shell_input(shell, *g, &key(1), &mut event), 0);
            *g = event.generation;
            assert_eq!(event.kind, 8);
            assert_eq!(
                rl_shell_menu_init(shell, *g, files.as_ptr(), files.len() as u32, g),
                0,
                "{}",
                last_error()
            );
            let plan = mode(shell, 10);
            assert_eq!(plan.same_mode, 1);
            assert_eq!(&plan.background[..12], b"\\credits.pie");
            assert_eq!(
                rl_shell_paint_prepared(shell, *g, desc(&credits_pixels), g),
                0
            );
        };
        credits(
            &mut g,
            &[RlShellFile {
                path: desc(&flc_path),
                bytes: desc(flc),
            }],
        );
        let mut out = RlShellCredits::default();
        let mut untouched = 77;
        assert_ne!(rl_shell_draw(shell, g, 0, &mut untouched), 0);
        assert_eq!(
            rl_shell_credits_frame(shell, g, 0, &mut out),
            0,
            "{}",
            last_error()
        );
        g = out.generation;
        assert_eq!(
            (out.state, out.frame, out.frames, out.width, out.height, out.x, out.y),
            (1, 0, 990, 320, 240, 464, 155)
        );
        assert_eq!(rl_shell_credits_frame(shell, g, 71 * 3 + 70, &mut out), 0);
        g = out.generation;
        assert_eq!((out.state, out.frame), (1, 3));
        assert_eq!(rl_shell_credits_frame(shell, g, 71 * 2000, &mut out), 0);
        g = out.generation;
        assert_eq!((out.state, out.frame), (2, 2003));
        // No key, no click: nothing; Escape -> SHOW main, completed by MenuInit(2).
        assert_eq!(rl_shell_input(shell, g, &key(0), &mut event), 0);
        g = event.generation;
        assert_eq!(event.kind, 0);
        assert_eq!(rl_shell_input(shell, g, &key(8), &mut event), 0);
        g = event.generation;
        assert_eq!((event.kind, event.mode), (1, 2));
        assert_eq!(
            rl_shell_menu_init(shell, g, ptr::null(), 0, &mut g),
            0,
            "{}",
            last_error()
        );
        assert_eq!(mode(shell, 2).same_mode, 1);
        // Absent CREDITS.FLC: the first AffCredits ends -> pending SHOW main.
        credits(&mut g, &[]);
        assert_eq!(rl_shell_credits_frame(shell, g, 0, &mut out), 0);
        g = out.generation;
        assert_eq!(out.state, 0);
        assert_eq!(rl_shell_credits_frame(shell, g, 0, &mut out), 1);
        assert_eq!(
            rl_shell_menu_init(shell, g, ptr::null(), 0, &mut g),
            0,
            "{}",
            last_error()
        );
        assert_eq!(mode(shell, 2).same_mode, 1);
        // E3: single player -> new game -> briefing -> START_MISSION.
        assert_eq!(rl_shell_input(shell, g, &key(1), &mut event), 0);
        g = event.generation;
        assert_eq!((event.kind, event.mode), (1, 3));
        assert_eq!(rl_shell_menu_init(shell, g, ptr::null(), 0, &mut g), 0);
        assert_eq!(rl_shell_input(shell, g, &key(1), &mut event), 0);
        g = event.generation;
        assert_eq!((event.kind, event.mode), (2, 17));
        let mut relative = (0..19)
            .map(|i| menu_memory.u8(0x498430 + i).unwrap())
            .take_while(|&b| b != 0)
            .collect::<Vec<_>>();
        relative[12] = b'0';
        relative[13] = b'1';
        let mut text_path = root.clone();
        text_path.extend(&relative);
        let briefing = fs::read(cd.join("WORLD/MONDE01.TXT")).unwrap();
        let vfs = [RlShellFile {
            path: desc(&text_path),
            bytes: desc(&briefing),
        }];
        assert_eq!(
            rl_shell_campaign_reset(shell, g, vfs.as_ptr(), 1, &mut g),
            0,
            "{}",
            last_error()
        );
        let input = key(1);
        assert_eq!(rl_shell_input(shell, g, &input, &mut event), 0);
        g = event.generation;
        assert_eq!(event.kind, 3);
        let mut frame_input = carried;
        frame_input.cursor_x = input.mouse_x as i32;
        frame_input.cursor_y = input.mouse_y as i32;
        let mut stage_name = root.clone();
        stage_name.extend(rl_core::mission_load::mission_name(&menu_memory, 1).unwrap());
        let eta = fs::read(cd.join("WORLD/MONDE01.ETA")).unwrap();
        let monsters = fs::read(cd.join("INSTALHD/MONSTRE.INI")).unwrap();
        let load = |preset: Option<u32>| {
            let (mut world, mut next) = (0u64, FrameInput::default());
            let code = match preset {
                None => rl_shell_campaign_load(
                    shell,
                    g,
                    desc(&stage_name),
                    desc(&eta),
                    desc(&monsters),
                    &frame_input,
                    &mut world,
                    &mut next,
                ),
                Some(p) => rl_shell_campaign_load_dificuldade(
                    shell,
                    g,
                    desc(&stage_name),
                    desc(&eta),
                    desc(&monsters),
                    &frame_input,
                    p,
                    &mut world,
                    &mut next,
                ),
            };
            (code, world, next)
        };
        let (code, plain, plain_input) = load(None);
        assert_eq!(code, 0, "{}", last_error());
        let project = |w: u64| with_world(w, |w| Ok(w.project()?)).unwrap();
        for preset in [0u32, 1, 2] {
            let (code, atomic, atomic_input) = load(Some(preset));
            assert_eq!(code, 0, "{}", last_error());
            assert_eq!(atomic_input, plain_input);
            let (code, separate, _) = load(None);
            assert_eq!(code, 0);
            assert_eq!(
                crate::difficulty_abi::rl_aplicar_dificuldade(separate, preset),
                0,
                "{}",
                last_error()
            );
            assert_eq!(project(atomic), project(separate), "preset {preset}");
            if preset == 1 {
                assert_eq!(project(atomic), project(plain));
            } else {
                assert_ne!(project(atomic), project(plain));
            }
            // The capability is consumed: a second application is the original refusal.
            assert_ne!(
                crate::difficulty_abi::rl_aplicar_dificuldade(atomic, preset),
                0
            );
            assert_eq!(rl_liberar_mundo(atomic), 0);
            assert_eq!(rl_liberar_mundo(separate), 0);
        }
        let mut untouched = 99;
        let mut kept = FrameInput::default();
        assert_eq!(
            rl_shell_campaign_load_dificuldade(
                shell,
                g,
                desc(&stage_name),
                desc(&eta),
                desc(&monsters),
                &frame_input,
                3,
                &mut untouched,
                &mut kept
            ),
            1
        );
        assert_eq!(untouched, 99);
        assert_eq!(rl_liberar_mundo(plain), 0);
    }
    assert_eq!(rl_shell_free(shell), 0);
    assert_eq!(rl_liberar_mundo(menu), 0);
}
fn serde_free_json(
    differing: &[(u32, u32)],
    pixel_hashes: &[String],
    via_menu: &MemoriaJogo,
    via_direct: &MemoriaJogo,
) -> String {
    let ranges = differing
        .iter()
        .map(|(s, e)| {
            format!(
                "{{\"start\":\"{s:#x}\",\"end\":\"{e:#x}\",\"menu\":\"{}\",\"direct\":\"{}\"}}",
                via_menu
                    .read(*s, (*e - *s + 1).min(16) as usize)
                    .unwrap()
                    .iter()
                    .map(|v| format!("{v:02x}"))
                    .collect::<String>(),
                via_direct
                    .read(*s, (*e - *s + 1).min(16) as usize)
                    .unwrap()
                    .iter()
                    .map(|v| format!("{v:02x}"))
                    .collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"differing_ranges\":[{ranges}],\"shell_pixel_sha256\":[{}]}}\n",
        pixel_hashes
            .iter()
            .map(|h| format!("\"{h}\""))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn last_error() -> String {
    super::super::LAST_ERROR.with(|e| e.borrow().clone())
}
