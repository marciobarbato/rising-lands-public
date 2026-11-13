/* Port of NativeModernHotkeys.swift (NativeModernSnapshot, NativeModernPlanner,
 * NativeModernJob): same addresses, rules and stage order. See rlw_modern.h. */
#include "rlw_modern.h"
#include "rlw_hotkeys.h"
#include <stdlib.h>
#include <string.h>

int rlw_step_is_original(const RlwStep *s) {
 switch (s->type) {
 case RLW_STEP_KEY: return rlw_accepted_vk(s->code);
 case RLW_STEP_MOUSE: return s->code >= 0x200 && s->code <= 0x205 && s->x >= 0 && s->x < 800 && s->y >= 0 && s->y < 600;
 case RLW_STEP_WAIT: return 1;
 default: return 0;
 }
}
static RlwStep key_step(uint32_t vk, int down) { RlwStep s = {RLW_STEP_KEY, (uint8_t)(down != 0), (uint16_t)vk, 0, 0}; return s; }
static RlwStep mouse_step(uint32_t msg, int x, int y) { RlwStep s = {RLW_STEP_MOUSE, 0, (uint16_t)msg, (int16_t)x, (int16_t)y}; return s; }
static RlwStep wait_step(void) { RlwStep s = {RLW_STEP_WAIT, 0, 0, 0, 0}; return s; }

/* Second tap of digit N: the original's own Shift+N. Shift stays held until a
 * game frame has consumed the digit message (`group` polls Shift in that frame). */
int rlw_modern_center_group(uint32_t digit_vk, RlwStep out[5]) {
 if (digit_vk < 49 || digit_vk > 57) return 0;
 out[0] = key_step(16, 1); out[1] = key_step(digit_vk, 1); out[2] = key_step(digit_vk, 0); out[3] = wait_step(); out[4] = key_step(16, 0);
 return 5;
}
/* The original's X: centre on the last attack (Enter is the other original key). */
int rlw_modern_last_alert(RlwStep out[3]) { out[0] = key_step(88, 1); out[1] = wait_step(); out[2] = key_step(88, 0); return 3; }

/* ---------- NativeModernSnapshot ---------- */

void rlw_snapshot_defaults(RlwSnapshot *s) {
 memset(s, 0, sizeof(*s));
 s->in_mission = 1; s->view_w = 25; s->view_h = 17; s->map_w = 128; s->map_h = 128;
 s->minimap_visible = 1; s->minimap_w = 128; s->minimap_h = 128; s->selected_structure = 0xffff;
}
static int u8(const uint8_t *b, size_t o) { return b[o]; }
static int u16(const uint8_t *b, size_t o) { return b[o] | (b[o + 1] << 8); }
static uint32_t u32(const uint8_t *b, size_t o) { return b[o] | ((uint32_t)b[o + 1] << 8) | ((uint32_t)b[o + 2] << 16) | ((uint32_t)b[o + 3] << 24); }
static int i32(const uint8_t *b, size_t o) { return (int)(int32_t)u32(b, o); }

