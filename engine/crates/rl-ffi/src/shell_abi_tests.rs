use super::*;
#[test]
fn shell_abi_layout_and_requests() {
    assert_eq!(
        (size_of::<RlShellBytes>(), align_of::<RlShellBytes>()),
        (16, 8)
    );
    assert_eq!(size_of::<RlShellFile>(), 32);
    assert_eq!(size_of::<RlShellInput>(), 40);
    assert_eq!(size_of::<RlShellEvent>(), 536);
    assert_eq!(size_of::<RlShellPlan>(), 72);
    let requests = [
        Request::None,
        Request::Show(Screen::Main),
        Request::ResetCampaign { next_mode: 17 },
        Request::StartMission,
        Request::MissionNext,
        Request::ResumeGame,
        Request::SaveLoadMenu { mode: 14 },
        Request::Introduction {
            path: b"\\events\\introfr.avi".to_vec(),
        },
        Request::Credits,
        Request::Multiplayer,
        Request::Quit,
        Request::DefeatMovie {
            path: vec![0x80, 0xff],
        },
        Request::LoadSlot {
            slot: 9,
            from_menu: true,
            path: b"save0.sav".to_vec(),
        },
        Request::SaveSlot {
            slot: 2,
            path: b"save3.sav".to_vec(),
        },
    ];
    for (kind, r) in requests.into_iter().enumerate() {
        let e = event(r, 42).unwrap();
        assert_eq!(
            (e.kind, e.generation, e.reserved),
            (
                kind as u32,
                42,
                [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 9, 2][kind]
            )
        );
        assert_eq!(e.mode, [0, 2, 17, 0, 0, 0, 14, 1, 0, 0, 0, 0, 13, 14][kind]);
        if kind == 7 {
            assert_eq!(&e.path[..e.path_length as usize], b"\\events\\introfr.avi");
        }
        if kind == 11 {
            assert_eq!(&e.path[..e.path_length as usize], &[0x80, 0xff]);
        }
        if kind == 12 {
            assert_eq!(&e.path[..e.path_length as usize], b"save0.sav");
        }
    }
    assert!(event(Request::DefeatMovie { path: vec![1; 513] }, 0).is_err());
}
#[test]
fn shell_abi_bad_handles_bounds_and_atomic_outputs() {
    let mut g = 0xabababab;
    let mut plan = RlShellPlan::default();
    let mut e = RlShellEvent {
        generation: 99,
        ..RlShellEvent::default()
    };
    let i = RlShellInput {
        version: 1,
        ..RlShellInput::default()
    };
    unsafe {
        assert_eq!(rl_shell_version(), 1);
        assert_eq!(rl_shell_plan(0, 2, &mut plan), 2);
        assert_eq!(rl_shell_draw(0, 0, 0, &mut g), 2);
        assert_eq!(rl_shell_export_world(0, 0, &mut g), 2);
        assert_eq!(rl_shell_input(0, 0, &i, &mut e), 2);
        assert_eq!(rl_shell_input(0, 0, ptr::null(), &mut e), 1);
        assert_eq!(
            rl_shell_copy(0, 0, ptr::null_mut(), 0, ptr::null_mut(), 0),
            2
        );
        assert_eq!(
            rl_shell_enter(
                0,
                0,
                2,
                0,
                RlShellBytes {
                    data: ptr::null(),
                    length: 0
                },
                ptr::null(),
                65,
                &mut g
            ),
            9
        );
        assert_eq!(rl_shell_free(0), 2);
    }
    assert_eq!(g, 0xabababab);
    assert_eq!(e.generation, 99);
}
#[test]
fn shell_abi_exact_binary_vfs() {
    let mut f = HostFiles {
        files: BTreeMap::from([(
            b"CD\\WORLD\\MISSION01.TXT".to_vec(),
            vec![0x80, 13, 10, 0x1a],
        )]),
        active: None,
        position: 0,
    };
    assert!(!f.open(b"cd\\WORLD\\MISSION01.TXT", b"rb").unwrap());
    assert!(f.read(10).is_err());
    assert!(f.open(b"CD\\WORLD\\MISSION01.TXT", b"rb").unwrap());
    assert_eq!(f.read(3).unwrap(), [0x80, 13, 10]);
    f.close().unwrap();
    assert!(f.close().is_err());
    assert!(f.open(b"CD\\WORLD\\MISSION01.TXT", b"rt").is_err());
}

