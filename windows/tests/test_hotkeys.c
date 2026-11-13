/* Portable unit tests (no game data, no Win32) for the keyboard bindings, the
 * input router and the "Atalhos modernos" planner/job (rlw_hotkeys.c,
 * rlw_modern.c). The snapshot reads a synthetic memory map through the same
 * peek contract as rlw_driver_peek. */
#include "rlw_hotkeys.h"
#include "rlw_modern.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "%s:%d: CHECK(%s)\n", __FILE__, __LINE__, #c); failures++; } } while (0)

/* ---------- router sink log ---------- */
static struct { int kind; uint32_t a, b; } log_[64];
static int log_n;
static void sink_emit(void *ctx, uint32_t vk, int down) { (void)ctx; if (log_n < 64) { log_[log_n].kind = 1; log_[log_n].a = vk; log_[log_n].b = (uint32_t)down; log_n++; } }
static void sink_modern(void *ctx, int command, uint32_t digit) { (void)ctx; if (log_n < 64) { log_[log_n].kind = 2; log_[log_n].a = (uint32_t)command; log_[log_n].b = digit; log_n++; } }
static RlwRouterSink sink(void) { RlwRouterSink s = {NULL, sink_emit, sink_modern}; return s; }

static void test_bindings(void) {
 RlwBindings b; rlw_bindings_defaults(&b); RlwBindError e;
 CHECK(rlw_bindings_validate(&b, &e) == 0);
 CHECK(rlw_bindings_action(&b, 'X', 0) == RLW_ACT_X);
 CHECK(rlw_bindings_action(&b, 'X', RLW_MOD_SHIFT | RLW_MOD_CONTROL) == RLW_ACT_X); /* optional modifiers */
 CHECK(rlw_bindings_action(&b, 0x24, 0) == RLW_ACT_LAST_ALERT);
 CHECK(rlw_bindings_action(&b, 0x24, RLW_MOD_SHIFT) == -1); /* modern chords are exact */
 CHECK(rlw_bindings_action(&b, 0xbe, 0) == RLW_ACT_NEXT_IDLE_WORKER);
 CHECK(rlw_bindings_action(&b, 0xbc, 0) == RLW_ACT_NEXT_IDLE_MILITARY);
 CHECK(rlw_bindings_action(&b, 'H', 0) == RLW_ACT_MAIN_STRUCTURE);
 CHECK(rlw_bindings_action(&b, 'Q', 0) == -1);
 /* Every channel action emits one of the 23 accepted keys, each a different one. */
 for (int a = 0; a < RLW_FIRST_MODERN_ACTION; a++) {
  CHECK(rlw_accepted_vk(rlw_action_vk(a)));
  for (int o = 0; o < a; o++) CHECK(rlw_action_vk(a) != rlw_action_vk(o));
 }
 for (int a = RLW_FIRST_MODERN_ACTION; a < RLW_ACTION_COUNT; a++) CHECK(rlw_action_vk(a) == 0 && rlw_action_is_modern(a));
 CHECK(rlw_accepted_vk(16) && rlw_accepted_vk(17) && rlw_accepted_vk(18) && !rlw_accepted_vk('H'));
 /* Conflicts are reported (first, second) in action order; b is unchanged. */
 RlwChord home = {0x24, 0, 0};
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_X, &home, 1, &e) == RLW_BIND_CONFLICT && e.first == RLW_ACT_X && e.second == RLW_ACT_LAST_ALERT);
 CHECK(rlw_bindings_action(&b, 'X', 0) == RLW_ACT_X);
 /* A Shift variant of an exact modern chord is free. */
 RlwChord shift_home[2] = {{'X', 0, 1}, {0x24, RLW_MOD_SHIFT, 0}};
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_X, shift_home, 2, &e) == 0);
 CHECK(rlw_bindings_action(&b, 0x24, RLW_MOD_SHIFT) == RLW_ACT_X && rlw_bindings_action(&b, 0x24, 0) == RLW_ACT_LAST_ALERT);
 /* An optional chord overlaps every modifier variant of the same key. */
 RlwChord shift_x = {'X', RLW_MOD_SHIFT, 0};
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_ENTER, &shift_x, 1, &e) == RLW_BIND_CONFLICT && e.first == RLW_ACT_ENTER && e.second == RLW_ACT_X);
 /* The same chord twice in one action. */
 RlwChord twice[2] = {{'Z', 0, 0}, {'Z', 0, 0}};
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_SPACE, twice, 2, &e) == RLW_BIND_CONFLICT && e.first == e.second);
 char msg[256]; rlw_bind_error_message(&e, msg, sizeof(msg)); CHECK(strstr(msg, "já está na lista") != NULL);
 /* Host keys and modifier keys are refused. */
 RlwChord bad[] = {{'P', 0, 0}, {0x13, 0, 0}, {0x6b, 0, 0}, {0xbd, RLW_MOD_CONTROL, 0}, {0x74, 0, 0}, {0x78, 0, 0}, {0x7a, 0, 0}, {0x73, RLW_MOD_ALT, 0}};
 for (unsigned i = 0; i < sizeof(bad) / sizeof(bad[0]); i++) CHECK(rlw_bindings_rebind(&b, RLW_ACT_SPACE, &bad[i], 1, &e) == RLW_BIND_RESERVED);
 RlwChord mods[] = {{0x10, 0, 0}, {0x11, 0, 0}, {0x12, 0, 0}, {0xa0, 0, 0}, {0x5b, 0, 0}, {0x14, 0, 0}, {0, 0, 0}};
 for (unsigned i = 0; i < sizeof(mods) / sizeof(mods[0]); i++) CHECK(rlw_bindings_rebind(&b, RLW_ACT_SPACE, &mods[i], 1, &e) == RLW_BIND_INVALID_KEY);
 RlwChord f4 = {0x73, 0, 0}, wide = {'Z', 8, 0};
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_SPACE, &f4, 1, &e) == 0);
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_SPACE, &wide, 1, &e) == RLW_BIND_INVALID_MODIFIERS);
 /* An empty list unbinds. */
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_SPACE, NULL, 0, &e) == 0 && rlw_bindings_action(&b, 0x73, 0) == -1);
 RlwBindings d; rlw_bindings_defaults(&d); CHECK(!rlw_bindings_equal(&b, &d)); CHECK(rlw_bindings_equal(&d, &d));
 char label[32]; rlw_vk_label(0xbe, label, sizeof(label)); CHECK(!strcmp(label, "."));
 rlw_vk_label('H', label, sizeof(label)); CHECK(!strcmp(label, "H"));
 rlw_vk_label(0x24, label, sizeof(label)); CHECK(!strcmp(label, "Home"));
}

