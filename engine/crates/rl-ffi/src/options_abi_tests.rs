use super::*;
use crate::shell_abi::{
    rl_shell_copy, rl_shell_create, rl_shell_export_world, rl_shell_free, rl_shell_input,
    rl_shell_paint_prepared, RlShellBytes, RlShellEvent, RlShellInput,
};
use std::mem::{align_of, size_of};

fn ok(code: i32) {
    assert_eq!(code, 0, "{}", last_error());
}
fn last_error() -> String {
    let mut buffer = vec![0u8; 4096];
    let mut n = 0u32;
    unsafe { crate::rl_erro_mensagem(buffer.as_mut_ptr(), buffer.len() as u32, &mut n) };
    String::from_utf8_lossy(&buffer[..(n as usize).min(buffer.len())]).into_owned()
}
fn defaults() -> RlOpcoesValores {
    RlOpcoesValores {
        version: 1,
        atalhos: 1,
        caminhos: 0,
        dificuldade: 1,
        velocidade: 1,
        disponivel: 0x3f,
        idioma: 0,
        reserved: 0,
    }
}

#[test]
fn options_abi_layout_and_invalid_arguments_leave_outputs_unchanged() {
    assert_eq!(size_of::<RlOpcoesValores>(), 32);
    assert_eq!(size_of::<RlOpcoesEntrada>(), 32);
    assert_eq!(size_of::<RlOpcoesEvento>(), 48);
    assert_eq!(align_of::<RlOpcoesEvento>(), 4);
    assert_eq!(rl_opcoes_versao(), 1);
    let background = vec![0u8; PIXELS];
    let bytes = RlShellBytes {
        data: background.as_ptr(),
        length: PIXELS as u32,
    };
    let mut handle = 0xabcd;
    unsafe {
        for bad in [
            RlOpcoesValores {
                version: 2,
                ..defaults()
            },
            RlOpcoesValores {
                velocidade: 4,
                ..defaults()
            },
            RlOpcoesValores {
                dificuldade: 3,
                ..defaults()
            },
            RlOpcoesValores {
                disponivel: 0x40,
                ..defaults()
            },
            RlOpcoesValores {
                atalhos: 2,
                ..defaults()
            },
        ] {
            assert_eq!(rl_opcoes_criar(&bad, bytes, &mut handle), 1);
        }
        let short = RlShellBytes {
            data: background.as_ptr(),
            length: 1000,
        };
        assert_eq!(rl_opcoes_criar(&defaults(), short, &mut handle), 1);
        assert_eq!(handle, 0xabcd);
        ok(rl_opcoes_criar(&defaults(), bytes, &mut handle));
        let mut v = RlOpcoesValores::default();
        ok(rl_opcoes_valores(handle, &mut v));
        assert_eq!(v, defaults());
        let changed = RlOpcoesValores {
            velocidade: 3,
            caminhos: 1,
            ..defaults()
        };
        ok(rl_opcoes_definir(handle, &changed));
        ok(rl_opcoes_valores(handle, &mut v));
        assert_eq!(v, changed);
        let mut open = 9;
        ok(rl_opcoes_aberta(handle, &mut open));
        assert_eq!(open, 0);
        // No shell session: opening, input and drawing are refused atomically.
        assert_eq!(rl_opcoes_abrir(handle, 999_999), 2);
        let mut e = RlOpcoesEvento {
            kind: 77,
            ..Default::default()
        };
        let i = RlOpcoesEntrada {
            version: 1,
            ..Default::default()
        };
        assert_eq!(rl_opcoes_entrada(handle, 999_999, &i, &mut e), 2);
        let bad_input = RlOpcoesEntrada { keys: 64, ..i };
        assert_eq!(rl_opcoes_entrada(handle, 999_999, &bad_input, &mut e), 1);
        assert_eq!(e.kind, 77);
        let mut pixels = vec![5u8; PIXELS];
        let mut palette = vec![5u8; 768];
        assert_eq!(
            rl_opcoes_desenhar(
                handle,
                999_999,
                pixels.as_mut_ptr(),
                PIXELS as u32,
                palette.as_mut_ptr(),
                768
            ),
            2
        );
        assert_eq!(
            rl_opcoes_sobrepor(handle, 999_999, pixels.as_mut_ptr(), 10),
            6
        );
        assert!(pixels.iter().all(|&p| p == 5));
        ok(rl_opcoes_fechar(handle));
        ok(rl_opcoes_liberar(handle));
        assert_eq!(rl_opcoes_liberar(handle), 2);
        assert_eq!(rl_opcoes_valores(handle, &mut v), 2);
    }
}

