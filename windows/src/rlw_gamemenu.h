/* The "Jogo" menu of the Windows host, portable C (no Win32; unit-tested on any
 * host). Port of the macOS player app's Jogo menu (NativePlayerApp.swift
 * makePlayerMenu, main.swift validateMenuItem and the save menus): item ids,
 * labels, enabled/checked state from a snapshot of the host, and the command a
 * click means. The Win32 menu itself is built by rlw_gamemenu_win.c; the host
 * (rlw_main.c) fills RlwGameMenuState and carries out the decoded commands.
 *
 *   Pausar / Continuar          P
 *   Velocidade      > 0,5x · 15 quadros/s … 4x · 120 quadros/s | Mais rápido + | Mais devagar −
 *   Dificuldade     > Fácil | Normal | Difícil | (Vale a partir do próximo início de fase)
 *   ---
 *   Salvar rápido   F5 / Carregar rápido   F9
 *   Salvar partida… > posições 1–5 / Carregar partida > posições 1–5 | Início da fase | Salvamento automático
 *   Gerenciar partidas salvas…
 *   Salvamento automático > Desligado | A cada 5/10/15 minutos | (Conta o tempo de jogo…)
 *   ---
 *   Opções… / Configurar teclas… / Atalhos modernos / Caminhos melhores
 *
 * Ids 100..199 belong to this menu (other popups, e.g. Áudio, use other ids). */
#ifndef RLW_GAMEMENU_H
#define RLW_GAMEMENU_H
#include <stddef.h>
#include <stdint.h>

enum {
 RLW_GM_QUICK_SAVE = 100, RLW_GM_QUICK_LOAD = 101, RLW_GM_STAGE_LOAD = 102,
 RLW_GM_SAVE_POSITION = 110, /* + 1..5 */
 RLW_GM_LOAD_POSITION = 120, /* + 1..5 */
 RLW_GM_KEYS = 130, RLW_GM_MANAGE = 131,
 RLW_GM_AUTOSAVE = 140,      /* + 0..3: RLW_AUTOSAVE_CHOICES */
 RLW_GM_AUTOSAVE_LOAD = 150,
 RLW_GM_PAUSE = 160,
 RLW_GM_SPEED = 170,         /* + 0..3: 0,5x 1x 2x 4x */
 RLW_GM_FASTER = 174, RLW_GM_SLOWER = 175,
 RLW_GM_DIFFICULTY = 180,    /* + 0..2: Fácil Normal Difícil */
 RLW_GM_OPTIONS = 190, RLW_GM_MODERN = 191, RLW_GM_PATHS = 192,
 RLW_GM_FIRST = 100, RLW_GM_LAST = 199
};
#define RLW_GM_POSITIONS 5 /* the menus show positions 1..5 (the original lists 1..10) */

/* What the menu needs to know about the host, filled by the host before a refresh
 * or a command. */
typedef struct {
 int driver;            /* a game or the original menus are loaded */
 int failed;            /* the menus or the game stopped for good */
 int playing;           /* in play: no original screen open, engine not stopped */
 uint32_t shell_mode;   /* original screen shown (RL_SHELL_*), 0 in play */
 int paused;            /* the host's pause (P) */
 int saves;             /* the player's save folder is in use (not in smoke runs) */
 int writing;           /* a save is still being written */
 int keys;              /* Configurar teclas… offered */
 int options;           /* Opções… and the option items offered (players' runs) */
 unsigned speed;        /* 0..3 */
 unsigned difficulty;   /* 0..2 */
 unsigned atalhos, caminhos, autosave_minutes;
 /* Slot files present (rlw_gamemenu_scan). */
 int quick, stage, autosave; unsigned positions; /* bit n = posicao-n exists (1..5) */
} RlwGameMenuState;

/* Fills quick/stage/autosave/positions from the save folder ("" = none). */
void rlw_gamemenu_scan(RlwGameMenuState *s, const char *directory);

typedef struct { int enabled, checked; } RlwGameMenuItem;
RlwGameMenuItem rlw_gamemenu_item(const RlwGameMenuState *s, unsigned id);
/* Fixed or state-dependent label (UTF-8, with a "\t<key>" hint where the host
 * has a key): "Pausar\tP"/"Continuar\tP", "1x · 30 quadros/s", "Fácil", "A cada
 * 5 minutos"… Position titles come from rlw_saves_title. "" for an unknown id. */
void rlw_gamemenu_label(const RlwGameMenuState *s, unsigned id, char *out, size_t capacity);

/* A click: action is the item family (RLW_GM_*), arg its value — the position
 * (1..5), the speed (Mais rápido/devagar resolved), the difficulty, the autosave
 * minutes, the new on/off of a toggle. action 0: nothing (disabled or unknown). */
typedef struct { unsigned action, arg; } RlwGameMenuCommand;
RlwGameMenuCommand rlw_gamemenu_decode(const RlwGameMenuState *s, unsigned id);

/* Opções… during play (NativeLiveWindow.openOptions): the host holds a synthetic
 * Escape in the world until the original pause opens, then shows the Opções
 * screen over it; if no pause came within 30 frames the Escape is released. */
typedef struct { int pending; uint64_t deadline; } RlwOptionsRequest;
/* 1: deliver Escape down now (0 when one is already pending). */
int rlw_options_request_start(RlwOptionsRequest *r, uint64_t frames);
/* The original pause opened: 1 when the Opções screen must be shown now. */
int rlw_options_request_shell(RlwOptionsRequest *r);
/* After a played frame: 1 when the Escape must be released (deadline passed in play). */
int rlw_options_request_frame(RlwOptionsRequest *r, uint64_t frames, int playing);
#endif
