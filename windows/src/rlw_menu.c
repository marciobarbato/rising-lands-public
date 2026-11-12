/* Port of NativeLiveWindow.swift's menu routing (see rlw_menu.h). */
#include "rlw_menu.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <string.h>

void rlw_menu_init(RlwMenu *m, const RlwMenuOps *ops, void *driver, const char *credits_path) {
 memset(m, 0, sizeof(*m)); m->ops = ops; m->driver = driver;
 snprintf(m->credits_path, sizeof(m->credits_path), "%s", credits_path ? credits_path : "");
}
void rlw_menu_reset(RlwMenu *m) { memset(m->held, 0, sizeof(m->held)); m->move_pending = 0; m->mouse_down = 0; }
static void close_credits(RlwMenu *m) { if (m->credits_open) rlw_flc_free(&m->credits); m->credits_open = 0; }
void rlw_menu_close(RlwMenu *m) { close_credits(m); }
void rlw_menu_cancel(RlwMenu *m) { m->move_pending = 0; m->mouse_down = 0; }
uint32_t rlw_menu_tick_ms(const RlwMenu *m) { return m->ops->mode(m->driver) == RL_SHELL_CREDITS_SCREEN ? 10 : 50; }

static int failed(RlwMenu *m, int code, const char *fallback) {
 char detail[512] = {0}; if (m->ops->error) m->ops->error(m->driver, detail, sizeof(detail));
 snprintf(m->error, sizeof(m->error), "%s", detail[0] ? detail : fallback);
 if (!m->error[0]) snprintf(m->error, sizeof(m->error), "Tela original: erro %d", code);
 return RLW_MENU_FAILED;
}

/* beginCredits(): the host decoder opens before the engine's MenuInit(10); an
 * absent file is the original's failed open (the engine then ends the screen). */
static int begin_credits(RlwMenu *m, double now_ms) {
 close_credits(m);
 if (m->credits_path[0] && rlw_exists(m->credits_path)) {
  if (rlw_flc_open(&m->credits, m->credits_path)) { snprintf(m->error, sizeof(m->error), "CREDITS.FLC: %s", m->credits.error); rlw_flc_free(&m->credits); return -1; }
  m->credits_open = 1;
 }
 int c = m->ops->show(m->driver); m->credits_read_ms = now_ms;
 return c;
}
/* creditsFrame(): the engine owns the counter, the host the decoder and the
 * clock (elapsed = ms since the last FLC frame read). *changed = new picture. */
static int credits_frame(RlwMenu *m, double now_ms, int *changed) {
 RlShellCredits out; memset(&out, 0, sizeof(out)); *changed = 0;
 double elapsed = now_ms - m->credits_read_ms; if (elapsed < 0) elapsed = 0;
 int c = m->ops->credits_frame(m->driver, (uint32_t)elapsed, &out);
 if (c) return c;
 if (out.state == RL_SHELL_CREDITS_ENDED) { close_credits(m); *changed = 1; return 0; }
 if (out.state != RL_SHELL_CREDITS_DRAW) return 0;
 if (!m->credits_open || m->credits.width != out.width || m->credits.height != out.height) { snprintf(m->error, sizeof(m->error), "CREDITS.FLC: cabeçalho diferente do declarado"); return -1; }
 if (m->credits.shown > out.frame) return 0;
 while (m->credits.shown <= out.frame) if (rlw_flc_next(&m->credits)) { snprintf(m->error, sizeof(m->error), "CREDITS.FLC: %s", m->credits.error); return -1; }
 m->credits_read_ms = now_ms; *changed = 1;
 return m->ops->credits_present(m->driver, &out, m->credits.pixels, m->credits.palette);
}