static void test_router(void) {
 RlwRouter r; rlw_router_init(&r, NULL, sink());
 /* Off: digits pass, a double tap is two taps, modern keys are not handled. */
 log_n = 0;
 CHECK(rlw_router_key_down(&r, '1', 0, 0, 0.0) == 1 && rlw_router_key_up(&r, '1') == 1);
 CHECK(rlw_router_key_down(&r, '1', 0, 0, 0.1) == 1 && rlw_router_key_up(&r, '1') == 1);
 CHECK(log_n == 4 && log_[0].kind == 1 && log_[0].a == 49 && log_[0].b == 1 && log_[1].b == 0 && log_[2].b == 1 && log_[3].b == 0);
 log_n = 0;
 CHECK(rlw_router_key_down(&r, 0x24, 0, 0, 0.2) == 0 && rlw_router_key_up(&r, 0x24) == 0 && log_n == 0);
 CHECK(rlw_router_key_down(&r, 'Q', 0, 0, 0.2) == 0 && rlw_router_key_up(&r, 'Q') == 0 && log_n == 0);
 /* Repeats never emit; an unknown up emits nothing. */
 CHECK(rlw_router_key_down(&r, 'X', RLW_MOD_SHIFT, 0, 0.3) == 1 && rlw_router_key_down(&r, 'X', 0, 1, 0.3) == 1);
 CHECK(rlw_router_key_up(&r, 'X') == 1 && rlw_router_key_up(&r, 'X') == 0);
 CHECK(log_n == 2 && log_[0].a == 88 && log_[0].b == 1 && log_[1].a == 88 && log_[1].b == 0);
 CHECK(rlw_router_key_down(&r, 'X', 0, 1, 0.4) == 0 && log_n == 2); /* repeat after a reset/focus loss */

 /* On: second tap of the same digit within 0.35 s centres on the group. */
 r.modern_enabled = 1; log_n = 0;
 CHECK(rlw_router_key_down(&r, '3', 0, 0, 1.0) == 1); CHECK(rlw_router_key_up(&r, '3') == 1);
 CHECK(rlw_router_key_down(&r, '3', 0, 0, 1.3) == 1); CHECK(rlw_router_key_up(&r, '3') == 1);
 CHECK(log_n == 3 && log_[2].kind == 2 && log_[2].a == RLW_MODERN_CENTER_GROUP && log_[2].b == 51);
 /* A third tap starts over; a late tap is an ordinary tap. */
 log_n = 0;
 rlw_router_key_down(&r, '3', 0, 0, 1.4); rlw_router_key_up(&r, '3');
 rlw_router_key_down(&r, '3', 0, 0, 2.0); rlw_router_key_up(&r, '3');
 CHECK(log_n == 4 && log_[0].kind == 1 && log_[2].kind == 1);
 /* Ctrl+N (save group) never counts as a tap; another key in between resets. */
 log_n = 0;
 rlw_router_key_down(&r, '4', RLW_MOD_CONTROL, 0, 3.0); rlw_router_key_up(&r, '4');
 rlw_router_key_down(&r, '4', RLW_MOD_CONTROL, 0, 3.1); rlw_router_key_up(&r, '4');
 rlw_router_key_down(&r, '5', 0, 0, 3.2); rlw_router_key_up(&r, '5');
 rlw_router_key_down(&r, 'X', 0, 0, 3.3); rlw_router_key_up(&r, 'X');
 rlw_router_key_down(&r, '5', 0, 0, 3.4); rlw_router_key_up(&r, '5');
 for (int i = 0; i < log_n; i++) CHECK(log_[i].kind == 1);
 CHECK(log_n == 10);
 /* The four modern keys. */
 log_n = 0;
 uint32_t keys[4] = {0x24, 0xbe, 0xbc, 'H'}; int commands[4] = {RLW_MODERN_LAST_ALERT, RLW_MODERN_NEXT_IDLE_WORKER, RLW_MODERN_NEXT_IDLE_MILITARY, RLW_MODERN_MAIN_STRUCTURE};
 for (int i = 0; i < 4; i++) { CHECK(rlw_router_key_down(&r, keys[i], 0, 0, 5.0) == 1); CHECK(rlw_router_key_down(&r, keys[i], 0, 1, 5.0) == 1); CHECK(rlw_router_key_up(&r, keys[i]) == 1); }
 CHECK(log_n == 4); for (int i = 0; i < 4 && i < log_n; i++) CHECK(log_[i].kind == 2 && log_[i].a == (uint32_t)commands[i]);
 /* Shift+Home is not the modern chord. */
 log_n = 0; CHECK(rlw_router_key_down(&r, 0x24, RLW_MOD_SHIFT, 0, 6.0) == 0 && log_n == 0); rlw_router_key_up(&r, 0x24);

 /* Aliases: one down per action, the up when the last alias is released. */
 RlwBindings b; rlw_bindings_defaults(&b); RlwChord x2[2] = {{'X', 0, 1}, {'Z', 0, 0}};
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_X, x2, 2, NULL) == 0);
 rlw_router_install(&r, &b); log_n = 0;
 rlw_router_key_down(&r, 'X', 0, 0, 7.0); rlw_router_key_down(&r, 'Z', 0, 0, 7.0); rlw_router_key_up(&r, 'X');
 CHECK(log_n == 1 && log_[0].a == 88 && log_[0].b == 1);
 rlw_router_key_up(&r, 'Z'); CHECK(log_n == 2 && log_[1].a == 88 && log_[1].b == 0);
 /* The release follows the action recorded at the press, whatever the modifiers. */
 log_n = 0; rlw_router_key_down(&r, 'Z', 0, 0, 8.0); CHECK(rlw_router_key_up(&r, 'Z') == 1 && log_n == 2);
 /* Installing a different set forgets presses (the host released the keys). */
 rlw_router_key_down(&r, 'X', 0, 0, 9.0); RlwBindings d; rlw_bindings_defaults(&d); rlw_router_install(&r, &d);
 log_n = 0; CHECK(rlw_router_key_up(&r, 'X') == 0 && log_n == 0);
 CHECK(rlw_router_key_down(&r, 'Z', 0, 0, 9.5) == 0);
}

