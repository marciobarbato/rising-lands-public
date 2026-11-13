#ifndef MACOS_LIVE_DRIVER_H
#define MACOS_LIVE_DRIVER_H
#include <stdint.h>
#include "rising_engine.h"
#include "rising_shell.h"
#include "rising_options.h"
#include "MacOSAudioMapper.h"
typedef struct MacOSLiveDriver MacOSLiveDriver;
typedef struct {
 uint64_t world, generation, frames, surfaces_equal, phases_equal, checkpoints_equal;
 uint64_t logic_ns, render_ns, proof_ns;
 uint32_t phase_index, disposition, stopped, stop_phase, counter;
 uint8_t state_sha[32], surface_sha[32];
 char message[512];
} MacOSLiveStatus;
/* Opaque app handle owns host buffers and Rust world/renderer; serialize all calls.
 * phase observer runs only in proof tooling, after each successful actual phase.
 * No callback is used by the normal native UI. */
typedef int32_t (*MacOSPhaseObserver)(void *,uint64_t,uint32_t,uint32_t);
uint32_t macos_live_abi_version(void);
uint32_t macos_live_engine_binding(void);
uint32_t macos_live_status_size(void);
MacOSLiveDriver *macos_live_create(const char *library,const char *inputs,char *error,uint32_t capacity);
/* Asset-based production startup. Historical create is restricted to explicit
 * validation tools and the shell's --debug-s0 switch. No automatic fallback. */
MacOSLiveDriver *macos_live_create_mission(const char *library,const char *inputs,const char *name,uint32_t difficulty,char *error,uint32_t capacity);
/* Select before load_tail: 0 = production play with original text startup;
 * 1 = historical no-text validation (default); 2 = historical scripted replay
 * with engine input audits and no text startup. Replay is CLI-only.
 * Play mode performs no projection, hash, comparison or observer callback. */
int32_t macos_live_validation(MacOSLiveDriver *,uint32_t);
void macos_live_audit_window(MacOSLiveDriver *,uint32_t);
int32_t macos_live_advance(MacOSLiveDriver *,MacOSPhaseObserver,void *);
/* ABI1.5 descriptor is copied synchronously; arrays must outlive the call. */
typedef struct {uint32_t message,wparam,lparam;} MacOSInputMessage;
typedef struct {uint32_t vk;int32_t value;} MacOSInputKey;
typedef struct {
 uint32_t size,flags;int32_t cursor_x,cursor_y;
 const MacOSInputMessage *messages;uint32_t message_count;
 const MacOSInputKey *keys;uint32_t key_count;
} MacOSDeclaredInput;
/* Input phases are the same calls used by play; this test entry omits rendering.
 * Production after successful load-tail, in exact 2/3 order; historical replay
 * remains frame128+. Human phase2 samples the retained host cursor even without
 * a mouse packet; phase1 never samples it. Context/world never reloaded. */
int32_t macos_live_human_phase(MacOSLiveDriver *,uint32_t);
int32_t macos_live_input_phase(MacOSLiveDriver *,uint32_t,const MacOSDeclaredInput *);
int32_t macos_live_frame_context(MacOSLiveDriver *,uint8_t *,uint32_t);
/* Mac events are scheduled at a game frame, independent of presentation/clock.
 * kind1 mouse,2 key. Returns 0 or a latched named input/queue error. */
int32_t macos_live_event(MacOSLiveDriver *,uint32_t kind,uint32_t message,int32_t x,int32_t y,uint32_t wparam,uint32_t vk,uint32_t down);
/* Left press (0x201/0x203) stays held until 0x202, never released before the
 * fixed click gesture's k+3; moves while held carry MK_LBUTTON. A press while
 * held first queues the missing release at the last held position. The app
 * sends its own release on focus loss; a held press refuses save (busy). */
/* Optional test/benchmark diagnostics; never called by normal play. */
uint32_t macos_live_order_tags(MacOSLiveDriver *,uint32_t *,uint32_t);
uint64_t macos_live_human_events(MacOSLiveDriver *);
/* Play-mode sound requests: enable after create; each successful input phase
 * appends its kind-10 PostSound/StopSound copies plus PlaySnd_Manage ticks.
 * Draining is the app's only consumer; game state and audits are untouched. */
int32_t macos_live_sound_capture(MacOSLiveDriver *,uint32_t enabled);
uint32_t macos_live_sound_events(MacOSLiveDriver *,MacOSSoundEvent *,uint32_t capacity);
uint64_t macos_live_sound_dropped(MacOSLiveDriver *);
uint64_t macos_live_declared_frames(MacOSLiveDriver *);
void macos_live_status(MacOSLiveDriver *,MacOSLiveStatus *);
uint64_t macos_live_renderer(MacOSLiveDriver *);
const uint8_t *macos_live_pixels(MacOSLiveDriver *);
const uint8_t *macos_live_palette(MacOSLiveDriver *);
int32_t macos_live_read(MacOSLiveDriver *,uint32_t,uint32_t,uint8_t *);
/* "Atalhos modernos" (play only, read-only): copies each (address,size) range of
 * ONE world projection into out, concatenated. Call between frames on the work
 * queue. Refused in validation, menus/shell and after a stop. */
