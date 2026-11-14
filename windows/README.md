# Rising Lands for Windows (pre-alpha 4)

> **Resumo em português.** Cliente Windows sobre o mesmo motor em Rust e a mesma ABI em C (`engine/include`) do app
> macOS. Este primeiro marco importa o CD brasileiro do jogador (imagem `.BIN`/`.CUE`, `.ISO` ou pasta), roda a
> MONDE01 a 30 quadros/s no 1x, mostra o quadro 800x600 com a paleta do jogo (com aspect-fit e tela cheia), entrega
> mouse e teclado pelo mesmo caminho do macOS (as mesmas mensagens, as mesmas 23 teclas, F11 nunca entregue) e
> pausa. Stack: Win32 + GDI em C, motor ligado estaticamente, um único `RisingLands.exe` sem DLLs de terceiros.
> Compila sem máquina Windows (mingw-w64). O marco 2 abre no menu principal original e roda os menus pelo shell
> preparado do motor, como o macOS: "Um jogador", "Novo jogo", briefing e objetivos (páginas por clique, setas ou
> roda do mouse), pausa no jogo (Esc: continuar, objetivos, abandonar, reiniciar a missão), tela de resultado com a
> próxima fase, créditos (FLC decodificado) e "Sair". O marco 3 traz os saves, com os mesmos arquivos do macOS:
> menu "Partida", hoje "Jogo" (salvar rápido F5, carregar rápido F9, posições com nome, início da fase), as listas originais de
> salvar e carregar na pausa e "Carregar jogo" no menu principal, e o salvamento automático no início de cada fase.
> Uma partida salva pela lista da pausa volta jogando ao ser carregada, como no original. O menu da janela se chama
> "Jogo", como no macOS: pausa, velocidade, dificuldade, Opções…, Atalhos modernos, Caminhos melhores, os saves,
> Jogo → Gerenciar partidas salvas… (lista, renomeia e apaga para a Lixeira, com o arquivo .view) e Jogo → Salvamento
> automático (a cada 5, 10 ou 15 minutos de jogo, desligado por padrão, na posição própria "autosave"). Só x86-64:
> Windows em ARM pode rodar o build x64 pela emulação do próprio Windows, sem suporte (as pré-alfas 1 a 3 também
> tinham um build ARM64).
> A pré-alfa 3 trouxe a tela de Opções, os atalhos modernos, a janela de configurar teclas e os caminhos melhores.
> A pré-alfa 4 (a versão atual, um único `RisingLands-windows-x86_64.exe`) traz efeitos sonoros e o menu "Áudio"
> (silenciar e volume de efeitos e música), a música do CD (lida da imagem `.BIN`/`.CUE` do próprio jogador), as
> figuras de evento (FLC) durante o jogo e os filmes originais (introdução, derrota, fim da campanha); as bandeiras da
> fila de ordens (Shift+clique) desenhadas sobre o mapa, como no macOS; a imagem nítida em telas com escala (DPI por
> monitor); o menu Jogo, o gerenciador de partidas salvas e o salvamento automático periódico; o ícone original do
> jogo (a abóbora, lido em tempo de execução do `RISING.EXE` importado) na janela, num atalho no menu Iniciar e, se o
> jogador quiser, na Área de Trabalho; Jogo → Importar o CD novamente… (quem vem das pré-alfas 1 a 3 precisa
> reimportar para ter sons e música; o jogo avisa uma vez); e smokes automáticos no CI sem dados do jogo (recusas da
> importação, primeira execução, desenho das bandeiras, DPI, áudio).
> Nenhum dado do jogo vai para o repositório: tudo vem do CD importado na primeira execução.

## Stack and why

**Win32 + GDI, in C11, with the engine linked statically** (`librising_engine.a` from `engine/crates/rl-ffi`).

- The host is a thin layer: the engine renders an 8-bit indexed 800x600 surface plus a 768-byte palette. GDI's
  `StretchDIBits` with an 8-bit `BITMAPINFO` does exactly the palette lookup and nearest-neighbour aspect-fit scale the
  macOS Metal shader does, at negligible cost for 30–120 frames/s.
- No third-party runtime: one self-contained `RisingLands.exe` that imports only system DLLs (UCRT, user32, gdi32,
  comdlg32, winmm…). Nothing to install besides the player's CD.
- The driver is C, so the production path of `macos/src/macos_live_driver.c` ports almost line for
  line (same constants, call order, input queue and click gesture). That is what keeps the faithful mode byte for byte.
- It cross-compiles from macOS/Linux with mingw-w64, builds natively under MSYS2 or on `windows-latest`, and builds
  with MSVC (`cl.exe`) for x64. The client is x86-64 only (decision of 2026-11-14; pre-alphas 1–3 also shipped an
  MSVC ARM64 build). Windows on ARM may run the x64 build under its own emulation, unsupported.
- SDL2 would add a DLL and an input layer to re-map back to Win32 messages the engine already speaks; D3D11 is not
  needed for this frame size. Either remains possible later behind the same driver if fullscreen performance or
  scaling filters call for it.

## Layout