/* ---------- synthetic memory for the snapshot ---------- */
typedef struct { uint32_t base, size; uint8_t *bytes; } Region;
static Region regions[32]; static int region_n;
static uint8_t *region(uint32_t base, uint32_t size) {
 Region *r = &regions[region_n++]; r->base = base; r->size = size; r->bytes = calloc(size, 1); return r->bytes;
}
static void put16(uint8_t *p, int v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }
static void put32(uint8_t *p, uint32_t v) { for (int i = 0; i < 4; i++) p[i] = (uint8_t)(v >> (8 * i)); }
static int peeks;
static int fake_peek(void *ctx, const uint32_t *a, const uint32_t *s, uint32_t n, uint8_t *out) {
 (void)ctx; peeks++; size_t w = 0;
 for (uint32_t i = 0; i < n; i++) {
  int found = 0; if (!s[i]) return 1;
  for (int k = 0; k < region_n && !found; k++)
   if (a[i] >= regions[k].base && (uint64_t)a[i] + s[i] <= (uint64_t)regions[k].base + regions[k].size) { memcpy(out + w, regions[k].bytes + (a[i] - regions[k].base), s[i]); found = 1; }
  if (!found) return 3;
  w += s[i];
 }
 return 0;
}
static uint8_t *units_mem, *selection_mem, *zone_range_mem;
static void build_world(void) {
 region(0x4e2f78, 1)[0] = 0x12;
 region(0x508041, 1)[0] = 0;
 uint8_t *view = region(0x508720, 8); put16(view, 10); put16(view + 2, 20); put16(view + 4, 25); put16(view + 6, 17);
 selection_mem = region(0x508728, 4 * 512);
 uint8_t *map = region(0x497168, 4); put16(map, 128); put16(map + 2, 128);
 region(0x508f90, 1)[0] = 0;
 region(0x497285, 1)[0] = 1;
 uint8_t *mm = region(0x507d68, 4); put16(mm, 650); put16(mm + 2, 450);
 uint8_t *mms = region(0x496fe0, 4); put16(mms, 128); put16(mms + 2, 128);
 uint8_t *zr = zone_range_mem = region(0x50a9b4, 4); put16(zr, 10); put16(zr + 2, 11);
 uint8_t *zones = region(0x5099b4 + 16 * 10, 32);
 put32(zones, 650); put32(zones + 4, 450); put32(zones + 8, 777); put32(zones + 12, 577);
 put32(zones + 16, 0); put32(zones + 20, 544); put32(zones + 24, 640); put32(zones + 28, 599);
 put32(region(0x4e19a0, 4), 0x600000); put32(region(0x4e0f38, 4), 0x700000);
 uint8_t *sel = region(0x495be0, 8); put16(sel, 0xffff);
 units_mem = region(0x600000, 800 * 276);
 struct { int i, owner, kind, state, x, y, on; } u[] = {{3, 0, 5, 7, 15, 25, 1}, {7, 0, 6, 7, 100, 100, 1}, {9, 1, 5, 7, 16, 25, 1},
                                                         {12, 0, 0, 7, 17, 26, 1}, {13, 0, 5, 3, 18, 26, 1}, {14, 0, 5, 7, 19, 26, 0}, {15, 0, 5, 4, 19, 27, 1}};
 for (unsigned k = 0; k < sizeof(u) / sizeof(u[0]); k++) {
  uint8_t *p = units_mem + 276 * u[k].i; p[0] = (uint8_t)u[k].owner; p[1] = (uint8_t)u[k].kind; p[2] = (uint8_t)u[k].state;
  put16(p + 8, (u[k].y << 8) | u[k].x); p[0x72] = (uint8_t)u[k].on;
 }
 uint8_t *structures = region(0x49a0c8, 1000 * 276), *s4 = structures + 276 * 4;
 put16(s4 + 2, (22 << 8) | 12); s4[4] = 0; s4[5] = 1; s4[8] = 2;
 uint8_t *s5 = structures + 276 * 5; put16(s5 + 2, (40 << 8) | 40); s5[4] = 1; s5[5] = 1; s5[8] = 2; /* not ours */
 uint8_t *fp = region(0x495d8c + 104, 4); put16(fp, 2); put16(fp + 2, 2);
 uint8_t *grid = region(0x700000, 2 * 256 * 128);
 for (int r = 0; r < 2; r++) for (int c = 0; c < 2; c++) put16(grid + 2 * (((22 + r) << 8) | (12 + c)), 0x8000 | (4 << 5));
}