int rlw_snapshot_read(RlwPeek peek, void *ctx, RlwSnapshot *s) {
 static const uint32_t first_a[11] = {0x4e2f78, 0x508041, 0x508720, 0x497168, 0x508f90, 0x497285, 0x507d68, 0x496fe0, 0x50a9b4, 0x4e19a0, 0x4e0f38};
 static const uint32_t first_s[11] = {1, 1, 8, 4, 1, 1, 4, 4, 4, 4, 4};
 uint8_t a[36];
 if (peek(ctx, first_a, first_s, 11, a)) return 1;
 rlw_snapshot_defaults(s);
 s->in_mission = u8(a, 0) == 0x12;
 s->local_player = u8(a, 1);
 s->view_x = u16(a, 2); s->view_y = u16(a, 4); s->view_w = u16(a, 6); s->view_h = u16(a, 8);
 s->map_w = u16(a, 10); s->map_h = u16(a, 12);
 s->margin = (int8_t)a[14];
 s->minimap_visible = a[15] != 0;
 s->minimap_x = u16(a, 16); s->minimap_y = u16(a, 18); s->minimap_w = u16(a, 20); s->minimap_h = u16(a, 22);
 int zone_first = u16(a, 24), zone_last = u16(a, 26);
 uint32_t unit_base = u32(a, 28), grid_base = u32(a, 32);
 if (!s->in_mission || s->local_player >= 4 || zone_first > zone_last || zone_last >= 64 || s->map_h > 256) return s->in_mission ? 1 : 0;
 uint32_t p = (uint32_t)s->local_player, zones = (uint32_t)(zone_last - zone_first + 1);
 uint32_t second_a[5] = {0x5099b4 + 16 * (uint32_t)zone_first, 0x508728 + 512 * p, 0x495be0 + 2 * p, unit_base, 0x49a0c8};
 uint32_t second_s[5] = {16 * zones, 2, 2, 800 * 276, 1000 * 276};
 size_t total = 0; for (int i = 0; i < 5; i++) total += second_s[i];
 uint8_t *b = malloc(total); if (!b) return 1;
 if (peek(ctx, second_a, second_s, 5, b)) { free(b); return 1; }
 const uint8_t *bz = b, *bsel = b + second_s[0], *bstr = bsel + 2, *bu = bstr + 2, *bs = bu + 800 * 276;
 s->zone_count = (int)zones;
 for (int i = 0; i < s->zone_count; i++)
  s->zones[i] = (RlwSnapZone){zone_first + i, i32(bz, 16 * i), i32(bz, 16 * i + 4), i32(bz, 16 * i + 8), i32(bz, 16 * i + 12)};
 s->selected_units = u16(bsel, 0);
 s->selected_structure = u16(bstr, 0);
 s->unit_count = 0;
 for (int i = 0; i < 800; i++) {
  size_t o = 276 * (size_t)i; int state = u8(bu, o + 2);
  if (state == 0 || state == 4) continue;
  s->units[s->unit_count++] = (RlwSnapUnit){i, u8(bu, o), u8(bu, o + 1), state, u16(bu, o + 8), u8(bu, o + 0x72) != 0};
 }
 /* Own sanctuaries only: their solid cells come from the hover grid word (bit 15
  * set, bits 5..14 = structure index), as bin/victory_bot/view.rs does. */
 int own_index[RLW_SNAPSHOT_STRUCTURES], own_origin[RLW_SNAPSHOT_STRUCTURES], own_state[RLW_SNAPSHOT_STRUCTURES], own = 0;
 for (int i = 0; i < 1000 && own < RLW_SNAPSHOT_STRUCTURES; i++) {
  size_t o = 276 * (size_t)i; int state = u8(bs, o + 5), owner = u8(bs, o + 4), type = u8(bs, o + 8);
  if (state != 0 && owner == s->local_player && type == 2) { own_index[own] = i; own_origin[own] = u16(bs, o + 2); own_state[own] = state; own++; }
 }
 free(b);
 if (!own) return 0;
 uint32_t third_a[2] = {0x495d8c + 52 * 2, grid_base}, third_s[2] = {4, 2 * 256 * (uint32_t)s->map_h};
 uint8_t *c = malloc(4 + (size_t)third_s[1]); if (!c) return 1;
 if (peek(ctx, third_a, third_s, 2, c)) { free(c); return 1; }
 int fw = u16(c, 0), fh = u16(c, 2); const uint8_t *grid = c + 4; size_t grid_size = third_s[1];
 s->structure_count = own;
 for (int k = 0; k < own; k++) {
  RlwSnapStructure *t = &s->structures[k];
  memset(t, 0, sizeof(*t)); t->index = own_index[k]; t->owner = s->local_player; t->type = 2; t->state = own_state[k];
  for (int row = 0; row < fh; row++) for (int col = 0; col < fw; col++) {
   int cell = (own_origin[k] + row * 256 + col) & 0xffff;
   if (2 * (size_t)cell + 1 >= grid_size) continue;
   int word = u16(grid, 2 * (size_t)cell);
   if ((word & 0x8000) && ((word & 0x7fe0) >> 5) == own_index[k] && t->cell_count < RLW_STRUCTURE_CELLS) t->cells[t->cell_count++] = cell;
  }
 }
 free(c);
 return 0;
}

/* MouseZone rectangles in test order (first match wins; none = map, zone 0). */
int rlw_snapshot_zone(const RlwSnapshot *s, int x, int y) {
 for (int i = 0; i < s->zone_count; i++) {
  const RlwSnapZone *z = &s->zones[i];
  if (z->x0 <= x && z->y0 <= y && x <= z->x1 && y <= z->y1) return z->id;
 }
 return 0;
}

