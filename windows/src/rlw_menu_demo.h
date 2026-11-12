/* Fixed scripted walk through the original menus, for parity and smoke runs.
 * Program start in the main menu, "Um jogador", "Novo jogo", the briefing (one
 * page turned), the stage start, play with the fixed synthetic input (rlw_demo.h),
 * the pause menu (objectives, Enter back to the pause, Escape to the game), more play, the pause menu's
 * "Reiniciar a missão" and the restarted stage. Every action goes through the
 * normal host paths (rlw_menu.c and the driver's event queue), never into game
 * state. One line per step: "<step> <action> <result> <shell mode> <screen>
 * <frames> <sha256(pixels||palette)>". The same script drives the Windows driver
 * and, in tests/parity.c, the macOS driver: identical output is the parity proof.
 * Transitions depend on the original menus; an unexpected one is printed and the
 * walk ends there, identically for both drivers. */
#ifndef RLW_MENU_DEMO_H
#define RLW_MENU_DEMO_H
#include <stdio.h>
#include "rlw_menu.h"
typedef struct {
 RlwMenuOps menu;
 int (*advance)(void *driver);
 int (*event)(void *driver, uint32_t kind, uint32_t message, int32_t x, int32_t y, uint32_t wparam, uint32_t vk, uint32_t down);
 int (*begin)(void *driver);
 uint32_t (*screen)(void *driver);
 uint64_t (*frames)(void *driver);
 const uint8_t *(*pixels)(void *driver);
 const uint8_t *(*palette)(void *driver);
 void (*stop_message)(void *driver, char *out, size_t capacity);
 /* Saves (milestone 3): the driver's own save, stage autosave, load and the
  * pause list's SAVE_SLOT completion; errors through menu.error. */
 int (*save)(void *driver, const char *path);
 int (*autosave)(void *driver, const char *path);
 int (*load)(void *driver, const char *path);
 int (*shell_save_slot)(void *driver, const char *path, int32_t *saved);
} RlwDemoOps;
/* Returns 0 when the whole walk ran, 1 when it ended early (reason on stderr). */
int rlw_menu_demo_run(const RlwDemoOps *ops, void *driver, FILE *out);
/* Save walk (milestone 3), from a fresh MONDE01 driver in play mode, writing in
 * the empty folder `directory`: the stage autosave after the load tail, 120
 * frames with the fixed input, a save, 60 more frames; the save loaded and the
 * same 60 frames replayed (compared), the autosave loaded and its first 30
 * frames replayed (compared); the pause menu's Salvar list (SAVE_SLOT over the
 * same slot), 20 frames, the pause menu's Carregar list (LOAD_SLOT of that
 * paused save), the reopened pause and "continuar", 20 frames. Lines as in the
 * menu walk, plus the SHA-256 of each slot file and of its .view companion. */
int rlw_save_demo_run(const RlwDemoOps *ops, void *driver, const char *directory, FILE *out);
#endif
