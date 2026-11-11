# Rising Lands — reimplemented engine (macOS)

[Versão em português](README.md)

This repository holds a reimplementation of the **Rising Lands** (1997, Microïds) game engine in Rust and a native macOS app that runs the game on top of it. The logic was ported routine by routine from the original executable and compared against it.

**The game is not included.** To play, you need the image of the **original Brazilian CD** of Rising Lands. On first launch the app reads all game data from that CD: graphics, palette, sounds, music, texts, stages and movies. No game data is distributed here, not even inside the app.

## Download and play

1. Download the `.dmg` of the latest version from **[Releases](https://github.com/marciobarbato/rising-lands-public/releases)**. The current one is the alpha `v0.1.0-alpha.2`. The app is signed with Developer ID and notarized by Apple.
2. Open the `.dmg` and drag **Rising Lands** to **Applications**.
3. Get the CD image (see "Where to get the CD").
4. Open the app. In the **"Rising Lands — importar o CD original"** window, click **Escolher o CD…** and select **RISING_LANDS_BR_.BIN**. The .CUE, an .ISO or the folder with the CD files also work, but music comes only from the .BIN.
5. The files are checked and copied to Application Support once (about 630 MB). After that the game opens straight into the original menu. To import again: **Jogo → Importar o CD novamente…**.

Requirements to play: a Mac with **Apple Silicon** and **macOS 13** or later. The app's interface is in Portuguese, like the game.

## Where to get the CD

The original Brazilian CD image can be found on the **Internet Archive**. Download **RISING_LANDS_BR_.BIN** and **RISING_LANDS_BR_.CUE**.

## Current state (alpha)

- The campaign runs on all 25 stages. An automated bot played every one of them, each game up to 72,000 frames, with no engine stop. The first missions were also played by hand without problems.
- Nobody has finished the whole campaign yet. That is where your testing helps.
- If the game reaches something not ported yet, it does not freeze silently: it shows a **stop panel** with the original routine's address. That text is the most useful part of a report.
- Only the macOS version exists. There is no Windows client yet (see "Help wanted").
- Only the Brazilian Portuguese release of the game is supported.

## Controls

- **Speed:** menu **Jogo → Velocidade**, with 0,5x, 1x, 2x and 4x (15, 30, 60 and 120 frames per second). Normal speed (1x) is 30 frames per second. Shortcuts: **⌘]** faster, **⌘[** slower, **⌘P** pause and resume.
- **Fullscreen:** menu **Janela → Tela cheia** (**⌘F**). Fullscreen shows only the game, on black.
- **Moving around the map:** push the mouse against the edge. In fullscreen, move the mouse to the edge of the screen. In a window, the map scrolls while the mouse is inside the window, at its edge. The arrow keys also scroll.
- **Keyboard:** Enter, Esc, Space, arrows, Delete, X, Tab (minimap) and the digits 0–9, as in the original. Keys can be remapped in **Jogo → Configurar teclas…**. The original's F11 (switch to 640x480) is not used.

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

Tests that need the original game files are skipped when the files are absent. With `RL_CD_ROOT` pointing at the CD folder, `cd_derived_assets` checks that the data computed from the CD (palette, tables, menu screens and terrain cache) equals the original's, byte for byte. What is and is not in this repository is listed in [PUBLICATION_AUDIT.md](PUBLICATION_AUDIT.md).

## Help wanted

- **Windows client:** the engine has a C interface (`engine/include`), and [docs/WINDOWS_CLIENT.md](docs/WINDOWS_CLIENT.md) describes the steps. See issue #1.
- **Other languages:** only the Brazilian release is supported today. Other versions, such as the English CD, would need the engine checked against that release's executable and files. If you own another version and want to help, please open an issue.

## License

The source code in this repository is released under the MIT License (see [LICENSE](LICENSE)): anyone may read, use, modify and redistribute it. It covers the code only, not the original game.

## Rights

Rising Lands and all its original materials (graphics, sounds, music, texts, stages, movies and executable) remain the property of their rights holders. This repository distributes none of them and grants no license to them. Players use their own copy of the CD.