/* sendShell(input:) */
/* publishSlots(): the host's current captions, then the driver's copy. */
static int publish(RlwMenu *m) {
 if (m->publish_slots) m->publish_slots(m->publish_ctx, m->slot_table);
 return m->ops->slots(m->driver, m->slot_table, sizeof(m->slot_table));
}
/* sendShell(input:optionsKeys:): the port's options screen sees the input first. */
static int send(RlwMenu *m, const RlShellInput *supplied, uint32_t option_keys, double now_ms) {
 if (!m->ops->mode(m->driver)) return RLW_MENU_IDLE;
 if (m->ops->options_input) {
  RlOpcoesEvento o; memset(&o, 0, sizeof(o));
  int c = m->ops->options_input(m->driver, supplied, option_keys, &o);
  if (c) return failed(m, c, "Tela de opções");
  if (o.consumed) {
   if (o.kind == RL_OPCOES_ABRIU || o.kind == RL_OPCOES_FECHOU) rlw_menu_reset(m);
   if (o.kind == RL_OPCOES_MUDOU || o.kind == RL_OPCOES_CONFIGURAR_TECLAS) { m->options_event = o; return RLW_MENU_OPTIONS; }
   return RLW_MENU_REPAINT;
  }
 }
 if (supplied->keys == 0 && option_keys != 0) return RLW_MENU_IDLE; /* the screen's keys only */
 RlShellInput input = *supplied; RlShellEvent event; memset(&event, 0, sizeof(event));
 int c = m->ops->input(m->driver, &input, &event);
 if (c) return failed(m, c, "Entrada na tela original");
 if (event.kind != RL_SHELL_NONE) { m->zone_known[0] = m->zone_known[1] = 0; }
 switch (event.kind) {
 case RL_SHELL_NONE:
  if (m->ops->mode(m->driver) == RL_SHELL_CREDITS_SCREEN) {
   int changed = 0; c = credits_frame(m, now_ms, &changed);
   if (c < 0) return RLW_MENU_FAILED;
   if (!c && !changed) return RLW_MENU_IDLE;
  } else c = m->ops->draw(m->driver, input.animation_84);
  break;
 case RL_SHELL_SHOW: case RL_SHELL_SAVE_LOAD:
  close_credits(m); c = publish(m); if (!c) c = m->ops->show(m->driver); rlw_menu_reset(m);
  break;
 case RL_SHELL_MISSION_NEXT:
  c = m->ops->prepare(m->driver); rlw_menu_reset(m);
  break;
 case RL_SHELL_RESUME:
  /* Pause "continuar"/Escape: original in-game MenuInit(18), same clock and frame. */
  c = m->ops->request(m->driver);
  if (!c) { rlw_menu_reset(m); m->game_kind = RL_SHELL_RESUME; return RLW_MENU_GAME; }
  break;
 case RL_SHELL_RESET_CAMPAIGN:
  /* "Novo jogo"/"Carregar jogo": the driver owns the engine call. */
  c = publish(m); if (!c) c = m->ops->request(m->driver); rlw_menu_reset(m);
  break;
 case RL_SHELL_START_MISSION:
  c = m->ops->load(m->driver);
  if (!c) { rlw_menu_reset(m); m->game_kind = RL_SHELL_START_MISSION; return RLW_MENU_GAME; }
  break;
 case RL_SHELL_CREDITS:
  c = begin_credits(m, now_ms); rlw_menu_reset(m);
  if (c < 0) return RLW_MENU_FAILED;
  break;
 case RL_SHELL_QUIT: case RL_SHELL_MULTIPLAYER: case RL_SHELL_DEFEAT_MOVIE: case RL_SHELL_CAMPAIGN_MOVIE:
 case RL_SHELL_INTRODUCTION: case RL_SHELL_LOAD_SLOT: case RL_SHELL_SAVE_SLOT:
  m->host_event = event; rlw_menu_reset(m);
  return RLW_MENU_HOST;
 default:
  snprintf(m->error, sizeof(m->error), "Pedido da tela original ainda não disponível: %u", event.kind);
  return RLW_MENU_FAILED;
 }
 if (c) return failed(m, c, "Tela original");
 return RLW_MENU_REPAINT;
}

int rlw_menu_send(RlwMenu *m, const RlShellInput *input, double now_ms) { return send(m, input, input->keys, now_ms); }

int rlw_menu_keys(RlwMenu *m, uint32_t keys, double now_ms) {
 RlShellInput input; if (m->ops->state(m->driver, &input)) return RLW_MENU_IDLE;
 input.keys = keys & 15u; input.mouse_ae = 0; input.mouse_af = 0;
 return send(m, &input, keys, now_ms);
}

/* flushShellMove() */
int rlw_menu_flush_move(RlwMenu *m, double now_ms) {
 if (!m->move_pending) return RLW_MENU_IDLE;
 m->move_pending = 0;
 RlShellInput input; if (m->ops->state(m->driver, &input)) return RLW_MENU_IDLE;
 input.mouse_x = m->move_x; input.mouse_y = m->move_y; input.mouse_af = 0; input.mouse_b0 = 0;
 input.mouse_ae = m->mouse_down ? 1 : 0;
 /* A held button must not turn a second page at an already used clock. */
 if (m->mouse_down) input.clock_7c = m->ops->click_clock(m->ops->mode(m->driver), input.clock_7c, 0);
 return rlw_menu_send(m, &input, now_ms);
}

/* pageObjectives(): a click on the original arrow zone 6/7 at an aligned clock,
 * then the pointer returns. Same engine path as a real click. */
