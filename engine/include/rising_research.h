#ifndef RISING_RESEARCH_H
#define RISING_RESEARCH_H
#include "rising_shell.h"
#ifdef __cplusplus
extern "C" {
#endif
/* "Redistribuir descobertas" extension v1: a port-only option (RISING.EXE has
 * none) on the briefing screen (prepared shell mode 17), offered by the host
 * when the Options value `redistribuir` is on (rising_options.h v3).
 *
 * Each research pillar gets a - under its base and a + over its top. - returns
 * the pillar's last discovery as a free point; + spends a free point on the
 * pillar's next discovery, in the branch's original order. A pillar never drops
 * below the discoveries the mission grants by itself, never grows past the
 * entries the mission makes available (religion stays empty before mission 7),
 * and the total never changes. Free points left when the briefing is left (by
 * rl_shell_input, e.g. starting the mission or going back) return to the
 * pillars they came from, newest first, inside that same call.
 *
 * A step writes the local player's stored research record (0x508134) and runs
 * the original RechercheCalculNew 0x47150f again; the result is a distribution
 * the original reaches by researching other branches, so worlds and saves keep
 * their format. Same conventions as rising_shell.h: integer handles, buffers
 * copied, failures leave the session and outputs unchanged, rl_erro_mensagem
 * holds the diagnostic. Hit tests, state and drawing never change the session. */
#define RL_REDISTRIBUIR_VERSION 1u
typedef struct {
 uint32_t version;    /* RL_REDISTRIBUIR_VERSION */
 uint32_t ativo;      /* 1 while the session shows the briefing (mode 17) */
 uint32_t livres;     /* free points of the current edit */
 uint32_t nivel[4];   /* per branch: 0 agriculture, 1 military, 2 civil, 3 religious */
 uint32_t minimo[4];  /* discoveries the mission grants by itself */
 uint32_t maximo[4];  /* entries the mission makes available */
} RlRedistribuicao;
uint32_t rl_redistribuir_versao(void);
int32_t rl_redistribuir_estado(RlShell shell, RlRedistribuicao *output);
/* Button under the 800x600 point (x, y) that acts now: *ramo 0..3 and
 * *delta +1/-1, or *delta 0 when none. */
int32_t rl_redistribuir_alvo(RlShell shell, uint32_t x, uint32_t y, uint32_t *ramo, int32_t *delta);
/* One - (delta -1) or + (delta +1) on branch ramo. *mudou 0 when the button does
 * not act (nothing changes, the generation stays). Otherwise the session memory
 * changes and *generation is the new one; repaint the briefing picture
 * (rl_shell_paint_prepared) and draw the overlay again. */
int32_t rl_redistribuir_passo(RlShell shell, uint64_t generation, uint32_t ramo, int32_t delta,
 uint64_t *new_generation, uint32_t *mudou);
/* The enabled buttons and the free-points line over the host's copy of the
 * briefing picture (480000 indices) with its 768-byte palette; idioma 0 pt-BR,
 * 1 English. Nothing outside the briefing. */
int32_t rl_redistribuir_sobrepor(RlShell shell, uint8_t *pixels, uint32_t capacity,
 const uint8_t *palette, uint32_t palette_length, uint32_t idioma);
#ifdef __cplusplus
}
#endif
#endif