static void test_snapshot_and_planner(void) {
 build_world();
 static RlwSnapshot s; CHECK(rlw_snapshot_read(fake_peek, NULL, &s) == 0);
 CHECK(s.in_mission && s.local_player == 0 && s.view_x == 10 && s.view_y == 20 && s.view_w == 25 && s.view_h == 17);
 CHECK(s.zone_count == 2 && s.zones[0].id == 10 && s.zones[1].id == 11 && s.zones[1].y0 == 544);
 CHECK(s.unit_count == 6); /* states 0 and 4 are skipped */
 CHECK(s.structure_count == 1 && s.structures[0].index == 4 && s.structures[0].cell_count == 4);
 CHECK(s.selected_units == 0 && s.selected_structure == 0xffff);
 CHECK(rlw_snapshot_zone(&s, 700, 500) == 10 && rlw_snapshot_zone(&s, 100, 560) == 11 && rlw_snapshot_zone(&s, 100, 100) == 0);

 RlwPlanner p; rlw_planner_init(&p); RlwTarget t;
 /* Idle own workers on the map, cycling by index: 3, 7, 3. */
 CHECK(rlw_planner_choose(&p, RLW_MODERN_NEXT_IDLE_WORKER, &s, &t) && t.kind == RLW_TARGET_UNIT && t.index == 3);
 CHECK(rlw_planner_choose(&p, RLW_MODERN_NEXT_IDLE_WORKER, &s, &t) && t.index == 7);
 CHECK(rlw_planner_choose(&p, RLW_MODERN_NEXT_IDLE_WORKER, &s, &t) && t.index == 3);
 CHECK(rlw_planner_choose(&p, RLW_MODERN_NEXT_IDLE_MILITARY, &s, &t) && t.index == 12);
 CHECK(rlw_planner_choose(&p, RLW_MODERN_MAIN_STRUCTURE, &s, &t) && t.kind == RLW_TARGET_STRUCTURE && t.index == 4);
 CHECK(!rlw_planner_choose(&p, RLW_MODERN_LAST_ALERT, &s, &t));

 /* In view: one click on the unit's cell centre. */
 RlwModernProgress g = {0, 0}; RlwTarget u3 = {RLW_TARGET_UNIT, 3};
 RlwStage st = rlw_planner_next(u3, &s, &g);
 CHECK(st.kind == RLW_STAGE_FINISH && st.count == 2 && st.steps[0].code == 0x201 && st.steps[0].x == 176 && st.steps[0].y == 176 && st.steps[1].code == 0x202);
 /* Out of view: the minimap press that recentres on it (want 88,92), once. */
 RlwTarget u7 = {RLW_TARGET_UNIT, 7}; g = (RlwModernProgress){0, 0};
 st = rlw_planner_next(u7, &s, &g);
 CHECK(st.kind == RLW_STAGE_DELIVER && st.steps[0].code == 0x201 && st.steps[0].x == 750 && st.steps[0].y == 550 && g.recentered);
 CHECK(rlw_planner_next(u7, &s, &g).kind == RLW_STAGE_ABORT);
 int ox, oy; rlw_snapshot_minimap_origin(&s, 100, 100, &ox, &oy); CHECK(ox == 88 && oy == 92);
 rlw_snapshot_minimap_origin(&s, 2, 127, &ox, &oy); CHECK(ox == 0 && oy == 111);
 /* A selection is cleared first with a right press on the map, once. */
 put16(selection_mem, 1); CHECK(rlw_snapshot_read(fake_peek, NULL, &s) == 0 && s.selected_units == 1);
 g = (RlwModernProgress){0, 0}; st = rlw_planner_next(u3, &s, &g);
 CHECK(st.kind == RLW_STAGE_DELIVER && st.count == 1 && st.steps[0].code == 0x204 && st.steps[0].x == 400 && st.steps[0].y == 272);
 CHECK(rlw_planner_next(u3, &s, &g).kind == RLW_STAGE_ABORT);
 put16(selection_mem, 0); CHECK(rlw_snapshot_read(fake_peek, NULL, &s) == 0);
 /* The sanctuary: its first visible solid cell. */
 g = (RlwModernProgress){0, 0}; RlwTarget s4 = {RLW_TARGET_STRUCTURE, 4}; st = rlw_planner_next(s4, &s, &g);
 CHECK(st.kind == RLW_STAGE_FINISH && st.steps[0].x == 80 && st.steps[0].y == 80);
 /* Gone or not ours: abort. */
 RlwTarget enemy = {RLW_TARGET_UNIT, 9}, gone = {RLW_TARGET_UNIT, 300};
 g = (RlwModernProgress){0, 0}; CHECK(rlw_planner_next(enemy, &s, &g).kind == RLW_STAGE_ABORT && rlw_planner_next(gone, &s, &g).kind == RLW_STAGE_ABORT);
 /* Outside a mission the read succeeds with in_mission = 0 and plans nothing. */
 regions[0].bytes[0] = 0x10; CHECK(rlw_snapshot_read(fake_peek, NULL, &s) == 0 && !s.in_mission);
 g = (RlwModernProgress){0, 0}; CHECK(rlw_planner_next(u3, &s, &g).kind == RLW_STAGE_ABORT);
 regions[0].bytes[0] = 0x12;
 /* A failed peek is a failed read. */
 put16(zone_range_mem + 2, 70); CHECK(rlw_snapshot_read(fake_peek, NULL, &s) != 0); put16(zone_range_mem + 2, 12); CHECK(rlw_snapshot_read(fake_peek, NULL, &s) != 0); put16(zone_range_mem + 2, 11);
}

