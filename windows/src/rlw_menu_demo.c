#include "rlw_menu_demo.h"
#include "rlw_demo.h"
#include "rlw_sha256.h"
#include <string.h>

typedef struct { const RlwDemoOps *ops; void *d; RlwMenu menu; FILE *out; unsigned step; int ended; } Walk;

static const char *result_name(int r) {
 static const char *names[] = {"idle", "repaint", "host", "game", "failed"};
 return r >= 0 && r <= 4 ? names[r] : "?";
}
static void line(Walk *w, const char *action, const char *result) {
 RlwSha256 c; uint8_t digest[32]; char hex[65];
 rlw_sha256_init(&c); rlw_sha256_update(&c, w->ops->pixels(w->d), 480000); rlw_sha256_update(&c, w->ops->palette(w->d), 768);
 rlw_sha256_final(&c, digest); rlw_sha256_hex(digest, hex);
 fprintf(w->out, "%u %s %s %u %u %llu %s\n", w->step++, action, result, w->ops->menu.mode(w->d),
         w->ops->menu.mode(w->d) ? 0u : w->ops->screen(w->d), (unsigned long long)w->ops->frames(w->d), hex);
}
static int end_walk(Walk *w, const char *why) {
 if (!w->ended) { fprintf(w->out, "end %s\n", why); fprintf(stderr, "menu demo ended: %s\n", why); }
 w->ended = 1; return 1;
}
/* One menu action; `want` is the outcome that keeps the walk going. */
static int act(Walk *w, const char *action, int r, int want) {
 if (r == RLW_MENU_HOST) { char why[96]; snprintf(why, sizeof(why), "host-request %u/%u at %s", w->menu.host_event.kind, w->menu.host_event.mode, action); line(w, action, "host"); return end_walk(w, why); }
 line(w, action, result_name(r));
 if (r == RLW_MENU_FAILED) { char why[600]; snprintf(why, sizeof(why), "%s: %s", action, w->menu.error); return end_walk(w, why); }
 if (want == RLW_MENU_GAME && r != RLW_MENU_GAME) return end_walk(w, action);
 if (want != RLW_MENU_GAME && r == RLW_MENU_GAME) return end_walk(w, action);
 return 0;
}
static int ticks(Walk *w, unsigned n) {
 for (unsigned i = 0; i < n; i++) if (act(w, "tick", rlw_menu_tick(&w->menu, 0), RLW_MENU_REPAINT)) return 1;
 return 0;
}
static int keys(Walk *w, const char *action, uint32_t k, int want) { return act(w, action, rlw_menu_keys(&w->menu, k, 0), want); }

/* Play `n` frames with the fixed synthetic input, opening the shell at 16/19
 * exactly where the macOS host does (right after the step that reached it). */
static int play(Walk *w, unsigned n, const RlwDemoEvent *extra, unsigned extra_count) {
 for (unsigned i = 0; i < n; i++) {
  uint64_t next = w->ops->frames(w->d) + 1;
  for (unsigned k = 0; k < RLW_DEMO_COUNT; k++) if (RLW_DEMO[k].frame == next) {
   const RlwDemoEvent *e = &RLW_DEMO[k];
   if (w->ops->event(w->d, e->kind, e->message, e->x, e->y, e->wparam, e->vk, e->down)) return end_walk(w, "event refused");
  }
  if (extra && i == n - 1) for (unsigned k = 0; k < extra_count; k++) {
   const RlwDemoEvent *e = &extra[k];
   if (w->ops->event(w->d, e->kind, e->message, e->x, e->y, e->wparam, e->vk, e->down)) return end_walk(w, "event refused");
  }
  if (w->ops->advance(w->d)) { char m[512] = {0}; w->ops->stop_message(w->d, m, sizeof(m)); line(w, "frame", "stopped"); return end_walk(w, m[0] ? m : "advance"); }
  uint32_t screen = w->ops->screen(w->d);
  if (screen == RL_SHELL_PAUSE || screen == 19) {
   int c = w->ops->begin(w->d);
   line(w, "frame", c ? "begin-failed" : "shell");
   if (c) return end_walk(w, "shell begin");
   return 0;
  }
  line(w, "frame", "ok");
 }
 return 0;
}

/* The pointer rests off the items: hover selects the item under it on every
 * input (as originally), so the keys alone choose, as in the macOS tests'
 * pause_choose (pointer at 4,4). */
static int pointer_off_items(Walk *w) {
 return act(w, "move", rlw_menu_mouse(&w->menu, 0x200, 4, 4, 0), RLW_MENU_IDLE) || act(w, "flush", rlw_menu_flush_move(&w->menu, 0), RLW_MENU_REPAINT);
}

/* Escape down through the event queue, then frames until the original pause opens. */
static int escape_to_pause(Walk *w) {
 static const RlwDemoEvent escape_down[] = {{0, 2, 0, 0, 0, 0, 27, 1}};
 for (unsigned i = 0; i < 5 && !w->ops->menu.mode(w->d); i++)
  if (play(w, 1, i ? NULL : escape_down, 1)) return 1;
 if (!w->ops->menu.mode(w->d)) return end_walk(w, "no pause after Escape");
 return 0;
}