/* Pixel of a cell's centre in the current view, if a click there reaches the
 * map handler on that cell (inside the view minus margin, zone 0). */
int rlw_snapshot_click_pixel(const RlwSnapshot *s, int cell, int *px, int *py) {
 int x = ((cell & 255) - s->view_x) * 32 + 16, y = ((cell >> 8) - s->view_y) * 32 + 16, m = s->margin * 32;
 if (x < m || y < m || x >= s->view_w * 32 - m || y >= s->view_h * 32 - m || x >= 800 || y >= 600) return 0;
 if (rlw_snapshot_zone(s, x, y) != 0) return 0;
 *px = x; *py = y; return 1;
}

/* A map pixel for a right press that only deselects: zone 0, inside the view. */
int rlw_snapshot_deselect_pixel(const RlwSnapshot *s, int *px, int *py) {
 int m = s->margin * 32 + 8, w = s->view_w * 32, h = s->view_h * 32;
 int cx[9] = {w / 2, w * 1 / 4, w * 3 / 4, w * 1 / 4, w * 3 / 4, m, w - m - 1, m, w - m - 1};
 int cy[9] = {h / 2, h * 1 / 4, h * 1 / 4, h * 3 / 4, h * 3 / 4, m, m, h - m - 1, h - m - 1};
 for (int i = 0; i < 9; i++) {
  int x = cx[i], y = cy[i];
  if (x >= m && y >= m && x < w - m && y < h - m && x < 800 && y < 600 && rlw_snapshot_zone(s, x, y) == 0) { *px = x; *py = y; return 1; }
 }
 return 0;
}

/* Exact replica of the original `minimap_center` (player_input.rs) for a press at (dx, dy). */
void rlw_snapshot_minimap_origin(const RlwSnapshot *s, int dx, int dy, int *ox, int *oy) {
 int bx = (dx - (s->view_w * s->minimap_w / s->map_w) / 2) * s->map_w / s->minimap_w;
 int by = (dy - (s->view_h * s->minimap_h / s->map_h) / 2) * s->map_h / s->minimap_h;
 if ((int16_t)bx < 0) bx = 0;
 if ((int16_t)by < 0) by = 0;
 if ((int)(int16_t)bx + s->view_w > s->map_w) bx = s->map_w - s->view_w;
 if ((uint32_t)(s->view_h + (int)(int16_t)by) > (uint32_t)s->map_h) by = s->map_h - s->view_h;
 *ox = bx; *oy = by;
}

/* The 8 offsets whose origin is closest to want (ties keep the lower offset, as
 * Swift's stable sort does). */
static int closest(const RlwSnapshot *s, int count, int axis, int want, int out[8]) {
 int n = 0; uint8_t used_small[1024]; uint8_t *used = count <= 1024 ? used_small : calloc((size_t)count, 1);
 if (!used) return 0;
 if (used == used_small) memset(used_small, 0, sizeof(used_small));
 while (n < 8 && n < count) {
  int best = -1, best_d = 0;
  for (int i = 0; i < count; i++) {
   if (used[i]) continue;
   int ox, oy; rlw_snapshot_minimap_origin(s, axis ? 0 : i, axis ? i : 0, &ox, &oy);
   int d = abs((axis ? oy : ox) - want);
   if (best < 0 || d < best_d) { best = i; best_d = d; }
  }
  used[best] = 1; out[n++] = best;
 }
 if (used != used_small) free(used);
 return n;
}

/* Minimap press pixel whose recentring brings `cell` closest to the view centre,
 * or 0 when the minimap is hidden or not pressable. */
int rlw_snapshot_minimap_press(const RlwSnapshot *s, int cell, int *px, int *py) {
 if (!s->minimap_visible || s->map_w <= 0 || s->map_h <= 0 || s->minimap_w <= 0 || s->minimap_h <= 0) return 0;
 int max_x = s->map_w - s->view_w > 0 ? s->map_w - s->view_w : 0, max_y = s->map_h - s->view_h > 0 ? s->map_h - s->view_h : 0;
 int want_x = (cell & 255) - s->view_w / 2, want_y = (cell >> 8) - s->view_h / 2;
 want_x = want_x < 0 ? 0 : want_x > max_x ? max_x : want_x;
 want_y = want_y < 0 ? 0 : want_y > max_y ? max_y : want_y;
 int xs[8], ys[8], nx = closest(s, s->minimap_w, 0, want_x, xs), ny = closest(s, s->minimap_h, 1, want_y, ys);
 for (int j = 0; j < ny; j++) for (int i = 0; i < nx; i++) {
  int x = s->minimap_x + xs[i], y = s->minimap_y + ys[j];
  if (x >= 0 && x < 800 && y >= 0 && y < 600 && rlw_snapshot_zone(s, x, y) == 10) { *px = x; *py = y; return 1; }
 }
 return 0;
}