/* ---------- job over a fake host ---------- */
static struct { uint64_t frames; uint32_t pending; int refuse; RlwStep sent[32]; int sent_n; RlwSnapshot snap; int snap_ok; } H;
static int h_deliver(void *c, const RlwStep *s) { (void)c; if (H.refuse) return 1; if (H.sent_n < 32) H.sent[H.sent_n++] = *s; H.pending++; return 0; }
static uint32_t h_pending(void *c) { (void)c; return H.pending; }
static uint64_t h_frames(void *c) { (void)c; return H.frames; }
static const RlwSnapshot *h_snapshot(void *c) { (void)c; return H.snap_ok ? &H.snap : NULL; }
static void h_pointer(void *c, int *x, int *y) { (void)c; *x = 321; *y = 123; }
static void frame(void) { H.frames++; H.pending = 0; }

static void test_job(void) {
 RlwModernHost host = {NULL, h_deliver, h_pending, h_frames, h_snapshot, h_pointer};
 RlwModernJob job; RlwStep steps[5];
 /* Centre group 2: Shift held until a frame consumed the digit. */
 memset(&H, 0, sizeof(H)); H.frames = 200;
 CHECK(rlw_modern_center_group(50, steps) == 5); CHECK(rlw_modern_center_group(48, steps) == 0);
 rlw_modern_center_group(50, steps); rlw_job_steps(&job, steps, 5); CHECK(!rlw_job_uses_mouse(&job));
 rlw_job_pump(&job, &host);
 CHECK(H.sent_n == 3 && H.sent[0].code == 16 && H.sent[0].down && H.sent[1].code == 50 && H.sent[1].down && H.sent[2].code == 50 && !H.sent[2].down);
 rlw_job_pump(&job, &host); CHECK(H.sent_n == 3 && job.state == RLW_JOB_RUNNING); /* same frame */
 frame(); rlw_job_pump(&job, &host);
 CHECK(H.sent_n == 4 && H.sent[3].code == 16 && !H.sent[3].down && job.state == RLW_JOB_FINISHED);
 /* Last alert: X down, a frame, X up. */
 memset(&H, 0, sizeof(H)); RlwStep la[3]; rlw_modern_last_alert(la); rlw_job_steps(&job, la, 3);
 rlw_job_pump(&job, &host); frame(); rlw_job_pump(&job, &host);
 CHECK(H.sent_n == 2 && H.sent[0].code == 88 && H.sent[0].down && !H.sent[1].down && job.state == RLW_JOB_FINISHED);
 /* Mouse command: click, pointer back, wait, done. */
 memset(&H, 0, sizeof(H)); H.snap_ok = 1; rlw_snapshot_read(fake_peek, NULL, &H.snap);
 RlwTarget u3 = {RLW_TARGET_UNIT, 3}; rlw_job_target(&job, u3); CHECK(rlw_job_uses_mouse(&job));
 rlw_job_pump(&job, &host);
 CHECK(H.sent_n == 3 && H.sent[0].code == 0x201 && H.sent[1].code == 0x202 && H.sent[2].code == 0x200 && H.sent[2].x == 321 && H.sent[2].y == 123);
 frame(); rlw_job_pump(&job, &host); CHECK(job.state == RLW_JOB_FINISHED && H.sent_n == 3);
 /* Recentre (verified on a new snapshot), then the click. */
 memset(&H, 0, sizeof(H)); H.snap_ok = 1; rlw_snapshot_read(fake_peek, NULL, &H.snap);
 RlwTarget u7 = {RLW_TARGET_UNIT, 7}; rlw_job_target(&job, u7); rlw_job_pump(&job, &host);
 CHECK(H.sent_n == 2 && H.sent[0].x == 750 && H.sent[0].y == 550 && job.state == RLW_JOB_RUNNING);
 H.snap.view_x = 88; H.snap.view_y = 92; frame(); rlw_job_pump(&job, &host);
 CHECK(H.sent_n == 5 && H.sent[2].code == 0x201 && H.sent[2].x == (100 - 88) * 32 + 16 && H.sent[2].y == (100 - 92) * 32 + 16);
 frame(); rlw_job_pump(&job, &host); CHECK(job.state == RLW_JOB_FINISHED);
 /* A recentre that did not move the view aborts and returns the cursor. */
 memset(&H, 0, sizeof(H)); H.snap_ok = 1; rlw_snapshot_read(fake_peek, NULL, &H.snap);
 rlw_job_target(&job, u7); rlw_job_pump(&job, &host); frame(); rlw_job_pump(&job, &host);
 CHECK(job.state == RLW_JOB_ABORTED && H.sent_n == 3 && H.sent[2].code == 0x200 && H.sent[2].x == 321);
 /* Over budget: held synthetic keys are released. */
 memset(&H, 0, sizeof(H)); H.frames = 1000; rlw_modern_center_group(49, steps); rlw_job_steps(&job, steps, 5);
 rlw_job_pump(&job, &host); H.frames += RLW_JOB_FRAME_BUDGET + 1; H.pending = 5; rlw_job_pump(&job, &host);
 CHECK(job.state == RLW_JOB_ABORTED && H.sent_n == 4 && H.sent[3].code == 16 && !H.sent[3].down);
 /* A refused delivery aborts at once; no snapshot aborts. */
 memset(&H, 0, sizeof(H)); H.refuse = 1; rlw_job_steps(&job, la, 3); rlw_job_pump(&job, &host); CHECK(job.state == RLW_JOB_ABORTED);
 memset(&H, 0, sizeof(H)); rlw_job_target(&job, u3); rlw_job_pump(&job, &host); CHECK(job.state == RLW_JOB_ABORTED && H.sent_n == 0);
 /* Only original input is delivered. */
 RlwStep bad = {RLW_STEP_KEY, 1, 'H', 0, 0}, off = {RLW_STEP_MOUSE, 0, 0x201, 800, 10};
 CHECK(!rlw_step_is_original(&bad) && !rlw_step_is_original(&off));
}

