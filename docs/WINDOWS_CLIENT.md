# Writing a Windows client

> **Resumo em português.** O jogo roda sobre o motor em Rust, que expõe uma ABI em C (`engine/include`). O cliente macOS
> (`macos/src`) é a implementação de referência: janela e apresentação 800x600, paleta, ritmo de 30 quadros por segundo
> no 1x, mouse e teclado convertidos em mensagens no estilo Win32, áudio, música do CD, filmes, importação do CD na
> primeira execução, saves e menus. Um cliente Windows precisa refazer só essa camada de host, sobre a mesma ABI. Este
> documento lista o que cada parte faz, onde está no código macOS, as chamadas da ABI em ordem e o que ainda é
> específico do macOS. Nenhum dado do jogo pode ser incluído: tudo vem do CD original do jogador.

> **Estado (2026-11-12).** O cliente Windows já existe em [`windows/`](../windows/README.md) (Win32 + GDI em C, motor
> estático, um único `RisingLands.exe`): importa o CD, abre no menu principal original, joga a campanha e salva e
> carrega no mesmo formato do macOS, com paridade quadro a quadro com o macOS. Faltam áudio e filmes (#4) e a
> assinatura do código (#5). Este guia continua valendo como mapa da camada de host.

This guide is for a contributor who wants to build a Windows host over the same engine. Each statement points to the
macOS file that implements it. Items not checked against the code are marked **unverified**.

## 0. Current state: `windows/`

A Windows client now lives in [`windows/`](../windows/README.md): Win32 + GDI in C11, the engine linked statically, one
self-contained `RisingLands.exe` that imports only system DLLs. It cross-compiles with mingw-w64 and is built on
`windows-latest` by `.github/workflows/windows-client.yml`.

| Part | macOS reference | Windows |
|---|---|---|
| Window, 800x600 indexed presentation with palette, aspect-fit, fullscreen | `NativeLiveWindow.swift`, `NativeViewportPolicy.swift` | `rlw_main.c`: 8-bit DIB + `StretchDIBits`, black letterbox bars, borderless fullscreen (F11) — done |
| Frame pacing, 1x = 30 frames/s, 0.5x/2x/4x, pause | `NativeGameSpeed.swift` (`NativeFramePacer`) | `rlw_main.c`: QPC deadline, debt above 0.25 s forgiven; P/Pause, +/− — done |
| Mouse and keyboard (the 23 virtual keys, held drags, edge zones, F11 never delivered) | `NativeLiveWindow.swift`, `NativeHotkeyInputRouter.swift`, `macos_live_driver.c` | `rlw_main.c`, `rlw_driver.c` (same queue and click gesture) — done |
| Mission start, renderer from the CD, per-frame phases | `macos_live_driver.c` | `rlw_driver.c` — done, per-tick frame hashes equal to the macOS driver's |
| First-run CD import (BIN/CUE/ISO/folder, SHA-256) | `NativeCDImport.swift`, `NativePlayerImport.swift` | `rlw_cd.c` — done (no audio decoding yet) |
| Original menus, briefing, pause, result, credits | `macos_campaign_shell.inc`, `NativeLiveWindow.swift` shell paths, `NativeFLC.swift` | `rlw_shell.inc`, `rlw_menu.c`, `rlw_flc.c` — done, menu walk equal to the macOS driver's |
| Saves and loads: slots + `.view` companion, background write, stage autosave, the original save/load lists, "Partida" menu (F5/F9) | `macos_live_driver.c` (`snapshot_capture`, `macos_live_save*`, `macos_live_load`, `macos_live_autosave`), `macos_campaign_shell.inc`, `NativeLiveWindow.swift`, `main.swift` | `rlw_save.inc`, `rlw_saves.c`, `rlw_fs.c` (durable replace with `FlushFileBuffers`) — done, same files as macOS, saved files equal to the macOS driver's |
| Sound effects, CD music, event pictures (FLC), movies (AVI) | `NativeAudio.swift`, `NativeCDMusic*.swift`, `NativeEventFLCPlayback.swift`, `NativeDefeatMovie.swift` | **open** (#4): in-game FLC events end at once; movies complete as unavailable |
| MSVC and ARM64 builds, code signing, DPI awareness | — | **open** (#5) |
| Modern shortcuts and the Shift+click order queue (`RL_REGRA_FILA_ORDENS`) | `NativeModernHotkeys.swift`, `macos_live_default_rules` | **open**: the Windows host creates worlds with `regras = 0` (the original rules) |

The rest of this guide is the original contributor map, kept for anyone working on the open parts.

## 1. Build the engine as a Windows library

The engine is a Rust workspace (`engine/Cargo.toml`). Crate `rl-ffi` builds library `rising_engine` as `cdylib` and `rlib`
(`engine/crates/rl-ffi/Cargo.toml`). The macOS build also links it statically (`cargo rustc -p rl-ffi --lib --crate-type
staticlib`, see `build-macos.sh`).

```
rustup target add x86_64-pc-windows-msvc        # and/or aarch64-pc-windows-msvc
cargo build --release --locked --manifest-path engine/Cargo.toml --target x86_64-pc-windows-msvc
```

The DLL (`rising_engine.dll`) or the static library is then used with the headers in `engine/include`. The core headers are
`rising_engine.h`, `rising_shell.h`, `rising_difficulty.h`, `rising_load_tail_input.h` and `game_hud_background_abi.h`;
`module.modulemap` is only for Swift. **Verified (2026-11-11):** the engine builds unchanged as a static library for
`x86_64-pc-windows-gnu` (`cargo rustc --release --locked -p rl-ffi --lib --crate-type staticlib --target
x86_64-pc-windows-gnu`; native libraries `kernel32 ntdll userenv ws2_32 dbghelp`), which is what `windows/build.sh`
does. The MSVC targets are still **unverified**. The engine has no OS calls in its logic: no wall clock, no threads,
and file I/O only in save and load.

The engine is deterministic and frame based. The host owns time, input, presentation and audio, and passes everything
across the ABI.

## 2. What the host must provide

| Part | macOS reference | Notes |
|---|---|---|
| Window and 800x600 presentation, aspect-fit, fullscreen | `NativeLiveWindow.swift` (`NativeMetalGameView`: Metal texture 800x600 `r8Uint`, palette lookup in the shader), `NativeViewportPolicy.swift` (continuous aspect-fit, letterbox bars, mapping between window points and original pixels) | The engine renders an 8-bit indexed 800x600 surface plus a 768-byte palette (`rl_render_quadro` writes `pixels` and `palette`). The host expands it to RGB. Fullscreen shows only the game on black |
| Frame pacing, 1x = 30 frames/s | `NativeGameSpeed.swift` (`NativeGameSpeed.normalFrameRate = 30`, speeds 15/30/60/120; `NativeFramePacer`: no bursts, debt above 0.25 s forgiven) | One engine frame per tick at every speed. The engine's virtual TickClock stays at 20 Hz (`ticks_per_second = 20` in `RlMissaoInicio`). That is virtual time, not the play pace |
| Mouse into Win32-style messages | `NativeLiveWindow.swift` (`mouse(_:_:held:)`, `mouseExited`, `leaveScrollEdge`) and `macos_live_driver.c` (`macos_live_event`, mouse branch) | Messages `0x200` move, `0x201` left down, `0x202` left up, `0x203` double click, `0x204` right down, in original pixels 0..799 × 0..599. A left press is held across frames until the release. Clicks are spread over frames k, k+1 and k+3 |
| Edge scrolling | Same files; engine zones in `engine/crates/rl-core/src/frame_core.rs` (`init_zone_plan`) | The original's scroll zones are 2 px wide at the screen edges. A plain move over the letterbox bars clamps to the nearest surface pixel. In fullscreen, leaving the view keeps the pointer on that edge. In a window the map scrolls only while the cursor is inside it. Focus loss pulls the pointer 2 px inward |
| Keyboard | `NativeHotkeyBindings.swift`, `NativeHotkeyInputRouter.swift`, `NativeHotkeyPreferences.swift`; `macos_live_driver.c` (`macos_live_event`, key branch) | Exactly 23 virtual keys are accepted: 13, 16, 17, 18, 27, 32, 37–40, 46, 88, 48–57, 9. They become `WM_KEYDOWN`/`WM_KEYUP` (`0x100`/`0x101`) with the original scan codes and `lParam` bits. Any other key is rejected (`UnsupportedMacVirtualKey`). On Windows the physical keys already are VKs, but keep the same whitelist |
| F11 | `macos/tests/HotkeyBindingsTests.swift`; host contract in section 5 | Never deliver F11 (VK `0x7a`). The engine ports the original 800x600 ↔ 640x480 switch, but the renderer composes only 800x600 |
| Sound effects | `NativeAudio.swift`, `macos_audio_mapper.c` | Sound requests come from the engine's frame events (`rl_eventos_entrada` / `rl_eventos_quadro`). The samples were decoded from the CD's `.SND` banks at import |
| CD music | `NativeCDMusicHost.swift`, `NativeCDMusicPlayer.swift`, `NativeCDMusicDisc.swift` | Tracks 2 and 3, read from the raw `.BIN` image only, at import. The engine requests play and stop as frame events (MCICD) |
| Event pictures (FLC) and movies (AVI) | `NativeEventFLCPlayback.swift`, `NativeFLC.swift`, `macos_event_flc.inc`; `NativeDefeatMovie.swift` (intro and defeat AVI) | FLC: `rl_eventos_flc_v1`, `rl_evento_flc_estado_v1`, `rl_evento_flc_terminar_v1`. A movie ends on the Escape key-up, as in the original |
| First-run CD import | `NativeCDImport.swift`, `NativePlayerImport.swift` | Reads the player's `.BIN`/`.CUE`/`.ISO` or CD folder, verifies by SHA-256 and writes to the user's data folder. **No game data may be bundled.** Everything else the renderer needs is computed by the engine from these CD files (see `PUBLICATION_AUDIT.md`, section 5) |
| Saves | `macos_live_driver.c` (`macos_live_save_begin`/`macos_live_save_commit`, `macos_live_load`, `macos_live_autosave`); menus in `NativePlayerApp.swift` | `rl_salvar_bytes`, `rl_autosalvar_bytes`, `rl_carregar_arquivo`, `rl_salvar_versao` |
| Original menus, briefing, results, credits | `NativeCampaignMenu.swift`, `macos_campaign_shell.inc` | The shell ABI (`rl_shell_*`, `rising_shell.h`): `rl_shell_create`, `rl_shell_plan`, `rl_shell_enter`, `rl_shell_input`, `rl_shell_draw`, `rl_shell_copy`, `rl_shell_credits_frame`, `rl_shell_defeat_finish`, `rl_shell_free` |
| Player menus (pause, speed, difficulty, keys, re-import) | `NativePlayerApp.swift` | Host UI only |

## 3. ABI calls of the macOS host, in order

From `macos/src/macos_live_driver.c` (`create`, `macos_live_advance`, `macos_live_input_phase`, save and continue
functions).

1. **Version checks:** `rl_versao` (major 1, minor ≥ 6), `rl_render_contrato` (`0x10009`), `rl_salvar_versao`,
   `rl_game_hud_background_version`, `rl_missao_versao` (1), `rl_shell_version`.
2. **Start:** either a stage or the original main menu.
   - **A stage:** pass ten blocks to `rl_missao_iniciar_eventos_v1`, with an `RlMissaoInicio` config: `ticks_per_second = 20`,
     `allocation_fill = 0` (a non-zero fill makes recruits' level icons blink), cursor (400, 300), execution mode `RL_EXECUCAO_JOGO`, and an `RlEventsHostV1` for the
     `EVENTS` folder. The ten blocks are `RISING.EXE`, `RISING.INI`, `RISING.RUL`, `RISING.SBL`, `TABLEAU.COL`, `COULEUR.MAP`,
     `PLATGAME.PIE`, `MONSTRE.INI`, `TEXTES.TXT` and the stage `.ETA`. Without an events host, call `rl_missao_iniciar`.
     Then call `rl_aplicar_dificuldade`, and `rl_carregar_eventos` with `EVENTS.INI`.
   - **The original main menu:** `rl_missao_iniciar_menu_eventos_v1` with the first nine blocks.
3. **Renderer:** `rl_game_hud_background_initialize` (from `PLATGAME.PIE`), then `rl_render_criar_cd(world, banks[7],
   surface)` with the six CD graphics banks (`BLOCS`, `TROOPS`, `VEHICULE`, `CREATURE`, `BUILDING`, `OTHERS.CMP`) and
   `PLATGAME.PIE`. The engine computes the shadow and lighting tables and a new game's terrain cache itself. The game
   palette comes from `rl_mundo_paleta_jogo`, the menu shell's tables 7/8 from `rl_mundo_tabelas_sombra_luz`, and the
   five menu backgrounds (`TITRE.PIE`, `PAGEMIS.PIE`, `PAGEMIS2.PIE`, `CREDITS.PIE`, `ECRANFIN.LZS`) from
   `rl_shell_imagem_cd`. `rl_render_reset_cd` resets the renderer with a new game's cache.
4. **First frame:** `rl_load_tail_entrada_v1` with the held keys, then the first draw (`rl_render_geracao`,
   `rl_render_quadro`), then `rl_autosalvar_inicio_fase`.
5. **Every frame:** `rl_missao_tela` (screens 16 and 19 pause the stage loop), then `rl_passo_quadro_entrada` once per input
   phase with the queued messages and held keys. Then `rl_render_geracao` and `rl_render_quadro` (indexed pixels and
   palette), and `rl_eventos_entrada` / `rl_eventos_quadro` for sound, music and FLC events.
6. **Stage transitions:** `rl_missao_tela`, `rl_missao_nome_proxima`, `rl_missao_continuar`, with a new renderer for the
   next world. Free the old ones with `rl_render_liberar` and `rl_liberar_mundo`.
7. **Saves:** `rl_salvar_bytes` / `rl_autosalvar_bytes` to write, `rl_carregar_arquivo` to load.
8. **Errors:** `rl_erro_mensagem`.

## 4. What is still macOS-specific

- Presentation (Metal), windowing and menus (AppKit), audio (AVFoundation), keyboard layout labels (Carbon).
- The C driver `macos_live_driver.c` is C, but uses POSIX (`unistd.h`, `fcntl.h`, `clock_gettime`), CommonCrypto
  (SHA-256) and, without `MACOS_STATIC_ENGINE`, dynamic symbol loading. On Windows these need replacements (for example
  `QueryPerformanceCounter`, `BCrypt` or a small SHA-256, `LoadLibrary`/`GetProcAddress`). The rest of the driver
  (input queue, frame phases, save jobs) could be reused (**unverified**).
- The CD image reader in `NativeCDImport.swift` (BIN/CUE/ISO, IMA ADPCM decoding) is Swift and needs a port.
- The edition switch `-DRISING_PLAYER_EDITION` removes the research UI. A Windows client needs only the player
  behaviour.

## 5. Rules

- Never commit game assets or anything derived from the CD: no sprites, palettes, sounds, music, texts, stages, movies,
  fonts, caches or tables taken from `RISING.EXE`. Everything comes from the player's CD at first run.
- Keep the engine logic unchanged. The host only paces frames, maps input, presents and plays audio.
- No hardcoded paths. The player chooses the CD, and data goes to the user's application data folder.
