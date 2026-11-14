# Writing a Windows client

> **Resumo em português.** O jogo roda sobre o motor em Rust, que expõe uma ABI em C (`engine/include`). O cliente macOS
> (`macos/src`) é a implementação de referência: janela e apresentação 800x600, paleta, ritmo de 30 quadros por segundo
> no 1x, mouse e teclado convertidos em mensagens no estilo Win32, áudio, música do CD, filmes, importação do CD na
> primeira execução, saves e menus. Um cliente Windows precisa refazer só essa camada de host, sobre a mesma ABI. Este
> documento lista o que cada parte faz, onde está no código macOS, as chamadas da ABI em ordem e o que ainda é
> específico do macOS. Nenhum dado do jogo pode ser incluído: tudo vem do CD original do jogador.

> **Estado (2026-11-14).** O cliente Windows já existe em [`windows/`](../windows/README.md) (Win32 + GDI em C, motor
> estático, um único `RisingLands.exe`): importa o CD, abre no menu principal original, joga a campanha, salva e
> carrega no mesmo formato do macOS e tem a tela de Opções, com paridade quadro a quadro com o macOS. Desde a #7 também
> tem os caminhos melhores, os atalhos modernos (N duas vezes, Home, ponto, vírgula, H) e a janela de configurar
> teclas, com os mesmos padrões do macOS (atalhos modernos ligados, caminhos melhores desligados). Para a pré-alfa 4
> (em andamento, ainda não publicada) chegaram os efeitos sonoros com o menu Áudio, a música do CD (lida do `.BIN` do
> próprio jogador), as figuras de evento (FLC) e os filmes (#4); as bandeiras da fila de ordens sobre o mapa; o menu
> "Jogo" do macOS (substitui "Partida"); o gerenciador de partidas salvas e o salvamento automático periódico; a
> partida salva pela lista da pausa voltando jogando; a janela ciente de DPI por monitor; e smokes do `.exe` no CI sem
> dados do jogo. Compila com mingw-w64 e com MSVC, só x86-64 (decisão de 2026-11-14; as pré-alfas 1 a 3 também
> tinham um build ARM64; o Windows em ARM pode rodar o build x64 pela própria emulação, sem suporte). Falta um teste
> num PC com Windows de verdade (#6), que fica com quem tem Windows. Os binários não são assinados, e não há plano de
> assiná-los. Este guia continua valendo como mapa da camada de host.

This guide is for a contributor who wants to build a Windows host over the same engine. Each statement points to the
macOS file that implements it. Items not checked against the code are marked **unverified**.

## 0. Current state: `windows/`

A Windows client now lives in [`windows/`](../windows/README.md): Win32 + GDI in C11, the engine linked statically, one
self-contained `RisingLands.exe` that imports only system DLLs. It builds with mingw-w64 (also cross from
macOS/Linux) and with MSVC, **x86-64 only** (decision of 2026-11-14; pre-alphas 1–3 also shipped an MSVC ARM64 build;
Windows on ARM may run the x64 build under its own emulation, unsupported). `.github/workflows/windows-client.yml`
builds both, runs the unit tests and the smokes of the `.exe` without game data (`windows/ci/smoke.sh`) and uploads
each `.exe` with its SHA-256. **Nobody has played it on a real Windows PC yet**: CI has no game data, and the parity
checks run on macOS. Testing on Windows is left to the community (#6).

| Part | macOS reference | Windows |
|---|---|---|
| Window, 800x600 indexed presentation with palette, aspect-fit, fullscreen | `NativeLiveWindow.swift`, `NativeViewportPolicy.swift` | `rlw_main.c`: 8-bit DIB + `StretchDIBits`, black letterbox bars, borderless fullscreen (F11) — done |
| Frame pacing, 1x = 30 frames/s, 0.5x/2x/4x, pause | `NativeGameSpeed.swift` (`NativeFramePacer`) | `rlw_main.c`: QPC deadline, debt above 0.25 s forgiven; P/Pause, +/− — done |
| Mouse and keyboard (the 23 virtual keys, held drags, edge zones, F11 never delivered) | `NativeLiveWindow.swift`, `NativeHotkeyInputRouter.swift`, `macos_live_driver.c` | `rlw_main.c`, `rlw_driver.c` (same queue and click gesture) — done |
| Key bindings and router: actions, aliases, optional modifiers, conflicts, digit double tap | `NativeHotkeyBindings.swift`, `NativeHotkeyInputRouter.swift` | `rlw_hotkeys.c` (portable C, `tests/test_hotkeys.c`) — done; with the default bindings the game gets exactly the keys it got before |
| Mission start, renderer from the CD, per-frame phases | `macos_live_driver.c` | `rlw_driver.c` — done, per-tick frame hashes equal to the macOS driver's |
| First-run CD import (BIN/CUE/ISO/folder, SHA-256) | `NativeCDImport.swift`, `NativePlayerImport.swift` | `rlw_cd.c` — done; then `rlw_sound_import.c` (the six sound banks) and `rlw_cdmusic.c` (CD audio tracks 2 and 3 from the raw `.BIN` only, as pinned WAVs) |
| Original menus, briefing, pause, result, credits | `macos_campaign_shell.inc`, `NativeLiveWindow.swift` shell paths, `NativeFLC.swift` | `rlw_shell.inc`, `rlw_menu.c`, `rlw_flc.c` — done, menu walk equal to the macOS driver's |
| Saves and loads: slots + `.view` companion, background write, stage autosave, the original save/load lists, F5/F9; a game saved from the pause list loads playing | `macos_live_driver.c` (`snapshot_capture`, `macos_live_save*`, `macos_live_load`, `macos_live_autosave`), `macos_campaign_shell.inc` (`macos_live_load_resume`), `NativeLiveWindow.swift`, `main.swift` | `rlw_save.inc`, `rlw_shell.inc` (`rlw_driver_load_resume`), `rlw_saves.c`, `rlw_fs.c` (durable replace with `FlushFileBuffers`) — done, same files as macOS, saved files equal to the macOS driver's |
| Saved-games manager (Jogo → Gerenciar partidas salvas…: list, rename, Recycle Bin) | `NativeSaveLibrary.swift`, `NativeSaveManager.swift` | `rlw_savemgr.c` (portable, `tests/test_savemgr.c`), `rlw_savemgr_dialog.c` (Win32, `SHFileOperationW` with `FOF_ALLOWUNDO`) — done |
| Periodic autosave (Jogo → Salvamento automático: off, 5/10/15 game minutes, slot `autosave`) | `NativePlayerOptions.swift` (`NativeAutosaveInterval`, `NativeAutosaveSchedule`), `NativeLiveWindow.swift` | `rlw_autosave.c` (portable), `rlw_main.c` (`autosave=` in `opcoes.ini`) — done |
| Jogo menu (replaces "Partida"): Pausar/Continuar, Velocidade, Dificuldade, saves, Opções… (in play over the pause), Configurar teclas…, Atalhos modernos, Caminhos melhores | `NativePlayerApp.swift` (`makePlayerMenu`), `main.swift`, `macos_options.inc` | `rlw_gamemenu.c` (portable, `tests/test_gamemenu.c`), `rlw_gamemenu_win.c` (Win32 popup) — done |
| Sound effects | `NativeAudio.swift`, `macos_audio_mapper.c` | `rlw_sound.c` (banks decoded in memory), `rlw_audio.c` (mixer and sound host over `macos_audio_mapper.c`, reused unchanged), `rlw_audio_win32.c` (`waveOut`) — done; Áudio menu (mute and volume for effects and music) in `rlw_audio_ui.c` |
| CD music | `NativeCDMusicHost.swift`, `NativeCDMusicPlayer.swift`, `NativeCDMusicDisc.swift` | `rlw_cdmusic.c` — done: tracks from the player's own `.BIN`, the macOS selection policy |
| Event pictures (FLC) | `NativeEventFLCPlayback.swift`, `macos_event_flc.inc` | `rlw_eventflc.c` — done: the picture over the view at the file's speed, with its sound ticks |
| Movies (AVI: introduction, defeat, campaign end) | `NativeDefeatMovie.swift` | `rlw_movie.c` — done: Cinepak and Microsoft ADPCM decoded by the host (no system codec), Escape key-up ends it |
| "Opções" screen over the main and pause menus, settings kept between runs | `macos_options.inc`, `NativeLiveWindow.swift` | `rlw_options.inc`, `rlw_main.c` (`opcoes.ini` beside `Dados` and `Partidas`) — done, with the macOS defaults (section 0.1) |
| Key window ("Configurar teclas"): one row per action, alternatives, capture field, defaults, apply/cancel | `NativeHotkeyPreferences.swift` | `rlw_keys_dialog.c` (plain Win32 controls), editor and `teclas.ini` format in `rlw_hotkeys.c` — done; opened from the Opções item and **Jogo → Configurar teclas…** |
| MSVC x64 build | — | `build-msvc.ps1`, CI job `msvc-x64` — done (section 1). x86-64 only since 2026-11-14 (the ARM64 build and its `windows-11-arm` job were removed) |
| Modern shortcuts (N twice, Home, period/comma, H) | `NativeModernHotkeys.swift`, `NativeLiveWindow.swift` (`startModern`, `stepModern`), `macos_live_peek` | `rlw_modern.c` (portable C: read-only snapshot, planner, command job), `rlw_main.c`, `rlw_driver.c` (`rlw_driver_peek`) — done; they follow the "Atalhos modernos" item |
| Shift+click order queue (`RL_REGRA_FILA_ORDENS`, rule bit 1) and its markers over the map | `macos_live_default_rules`, `NativeModernHotkeys.swift` (`NativeOrderQueueMarkers`), `NativeLiveWindow.swift` (`layoutWaypoints`) | done: "Atalhos modernos" in the Opções screen turns the rule on for worlds created afterwards; `rlw_markers.c` (portable, `tests/test_markers.c`) and `rlw_markers_gdi.c` draw the pending waypoints over the map as on macOS |
| Better unit paths (`RL_REGRA_CAMINHOS_MELHORES`, rule bit 0) | `macos_live_default_rules`, menu Jogo > Caminhos melhores | done: "Caminhos melhores" in the Opções screen turns the rule on for worlds created afterwards; `windows/parity.sh` checks `rl_regras` of the created world |
| DPI awareness | `NativeViewportPolicy.swift` (backing scale) | `rlw_dpi.c` — done: the game window per-monitor aware (sharp picture, real fullscreen resolution), fixed-layout windows scaled by Windows |
| A D3D11 presenter | — | open |
| Code signing | — | not planned (section 6) |

### 0.1 Opções defaults and rules

When `opcoes.ini` lacks a key the Windows host uses the macOS app's defaults (`NativeModernHotkeys`, `NativeModernPaths`,
`NativeDifficulty`, `NativeGameSpeed`): **Atalhos modernos on**, Caminhos melhores off, difficulty Normal, speed 1x.
Before 2026-11-13 Atalhos modernos was off on Windows; a saved `atalhos=0` keeps it off. The two rule items work as on
macOS (`NativeEngineRules.current()`): every world created afterwards (menu or stage) gets `regras` = bit 1
(`RL_REGRA_FILA_ORDENS`) for Atalhos modernos | bit 0 (`RL_REGRA_CAMINHOS_MELHORES`) for Caminhos melhores, and a
change also reaches the main-menu world before its load tail (`rl_regras_definir`). A running stage and a loaded save
keep the rules they were made with. Smoke, demo and parity runs read no `opcoes.ini` and keep every option off.

### 0.2 Modern shortcuts and the key window

With Atalhos modernos on: the same digit twice within 0.35 s centres on that group (the original's Shift+N), **Home**
goes to the last alert (the original's X), **.** / **,** select the next idle worker / soldier and **H** the sanctuary,
cycling through each. Each becomes ordinary original input through the player's own path (`rlw_driver_event`),
planned from a read-only snapshot taken between frames: a right press on the map first if something is selected, a
minimap press to bring the target into view, then a left click, each stage checked on a fresh snapshot; the pointer
then returns to the player's. A command waits for a held left button, lasts at most 300 frames and is dropped in
menus, while paused or stopped. Off, the four keys are not handled and digits are ordinary taps.

The key window edits a draft: Alterar replaces the selected alternative and Adicionar alternativa appends one, taking
the next combination pressed in the capture field (Enter, Esc and Tab included). Every action still reaches the game
only as one of the 23 whitelisted keys; a combination that overlaps another action's is refused, and so are the host's
own keys (P, Pause, + and −, F5, F9, F11, Alt+F4) and modifier/lock/system keys. Aplicar writes `teclas.ini` beside
`opcoes.ini` (temporary file + atomic replace) and only then installs the set. File format: `versao=1`, then
`<action>=<vk>/<mods>/<optional> …` per action (mods 1 Shift, 2 Ctrl, 4 Alt; an empty value unbinds), with the
macOS file's action names.

### 0.3 Checks

- `windows/build.sh test` (and `build-msvc.ps1 -Test`): portable unit tests, among them `windows/tests/test_hotkeys.c`
  (bindings, router, the `teclas.ini` format, the editor, the snapshot over a synthetic memory map, planner and job),
  `test_markers.c` (order queue markers), `test_savemgr.c` (saved-games manager, autosave schedule),
  `test_gamemenu.c` (Jogo menu model) and `test_audio.c` (sound banks, mixer, sound host, CD music import and policy,
  event FLC, movie decoding; all synthetic). CI runs them on `windows-latest` with gcc and with cl.
- `windows/ci/smoke.sh` runs the built `.exe` in CI without game data: `--selftest`, usage, the importer's refusals
  with their messages and no `Dados` left behind, the order queue markers through GDI (`--smoke-markers`), the DPI
  mixed mode (`--smoke-window`) and the first start's import dialogs (`--smoke-import-prompt`).
- `windows/parity.sh --cd …` on macOS with the player's CD: the Windows and macOS drivers give the same frames tick by
  tick, the same menu walk and the same save files, with every option off; then "REGRAS DAS OPÇÕES": each
  combination of the two items gives the created world exactly those `regras` (with Caminhos melhores on, `rl_regras`
  has bit 0 after the stage start). The save walk also loads a pause-list save playing on and runs the periodic
  autosave schedule.
- `windows/audio_parity.sh --cd …` on macOS with the player's CD: the sound and CD music requests of both drivers are
  identical, the play run renders to a non-silent, unclipped WAV, and every frame and sample of the original movies
  decoded by `rlw_movie.c` equals the macOS decoder's.
- Sound, music, movies, the key window and the modern shortcuts are built by CI but have not been tried interactively
  on a real Windows PC (#6).

The rest of this guide is the original contributor map, kept for anyone working on the open parts.

## 1. Build the engine as a Windows library

The engine is a Rust workspace (`engine/Cargo.toml`). Crate `rl-ffi` builds library `rising_engine` as `cdylib` and `rlib`
(`engine/crates/rl-ffi/Cargo.toml`). The macOS build also links it statically (`cargo rustc -p rl-ffi --lib --crate-type
staticlib`, see `build-macos.sh`).

```
rustup target add x86_64-pc-windows-msvc
cargo build --release --locked --manifest-path engine/Cargo.toml --target x86_64-pc-windows-msvc
```

The DLL (`rising_engine.dll`) or the static library is then used with the headers in `engine/include`. The core headers are
`rising_engine.h`, `rising_shell.h`, `rising_difficulty.h`, `rising_load_tail_input.h` and `game_hud_background_abi.h`;
`module.modulemap` is only for Swift. **Verified (2026-11-11):** the engine builds unchanged as a static library for
`x86_64-pc-windows-gnu` (`cargo rustc --release --locked -p rl-ffi --lib --crate-type staticlib --target
x86_64-pc-windows-gnu`; native libraries `kernel32 ntdll userenv ws2_32 dbghelp`), which is what `windows/build.sh`
does. **Verified (2026-11-12):** it also builds unchanged for `x86_64-pc-windows-msvc` (and `aarch64-pc-windows-msvc`,
no longer built) with `-C target-feature=+crt-static` (native set `kernel32 ntdll userenv ws2_32 dbghelp /defaultlib:libcmt`). The
engine has no OS calls in its logic: no wall clock, no threads, and file I/O only in save and load.

| Build | Rust target | C compiler | Script | CI job / artifact |
|---|---|---|---|---|
| mingw x64 | `x86_64-pc-windows-gnu` | mingw-w64 gcc | `windows/build.sh` | `mingw-x64` / `RisingLands-windows-x86_64-mingw` |
| MSVC x64 | `x86_64-pc-windows-msvc` | `cl.exe` x64, `/MT` | `windows/build-msvc.ps1` | `msvc-x64` / `RisingLands-windows-x86_64-msvc` |

Both are one `RisingLands.exe` importing system DLLs only, and both run the unit tests and `windows/ci/smoke.sh` in
CI. **x86-64 only** (decision of 2026-11-14): the ARM64 build and its `windows-11-arm` job were removed. Pre-alphas
1–3 did ship an ARM64 `.exe`, built by MSVC and tested natively on a `windows-11-arm` runner. Windows on ARM may run
the x64 build under its own emulation, unsupported.

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
- The CD image reader in `NativeCDImport.swift` (BIN/CUE/ISO, IMA ADPCM decoding) is Swift; the Windows client
  ports it in C (`windows/src/rlw_cd.c`, `rlw_sound.c`, `rlw_cdmusic.c`).
- The edition switch `-DRISING_PLAYER_EDITION` removes the research UI. A Windows client needs only the player
  behaviour.

## 5. Rules

- Never commit game assets or anything derived from the CD: no sprites, palettes, sounds, music, texts, stages, movies,
  fonts, caches or tables taken from `RISING.EXE`. Everything comes from the player's CD at first run.
- Keep the engine logic unchanged. The host only paces frames, maps input, presents and plays audio.
- No hardcoded paths. The player chooses the CD, and data goes to the user's application data folder.

## 6. Code signing

The Windows binaries are not signed, and the project does not plan to sign them. On the first run of a downloaded
`.exe`, Windows SmartScreen shows "Windows protected your PC" with "Unknown publisher": click "More info", then "Run
anyway". To check a download instead, compare its SHA-256 with the `.sha256` file of the CI artifact and with the
release notes, which name the commit it was built from.
