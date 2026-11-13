/* "Atalhos modernos" (AoE2-style shortcuts) of the Windows host. Port of
 * macos/src/NativeModernHotkeys.swift: host layer only.
 * Every command becomes ordinary key/mouse input the original already accepts,
 * delivered through the player's own path (rlw_driver_event). Nothing here
 * writes game state; the game is read through a read-only projection
 * (rlw_driver_peek) between frames. Portable C (unit tests on any OS).
 *
 * Original facts used (engine/crates/rl-core/src), as on macOS:
 * - player_input.rs `group`: Ctrl+N saves group N; N selects it; Shift+N selects
 *   and centres (EnginGetCentreGroupe). Only 1..9.
 * - player_input.rs `input_context`: Enter or X centre on the last attack.
 * - player_input.rs `zone`/`map`: zone 0 (no MouseZone rectangle) is the map. A
 *   left press+release on your own unit/building selects it; a right press
 *   outside zone 11 deselects everything and issues no order.
 * - player_input.rs `minimap`: with nothing selected a left press on zone 10
 *   recentres the view (`minimap_center`); with a selection it orders a move. */
#ifndef RLW_MODERN_H
#define RLW_MODERN_H
#include <stdint.h>

/* NativeModernStep. */
enum { RLW_STEP_KEY, RLW_STEP_MOUSE, RLW_STEP_WAIT };
typedef struct { uint8_t type, down; uint16_t code; int16_t x, y; } RlwStep;
/* True when the driver accepts this input (the 23 keys, mouse 0x200..0x205 in
 * 800x600) or it is a barrier. */
int rlw_step_is_original(const RlwStep *s);

#define RLW_SNAPSHOT_UNITS 800
#define RLW_SNAPSHOT_STRUCTURES 64
#define RLW_STRUCTURE_CELLS 64
typedef struct { int index, owner, kind, state, cell, on_map; } RlwSnapUnit;
typedef struct { int index, owner, type, state, cell_count; int cells[RLW_STRUCTURE_CELLS]; } RlwSnapStructure;
typedef struct { int id, x0, y0, x1, y1; } RlwSnapZone;
/* NativeModernSnapshot. */
typedef struct {
 int in_mission, local_player;
 int view_x, view_y, view_w, view_h, map_w, map_h, margin;
 int minimap_visible, minimap_x, minimap_y, minimap_w, minimap_h;
 int zone_count; RlwSnapZone zones[64];
 int selected_units, selected_structure;
 int unit_count; RlwSnapUnit units[RLW_SNAPSHOT_UNITS];
 int structure_count; RlwSnapStructure structures[RLW_SNAPSHOT_STRUCTURES];
} RlwSnapshot;

/* Fills ONE buffer with the ranges concatenated in request order (rlw_driver_peek). 0 = success. */
typedef int (*RlwPeek)(void *ctx, const uint32_t *addresses, const uint32_t *sizes, uint32_t count, uint8_t *out);
void rlw_snapshot_defaults(RlwSnapshot *s);
/* 0 on success (also outside a mission, with in_mission = 0); nonzero on any failure. */
int rlw_snapshot_read(RlwPeek peek, void *ctx, RlwSnapshot *out);
int rlw_snapshot_zone(const RlwSnapshot *s, int x, int y);
int rlw_snapshot_click_pixel(const RlwSnapshot *s, int cell, int *x, int *y);
int rlw_snapshot_deselect_pixel(const RlwSnapshot *s, int *x, int *y);
void rlw_snapshot_minimap_origin(const RlwSnapshot *s, int dx, int dy, int *x, int *y);
int rlw_snapshot_minimap_press(const RlwSnapshot *s, int cell, int *x, int *y);

/* NativeModernPlanner. command = RLW_MODERN_* (rlw_hotkeys.h). */
enum { RLW_TARGET_NONE, RLW_TARGET_UNIT, RLW_TARGET_STRUCTURE };
typedef struct { int kind, index; } RlwTarget;
typedef struct { int last[3]; } RlwPlanner;
enum { RLW_STAGE_DELIVER, RLW_STAGE_FINISH, RLW_STAGE_ABORT };
typedef struct { int kind, count; RlwStep steps[2]; } RlwStage;
typedef struct { int deselected, recentered; } RlwModernProgress;
void rlw_planner_init(RlwPlanner *p);
/* Next idle worker/army unit or own sanctuary after the previous choice, cycling. 1 = chosen. */
int rlw_planner_choose(RlwPlanner *p, int command, const RlwSnapshot *s, RlwTarget *out);
/* One stage: deselect (verified), recentre with the minimap (verified), then click. */
RlwStage rlw_planner_next(RlwTarget target, const RlwSnapshot *s, RlwModernProgress *progress);
/* Fixed steps of the key-only commands (NativeModernHotkeys.centerGroup/lastAlert). */
int rlw_modern_center_group(uint32_t digit_vk, RlwStep out[5]);
int rlw_modern_last_alert(RlwStep out[3]);

/* NativeModernJob: carries one command across game frames; pump runs between
 * frames after each advance. */
typedef struct {
 void *ctx;
 /* The player's input path. 0 = delivered; nonzero = refused. */
 int (*deliver)(void *ctx, const RlwStep *step);
 uint32_t (*pending_input)(void *ctx);
 uint64_t (*frames)(void *ctx);
 /* Fresh read-only snapshot, or NULL. */
 const RlwSnapshot *(*snapshot)(void *ctx);
 /* Player pointer to return the game cursor to. */
 void (*pointer)(void *ctx, int *x, int *y);
} RlwModernHost;
enum { RLW_JOB_RUNNING, RLW_JOB_FINISHED, RLW_JOB_ABORTED };
/* A command never takes longer than this many frames (about 10 s at 1x). */
#define RLW_JOB_FRAME_BUDGET 300u
typedef struct {
 int state, replan; RlwTarget target;
 RlwStep queue[8]; int queue_length;
 RlwModernProgress progress;
 int waiting, started_set; uint64_t wait_from, started;
 uint8_t held[256]; int moved_mouse;
} RlwModernJob;
void rlw_job_steps(RlwModernJob *job, const RlwStep *steps, int count);
void rlw_job_target(RlwModernJob *job, RlwTarget target);
int rlw_job_uses_mouse(const RlwModernJob *job);
void rlw_job_pump(RlwModernJob *job, const RlwModernHost *host);
#endif
