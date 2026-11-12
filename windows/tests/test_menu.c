/* Portable unit tests (no game data, no engine) for the menu routing
 * (rlw_menu.c, against a simulated driver) and the FLC decoder (rlw_flc.c,
 * against a synthetic file). Run: windows/build.sh test. */
#include "rlw_flc.h"
#include "rlw_menu.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures = 0;
#define CHECK(cond) do { if (!(cond)) { fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); failures++; } } while (0)

/* ---- simulated driver ---- */
typedef struct {
 uint32_t mode; RlShellInput state; RlShellEvent next; RlShellEvent pending;
 RlShellInput inputs[16]; unsigned input_count;
 unsigned draws, shows, requests, loads, slots, prepares; uint32_t shown_mode;
 uint32_t credits_state;
} Fake;
static uint32_t f_mode(void *d) { return ((Fake *)d)->mode; }
static int f_state(void *d, RlShellInput *o) { Fake *f = d; if (!f->mode) return 1; *o = f->state; o->keys = 0; o->mouse_b0 = 0; return 0; }
static int f_input(void *d, const RlShellInput *in, RlShellEvent *out) {
 Fake *f = d; if (f->pending.kind) return 11;
 if (f->input_count < 16) f->inputs[f->input_count++] = *in;
 f->state = *in; *out = f->next; f->pending = f->next; memset(&f->next, 0, sizeof(f->next)); return 0;
}
static int f_draw(void *d, uint32_t a) { Fake *f = d; f->draws++; f->state.animation_84 = a; return f->pending.kind ? 1 : 0; }
static int f_show(void *d) { Fake *f = d; f->shows++; f->mode = f->pending.kind == RL_SHELL_CREDITS ? RL_SHELL_CREDITS_SCREEN : f->pending.mode; f->shown_mode = f->mode; memset(&f->pending, 0, sizeof(f->pending)); return 0; }
static int f_prepare(void *d) { Fake *f = d; f->prepares++; f->mode = 17; memset(&f->pending, 0, sizeof(f->pending)); return 0; }
static int f_request(void *d) { Fake *f = d; f->requests++; if (f->pending.kind == RL_SHELL_RESUME) f->mode = 0; else f->mode = f->pending.mode; memset(&f->pending, 0, sizeof(f->pending)); return 0; }
static int f_load(void *d) { Fake *f = d; f->loads++; f->mode = 0; memset(&f->pending, 0, sizeof(f->pending)); return 0; }
static int f_slots(void *d, const uint8_t *t, uint32_t n) { (void)t; ((Fake *)d)->slots++; return n == 330 ? 0 : 1; }
static int f_zone(void *d, uint32_t z, int32_t r[4]) { (void)d; r[0] = z == 7 ? 700 : 600; r[1] = 500; r[2] = r[0] + 20; r[3] = 540; return 0; }
/* Same policy as rlw_shell_click_clock (the driver needs the engine to link). */
static uint32_t f_clock(uint32_t mode, uint32_t clock, uint32_t press) {
 if (mode != RL_SHELL_BRIEFING && mode != RL_SHELL_OBJECTIVES) return clock;
 if (press) return clock % 10 ? clock + (10 - clock % 10) : clock;
 return clock % 10 ? clock : clock + 1;
}
static int f_credits_frame(void *d, uint32_t ms, RlShellCredits *o) {
 Fake *f = d; (void)ms; memset(o, 0, sizeof(*o)); o->state = f->credits_state;
 if (o->state == RL_SHELL_CREDITS_ENDED) f->mode = RL_SHELL_MAIN;
 return 0;
}
static int f_credits_present(void *d, const RlShellCredits *fr, const uint8_t *p, const uint8_t *c) { (void)d; (void)fr; (void)p; (void)c; return 0; }
static void f_error(void *d, char *o, size_t n) { (void)d; snprintf(o, n, "fake"); }
static const RlwMenuOps OPS = {f_mode, f_state, f_input, f_draw, f_show, f_prepare, f_request, f_load, f_slots, f_zone, f_clock,
 f_credits_frame, f_credits_present, f_error, NULL, NULL, NULL, NULL};

