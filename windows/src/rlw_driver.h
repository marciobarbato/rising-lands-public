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
#include "rising_options.h"

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
 * RESUME, START_MISSION, the three movie requests and SAVE_SLOT (below). The
 * host completes LOAD_SLOT with rlw_driver_load (event.reserved = slot index). */
int rlw_shell_show(RlwDriver *d);
int rlw_shell_prepare(RlwDriver *d);
int rlw_shell_request(RlwDriver *d);
int rlw_shell_load(RlwDriver *d);
int rlw_shell_movie_finish(RlwDriver *d, uint32_t completion);
/* 10 x [present 0/1, 32-byte CP1252 caption] for the original save lists. */
int rlw_shell_slots(RlwDriver *d, const uint8_t *table, uint32_t length);
int rlw_shell_peek(RlwDriver *d, uint32_t address, uint32_t size, uint8_t *out);
int rlw_shell_zone(RlwDriver *d, uint32_t zone, int32_t rect[4]);
uint32_t rlw_shell_click_clock(uint32_t mode, uint32_t clock, uint32_t press);
/* Mode10: one AffCredits frame, then the host's decoded FLC frame for DRAW. */
int rlw_shell_credits_frame(RlwDriver *d, uint32_t elapsed_ms, RlShellCredits *out);
int rlw_shell_credits_present(RlwDriver *d, const RlShellCredits *frame, const uint8_t *pixels, const uint8_t *palette);
void rlw_shell_error(const RlwDriver *d, char *out, size_t capacity);

/* ---- Saves (milestone 3) ----
 * Same contract and file format as MacOSLiveDriver.h's macos_live_save*,
 * macos_live_autosave, macos_live_load and macos_live_shell_save_slot*: the slot
 * file holds exactly the engine's save bytes, a content-addressed companion
 * "<slot>.<sha256>.view" the host presentation. A save written by either host
 * loads in the other. Errors are separate from the simulation stop latch and
 * readable with rlw_driver_save_error (RL_SAVE_BUSY: retry at a later frame). */
typedef struct RlwSaveJob RlwSaveJob;
/* Between complete frames in play, no host input waiting for its frame, no held button. */
int rlw_driver_save(RlwDriver *d, const char *path);
/* Same checks; captures bytes + companion now, rlw_save_commit writes them later. */
int rlw_driver_save_begin(RlwDriver *d, const char *path, RlwSaveJob **job);
/* Stage-start autosave: the world captured by the load tail and the first picture. */
int rlw_driver_autosave(RlwDriver *d, const char *path);
/* Replaces world, renderer and picture together (an open shell session is
 * discarded); host input is reset. A pause-list save (mode16) stays paused
 * until rlw_driver_load_resume (rlw_driver_advance reports screen 16). */
int rlw_driver_load(RlwDriver *d, const char *path);
/* Right after a successful rlw_driver_load (macos_live_load_resume): a game
 * saved from the original pause list (mode16) continues as after the original
 * load lists (GameLoad 0x46b88d, then MenuInit(18) + HUD latch 0x4e05a2),
 * through the pause menu's own "Voltar ao jogo" item. 0 and nothing changed
 * for any other save. */
int rlw_driver_load_resume(RlwDriver *d);
void rlw_driver_save_error(const RlwDriver *d, char *out, size_t capacity);
/* 1 before a stage's first rlw_driver_advance (its load tail): the host takes the
 * stage autosave right after that advance succeeds. */
int rlw_driver_stage_start(const RlwDriver *d);
/* Current stage file name ("MONDE01.ETA"; "MENU" in the original main menu). */
const char *rlw_driver_mission(const RlwDriver *d);
/* The current world's clock (rl_relogio): tick and milliseconds. */
int rlw_driver_clock(const RlwDriver *d, uint64_t *tick, uint32_t *milliseconds);
/* Host messages queued by rlw_driver_event and not yet given to a frame. */
uint32_t rlw_driver_pending_input(const RlwDriver *d);
/* Any thread, one job at a time, no driver: companion first, then the slot
 * through a temporary + flush + atomic replace (a failure keeps the old save). */
int rlw_save_commit(RlwSaveJob *job);
void rlw_save_job_error(const RlwSaveJob *job, char *out, size_t capacity);
void rlw_save_job_free(RlwSaveJob *job);
/* Pending SAVE_SLOT (in-game list 14): modern save of the paused game to path
 * (*saved = its result), then the original MenuInit(18) resume; returns the
 * resume result (play continues even if the save failed). */
int rlw_shell_save_slot(RlwDriver *d, const char *path, int32_t *saved);
int rlw_shell_save_slot_begin(RlwDriver *d, const char *path, int32_t *saved, RlwSaveJob **job);
/* ---- Port-only "Opções" screen (engine/include/options-screen.md) ----
 * Same contract as MacOSLiveDriver.h's macos_live_options_*: sync creates the
 * screen (first call, from the CD's TITRE.PIE) or updates the host's values;
 * input goes BEFORE rlw_shell_input with the same input (keys may add
 * RL_OPCOES_ESQUERDA/DIREITA); when out->consumed the shell must not get it and
 * rlw_driver_pixels/palette are already the screen to show. Never touches the
 * shell session; nothing is consumed without one. */
