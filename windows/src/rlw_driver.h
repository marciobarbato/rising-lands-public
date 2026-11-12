/* Windows host driver: orchestration and transport only. All logic and pixels
 * come from the Rust engine through the C ABI in engine/include, statically
 * linked. This is a port of the production path of
 * macos/src/macos_live_driver.c (create with explicit
 * EVENTS host, macos_live_advance, macos_live_event, macos_live_human_phase),
 * with the same call order, constants and input mapping. It never writes game
 * state: the host only paces frames, queues mouse/keyboard messages and presents.
 * Serialize all calls (one game thread). */
#ifndef RLW_DRIVER_H
#define RLW_DRIVER_H
#include <stddef.h>
#include <stdint.h>
#include "rising_shell.h"

typedef struct RlwDriver RlwDriver;
typedef struct {
 uint64_t frames;          /* complete engine frames (phase 3) since start */
 uint64_t generation;
 uint32_t screen;          /* rl_missao_tela after the last step (16 pause, 19 result: open the shell) */
 uint32_t stopped;         /* latched named stop; message holds the reason */
 uint32_t flc_skipped;     /* in-game FLC events ended without a picture */
 char message[512];
} RlwStatus;

/* Version checks of every extension the driver needs, without game data.
 * Returns 0 or writes a reason. */
int rlw_engine_check(char *error, size_t capacity);
/* root: the imported Dados folder; mission: e.g. "MONDE01.ETA"; difficulty 0..2. */
RlwDriver *rlw_driver_create(const char *root, const char *mission, uint32_t difficulty, char *error, size_t capacity);
/* Original program start into the main menu (MenuInit 2; no stage, no frame
 * state, no renderer) with an open prepared session painting titre.pie
 * (macos_live_create_menu_host_v1). difficulty is the preset of the stages the
 * menus start. */
RlwDriver *rlw_driver_create_menu(const char *root, uint32_t difficulty, char *error, size_t capacity);
void rlw_driver_free(RlwDriver *d);
/* One complete host tick: the first call runs the load tail and the first draw;
 * later calls render the current generation, then run input phases 2 and 3,
 * exactly as macos_live_advance does in play mode. Returns 0 or nonzero with
 * status.stopped set. Screens 16/19 return 0 without stepping. */
int rlw_driver_advance(RlwDriver *d);
/* kind 1 mouse (msg 0x200..0x205, x 0..799, y 0..599, wparam MK_SHIFT/MK_CONTROL),
 * kind 2 key (vk in the 23 accepted keys, down 0/1). Stamped for the next frame. */
int rlw_driver_event(RlwDriver *d, uint32_t kind, uint32_t message, int32_t x, int32_t y,
                     uint32_t wparam, uint32_t vk, uint32_t down);
/* 1 while the left button is held in the engine queue (a press without release). */
int rlw_driver_left_held(const RlwDriver *d, int32_t *x, int32_t *y);
int rlw_driver_key_held(const RlwDriver *d, uint32_t vk);
void rlw_driver_status(const RlwDriver *d, RlwStatus *out);
const uint8_t *rlw_driver_pixels(const RlwDriver *d);   /* 800x600 indexed, stride 800 */
const uint8_t *rlw_driver_palette(const RlwDriver *d);  /* 256 x RGB24, 8-bit */

/* ---- Original menus (prepared shell, milestone 2) ----
 * Same contract as macos/src/MacOSLiveDriver.h's
 * macos_live_shell_* functions (see rising_shell.h for the engine side). While a
 * session is open (rlw_shell_mode != 0) rlw_driver_advance refuses; the host
 * feeds RlShellInput and completes each returned request before the next input.
 * Errors set a host message readable with rlw_shell_error; the world is kept. */
typedef struct { uint32_t outcome, stage, has_next; char next[16]; } RlwCampaignEnd;
/* Read once at the result screen 19: outcome 0 ongoing, 1 victory, 2 defeat. */
int rlw_campaign_end(RlwDriver *d, RlwCampaignEnd *out);
/* After rlw_driver_advance reports screen 16 (pause) or 19 (result). */
int rlw_shell_begin(RlwDriver *d);
uint32_t rlw_shell_mode(const RlwDriver *d);
int rlw_shell_state(const RlwDriver *d, RlShellInput *out);
int rlw_shell_input(RlwDriver *d, const RlShellInput *input, RlShellEvent *out);
int rlw_shell_draw(RlwDriver *d, uint32_t animation);
/* Request completions: SHOW/SAVE_LOAD/CREDITS, MISSION_NEXT, RESET_CAMPAIGN and
 * RESUME, START_MISSION, the three movie requests, SAVE_SLOT (no save yet). */
int rlw_shell_show(RlwDriver *d);
int rlw_shell_prepare(RlwDriver *d);
int rlw_shell_request(RlwDriver *d);
int rlw_shell_load(RlwDriver *d);
int rlw_shell_movie_finish(RlwDriver *d, uint32_t completion);
int rlw_shell_save_slot(RlwDriver *d, int32_t *saved);
/* 10 x [present 0/1, 32-byte CP1252 caption] for the original save lists. */
int rlw_shell_slots(RlwDriver *d, const uint8_t *table, uint32_t length);
int rlw_shell_peek(RlwDriver *d, uint32_t address, uint32_t size, uint8_t *out);
int rlw_shell_zone(RlwDriver *d, uint32_t zone, int32_t rect[4]);
uint32_t rlw_shell_click_clock(uint32_t mode, uint32_t clock, uint32_t press);
/* Mode10: one AffCredits frame, then the host's decoded FLC frame for DRAW. */
int rlw_shell_credits_frame(RlwDriver *d, uint32_t elapsed_ms, RlShellCredits *out);
int rlw_shell_credits_present(RlwDriver *d, const RlShellCredits *frame, const uint8_t *pixels, const uint8_t *palette);
void rlw_shell_error(const RlwDriver *d, char *out, size_t capacity);
#endif