#[test]
fn shell_abi_descriptor_validation() {
    let empty = RlShellBytes {
        data: ptr::null(),
        length: 0,
    };
    let path = b"CD\\WORLD\\MONDE01.TXT";
    let file = RlShellFile {
        path: RlShellBytes {
            data: path.as_ptr(),
            length: path.len() as u32,
        },
        bytes: empty,
    };
    unsafe {
        assert!(bytes(empty, 1).unwrap().is_empty());
        assert!(bytes(
            RlShellBytes {
                data: ptr::null(),
                length: 1
            },
            1
        )
        .is_err());
        assert!(bytes(
            RlShellBytes {
                data: ptr::null(),
                length: u32::MAX
            },
            LIMIT
        )
        .is_err());
        assert!(files(ptr::null(), 1).is_err());
        assert!(files(ptr::null(), 65).is_err());
        assert!(files([file, file].as_ptr(), 2).is_err());
        let mut f = files(&file, 1).unwrap();
        assert!(f.open(path, b"rb").unwrap());
        assert!(f.read(16000).unwrap().is_empty());
        f.close().unwrap();
    }
    assert!(disjoint(&[(10, 20), (19, 30)]).is_err());
    assert!(disjoint(&[(10, 20), (20, 30)]).is_ok());
    let mut m = MemoriaJogo::from_blocks([(0x498430, vec![0; 19])]).unwrap();
    assert!(validate_briefing_template(&m).is_err());
    m.write(0x498430, b"\\WORLD\\MONDE01.TXT\0").unwrap();
    assert!(validate_briefing_template(&m).is_ok());
}

