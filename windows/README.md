# Rising Lands for Windows (milestones 1–3)

> **Resumo em português.** Cliente Windows sobre o mesmo motor em Rust e a mesma ABI em C (`engine/include`) do app
> macOS. Este primeiro marco importa o CD brasileiro do jogador (imagem `.BIN`/`.CUE`, `.ISO` ou pasta), roda a
> MONDE01 a 30 quadros/s no 1x, mostra o quadro 800x600 com a paleta do jogo (com aspect-fit e tela cheia), entrega
> mouse e teclado pelo mesmo caminho do macOS (as mesmas mensagens, as mesmas 23 teclas, F11 nunca entregue) e
> pausa. Stack: Win32 + GDI em C, motor ligado estaticamente, um único `RisingLands.exe` sem DLLs de terceiros.
> Compila sem máquina Windows (mingw-w64). O marco 2 abre no menu principal original e roda os menus pelo shell
> preparado do motor, como o macOS: "Um jogador", "Novo jogo", briefing e objetivos (páginas por clique, setas ou
> roda do mouse), pausa no jogo (Esc: continuar, objetivos, abandonar, reiniciar a missão), tela de resultado com a
> próxima fase, créditos (FLC decodificado) e "Sair". O marco 3 traz os saves, com os mesmos arquivos do macOS:
> menu "Partida" (salvar rápido F5, carregar rápido F9, posições com nome, início da fase), as listas originais de
> salvar e carregar na pausa e "Carregar jogo" no menu principal, e o salvamento automático no início de cada fase.
> Áudio e filmes ficam para os próximos marcos (os filmes contam como indisponíveis).
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
  with MSVC (`cl.exe`) for x64 and ARM64.
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
| `src/rlw_menu.c/.h` | Menu input routing, portable: keys on release, coalesced moves, one page per click, request dispatch (port of `NativeLiveWindow.swift`'s shell paths) |
| `src/rlw_menu_ops.c/.h` | The Windows driver behind that routing |
| `src/rlw_menu_demo.c/.h` | The fixed scripted walk through the menus (smoke and parity) |
| `src/rlw_flc.c/.h` | FLC/FLI decoder for the credits (port of `NativeFLC.swift`) |
| `src/rlw_cd.c/.h` | First-run import: ISO 9660 over raw 2352-byte BIN (MODE1/MODE2, via `.CUE`), `.ISO` or a CD folder; SHA-256 verification; writes `%LOCALAPPDATA%\RisingLands-ptbr\Dados` |
| `src/rlw_fs.c/.h`, `src/rlw_sha256.c/.h` | UTF-8 file helpers (wide Win32 APIs; POSIX for tests) and a small SHA-256 |
| `src/rlw_demo.h` | The fixed synthetic input used by smoke and parity runs |
| `tests/test_cd.c` | Portable unit tests: SHA-256 vectors and a synthetic ISO 9660 image as ISO, BIN MODE1 via CUE, BIN MODE2 and folder; import success, pinned-hash refusals, atomic replacement |
| `tests/test_menu.c` | Portable unit tests: menu routing against a simulated driver, FLC decoding of a synthetic file |
| `tests/parity.c`, `parity.sh` | Driver parity: the Windows driver vs the macOS driver over the same engine, per-tick frame hashes and the scripted menu walk |
| `tests/import_cli.c` | `rlw-import`, the same importer as a portable CLI |
| `../.github/workflows/windows-client.yml` | GitHub Actions workflow: jobs `mingw-x64`, `msvc-x64`, `msvc-arm64` on `windows-latest` and `arm64-run` on `windows-11-arm`; each build uploads its `.exe` with a SHA-256 |
| `ci/smoke.sh` | `--selftest` and the refusal of a file that is not the CD, for any built `.exe` |
| `build.sh` | Engine static library + host executable with mingw-w64 (+ `test`) |
| `build-msvc.ps1` | The same with MSVC, x64 or ARM64 (+ `-Test`) |

## Build

Requirements: Rust stable (the repository's `engine/rust-toolchain.toml`), the `x86_64-pc-windows-gnu` target and
mingw-w64.

From macOS (Homebrew) or Linux:

```sh
rustup target add x86_64-pc-windows-gnu
brew install mingw-w64            # Debian/Ubuntu: apt install gcc-mingw-w64-x86-64
windows/build.sh             # -> windows/build/RisingLands.exe
windows/build.sh test        # also builds and runs tests/test_cd.c and tests/test_menu.c with the host compiler
```

On Windows, from Git Bash or MSYS2 with mingw-w64 `gcc` on `PATH`:

```sh
rustup target add x86_64-pc-windows-gnu
CC=gcc HOST_CC=gcc windows/build.sh test
```

Variables: `TARGET` (Rust target), `CC` (Windows C compiler), `OUT` (output folder, default `windows/build`),
`HOST_CC` (compiler for the unit tests). The engine is built with `cargo rustc --release --locked -p rl-ffi --lib
--crate-type staticlib`, so no Rust linker for Windows is needed; mingw links the final `.exe`.

### MSVC (x64 and ARM64)

On Windows with Visual Studio 2022 or the Build Tools (C++ workload; the ARM64 build tools for ARM64) and rustup,
from PowerShell:

```powershell
windows\build-msvc.ps1 -Test            # x64: tests with cl.exe, then -> windows\build\msvc-x64\RisingLands.exe
windows\build-msvc.ps1 -Arch arm64      # ARM64 cross build (native on an ARM64 machine) -> build\msvc-arm64\
```

The script finds `vcvarsall.bat` with `vswhere` when `cl.exe` for that architecture is not on `PATH`, adds the Rust
target (`x86_64-pc-windows-msvc` / `aarch64-pc-windows-msvc`), builds the engine with `-C target-feature=+crt-static`
and the host with `cl /std:c11 /utf-8 /MT` (the C sources carry PT-BR UTF-8 messages), links `rising_engine.lib` with
the Win32 libraries and the Rust std's native set (`kernel32 ntdll userenv ws2_32 dbghelp`), and fails if
`dumpbin /dependents` shows anything besides system DLLs (no vcruntime/UCRT redistributable). With `-Test` it builds
the three portable test programs with `cl /W3 /WX` (C4244/C4267 off, the gcc build's `-Wall -Wextra` has no
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
  Control and Alt map to the single VK 16/17/18 channels.
- Mouse: `0x200`–`0x205` in original pixels; a left press is held across frames until its release (drag selection,
  `MK_LBUTTON`), never released before the k/k+1/k+3 click gesture; right clicks keep the fixed gesture; Shift/Control
  become `MK_SHIFT`/`MK_CONTROL`. Clicks on the letterbox bars are ignored; moves and held drags clamp to the nearest
  surface pixel, so in fullscreen the screen edge reaches the original's 2-px edge-scroll zones. Focus loss releases a
  held press and held keys and pulls an edge pointer 2 px inward. The system cursor is hidden over the game (the game
  draws its own).

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
- Not yet (later milestones): movies (defeat and campaign-end movies complete as unavailable; the introduction
  completes as finished when the CD has it, as the macOS host does) and audio.

### Opções

The port's "Opções" screen (not in the original; `engine/include/options-screen.md`) is drawn by the engine with the
original menu material (TITRE.PIE, MENU.CMP, the menu fonts), the same screen as on macOS. An "OPÇÕES" caption on the
wooden floor of the main and pause menus opens it; only a click that starts on it is taken, every other input reaches
the original shell unchanged. Up/Down/Enter/Escape and Left/Right (only while it is open) or the mouse choose:
"Atalhos modernos" (here: the Shift+click order queue rule, `RL_REGRA_FILA_ORDENS`, for worlds created afterwards;
Windows has no modern hotkey layer yet), "Caminhos melhores" (stored; rule bit 0 belongs to the pathing front),
difficulty (next stage start) and speed (at once, like `+`/`-`). "Configurar teclas" is shown unavailable: Windows has
no key dialog yet. The settings live in `opcoes.ini` beside `Dados` and `Partidas` (`atalhos`, `caminhos`,
`dificuldade`, `velocidade`); `--difficulty`/`--speed` override them for one run. Smoke and demo runs neither read
nor show them, except the menu walk, which opens and closes the screen on both drivers.

### Saves

Same files, names and checks as the macOS app, so a `Partidas` folder can be copied between the two
(`src/rlw_saves.h`, `src/rlw_save.inc`):

- Folder: `%LOCALAPPDATA%\RisingLands-ptbr\Partidas` (`--save-directory` to change it). `posicao-1`…`posicao-10.rls`
  are the named positions (the original lists show them as `save1.sav`…`save9.sav`, `save0.sav`), `rapido.rls` the
  quick save, `inicio-fase.rls` the stage-start autosave, `nomes.plist` the position names. Each slot file holds
  exactly the engine's save bytes; its presentation (picture, terrain cache, pointer, stage and difficulty) is the
  companion `<slot>.<sha256>.view`.
- "Partida" menu: **Salvar rápido** (F5), **Carregar rápido** (F9), **Salvar partida…** (positions 1–5, asks for a
  name), **Carregar partida** (positions 1–5 and "Início da fase · salvamento automático"). F5/F9 are host keys.
- The pause menu's Salvar (list 14) saves the paused game and resumes it; its Carregar (list 12) and the main menu's
  "Carregar jogo" (list 13) load a position. The lists show "<name> · dd/MM HH:mm" for existing positions.
- A save is captured between complete frames (held keys are released first; while their releases or a held button
  are waiting, it retries every 80 ms) and written on a background thread: companion first, then the slot through a
  temporary file, `FlushFileBuffers` and an atomic replace, so a failure keeps the previous save. Loading waits for a
  save still being written and replaces world, renderer and picture together; a game saved in the pause menu loads
  with its picture and reopens the pause. A failed load from an original list reopens the main menu, as on macOS.
- The autosave is written after each stage's first frame (start, restart, next stage). Frame-count smoke runs write
  no saves unless `--save-directory` is given.

Test switches: `--selftest` (engine ABI versions, no data), `--import-only`, `--frames N` (run N frames as fast as
possible in the window, then exit; with `--screenshot out.bmp` saves the presented client area), `--headless` (no
window), `--hashes out.txt` (one line per tick: frames, screen, SHA-256 of pixels and palette), `--demo-input` (the
fixed synthetic input of `src/rlw_demo.h`), `--no-verify-image`, `--menu-demo out.txt` (no window: the scripted menu
walk of `src/rlw_menu_demo.c`, one line per step), `--save-demo out.txt [--save-directory EMPTY]` (no window: the
scripted save walk, writing in its own empty folder, never in the player's saves). `--frames`/`--headless` without
`--mission` start MONDE01 as in milestone 1.

## Tests

| Test | Needs | What it shows |
|---|---|---|
| `windows/build.sh test` | any OS with a C compiler | SHA-256, ISO 9660 reader over ISO/BIN MODE1+CUE/BIN MODE2/folder, import (menus, briefings, movies), refusals, atomic replacement; menu routing; FLC decoding; save folder (durable replace, `nomes.plist`, list captions, CP1252) |
| `windows/build.sh` | mingw-w64 | the engine and host build for Windows; `objdump -p` shows only system DLL imports |
| `windows/parity.sh --cd …/RISING_LANDS_BR_.cue 300` | macOS + the player's CD | the Windows driver and the macOS driver give identical per-tick frame hashes with the same input (`PARIDADE OK`), then identical lines for the scripted menu walk: main menu, Novo jogo, briefing page, stage start, 150 frames, pause, objectives page, Enter back to the pause, Escape to resume, pause, restart, 60 frames (`PARIDADE DOS MENUS OK`); then the save walk: stage autosave, 120 frames, save, 60 frames, load and the same 60 frames (compared), autosave load and its first 30 frames (compared), the pause list's save over the same slot, the pause list's load of that paused save, the reopened pause, continue, 20 frames; the lines carry the SHA-256 of every slot and `.view` file, so both drivers must write identical files (`PARIDADE DOS SAVES OK`) |
| `RisingLands.exe --selftest` | Windows, wine or CI | engine ABI/extension versions inside the Windows build |
| `RisingLands.exe --headless --cd … --frames 300 --demo-input --hashes w.txt` | Windows/wine + CD | MONDE01 runs; the hashes must equal `parity.sh`'s `windows.txt` (cross-platform determinism) |
| `RisingLands.exe --cd … --frames 300 --screenshot s.bmp` | Windows/wine + CD | presentation (palette, aspect-fit) |
| `RisingLands.exe --cd … --menu-demo m.txt` | Windows/wine + CD | the menu walk on Windows; must equal `parity.sh`'s `windows-menu.txt` |
| `RisingLands.exe --cd … --save-demo s.txt` | Windows/wine + CD | the save walk on Windows (real Win32 file writes); must equal `parity.sh`'s `windows-save.txt` |
| `.github/workflows/windows-client.yml` | GitHub Actions | three builds (mingw x64, MSVC x64, MSVC ARM64), unit tests through the Win32 file APIs with gcc and with cl, self-test and import refusal on both x64 builds and natively on a `windows-11-arm` runner, ARM64 machine type |
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

- Audio (sound effects, CD music), event pictures (FLC) and movies. An in-game FLC event is ended at once with
  `rl_evento_flc_terminar_v1` (the call the macOS host makes when its picture ends), so play continues without the picture.
- Position names only for 1–5 in the "Partida" menu (6–10 through the original lists, unnamed), as on macOS.
- A D3D11 presenter and DPI awareness (MSVC x64 and ARM64 builds are done).
- A key dialog ("Configurar teclas" in the Opções screen) and the modern hotkey layer of macOS.
- "Caminhos melhores" in the Opções screen is stored but not applied yet: the host does not pass
  `RL_REGRA_CAMINHOS_MELHORES` (rule bit 0) to new worlds.