/* ---------- NativeModernPlanner ---------- */

/* Workers/carriers and army kinds (player_pages.rs 148-157, player_input.rs 85-87);
 * unit state 7 is idle (frame_units.rs arrival, player_orders.rs set_idle). */
static int worker_kind(int k) { return k == 5 || k == 6 || k == 7 || k == 13; }
static int army_kind(int k) {
 static const int kinds[17] = {0, 1, 2, 3, 8, 11, 12, 15, 16, 18, 19, 20, 21, 22, 23, 24, 25};
 for (int i = 0; i < 17; i++) if (kinds[i] == k) return 1;
 return 0;
}
#define IDLE_STATE 7

void rlw_planner_init(RlwPlanner *p) { p->last[0] = p->last[1] = p->last[2] = -1; }

int rlw_planner_choose(RlwPlanner *p, int command, const RlwSnapshot *s, RlwTarget *out) {
 int slot, first = -1, chosen = -1;
 if (command == RLW_MODERN_NEXT_IDLE_WORKER || command == RLW_MODERN_NEXT_IDLE_MILITARY) {
  slot = command == RLW_MODERN_NEXT_IDLE_WORKER ? 0 : 1;
  for (int i = 0; i < s->unit_count; i++) {
   const RlwSnapUnit *u = &s->units[i];
   if (u->owner != s->local_player || u->state != IDLE_STATE || !u->on_map) continue;
   if (slot == 0 ? !worker_kind(u->kind) : !army_kind(u->kind)) continue;
   if (first < 0) first = u->index;
   if (chosen < 0 && u->index > p->last[slot]) chosen = u->index;
  }
 } else if (command == RLW_MODERN_MAIN_STRUCTURE) {
  slot = 2;
  for (int i = 0; i < s->structure_count; i++) {
   const RlwSnapStructure *t = &s->structures[i];
   if (t->owner != s->local_player || t->type != 2 || !t->cell_count) continue;
   if (first < 0) first = t->index;
   if (chosen < 0 && t->index > p->last[slot]) chosen = t->index;
  }
 } else return 0;
 if (first < 0) return 0;
 if (chosen < 0) chosen = first;
 p->last[slot] = chosen;
 out->kind = slot == 2 ? RLW_TARGET_STRUCTURE : RLW_TARGET_UNIT; out->index = chosen;
 return 1;
}

/* Cells a click may use for the target in this snapshot (none if it is gone). */
static int target_cells(RlwTarget t, const RlwSnapshot *s, const int **cells) {
 static int unit_cell;
 if (t.kind == RLW_TARGET_UNIT) {
  for (int i = 0; i < s->unit_count; i++) {
   const RlwSnapUnit *u = &s->units[i];
   if (u->index != t.index) continue;
   if (u->owner != s->local_player || !u->on_map) return 0;
   unit_cell = u->cell; *cells = &unit_cell; return 1;
  }
  return 0;
 }
 if (t.kind == RLW_TARGET_STRUCTURE)
  for (int i = 0; i < s->structure_count; i++)
   if (s->structures[i].index == t.index && s->structures[i].owner == s->local_player) { *cells = s->structures[i].cells; return s->structures[i].cell_count; }
 return 0;
}

RlwStage rlw_planner_next(RlwTarget target, const RlwSnapshot *s, RlwModernProgress *progress) {
 RlwStage st; memset(&st, 0, sizeof(st)); st.kind = RLW_STAGE_ABORT;
 if (!s->in_mission) return st;
 const int *cells = NULL; int n = target_cells(target, s, &cells);
 if (n <= 0) return st;
 int x, y;
 if (s->selected_units != 0 || s->selected_structure != 0xffff) {
  /* A selection could turn the click (or a minimap press) into an order. */
  if (progress->deselected || !rlw_snapshot_deselect_pixel(s, &x, &y)) return st;
  progress->deselected = 1;
  st.kind = RLW_STAGE_DELIVER; st.count = 1; st.steps[0] = mouse_step(0x204, x, y); return st;
 }
 for (int i = 0; i < n; i++)
  if (rlw_snapshot_click_pixel(s, cells[i], &x, &y)) {
   st.kind = RLW_STAGE_FINISH; st.count = 2; st.steps[0] = mouse_step(0x201, x, y); st.steps[1] = mouse_step(0x202, x, y); return st;
  }
 if (progress->recentered || !rlw_snapshot_minimap_press(s, cells[n / 2], &x, &y)) return st;
 progress->recentered = 1;
 st.kind = RLW_STAGE_DELIVER; st.count = 2; st.steps[0] = mouse_step(0x201, x, y); st.steps[1] = mouse_step(0x202, x, y);
 return st;
}

