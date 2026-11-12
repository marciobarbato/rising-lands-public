/* Host-side input routing for the original menus (prepared shell), portable C.
 * Port of NativeLiveWindow.swift's shellEvent / flushShellMove / pageObjectives /
 * shellTick / sendShell: keys act on release (Enter/Up/Down/Escape), pointer moves
 * are coalesced, presses and releases never are, the objective/briefing pages turn
 * one page per click (rlw_shell_click_clock) and Up/Down/wheel click the original
 * arrow zones 6/7. Requests the driver completes itself (SHOW, SAVE_LOAD,
 * RESET_CAMPAIGN, MISSION_NEXT, RESUME, START_MISSION, CREDITS) are completed here;
 * the others are handed to the host (movies, quit, multiplayer, save slots).
 *
 * The driver is reached through RlwMenuOps so the same routing drives the Windows
 * driver in the app and both drivers in tests/parity.c. No Win32 here. */
#ifndef RLW_MENU_H
#define RLW_MENU_H
#include <stdint.h>
#include "rising_shell.h"
#include "rlw_flc.h"

typedef struct {
 uint32_t (*mode)(void *driver);
 int (*state)(void *driver, RlShellInput *out);
 int (*input)(void *driver, const RlShellInput *in, RlShellEvent *out);
 int (*draw)(void *driver, uint32_t animation);
 int (*show)(void *driver);
 int (*prepare)(void *driver);
 int (*request)(void *driver);
 int (*load)(void *driver);
 int (*slots)(void *driver, const uint8_t *table, uint32_t length);
 int (*zone)(void *driver, uint32_t zone, int32_t rect[4]);
 uint32_t (*click_clock)(uint32_t mode, uint32_t clock, uint32_t press);
 int (*credits_frame)(void *driver, uint32_t elapsed_ms, RlShellCredits *out);
 int (*credits_present)(void *driver, const RlShellCredits *frame, const uint8_t *pixels, const uint8_t *palette);
 void (*error)(void *driver, char *out, size_t capacity);
} RlwMenuOps;

enum {
 RLW_MENU_IDLE = 0,      /* nothing new to present */
 RLW_MENU_REPAINT = 1,   /* the driver's picture changed */
 RLW_MENU_HOST = 2,      /* menu->host_event is a request for the host */
 RLW_MENU_GAME = 3,      /* the session closed: gameplay continues (resume or a stage start) */
 RLW_MENU_FAILED = 4     /* menu->error holds the reason; the world is kept */
};

typedef struct {
 const RlwMenuOps *ops; void *driver;
 int mouse_down; uint8_t held[256];
 int move_pending; uint32_t move_x, move_y;
 int zone_known[2]; int32_t zone_rect[2][4];
 /* Host captions for the original save lists, handed to the driver before every
  * request that may open one; publish_slots (when set) refreshes them first. */
 uint8_t slot_table[330];
 void (*publish_slots)(void *ctx, uint8_t table[330]); void *publish_ctx;
 /* Request that closed the session with RLW_MENU_GAME: RESUME or START_MISSION
  * (a new stage: the host takes its stage autosave). */
 uint32_t game_kind;
 /* Credits (mode10): CREDITS.FLC path, decoder and the time of its last frame read. */
 char credits_path[4096]; RlwFlc credits; int credits_open; double credits_read_ms;
 RlShellEvent host_event;
 char error[512];
} RlwMenu;

void rlw_menu_init(RlwMenu *m, const RlwMenuOps *ops, void *driver, const char *credits_path);
void rlw_menu_reset(RlwMenu *m);     /* drop held keys, a pending move and a held button */
void rlw_menu_close(RlwMenu *m);     /* frees the credits decoder */
/* vk: Windows virtual key; down 1/0. */
int rlw_menu_key(RlwMenu *m, uint32_t vk, int down, double now_ms);
/* msg 0x200..0x205, x 0..799, y 0..599. Moves are only recorded (flush/tick apply them). */
int rlw_menu_mouse(RlwMenu *m, uint32_t msg, uint32_t x, uint32_t y, double now_ms);
int rlw_menu_flush_move(RlwMenu *m, double now_ms);
void rlw_menu_cancel(RlwMenu *m);    /* an interrupted press is not a click */
int rlw_menu_page(RlwMenu *m, int next, double now_ms);
/* One shell tick: virtual counters 0x508f7c/84 + 1 (every 50 ms; 10 ms in credits). */
int rlw_menu_tick(RlwMenu *m, double now_ms);
/* Keys bitmask (RL_SHELL_ENTER/UP/DOWN/ESCAPE) as one completed press. */
int rlw_menu_keys(RlwMenu *m, uint32_t keys, double now_ms);
int rlw_menu_send(RlwMenu *m, const RlShellInput *input, double now_ms);
uint32_t rlw_menu_tick_ms(const RlwMenu *m);
#endif