int rlw_options_sync(RlwDriver *d, const RlOpcoesValores *values);
int rlw_options_input(RlwDriver *d, const RlShellInput *input, uint32_t keys, RlOpcoesEvento *out);
uint32_t rlw_options_open(const RlwDriver *d);
int rlw_options_button(RlwDriver *d, int32_t rect[4]);
/* Jogo > Opções… (macos_live_options_show): opens the screen over the main or
 * pause menu; rlw_options_redraw repaints an open screen after a host sync. */
int rlw_options_show(RlwDriver *d);
int rlw_options_redraw(RlwDriver *d);
/* Rule set (RL_REGRA_*) for worlds created afterwards: "Atalhos modernos" turns
 * on the Shift+click order queue (RL_REGRA_FILA_ORDENS) and "Caminhos melhores"
 * the modern unit paths (RL_REGRA_CAMINHOS_MELHORES), as on macOS. The menu
 * world takes it too while its load tail has not run. Default 0 (original). */
void rlw_driver_default_rules(uint32_t rules);
int rlw_driver_set_rules(RlwDriver *d, uint32_t rules);
/* NativeEngineRules.current(): the rule set of the two Opções items. */
uint32_t rlw_options_rules(unsigned atalhos, unsigned caminhos);
/* Rule set of the current world (rl_regras; macos_live_rules), 0 on error. */
uint32_t rlw_driver_rules(const RlwDriver *d);
/* "Atalhos modernos" read-only snapshot (macos_live_peek): in play only (no
 * shell, not stopped), between frames. Copies each (address, size) range of one
 * complete projection to out, concatenated in request order. 0 or an error. */
int rlw_driver_peek(RlwDriver *d, const uint32_t *addresses, const uint32_t *sizes, uint32_t count, uint8_t *out);
/* Pending Shift+click waypoints of one unit (rl_fila_ordens; macos_live_order_queue),
 * play only (no shell). Read-only; 0 or an error. */
int rlw_driver_order_queue(RlwDriver *d, uint32_t unit, uint16_t *cells, uint32_t capacity, uint32_t *count);
/* Preset (0..2) of the next stage start: New game, Restart or Next stage. */
int rlw_driver_set_difficulty(RlwDriver *d, uint32_t difficulty);

/* ---- Sound requests (front 2, audio) ----
 * Same contract and record layout as MacOSLiveDriver.h's macos_live_sound_*
 * (MacOSSoundEvent in MacOSAudioMapper.h): once enabled, every successful play
 * input phase appends its kind-10 PostSound/StopSound/StopAllSound copies plus
 * the WinMain loop's PlaySnd_Manage ticks. The host drains them after each
 * rlw_driver_advance; game state is never touched. A new world clears the queue. */
enum { RLW_SOUND_MANAGE = 0, RLW_SOUND_POST = 1, RLW_SOUND_STOP = 2, RLW_SOUND_STOP_ALL = 3 };
typedef struct { uint32_t kind, frame, phase, site, eax, edx, ebx, ecx, volume, has_volume; } RlwSoundEvent;
int rlw_driver_sound_capture(RlwDriver *d, uint32_t enabled);
uint32_t rlw_driver_sound_events(RlwDriver *d, RlwSoundEvent *out, uint32_t capacity);
uint64_t rlw_driver_sound_dropped(const RlwDriver *d);
/* Identity of the current world (changes on load, next stage, restart, resume):
 * the host resets its playback when it changes (NativeAudioPlayback.reset). */
uint64_t rlw_driver_sound_world(const RlwDriver *d);
/* The current world's frame events (rl_eventos_quadro; macos_live_cd_events):
 * the CD music requests are kind 2, id 102 (MCICD_Play), registers[0] the track,
 * registers[1] & 255 the loop flag, site the caller. 0 or the ABI code. */
int rlw_driver_cd_events(RlwDriver *d, RlFrameEvent *out, uint32_t capacity, uint32_t *required);
/* In-game FLC events (event-flc.md), MacOSLiveDriver's macos_live_event_flc_*:
 * with host playback on (the window's default; off in smoke, parity and demo
 * runs, which keep ending an event at once) the driver leaves a running event
 * to the host, which polls the state after each frame (idle in a shell
 * session), asks the name of the declared entry and calls finish once after the
 * last frame. */
void rlw_driver_event_flc_host(RlwDriver *d, int enabled);
int rlw_driver_event_flc_state(RlwDriver *d, RlEventFlcStateV1 *out);
int rlw_driver_event_flc_finish(RlwDriver *d);
int rlw_driver_event_flc_name(RlwDriver *d, uint32_t index, char out[16]);
#endif
