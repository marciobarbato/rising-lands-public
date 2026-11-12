/* App orchestration/transport only. Actual logic and pixels come from the Rust ABI.
 * Ported from macos/src/macos_live_driver.c (production path);
 * comments name the macOS function each part mirrors. */
#include "rlw_driver.h"
#include "rlw_fs.h"
#include "rising_engine.h"
#include "rising_difficulty.h"
#include "rising_load_tail_input.h"
#include "game_hud_background_abi.h"
#include "rising_shell.h"
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAX_INPUT (128u * 1024u * 1024u)
/* Gameplay presentation: surface, terrain cache keys/payloads, cursor. */
#define PRESENTATION_SIZE (480000u + 1024u + 270336u + 4u)
/* Logical original drive directory of EVENTS.INI (macOS Info.plist
 * RisingLandsEventsDirectory). A host namespace, not a game rule. */
static const char EVENTS_DIRECTORY[] = "C:\\RISING";

typedef struct { uint64_t frame; int32_t x, y; RlInputMessage message; uint32_t cursor; } QueuedInput;

struct RlwDriver {
 char *root; char mission[16];
 uint8_t *events_directory, *events_bytes; RlEventsHostV1 events_host;
 RlMundo world; RlRenderer renderer; RlFrameInput input, phase2;
 QueuedInput pending[4096]; uint32_t pending_count, input_next_phase;
 RlInputMessage frame_messages[4096]; RlInputKey frame_keys[256];
 int32_t held[256]; int32_t cursor_x, cursor_y;
 uint64_t mouse_until, left_down_frame; int32_t left_held, left_x, left_y;
 uint32_t phase_index; int started;
 RlwStatus status;
 uint8_t pixels[480000], palette[768], game_background[480000];
 int game_background_ready, game_surface;
 uint32_t difficulty;
 /* Prepared shell session (macos_campaign_shell.inc): menus, briefing, pause, result. */
 RlShell shell; uint64_t shell_generation; uint32_t shell_mode; RlShellInput shell_input; RlShellEvent shell_request;
 /* menu_world: started in the original main menu (no frame state, no renderer).
  * new_campaign: START_MISSION loads without a victory: 1 after "Novo jogo", the
  * current stage after the pause menu's "Reiniciar a missão". */
 int menu_world, new_campaign;
 /* Host captions for the original save lists: [present, 32-byte CP1252 caption] x10. */
 uint8_t slots[10][33];
 /* Gameplay presentation retained across the pause menu. */
 uint8_t *pause_presentation, pause_palette[768];
 char save_error[512];
};

static uint8_t *file(const char *root, const char *name, uint32_t *length) {
 char path[4096]; snprintf(path, sizeof(path), "%s" RLW_SEP "%s", root, name);
 return rlw_read_file(path, length, MAX_INPUT);
}

static int fail(RlwDriver *d, int code, const char *context) {
 d->status.stopped = 1; uint32_t n = 0; char detail[320] = {0};
 rl_erro_mensagem(NULL, 0, &n); if (n && n <= sizeof(detail)) rl_erro_mensagem((uint8_t *)detail, n, &n);
 snprintf(d->status.message, sizeof(d->status.message), "%s · quadro %llu · ABI %d: %s", context,
          (unsigned long long)(d->status.frames + 1), code, detail);
 return code ? code : 5;
}
static int named_stop(RlwDriver *d, const char *name) {
 d->status.stopped = 1; snprintf(d->status.message, sizeof(d->status.message), "%s", name); return 1;
}

int rlw_engine_check(char *error, size_t capacity) {
 uint32_t v = rl_versao();
 if ((v >> 16) != 1 || (v & 0xffff) < 6 || rl_render_contrato() != 0x10009) { snprintf(error, capacity, "ABI1.6 / Graphics1.9 required (engine %08x, render %08x)", v, rl_render_contrato()); return 1; }
 if (rl_salvar_versao() != RL_SAVE_FORMAT_VERSION) { snprintf(error, capacity, "Save extension v1 required"); return 1; }
 if (rl_game_hud_background_version() != RL_GAME_HUD_BACKGROUND_VERSION) { snprintf(error, capacity, "Gameplay background extension v1 required"); return 1; }
 if (rl_missao_versao() != 1) { snprintf(error, capacity, "Mission extension v1 required"); return 1; }
 return 0;
}