int32_t macos_live_peek(MacOSLiveDriver *,const uint32_t *addresses,const uint32_t *sizes,uint32_t count,uint8_t *out);
/* Inputs queued by macos_live_event and not yet consumed by a game frame. */
uint32_t macos_live_pending_input(MacOSLiveDriver *);
/* World rules (RL_REGRA_*). default_rules: production worlds created afterwards
 * (0 when the engine lacks rl_regras_definir). set_rules: the current world,
 * only before its load tail (the menu world); 0 or the ABI error. rules: the
 * current world's set. order_queue: one unit's pending Shift+click waypoints
 * (rl_fila_ordens), play only; 0 or an error. */
void macos_live_default_rules(uint32_t rules);
int32_t macos_live_set_rules(MacOSLiveDriver *,uint32_t rules);
uint32_t macos_live_rules(MacOSLiveDriver *);
int32_t macos_live_order_queue(MacOSLiveDriver *,uint32_t unit,uint16_t *cells,uint32_t capacity,uint32_t *count);
/* Complete-frame play snapshots, using only the engine save extension. Errors
 * are separate from the simulation stop latch. Load publishes world+renderer
 * together; host input and device playback are reset by the shell. */
int32_t macos_live_save(MacOSLiveDriver *,const char *path);
int32_t macos_live_load(MacOSLiveDriver *,const char *path);
int32_t macos_live_autosave(MacOSLiveDriver *,const char *path);
void macos_live_save_error(MacOSLiveDriver *,char *,uint32_t);
/* Background save: macos_live_save_begin (same checks as macos_live_save) and
 * macos_live_shell_save_slot_begin capture the slot bytes, their SHA-256 and the
 * presentation companion on the game worker. macos_live_save_commit writes the
 * companion, then the slot through temporary + fsync + rename (a failure keeps
 * the previous save) and needs no driver: any thread, one job at a time. */
typedef struct MacOSSaveJob MacOSSaveJob;
int32_t macos_live_save_begin(MacOSLiveDriver *,const char *path,MacOSSaveJob **job);
int32_t macos_live_save_commit(MacOSSaveJob *);
void macos_live_save_job_error(MacOSSaveJob *,char *,uint32_t);
void macos_live_save_job_free(MacOSSaveJob *);
typedef struct {uint32_t outcome,stage,has_next;char next[16];} MacOSCampaignEnd;
/* Read once at the original MenuInit mission-end boundary, never per frame.
 * outcome: 0 ongoing, 1 victory, 2 defeat. No world/RNG mutation. */
int32_t macos_live_campaign_end(MacOSLiveDriver *,MacOSCampaignEnd *);
int32_t macos_live_campaign_screen(MacOSLiveDriver *,uint32_t *);
/* Explicit confirmation discards unsent old-stage host input. Engine FIFO must
 * be empty. Publish a new world/renderer/autosave only after successful setup. */
int32_t macos_live_continue(MacOSLiveDriver *,uint32_t difficulty);
/* Real prepared result/briefing session. Source gameplay world is retained
 * until successful campaign load and first picture. Input is serialized with
 * gameplay; a returned request must be completed before another input. */
int32_t macos_live_shell_begin(MacOSLiveDriver *);
uint32_t macos_live_shell_mode(MacOSLiveDriver *);
int32_t macos_live_shell_state(MacOSLiveDriver *,RlShellInput *);
int32_t macos_live_shell_input(MacOSLiveDriver *,const RlShellInput *,RlShellEvent *);
int32_t macos_live_shell_prepare(MacOSLiveDriver *);
int32_t macos_live_shell_show(MacOSLiveDriver *);
/* Pending RESET_CAMPAIGN request: mode17 ("Novo jogo") runs the engine's
 * MissionReset/MenuInit(17) and paints the briefing; mode13 and RESUME are an
 * explicit RL_UNSUPPORTED_PATH (the app shows its modern load menu for 13).
 * SHOW 12/13/14 likewise stays with the app. Failures change nothing. */
int32_t macos_live_shell_request(MacOSLiveDriver *);
/* Host captions for the original save lists, set before completing a request
 * that opens one: 10 x [present 0/1, 32-byte CP1252 caption], slot index0..9 =
 * original save1.sav..save9.sav,save0.sav = modern posicao-1..posicao-10.
 * LOAD_SLOT/SAVE_SLOT events carry that index in event.reserved. */
int32_t macos_live_shell_slots(MacOSLiveDriver *,const uint8_t *table,uint32_t length);
/* Pending SAVE_SLOT (in-game list 14): modern save of the paused game to path,
 * result in *saved, then the original MenuInit(18) resume; returns the resume
 * result (play continues even if the save failed; the error stays readable). */
int32_t macos_live_shell_save_slot(MacOSLiveDriver *,const char *path,int32_t *saved);
int32_t macos_live_shell_save_slot_begin(MacOSLiveDriver *,const char *path,int32_t *saved,MacOSSaveJob **job);
/* Read-only: bytes of the open session's memory; mouse zone rectangle
 * {left,top,right,bottom} of the current screen. */