/* ---------- NativeModernJob ---------- */

void rlw_job_steps(RlwModernJob *job, const RlwStep *steps, int count) {
 memset(job, 0, sizeof(*job)); job->state = RLW_JOB_RUNNING;
 if (count > 8) count = 8;
 memcpy(job->queue, steps, sizeof(RlwStep) * (size_t)count); job->queue_length = count;
}
void rlw_job_target(RlwModernJob *job, RlwTarget target) {
 memset(job, 0, sizeof(*job)); job->state = RLW_JOB_RUNNING; job->target = target; job->replan = 1;
}
int rlw_job_uses_mouse(const RlwModernJob *job) { return job->target.kind != RLW_TARGET_NONE; }

static void pop(RlwModernJob *job) { memmove(job->queue, job->queue + 1, sizeof(RlwStep) * (size_t)(job->queue_length - 1)); job->queue_length--; }

static int send(RlwModernJob *job, const RlwStep *step, const RlwModernHost *host) {
 if (!rlw_step_is_original(step) || step->type == RLW_STEP_WAIT || host->deliver(host->ctx, step)) return 0;
 if (step->type == RLW_STEP_KEY) job->held[step->code & 255] = step->down;
 else job->moved_mouse = 1;
 return 1;
}

/* Releases synthetic keys and returns the cursor; delivers nothing else. */
static void abort_job(RlwModernJob *job, const RlwModernHost *host) {
 job->state = RLW_JOB_ABORTED; job->queue_length = 0;
 for (int vk = 0; vk < 256; vk++) if (job->held[vk]) { RlwStep up = key_step((uint32_t)vk, 0); (void)send(job, &up, host); }
 if (job->moved_mouse) { int x = 400, y = 300; host->pointer(host->ctx, &x, &y); RlwStep m = mouse_step(0x200, x, y); (void)send(job, &m, host); }
 job->state = RLW_JOB_ABORTED;
}

void rlw_job_pump(RlwModernJob *job, const RlwModernHost *host) {
 if (job->state != RLW_JOB_RUNNING) return;
 uint64_t now = host->frames(host->ctx);
 if (!job->started_set) { job->started_set = 1; job->started = now; }
 if (now - job->started > RLW_JOB_FRAME_BUDGET) { abort_job(job, host); return; }
 while (job->state == RLW_JOB_RUNNING) {
  if (!job->queue_length) {
   if (!job->replan || job->target.kind == RLW_TARGET_NONE) { job->state = RLW_JOB_FINISHED; return; }
   const RlwSnapshot *s = host->snapshot(host->ctx);
   if (!s) { abort_job(job, host); return; }
   RlwStage st = rlw_planner_next(job->target, s, &job->progress);
   if (st.kind == RLW_STAGE_ABORT) { abort_job(job, host); return; }
   for (int i = 0; i < st.count; i++) job->queue[job->queue_length++] = st.steps[i];
   if (st.kind == RLW_STAGE_FINISH) {
    int x = 400, y = 300; host->pointer(host->ctx, &x, &y);
    job->queue[job->queue_length++] = mouse_step(0x200, x, y);
    job->replan = 0;
   }
   job->queue[job->queue_length++] = wait_step();
   continue;
  }
  RlwStep step = job->queue[0];
  if (step.type == RLW_STEP_WAIT) {
   uint64_t frame = host->frames(host->ctx);
   if (!job->waiting) { job->waiting = 1; job->wait_from = frame; }
   if (host->pending_input(host->ctx) != 0 || frame <= job->wait_from) return;
   job->waiting = 0; pop(job);
   continue;
  }
  if (!send(job, &step, host)) { abort_job(job, host); return; }
  pop(job);
 }
}