// Original-backed inputs exported by the dependency's existing fixture exporter.
// Explicitly ignored in the media-independent gate, run by shell_abi_validate.sh.
#[test]
#[ignore = "requires RL_SHELL_ABI_FIXTURES, RL_CD_ROOT and RL_SHELL_ABI_OUTPUT"]
fn shell_abi_original_prepared_equivalence() {
    use rl_core::{render_assets::sha256, shell_picture, text, TickClock, THREAD_DATA, WORLD_SEED};
    use std::{fs, path::PathBuf};
    let fixtures = PathBuf::from(std::env::var_os("RL_SHELL_ABI_FIXTURES").expect("fixture path"));
    let cd = PathBuf::from(std::env::var_os("RL_CD_ROOT").expect("media path"));
    let out = PathBuf::from(std::env::var_os("RL_SHELL_ABI_OUTPUT").expect("output path"));
    let input = fs::read(fixtures.join("s0.bin")).unwrap();
    let hash: String = sha256(&input).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hash,
        "776dfa2e5c083ac6262218fd3992fbfd25996d9f938218bb761dc24d9c4b9281"
    );
    let mut offset = 0;
    let mut word = || {
        let n = u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap());
        offset += 4;
        n
    };
    let count = word();
    let mut offset = 4;
    let mut original = MemoriaJogo::default();
    for _ in 0..count {
        let address = u32::from_le_bytes(input[offset..offset + 4].try_into().unwrap());
        let size = u32::from_le_bytes(input[offset + 4..offset + 8].try_into().unwrap()) as usize;
        offset += 8;
        original
            .declare(address, input[offset..offset + size].to_vec())
            .unwrap();
        offset += size;
    }
    assert_eq!(offset, input.len());
    let text_bytes = fs::read(cd.join("INSTALHD/TEXTES.TXT")).unwrap();
    text::init_langue(&mut original, &text_bytes, 0).unwrap();
    let mut blobs = (0..9)
        .map(|i| fs::read(fixtures.join(format!("assets/asset{i}.bin"))).unwrap())
        .collect::<Vec<_>>();
    blobs.push(fs::read(fixtures.join("assets/MENU.CMP")).unwrap());
    blobs.push(fs::read(fixtures.join("assets/INVENTE.CMP")).unwrap());
    let descriptor = |b: &[u8]| RlShellBytes {
        data: b.as_ptr(),
        length: b.len() as u32,
    };
    let descriptors: Vec<_> = blobs.iter().map(|v| descriptor(v)).collect();
    let assets =
        Arc::new(Assets::load(&blobs[..9].iter().map(Vec::as_slice).collect::<Vec<_>>()).unwrap());
    let mut backgrounds = BTreeMap::new();
    for name in ["TITRE.PIE", "PAGEMIS2.PIE", "PAGEMIS.PIE", "ECRANFIN.LZS"] {
        let bytes = fs::read(cd.join("INSTALHD").join(name)).unwrap();
        let mut pixels = vec![0; PIXELS];
        if name.ends_with(".PIE") {
            shell_picture::pie(&bytes, &mut pixels, 800).unwrap();
        } else {
            assert_eq!(&bytes[..4], b"LZSS");
            assert_eq!(&bytes[780..784], b"FORM");
            let size = u32::from_le_bytes(bytes[784..788].try_into().unwrap()) as usize;
            let mut m = MemoriaJogo::from_blocks([
                (0x6000000, bytes[788..788 + size].to_vec()),
                (
                    shell_picture::RING,
                    original.read(shell_picture::RING, 4096).unwrap().to_vec(),
                ),
                (0x7000000, pixels),
            ])
            .unwrap();
            assert_eq!(
                shell_picture::lzss(&mut m, 0x6000000, 0x7000000, size as u32).unwrap(),
                PIXELS as u32
            );
            pixels = m.read(0x7000000, PIXELS).unwrap().to_vec();
        }
        fs::write(out.join(format!("{name}.pixels")), &pixels).unwrap();
        backgrounds.insert(name, pixels);
    }
    fn cstring(m: &MemoriaJogo, address: u32) -> Vec<u8> {
        (0..512)
            .map(|i| m.u8(address + i).unwrap())
            .take_while(|&b| b != 0)
            .collect()
    }
    struct DiskFiles {
        root: PathBuf,
        active: Option<Vec<u8>>,
        opens: usize,
        missing_cd: bool,
    }
    impl Files for DiskFiles {
        fn open(&mut self, path: &[u8], mode: &[u8]) -> rl_core::Result<bool> {
            assert_eq!(mode, b"rb");
            self.opens += 1;
            if self.missing_cd && self.opens == 1 {
                return Ok(false);
            }
            let name = path.rsplit(|&b| b == b'\\' || b == b'/').next().unwrap();
            let name = String::from_utf8(name.to_ascii_uppercase()).unwrap();
            self.active = Some(std::fs::read(self.root.join("WORLD").join(name)).unwrap());
            Ok(true)
        }
        fn read(&mut self, max: usize) -> rl_core::Result<Vec<u8>> {
            let b = self.active.as_ref().unwrap();
            Ok(b[..b.len().min(max)].to_vec())
        }
        fn close(&mut self) -> rl_core::Result<()> {
            assert!(self.active.take().is_some());
            Ok(())
        }
    }
    let mut checked_frames = 0;
    let mut checked_inputs = 0;
    let mut checked_entries = 0;
    let mut seen_requests = std::collections::BTreeSet::new();
    // Includes pre/post mission7 picture choice, both original result latches,
    // and all four research animation branches. This is actual prepared S0 data;
    // no campaign reset/research preparer is claimed as executed by this fixture.
    let cases = [
        (2, 1, 1),
        (3, 1, 1),
        (16, 1, 1),
        (17, 1, 1),
        (17, 7, 1),
        (19, 1, 1),
        (19, 1, 0),
        (23, 1, 1),
    ];
    for (mode, mission, latch) in cases {
        let mut m = original.clone();
        m.set_u8(rl_core::shell_state::MODE, 18).unwrap();
        m.set_u8(0x496fe4, mission).unwrap();
        m.set_u8(0x4e1d84, latch).unwrap();
        for p in [0x5099a8, 0x5099aa] {
            m.set_u16(p, 0).unwrap();
        }
        for p in [0x5099ae, 0x5099af, 0x5099b0] {
            m.set_u8(p, 0).unwrap();
        }
        let crt = m.u32(THREAD_DATA).unwrap() + 12;
        let rng = (
            m.u32(WORLD_SEED).unwrap(),
            m.u32(crt).unwrap(),
            m.u8(0x4e1d18).unwrap(),
        );
        let mut source = World::load(m.clone(), TickClock::new(20, 0).unwrap()).unwrap();
        source.frame_phase = 3;
        source.set_execution_mode(1).unwrap();
        let world = with_registry(|r| {
            let h = r.next;
            r.next += 1;
            r.worlds.insert(h, source);
            Ok(h)
        })
        .unwrap();
        let title = &backgrounds["TITRE.PIE"];
        let mut handle = 0;
        unsafe {
            assert_eq!(
                rl_shell_create(
                    1,
                    world,
                    descriptors.as_ptr(),
                    11,
                    descriptor(title),
                    &mut handle
                ),
                0
            );
        }
        let mut plan = RlShellPlan::default();
        unsafe {
            assert_eq!(rl_shell_plan(handle, mode, &mut plan), 0);
        }
        let screen = screen(mode).unwrap();
        assert_eq!(
            plan.required_count as usize,
            screen.required_game_calls().len()
        );
        for (a, b) in plan.required_calls.iter().zip(screen.required_game_calls()) {
            assert_eq!(*a, b.address);
        }
        assert_eq!(
            &plan.background[..screen.background(mission).len()],
            screen.background(mission)
        );
        assert_eq!(rl_liberar_mundo(world), 0); // session survives source free
        let mut r = Renderer::new(
            Arc::clone(&assets),
            title,
            Cache {
                keys: vec![0; 1024],
                raw: vec![0; 270336],
                cursor: 0,
            },
        )
        .unwrap();
        let banks = Banks::load(&blobs[9], &blobs[10], &r).unwrap();
        let name = String::from_utf8(screen.background(mission)[1..].to_ascii_uppercase()).unwrap();
        let background = &backgrounds[name.as_str()];
        let mut relative = cstring(&m, 0x498430);
        relative[12] = b'0' + mission / 10;
        relative[13] = b'0' + mission % 10;
        let mut cd_path = cstring(&m, 0x509594);
        cd_path.extend_from_slice(&relative);
        let mut installed_path = cstring(&m, 0x509494);
        installed_path.extend_from_slice(&relative);
        let briefing = fs::read(cd.join("WORLD").join(format!("MONDE{mission:02}.TXT"))).unwrap();
        let file = RlShellFile {
            path: descriptor(if mission == 7 {
                &installed_path
            } else {
                &cd_path
            }),
            bytes: descriptor(&briefing),
        };
        let mut files = DiskFiles {
            root: cd.clone(),
            active: None,
            opens: 0,
            missing_cd: mission == 7,
        };
        let mut g = 0;
        if plan.required_mask != 0 {
            let mut untouched = 99;
            unsafe {
                assert_eq!(
                    rl_shell_enter(
                        handle,
                        g,
                        mode,
                        0,
                        descriptor(background),
                        &file,
                        1,
                        &mut untouched
                    ),
                    5
                );
            }
            assert_eq!(untouched, 99);
        }
        unsafe {
            let mut untouched = 99;
            assert_eq!(
                rl_shell_enter(
                    handle,
                    g,
                    mode,
                    plan.required_mask,
                    descriptor(&[1; 7]),
                    &file,
                    1,
                    &mut untouched
                ),
                5
            );
            assert_eq!(untouched, 99);
            session(handle, g, |s| {
                assert_eq!(s.memory, m);
                assert_eq!(s.renderer.surface, r.surface);
                Ok(())
            })
            .unwrap();
            assert_eq!(
                rl_shell_enter(
                    handle,
                    g,
                    mode,
                    plan.required_mask,
                    descriptor(background),
                    &file,
                    1,
                    &mut g
                ),
                0
            );
        }
        shell::enter_prepared(&mut m, &mut r, &banks, screen, background, &mut files).unwrap();
        checked_entries += 1;
        if matches!(mode, 17 | 23) {
            assert_eq!(files.opens, 2);
        }
        let compare = |m: &MemoriaJogo, r: &Renderer, g| {
            session(handle, g, |s| {
                assert_eq!(&s.memory, m);
                assert_eq!(s.renderer.surface, r.surface);
                assert_eq!(s.renderer.assets.palette, r.assets.palette);
                assert_eq!(
                    (
                        s.memory.u32(WORLD_SEED)?,
                        s.memory.u32(crt)?,
                        s.memory.u8(0x4e1d18)?
                    ),
                    rng
                );
                Ok(())
            })
            .unwrap();
            let mut p = vec![0; PIXELS];
            let mut palette = [0; 768];
            unsafe {
                assert_eq!(
                    rl_shell_copy(
                        handle,
                        g,
                        p.as_mut_ptr(),
                        PIXELS as u32,
                        palette.as_mut_ptr(),
                        768
                    ),
                    0
                );
            }
            assert_eq!(p, r.surface);
            assert_eq!(palette, r.assets.palette);
        };
        compare(&m, &r, g);
        // Error atomicity and generation checks on a real, live session.
        unsafe {
            let mut p = vec![0x77; PIXELS];
            let mut palette = [0x66; 768];
            let mut untouched = 99;
            assert_eq!(
                rl_shell_copy(
                    handle,
                    g,
                    p.as_mut_ptr(),
                    (PIXELS - 1) as u32,
                    palette.as_mut_ptr(),
                    768
                ),
                6
            );
            assert_eq!(p, vec![0x77; PIXELS]);
            assert_eq!(palette, [0x66; 768]);
            assert_eq!(
                rl_shell_copy(
                    handle,
                    g,
                    p.as_mut_ptr(),
                    PIXELS as u32,
                    p.as_mut_ptr(),
                    768
                ),
                1
            );
            assert_eq!(rl_shell_draw(handle, g - 1, 1, &mut untouched), 11);
            assert_eq!(untouched, 99);
            assert_eq!(
                rl_shell_enter(
                    handle,
                    g,
                    mode,
                    0,
                    descriptor(&[]),
                    ptr::null(),
                    0,
                    &mut untouched
                ),
                0
            );
            assert_eq!(untouched, g);
            let i = RlShellInput {
                version: 1,
                mouse_x: 65536,
                ..RlShellInput::default()
            };
            let mut e = RlShellEvent {
                generation: 99,
                ..RlShellEvent::default()
            };
            assert_eq!(rl_shell_input(handle, g, &i, &mut e), 1);
            assert_eq!(e.generation, 99);
        }
        compare(&m, &r, g);
        let apply = |i: RlShellInput, m: &mut MemoriaJogo, g: &mut u64| {
            m.set_u16(0x5099a8, i.mouse_x as u16).unwrap();
            m.set_u16(0x5099aa, i.mouse_y as u16).unwrap();
            m.set_u8(0x5099ae, i.mouse_ae as u8).unwrap();
            m.set_u8(0x5099af, i.mouse_af as u8).unwrap();
            m.set_u8(0x5099b0, i.mouse_b0 as u8).unwrap();
            m.set_u32(0x508f7c, i.clock_7c).unwrap();
            m.set_u32(0x508f84, i.animation_84).unwrap();
            let request = shell::input(
                m,
                Keys {
                    enter: i.keys & 1 != 0,
                    up: i.keys & 2 != 0,
                    down: i.keys & 4 != 0,
                    escape: i.keys & 8 != 0,
                },
            )
            .unwrap();
            let expected = event(request, *g + 1).unwrap();
            let mut actual = RlShellEvent::default();
            unsafe {
                assert_eq!(rl_shell_input(handle, *g, &i, &mut actual), 0);
            }
            assert_eq!(actual, expected);
            *g = actual.generation;
            actual.kind
        };
        // All reached MENU phases and four INVENTE branches, then original requests.
        for branch in 0..if mode == 17 {
            if mission >= 7 {
                4
            } else {
                3
            }
        } else {
            1
        } {
            if mode == 17 {
                // Research hit-zone order0/2/1/3; use actual native rectangles.
                let x = [430, 650, 530, 750][branch];
                let i = RlShellInput {
                    version: 1,
                    mouse_x: x,
                    mouse_y: 510,
                    mouse_ae: 1,
                    ..RlShellInput::default()
                };
                let kind = apply(i, &mut m, &mut g);
                seen_requests.insert(kind);
                checked_inputs += 1;
                assert_eq!(
                    m.u8(0x5080f8 + 16 * u32::from(m.u8(0x508041).unwrap()))
                        .unwrap(),
                    branch as u8
                );
            }
            for frame in 0..32 {
                m.set_u32(0x508f84, frame).unwrap();
                shell::draw(&mut m, &mut r, &banks).unwrap();
                unsafe {
                    assert_eq!(rl_shell_draw(handle, g, frame, &mut g), 0);
                }
                compare(&m, &r, g);
                checked_frames += 1;
            }
        }
        let mut actions: Vec<RlShellInput> = if matches!(mode, 2 | 3 | 16) {
            [2, 4, 6, 8, 15]
                .into_iter()
                .map(|keys| RlShellInput {
                    version: 1,
                    keys,
                    ..RlShellInput::default()
                })
                .collect()
        } else if matches!(mode, 17 | 23) {
            // Native page arrows: counter divisibility controls scrolling.
            [(280, 170, 10), (310, 170, 11), (310, 170, 20)]
                .into_iter()
                .map(|(x, y, c)| RlShellInput {
                    version: 1,
                    mouse_x: x,
                    mouse_y: y,
                    mouse_ae: 1,
                    clock_7c: c,
                    ..RlShellInput::default()
                })
                .collect()
        } else {
            vec![]
        };
        actions.extend(match mode {
            2 | 3 | 16 => (0..if mode == 2 {
                5
            } else if mode == 3 {
                2
            } else {
                6
            })
                .flat_map(|row| {
                    // Hover selects each actual original menu row; Enter then dispatches.
                    [
                        RlShellInput {
                            version: 1,
                            mouse_x: 100,
                            mouse_y: 290 + 25 * row,
                            ..RlShellInput::default()
                        },
                        RlShellInput {
                            version: 1,
                            keys: 1,
                            ..RlShellInput::default()
                        },
                    ]
                })
                .collect::<Vec<_>>(),
            17 => vec![RlShellInput {
                version: 1,
                keys: 1,
                ..RlShellInput::default()
            }],
            23 => vec![RlShellInput {
                version: 1,
                keys: 1,
                ..RlShellInput::default()
            }],
            _ => vec![
                RlShellInput {
                    version: 1,
                    ..RlShellInput::default()
                },
                RlShellInput {
                    version: 1,
                    keys: 1,
                    ..RlShellInput::default()
                },
            ],
        });
        for i in actions {
            let kind = apply(i, &mut m, &mut g);
            seen_requests.insert(kind);
            checked_inputs += 1;
            compare(&m, &r, g);
        }
        if mode == 17 {
            // StartMission set mode0. Neither an unsupported draw nor input may
            // publish the supplied counter/mouse changes or any output bytes.
            let mut untouched = 99;
            let mut e = RlShellEvent {
                generation: 99,
                ..RlShellEvent::default()
            };
            let i = RlShellInput {
                version: 1,
                mouse_x: 123,
                clock_7c: 456,
                animation_84: 789,
                ..RlShellInput::default()
            };
            unsafe {
                assert_eq!(rl_shell_draw(handle, g, 999, &mut untouched), 5);
                assert_eq!(rl_shell_input(handle, g, &i, &mut e), 5);
            }
            assert_eq!(untouched, 99);
            assert_eq!(e.generation, 99);
            compare(&m, &r, g);
        }
        let mut exported = 0;
        unsafe {
            assert_eq!(rl_shell_export_world(handle, g, &mut exported), 0);
        }
        with_world(exported, |w| {
            assert_eq!(w.project()?, m);
            assert_eq!(w.frame_phase, 3);
            assert_eq!(w.execution_mode(), 1);
            assert_eq!(w.clock.tick(), 0);
            Ok(())
        })
        .unwrap();
        assert_eq!(rl_shell_free(handle), 0);
        assert_eq!(rl_shell_free(handle), 2);
        with_world(exported, |w| {
            assert_eq!(w.project()?, m);
            Ok(())
        })
        .unwrap();
        assert_eq!(rl_liberar_mundo(exported), 0);
    }
    assert_eq!(seen_requests, (0..=11).collect());
    eprintln!("SHELL_ABI_FIXTURES {{\"entries\":{checked_entries},\"frames\":{checked_frames},\"inputs\":{checked_inputs},\"request_kinds\":12,\"s0_sha256\":\"{hash}\",\"native_differentials_rerun\":false}}");
}