static void fresh(Fake *f, RlwMenu *m, uint32_t mode) {
 memset(f, 0, sizeof(*f)); f->mode = mode; f->state.version = 1; f->state.clock_7c = 13; f->state.animation_84 = 40;
 f->state.mouse_x = 400; f->state.mouse_y = 300;
 rlw_menu_init(m, &OPS, f, "nao-existe/CREDITS.FLC");
}

static void test_menu(void) {
 Fake f; RlwMenu m;
 /* Keys act on release; repeats and stray releases are ignored. */
 fresh(&f, &m, RL_SHELL_MAIN);
 CHECK(rlw_menu_key(&m, 13, 1, 0) == RLW_MENU_IDLE && f.input_count == 0);
 CHECK(rlw_menu_key(&m, 13, 1, 0) == RLW_MENU_IDLE && f.input_count == 0);
 CHECK(rlw_menu_key(&m, 13, 0, 0) == RLW_MENU_REPAINT && f.input_count == 1);
 CHECK(f.inputs[0].keys == RL_SHELL_ENTER && f.inputs[0].mouse_ae == 0 && f.draws == 1);
 CHECK(rlw_menu_key(&m, 13, 0, 0) == RLW_MENU_IDLE && f.input_count == 1);
 CHECK(rlw_menu_key(&m, 65, 1, 0) == RLW_MENU_IDLE && rlw_menu_key(&m, 65, 0, 0) == RLW_MENU_IDLE && f.input_count == 1);
 rlw_menu_key(&m, 27, 1, 0); rlw_menu_key(&m, 27, 0, 0); CHECK(f.inputs[1].keys == RL_SHELL_ESCAPE);
 /* Moves are coalesced: recorded, then applied by flush or by the next tick. */
 fresh(&f, &m, RL_SHELL_MAIN);
 CHECK(rlw_menu_mouse(&m, 0x200, 10, 20, 0) == RLW_MENU_IDLE && rlw_menu_mouse(&m, 0x200, 11, 21, 0) == RLW_MENU_IDLE && f.input_count == 0);
 CHECK(rlw_menu_flush_move(&m, 0) == RLW_MENU_REPAINT && f.input_count == 1 && f.inputs[0].mouse_x == 11 && f.inputs[0].mouse_y == 21);
 CHECK(rlw_menu_flush_move(&m, 0) == RLW_MENU_IDLE);
 rlw_menu_mouse(&m, 0x200, 30, 40, 0);
 CHECK(rlw_menu_tick(&m, 0) == RLW_MENU_REPAINT && f.inputs[1].clock_7c == 14 && f.inputs[1].animation_84 == 41 && f.inputs[1].mouse_x == 30);
 /* Press/release: ae while held, af on release; briefing clock alignment. */
 fresh(&f, &m, RL_SHELL_BRIEFING);
 rlw_menu_mouse(&m, 0x201, 50, 60, 0);
 CHECK(f.inputs[0].mouse_ae == 1 && f.inputs[0].mouse_af == 0 && f.inputs[0].clock_7c == 20);
 rlw_menu_mouse(&m, 0x202, 50, 60, 0);
 CHECK(f.inputs[1].mouse_ae == 1 && f.inputs[1].mouse_af == 1 && f.inputs[1].clock_7c == 21);
 rlw_menu_tick(&m, 0); CHECK(f.inputs[2].mouse_ae == 0 && f.inputs[2].mouse_af == 0);
 /* Up/Down on the pages: a click at the arrow zone's centre, then the pointer returns. */
 fresh(&f, &m, RL_SHELL_OBJECTIVES);
 CHECK(rlw_menu_key(&m, 40, 1, 0) == RLW_MENU_REPAINT && f.input_count == 2);
 CHECK(f.inputs[0].mouse_x == 710 && f.inputs[0].mouse_y == 520 && f.inputs[0].mouse_ae == 1 && f.inputs[0].clock_7c == 20);
 CHECK(f.inputs[1].mouse_x == 400 && f.inputs[1].mouse_y == 300 && f.inputs[1].mouse_ae == 0);
 CHECK(rlw_menu_key(&m, 40, 0, 0) == RLW_MENU_IDLE && f.input_count == 2);
 /* Requests the driver completes, and the ones for the host. */
 fresh(&f, &m, RL_SHELL_MAIN); f.next.kind = RL_SHELL_SHOW; f.next.mode = RL_SHELL_SINGLE;
 rlw_menu_key(&m, 13, 1, 0);
 CHECK(rlw_menu_key(&m, 13, 0, 0) == RLW_MENU_REPAINT && f.slots == 1 && f.shows == 1 && f.mode == RL_SHELL_SINGLE);
 f.next.kind = RL_SHELL_RESET_CAMPAIGN; f.next.mode = RL_SHELL_BRIEFING;
 CHECK(rlw_menu_keys(&m, RL_SHELL_ENTER, 0) == RLW_MENU_REPAINT && f.requests == 1 && f.mode == RL_SHELL_BRIEFING);
 f.next.kind = RL_SHELL_START_MISSION;
 CHECK(rlw_menu_keys(&m, RL_SHELL_ENTER, 0) == RLW_MENU_GAME && f.loads == 1 && f.mode == 0);
 fresh(&f, &m, RL_SHELL_PAUSE); f.next.kind = RL_SHELL_RESUME;
 CHECK(rlw_menu_keys(&m, RL_SHELL_ESCAPE, 0) == RLW_MENU_GAME && f.mode == 0);
 fresh(&f, &m, 19); f.next.kind = RL_SHELL_MISSION_NEXT;
 CHECK(rlw_menu_keys(&m, RL_SHELL_ENTER, 0) == RLW_MENU_REPAINT && f.prepares == 1 && f.mode == 17);
 fresh(&f, &m, RL_SHELL_MAIN); f.next.kind = RL_SHELL_QUIT;
 CHECK(rlw_menu_keys(&m, RL_SHELL_ENTER, 0) == RLW_MENU_HOST && m.host_event.kind == RL_SHELL_QUIT);
 /* Credits with an absent CREDITS.FLC: the engine ends the screen by itself. */
 fresh(&f, &m, RL_SHELL_MAIN); f.next.kind = RL_SHELL_CREDITS;
 CHECK(rlw_menu_keys(&m, RL_SHELL_ENTER, 0) == RLW_MENU_REPAINT && f.mode == RL_SHELL_CREDITS_SCREEN && !m.credits_open);
 CHECK(rlw_menu_tick_ms(&m) == 10);
 f.credits_state = RL_SHELL_CREDITS_ENDED;
 CHECK(rlw_menu_tick(&m, 100) == RLW_MENU_REPAINT && f.mode == RL_SHELL_MAIN && rlw_menu_tick_ms(&m) == 50);
 /* A driver error is reported, not hidden. */
 fresh(&f, &m, RL_SHELL_MAIN); f.pending.kind = 99;
 CHECK(rlw_menu_keys(&m, RL_SHELL_ENTER, 0) == RLW_MENU_FAILED && !strcmp(m.error, "fake"));
 rlw_menu_close(&m);
}