/* macos_event_flc.inc: one entry per distinct [EVENEMENT] value of EVENTS.INI,
 * sorted, checked against events/NAME.FLC (the imported copy of the CD's EVENTS). */
static int flc_name_ok(const char *name) {
 size_t n = strlen(name); if (n < 1 || n > 15) return 0;
 for (size_t i = 0; i < n; i++) if (!isalnum((unsigned char)name[i]) && name[i] != '_') return 0;
 return 1;
}
static int flc_compare(const void *a, const void *b) { return strcmp(a, b); }
static uint32_t le_u32(const uint8_t *p) { return p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24); }
/* event_flc_present(): the engine keeps the declared table in its own world
 * block (0x06300000). */
static int event_flc_present(RlMundo world) {
 uint32_t n = 0; if (rl_projetar_tudo(world, NULL, 0, &n) != RL_BUFFER_TOO_SMALL) return 0;
 uint8_t *data = malloc(n); if (!data) return 0; int found = 0;
 if (!rl_projetar_tudo(world, data, n, &n) && n >= 4) {
  uint32_t off = 4;
  for (uint32_t i = 0; i < le_u32(data); i++) {
   if ((uint64_t)off + 8 > n) break; uint32_t base = le_u32(data + off), length = le_u32(data + off + 4); off += 8;
   if ((uint64_t)off + length > n) break; if (base == 0x06300000) found = 1; off += length;
  }
 }
 free(data); return found;
}
/* Every new or loaded world, before its first frame. A world resumed from the
 * pause menu (or continued from one that had it) already carries its table and
 * is left as it is, without reading EVENTS.INI; "already declared" is no error. */
static int event_flc_declare(RlwDriver *d, RlMundo world) {
 if (!world || event_flc_present(world)) return 0;
 uint32_t n = 0; uint8_t *ini = file(d->root, "EVENTS.INI", &n); if (!ini) return RL_IO_ERROR;
 static char names[256][16]; uint32_t found = 0; int section = 0;
 for (uint32_t i = 0; i < n;) {
  uint32_t j = i; while (j < n && ini[j] != '\n' && ini[j] != '\r') j++;
  char line[256]; uint32_t len = j - i < 255 ? j - i : 255; memcpy(line, ini + i, len); line[len] = 0;
  i = j; while (i < n && (ini[i] == '\n' || ini[i] == '\r')) i++;
  char *s = line; while (*s == ' ' || *s == '\t') s++;
  if (*s == '[') {
   char head[12] = {0}; for (int k = 0; k < 11 && s[k]; k++) head[k] = (char)toupper((unsigned char)s[k]);
   section = !strcmp(head, "[EVENEMENT]"); continue;
  }
  char *eq = strchr(s, '='); if (!section || !eq) continue;
  char *v = eq + 1; while (*v == ' ' || *v == '\t') v++;
  char *e = v + strlen(v); while (e > v && (e[-1] == ' ' || e[-1] == '\t')) *--e = 0;
  for (char *c = v; *c; c++) *c = (char)toupper((unsigned char)*c);
  if (!flc_name_ok(v)) { free(ini); return RL_INVALID_ARGUMENT; }
  int dup = 0; for (uint32_t k = 0; k < found; k++) if (!strcmp(names[k], v)) dup = 1;
  if (!dup) { if (found == 256) { free(ini); return RL_INVALID_ARGUMENT; } snprintf(names[found++], 16, "%s", v); }
 }
 free(ini);
 if (!found) return 0;
 qsort(names, found, 16, flc_compare);
 RlEventFlcV1 *table = calloc(found, sizeof(*table)); if (!table) return RL_MEMORY_ERROR;
 for (uint32_t k = 0; k < found; k++) {
  memcpy(table[k].name, names[k], strlen(names[k]));
  char path[4096]; snprintf(path, sizeof(path), "%s" RLW_SEP "events" RLW_SEP "%s.FLC", d->root, names[k]);
  FILE *f = rlw_fopen(path, "rb");
  if (f) { table[k].available = fread(table[k].header, 1, 128, f) == 128; fclose(f); }
  if (!table[k].available) memset(table[k].header, 0, 128);
 }
 int c = rl_eventos_flc_v1(world, table, found); free(table);
 if (c) {
  char detail[320] = {0}; uint32_t m = 0; rl_erro_mensagem(NULL, 0, &m); if (m && m <= sizeof(detail)) rl_erro_mensagem((uint8_t *)detail, m, &m);
  if (strstr(detail, "already declared")) return 0;
 }
 return c;
}

