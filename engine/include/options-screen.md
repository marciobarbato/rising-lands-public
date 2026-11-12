# "Opções" screen (extension v1)

Include `rising_options.h` (it includes `rising_shell.h`). The dylib and the
static library export `rl_opcoes_*` directly (private research repository issue #6).

RISING.EXE has no options screen. This one is a **port feature** drawn only with
the game's own material at run time, so it looks like the original menus:

- background: the decoded TITRE.PIE the host already passes to the shell
  (`rl_shell_imagem_cd`);
- MENU.CMP's three animations at AffGame's menu positions (35,134), (702,0),
  (528,315), as `shell_animation::menu_sprites` draws them;
- title and items with AffText in MenuDisplay's rows (title [0,246,800,276],
  item *i* [0,286+25*i*,800,306+25*i*]) in the fonts MenuInit stored at
  0x4e2f74 (title), 0x4e2f75 (item) and 0x4e2f76 (selected item).

Nothing of it is in the repository. The texts are the port's own table
(`rl-core/src/options_screen.rs`), in CP1252 like TEXTES.TXT and in its
upper-case menu style; pt-BR ships, English is prepared for the language
option (private research repository issue #7, `RlOpcoesValores.idioma`).

## Versioning

`rl_opcoes_versao() == RL_OPCOES_VERSION == 1`. This is an additive extension
with its own version, like the shell (`rl_shell_version`), mission and save
extensions. `RL_ABI_VERSION` stays 1.6 and no existing function or structure
changes: hosts that check `rl_versao()` (the macOS build script matches the
exact `#define`) keep working, and a host uses the options only after
`rl_opcoes_versao()==1`.

## Separation from the faithful shell

The module (`options_screen.rs`, `options_abi.rs`) sits beside the shell state
machine (`shell*.rs`, MenuInit) and does not take part in it:

- it **reads** a prepared session (fonts, mode 0x4e2f78, counter 0x508f84,
  renderer fonts, MENU.CMP bank) through `shell_abi::session_view` and never
  changes it: not the surface, memory, generation or pending request;
- it draws into its own buffer (open screen) or into the **host's copy** of the
  shell picture (button), never into the session's persistent surface;
- it uses no RNG, clock or game memory; its frames depend only on the inputs.

Hence, without the options calls, every shell input, picture and request is
byte for byte what it was; and after opening and closing the screen the session
is still identical to an untouched twin (test
`options_screen_over_the_real_main_and_pause_menus`).

## Host loop

1. At startup: `rl_opcoes_criar(values, decoded TITRE.PIE)`. Values come from
   the host's own preferences; `disponivel` offers the items the host supports
   (`RL_OPCOES_TODOS`, or without `RL_OPCOES_TECLAS` when there is no key
   dialog).
2. Every shell input (keys, mouse, ticks) goes **first** to
   `rl_opcoes_entrada(options, shell, input)`, with `ticks` = animation steps
   since the last input (1 for each shell tick). If `consumed`, do not call
   `rl_shell_input`. Otherwise proceed with the shell as before.
3. After every shell picture copy (`rl_shell_copy`), call
   `rl_opcoes_sobrepor(options, shell, pixels)`: on main 2 and pause 16 it
   draws the "OPÇÕES" button; elsewhere it leaves the pixels untouched.
4. While `open`: present `rl_opcoes_desenhar(options, shell, pixels, palette)`
   instead of the shell picture.
5. Events: `MUDOU` → apply and store `values` (hotkeys, paths, difficulty,
   speed); `CONFIGURAR_TECLAS` → open the host's key dialog (the screen stays
   open); `FECHOU` → repaint the shell picture (still current).

## Input rules

- Closed: only a press that **starts on the button** is consumed, and so is
  its release (wherever it lands: the original MenuInput activates the
  selected item on any click, so letting that release through would run a
  menu item). A release on the button opens the screen. Hover is not consumed
  (the move reaches the shell too; no original zone lies under the button).
  Keys are never consumed while closed.
- Open: everything is consumed. Up/Down move over offered items, the item
  under the pointer is selected (as MenuInput selects its zone), Enter or a
  click activates: toggles flip, Difficulty and Speed cycle, Configure keys
  raises its event, Back closes. Left/Right step Difficulty/Speed (clamped)
  and flip toggles. Escape closes.

## Button position

Centre x 455, caption top y 548 (`BUTTON_CENTRE_X`, `BUTTON_TOP`): the empty
middle of the wooden floor of TITRE.PIE, below every original item of both
menus (main menu ends at y 406, pause at y 431), right of the arrow's tip and
left of the wheel; no MENU.CMP animation reaches it.
`rl_opcoes_botao` returns the clickable rectangle (caption ± 8 px, 25 px tall).

## Items and values

| Item | Value | Host meaning |
|---|---|---|
| ATALHOS MODERNOS: SIM/NÃO | `atalhos` | macOS `RisingLandsModernHotkeys` (hotkeys + order queue) |
| CONFIGURAR TECLAS | event | macOS: the existing key dialog; Windows: not offered yet |
| CAMINHOS MELHORES: SIM/NÃO | `caminhos` | macOS `RisingLandsModernPaths` → rule bit 0 (`RULE_MODERN_PATHS`, modern unit paths); Windows: not offered yet |
| DIFICULDADE: FÁCIL/NORMAL/DIFÍCIL | `dificuldade` | stage-start preset (next New game, Restart or Next stage) |
| VELOCIDADE: 0,5x/1x/2x/4x | `velocidade` | host pace |
| VOLTAR | closes | — |

Unavailable items are engraved: AffText's layout in the item font with
PasteOmbreCmp's raster (the original shadow table, applied twice), so the
glyphs darken the wall instead of painting it. The original menus have no
disabled style; this is the port's choice. Navigation and the pointer skip
them.

## Errors

Null/misaligned pointers, out-of-range values, `reserved != 0`, input keys
above 63 → `RL_INVALID_ARGUMENT`; unknown options or shell handle →
`RL_INVALID_HANDLE`; small buffers → `RL_BUFFER_TOO_SMALL`; drawing while
closed, or opening over another screen or a pending request →
`RL_INVALID_ARGUMENT`. Every failing call leaves the handle and the outputs
unchanged (the state is updated on a copy and published on success).