static int commit_calls, commit_fail; static RlwBindings committed;
static int commit(void *ctx, const RlwBindings *b) { (void)ctx; commit_calls++; if (commit_fail) return 1; committed = *b; return 0; }

static void test_file_and_editor(void) {
 RlwBindings d, b, r; RlwBindError e; char text[4096];
 rlw_bindings_defaults(&d);
 /* Round trip, defaults and an edited set. */
 rlw_bindings_format(&d, text, sizeof(text));
 CHECK(!strncmp(text, "versao=1\r\nenter=13/0/1\r\n", 24));
 CHECK(strstr(text, "lastAlert=36/0/0\r\n") && strstr(text, "mainStructure=72/0/0\r\n"));
 CHECK(rlw_bindings_parse(text, strlen(text), &r, &e) == 0 && rlw_bindings_equal(&r, &d));
 b = d; RlwChord x2[2] = {{'X', 0, 1}, {'Z', RLW_MOD_CONTROL | RLW_MOD_ALT, 0}};
 CHECK(rlw_bindings_rebind(&b, RLW_ACT_X, x2, 2, NULL) == 0 && rlw_bindings_rebind(&b, RLW_ACT_TAB, NULL, 0, NULL) == 0);
 rlw_bindings_format(&b, text, sizeof(text));
 CHECK(strstr(text, "\r\nx=88/0/1 90/6/0\r\n") && strstr(text, "\r\ntab=\r\n"));
 CHECK(rlw_bindings_parse(text, strlen(text), &r, &e) == 0 && rlw_bindings_equal(&r, &b));
 /* LF only, comments and blank lines are accepted. */
 const char *lf = "# atalhos\nversao=1\n\nx=90/0/0\n";
 CHECK(rlw_bindings_parse(lf, strlen(lf), &r, &e) == 0 && rlw_bindings_action(&r, 'Z', 0) == RLW_ACT_X && rlw_bindings_action(&r, 'X', 0) == -1);
 /* Missing actions take their default unless the key is taken: here H is X's, so mainStructure stays unbound. */
 const char *taken = "versao=1\r\nx=72/0/0\r\n";
 CHECK(rlw_bindings_parse(taken, strlen(taken), &r, &e) == 0 && r.count[RLW_ACT_MAIN_STRUCTURE] == 0 && r.count[RLW_ACT_LAST_ALERT] == 1);
 /* Refusals leave *out unchanged. */
 RlwBindings keep = r;
 const char *bad[] = {"x=88/0/1\r\n", "versao=2\r\n", "versao=1\r\nnada=1/0/0\r\n", "versao=1\r\nx=88/0/1\r\nx=88/0/1\r\n",
                      "versao=1\r\nx=88/0\r\n", "versao=1\r\nx=88/0/2\r\n", "versao=1\r\nx=300/0/0\r\n", "versao=1\r\nx=80/0/0\r\n",
                      "versao=1\r\nx=36/0/0\r\nlastAlert=36/0/0\r\n", "versao=1\r\nx=88/0/1x\r\n", ""};
 int codes[] = {RLW_BIND_INVALID_FILE, RLW_BIND_UNSUPPORTED_VERSION, RLW_BIND_INVALID_FILE, RLW_BIND_DUPLICATE_ACTION,
                RLW_BIND_INVALID_FILE, RLW_BIND_INVALID_FILE, RLW_BIND_INVALID_FILE, RLW_BIND_RESERVED, RLW_BIND_CONFLICT,
                RLW_BIND_INVALID_FILE, RLW_BIND_INVALID_FILE};
 for (unsigned i = 0; i < sizeof(codes) / sizeof(codes[0]); i++) {
  int c = rlw_bindings_parse(bad[i], strlen(bad[i]), &r, &e);
  if (c != codes[i]) fprintf(stderr, "parse case %u: %d\n", i, c);
  CHECK(c == codes[i] && rlw_bindings_equal(&r, &keep));
 }

 /* Editor: capture, refusal keeps the capture, apply commits once. */
 RlwEditor ed; rlw_editor_init(&ed, &d);
 CHECK(!rlw_editor_has_changes(&ed));
 CHECK(rlw_editor_record(&ed, 'Q', 0, 0, &e) == 0); /* no capture */
 CHECK(rlw_editor_begin_capture(&ed, RLW_ACT_X, 3, &e) == RLW_EDIT_INVALID_SELECTION);
 CHECK(rlw_editor_begin_capture(&ed, RLW_ACT_X, -1, &e) == 0 && ed.capturing);
 CHECK(rlw_editor_record(&ed, 'P', 0, 0, &e) < 0 && e.code == RLW_BIND_RESERVED && ed.capturing);
 CHECK(rlw_editor_record(&ed, 0x24, 0, 0, &e) < 0 && e.code == RLW_BIND_CONFLICT && e.first == RLW_ACT_X && e.second == RLW_ACT_LAST_ALERT);
 CHECK(rlw_editor_record(&ed, 'Q', RLW_MOD_SHIFT, 1, &e) == 0 && ed.capturing); /* repeats are ignored */
 CHECK(rlw_editor_record(&ed, 'Q', RLW_MOD_SHIFT, 0, &e) == 1 && !ed.capturing);
 CHECK(ed.draft.count[RLW_ACT_X] == 2 && ed.draft.chords[RLW_ACT_X][1].vk == 'Q' && ed.draft.chords[RLW_ACT_X][1].mods == RLW_MOD_SHIFT && !ed.draft.chords[RLW_ACT_X][1].optional);
 CHECK(rlw_editor_has_changes(&ed));
 /* Replace one alternative. */
 CHECK(rlw_editor_begin_capture(&ed, RLW_ACT_X, 0, &e) == 0 && rlw_editor_record(&ed, 'K', 0, 0, &e) == 1);
 CHECK(rlw_bindings_action(&ed.draft, 'K', 0) == RLW_ACT_X && rlw_bindings_action(&ed.draft, 'X', 0) == -1);
 /* Apply refuses during a capture. */
 CHECK(rlw_editor_begin_capture(&ed, RLW_ACT_SPACE, -1, &e) == 0);
 CHECK(rlw_editor_apply(&ed, commit, NULL, &e) == RLW_EDIT_FINISH_CAPTURE && commit_calls == 0);
 rlw_editor_cancel_capture(&ed);
 /* A failed save changes nothing. */
 commit_fail = 1; CHECK(rlw_editor_apply(&ed, commit, NULL, &e) == RLW_EDIT_WRITE_FAILED && commit_calls == 1 && rlw_editor_has_changes(&ed));
 char msg[256]; rlw_bind_error_message(&e, msg, sizeof(msg)); CHECK(strstr(msg, "não foram aplicadas") != NULL);
 commit_fail = 0; CHECK(rlw_editor_apply(&ed, commit, NULL, &e) == 0 && commit_calls == 2 && !rlw_editor_has_changes(&ed));
 CHECK(rlw_bindings_equal(&committed, &ed.applied));
 CHECK(rlw_editor_apply(&ed, commit, NULL, &e) == 0 && commit_calls == 2); /* nothing to apply */
 /* Remove, cancel restores the applied set; reset gives the defaults in the draft only. */
 CHECK(rlw_editor_remove_alias(&ed, RLW_ACT_X, 5, &e) == RLW_EDIT_INVALID_SELECTION);
 CHECK(rlw_editor_remove_alias(&ed, RLW_ACT_X, 0, &e) == 0 && ed.draft.count[RLW_ACT_X] == 1 && ed.draft.chords[RLW_ACT_X][0].vk == 'Q');
 rlw_editor_cancel(&ed); CHECK(!rlw_editor_has_changes(&ed) && ed.draft.count[RLW_ACT_X] == 2);
 CHECK(rlw_editor_reset_defaults(&ed, &e) == 0 && rlw_bindings_equal(&ed.draft, &d) && rlw_editor_has_changes(&ed));
}

int main(void) {
 test_bindings();
 test_file_and_editor();
 test_router();
 test_snapshot_and_planner();
 test_job();
 if (failures) { fprintf(stderr, "test_hotkeys: %d failure(s)\n", failures); return 1; }
 printf("test_hotkeys ok\n");
 return 0;
}