static int refresh_screen(RlwDriver *d);
static int shell_open(RlwDriver *d, uint32_t mode);
static int save_result(RlwDriver *d, int code, const char *action);

/* game_background(): the gameplay seed decoded once from PLATGAME.PIE. */
static int game_background(RlwDriver *d, const uint8_t *pie, uint32_t length) {
 if (d->game_background_ready) return 0;
 uint8_t *owned = NULL; if (!pie) { owned = file(d->root, "asset6.bin", &length); pie = owned; }
 int c = pie ? rl_game_hud_background_initialize(RL_GAME_HUD_BACKGROUND_VERSION, pie, length, d->game_background, sizeof(d->game_background)) : RL_IO_ERROR;
 free(owned); if (!c) d->game_background_ready = 1; return c;
}

/* new_renderer_cd(): the six CMP banks and PLATGAME.PIE; a retained
 * presentation (pause) carries its own surface and terrain cache. */
static int new_renderer(RlwDriver *d, RlMundo world, const uint8_t *presentation, RlRenderer *out) {
 RlRenderBytes assets[7] = {0}; uint8_t *back[7] = {0}; char name[32]; uint32_t n = 0; int c = 1;
 for (unsigned i = 0; i < 7; i++) { snprintf(name, sizeof(name), "asset%u.bin", i); back[i] = file(d->root, name, &n); assets[i] = (RlRenderBytes){back[i], n}; if (!back[i]) goto done; }
 if (presentation) {
  RlRenderBytes sd = {presentation, 480000};
  uint32_t cursor; memcpy(&cursor, presentation + 751360, 4);
  RlRenderCache cache = {{presentation + 480000, 1024}, {presentation + 481024, 270336}, cursor};
  c = rl_render_criar_cd(world, assets, 7, &sd, out);
  if (!c) { c = rl_render_reset(*out, &sd, &cache); if (c) rl_render_liberar(*out); }
 } else {
  c = game_background(d, back[6], assets[6].length);
  if (!c) { RlRenderBytes sd = {d->game_background, 480000}; c = rl_render_criar_cd(world, assets, 7, &sd, out); }
  if (!c) { memcpy(d->pixels, d->game_background, 480000); d->game_surface = 1; }
 }
done:
 for (unsigned i = 0; i < 7; i++) free(back[i]);
 return c;
}

/* capture_presentation(): surface + renderer cache + cursor (no state change). */
static int capture_presentation(RlwDriver *d, uint8_t *out) {
 uint32_t keys = 0, payloads = 0, cursor = 0;
 int c = rl_render_cache(d->renderer, out + 480000, 1024, out + 481024, 270336, &keys, &payloads, &cursor);
 if (c) return c; if (keys != 1024 || payloads != 270336) return 5;
 memcpy(out, d->pixels, 480000); memcpy(out + 751360, &cursor, 4); return 0;
}

static void base_config(RlwDriver *d, RlMissaoInicio *config) {
 memset(config, 0, sizeof(*config)); config->version = 1; config->allocation_fill = 0; config->ai_clock_ms = 0x123456;
 config->thread_id = 1; config->ticks_per_second = 20; config->execution_mode = RL_EXECUCAO_JOGO;
 config->iterations[0] = 5; config->iterations[1] = 2; config->iterations[2] = 1;
 config->input = d->input; config->input.cursor_x = 400; config->input.cursor_y = 300;
}

/* create(): mission branch (mission != NULL) or the original program start into
 * the main menu (MenuInit 2, first nine descriptors, no stage and no renderer),
 * both with an explicit EVENTS host. */
