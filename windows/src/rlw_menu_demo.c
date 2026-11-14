#include "rlw_menu_demo.h"
#include "rlw_demo.h"
#include "rlw_sha256.h"
#include "rlw_fs.h"
#include "rlw_autosave.h"
#include <stdlib.h>
#include <string.h>

typedef struct {
 const RlwDemoOps *ops; void *d; RlwMenu menu; FILE *out; unsigned step; int ended;
 /* Save walk: the last line's picture hash and an optional per-frame record. */
 char last_hex[65]; char (*record)[65]; unsigned recorded;
} Walk;

static const char *result_name(int r) {
 static const char *names[] = {"idle", "repaint", "host", "game", "failed", "options"};
 return r >= 0 && r <= 5 ? names[r] : "?";
}
static void line(Walk *w, const char *action, const char *result) {
 RlwSha256 c; uint8_t digest[32]; char hex[65];
 rlw_sha256_init(&c); rlw_sha256_update(&c, w->ops->pixels(w->d), 480000); rlw_sha256_update(&c, w->ops->palette(w->d), 768);
 rlw_sha256_final(&c, digest); rlw_sha256_hex(digest, hex); memcpy(w->last_hex, hex, 65);
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
  if (w->record) memcpy(w->record[w->recorded++], w->last_hex, 65);
 }
 return 0;
}

/* The pointer rests off the items: hover selects the item under it on every
 * input (as originally), so the keys alone choose, as in the macOS tests'
 * pause_choose (pointer at 4,4). */
static int pointer_off_items(Walk *w) {
 return act(w, "move", rlw_menu_mouse(&w->menu, 0x200, 4, 4, 0), RLW_MENU_IDLE) || act(w, "flush", rlw_menu_flush_move(&w->menu, 0), RLW_MENU_REPAINT);
}

/* The port's "Opções" screen over the current main/pause menu: hover, press and
 * release on its button (open), a few ticks, Down (over the unavailable
 * "configurar teclas" to "caminhos melhores"), Right (caminhos on: an options
 * event the walk does not apply), Escape (closed). Every picture is hashed like
 * the other steps. Skipped without the extension. */
