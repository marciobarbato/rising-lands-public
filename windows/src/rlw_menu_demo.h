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
} RlwDemoOps;
/* Returns 0 when the whole walk ran, 1 when it ended early (reason on stderr). */
int rlw_menu_demo_run(const RlwDemoOps *ops, void *driver, FILE *out);
#endif