static RlwDriver *create(const char *root, const char *mission, uint32_t difficulty, char *error, size_t capacity) {
 error[0] = 0;
 if (!root || (mission && (strchr(mission, '/') || strchr(mission, '\\') || strlen(mission) >= 16)) || difficulty > 2) {
  snprintf(error, capacity, "Contrato explícito de missão inválido"); return NULL;
 }
 if (rlw_engine_check(error, capacity)) return NULL;
 RlwDriver *d = calloc(1, sizeof(*d)); if (!d) { snprintf(error, capacity, "Sem memória"); return NULL; }
 d->phase_index = UINT32_MAX; snprintf(d->mission, sizeof(d->mission), "%s", mission ? mission : "MENU"); d->difficulty = difficulty;
 d->root = malloc(strlen(root) + 1); if (!d->root) goto bad; strcpy(d->root, root);
 uint32_t n = 0; int c = 0;
 /* EVENTS host: logical directory + the imported EVENTS.INI bytes. */
 d->events_bytes = file(root, "EVENTS.INI", &n);
 if (!d->events_bytes || n == 0 || n > 1024 * 1024 || memchr(d->events_bytes, 0, n)) { snprintf(error, capacity, "EVENTS.INI original ausente ou inválido"); goto bad; }
 uint32_t dl = (uint32_t)strlen(EVENTS_DIRECTORY);
 d->events_directory = malloc(dl); if (!d->events_directory) goto bad; memcpy(d->events_directory, EVENTS_DIRECTORY, dl);
 d->events_host = (RlEventsHostV1){sizeof(RlEventsHostV1), 1, d->events_directory, dl, d->events_bytes, n};
 /* The ABI's host input context (no game data): WinMain entry registers before
  * load_tail (phase 1) and between frames (phase 2), offscreen binding, message slot. */
 d->input = (RlFrameInput){0, 0, 0x60076000u, {1, 0, 0, 0, 0, 0, 0, 0x2007ec0u}, {0, 0, 0}, {0x3000000u, 0, 0, 0}};
 d->phase2 = (RlFrameInput){0, 0, 0x60076000u, {0, 3, 0, 0x2007fe8u, 0, 0, 0, 0x2007fe8u}, {0, 0, 0}, {0x3000000u, 0, 0, 0}};
 d->cursor_x = d->input.cursor_x; d->cursor_y = d->input.cursor_y;
 const char *names[] = {"RISING.EXE", "RISING.INI", "RISING.RUL", "RISING.SBL", "TABLEAU.COL", "COULEUR.MAP", "PLATGAME.PIE", "MONSTRE.INI", "TEXTES.TXT", mission};
 unsigned count = mission ? 10 : 9;
 RlBloco inputs[10] = {0}; uint8_t *data[10] = {0};
 for (unsigned i = 0; i < count; i++) {
  data[i] = file(root, names[i], &n); inputs[i] = (RlBloco){0, n, data[i]};
  if (!data[i]) { snprintf(error, capacity, mission ? "Dados da missão ausentes: %s" : "Dados do menu ausentes: %s", names[i]); c = 1; break; }
 }
 RlMissaoInicio config; base_config(d, &config);
 if (!c && mission) c = rl_missao_iniciar_eventos_v1(inputs, 10, (const uint8_t *)mission, (uint32_t)strlen(mission), &config, &d->events_host, &d->world, &d->input);
 if (!c && !mission) c = rl_missao_iniciar_menu_eventos_v1(inputs, 9, &config, &d->events_host, &d->world, &d->input);
 for (unsigned i = 0; i < count; i++) free(data[i]);
 if (c) { if (!error[0]) { fail(d, c, mission ? "MissionLoad 0x46c2cf" : "MenuInit 0x4682bb"); snprintf(error, capacity, "%s", d->status.message); } goto bad; }
 if (mission) {
  c = rl_aplicar_dificuldade(d->world, difficulty);
  if (c) { fail(d, c, "Dificuldade"); snprintf(error, capacity, "%s", d->status.message); goto bad; }
  /* load_events(): explicit host reassociation, then the FLC table. */
  c = rl_eventos_reassociar_v1(d->world, &d->events_host);
  if (!c) c = event_flc_declare(d, d->world);
  if (c) { fail(d, c, "EVENTS.INI"); snprintf(error, capacity, "%s", d->status.message); goto bad; }
 } else {
  d->cursor_x = d->input.cursor_x; d->cursor_y = d->input.cursor_y; d->menu_world = 1;
 }
 /* Renderer from the six CMP banks and PLATGAME.PIE; gameplay background seed
  * (the menu world decodes the seed but has no renderer). */
 if (mission) c = new_renderer(d, d->world, NULL, &d->renderer);
 else c = game_background(d, NULL, 0);
 if (c) { if (!error[0]) { fail(d, c, "renderer create"); snprintf(error, capacity, "%s", d->status.message); } goto bad; }
 c = rl_mundo_paleta_jogo(d->world, d->palette, 768);
 if (c) { fail(d, c, "paleta do jogo"); snprintf(error, capacity, "%s", d->status.message); goto bad; }
 if (!mission) {
  c = shell_open(d, RL_SHELL_MAIN);
  if (c) { snprintf(error, capacity, "%s", d->save_error[0] ? d->save_error : "Menu original indisponível"); goto bad; }
 }
 if (mission && (c = refresh_screen(d))) { snprintf(error, capacity, "%s", d->status.message); goto bad; }
 return d;
bad:
 if (!error[0]) snprintf(error, capacity, "Invalid native initialization inputs");
 rlw_driver_free(d); return NULL;
}

