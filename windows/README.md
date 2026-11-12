# Rising Lands for Windows (milestones 1–2)

> **Resumo em português.** Cliente Windows sobre o mesmo motor em Rust e a mesma ABI em C (`engine/include`) do app
> macOS. Este primeiro marco importa o CD brasileiro do jogador (imagem `.BIN`/`.CUE`, `.ISO` ou pasta), roda a
> MONDE01 a 30 quadros/s no 1x, mostra o quadro 800x600 com a paleta do jogo (com aspect-fit e tela cheia), entrega
> mouse e teclado pelo mesmo caminho do macOS (as mesmas mensagens, as mesmas 23 teclas, F11 nunca entregue) e
> pausa. Stack: Win32 + GDI em C, motor ligado estaticamente, um único `RisingLands.exe` sem DLLs de terceiros.
> Compila sem máquina Windows (mingw-w64). O marco 2 abre no menu principal original e roda os menus pelo shell
> preparado do motor, como o macOS: "Um jogador", "Novo jogo", briefing e objetivos (páginas por clique, setas ou
> roda do mouse), pausa no jogo (Esc: continuar, objetivos, abandonar, reiniciar a missão), tela de resultado com a
> próxima fase, créditos (FLC decodificado) e "Sair". Áudio, filmes e saves ficam para os próximos marcos: as listas
> de salvar/carregar aparecem vazias, salvar avisa e volta ao jogo, e os filmes contam como indisponíveis.
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
- It cross-compiles from macOS/Linux with mingw-w64 and builds natively under MSYS2 or on `windows-latest`.
- SDL2 would add a DLL and an input layer to re-map back to Win32 messages the engine already speaks; D3D11 is not
  needed for this frame size. Either remains possible later behind the same driver if fullscreen performance or
  scaling filters call for it.

## Layout

| File | Role |
|---|---|
| `src/rlw_main.c` | Win32 window, GDI presentation, frame pacing, mouse/keyboard mapping, pause, speeds, fullscreen, CD selection, menu ticks and host requests, smoke modes |
| `src/rlw_driver.c/.h` | Engine orchestration: main-menu or mission start with the explicit EVENTS host, renderer from the CD, load tail, per-frame input phases (port of the macOS driver's production path) |
| `src/rlw_shell.inc` | Original menus on the prepared shell: session over pause 16 / result 19 / main menu, backgrounds decoded from the CD, briefing VFS, "Novo jogo", restart, next stage, resume, credits (port of `macos_campaign_shell.inc` and the driver's campaign continuation) |
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
| `../.github/workflows/windows-client.yml` | GitHub Actions workflow (`windows-latest`): build, tests and the `.exe` artifact |
| `build.sh` | Engine static library + host executable (+ `test`) |

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

**MSVC** is not wired yet. The path is `cargo xwin rustc --target x86_64-pc-windows-msvc …` (or native `cargo` on
Windows) for the engine plus `clang-cl`/`cl` for the five C files, linking `rising_engine.lib` with `gdi32 user32
comdlg32 shell32 ole32 winmm ntdll userenv ws2_32 bcrypt` (`cargo rustc … -- --print native-static-libs` lists the
exact set). The C sources need `/utf-8` for their PT-BR messages.

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
- Not yet (later milestones): saves (the lists show empty slots; saving shows a notice and resumes without writing;
  loading reopens the main menu), movies (defeat and campaign-end movies complete as unavailable; the introduction
  completes as finished when the CD has it, as the macOS host does) and audio.

Test switches: `--selftest` (engine ABI versions, no data), `--import-only`, `--frames N` (run N frames as fast as
possible in the window, then exit; with `--screenshot out.bmp` saves the presented client area), `--headless` (no
window), `--hashes out.txt` (one line per tick: frames, screen, SHA-256 of pixels and palette), `--demo-input` (the
fixed synthetic input of `src/rlw_demo.h`), `--no-verify-image`, `--menu-demo out.txt` (no window: the scripted menu
walk of `src/rlw_menu_demo.c`, one line per step). `--frames`/`--headless` without `--mission` start MONDE01 as in
milestone 1.

## Tests

| Test | Needs | What it shows |
|---|---|---|
| `windows/build.sh test` | any OS with a C compiler | SHA-256, ISO 9660 reader over ISO/BIN MODE1+CUE/BIN MODE2/folder, import (menus, briefings, movies), refusals, atomic replacement; menu routing; FLC decoding |
| `windows/build.sh` | mingw-w64 | the engine and host build for Windows; `objdump -p` shows only system DLL imports |
| `windows/parity.sh --cd …/RISING_LANDS_BR_.cue 300` | macOS + the player's CD | the Windows driver and the macOS driver give identical per-tick frame hashes with the same input (`PARIDADE OK`), then identical lines for the scripted menu walk: main menu, Novo jogo, briefing page, stage start, 150 frames, pause, objectives page, Enter back to the pause, Escape to resume, pause, restart, 60 frames (`PARIDADE DOS MENUS OK`) |
| `RisingLands.exe --selftest` | Windows, wine or CI | engine ABI/extension versions inside the Windows build |
| `RisingLands.exe --headless --cd … --frames 300 --demo-input --hashes w.txt` | Windows/wine + CD | MONDE01 runs; the hashes must equal `parity.sh`'s `windows.txt` (cross-platform determinism) |
| `RisingLands.exe --cd … --frames 300 --screenshot s.bmp` | Windows/wine + CD | presentation (palette, aspect-fit) |
| `RisingLands.exe --cd … --menu-demo m.txt` | Windows/wine + CD | the menu walk on Windows; must equal `parity.sh`'s `windows-menu.txt` |
| `.github/workflows/windows-client.yml` | GitHub Actions | build, unit tests through the Win32 file APIs, self-test, import refusal |

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

## Known gaps (next milestones)

- Audio (sound effects, CD music), event pictures (FLC) and movies. An in-game FLC event is ended at once with
  `rl_evento_flc_terminar_v1` (the call the macOS host makes when its picture ends), so play continues without the picture.
- Saves/loads (the original lists show empty slots), the stage-start autosave file (the engine slot is captured, not
  written) and saved input queues.
- MSVC build, ARM64 Windows, a D3D11 presenter, DPI awareness and a settings UI.