| File | Role |
|---|---|
| `src/rlw_main.c` | Win32 window, GDI presentation, frame pacing, mouse/keyboard mapping, pause, speeds, fullscreen, CD selection, menu ticks and host requests, smoke modes |
| `src/rlw_driver.c/.h` | Engine orchestration: main-menu or mission start with the explicit EVENTS host, renderer from the CD, load tail, per-frame input phases (port of the macOS driver's production path) |
| `src/rlw_shell.inc` | Original menus on the prepared shell: session over pause 16 / result 19 / main menu, backgrounds decoded from the CD, briefing VFS, "Novo jogo", restart, next stage, resume, credits (port of `macos_campaign_shell.inc` and the driver's campaign continuation) |
| `src/rlw_options.inc` | The port's "Opções" screen over the main and pause menus (port of `macos_options.inc`): creation from the CD's TITRE.PIE, input before the shell, the button overlay |
| `src/rlw_hotkeys.c/.h` | Keyboard bindings and the physical-key router, portable (port of `NativeHotkeyBindings.swift` and `NativeHotkeyInputRouter.swift`): actions, the 23-key whitelist, aliases, optional modifiers, conflicts, the digit double tap |
| `src/rlw_keys_dialog.c/.h` | The "Atalhos de teclado" window (Win32, port of `NativeHotkeyPreferencesController`): one row per action, alternatives, capture field, defaults, apply/cancel |
| `src/rlw_modern.c/.h` | "Atalhos modernos", portable (port of `NativeModernHotkeys.swift`): read-only snapshot, target planner, the command job that turns a shortcut into ordinary input across frames |
| `src/rlw_markers.c/.h` | Shift+click order queue markers, portable (port of `NativeOrderQueueMarkers` and `NativeLiveWindow.layoutWaypoints`): reader over one read-only snapshot plus `rl_fila_ordens`, the aspect-fit layout in window pixels |
| `src/rlw_markers_gdi.c/.h` | Painting those markers with GDI over the map part of the picture (off-screen bitmap, clipped to the map) |
| `src/rlw_smoke.c/.h` | CI smokes inside the real `.exe` without game data: `--smoke-markers`, `--smoke-window`, `--smoke-import-prompt` (`--smoke-audio` lives in `rlw_audio_ui.c`) |
| `src/rlw_icon.c/.h` | The game's original icon, portable: the first icon group of the player's imported `RISING.EXE` (PE resources) written as `rising.ico` in the data folder at run time; no game bytes in the repository or the download |
| `src/rlw_icon_win.c/.h` | That icon on the window and taskbar (`WM_SETICON`); the one time the `.ico` is made, a Start menu shortcut (`Rising Lands.lnk`) and a question about a Desktop shortcut; Jogo → Criar atalho na Área de Trabalho later |
| `src/rlw_reimport.c/.h` | Jogo → Importar o CD novamente…, portable (port of the macOS player's `reimportCD`): the relaunch command line, the rule of the one-time notice for data imported before the sound |
| `src/rlw_reimport_win.c/.h` | The re-import on Win32: the CD is chosen in the running game, which closes the sound host and starts again with `--cd <image> --reimport`, so the import runs before anything opens a file in `Dados`; a failed re-import keeps the installed data |
| `src/rlw_dpi.c/.h` | DPI mixed mode: the game window per-monitor aware (sharp picture and real fullscreen resolution on scaled screens), fixed-layout windows unaware; the fullscreen fit to the monitor |
| `src/rlw_gamemenu.c/.h` | The Jogo menu, portable (port of the macOS player menu): item ids, labels, enabled/checked state, decoded commands, the Opções… request during play |
| `src/rlw_gamemenu_win.c/.h` | The Jogo popup on the Win32 menu bar |
| `src/rlw_savemgr.c/.h` | Saved games in the folder, portable (port of `NativeSaveLibrary.swift`): slots in Carregar partida order with their companions, names, stage and difficulty, rename, Recycle Bin through an injected mover |
| `src/rlw_savemgr_dialog.c/.h` | The "Partidas salvas" window (Win32, port of `NativeSaveManager`): list, Renomear…, Apagar… to the Recycle Bin (`SHFileOperationW`, `FOF_ALLOWUNDO`, warns before anything that cannot be recycled) |
| `src/rlw_autosave.c/.h` | Periodic autosave, portable (port of `NativeAutosaveInterval`/`NativeAutosaveSchedule`): game-time interval, eligibility, retry and restart rules, the `autosave=` option and the Carregar partida caption |
| `src/rlw_sound.c/.h` | The CD's A/B/P `.HED`/`.SND` sound banks, samples decoded in memory (IMA ADPCM, plain PCM), keyed like `MacOSAudioMapper` |
| `src/rlw_sound_import.c/.h` | Sound import after the CD import: `INSTALHD/SOUND/{A,B,P}.{HED,SND}` into `Dados/sound`, SHA-256 pinned, staging folder |
| `src/rlw_audio.c/.h` | Portable mixer (64 voices, DirectSound-style volume, loops, music stream input) and the play-mode sound host over `macos/src/macos_audio_mapper.c`, reused unchanged (port of `NativeAudioPlayback`); the Áudio settings |
| `src/rlw_audio_win32.c/.h` | `waveOut` output (winmm only) and the driver pump; no device = real-time sink, and a device that appears is picked up |
| `src/rlw_audio_ui.c/.h` | The "Áudio" menu, its settings in `opcoes.ini`, the sound and music import after the CD import, the event picture and movie hooks of the window |
| `src/rlw_cdmusic.c/.h` | CD music (port of `NativeCDMusicDisc/Host/Player`): the CUE track table, tracks 2 and 3 imported from the player's raw `.BIN` as pinned WAVs, a streaming player, the macOS selection policy |
| `src/rlw_eventflc.c/.h` | In-game FLC event pictures (port of `NativeEventFLCPlayback` and `macos_event_flc.inc`): the decoded frame over the view at the file's speed, the event loop sound tick, one `rl_evento_flc_terminar_v1` after the last frame |
| `src/rlw_movie.c/.h` | Original movies (port of `NativeDefeatMovie.swift`): RIFF/AVI, Cinepak and Microsoft ADPCM decoded by the host (no system codec) |
| `src/rlw_menu.c/.h` | Menu input routing, portable: keys on release, coalesced moves, one page per click, request dispatch (port of `NativeLiveWindow.swift`'s shell paths) |
| `src/rlw_menu_ops.c/.h` | The Windows driver behind that routing |
| `src/rlw_menu_demo.c/.h` | The fixed scripted walk through the menus (smoke and parity) |
| `src/rlw_flc.c/.h` | FLC/FLI decoder for the credits (port of `NativeFLC.swift`) |
| `src/rlw_cd.c/.h` | First-run import: ISO 9660 over raw 2352-byte BIN (MODE1/MODE2, via `.CUE`), `.ISO` or a CD folder; SHA-256 verification; writes `%LOCALAPPDATA%\RisingLands-ptbr\Dados` |
| `src/rlw_fs.c/.h`, `src/rlw_sha256.c/.h` | UTF-8 file helpers (wide Win32 APIs; POSIX for tests) and a small SHA-256 |
| `src/rlw_demo.h` | The fixed synthetic input used by smoke and parity runs |
| `tests/test_cd.c` | Portable unit tests: SHA-256 vectors and a synthetic ISO 9660 image as ISO, BIN MODE1 via CUE, BIN MODE2 and folder; import success, pinned-hash refusals, atomic replacement |
| `tests/test_menu.c` | Portable unit tests: menu routing against a simulated driver, FLC decoding of a synthetic file |
| `tests/test_hotkeys.c` | Portable unit tests: bindings, router (double tap, aliases, modern keys on/off), snapshot over a synthetic memory map, planner and job |
| `tests/test_markers.c` | Portable unit tests: the order queue marker reader over a synthetic memory map (the macOS test case), limits, the aspect-fit layout |
| `tests/test_savemgr.c` | Portable unit tests: saved-games list/rename/trash and the periodic autosave schedule |
| `tests/test_gamemenu.c` | Portable unit tests of the Jogo menu model |
| `tests/test_icon.c` | Portable unit tests: the icon reader over a synthetic PE with a real 32x32 icon (on Windows the written `.ico` is loaded with `LoadImageW`) |
| `tests/test_reimport.c` | Portable unit tests: the re-import command line and the notice rule |
| `tests/test_audio.c` | Portable unit tests: synthetic IMA ADPCM and banks, mixer and rendered WAV statistics, the sound host over the mapper, sound import, settings; synthetic CUE/BIN for the CD music import, player and policy; a synthetic FLC event; a synthetic Cinepak/MS ADPCM AVI |
| `tests/parity.c`, `parity.sh` | Driver parity: the Windows driver vs the macOS driver over the same engine, per-tick frame hashes and the scripted menu walk |
| `tests/audio_parity.c`, `tests/movie_parity.swift`, `audio_parity.sh` | Sound parity: the engine's sound and CD music requests through both drivers, the decoded samples, a rendered WAV, and every movie frame and sample against the macOS decoder |
| `tests/import_cli.c` | `rlw-import`, the same importer as a portable CLI |
| `../.github/workflows/windows-client.yml` | GitHub Actions workflow: jobs `mingw-x64` and `msvc-x64` on `windows-latest`; each build uploads its `.exe` with a SHA-256 |
| `ci/smoke.sh` | Smokes without game data for any built `.exe`: `--selftest`, usage, the importer's refusals (no CD, a non-CD file, broken `.CUE`s, a folder without `INSTALHD`, a fake CD with the right names) with their messages and no `Dados` left behind, `--smoke-markers`, `--smoke-window`, `--smoke-audio`, `--smoke-import-prompt` (`--no-ui` skips the last) |
| `build.sh` | Engine static library + host executable with mingw-w64 (+ `test`) |
| `build-msvc.ps1` | The same with MSVC x64 (+ `-Test`) |

## Build

Requirements: Rust stable (the repository's `engine/rust-toolchain.toml`), the `x86_64-pc-windows-gnu` target and
mingw-w64.

From macOS (Homebrew) or Linux:

```sh
rustup target add x86_64-pc-windows-gnu
brew install mingw-w64            # Debian/Ubuntu: apt install gcc-mingw-w64-x86-64
windows/build.sh             # -> windows/build/RisingLands.exe
windows/build.sh test        # also builds and runs the portable tests (tests/test_*.c) with the host compiler
```

On Windows, from Git Bash or MSYS2 with mingw-w64 `gcc` on `PATH`:

```sh
rustup target add x86_64-pc-windows-gnu
CC=gcc HOST_CC=gcc windows/build.sh test
```

Variables: `TARGET` (Rust target), `CC` (Windows C compiler), `OUT` (output folder, default `windows/build`),
`HOST_CC` (compiler for the unit tests). The engine is built with `cargo rustc --release --locked -p rl-ffi --lib
--crate-type staticlib`, so no Rust linker for Windows is needed; mingw links the final `.exe`.

### MSVC (x64)

On Windows with Visual Studio 2022 or the Build Tools (C++ workload) and rustup, from PowerShell:

```powershell
windows\build-msvc.ps1 -Test            # x64: tests with cl.exe, then -> windows\build\msvc-x64\RisingLands.exe
```

The script finds `vcvarsall.bat` with `vswhere` when `cl.exe` is not on `PATH`, adds the Rust target
(`x86_64-pc-windows-msvc`), builds the engine with `-C target-feature=+crt-static`
and the host with `cl /std:c11 /utf-8 /MT` (the C sources carry PT-BR UTF-8 messages), links `rising_engine.lib` with
the Win32 libraries and the Rust std's native set (`kernel32 ntdll userenv ws2_32 dbghelp`), and fails if
`dumpbin /dependents` shows anything besides system DLLs (no vcruntime/UCRT redistributable). With `-Test` it builds
every portable test program with `cl /W3 /WX` (C4244/C4267 off, the gcc build's `-Wall -Wextra` has no
`-Wconversion` either) and runs them when the target matches the machine. `-Out` changes the output folder.

### Code signing

The binaries are not signed, and the project does not plan to sign them: Windows SmartScreen warns on the first run.
Each CI artifact carries its SHA-256 and the commit it was built from (`docs/WINDOWS_CLIENT.md`, section 6).

## Run

```text
RisingLands.exe [--cd CAMINHO] [--data PASTA] [--mission MONDE01.ETA] [--difficulty 0|1|2] [--speed 0|1|2|3] [--fullscreen]
```

- Without `--mission` the game opens in the original main menu (program start, `MenuInit 2`); `--mission` starts that
  stage directly, as milestone 1 did.
- First run: a dialog asks for the player's CD (`RISING_LANDS_BR_.BIN`, the `.CUE` or an `.ISO`); `--cd` also accepts
  the CD folder. A raw BIN is checked whole against the Brazilian image's SHA-256
  (`c2c09e87…a616f`, `src/rlw_cd.h`); `RISING.EXE`, `PLATGAME.PIE` and `EVENTS.INI` are checked by SHA-256 for every
  source. The import goes to `%LOCALAPPDATA%\RisingLands-ptbr\Dados` through a staging folder that is renamed in only
  after every file verified, with a receipt `importacao.json` (schema `rising-lands/windows-import/2`; a milestone-1
  import lacks the menu files and asks for the CD again). Besides the stage inputs it copies `MENU.CMP`,
  `INVENTE.CMP`, the backgrounds `TITRE.PIE`, `PAGEMIS.PIE`, `PAGEMIS2.PIE`, `CREDITS.PIE`, `ECRANFIN.LZS`, the
  briefings `WORLD/MONDE*.TXT`, `EVENTS/*.FLC` to `events/` and `EVENTS/*.AVI` to `movies/`.
- Host keys (never delivered to the game): **P** or **Pause** pauses, **+**/**−** change speed (0.5x/1x/2x/4x =
  15/30/60/120 frames/s), **F11** toggles fullscreen. Alt+F4 closes.
- Game keys: exactly the macOS host's 23 virtual keys (13, 16, 17, 18, 27, 32, 37–40, 46, 88, 48–57, 9) with the
  original scan codes and `lParam` bits; every other key is dropped. Auto-repeat is ignored; left/right Shift,
  Control and Alt map to the single VK 16/17/18 channels. The other 20 channels go through the bindings
  (`src/rlw_hotkeys.c`, the macOS app's actions and rules): by default each is its own key with optional modifiers,
  so the game receives exactly what it did before the bindings existed.
- "Atalhos modernos" (Opções; on by default, as on macOS), as on macOS (`NativeModernHotkeys.swift`): the same digit
  twice within 0.35 s centres on that group (the original's Shift+N), **Home** the last alert (the original's X),
  **.** the next idle worker, **,** the next idle soldier, **H** the sanctuary (cycling through each). Each becomes
  ordinary original input through the player's own path, planned from a read-only snapshot (`rlw_driver_peek`)
  between frames: a right press on the map first if something is selected, a minimap press to bring the target into
  view, then a left click on it, each stage verified on a fresh snapshot; the pointer returns to the player's. A
  command waits for a held left button, lasts at most 300 frames, is dropped in menus, while paused or stopped, and
  lets the player's mouse only move the remembered pointer while it clicks. Exact chords: Shift/Control/Alt+Home
  etc. stay free. With the item off nothing changes: the four keys are not handled and digits are ordinary taps.
- Order queue markers, as on macOS (`NativeOrderQueueMarkers`): in a world created with the Shift+click order queue
  rule (`RL_REGRA_FILA_ORDENS`, `regras` bit 1, on with "Atalhos modernos"), the pending waypoints of the local
  player's selected units (at most 64 units, 8 waypoints each) are drawn by the host over the map part of the picture:
  a dashed line through each unit's waypoints and, on every waypoint, a pole, a flag and a dot in the macOS colour
  (255, 217, 51), scaled with the aspect-fit and clipped to the map (never over the HUD). Read-only, every 4 frames,
  only in play (no menu, not paused or stopped) and only while some unit has a waypoint; the picture is composed off
  screen so a frame never shows without its markers. Never part of the game image, a save, `--hashes` or a
  screenshot of a smoke run (those keep every option off).
- Mouse: `0x200`–`0x205` in original pixels; a left press is held across frames until its release (drag selection,
  `MK_LBUTTON`), never released before the k/k+1/k+3 click gesture; right clicks keep the fixed gesture; Shift/Control
  become `MK_SHIFT`/`MK_CONTROL`. Clicks on the letterbox bars are ignored; moves and held drags clamp to the nearest
  surface pixel, so in fullscreen the screen edge reaches the original's 2-px edge-scroll zones. Focus loss releases a
  held press and held keys and pulls an edge pointer 2 px inward. The system cursor is hidden over the game (the game
  draws its own).
- High-DPI screens: the game window is per-monitor DPI aware (`src/rlw_dpi.c`, Windows 10 1703+): it opens with an
  800x600 client area at 100 % scale, i.e. larger on a scaled screen, the picture is stretched once into real device
  pixels (sharp) and fullscreen uses the monitor's real resolution; it follows a move to a monitor with another scale
  and, in fullscreen, a resolution change. The "Atalhos de teclado" window keeps its fixed layout and is scaled by
  Windows as before; message boxes, the file dialog and template dialogs scale themselves.

### Menus

The prepared shell (`engine/include/rising_shell.h`) draws every original screen; the host only routes input and
completes requests, like the macOS app:

- Keyboard: Enter, Up, Down and Escape act on release (the original's completed key presses). On the briefing and
  objective pages Up/Down (and the mouse wheel) click the original arrow zones, one page per press.
- Mouse: moves are coalesced to at most 60 per second; presses and releases go through at once; a page turns once per
  click (the original turns while the button is down on a clock multiple of 10). The system pointer stays visible.
- Ticks: the shell's virtual counters advance 20 times per second (100 in the credits, whose FLC runs on the host's
  clock).
- Requests: SHOW, the pause menu's save/load lists, "Novo jogo", "Carregar jogo", resume, objectives, restart, next
  stage after a victory (MISSION_NEXT, then the briefing's START_MISSION) and credits are completed through the driver.
  "Sair" closes the program. "Multijogador" reports that network play is not part of this version and reopens the
  main menu, as on macOS.
- Movies: the introduction, defeat and campaign-end requests play the original AVI (see Audio below); a movie that
  cannot be opened completes as before (unavailable; the introduction as finished when its file exists), as the macOS
  host does.

### Opções

The port's "Opções" screen (not in the original; `engine/include/options-screen.md`) is drawn by the engine with the
original menu material (TITRE.PIE, MENU.CMP, the menu fonts), the same screen as on macOS. An "OPÇÕES" caption on the
wooden floor of the main and pause menus opens it; only a click that starts on it is taken, every other input reaches
the original shell unchanged. Up/Down/Enter/Escape and Left/Right (only while it is open) or the mouse choose:
"Atalhos modernos" (the modern shortcuts below, and the Shift+click order queue rule, `RL_REGRA_FILA_ORDENS`, for
worlds created afterwards), "Caminhos melhores" (the engine's modern unit paths,
`RL_REGRA_CAMINHOS_MELHORES`, for worlds created afterwards), difficulty (next stage start) and speed (at once, like `+`/`-`). "Configurar teclas" opens the "Atalhos de teclado"
window (below). The settings live in `opcoes.ini` beside `Dados` and `Partidas` (`atalhos`, `caminhos`,
`dificuldade`, `velocidade`); `--difficulty`/`--speed` override them for one run. Defaults when a key is missing
are the macOS app's: Atalhos modernos on, Caminhos melhores off, Normal, 1x (since 2026-11-12; before, Atalhos
modernos was off on Windows). The two rule items work as on
macOS (`NativeEngineRules.current()`): every world created afterwards (menu or stage) gets `regras` = bit 1 for
"Atalhos modernos" | bit 0 for "Caminhos melhores", and a change also reaches the main-menu world before its load tail
(`rl_regras_definir`); a running stage and a loaded save keep the rules they were made with. Smoke and demo runs neither read
nor show them (every option stays off there), except the menu walk, which opens and closes the screen on both
drivers. `parity.sh` ends with the rule check (`tests/parity.c rules`): for each combination of the two items the
stage world after its first frame and the menu world have exactly the expected `regras` (`rl_regras`), the menu world
takes a change before its load tail, and the default stays 0.

### Atalhos de teclado

"Configurar teclas" (Opções screen) or **Jogo → Configurar teclas…** opens the port of the macOS app's key window
(`NativeHotkeyPreferences.swift`): one row per action (Enter, Esc, Espaço, the four arrows, Delete, X, Tab, 0–9 and
the four modern commands), each with its current shortcuts. Alterar replaces the selected alternative, Adicionar
alternativa appends one: the next key combination pressed in the capture field is taken (Enter, Esc and Tab
included; modifier keys alone are not a combination). Remover, Cancelar captura, Restaurar padrões, Cancelar and
Aplicar work as on macOS, with its messages. Every action reaches the game only as one of the same 23 virtual keys
(the 20 channels; Shift/Control/Alt stay independent channels), so no binding can send the game a key the original
does not accept. A new combination is exact (no optional modifiers); a combination that overlaps another action's is
refused and named; the host's own keys (P, Pause, + and −, F5, F9, F11, Alt+F4) and modifier/lock/system keys cannot
be used. Changes are a draft until Aplicar, which writes `teclas.ini` beside `opcoes.ini` (temporary file + atomic
replace) and only then installs the set; a failed save changes nothing. `teclas.ini`: `versao=1`, then
`<action>=<vk>/<mods>/<optional> …` per action (mods: 1 Shift, 2 Ctrl, 4 Alt; an empty value unbinds), the action
names of the macOS file; an action missing from the file takes its default unless that key is taken; an invalid file
is reported on the console and the defaults stay. The game window is disabled while the key window is open. Key
names come from the current keyboard layout for character keys. Smoke and demo runs neither read nor offer it.

### Audio, event pictures and movies

Ports of the macOS app's audio, CD music, event FLC and movie code; presentation only, never game state, and off in
smoke, demo and parity runs.

- Import: after the CD import, the six sound banks `INSTALHD/SOUND/{A,B,P}.{HED,SND}` are copied unchanged into
  `Dados/sound` (each checked by size and SHA-256, staging folder), and, from a raw `.BIN` (with its `.CUE`), CD audio
  tracks 2 and 3 are written as `Dados/music/TRACK02.wav` and `TRACK03.wav` (exact 44.1 kHz stereo PCM, pinned to the
  macOS player's WAV identity). Music comes only from the player's own `.BIN`; an `.ISO` or a folder has no CD audio. A
  failure leaves the game playable without sound or music. An import from pre-alphas 1–3 has neither: use **Jogo →
  Importar o CD novamente…** (the game says so once at start, and the Áudio menu says why sound or music is missing);
  from the command line, `--cd <image> --reimport` does the same.
- Sound effects: the engine's sound requests go through `macos/src/macos_audio_mapper.c` (reused unchanged) into a
  64-voice mixer, output with `waveOut` (winmm only). Samples are decoded in memory from the banks. Without a sound
  device, or when it is unplugged, the game goes on silently and picks up a device that appears (or a new default
  device) within about 3 s.
- CD music: the game's MCICD_Play requests play the requested original track once, with the macOS policy (a stage
  start re-arms, the result screen keeps the track, leaving the stage, a movie or the credits stop it).
- **Áudio** menu: Silenciar efeitos, Volume dos efeitos (100/75/50/25 %), Silenciar música, Volume da música; kept in
  `opcoes.ini` (`efeitos_volume`, `efeitos_mudo`, `musica_volume`, `musica_mudo`); it shows why sound or music is
  unavailable.
- In-game FLC events: the decoded FLC frame is shown centred over the view with its own palette at the file's speed,
  with the event's sound ticks, and the event ends once after the last frame (an FLC that cannot be shown still ends
  at once, as before).
- Movies: the introduction, defeat and campaign-end AVIs (Cinepak video, Microsoft ADPCM audio) are decoded by the host
  itself (no system codec), shown aspect-fit with the CD music stopped, and end at their end or on the Escape key-up
  (the mouse does not end them), as in the original.

### Saves

Same files, names and checks as the macOS app, so a `Partidas` folder can be copied between the two
(`src/rlw_saves.h`, `src/rlw_save.inc`):

- Folder: `%LOCALAPPDATA%\RisingLands-ptbr\Partidas` (`--save-directory` to change it). `posicao-1`…`posicao-10.rls`
  are the named positions (the original lists show them as `save1.sav`…`save9.sav`, `save0.sav`), `rapido.rls` the
  quick save, `inicio-fase.rls` the stage-start autosave, `nomes.plist` the position names. Each slot file holds
  exactly the engine's save bytes; its presentation (picture, terrain cache, pointer, stage and difficulty) is the
  companion `<slot>.<sha256>.view`.
- "Jogo" menu: **Salvar rápido** (F5), **Carregar rápido** (F9), **Salvar partida…** (positions 1–5, asks for a
  name), **Carregar partida** (positions 1–5 and "Início da fase · salvamento automático"). F5/F9 are host keys.
- The pause menu's Salvar (list 14) saves the paused game and resumes it; its Carregar (list 12) and the main menu's
  "Carregar jogo" (list 13) load a position. The lists show "<name> · dd/MM HH:mm" for existing positions.
- A save is captured between complete frames (held keys are released first; while their releases or a held button
  are waiting, it retries every 80 ms) and written on a background thread: companion first, then the slot through a
  temporary file, `FlushFileBuffers` and an atomic replace, so a failure keeps the previous save. Loading waits for a
  save still being written and replaces world, renderer and picture together; a game saved in the pause menu loads
  with its picture and continues playing, as the original lists do (the host opens the pause session on the loaded
  world and activates "Voltar ao jogo", then resumes: `rlw_driver_load_resume`, port of `macos_live_load_resume`). A
  failed load from an original list reopens the main menu, as on macOS.
- The autosave is written after each stage's first frame (start, restart, next stage). Frame-count smoke runs write
  no saves unless `--save-directory` is given.
- **Jogo → Gerenciar partidas salvas…** (list with name, stage and date; Renomear… for positions 1–10, whose names the
  original lists show; Apagar… moves the slot and its `.view` to the Recycle Bin, never a permanent delete; the game
  waits while it is open) and **Jogo → Salvamento automático** (off by default; every 5, 10 or 15 minutes of game time,
  counting only frames of the game in progress, so it follows the speed and pauses/menus count nothing; its own slot
  `autosave`, listed in Carregar partida; `autosave=` in `opcoes.ini`). The host links `comctl32` (list view).

### Jogo menu

The menu-bar popup is **Jogo**, the macOS player app's name (`src/rlw_gamemenu.c`): Pausar/Continuar (P), Velocidade
(0,5x–4x, Mais rápido +, Mais devagar −), Dificuldade (next stage start), the save items above, Opções… (in play it
pauses and opens the screen over the pause), Configurar teclas…, Atalhos modernos and Caminhos melhores (checked when
on), **Importar o CD novamente…** (asks for the CD image, then the game restarts itself to import it: game files,
sound banks, CD music and the icon) and **Criar atalho na Área de Trabalho**. The **Áudio** popup sits beside it.

### Icon and shortcuts

The window, the taskbar and the shortcuts show the game's original icon. It is not in the repository or the download:
at run time `src/rlw_icon.c` reads the first icon group of the player's imported `RISING.EXE` and writes it as
`rising.ico` in the data folder (`%LOCALAPPDATA%\RisingLands-ptbr`). The one time that file is made, the game puts a
`Rising Lands` shortcut in the Start menu and asks once whether to put one on the Desktop; **Jogo → Criar atalho na
Área de Trabalho** makes it later. Frame-count and headless runs make no shortcuts.

Test switches: `--selftest` (engine ABI versions, no data), `--import-only`, `--frames N` (run N frames as fast as
possible in the window, then exit; with `--screenshot out.bmp` saves the presented client area), `--headless` (no
window), `--hashes out.txt` (one line per tick: frames, screen, SHA-256 of pixels and palette), `--demo-input` (the
fixed synthetic input of `src/rlw_demo.h`), `--no-verify-image`, `--menu-demo out.txt` (no window: the scripted menu
walk of `src/rlw_menu_demo.c`, one line per step), `--save-demo out.txt [--save-directory EMPTY]` (no window: the
scripted save walk, writing in its own empty folder, never in the player's saves), `--smoke-markers` (no data:
paints a synthetic picture with order queue markers through the real GDI path into a memory DC at 1:1, 2x letterboxed
and a small size, and checks flags, dots, dashes, the clip under the map and that nothing spills outside the picture;
exit 0/1), `--smoke-window` (no data: a per-monitor-aware and an unaware window, the 800x600 client at the window's
DPI and the borderless fullscreen rect of its monitor; exit 0/1), `--smoke-audio` (no data: the sound host starts and
plays a synthetic buffer, with or without a sound device; exit 0/1), `--smoke-import-prompt --data EMPTY` (no data: the
player's first start, whose import notice and CD file dialog must open; a watcher thread closes each as Cancel would;
exit 0 only if both opened and no `Dados` was created; a watchdog ends it with 9 after 60 s). `--frames`/`--headless`
without `--mission` start MONDE01 as in milestone 1.

## Tests

| Test | Needs | What it shows |
|---|---|---|
| `windows/build.sh test` | any OS with a C compiler | SHA-256, ISO 9660 reader over ISO/BIN MODE1+CUE/BIN MODE2/folder, import (menus, briefings, movies), refusals, atomic replacement; menu routing; FLC decoding; save folder (durable replace, `nomes.plist`, list captions, CP1252); saved-games manager and autosave schedule; Jogo menu model; bindings, router, modern shortcuts; order queue markers (reader, layout); sound banks, mixer, sound host, CD music import/player/policy, event FLC, movie decoding (all synthetic); the icon reader over a synthetic PE; the re-import command line and notice rule |
| `windows/build.sh` | mingw-w64 | the engine and host build for Windows; `objdump -p` shows only system DLL imports |
| `windows/parity.sh --cd …/RISING_LANDS_BR_.cue 300` | macOS + the player's CD | the Windows driver and the macOS driver give identical per-tick frame hashes with the same input (`PARIDADE OK`), then identical lines for the scripted menu walk: main menu, Novo jogo, briefing page, stage start, 150 frames, pause, objectives page, Enter back to the pause, Escape to resume, pause, restart, 60 frames (`PARIDADE DOS MENUS OK`); then the save walk: stage autosave, 120 frames, save, 60 frames, load and the same 60 frames (compared), autosave load and its first 30 frames (compared), the pause list's save over the same slot, the pause list's load of that paused save, the reopened pause, continue, 20 frames; the lines carry the SHA-256 of every slot and `.view` file, so both drivers must write identical files (`PARIDADE DOS SAVES OK`) |
| `windows/audio_parity.sh --cd …/RISING_LANDS_BR_.cue 600` | macOS + the player's CD | the sound and CD music requests of the Windows and macOS drivers are identical tick by tick in play and over the menu walk, the play run holds sound requests and renders to a non-silent, unclipped WAV, the music tracks match their pins, and every frame and sample of the three original AVIs decoded by `rlw_movie.c` equals the macOS decoder's (needs `swiftc`) (`SONS OK`). `--build-only` builds without data |
| `RisingLands.exe --selftest` | Windows, wine or CI | engine ABI/extension versions inside the Windows build |
| `ci/smoke.sh RisingLands.exe` | Windows (Git Bash), wine or CI | every case above without game data: self-test, usage, six import refusals with their messages and nothing left in the data folder, the GDI marker paint, the DPI mixed mode and fullscreen rect, the audio start, the first start's import dialogs |
| `RisingLands.exe --headless --cd … --frames 300 --demo-input --hashes w.txt` | Windows/wine + CD | MONDE01 runs; the hashes must equal `parity.sh`'s `windows.txt` (cross-platform determinism) |
| `RisingLands.exe --cd … --frames 300 --screenshot s.bmp` | Windows/wine + CD | presentation (palette, aspect-fit) |
| `RisingLands.exe --cd … --menu-demo m.txt` | Windows/wine + CD | the menu walk on Windows; must equal `parity.sh`'s `windows-menu.txt` |
| `RisingLands.exe --cd … --save-demo s.txt` | Windows/wine + CD | the save walk on Windows (real Win32 file writes); must equal `parity.sh`'s `windows-save.txt` |
| `.github/workflows/windows-client.yml` | GitHub Actions | two builds (mingw x64, MSVC x64), unit tests through the Win32 file APIs with gcc and with cl, `ci/smoke.sh` on both, each `.exe` uploaded with its SHA-256 |
| `build-msvc.ps1 -Test` | Windows + Visual Studio | the portable tests with cl.exe; the MSVC build imports only system DLLs |

Wine on macOS: the Homebrew `wine-stable`/`wine@staging` casks are disabled since 2026-09-01 because they fail
Gatekeeper, so the client was not tried under wine on macOS; on Linux, `wine RisingLands.exe …` runs the same switches.

## Faithful mode

The host never writes game state. It calls the engine exactly like the macOS host in play mode
(`RL_EXECUCAO_JOGO`): `rl_missao_iniciar_eventos_v1` with the same `RlMissaoInicio` (fill 0, AI clock `0x123456`,
thread 1, 20 ticks/s, counters 5/2/1, cursor 400,300), `rl_aplicar_dificuldade`, `rl_eventos_reassociar_v1`,
`rl_eventos_flc_v1`, `rl_game_hud_background_initialize`, `rl_render_criar_cd`, `rl_mundo_paleta_jogo`; then
`rl_load_tail_entrada_v1`, the first draw and `rl_autosalvar_inicio_fase`; then per tick `rl_missao_tela`,
`rl_render_geracao` + `rl_render_quadro`, and `rl_passo_quadro_entrada` for phases 2 and 3 with the queued messages
and held keys. Speed only changes how many ticks run per second.

Menus follow `macos_campaign_shell.inc`: the program start is `rl_missao_iniciar_menu_eventos_v1` (the first nine
descriptors), and a session is `rl_shell_create` over the current world with the eleven assets (six CMP banks,
`PLATGAME.PIE`, the shadow/lighting tables from `rl_mundo_tabelas_sombra_luz`, `MENU.CMP`, `INVENTE.CMP`), the shell
counters read from the world (`0x508f7c`, `0x508f84`, cursor `0x5099a8`), and backgrounds decoded from the CD files
by `rl_shell_imagem_cd`. The pause keeps the gameplay presentation and resumes through `rl_shell_resume` with a
renderer rebuilt from it; a stage start goes through `rl_shell_campaign_load_dificuldade` on a candidate that runs
the load tail and first picture before it replaces the current world. Briefing texts and the stage path are the
exact original CD-root byte strings (`0x509594`, `0x498430`, `0x494b27`, `0x494b36`, `0x497154`).

Saves follow `macos_live_driver.c`: `rl_salvar_bytes` (with the current frame input) or `rl_autosalvar_bytes`, the
first draw of the save boundary except in the pause (mode 16 keeps its retained picture), `rl_render_cache` for the
companion; loading is `rl_carregar_arquivo`, `rl_execucao_modo` and `rl_render_geracao` checks (play mode, phase 1 or
3), `rl_eventos_reassociar_v1`, the FLC table, a renderer reset from the companion and `rl_render_quadro` (or, for a
paused save, the saved picture with `rl_mundo_paleta_jogo`). A loaded world with a saved input queue drains it with
`rl_entrada_passo_pendente` to the next complete frame before new host input. The pause list's SAVE_SLOT saves first,
then resumes through `rl_shell_resume`; the save list slots are the exact original paths behind `0x497128`.

## Known gaps (next milestones)

- Position names only for 1–5 in the Jogo menu (6–10 through the original lists and the saved-games manager), as on
  macOS.
- A D3D11 presenter. ARM64 builds (x86-64 only since 2026-11-14).
- Nobody has played a build on a real Windows PC yet (#6), sound and movies included; CI covers builds, unit tests
  and the smokes above, not a played game.