RlwDriver *rlw_driver_create(const char *root, const char *mission, uint32_t difficulty, char *error, size_t capacity) {
 if (!mission) { if (error && capacity) snprintf(error, capacity, "Contrato explícito de missão inválido"); return NULL; }
 return create(root, mission, difficulty, error, capacity);
}
RlwDriver *rlw_driver_create_menu(const char *root, uint32_t difficulty, char *error, size_t capacity) {
 return create(root, NULL, difficulty, error, capacity);
}

void rlw_driver_free(RlwDriver *d) {
 if (!d) return;
 if (d->shell) rl_shell_free(d->shell);
 if (d->renderer) rl_render_liberar(d->renderer);
 if (d->world) rl_liberar_mundo(d->world);
 free(d->root); free(d->events_directory); free(d->events_bytes); free(d->pause_presentation); free(d);
}

static int check_render(RlwDriver *d, const RlRenderResult *out, uint64_t generation) {
 if (out->abi_status || out->generation != generation || out->width != 800 || out->height != 600 || out->stride != 800 ||
     out->required_pixels != 480000 || out->required_palette != 768 || out->palette_generation != 1 || out->disposition > 2)
  return named_stop(d, "NativeRenderResultContractMismatch");
 d->status.generation = generation;
 if (out->disposition == 2) {
  d->status.stopped = 1;
  snprintf(d->status.message, sizeof(d->status.message), "%.48s at 0x%08x: %.192s", out->stop_callee, out->stop_address, out->stop_reason);
  return 5;
 }
 return 0;
}

/* first_game_draw(): drawing only, no extra phase/tick/input/RNG call. */
static int first_game_draw(RlwDriver *d) {
 uint64_t generation = 0; uint32_t phase = 0, frame = 0; RlRenderResult out; memset(&out, 0, sizeof(out));
 int c = rl_render_geracao(d->world, &generation, &phase, &frame);
 if (!c) c = rl_render_quadro(d->renderer, generation, 1, d->pixels, sizeof(d->pixels), d->palette, sizeof(d->palette), &out);
 if (c) return fail(d, c, "primeiro desenho do jogo");
 return check_render(d, &out, generation);
}

/* macos_live_input_phase() */
static int input_phase(RlwDriver *d, uint32_t phase, const RlDeclaredInput *declared) {
 if (d->status.stopped || d->phase_index == UINT32_MAX || phase != (d->input_next_phase ? d->input_next_phase : 2)) return 1;
 int c = rl_passo_quadro_entrada(d->world, phase, &d->input, declared);
 if (c) return fail(d, c, phase == 2 ? "entrada entre quadros" : "entrada CalculGame");
 if (phase == 2) { d->cursor_x = d->input.cursor_x; d->cursor_y = d->input.cursor_y; }
 d->phase_index = (uint32_t)(2 * d->status.frames + (phase == 2 ? 1 : 2));
 d->input_next_phase = phase == 2 ? 3 : 2;
 if (phase == 3) d->status.frames++;
 return 0;
}

/* macos_live_human_phase(): phase 2 pumps the messages due by this frame and
 * samples the retained host cursor; phase 3 answers the key table only. */
static int human_phase(RlwDriver *d, uint32_t phase, RlDeclaredInput *frame_input) {
 if (phase == 3) {
  frame_input->flags = 6; frame_input->messages = NULL; frame_input->message_count = 0;
  return input_phase(d, 3, frame_input);
 }
 if (d->input_next_phase == 3) return 1;
 uint32_t count = 0, nkeys = 0, keep = 0, flags = 7; uint64_t frame = d->status.frames + 1;
 for (uint32_t i = 0; i < d->pending_count; i++) {
  QueuedInput q = d->pending[i];
  if (q.frame <= frame) { if (q.cursor) { d->cursor_x = q.x; d->cursor_y = q.y; flags |= 1; } d->frame_messages[count++] = q.message; }
  else d->pending[keep++] = q;
 }
 d->pending_count = keep;
 for (uint32_t vk = 0; vk < 256; vk++)
  if (d->held[vk] && (vk < 48 || vk > 57) && vk != 9) d->frame_keys[nkeys++] = (RlInputKey){vk, d->held[vk]};
 *frame_input = (RlDeclaredInput){sizeof(RlDeclaredInput), flags, d->cursor_x, d->cursor_y, d->frame_messages, count, d->frame_keys, nkeys};
 return input_phase(d, 2, frame_input);
}

