/* In-game FLC events for the Windows host (front 2), portable C. Port of
 * NativeEventFLCPlayback.swift over rlw_flc.c (engine/include/event-flc.md,
 * steps 2-5). After each play frame the host polls the engine's event state;
 * while an event runs, the decoded FLC frame is overlaid centred on the
 * 800x600 indexed view with the FLC palette, frames advance by the host clock
 * at the file's speed, terminar is called once after the last frame has been
 * shown for its delay, and the view goes back to the engine's pixels and game
 * palette when the event ends. Each display step after the opening one, while
 * frames remain, runs the host's tick (EvenementAffiche's loop PostSound, see
 * rlw_audio_event_tick). */
#ifndef RLW_EVENTFLC_H
#define RLW_EVENTFLC_H
#include <stdint.h>
#include "rlw_flc.h"

/* Same layout as RlEventFlcStateV1. */
typedef struct { uint32_t running, modal, index, frames, width, height, speed; } RlwEventFlcState;
typedef struct {
 void *ctx;
 int (*name)(void *ctx, uint32_t index, char out[16]);   /* 0 = found */
 int (*finish)(void *ctx);                                /* rl_evento_flc_terminar_v1 */
 void (*tick)(void *ctx);
} RlwEventFlcHost;
typedef struct {
 char folder[4096];            /* Dados/events */
 RlwFlc flc; int open, finished; uint32_t index; double started_ms;
 uint64_t terminations, ticks, opened;
 char failure[200];
 uint8_t surface[480000];
} RlwEventFlc;

void rlw_eventflc_init(RlwEventFlc *e, const char *folder);
void rlw_eventflc_free(RlwEventFlc *e);
/* Returns 1 when e->surface and e->flc.palette replace the engine's pixels and
 * palette for this frame, 0 to show the engine's own. */
int rlw_eventflc_update(RlwEventFlc *e, const RlwEventFlcState *state, double now_ms, const uint8_t *engine_pixels,
                        const RlwEventFlcHost *host);
#endif