/* ---- FLC ---- */
static void put16(uint8_t *p, uint32_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }
static void put32(uint8_t *p, uint32_t v) { put16(p, v & 0xffff); put16(p + 2, v >> 16); }

/* ---- simulated options screen (rising_options.h): a button at y >= 548;
 * open, it takes everything; Enter changes a value, Escape closes. ---- */
typedef struct { int open, armed; unsigned seen; uint32_t last_keys; } FakeOptions;
static FakeOptions fo;
static int f_options_input(void *d, const RlShellInput *in, uint32_t keys, RlOpcoesEvento *o) {
 (void)d; memset(o, 0, sizeof(*o)); o->item = UINT32_MAX; fo.seen++; fo.last_keys = keys;
 int on = in->mouse_y >= 548;
 if (!fo.open) {
  if (in->mouse_ae && in->mouse_af) { if (fo.armed) { fo.armed = 0; o->consumed = 1; if (on) { fo.open = 1; o->kind = RL_OPCOES_ABRIU; } } }
  else if (in->mouse_ae && on && !fo.armed) { fo.armed = 1; o->consumed = 1; }
 } else {
  o->consumed = 1;
  if (keys & RL_SHELL_ESCAPE) { fo.open = 0; o->kind = RL_OPCOES_FECHOU; }
  else if (keys & ((uint32_t)RL_SHELL_ENTER | (uint32_t)RL_OPCOES_DIREITA)) { o->kind = RL_OPCOES_MUDOU; o->item = RL_OPCOES_VELOCIDADE; o->values.velocidade = 2; }
 }
 o->open = (uint32_t)fo.open; return 0;
}
static uint32_t f_options_open(void *d) { (void)d; return (uint32_t)fo.open; }
static const RlwMenuOps OPTIONS_OPS = {f_mode, f_state, f_input, f_draw, f_show, f_prepare, f_request, f_load, f_slots, f_zone, f_clock,
 f_credits_frame, f_credits_present, f_error, f_options_input, f_options_open, NULL, NULL};