/* In-game FLC events (event-flc.md). The milestone has no FLC player yet: once
 * an event is running and modal, report "last frame shown" at once, which is
 * the same ABI call the macOS host makes when its picture ends. */
static int flc_skip(RlwDriver *d) {
 RlEventFlcStateV1 s; memset(&s, 0, sizeof(s));
 int c = rl_evento_flc_estado_v1(d->world, &s); if (c) return fail(d, c, "estado do evento FLC");
 if (s.running && s.modal) { c = rl_evento_flc_terminar_v1(d->world); if (c) return fail(d, c, "fim do evento FLC"); d->status.flc_skipped++; }
 return 0;
}

/* macos_live_advance(), play mode. */
static int advance_step(RlwDriver *d) {
 uint32_t screen = 0; int c = rl_missao_tela(d->world, &screen); if (c) return fail(d, c, "tela da campanha");
 if (screen == 19 || screen == 16) return 0;
 if (d->phase_index == UINT32_MAX) {
  RlInputKey keys[256]; uint32_t count = 0;
  for (uint32_t vk = 0; vk < 256; vk++) if (d->held[vk]) keys[count++] = (RlInputKey){vk, d->held[vk]};
  /* No message pump or cursor sample before the original phase2 boundary. */
  RlDeclaredInput declared = {sizeof(declared), 6, 0, 0, NULL, 0, keys, count};
  c = rl_load_tail_entrada_v1(d->world, &d->input, &declared); if (c) return fail(d, c, "load_tail");
  d->input.registers = d->phase2.registers;
  if ((c = first_game_draw(d))) return c;
  c = rl_autosalvar_inicio_fase(d->world, &d->input); if (c) return fail(d, c, "captura automática no início da fase");
  d->phase_index = 0; d->started = 1;
  return 0;
 }
 uint64_t gen = 0; uint32_t phase = 0, counter = 0;
 c = rl_render_geracao(d->world, &gen, &phase, &counter); if (c) return fail(d, c, "render generation");
 if (phase != 0) {
  RlRenderResult out; memset(&out, 0, sizeof(out));
  c = rl_render_quadro(d->renderer, gen, 1, d->pixels, sizeof(d->pixels), d->palette, sizeof(d->palette), &out);
  if (c) return fail(d, c, "Rust render");
  if ((c = check_render(d, &out, gen))) return c;
 }
 uint32_t pending = 0; c = rl_entrada_quantidade_pendente(d->world, &pending); if (c) return fail(d, c, "consulta de entradas salvas");
 if (pending) return named_stop(d, "SavedInputUnsupportedInMilestone");
 RlDeclaredInput frame_input;
 if ((c = human_phase(d, 2, &frame_input))) return c;
 if ((c = human_phase(d, 3, &frame_input))) return c;
 return flc_skip(d);
}

/* status.screen is the live screen after the step, as macos_live_campaign_screen
 * reports it (a pre-step read lags one tick: 0 instead of 18 after the load tail). */
static int refresh_screen(RlwDriver *d) {
 uint32_t screen = 0; int c = rl_missao_tela(d->world, &screen); if (c) return fail(d, c, "tela da campanha");
 d->status.screen = screen; return 0;
}

int rlw_driver_advance(RlwDriver *d) {
 if (!d || d->status.stopped || d->shell) return 1;
 int c = advance_step(d);
 return c ? c : refresh_screen(d);
}