int rlw_menu_demo_run(const RlwDemoOps *ops, void *driver, FILE *out) {
 Walk w; memset(&w, 0, sizeof(w)); w.ops = ops; w.d = driver; w.out = out;
 rlw_menu_init(&w.menu, &ops->menu, driver, NULL);
 static const RlwDemoEvent escape_up[] = {{0, 2, 0, 0, 0, 0, 27, 0}};
 int rc = 1;
 line(&w, "start", ops->menu.mode(driver) == RL_SHELL_MAIN ? "main" : "other");
 if (ops->menu.mode(driver) != RL_SHELL_MAIN) { end_walk(&w, "not in the main menu"); goto done; }
 /* The pointer rests off the items (hover selects, as originally). */
 if (act(&w, "move", rlw_menu_mouse(&w.menu, 0x200, 4, 4, 0), RLW_MENU_IDLE) || act(&w, "flush", rlw_menu_flush_move(&w.menu, 0), RLW_MENU_REPAINT)) goto done;
 if (ticks(&w, 10)) goto done;
 if (keys(&w, "enter-single", RL_SHELL_ENTER, RLW_MENU_REPAINT) || ticks(&w, 5)) goto done;          /* Um jogador -> single3 */
 if (keys(&w, "enter-new-game", RL_SHELL_ENTER, RLW_MENU_REPAINT) || ticks(&w, 10)) goto done;       /* Novo jogo -> briefing17 */
 if (act(&w, "page-next", rlw_menu_page(&w.menu, 1, 0), RLW_MENU_REPAINT) || ticks(&w, 10)) goto done;
 if (keys(&w, "enter-start", RL_SHELL_ENTER, RLW_MENU_GAME)) goto done;                              /* START_MISSION -> MONDE01 */
 line(&w, "stage", "loaded");
 if (play(&w, 150, NULL, 0) || escape_to_pause(&w)) goto done;                                       /* pause16 */
 if (pointer_off_items(&w) || ticks(&w, 5)) goto done;
 if (keys(&w, "down", RL_SHELL_DOWN, RLW_MENU_REPAINT) || keys(&w, "down", RL_SHELL_DOWN, RLW_MENU_REPAINT)) goto done;
 if (keys(&w, "enter-objectives", RL_SHELL_ENTER, RLW_MENU_REPAINT)) goto done;                      /* objectives23 */
 if (ops->menu.mode(driver) != RL_SHELL_OBJECTIVES) { end_walk(&w, "pause item 2 did not open the objectives"); goto done; }
 if (ticks(&w, 5)) goto done;
 if (act(&w, "page-next", rlw_menu_page(&w.menu, 1, 0), RLW_MENU_REPAINT) || ticks(&w, 5)) goto done;
 /* Objectives23 ignore Escape (CalculGame reads only Enter or a click on zones
  * 8/5 there): Enter returns to the pause (MenuBefore 23 -> 16), whose Escape
  * resumes the game (16 -> 18). */
 if (keys(&w, "enter-back", RL_SHELL_ENTER, RLW_MENU_REPAINT)) goto done;
 if (ops->menu.mode(driver) != RL_SHELL_PAUSE) { end_walk(&w, "Enter did not leave the objectives"); goto done; }
 if (ticks(&w, 5) || keys(&w, "escape", RL_SHELL_ESCAPE, RLW_MENU_GAME)) goto done;
 if (play(&w, 30, NULL, 0)) goto done;
 /* The Escape release reaches the resumed world (macOS test order). */
 if (ops->event(driver, 2, 0, 0, 0, 0, 27, 0)) { end_walk(&w, "event refused"); goto done; }
 if (play(&w, 30, NULL, 0) || escape_to_pause(&w)) goto done;
 /* Pause items from the top (macOS test): 0 load, 1 save, 2 objectives, 3 abandon,
  * 4 restart, 5 continue. Mode 12 here is the load list, not the briefing (17). */
 if (pointer_off_items(&w)) goto done;
 for (int i = 0; i < 4; i++) if (keys(&w, "down", RL_SHELL_DOWN, RLW_MENU_REPAINT)) goto done;
 if (keys(&w, "enter-restart", RL_SHELL_ENTER, RLW_MENU_REPAINT)) goto done;                         /* briefing17 of this stage */
 if (ops->menu.mode(driver) != RL_SHELL_BRIEFING) { end_walk(&w, "restart did not open the briefing"); goto done; }
 if (ticks(&w, 5)) goto done;
 if (keys(&w, "enter-start", RL_SHELL_ENTER, RLW_MENU_GAME)) goto done;
 line(&w, "stage", "restarted");
 if (play(&w, 60, escape_up, 1)) goto done;
 fprintf(out, "end complete\n");
 rc = 0;
done:
 rlw_menu_close(&w.menu);
 return rc;
}