static void test_options(void) {
 Fake f; RlwMenu m; memset(&fo, 0, sizeof(fo));
 memset(&f, 0, sizeof(f)); f.mode = RL_SHELL_MAIN; f.state.version = 1; f.state.mouse_x = 400; f.state.mouse_y = 300;
 rlw_menu_init(&m, &OPTIONS_OPS, &f, NULL);
 /* Closed: keys and clicks away from the button reach the shell. */
 CHECK(rlw_menu_key(&m, 40, 1, 0) == RLW_MENU_IDLE && rlw_menu_key(&m, 40, 0, 0) == RLW_MENU_REPAINT && f.input_count == 1);
 /* Left/Right are not mapped while closed (the shell has no such keys). */
 CHECK(rlw_menu_key(&m, 39, 1, 0) == RLW_MENU_IDLE && rlw_menu_key(&m, 39, 0, 0) == RLW_MENU_IDLE && f.input_count == 1);
 /* A press and release on the button never reach the shell; the release opens. */
 CHECK(rlw_menu_mouse(&m, 0x201, 455, 560, 0) == RLW_MENU_REPAINT && f.input_count == 1);
 CHECK(rlw_menu_mouse(&m, 0x202, 455, 560, 0) == RLW_MENU_REPAINT && f.input_count == 1 && fo.open);
 /* Open: ticks, keys (Right included) all stay with the screen. */
 CHECK(rlw_menu_tick(&m, 0) == RLW_MENU_REPAINT && f.input_count == 1);
 CHECK(rlw_menu_key(&m, 39, 1, 0) == RLW_MENU_IDLE && rlw_menu_key(&m, 39, 0, 0) == RLW_MENU_OPTIONS);
 CHECK(fo.last_keys == RL_OPCOES_DIREITA && m.options_event.item == RL_OPCOES_VELOCIDADE && m.options_event.values.velocidade == 2);
 CHECK(rlw_menu_key(&m, 27, 1, 0) == RLW_MENU_IDLE && rlw_menu_key(&m, 27, 0, 0) == RLW_MENU_REPAINT && !fo.open);
 CHECK(f.input_count == 1 && f.draws == 1);
 /* Closed again: the shell gets the next tick. */
 CHECK(rlw_menu_tick(&m, 0) == RLW_MENU_REPAINT && f.input_count == 2);
}