/* macos_live_event(): identical mapping, frame stamps and click gesture. */
int rlw_driver_event(RlwDriver *d, uint32_t kind, uint32_t msg, int32_t x, int32_t y, uint32_t wp, uint32_t vk, uint32_t down) {
 if (!d || d->status.stopped) return 1;
 uint64_t frame = d->status.frames + 1;
 if (kind == 1) {
  if (msg < 0x200 || msg > 0x205 || x < 0 || x >= 800 || y < 0 || y >= 600) return named_stop(d, "InvalidWindowsMouseEvent");
  if (msg == 0x205 || (msg == 0x202 && !d->left_held)) return 0;
  /* A left press is held across frames (MK_LBUTTON drag) until the actual
   * release, never earlier than the fixed k/k+1/k+3 click gesture. Right clicks
   * keep the fixed gesture and ignore the host release. */
  int left = msg == 0x201 || msg == 0x203, release = msg == 0x202;
  uint32_t count = msg == 0x200 || release ? 1 : left ? 2 : 3;
  if (left && d->left_held) {
   int32_t c = rlw_driver_event(d, 1, 0x202, d->left_x, d->left_y, 0, 0, 0); if (c) return c;
   frame = d->status.frames + 1;
  }
  if (d->pending_count + count > 4096) return named_stop(d, "WindowsInputQueueCapacityExceeded");
  if (frame <= d->mouse_until) frame = d->mouse_until + 1;
  uint32_t lp = ((uint32_t)y << 16) | (uint32_t)x, button = d->left_held ? 1u : 0u;
  if (release) {
   if (frame < d->left_down_frame + 2) frame = d->left_down_frame + 2;
   d->pending[d->pending_count++] = (QueuedInput){frame, x, y, {0x202, wp & ~3u, lp}, 1};
   d->mouse_until = frame; d->left_held = 0; d->left_x = x; d->left_y = y;
  } else {
   d->pending[d->pending_count++] = (QueuedInput){frame, x, y, {0x200, (wp & ~3u) | button, lp}, 1};
   if (left) {
    d->pending[d->pending_count++] = (QueuedInput){frame + 1, x, y, {msg, (wp & ~3u) | 1u, lp}, 0};
    d->mouse_until = frame + 1; d->left_down_frame = frame + 1; d->left_held = 1; d->left_x = x; d->left_y = y;
   } else if (count == 3) {
    d->pending[d->pending_count++] = (QueuedInput){frame + 1, x, y, {msg, (wp & ~3u) | 2u | button, lp}, 0};
    d->pending[d->pending_count++] = (QueuedInput){frame + 3, x, y, {0x205, (wp & ~3u) | button, lp}, 0};
    d->mouse_until = frame + 3;
   } else if (button) { d->left_x = x; d->left_y = y; }
  }
 } else if (kind == 2) {
  /* Exactly the 23 accepted virtual keys and their original scan codes. Tab (9)
   * and the digits are WndProc-only (never in the polled key table). */
  static const uint32_t keys[] = {13, 16, 17, 18, 27, 32, 37, 38, 39, 40, 46, 88, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 9};
  static const uint32_t scans[] = {0x1c, 0x2a, 0x1d, 0x38, 1, 0x39, 0x4b, 0x48, 0x4d, 0x50, 0x53, 0x2d, 0x0b, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0x0f};
  unsigned i = 0; for (; i < 23 && keys[i] != vk; i++) {}
  if (i == 23 || down > 1) return named_stop(d, "UnsupportedWindowsVirtualKey");
  if (d->pending_count == 4096) return named_stop(d, "WindowsInputQueueCapacityExceeded");
  uint32_t lp = 1 | (scans[i] << 16) | ((uint32_t)(vk == 37 || vk == 38 || vk == 39 || vk == 40 || vk == 46) << 24) |
                ((uint32_t)(d->held[18] != 0) << 29) | ((uint32_t)(d->held[vk] != 0) << 30) | ((uint32_t)!down << 31);
  d->pending[d->pending_count++] = (QueuedInput){frame, 0, 0, {down ? 0x100u : 0x101u, vk, lp}, 0};
  d->held[vk] = down ? -32768 : 0;
 } else return named_stop(d, "UnsupportedWindowsInputEvent");
 return 0;
}

int rlw_driver_left_held(const RlwDriver *d, int32_t *x, int32_t *y) {
 if (!d || !d->left_held) return 0;
 if (x) *x = d->left_x;
 if (y) *y = d->left_y;
 return 1;
}
int rlw_driver_key_held(const RlwDriver *d, uint32_t vk) { return d && vk < 256 && d->held[vk] != 0; }
void rlw_driver_status(const RlwDriver *d, RlwStatus *out) { if (d && out) *out = d->status; }
const uint8_t *rlw_driver_pixels(const RlwDriver *d) { return d ? d->pixels : NULL; }
const uint8_t *rlw_driver_palette(const RlwDriver *d) { return d ? d->palette : NULL; }

#include "rlw_shell.inc"
