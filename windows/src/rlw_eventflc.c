/* In-game FLC events (see rlw_eventflc.h); NativeEventFLCPlayback.update line by line. */
#include "rlw_eventflc.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <string.h>

void rlw_eventflc_init(RlwEventFlc *e, const char *folder) {
 memset(e, 0, sizeof(*e)); e->index = UINT32_MAX;
 snprintf(e->folder, sizeof(e->folder), "%s", folder ? folder : ".");
}
static void close_flc(RlwEventFlc *e) { if (e->open) rlw_flc_free(&e->flc); e->open = 0; }
void rlw_eventflc_free(RlwEventFlc *e) { if (e) close_flc(e); }

static int fail(RlwEventFlc *e, const char *text) { snprintf(e->failure, sizeof(e->failure), "%s", text); e->finished = 1; return 0; }

int rlw_eventflc_update(RlwEventFlc *e, const RlwEventFlcState *state, double now_ms, const uint8_t *engine_pixels,
                        const RlwEventFlcHost *host) {
 if (!state->running) { close_flc(e); e->index = UINT32_MAX; e->finished = 0; return 0; }
 if (e->finished) return 0;
 int opening = !e->open || e->index != state->index;
 if (opening) {
  close_flc(e); e->index = state->index; e->started_ms = now_ms; e->failure[0] = 0;
  char stem[16] = {0}, text[200];
  if (!host || !host->name || host->name(host->ctx, state->index, stem)) { snprintf(text, sizeof(text), "Evento FLC %u sem nome", state->index); return fail(e, text); }
  stem[15] = 0;
  char path[4200]; snprintf(path, sizeof(path), "%s" RLW_SEP "%s.FLC", e->folder, stem);
  if (rlw_flc_open(&e->flc, path)) { snprintf(text, sizeof(text), "Evento %s: %s", stem, e->flc.error); rlw_flc_free(&e->flc); return fail(e, text); }
  e->open = 1;
  if (e->flc.frames != state->frames || e->flc.width != state->width || e->flc.height != state->height) {
   snprintf(text, sizeof(text), "Evento %s: cabeçalho diferente do declarado", stem); close_flc(e); return fail(e, text);
  }
  if (rlw_flc_next(&e->flc)) { snprintf(text, sizeof(text), "Evento %s: %s", stem, e->flc.error); close_flc(e); return fail(e, text); }
  e->opened++;
 }
 double delay = e->flc.delay_ms > 1 ? e->flc.delay_ms : 1;
 double elapsed = now_ms - e->started_ms;
 uint32_t target = (uint32_t)(1 + (elapsed > 0 ? elapsed / delay : 0));
 if (target > e->flc.frames) target = e->flc.frames;
 while (e->flc.shown < target)
  if (rlw_flc_next(&e->flc)) { char text[200]; snprintf(text, sizeof(text), "Evento FLC: %s", e->flc.error); close_flc(e); return fail(e, text); }
 if (!opening && e->flc.shown < e->flc.frames && host && host->tick) { host->tick(host->ctx); e->ticks++; }
 if (e->flc.shown >= e->flc.frames && state->modal && elapsed >= (double)e->flc.frames * delay) {
  /* The last frame has been shown for its delay: the original player's end. */
  e->terminations++;
  if (!host || !host->finish || host->finish(host->ctx)) snprintf(e->failure, sizeof(e->failure), "rl_evento_flc_terminar_v1 recusado");
  e->finished = 1;
  return 0;
 }
 memcpy(e->surface, engine_pixels, sizeof(e->surface));
 uint32_t left = (800 - e->flc.width) / 2, top = (600 - e->flc.height) / 2;
 for (uint32_t y = 0; y < e->flc.height; y++) memcpy(e->surface + (size_t)(top + y) * 800 + left, e->flc.pixels + (size_t)y * e->flc.width, e->flc.width);
 return 1;
}
