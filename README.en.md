# Rising Lands — reimplemented engine (macOS and Windows)

[Versão em português](README.md)

This repository holds a reimplementation of the **Rising Lands** (1997, Microïds) game engine in Rust and native apps that run the game on top of it: the macOS app (alpha) and the Windows client (pre-alpha, in progress). The logic was ported routine by routine from the original executable and compared against it.

**The game is not included.** To play, you need the image of the **original Brazilian CD** of Rising Lands. On first launch the app reads all game data from that CD: graphics, palette, sounds, music, texts, stages and movies. No game data is distributed here, not even inside the app.

## Download and play (macOS)

1. Download the `.dmg` of the latest version from **[Releases](https://github.com/marciobarbato/rising-lands-public/releases)**. The current one is the alpha `v0.1.0-alpha.2`. The app is signed with Developer ID and notarized by Apple.
2. Open the `.dmg` and drag **Rising Lands** to **Applications**.
3. Get the CD image (see "Where to get the CD").
4. Open the app. In the **"Rising Lands — importar o CD original"** window, click **Escolher o CD…** and select **RISING_LANDS_BR_.BIN**. The .CUE, an .ISO or the folder with the CD files also work, but music comes only from the .BIN.
5. The files are checked and copied to Application Support once (about 630 MB). After that the game opens straight into the original menu. To import again: **Jogo → Importar o CD novamente…**.

Requirements to play: a Mac with **Apple Silicon** and **macOS 13** or later. The app's interface is in Portuguese, like the game.

## Windows (pre-alpha)

The Windows client is in progress, in [`windows/`](windows/README.md). There is a **pre-alpha** for early testers. **Testing on a real Windows PC is left to the community** ([#6](https://github.com/marciobarbato/rising-lands-public/issues/6)): the author will not test on Windows.

1. Download the `.exe` from the latest Windows pre-release (`v0.1.0-prealpha.…-windows`) on **[Releases](https://github.com/marciobarbato/rising-lands-public/releases)**: x64 for most PCs, ARM64 for PCs with an ARM processor. Check it against the SHA-256 in the release notes. The `.exe` was built by GitHub Actions from this source (the `windows-client` workflow).
2. **SmartScreen warning:** the `.exe` is **not signed**, so Windows shows "Windows protected your PC". Click **More info**, then **Run anyway**. There is no plan to sign it; if you would rather not, build it yourself (see below).
3. On first launch a window asks for the CD: pick **RISING_LANDS_BR_.BIN** (or the .CUE, or an .ISO). The files are checked by SHA-256 and copied to `%LOCALAPPDATA%\RisingLands-ptbr\Dados`. Then the game opens in the original menu.

Requirements: 64-bit Windows 10 or 11 (x86-64 or ARM64). It is a single `.exe`, with no installer and no extra DLLs.

What works:
- CD import (.BIN/.CUE, .ISO or the CD folder);
- the original menus: "Um jogador", "Novo jogo", briefing and objectives, pause (resume, objectives, abandon, restart), the result screen and next stage, credits and "Sair";
- **saves**: the original save and load lists and a "Partida" menu (**F5** quick save, **F9** quick load), with the stage-start autosave. The format is the macOS one: the `Partidas` folder works on both. Saves are in pre-alpha 2 and later;
- the **Options screen** in the original style, on the main and pause menus: difficulty, speed, **Atalhos modernos** (the key shortcuts and the Shift+click order queue) and **Caminhos melhores** (better unit paths), kept between games, with the macOS defaults (modern shortcuts on, better paths off; the modern shortcuts and better paths take effect from pre-alpha 3);
- the macOS **modern shortcuts**: the same digit twice centres on that group, **Home** goes to the last alert, **.** and **,** pick the next idle worker or soldier, **H** selects the sanctuary (pre-alpha 3 and later);
- the **key dialog** (the Options screen's "Configurar teclas" item or **Partida → Configurar teclas…**), with the macOS rules: the game still receives only the original's 23 keys (pre-alpha 3 and later);
- the campaign, with the macOS host's mouse and keyboard, edge scrolling, fullscreen (**F11**), pause (**P**) and speeds (**+** and **−**: 0.5x, 1x, 2x and 4x).
- Parity with macOS: with the same input, the Windows driver and the macOS driver produce the same frames, tick by tick, the same menu walk and the same save files.

Still missing:
- **audio** (sound effects and CD music), in-game event pictures (FLC) and **movies** ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4));

Note: parity was checked on macOS, by building the Windows driver against the same engine. On real Windows, CI only checks the build, the unit tests, the engine self-test and the refusal of a file that is not the CD. **Nobody has played this build on a Windows PC yet.** If you have Windows and the CD, you can help a lot: say on [#6](https://github.com/marciobarbato/rising-lands-public/issues/6) what worked and what did not, or open an issue with the **Problema no jogo / Game problem** template and say it is Windows.

## Where to get the CD

The original Brazilian CD image can be found on the **Internet Archive**. Download **RISING_LANDS_BR_.BIN** and **RISING_LANDS_BR_.CUE**.

## Current state

- The campaign runs on all 25 stages. An automated bot played every one of them, each game up to 72,000 frames, with no engine stop. The first missions were also played by hand without problems.
- Nobody has finished the whole campaign yet. That is where your testing helps.
- If the game reaches something not ported yet, it does not freeze silently: it shows a **stop panel** with the original routine's address. That text is the most useful part of a report.
- macOS: alpha `v0.1.0-alpha.2`. The source already has new work for the next release: the monastery clock with the side panel hidden, the balloon leaving the hangar, the modern shortcuts and the Shift+click order queue, the **Options screen** and **better unit paths** (see "Controls").
- Windows: pre-alpha, in progress; game, menus, saves, the Options screen, modern shortcuts, the key dialog and better unit paths done, with x64 and ARM64 builds (see "Windows (pre-alpha)").
- Only the Brazilian Portuguese release of the game is supported.

## Controls

- **Speed:** menu **Jogo → Velocidade**, with 0,5x, 1x, 2x and 4x (15, 30, 60 and 120 frames per second). Normal speed (1x) is 30 frames per second. Shortcuts: **⌘]** faster, **⌘[** slower, **⌘P** pause and resume.
- **Fullscreen:** menu **Janela → Tela cheia** (**⌃⌘F**). Fullscreen shows only the game, on black.
- **Moving around the map:** push the mouse against the edge. In fullscreen, move the mouse to the edge of the screen. In a window, the map scrolls while the mouse is inside the window, at its edge. The arrow keys also scroll.
- **Keyboard:** Enter, Esc, Space, arrows, Delete, X, Tab (minimap) and the digits 0–9, as in the original. Keys can be remapped in **Jogo → Configurar teclas…**. The original's F11 (switch to 640x480) is not used.
- **Modern shortcuts** (Age of Empires II style; in the current source, coming in the next macOS release). Each shortcut becomes input the original already accepts (a key or a click), through the same path as the player's own input; nothing changes game state behind the game's back.
  - **N twice** (groups 1–9): selects the group and centres on it (the original's Shift+N). Ctrl+N, N and Shift+N stay as in the original.
  - **Home:** last alert (the original's X).
  - **.** (period): next idle worker. **,** (comma): next idle military unit.
  - **H:** selects the Sanctuary.
  - **Shift+click** on open ground with units selected: queues a waypoint. Units walk the waypoints in order, and a plain order replaces the queue. Waypoints show as small flags joined by a dashed line. In the original this click does nothing; the queue is an optional engine rule that follows the **Atalhos modernos** option. Changing the option applies to games started afterwards; the game in progress and its following stages keep the rules it started with, and a save keeps its own.
  - They are on by default. To turn them off, untick **Jogo → Atalhos modernos**. Keys can be remapped in **Jogo → Configurar teclas…**. The key shortcuts do nothing in menus, in the briefing or while paused.
- **Better unit paths** (optional, off by default; in the current source, coming in the next macOS release): menu **Jogo → Caminhos melhores**. Units plan the whole path before walking, a group spreads its goals instead of fighting over one cell, and a blocked unit waits and replans instead of giving up. Combat, AI and speeds do not change. It applies from the next stage start; a save keeps the rules of its game. Details in [docs/MODERN_PATHS.md](docs/MODERN_PATHS.md).
- **Options screen** (in the current source, coming in the next macOS release): the **OPÇÕES** item on the wooden floor of the main and pause menus, or the menu **Jogo → Opções…** (**⌘O**), opens a screen in the game's look with modern shortcuts, better paths, difficulty, speed and key setup. During a game, **⌘O** first pauses the game, like Esc, and opens the screen over the pause menu. **Esc** or **Voltar** closes it. They are the same options as the app menus. The original game has no such screen; it is drawn only with material from your CD.

## Reporting problems

Open an issue in this repository with the **Problema no jogo / Game problem** template. It helps a lot to include:
- the macOS version and the Mac model;
- the stage and what you were doing;
- the whole text of the stop panel, if it appeared.

Screenshots and screen recordings are welcome. Do not attach saves or game files: they may contain original game data.

## How it was made

The reverse engineering was done with **Ghidra**, together with an x86 emulator that runs the original routines side by side with the port. Much of the port was done with the help of **Claude** (Anthropic's AI): reading the disassembly, writing the Rust code and building differential tests that compare each ported routine against the original executable. Code comments cite the original routine names and addresses.

## Building from source

Requirements: a Mac with Apple Silicon and macOS 13+, **Xcode Command Line Tools** (`xcode-select --install`) and stable **Rust** (https://rustup.rs).

```bash
./build-macos.sh
```

The app is written to `build/RisingLands.app`. To choose another output folder: `./build-macos.sh <folder>`. The default signature is ad hoc. To sign for distribution, set `RISING_SIGN_IDENTITY` to a Developer ID identity; `RISING_BUNDLE_ID` changes the app identifier.

Tests:

```bash
cargo test --locked --manifest-path engine/Cargo.toml
```
```bash
./macos/tests/run.sh
```

Windows client: from a Mac or Linux with mingw-w64, or on Windows with MSYS2/Git Bash. On Windows it also builds with Visual Studio (MSVC), for x64 and ARM64: `windows\build-msvc.ps1 -Test` in PowerShell. Details in [windows/README.md](windows/README.md).

```bash
rustup target add x86_64-pc-windows-gnu
```
```bash
./windows/build.sh test
```

The `.exe` is written to `windows/build/RisingLands.exe`.

Tests that need the original game files are skipped when the files are absent. With `RL_CD_ROOT` pointing at the CD folder, `cd_derived_assets` checks that the data computed from the CD (palette, tables, menu screens and terrain cache) equals the original's, byte for byte. What is and is not in this repository is listed in [PUBLICATION_AUDIT.md](PUBLICATION_AUDIT.md).

## Work fronts

What is being worked on, the state of each front and where outside help fits: **[docs/ROADMAP.md](docs/ROADMAP.md)**. Done since the last published macOS release: the Options screen and better unit paths (on Windows they are in pre-alpha 3). The open fronts each have an issue: the Windows client ([#1](https://github.com/marciobarbato/rising-lands-public/issues/1)), audio and movies on Windows ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)), testing on a Windows PC ([#6](https://github.com/marciobarbato/rising-lands-public/issues/6)), other CDs and languages ([#8](https://github.com/marciobarbato/rising-lands-public/issues/8)) and in-game checks ([#9](https://github.com/marciobarbato/rising-lands-public/issues/9)). Before starting anything large, comment on the front's issue.

## Help wanted

- **Test Windows on a real PC:** nobody has played the pre-alpha on a Windows PC yet, and the author will not test on Windows ([#6](https://github.com/marciobarbato/rising-lands-public/issues/6)).
- **Windows host:** audio, CD music and movies ([#4](https://github.com/marciobarbato/rising-lands-public/issues/4)); an outside contribution is already under discussion: agree on the issue first. [docs/WINDOWS_CLIENT.md](docs/WINDOWS_CLIENT.md) maps each part to the macOS code.
- **In-game checks:** the ESPELHO spell, the druid and the fusion monastery, and the full campaign ([#9](https://github.com/marciobarbato/rising-lands-public/issues/9)).
- **Other languages:** only the Brazilian CD works today. If you own another original CD (English, French...), say which on [#8](https://github.com/marciobarbato/rising-lands-public/issues/8). No translated game text may go into the repository.

## License

The source code in this repository is released under the MIT License (see [LICENSE](LICENSE)): anyone may read, use, modify and redistribute it. It covers the code only, not the original game.

## Rights

Rising Lands and all its original materials (graphics, sounds, music, texts, stages, movies and executable) remain the property of their rights holders. This repository distributes none of them and grants no license to them. Players use their own copy of the CD.