/// Minimal indexed PNG (stored deflate) for the review captures.
fn png(path: &std::path::Path, pixels: &[u8], palette: &[u8]) {
    fn crc(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 != 0 {
                    0xedb88320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for row in pixels.chunks(800) {
        raw.push(0);
        raw.extend(row);
    }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        z.push(u8::from((i + 1) * 65535 >= raw.len()));
        z.extend((block.len() as u16).to_le_bytes());
        z.extend((!(block.len() as u16)).to_le_bytes());
        z.extend(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &v in &raw {
        a = (a + u32::from(v)) % 65521;
        b = (b + a) % 65521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend(800u32.to_be_bytes());
    header.extend(600u32.to_be_bytes());
    header.extend([8, 3, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"PLTE", palette);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    std::fs::write(path, out).unwrap();
}

struct Shell {
    handle: u64,
    generation: u64,
}
impl Shell {
    fn copy(&self) -> (Vec<u8>, Vec<u8>) {
        let (mut pixels, mut palette) = (vec![0u8; PIXELS], vec![0u8; 768]);
        unsafe {
            ok(rl_shell_copy(
                self.handle,
                self.generation,
                pixels.as_mut_ptr(),
                PIXELS as u32,
                palette.as_mut_ptr(),
                768,
            ))
        };
        (pixels, palette)
    }
    fn memory(&self) -> Vec<(u32, Vec<u8>)> {
        let mut world = 0;
        unsafe {
            ok(rl_shell_export_world(
                self.handle,
                self.generation,
                &mut world,
            ))
        };
        let m = crate::with_world(world, |w| Ok(w.project()?)).unwrap();
        ok(crate::rl_liberar_mundo(world));
        m.blocks().map(|(a, b)| (a, b.to_vec())).collect()
    }
    fn input(&mut self, input: &RlShellInput) -> RlShellEvent {
        let mut event = RlShellEvent::default();
        unsafe {
            ok(rl_shell_input(
                self.handle,
                self.generation,
                input,
                &mut event,
            ))
        };
        self.generation = event.generation;
        event
    }
}
fn shell_over(world: u64, blobs: &[Vec<u8>], surface: &[u8], titre: &[u8]) -> Shell {
    let desc = |b: &[u8]| RlShellBytes {
        data: b.as_ptr(),
        length: b.len() as u32,
    };
    let descriptors = blobs.iter().map(|b| desc(b)).collect::<Vec<_>>();
    let mut s = Shell {
        handle: 0,
        generation: 0,
    };
    unsafe {
        ok(rl_shell_create(
            1,
            world,
            descriptors.as_ptr(),
            11,
            desc(surface),
            &mut s.handle,
        ));
        ok(rl_shell_paint_prepared(
            s.handle,
            0,
            desc(titre),
            &mut s.generation,
        ));
    }
    s
}
fn entrada(x: i32, y: i32, ae: u32, af: u32, keys: u32) -> RlOpcoesEntrada {
    RlOpcoesEntrada {
        version: 1,
        keys,
        mouse_x: x as u32,
        mouse_y: y as u32,
        mouse_ae: ae,
        mouse_af: af,
        ticks: 0,
        reserved: 0,
    }
}
fn send(o: u64, shell: u64, i: RlOpcoesEntrada) -> RlOpcoesEvento {
    let mut e = RlOpcoesEvento::default();
    unsafe { ok(rl_opcoes_entrada(o, shell, &i, &mut e)) };
    e
}
fn draw(o: u64, shell: u64) -> (Vec<u8>, Vec<u8>) {
    let (mut pixels, mut palette) = (vec![0u8; PIXELS], vec![0u8; 768]);
    unsafe {
        ok(rl_opcoes_desenhar(
            o,
            shell,
            pixels.as_mut_ptr(),
            PIXELS as u32,
            palette.as_mut_ptr(),
            768,
        ))
    };
    (pixels, palette)
}

/// The button on the original menu column (centre x = 400, row 286 + 25 * count)
/// touches no original item zone and no opaque pixel of any frame of MENU.CMP's
/// three AffGame animations (sprites 1..16 at (35,134), 17..32 at (702,0),
/// 33..40 at (528,315), drawn from their top-left corner).
fn assert_button_clear(rect: [i32; 4], count: i32, menu_cmp: &[u8]) {
    use rl_core::render_assets::Assets;
    assert_eq!(rect[0] + rect[2], 800, "centred on x = 400");
    assert_eq!(rect[1], 286 + 25 * count, "row after the last item");
    for i in 0..count {
        let zone = [0, 286 + 25 * i, 800, 306 + 25 * i];
        assert!(zone[3] <= rect[1] || zone[1] >= rect[3], "item zone {i}");
    }
    let base = Assets {
        sprites: Vec::new(),
        palette: [0; 768],
        shadow: [0; 256],
        light: [0; 256],
    };
    let bank = rl_core::shell_animation::Bank::load(menu_cmp, &base).unwrap();
    let mut nearest = i32::MAX;
    for (x, y, ids) in [(35, 134, 1..=16), (702, 0, 17..=32), (528, 315, 33..=40)] {
        for id in ids {
            let q = bank.assets().sprite(id).unwrap();
            for sy in 0..q.height {
                for sx in 0..q.width {
                    if q.mask[sy * q.width + sx] == 0 {
                        continue;
                    }
                    let (px, py) = (x + sx as i32, y + sy as i32);
                    let dx = (rect[0] - px).max(px - (rect[2] - 1)).max(0);
                    let dy = (rect[1] - py).max(py - (rect[3] - 1)).max(0);
                    assert!(dx > 0 || dy > 0, "MENU.CMP sprite {id} pixel ({px},{py})");
                    nearest = nearest.min(dx.max(dy));
                }
            }
        }
    }
    eprintln!("button {rect:?}: nearest MENU.CMP pixel {nearest} px away");
}

/// The real program-start main menu (and, with RL_OPTIONS_PAUSE_SAVE, a real
/// in-game pause menu 16): the button, the click rule, navigation, determinism,
/// and an original shell that behaves byte for byte as one never touched by
/// the options. RL_OPTIONS_CAPTURES (optional) receives the review PNGs.
#[test]
#[ignore = "requires RL_B_INPUTS, RL_CD_ROOT; optional RL_OPTIONS_PAUSE_SAVE, RL_OPTIONS_CAPTURES"]
fn options_screen_over_the_real_main_and_pause_menus() {
    use rl_core::{render_assets::sha256, shell_picture};
    use std::{fs, path::PathBuf};
    let path = |name| PathBuf::from(std::env::var_os(name).expect(name));
    let captures = std::env::var_os("RL_OPTIONS_CAPTURES").map(PathBuf::from);
    if let Some(dir) = &captures {
        fs::create_dir_all(dir).unwrap();
    }
    let capture = |name: &str, pixels: &[u8], palette: &[u8]| {
        if let Some(dir) = &captures {
            png(&dir.join(name), pixels, palette);
        }
    };
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
    let blocks: Vec<crate::RlBloco> = blobs
        .iter()
        .map(|b| crate::RlBloco {
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
    let (mut menu, mut carried) = (0u64, rl_core::FrameInput::default());
    unsafe {
        ok(crate::mission_abi::rl_missao_iniciar_menu(
            blocks.as_ptr(),
            9,
            &config,
            &mut menu,
            &mut carried,
        ))
    };
    let mut assets = (0..9)
        .map(|i| fs::read(inputs.join(format!("asset{i}.bin"))).unwrap())
        .collect::<Vec<_>>();
    assets.push(fs::read(cd.join("INSTALHD/GRAPHICS/MENU.CMP")).unwrap());
    assets.push(fs::read(cd.join("INSTALHD/GRAPHICS/INVENTE.CMP")).unwrap());
    let mut titre = vec![0; PIXELS];
    shell_picture::pie(
        &fs::read(cd.join("INSTALHD/TITRE.PIE")).unwrap(),
        &mut titre,
        800,
    )
    .unwrap();
    let surface = vec![0u8; PIXELS];
    // Twin sessions over the same world: `a` meets the options, `b` never does.
    let mut a = shell_over(menu, &assets, &surface, &titre);
    let mut b = shell_over(menu, &assets, &surface, &titre);
    let (original, palette) = a.copy();
    assert_eq!(b.copy().0, original);
    capture("main-menu-original.png", &original, &palette);

    let background = RlShellBytes {
        data: titre.as_ptr(),
        length: PIXELS as u32,
    };
    let mut o = 0;
    unsafe { ok(rl_opcoes_criar(&defaults(), background, &mut o)) };
    let mut rect = [0i32; 4];
    unsafe { ok(rl_opcoes_botao(o, a.handle, rect.as_mut_ptr())) };
    assert_eq!(rect[1], 411, "main menu: five items, last row ends at 406");
    assert_button_clear(rect, 5, &assets[9]);
    let (cx, cy) = ((rect[0] + rect[2]) / 2, (rect[1] + rect[3]) / 2);
    // Overlay: only pixels inside the button rectangle change.
    let mut with_button = original.clone();
    unsafe {
        ok(rl_opcoes_sobrepor(
            o,
            a.handle,
            with_button.as_mut_ptr(),
            PIXELS as u32,
        ))
    };
    let changed = (0..PIXELS)
        .filter(|&i| with_button[i] != original[i])
        .collect::<Vec<_>>();
    assert!(!changed.is_empty());
    assert!(changed.iter().all(|&i| {
        let (x, y) = ((i % 800) as i32, (i / 800) as i32);
        x >= rect[0] && x < rect[2] && y >= rect[1] && y < rect[3]
    }));
    capture("main-menu-button.png", &with_button, &palette);
    // Hover is not consumed (the move also reaches the shell) and lights the caption.
    assert_eq!(send(o, a.handle, entrada(cx, cy, 0, 0, 0)).consumed, 0);
    let mut hovered = original.clone();
    unsafe {
        ok(rl_opcoes_sobrepor(
            o,
            a.handle,
            hovered.as_mut_ptr(),
            PIXELS as u32,
        ))
    };
    assert_ne!(hovered, with_button);
    capture("main-menu-button-hover.png", &hovered, &palette);
    let memory_before = a.memory();
    let generation_before = a.generation;
    // Press and release on the button: both consumed, the screen opens.
    let press = send(o, a.handle, entrada(cx, cy, 1, 0, 0));
    assert_eq!((press.consumed, press.kind), (1, 0));
    let release = send(o, a.handle, entrada(cx, cy, 1, 1, 0));
    assert_eq!((release.consumed, release.kind, release.open), (1, 1, 1));
    let (first, options_palette) = draw(o, a.handle);
    assert_eq!(options_palette, palette);
    assert_eq!(draw(o, a.handle).0, first, "deterministic frame");
    capture("options-pt-br.png", &first, &palette);
    let digest = |p: &[u8]| {
        sha256(p)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>()
    };
    eprintln!("options frame {}", digest(&first));
    // On the main menu the button row (411) is the screen's own VOLTAR row. A
    // pointer left still where the click opened the screen selects nothing;
    // the first move hovers (selects) VOLTAR, as in the original menus.
    let still = send(o, a.handle, entrada(cx, cy, 0, 0, 0));
    assert_eq!((still.consumed, still.kind), (1, 0));
    assert_eq!(draw(o, a.handle).0, first, "still pointer selects nothing");
    send(o, a.handle, entrada(cx + 1, cy, 0, 0, 0));
    let hovered_frame = draw(o, a.handle).0;
    assert_ne!(hovered_frame, first, "VOLTAR lit after a move");
    capture(
        "options-pt-br-pointer-on-voltar.png",
        &hovered_frame,
        &palette,
    );
    // The keyboard navigation below runs with the pointer clear of every item,
    // back on the first item.
    let cy = 560;
    for _ in 0..5 {
        send(o, a.handle, entrada(cx, cy, 0, 0, 2));
    }
    assert_eq!(draw(o, a.handle).0, first, "back on the first item");
    // Navigation: the right values change, the host reads them back.
    let e = send(o, a.handle, entrada(cx, cy, 0, 0, 1)); // Enter on "atalhos"
    assert_eq!((e.kind, e.item, e.values.atalhos), (2, 0, 0));
    send(o, a.handle, entrada(cx, cy, 0, 0, 4));
    let e = send(o, a.handle, entrada(cx, cy, 0, 0, 1));
    assert_eq!((e.kind, e.item), (3, 1), "configurar teclas is the host's");
    send(o, a.handle, entrada(cx, cy, 0, 0, 4));
    let e = send(o, a.handle, entrada(cx, cy, 0, 0, 1));
    assert_eq!((e.kind, e.item, e.values.caminhos), (2, 2, 1));
    send(o, a.handle, entrada(cx, cy, 0, 0, 4));
    let e = send(o, a.handle, entrada(cx, cy, 0, 0, 32));
    assert_eq!((e.kind, e.item, e.values.dificuldade), (2, 3, 2));
    send(o, a.handle, entrada(cx, cy, 0, 0, 4));
    let e = send(o, a.handle, entrada(cx, cy, 0, 0, 32));
    assert_eq!((e.kind, e.item, e.values.velocidade), (2, 4, 2));
    // Animation follows the host's ticks only.
    let ticked = send(
        o,
        a.handle,
        RlOpcoesEntrada {
            ticks: 2,
            ..entrada(cx, cy, 0, 0, 0)
        },
    );
    assert_eq!((ticked.consumed, ticked.kind), (1, 0));
    let (changed_frame, _) = draw(o, a.handle);
    assert_ne!(changed_frame, first);
    capture("options-changed.png", &changed_frame, &palette);
    // English is prepared (language option, private research repository issue #7).
    let mut english = e.values;
    english.idioma = 1;
    unsafe { ok(rl_opcoes_definir(o, &english)) };
    capture("options-en.png", &draw(o, a.handle).0, &palette);
    // A host without a key dialog (Windows today) offers no "configurar teclas".
    let without_keys = RlOpcoesValores {
        disponivel: 0x3f & !2,
        ..e.values
    };
    unsafe { ok(rl_opcoes_definir(o, &without_keys)) };
    capture("options-without-keys.png", &draw(o, a.handle).0, &palette);
    unsafe { ok(rl_opcoes_definir(o, &e.values)) };
    // Escape closes; nothing of the session changed while the options were open.
    let e = send(o, a.handle, entrada(cx, cy, 0, 0, 8));
    assert_eq!((e.kind, e.open, e.consumed), (4, 0, 1));
    assert_eq!(a.generation, generation_before);
    assert_eq!(a.copy().0, original);
    assert_eq!(a.memory(), memory_before);
    // The original shell then runs exactly like its untouched twin, including a
    // release on the button region when no press started there (MenuInput's
    // click-anywhere activation of the selected item).
    let mut shell_input = RlShellInput {
        version: 1,
        // Off the button and below every item zone (zone 0).
        mouse_x: cx as u32,
        mouse_y: rect[3] as u32 + 40,
        ..Default::default()
    };
    let mut events = Vec::new();
    for (keys, ae, af) in [(0, 0, 0), (4, 0, 0), (2, 0, 0), (0, 1, 0), (0, 1, 1)] {
        shell_input.keys = keys;
        shell_input.mouse_ae = ae;
        shell_input.mouse_af = af;
        shell_input.clock_7c += 1;
        shell_input.animation_84 += 1;
        let opt = send(
            o,
            a.handle,
            entrada(
                shell_input.mouse_x as i32,
                shell_input.mouse_y as i32,
                ae,
                af,
                keys,
            ),
        );
        assert_eq!(opt.consumed, 0, "only presses that start on the button");
        let (ea, eb) = (a.input(&shell_input), b.input(&shell_input));
        assert_eq!(
            (ea.kind, ea.mode, ea.generation),
            (eb.kind, eb.mode, eb.generation)
        );
        events.push((ea.kind, ea.mode));
        if ea.kind != 0 {
            break;
        }
        assert_eq!(a.copy(), b.copy());
    }
    assert_eq!(events.last(), Some(&(1, 3)), "item0 'um jogador' activated");
    assert_eq!(a.memory(), b.memory());
    eprintln!("main menu events {events:?}");
    ok(rl_shell_free(a.handle));
    ok(rl_shell_free(b.handle));

    // Pause menu 16 over a real saved game (InputGame's MenuInit(16)).
    if let Some(save) = std::env::var_os("RL_OPTIONS_PAUSE_SAVE") {
        let presentation = rl_core::mission_files::Presentation::read(&inputs).unwrap();
        let (mut world, _) = rl_core::save::decode(&fs::read(save).unwrap()).unwrap();
        let mut memory = world.project().unwrap();
        let mut none = rl_core::campaign_flow::NoFiles;
        rl_core::campaign_flow::menu_init(&mut memory, 0, 16, &mut none).unwrap();
        memory.set_u32(0x508f64, 0).unwrap();
        world.replace_memory(memory).unwrap();
        let paused = crate::mission_abi::publish(world).unwrap();
        let p = shell_over(paused, &assets, &presentation.surface, &titre);
        let (pause, pause_palette) = p.copy();
        capture("pause-menu-original.png", &pause, &pause_palette);
        let mut pause_rect = [0i32; 4];
        unsafe { ok(rl_opcoes_botao(o, p.handle, pause_rect.as_mut_ptr())) };
        assert_eq!(
            pause_rect[1], 436,
            "pause menu: six items, last row ends at 431"
        );
        assert_button_clear(pause_rect, 6, &assets[9]);
        let mut over = pause.clone();
        unsafe {
            ok(rl_opcoes_sobrepor(
                o,
                p.handle,
                over.as_mut_ptr(),
                PIXELS as u32,
            ))
        };
        assert_ne!(over, pause);
        capture("pause-menu-button.png", &over, &pause_palette);
        ok(rl_opcoes_abrir(o, p.handle));
        let (frame, _) = draw(o, p.handle);
        capture("options-from-pause.png", &frame, &pause_palette);
        ok(rl_opcoes_fechar(o));
        assert_eq!(p.copy().0, pause);
        ok(rl_shell_free(p.handle));
        ok(crate::rl_liberar_mundo(paused));
    }
    ok(rl_opcoes_liberar(o));
    ok(crate::rl_liberar_mundo(menu));
}