int rlw_menu_page(RlwMenu *m, int next, double now_ms) {
 uint32_t mode = m->ops->mode(m->driver);
 if ((mode != RL_SHELL_BRIEFING && mode != RL_SHELL_OBJECTIVES) || m->mouse_down) return RLW_MENU_IDLE;
 unsigned slot = next ? 1 : 0; uint32_t zone = next ? 7 : 6;
 if (!m->zone_known[slot]) {
  int c = m->ops->zone(m->driver, zone, m->zone_rect[slot]);
  if (c) return failed(m, c, "Leitura da tela original");
  m->zone_known[slot] = 1;
 }
 int32_t *rect = m->zone_rect[slot];
 if (!(rect[2] > rect[0] && rect[3] > rect[1] && rect[0] >= 0 && rect[1] >= 0)) return RLW_MENU_IDLE;
 RlShellInput input; if (m->ops->state(m->driver, &input)) return RLW_MENU_IDLE;
 uint32_t px = m->move_pending ? m->move_x : input.mouse_x, py = m->move_pending ? m->move_y : input.mouse_y; m->move_pending = 0;
 input.mouse_x = (uint32_t)((rect[0] + rect[2]) / 2); input.mouse_y = (uint32_t)((rect[1] + rect[3]) / 2);
 input.mouse_ae = 1; input.mouse_af = 0; input.mouse_b0 = 0;
 input.clock_7c = m->ops->click_clock(mode, input.clock_7c, 1);
 int r = rlw_menu_send(m, &input, now_ms);
 if (r != RLW_MENU_REPAINT && r != RLW_MENU_IDLE) return r;
 if (m->ops->mode(m->driver) != mode || m->ops->state(m->driver, &input)) return r;
 input.mouse_x = px; input.mouse_y = py; input.mouse_ae = 0; input.mouse_af = 0; input.mouse_b0 = 0;
 int s = rlw_menu_send(m, &input, now_ms);
 return s == RLW_MENU_IDLE ? r : s;
}

/* shellEvent(), key branch: Enter/Up/Down/Escape act on release; on the
 * objective/briefing pages Up/Down turn a page on press (host convenience). */
int rlw_menu_key(RlwMenu *m, uint32_t vk, int down, double now_ms) {
 if (vk > 255) return RLW_MENU_IDLE;
 uint32_t mode = m->ops->mode(m->driver);
 int pages = mode == RL_SHELL_BRIEFING || mode == RL_SHELL_OBJECTIVES;
 int arrow = pages && (vk == 38 || vk == 40); int next = vk == 40;
 if (down) {
  if (!m->held[vk]) { m->held[vk] = 1; if (arrow) return rlw_menu_page(m, next, now_ms); }
  return RLW_MENU_IDLE;
 }
 if (!m->held[vk]) return RLW_MENU_IDLE;
 m->held[vk] = 0;
 if (arrow) return RLW_MENU_IDLE;
 uint32_t keys = vk == 13 ? RL_SHELL_ENTER : vk == 38 ? RL_SHELL_UP : vk == 40 ? RL_SHELL_DOWN : vk == 27 ? RL_SHELL_ESCAPE : 0;
 /* Left/Right exist only on the port's options screen (never for the shell). */
 if (!keys && m->ops->options_open && m->ops->options_open(m->driver)) keys = vk == 37 ? RL_OPCOES_ESQUERDA : vk == 39 ? RL_OPCOES_DIREITA : 0;
 return keys ? rlw_menu_keys(m, keys, now_ms) : RLW_MENU_IDLE;
}

/* shellEvent(), mouse branch. */
int rlw_menu_mouse(RlwMenu *m, uint32_t msg, uint32_t x, uint32_t y, double now_ms) {
 if (msg == 0x200) { m->move_pending = 1; m->move_x = x; m->move_y = y; return RLW_MENU_IDLE; }
 m->move_pending = 0; /* the press/release carries the newer position */
 RlShellInput input; if (m->ops->state(m->driver, &input)) return RLW_MENU_IDLE;
 input.mouse_x = x; input.mouse_y = y; input.mouse_af = 0; input.mouse_b0 = 0;
 int press = msg == 0x201 || msg == 0x203;
 if (press) m->mouse_down = 1;
 input.mouse_ae = m->mouse_down ? 1 : 0;
 if (msg == 0x202) { input.mouse_af = m->mouse_down ? 1 : 0; m->mouse_down = 0; }
 /* Objective/briefing arrows: one page per click. */
 if (press || input.mouse_ae) input.clock_7c = m->ops->click_clock(m->ops->mode(m->driver), input.clock_7c, press ? 1 : 0);
 return rlw_menu_send(m, &input, now_ms);
}

/* shellTick() */
int rlw_menu_tick(RlwMenu *m, double now_ms) {
 RlShellInput input; if (!m->ops->mode(m->driver) || m->ops->state(m->driver, &input)) return RLW_MENU_IDLE;
 input.clock_7c += 1; input.animation_84 += 1;
 input.mouse_ae = m->mouse_down ? 1 : 0; input.mouse_af = 0;
 if (m->move_pending) { input.mouse_x = m->move_x; input.mouse_y = m->move_y; m->move_pending = 0; }
 return rlw_menu_send(m, &input, now_ms);
}