static int options_walk(Walk *w) {
 const RlwMenuOps *m = &w->ops->menu;
 if (!m->options_sync || !m->options_button || !m->options_input) return 0;
 RlOpcoesValores v; memset(&v, 0, sizeof(v));
 v.version = RL_OPCOES_VERSION; v.atalhos = 0; v.caminhos = 0; v.dificuldade = 1; v.velocidade = 1;
 v.disponivel = RL_OPCOES_TODOS & ~(1u << RL_OPCOES_TECLAS); v.idioma = 0;
 int c = m->options_sync(w->d, &v);
 if (c) { line(w, "options-sync", "failed"); return end_walk(w, "options sync"); }
 line(w, "options-sync", "ok");
 int32_t r[4]; if (m->options_button(w->d, r)) return end_walk(w, "options button");
 /* Aligned with the menu column (engine 645c2f17): centred on AffText's axis
  * x = 400, on the row below the items, overlapping no item zone (1..7). */
 int overlap = 0;
 for (uint32_t z = 1; z < 8; z++) {
  int32_t q[4]; if (m->zone(w->d, z, q)) return end_walk(w, "menu zone");
  fprintf(w->out, "%u zone %u %d %d %d %d\n", w->step++, z, q[0], q[1], q[2], q[3]);
  if (q[2] > q[0] && q[3] > q[1] && q[0] < r[2] && r[0] < q[2] && q[1] < r[3] && r[1] < q[3]) overlap = 1;
 }
 fprintf(w->out, "%u options-button %d %d %d %d %s\n", w->step++, r[0], r[1], r[2], r[3], (r[0] + r[2]) / 2 == 400 && !overlap ? "aligned" : "MISPLACED");
 if ((r[0] + r[2]) / 2 != 400 || overlap) return end_walk(w, "the options button is off the menu column or over an item");
 uint32_t x = (uint32_t)((r[0] + r[2]) / 2), y = (uint32_t)((r[1] + r[3]) / 2);
 if (act(w, "options-move", rlw_menu_mouse(&w->menu, 0x200, x, y, 0), RLW_MENU_IDLE) ||
     act(w, "options-flush", rlw_menu_flush_move(&w->menu, 0), RLW_MENU_REPAINT) ||
     act(w, "options-press", rlw_menu_mouse(&w->menu, 0x201, x, y, 0), RLW_MENU_REPAINT) ||
     act(w, "options-release", rlw_menu_mouse(&w->menu, 0x202, x, y, 0), RLW_MENU_REPAINT)) return 1;
 if (!m->options_open(w->d)) return end_walk(w, "options did not open");
 if (ticks(w, 4) || keys(w, "options-down", RL_SHELL_DOWN, RLW_MENU_REPAINT) ||
     keys(w, "options-right", RL_OPCOES_DIREITA, RLW_MENU_REPAINT) ||
     keys(w, "options-escape", RL_SHELL_ESCAPE, RLW_MENU_REPAINT)) return 1;
 if (m->options_open(w->d)) return end_walk(w, "options did not close");
 /* Jogo > Opções… opens the same screen without the button; Escape closes it. */
 if (m->options_show) {
  int shown = m->options_show(w->d); line(w, "options-show", shown ? "failed" : "ok");
  if (shown || !m->options_open(w->d)) return end_walk(w, "Jogo > Opções… did not open the screen");
  if (ticks(w, 2) || keys(w, "options-escape", RL_SHELL_ESCAPE, RLW_MENU_REPAINT)) return 1;
  if (m->options_open(w->d)) return end_walk(w, "options did not close");
 }
 /* The pointer leaves the button (its caption returns to the item font). */
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
 if (ticks(&w, 10) || options_walk(&w)) goto done;
 if (keys(&w, "enter-single", RL_SHELL_ENTER, RLW_MENU_REPAINT) || ticks(&w, 5)) goto done;          /* Um jogador -> single3 */
 if (keys(&w, "enter-new-game", RL_SHELL_ENTER, RLW_MENU_REPAINT) || ticks(&w, 10)) goto done;       /* Novo jogo -> briefing17 */
 if (act(&w, "page-next", rlw_menu_page(&w.menu, 1, 0), RLW_MENU_REPAINT) || ticks(&w, 10)) goto done;
 if (keys(&w, "enter-start", RL_SHELL_ENTER, RLW_MENU_GAME)) goto done;                              /* START_MISSION -> MONDE01 */
 line(&w, "stage", "loaded");
 if (play(&w, 150, NULL, 0) || escape_to_pause(&w)) goto done;                                       /* pause16 */
 if (pointer_off_items(&w) || ticks(&w, 5) || options_walk(&w)) goto done;
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

/* ---- Save walk (milestone 3) ---- */

static int file_hex(const char *path, char hex[65]) {
 uint32_t n = 0; uint8_t *b = rlw_read_file(path, &n, 160u * 1024u * 1024u);
 if (!b) { snprintf(hex, 65, "absent"); return 1; }
 uint8_t digest[32]; rlw_sha256(b, n, digest); free(b); rlw_sha256_hex(digest, hex); return 0;
}
/* "<step> <action> rc=<code> <sha256 slot> <sha256 companion>" (no paths). */
static void slot_line(Walk *w, const char *action, int rc, const char *path) {
 char rls[65], view[65] = "absent", companion[4300];
 if (!file_hex(path, rls)) { snprintf(companion, sizeof(companion), "%s.%s.view", path, rls); file_hex(companion, view); }
 fprintf(w->out, "%u %s rc=%d %s %s\n", w->step++, action, rc, rls, view);
}
typedef struct { unsigned rls, view, tmp; } FileCount;
static int count_file(const char *name, int is_dir, void *ctx) {
 FileCount *f = ctx; size_t n = strlen(name); if (is_dir) return 0;
 if (strstr(name, ".tmp.")) f->tmp++; else if (n > 4 && !strcmp(name + n - 4, ".rls")) f->rls++; else if (n > 5 && !strcmp(name + n - 5, ".view")) f->view++;
 return 0;
}
static void files_line(Walk *w, const char *directory) {
 FileCount f = {0, 0, 0}; rlw_list_dir(directory, count_file, &f);
 fprintf(w->out, "%u files rls=%u view=%u tmp=%u\n", w->step++, f.rls, f.view, f.tmp);
}
static int play_record(Walk *w, unsigned n, char (*record)[65]) {
 w->record = record; w->recorded = 0; int c = play(w, n, NULL, 0); w->record = NULL; return c;
}
/* Replays n frames and compares them with an earlier record of the same frames. */
static int play_compare(Walk *w, unsigned n, char (*expected)[65], const char *what) {
 static char now[64][65];
 if (n > 64 || play_record(w, n, now)) return 1;
 if (w->recorded != n) return end_walk(w, what);
 for (unsigned i = 0; i < n; i++) if (strcmp(now[i], expected[i])) {
  fprintf(w->out, "%u %s differs at frame %u\n", w->step++, what, i + 1); return end_walk(w, what);
 }
 fprintf(w->out, "%u %s equal %u\n", w->step++, what, n); return 0;
}
static int expect_host(Walk *w, const char *action, int r, uint32_t kind) {
 if (r != RLW_MENU_HOST || w->menu.host_event.kind != kind) {
  line(w, action, result_name(r)); char why[96]; snprintf(why, sizeof(why), "%s did not request %u", action, kind); return end_walk(w, why);
 }
 fprintf(w->out, "%u %s host %u/%u slot %u\n", w->step++, action, w->menu.host_event.kind, w->menu.host_event.mode, w->menu.host_event.reserved);
 return 0;
}
static void slot_path(const char *directory, uint32_t index, char *out, size_t capacity) {
 snprintf(out, capacity, "%s" RLW_SEP "posicao-%u.rls", directory, index + 1);
}

int rlw_save_demo_run(const RlwDemoOps *ops, void *driver, const char *directory, FILE *out) {
 Walk w; memset(&w, 0, sizeof(w)); w.ops = ops; w.d = driver; w.out = out;
 rlw_menu_init(&w.menu, &ops->menu, driver, NULL);
 static char first[30][65], before[60][65];
 char slot[4200], stage[4200], path[4200];
 slot_path(directory, 0, slot, sizeof(slot)); snprintf(stage, sizeof(stage), "%s" RLW_SEP "inicio-fase.rls", directory);
 int rc = 1, c = 0;
 line(&w, "start", ops->menu.mode(driver) ? "menu" : "play");
 if (ops->menu.mode(driver)) { end_walk(&w, "not in play"); goto done; }
 /* Load tail and first picture, then the stage autosave (the host's stage start). */
 if (play(&w, 1, NULL, 0)) goto done;
 c = ops->autosave(driver, stage); slot_line(&w, "autosave", c, stage);
 if (c) { ops->menu.error(driver, w.menu.error, sizeof(w.menu.error)); end_walk(&w, w.menu.error); goto done; }
 if (play_record(&w, 30, first) || play(&w, 90, NULL, 0)) goto done;
 /* Frame 120: nothing held or queued (the fixed input ends at frame 60). */
 c = ops->save(driver, slot); slot_line(&w, "save", c, slot);
 if (c) { ops->menu.error(driver, w.menu.error, sizeof(w.menu.error)); end_walk(&w, w.menu.error); goto done; }
 if (play_record(&w, 60, before)) goto done;
 c = ops->load(driver, slot); line(&w, "load", c ? "failed" : "ok");
 if (c) { ops->menu.error(driver, w.menu.error, sizeof(w.menu.error)); end_walk(&w, w.menu.error); goto done; }
 if (play_compare(&w, 60, before, "replay")) goto done;
 c = ops->load(driver, stage); line(&w, "load-autosave", c ? "failed" : "ok");
 if (c) { ops->menu.error(driver, w.menu.error, sizeof(w.menu.error)); end_walk(&w, w.menu.error); goto done; }
 if (play_compare(&w, 30, first, "replay-autosave")) goto done;
 if (play(&w, 10, NULL, 0) || escape_to_pause(&w)) goto done;
 /* Salvar (pause item 1): list 14 with one host caption, Enter on its first slot. */
 memset(w.menu.slot_table, 0, sizeof(w.menu.slot_table)); w.menu.slot_table[0] = 1; memcpy(w.menu.slot_table + 1, "Teste", 5);
 if (pointer_off_items(&w) || ticks(&w, 5)) goto done;
 if (keys(&w, "down", RL_SHELL_DOWN, RLW_MENU_REPAINT) || keys(&w, "enter-save", RL_SHELL_ENTER, RLW_MENU_REPAINT)) goto done;
 if (ops->menu.mode(driver) != RL_SHELL_SAVE_LIST) { end_walk(&w, "pause item 1 did not open the save list"); goto done; }
 if (ticks(&w, 5) || expect_host(&w, "enter-slot", rlw_menu_keys(&w.menu, RL_SHELL_ENTER, 0), RL_SHELL_SAVE_SLOT)) goto done;
 int32_t saved = -1; slot_path(directory, w.menu.host_event.reserved, path, sizeof(path));
 c = ops->shell_save_slot(driver, path, &saved); rlw_menu_reset(&w.menu);
 slot_line(&w, saved ? "save-slot-failed" : "save-slot", c, path);
 if (c || saved) { ops->menu.error(driver, w.menu.error, sizeof(w.menu.error)); end_walk(&w, w.menu.error); goto done; }
 if (ops->menu.mode(driver)) { end_walk(&w, "save slot did not resume"); goto done; }
 /* The resumed world: what a load of this paused save must continue as. */
 uint8_t resumed[2] = {0, 0}, loaded[2] = {0, 0}; uint64_t resumed_tick = 0, loaded_tick = 0; uint32_t resumed_ms = 0, loaded_ms = 0;
 if (ops->probe(driver, resumed, &resumed_tick, &resumed_ms)) { end_walk(&w, "probe after the save slot"); goto done; }
 line(&w, "resumed", "game"); char resumed_hex[65]; memcpy(resumed_hex, w.last_hex, 65);
 fprintf(out, "%u resumed-state mode=%u latch=%u clock=%llu/%u\n", w.step++, resumed[0], resumed[1], (unsigned long long)resumed_tick, resumed_ms);
 files_line(&w, directory);
 if (play(&w, 20, NULL, 0) || escape_to_pause(&w)) goto done;
 /* Carregar (pause item 0): list 12, Enter on the first slot: the paused save. */
 if (pointer_off_items(&w) || ticks(&w, 5) || keys(&w, "enter-load", RL_SHELL_ENTER, RLW_MENU_REPAINT)) goto done;
 if (ops->menu.mode(driver) != RL_SHELL_LOAD_LIST) { end_walk(&w, "pause item 0 did not open the load list"); goto done; }
 if (ticks(&w, 5) || expect_host(&w, "enter-slot", rlw_menu_keys(&w.menu, RL_SHELL_ENTER, 0), RL_SHELL_LOAD_SLOT)) goto done;
 slot_path(directory, w.menu.host_event.reserved, path, sizeof(path));
 c = ops->load(driver, path); rlw_menu_reset(&w.menu); line(&w, "load-slot", c ? "failed" : "ok");
 if (c) { ops->menu.error(driver, w.menu.error, sizeof(w.menu.error)); end_walk(&w, w.menu.error); goto done; }
 /* The original load lists continue after GameLoad 0x46b88d with MenuInit(18)
  * and the HUD latch (list12 0x42c57a/0x42c57f): the paused save plays on in
  * mode 18 with the clock and picture it was saved with. */
 c = ops->load_resume(driver); line(&w, "load-resume", c ? "failed" : "ok");
 if (c) { ops->menu.error(driver, w.menu.error, sizeof(w.menu.error)); end_walk(&w, w.menu.error); goto done; }
 if (ops->menu.mode(driver)) { end_walk(&w, "the paused save did not resume play"); goto done; }
 if (ops->probe(driver, loaded, &loaded_tick, &loaded_ms)) { end_walk(&w, "probe after the load resume"); goto done; }
 int same = loaded[0] == 18 && loaded[1] == 1 && loaded_tick == resumed_tick && loaded_ms == resumed_ms && !strcmp(w.last_hex, resumed_hex);
 fprintf(out, "%u loaded-state mode=%u latch=%u clock=%llu/%u picture=%s %s\n", w.step++, loaded[0], loaded[1], (unsigned long long)loaded_tick, loaded_ms,
         strcmp(w.last_hex, resumed_hex) ? "differs" : "equal", same ? "ok" : "FAILED");
 if (!same) { end_walk(&w, "the loaded paused save is not the resumed game"); goto done; }
 uint64_t from = ops->frames(driver);
 if (play(&w, 10, NULL, 0)) goto done;
 if (ops->menu.mode(driver) || ops->frames(driver) != from + 10) { end_walk(&w, "the loaded paused save did not play 10 frames"); goto done; }
 if (play(&w, 20, NULL, 0)) goto done;
 files_line(&w, directory);
 /* Periodic autosave (Partida > Salvamento automático): the host's schedule over
  * eligible frames, with a 40-frame test interval, writing the ordinary save to
  * its own slot "autosave"; a busy boundary retries at the next frame. */
 RlwAutosave periodic; rlw_autosave_init(&periodic, 40);
 snprintf(path, sizeof(path), "%s" RLW_SEP RLW_AUTOSAVE_SLOT ".rls", directory);
 for (unsigned i = 0; i < 100; i++) {
  if (play(&w, 1, NULL, 0)) goto done;
  if (ops->menu.mode(driver)) { end_walk(&w, "the game paused during the periodic autosave"); goto done; }
  if (!rlw_autosave_frame(&periodic, 1)) continue;
  c = ops->save(driver, path);
  if (c == RL_SAVE_BUSY) continue;
  slot_line(&w, "autosave-periodic", c, path);
  if (c) { ops->menu.error(driver, w.menu.error, sizeof(w.menu.error)); end_walk(&w, w.menu.error); goto done; }
  rlw_autosave_saved(&periodic);
 }
 files_line(&w, directory);
 fprintf(out, "end complete\n");
 rc = 0;
done:
 rlw_menu_close(&w.menu);
 return rc;
}