int32_t macos_live_shell_peek(MacOSLiveDriver *,uint32_t address,uint32_t size,uint8_t *out);
int32_t macos_live_shell_zone(MacOSLiveDriver *,uint32_t zone,int32_t rect[4]);
/* Shell clock for a mouse press (press=1) or release/move (0) in modes 17/23. */
uint32_t macos_live_shell_click_clock(uint32_t mode,uint32_t clock,uint32_t press);
int32_t macos_live_shell_load(MacOSLiveDriver *);
/* Mode10: one AffCredits frame (see rl_shell_credits_frame; ENDED has already
 * returned to main), then the host's decoded FLC frame for a DRAW state. */
int32_t macos_live_shell_credits_frame(MacOSLiveDriver *,uint32_t elapsed_ms,RlShellCredits *);
int32_t macos_live_shell_credits_present(MacOSLiveDriver *,const RlShellCredits *,const uint8_t *pixels,const uint8_t *palette);
int32_t macos_live_shell_movie_finish(MacOSLiveDriver *,uint32_t completion);
int32_t macos_live_shell_draw(MacOSLiveDriver *,uint32_t animation);
/* Port-only "Opções" screen (engine/include/options-screen.md) over the main
 * menu and the pause menu. sync: create (first call, from the CD's TITRE.PIE)
 * or update the host's values; RL_UNSUPPORTED_PATH when the engine has no
 * options symbols (no button then). input: call BEFORE macos_live_shell_input
 * with the same input; when out->consumed the shell must not get it, and the
 * driver's picture (macos_live_pixels/palette) is already the screen to show.
 * Never touches the shell session; refused (consumed=0) without a session. */
int32_t macos_live_options_sync(MacOSLiveDriver *,const RlOpcoesValores *);
int32_t macos_live_options_input(MacOSLiveDriver *,const RlShellInput *,uint32_t keys,RlOpcoesEvento *);
uint32_t macos_live_options_open(MacOSLiveDriver *);
/* Opens the screen without the button (menu/keyboard shortcut): 0, or
 * RL_INVALID_ARGUMENT outside main2/pause16 (or with a pending request),
 * RL_UNSUPPORTED_PATH without the extension. The picture is ready on 0. */
int32_t macos_live_options_show(MacOSLiveDriver *);
int32_t macos_live_options_button(MacOSLiveDriver *,int32_t rect[4]);
int32_t macos_live_selection(MacOSLiveDriver *,char *mission,uint32_t capacity,uint32_t *difficulty);
/* Preset (0..2) of the next stage start: New game, Restart or Next stage. */
int32_t macos_live_set_difficulty(MacOSLiveDriver *,uint32_t difficulty);
/* Explicit host mapping, copied for this driver's lifetime and checked again
 * before saved candidate publication. Supply via events_host_file.h's authorized
 * directory capability reader. No implicit CWD or logical-directory default.
 * Legacy create exports retain their signatures and unbound filename semantics. */
MacOSLiveDriver *macos_live_create_mission_eventos_v1(const char*,const char*,const char*,uint32_t,const RlEventsHostV1*,char*,uint32_t);
/* Authorized packaged directory plus explicit logical namespace; no fallback. */
MacOSLiveDriver *macos_live_create_mission_host_v1(const char*,const char*,const char*,uint32_t,const uint8_t*,uint32_t,char*,uint32_t);
/* Original program start into the main menu (MenuInit 2; no stage, no frame
 * state, no renderer), with an open prepared session painting titre.pie.
 * Shell SHOW/RESET_CAMPAIGN(17) requests are completed by the engine's general
 * preparation; START_MISSION after "Novo jogo" loads MONDE01 (play mode). */
MacOSLiveDriver *macos_live_create_menu_host_v1(const char*,const char*,uint32_t difficulty,const uint8_t*,uint32_t,char*,uint32_t);
int32_t macos_live_cd_events(MacOSLiveDriver *,RlFrameEvent *,uint32_t,uint32_t *);
void macos_live_free(MacOSLiveDriver *);
/* In-game FLC events (lane B, event-flc.md): same layouts as RlEventFlcV1 /
 * RlEventFlcStateV1. The table is built from EVENTS.INI and events/NAME.FLC. */
typedef struct { uint8_t name[16]; uint32_t available; uint8_t header[128]; } MacOSEventFlcEntry;
typedef struct { uint32_t running,modal,index,frames,width,height,speed; } MacOSEventFlcState;
int32_t macos_live_event_flc_table(const char *root,const char *events,MacOSEventFlcEntry *,uint32_t capacity,uint32_t *count);
int32_t macos_live_event_flc_state(MacOSLiveDriver *,MacOSEventFlcState *);
int32_t macos_live_event_flc_finish(MacOSLiveDriver *);
int32_t macos_live_event_flc_name(MacOSLiveDriver *,uint32_t index,char *,uint32_t capacity);
uint32_t macos_live_event_flc_supported(void);
#endif