#[test]
fn redistribution_abi_layout_and_bad_arguments_leave_outputs_unchanged() {
    assert_eq!(size_of::<RlRedistribuicao>(), 60);
    assert_eq!(rl_redistribuir_versao(), 1);
    let mut state = RlRedistribuicao {
        version: 77,
        ..Default::default()
    };
    let (mut ramo, mut delta, mut g, mut mudou) = (9u32, 9i32, 9u64, 9u32);
    let mut pixels = vec![3u8; 480000];
    let palette = vec![0u8; 768];
    unsafe {
        assert_eq!(rl_redistribuir_estado(0, &mut state), 2);
        assert_eq!(rl_redistribuir_alvo(0, 1, 1, &mut ramo, &mut delta), 2);
        assert_eq!(rl_redistribuir_passo(0, 0, 0, 1, &mut g, &mut mudou), 2);
        // Branch above 3 or a delta other than ±1 is refused before the handle.
        assert_eq!(rl_redistribuir_passo(0, 0, 4, 1, &mut g, &mut mudou), 1);
        assert_eq!(rl_redistribuir_passo(0, 0, 0, 0, &mut g, &mut mudou), 1);
        assert_eq!(
            rl_redistribuir_sobrepor(0, pixels.as_mut_ptr(), 480000, palette.as_ptr(), 768, 0),
            2
        );
        assert_eq!(
            rl_redistribuir_sobrepor(0, pixels.as_mut_ptr(), 480000, palette.as_ptr(), 768, 2),
            1
        );
        assert_eq!(
            rl_redistribuir_alvo(0, 1, 1, ptr::null_mut(), &mut delta),
            1
        );
    }
    assert_eq!(state.version, 77);
    assert_eq!((ramo, delta, g, mudou), (9, 9, 9, 9));
    assert!(pixels.iter().all(|&p| p == 3));
}