static void test_flc(void) {
 /* 4x2, two frames: COLOR_256 + BRUN, then LC + SS2-free delta, then ring. */
 uint8_t f[512]; memset(f, 0, sizeof(f)); uint32_t o = 128;
 put16(f + 4, 0xAF12); put16(f + 6, 2); put16(f + 8, 4); put16(f + 10, 2); put16(f + 12, 8); put32(f + 16, 70); put32(f + 80, 128);
 /* prefix chunk 0xF100, skipped */
 put32(f + o, 16); put16(f + o + 4, 0xF100); o += 16;
 /* frame 1 */
 uint32_t frame = o; o += 16;
 uint32_t c1 = o; put16(f + o + 4, 4); o += 6;
 put16(f + o, 1); o += 2; f[o++] = 1; f[o++] = 2; f[o++] = 10; f[o++] = 20; f[o++] = 30; f[o++] = 40; f[o++] = 50; f[o++] = 60;
 put32(f + c1, o - c1);
 uint32_t c2 = o; put16(f + o + 4, 15); o += 6;
 f[o++] = 0; f[o++] = 4; f[o++] = 7;                        /* line 0: run 4 x 7 */
 f[o++] = 0; f[o++] = 0xFC; f[o++] = 1; f[o++] = 2; f[o++] = 3; f[o++] = 4; /* line 1: copy 4 */
 put32(f + c2, o - c2);
 put32(f + frame, o - frame); put16(f + frame + 4, 0xF1FA); put16(f + frame + 6, 2);
 /* frame 2: LC, line 1, one packet: skip 1, copy 2 */
 frame = o; o += 16;
 uint32_t c3 = o; put16(f + o + 4, 12); o += 6;
 put16(f + o, 1); put16(f + o + 2, 1); o += 4; f[o++] = 1; f[o++] = 1; f[o++] = 2; f[o++] = 9; f[o++] = 8;
 put32(f + c3, o - c3);
 put32(f + frame, o - frame); put16(f + frame + 4, 0xF1FA); put16(f + frame + 6, 1);
 uint8_t *data = malloc(o); memcpy(data, f, o);
 RlwFlc flc; CHECK(rlw_flc_init(&flc, data, o) == 0);
 CHECK(flc.frames == 2 && flc.width == 4 && flc.height == 2 && flc.delay_ms == 70.0);
 CHECK(rlw_flc_next(&flc) == 0 && flc.shown == 1 && flc.palette_changed);
 CHECK(flc.palette[3] == 10 && flc.palette[4] == 20 && flc.palette[8] == 60);
 static const uint8_t one[8] = {7, 7, 7, 7, 1, 2, 3, 4};
 CHECK(!memcmp(flc.pixels, one, 8));
 CHECK(rlw_flc_next(&flc) == 0 && flc.shown == 2 && !flc.palette_changed);
 static const uint8_t two[8] = {7, 7, 7, 7, 1, 9, 8, 4};
 CHECK(!memcmp(flc.pixels, two, 8));
 CHECK(rlw_flc_next(&flc) != 0 && strstr(flc.error, "beyond end"));
 rlw_flc_rewind(&flc); CHECK(rlw_flc_next(&flc) == 0 && !memcmp(flc.pixels, one, 8));
 rlw_flc_free(&flc);
 uint8_t *bad = calloc(1, 128); RlwFlc b; CHECK(rlw_flc_init(&b, bad, 128) != 0 && strstr(b.error, "magic")); rlw_flc_free(&b);
}

int main(void) {
 test_menu();
 test_options();
 test_flc();
 if (failures) { fprintf(stderr, "%d falha(s)\n", failures); return 1; }
 printf("test_menu ok: menu routing (keys, coalesced moves, pages, requests, credits, options screen) and FLC decoding\n");
 return 0;
}
