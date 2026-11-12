/* Prepared-shell consumer and campaign continuation (milestone 2), included by
 * rlw_driver.c. Port of macos/src/macos_campaign_shell.inc
 * and of macos_live_driver.c's campaign_byte/next_path/campaign_end/
 * continue_stage(prepared): same engine calls, order, addresses and VFS paths.
 * No game rules live here; every comment names the macOS function mirrored.
 * Saves (lists 12/13/14) show the host captions; the modern save/load itself
 * is milestone 3, so SAVE_SLOT resumes without writing (rlw_shell_save_slot). */

static uint32_t le32(const uint8_t *p) { return p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24); }

/* save_result(): the separate host error (no simulation stop). */
static int save_result(RlwDriver *d, int code, const char *action) {
 d->save_error[0] = 0; if (!code) return 0;
 uint32_t n = 0; char detail[360] = {0}; rl_erro_mensagem(NULL, 0, &n);
 if (n && n <= sizeof(detail)) rl_erro_mensagem((uint8_t *)detail, n, &n);
 snprintf(d->save_error, sizeof(d->save_error), "%s · ABI %d: %.360s", action, code, detail); return code;
}

/* campaign_byte(): one byte of a projection (rl_projetar_tudo). */
static int campaign_byte(const uint8_t *data, uint32_t n, uint32_t address, uint8_t *out) {
 if (n < 4) return RL_SAVE_INVALID; uint32_t off = 4;
 for (uint32_t i = 0; i < le32(data); i++) {
  if ((uint64_t)off + 8 > n) return RL_SAVE_INVALID; uint32_t base = le32(data + off), len = le32(data + off + 4); off += 8;
  if ((uint64_t)off + len > n) return RL_SAVE_INVALID;
  if (address >= base && (uint64_t)address < (uint64_t)base + len) { *out = data[off + address - base]; return 0; } off += len;
 }
 return RL_MEMORY_ERROR;
}
static int next_path(RlwDriver *d, uint8_t *path, uint32_t *size, char *name) {
 int c = rl_missao_nome_proxima(d->world, path, 256, size); if (c) return c;
 if (!*size || *size >= 256) return RL_INVALID_ARGUMENT; path[*size] = 0;
 const char *base = (const char *)path; for (uint32_t i = 0; i < *size; i++) if (path[i] == '/' || path[i] == 92) base = (const char *)path + i + 1;
 if (strlen(base) < 11 || strlen(base) > 12) return RL_INVALID_ARGUMENT;
 snprintf(name, 16, "%s", base); for (unsigned i = 0; name[i]; i++) if (name[i] >= 'a' && name[i] <= 'z') name[i] -= 'a' - 'A';
 if (strncmp(name, "MONDE", 5) || strcmp(name + strlen(name) - 4, ".ETA")) return RL_INVALID_ARGUMENT;
 return 0;
}
/* macos_live_campaign_end(): read once at the result screen, never per frame. */
int rlw_campaign_end(RlwDriver *d, RlwCampaignEnd *out) {
 if (!d || !out) return RL_INVALID_ARGUMENT;
 uint32_t n = 0; int c = rl_projetar_tudo(d->world, NULL, 0, &n); if (c != RL_BUFFER_TOO_SMALL) return save_result(d, c ? c : RL_SAVE_INVALID, "Resultado da fase");
 uint8_t *data = malloc(n); if (!data) return save_result(d, RL_IO_ERROR, "Resultado da fase");
 c = rl_projetar_tudo(d->world, data, n, &n); uint8_t end = 0, victory = 0, stage = 0;
 if (!c) c = campaign_byte(data, n, 0x4e05a0, &end);
 if (!c) c = campaign_byte(data, n, 0x4e1d84, &victory);
 if (!c) c = campaign_byte(data, n, 0x496fe4, &stage); free(data);
 if (c) return save_result(d, c, "Resultado da fase");
 RlwCampaignEnd result; memset(&result, 0, sizeof(result)); result.stage = stage; result.outcome = end ? (victory ? 1 : 2) : 0;
 if (result.outcome == 1) {
  uint8_t path[257]; uint32_t length = 0; c = next_path(d, path, &length, result.next);
  if (c) return save_result(d, c, "Próxima fase");
  uint32_t bytes = 0; uint8_t *eta = file(d->root, result.next, &bytes); result.has_next = eta != NULL; free(eta);
  if (!result.has_next && stage != 25) { snprintf(d->save_error, sizeof(d->save_error), "Arquivo da próxima fase ausente: %s", result.next); return RL_IO_ERROR; }
 }
 *out = result; return 0;
}

static int shell_copy_picture(RlwDriver *d) {
 uint8_t *p = malloc(480000 + 768); if (!p) return RL_IO_ERROR;
 int c = rl_shell_copy(d->shell, d->shell_generation, p, 480000, p + 480000, 768);
 if (!c) { memcpy(d->pixels, p, 480000); memcpy(d->palette, p + 480000, 768); } free(p); return c;
}
static int shell_paint(RlwDriver *d) {
 RlShellPlan plan; memset(&plan, 0, sizeof(plan)); int c = rl_shell_plan(d->shell, d->shell_mode, &plan); if (c) return c;
 /* The background is decoded from the CD's own file (imported unchanged). */
 const char *name = NULL;
 if (!strcmp((char *)plan.background, "\\ecranfin.lzs")) name = "ECRANFIN.LZS";
 else if (!strcmp((char *)plan.background, "\\pagemis2.pie")) name = "PAGEMIS2.PIE";
 else if (!strcmp((char *)plan.background, "\\pagemis.pie")) name = "PAGEMIS.PIE";
 else if (!strcmp((char *)plan.background, "\\titre.pie")) name = "TITRE.PIE";
 else if (!strcmp((char *)plan.background, "\\credits.pie")) name = "CREDITS.PIE";
 if (!name) return RL_UNSUPPORTED_PATH;
 uint32_t fn = 0, n = 480000; uint8_t *f = file(d->root, name, &fn), *p = malloc(480000);
 c = f && p ? rl_shell_imagem_cd(f, fn, p, 480000) : RL_IO_ERROR; free(f);
 if (c) { free(p); snprintf(d->save_error, sizeof(d->save_error), "Fundo original ausente ou inválido: %s", name); return RL_IO_ERROR; }
 c = rl_shell_paint_prepared(d->shell, d->shell_generation, (RlShellBytes){p, n}, &d->shell_generation); free(p);
 return c ? c : shell_copy_picture(d);
}
static int shell_world_bytes(RlwDriver *d, uint8_t **out, uint32_t *n) {
 int c = rl_projetar_tudo(d->world, NULL, 0, n); if (c != RL_BUFFER_TOO_SMALL) return c ? c : RL_SAVE_INVALID;
 *out = malloc(*n); if (!*out) return RL_IO_ERROR;
 c = rl_projetar_tudo(d->world, *out, *n, n); if (c) { free(*out); *out = NULL; } return c;
}
static int shell_word(const uint8_t *b, uint32_t n, uint32_t address, uint32_t size, uint32_t *out) {
 uint32_t value = 0; for (uint32_t i = 0; i < size; i++) { uint8_t v = 0; int c = campaign_byte(b, n, address + i, &v); if (c) return c; value |= (uint32_t)v << (8 * i); } *out = value; return 0;
}
static int shell_string(const uint8_t *b, uint32_t n, uint32_t address, uint8_t *out, uint32_t cap, uint32_t *length) {
 for (uint32_t i = 0; i < cap; i++) { int c = campaign_byte(b, n, address + i, &out[i]); if (c) return c; if (!out[i]) { *length = i; return 0; } } return RL_INVALID_ARGUMENT;
}
uint32_t rlw_shell_mode(const RlwDriver *d) { return d ? d->shell_mode : 0; }
int rlw_shell_state(const RlwDriver *d, RlShellInput *out) {
 if (!d || !d->shell || !out) return RL_INVALID_ARGUMENT; *out = d->shell_input; out->keys = 0; out->mouse_b0 = 0; return 0;
}
/* macos_live_shell_begin(): result19, or pause16 entered in game by the original
 * InputGame Escape (MenuInit 16 already ran in the world; the session paints titre.pie). */
int rlw_shell_begin(RlwDriver *d) {
 if (!d || d->shell || d->status.stopped) return RL_INVALID_ARGUMENT;
 uint32_t mode = 0; int c = rl_missao_tela(d->world, &mode); if (c || (mode != 19 && mode != RL_SHELL_PAUSE)) return c ? c : RL_INVALID_ARGUMENT;
 if (mode == RL_SHELL_PAUSE) {
  /* Keep the gameplay surface/cache: resume rebuilds the renderer from it. */
  free(d->pause_presentation); d->pause_presentation = malloc(PRESENTATION_SIZE);
  if (!d->pause_presentation) return save_result(d, RL_IO_ERROR, "Menu de pausa original");
  c = capture_presentation(d, d->pause_presentation);
  if (c) { free(d->pause_presentation); d->pause_presentation = NULL; return save_result(d, c, "Menu de pausa original"); }
  memcpy(d->pause_palette, d->palette, 768);
 }
 c = shell_open(d, mode); return c ? save_result(d, c, mode == 19 ? "Tela original de resultado" : "Menu de pausa original") : 0;
}
/* shell_open(): prepared session over the current world (result19, pause16 or
 * the menu world's main2). Assets: six CMP banks, PLATGAME.PIE, the shadow and
 * lighting tables computed by the world from the CD palette, MENU and INVENTE. */
static int shell_open(RlwDriver *d, uint32_t mode) {
 RlShellBytes assets[11]; uint8_t *owned[11] = {0}; uint32_t n = 0; char name[40]; RlShell shell = 0; int c = 0;
 memset(assets, 0, sizeof(assets));
 for (unsigned i = 0; i < 11; i++) {
  if (i == 7 || i == 8) {
   if (i == 7) { owned[7] = malloc(256); owned[8] = malloc(256); if (!owned[7] || !owned[8]) { c = RL_IO_ERROR; goto done; }
    c = rl_mundo_tabelas_sombra_luz(d->world, owned[7], owned[8]); if (c) goto done; }
   assets[i] = (RlShellBytes){owned[i], 256}; continue;
  }
  if (i < 9) snprintf(name, sizeof(name), "asset%u.bin", i); else snprintf(name, sizeof(name), "%s", i == 9 ? "MENU.CMP" : "INVENTE.CMP");
  owned[i] = file(d->root, name, &n); assets[i] = (RlShellBytes){owned[i], n}; if (!owned[i]) { c = RL_IO_ERROR; goto done; }
 }
 c = rl_shell_create(1, d->world, assets, 11, (RlShellBytes){d->pixels, 480000}, &shell);
 if (!c) {
  uint8_t *b = NULL; uint32_t size = 0; RlShellInput input; memset(&input, 0, sizeof(input)); input.version = 1;
  c = shell_world_bytes(d, &b, &size);
  if (!c) c = shell_word(b, size, 0x508f7c, 4, &input.clock_7c);
  if (!c) c = shell_word(b, size, 0x508f84, 4, &input.animation_84);
  if (!c) c = shell_word(b, size, 0x5099a8, 2, &input.mouse_x);
  if (!c) c = shell_word(b, size, 0x5099aa, 2, &input.mouse_y); free(b);
  if (!c) { d->shell = shell; d->shell_mode = mode; d->shell_generation = 0; d->shell_input = input; memset(&d->shell_request, 0, sizeof(d->shell_request)); c = shell_paint(d); }
 }
done:
 for (unsigned i = 0; i < 11; i++) free(owned[i]);
 if (c) { if (shell) rl_shell_free(shell); d->shell = 0; d->shell_mode = 0; } return c;
}
/* shell_text(): an absent file is absence (no entry); an empty file is valid;
 * any other host failure is an error. */
static int shell_text(RlwDriver *d, const char *name, uint8_t **out, uint32_t *size) {
 char path[4096]; if (snprintf(path, sizeof(path), "%s" RLW_SEP "%s", d->root, name) >= (int)sizeof(path)) return RL_IO_ERROR;
 *out = NULL; *size = 0;
 if (!rlw_exists(path)) return 0;
 FILE *f = rlw_fopen(path, "rb"); if (!f) return RL_IO_ERROR;
 int64_t n = rlw_file_size(f); fclose(f);
 if (n < 0 || n > 16 * 1024 * 1024) return RL_IO_ERROR;
 if (n == 0) { *out = malloc(1); if (!*out) return RL_IO_ERROR; return 0; }
 uint32_t got = 0; uint8_t *p = rlw_read_file(path, &got, 16u * 1024u * 1024u);
 if (!p || got != (uint32_t)n) { free(p); return RL_IO_ERROR; }
 *out = p; *size = got; return 0;
}
/* briefing_file(): original briefing text for stage `number` at its exact
 * CD-root path: root0x509594 + template0x498430 with the stage digits. */
static int briefing_file(RlwDriver *d, unsigned number, uint8_t *path, uint32_t *pn, uint8_t **text, uint32_t *tn) {
 uint8_t *world = NULL, relative[20]; uint32_t wn = 0, rn = 0; *text = NULL; *tn = 0;
 int c = shell_world_bytes(d, &world, &wn);
 if (!c) c = shell_string(world, wn, 0x509594, path, 256, pn);
 if (!c) c = shell_string(world, wn, 0x498430, relative, 19, &rn); free(world);
 if (!c && (rn != 18 || memcmp(relative, "\\World\\monde", 12) || memcmp(relative + 14, ".TXT", 4))) c = RL_INVALID_ARGUMENT;
 if (c) return c;
 relative[12] = (uint8_t)('0' + number / 10); relative[13] = (uint8_t)('0' + number % 10); memcpy(path + *pn, relative, rn); *pn += rn;
 char name[16]; snprintf(name, sizeof(name), "MONDE%02u.TXT", number % 100);
 c = shell_text(d, name, text, tn);
 if (c) snprintf(d->save_error, sizeof(d->save_error), "Falha ao ler o briefing original %s", name);
 return c;
}
/* new_campaign_path(): exact MakeMissionName path under the CD root (0x509594 +
 * 0x494b27 with the stage digits + the variant string behind 0x497154 +
 * 0x494b36): stage 1 for "Novo jogo"; the current stage and variant for restart. */
static int new_campaign_path(RlwDriver *d, uint8_t *path, uint32_t *size, char *name) {
 unsigned number = (unsigned)d->new_campaign; if (number < 1 || number > 25) return RL_INVALID_ARGUMENT;
 int variant = number != 1 && strlen(d->mission) == 12 ? d->mission[7] - 'A' : -1;
 uint8_t *world = NULL, prefix[32], suffix[16], extra[16]; uint32_t wn = 0, rn = 0, pn = 0, sn = 0, en = 0, pointer = 0;
 int c = shell_world_bytes(d, &world, &wn);
 if (!c) c = shell_string(world, wn, 0x509594, path, 200, &rn);
 if (!c) c = shell_string(world, wn, 0x494b27, prefix, 31, &pn);
 if (!c) c = shell_string(world, wn, 0x494b36, suffix, 15, &sn);
 if (!c && variant >= 0) { if (variant > 3) c = RL_INVALID_ARGUMENT; if (!c) c = shell_word(world, wn, 0x497154 + 4 * (unsigned)variant, 4, &pointer); if (!c) c = shell_string(world, wn, pointer, extra, 15, &en); }
 free(world);
 if (!c && pn != 14) c = RL_INVALID_ARGUMENT;
 if (c) return c;
 prefix[12] = (uint8_t)('0' + number / 10); prefix[13] = (uint8_t)('0' + number % 10);
 memcpy(path + rn, prefix, pn); memcpy(path + rn + pn, extra, en); memcpy(path + rn + pn + en, suffix, sn); *size = rn + pn + en + sn; path[*size] = 0;
 const char *base = (const char *)path; for (uint32_t i = 0; i < *size; i++) if (path[i] == '/' || path[i] == 92) base = (const char *)path + i + 1;
 if (strlen(base) < 11 || strlen(base) > 12) return RL_INVALID_ARGUMENT;
 snprintf(name, 16, "%s", base); for (unsigned i = 0; name[i]; i++) if (name[i] >= 'a' && name[i] <= 'z') name[i] -= 'a' - 'A';
 return strcmp(name, number == 1 ? "MONDE01.ETA" : d->mission) ? RL_INVALID_ARGUMENT : 0;
}
int rlw_shell_input(RlwDriver *d, const RlShellInput *input, RlShellEvent *out) {
 if (!d || !d->shell || !input || !out || d->shell_request.kind) return RL_INVALID_ARGUMENT;
 RlShellEvent event; memset(&event, 0, sizeof(event)); int c = rl_shell_input(d->shell, d->shell_generation, input, &event);
 if (c) return save_result(d, c, "Entrada na tela original");
 d->shell_generation = event.generation; d->shell_input = *input; d->shell_request = event;
 *out = event; return 0;
}
int rlw_shell_draw(RlwDriver *d, uint32_t animation) {
 if (!d || !d->shell || d->shell_request.kind) return RL_INVALID_ARGUMENT;
 int c = rl_shell_draw(d->shell, d->shell_generation, animation, &d->shell_generation);
 if (!c) { d->shell_input.animation_84 = animation; c = shell_copy_picture(d); } return c;
}
static int stage_number(RlwDriver *d, unsigned *number) {
 uint8_t *world = NULL; uint32_t wn = 0, value = 0; int c = shell_world_bytes(d, &world, &wn);
 if (!c) c = shell_word(world, wn, 0x496fe4, 1, &value); free(world);
 if (!c && (value < 1 || value > 25)) c = RL_INVALID_ARGUMENT; if (!c) *number = value; return c;
}
/* macos_live_shell_prepare(): MISSION_NEXT on a victorious result19. */
int rlw_shell_prepare(RlwDriver *d) {
 if (!d || !d->shell || d->shell_request.kind != RL_SHELL_MISSION_NEXT) return RL_INVALID_ARGUMENT;
 uint8_t path[257], text_path[512], relative[20], *world = NULL; uint32_t length = 0, wn = 0, pn = 0, rn = 0; char name[16];
 int c = next_path(d, path, &length, name); if (c) return save_result(d, c, "Próxima fase");
 uint32_t en = 0, tn = 0; uint8_t *eta = file(d->root, name, &en); char text_name[16]; snprintf(text_name, sizeof(text_name), "MONDE%c%c.TXT", name[5], name[6]);
 /* Packaged original CD files mapped to the retained CD-root namespace. */
 uint8_t *text = NULL; c = shell_text(d, text_name, &text, &tn);
 /* After the last stage MissionNext finds no next ETA: campaign end (mode24). */
 unsigned stage = 0; int final = !eta && !c && !stage_number(d, &stage) && stage == 25;
 if ((!eta && !final) || c) { free(eta); free(text); snprintf(d->save_error, sizeof(d->save_error), "Falha ao ler dados originais do briefing: %s / %s", name, text_name); return RL_IO_ERROR; }
 c = shell_world_bytes(d, &world, &wn);
 if (!c) c = shell_string(world, wn, 0x509594, text_path, 256, &pn);
 if (!c) c = shell_string(world, wn, 0x498430, relative, 19, &rn); free(world);
 if (!c && (rn != 18 || memcmp(relative, "\\World\\monde", 12) || memcmp(relative + 14, ".TXT", 4))) c = RL_INVALID_ARGUMENT;
 if (!c) {
  relative[12] = (uint8_t)name[5]; relative[13] = (uint8_t)name[6]; memcpy(text_path + pn, relative, rn); pn += rn;
  RlShellFile files[2] = {{{path, length}, {eta, en}}, {{text_path, pn}, {text, tn}}};
  const RlShellFile *vfs = final ? files + 1 : files; uint32_t count = final ? (text ? 1 : 0) : (text ? 2 : 1);
  c = rl_shell_campaign_prepare(d->shell, d->shell_generation, (RlShellBytes){path, length}, vfs, count, &d->shell_generation);
  if (!c) { d->shell_mode = final ? RL_SHELL_FINAL : 17; memset(&d->shell_request, 0, sizeof(d->shell_request)); c = shell_paint(d); }
 }
 free(eta); free(text); return c ? save_result(d, c, "Preparação do briefing original") : 0;
}

/* continue_stage(prepared): START_MISSION from a real briefing session. The new
 * world, renderer and first picture are built on a candidate; the current
 * driver changes only when everything succeeded. */
static int continue_stage(RlwDriver *d, uint32_t difficulty) {
 if (!d || !d->shell || difficulty > 2) return RL_INVALID_ARGUMENT;
 if (d->status.stopped) { snprintf(d->save_error, sizeof(d->save_error), "A parada atual do motor precisa ser resolvida antes de continuar."); return RL_UNSUPPORTED_PATH; }
 uint8_t path[257]; uint32_t length = 0; char name[16]; int c = 0;
 /* A completed "Novo jogo" (or restart) loads its stage; every other
  * continuation requires an actual victory with an available next stage. */
 if (d->new_campaign) { c = new_campaign_path(d, path, &length, name); if (c) return save_result(d, c, "Nova campanha"); }
 else {
  RlwCampaignEnd end; c = rlw_campaign_end(d, &end); if (c) return c;
  if (end.outcome != 1 || !end.has_next) { snprintf(d->save_error, sizeof(d->save_error), "Não há vitória com uma próxima fase disponível."); return RL_UNSUPPORTED_PATH; }
  uint32_t queued = 0; c = rl_entrada_quantidade_pendente(d->world, &queued);
  if (c) return save_result(d, c, "Entradas da fase anterior");
  if (queued) { snprintf(d->save_error, sizeof(d->save_error), "Há entradas salvas pendentes na fase anterior."); return RL_SAVE_BUSY; }
  c = next_path(d, path, &length, name); if (c) return save_result(d, c, "Próxima fase");
 }
 uint32_t eta_length = 0, monsters_length = 0; uint8_t *eta = file(d->root, name, &eta_length), *monsters = file(d->root, "MONSTRE.INI", &monsters_length);
 if (!eta || !monsters) { free(eta); free(monsters); snprintf(d->save_error, sizeof(d->save_error), "Dados originais da próxima fase ausentes: %s / MONSTRE.INI", name); return RL_IO_ERROR; }
 RlwDriver *candidate = malloc(sizeof(*candidate)); if (!candidate) { free(eta); free(monsters); return save_result(d, RL_IO_ERROR, "Próxima fase"); }
 *candidate = *d; snprintf(candidate->mission, 16, "%s", name); candidate->difficulty = difficulty; candidate->world = 0; candidate->renderer = 0;
 candidate->menu_world = 0; candidate->new_campaign = 0; candidate->pause_presentation = NULL;
 candidate->shell = 0; candidate->shell_generation = 0; candidate->shell_mode = 0; memset(&candidate->shell_request, 0, sizeof(candidate->shell_request));
 candidate->save_error[0] = 0;
 memset(&candidate->status, 0, sizeof(candidate->status)); candidate->phase_index = UINT32_MAX; candidate->started = 0;
 candidate->pending_count = 0; candidate->input_next_phase = 2; candidate->mouse_until = 0; candidate->left_held = 0; candidate->left_down_frame = 0;
 RlFrameInput carried = d->input; carried.cursor_x = (int32_t)d->shell_input.mouse_x; carried.cursor_y = (int32_t)d->shell_input.mouse_y;
 candidate->cursor_x = carried.cursor_x; candidate->cursor_y = carried.cursor_y; memset(candidate->held, 0, sizeof(candidate->held));
 /* Load and stage-start preset in one engine transaction. */
 c = rl_shell_campaign_load_dificuldade(d->shell, d->shell_generation, (RlShellBytes){path, length}, (RlShellBytes){eta, eta_length}, (RlShellBytes){monsters, monsters_length}, &carried, difficulty, &candidate->world, &candidate->input);
 /* The menu world carries no play execution mode: set it before the load tail. */
 if (!c && d->menu_world) c = rl_execucao_definir(candidate->world, RL_EXECUCAO_JOGO);
 /* The continuation carries the last sampled original cursor. */
 if (!c) { candidate->cursor_x = candidate->input.cursor_x; candidate->cursor_y = candidate->input.cursor_y; }
 free(eta); free(monsters);
 /* load_events(candidate, inherited): explicit host reassociation, FLC table. */
 if (!c) c = rl_eventos_reassociar_v1(candidate->world, &candidate->events_host);
 if (!c) c = event_flc_declare(candidate, candidate->world);
 if (!c) c = new_renderer(candidate, candidate->world, NULL, &candidate->renderer);
 if (!c) c = advance_step(candidate);
 uint64_t generation = 0; uint32_t phase = 0, frame = 0; RlRenderResult rendered; memset(&rendered, 0, sizeof(rendered)); int render_error = 0;
 if (!c) c = rl_render_geracao(candidate->world, &generation, &phase, &frame);
 if (!c) c = rl_render_quadro(candidate->renderer, generation, 1, candidate->pixels, sizeof(candidate->pixels), candidate->palette, sizeof(candidate->palette), &rendered);
 if (!c && (rendered.generation != generation || rendered.width != 800 || rendered.height != 600 || rendered.stride != 800 || rendered.required_pixels != 480000 || rendered.required_palette != 768 || rendered.palette_generation != 1 || rendered.disposition > 2)) {
  snprintf(d->save_error, sizeof(d->save_error), "Contrato de renderização inválido na próxima fase."); c = RL_UNSUPPORTED_PATH; render_error = 1;
 }
 if (!c && (rendered.abi_status || rendered.disposition == 2)) {
  snprintf(d->save_error, sizeof(d->save_error), "Próxima fase: %.48s at 0x%08x: %.192s", rendered.stop_callee, rendered.stop_address, rendered.stop_reason); c = RL_UNSUPPORTED_PATH; render_error = 1;
 }
 if (!c) c = refresh_screen(candidate);
 if (c) {
  if (candidate->status.stopped) snprintf(d->save_error, sizeof(d->save_error), "%s", candidate->status.message);
  else if (!render_error) { if (candidate->save_error[0]) snprintf(d->save_error, sizeof(d->save_error), "%s", candidate->save_error); else save_result(d, c, "Não foi possível continuar a campanha"); }
  if (candidate->renderer) rl_render_liberar(candidate->renderer); if (candidate->world) rl_liberar_mundo(candidate->world); free(candidate); return c;
 }
 candidate->status.generation = generation;
 if (d->shell) rl_shell_free(d->shell);
 if (d->renderer) rl_render_liberar(d->renderer); rl_liberar_mundo(d->world); free(d->pause_presentation);
 *d = *candidate; free(candidate); d->save_error[0] = 0; return 0;
}
int rlw_shell_load(RlwDriver *d) {
 if (!d || !d->shell || d->shell_request.kind != RL_SHELL_START_MISSION) return RL_INVALID_ARGUMENT;
 return continue_stage(d, d->difficulty);
}
int rlw_shell_movie_finish(RlwDriver *d, uint32_t completion) {
 if (!d || !d->shell || (d->shell_request.kind != RL_SHELL_DEFEAT_MOVIE && d->shell_request.kind != RL_SHELL_CAMPAIGN_MOVIE && d->shell_request.kind != RL_SHELL_INTRODUCTION)) return RL_INVALID_ARGUMENT;
 int c = rl_shell_defeat_finish(d->shell, d->shell_generation, completion, &d->shell_generation);
 if (!c) {
  d->shell_mode = RL_SHELL_MAIN; memset(&d->shell_request, 0, sizeof(d->shell_request));
  d->shell_input.keys = 0; d->shell_input.mouse_ae = 0; d->shell_input.mouse_af = 0; d->shell_input.mouse_b0 = 0;
  c = shell_paint(d);
 }
 return c ? save_result(d, c, "Retorno original após o filme") : 0;
}
/* shell_entered(): every SHOW is completed by rl_shell_menu_init, "Novo jogo"
 * (RESET_CAMPAIGN 17) and "Carregar jogo" (13) by rl_shell_campaign_reset. */
static int shell_entered(RlwDriver *d, uint32_t mode) {
 d->shell_mode = mode; memset(&d->shell_request, 0, sizeof(d->shell_request));
 d->shell_input.keys = 0; d->shell_input.mouse_ae = 0; d->shell_input.mouse_af = 0; d->shell_input.mouse_b0 = 0;
 return shell_paint(d);
}
int rlw_shell_slots(RlwDriver *d, const uint8_t *table, uint32_t length) {
 if (!d || !table || length != sizeof(d->slots)) return RL_INVALID_ARGUMENT;
 for (unsigned i = 0; i < 10; i++) if (table[33 * i] > 1) return RL_INVALID_ARGUMENT;
 memcpy(d->slots, table, sizeof(d->slots)); return 0;
}
/* slot_files(): the ten exact paths stored behind 0x497128; a present host save
 * is "SAV3" + its 32-byte caption, an empty slot has no entry. */
typedef struct { RlShellFile files[10]; uint8_t paths[10][256], headers[10][36]; uint32_t count; } SlotFiles;
static int slot_files(RlwDriver *d, SlotFiles *out) {
 uint8_t *world = NULL; uint32_t wn = 0; int c = shell_world_bytes(d, &world, &wn); out->count = 0;
 for (unsigned i = 0; !c && i < 10; i++) {
  uint32_t pointer = 0, length = 0; c = shell_word(world, wn, 0x497128 + 4 * i, 4, &pointer);
  if (!c) c = shell_string(world, wn, pointer, out->paths[i], 255, &length);
  if (c || !d->slots[i][0]) continue;
  memcpy(out->headers[i], "SAV3", 4); memcpy(out->headers[i] + 4, d->slots[i] + 1, 32);
  out->files[out->count++] = (RlShellFile){{out->paths[i], length}, {out->headers[i], 36}};
 }
 free(world); return c;
}
static int is_list(uint32_t mode) { return mode == RL_SHELL_LOAD_LIST || mode == RL_SHELL_LOAD_MENU || mode == RL_SHELL_SAVE_LIST; }
/* shell_resume(): RESUME and SAVE_SLOT. Original in-game MenuInit(18)
 * (rl_shell_resume) returns the resumed world (mode18, same clock and frame);
 * its renderer is rebuilt from the gameplay presentation retained at pause entry. */
static int shell_resume(RlwDriver *d) {
 if (!d->pause_presentation) { snprintf(d->save_error, sizeof(d->save_error), "Retorno do menu de pausa sem a apresentação do jogo"); return RL_INVALID_ARGUMENT; }
 RlMundo world = 0; RlRenderer renderer = 0; RlFrameInput input; memset(&input, 0, sizeof(input));
 int c = rl_shell_resume(d->shell, d->shell_generation, &d->input, &world, &input);
 if (!c) c = rl_eventos_reassociar_v1(world, &d->events_host);
 if (!c) c = event_flc_declare(d, world);
 if (!c) c = new_renderer(d, world, d->pause_presentation, &renderer);
 if (c) { if (renderer) rl_render_liberar(renderer); if (world) rl_liberar_mundo(world); return save_result(d, c, "Retorno do menu de pausa"); }
 rl_shell_free(d->shell); d->shell = 0; d->shell_mode = 0; memset(&d->shell_request, 0, sizeof(d->shell_request));
 if (d->renderer) rl_render_liberar(d->renderer); rl_liberar_mundo(d->world);
 d->world = world; d->renderer = renderer; d->input = input;
 memcpy(d->pixels, d->pause_presentation, 480000); memcpy(d->palette, d->pause_palette, 768); free(d->pause_presentation); d->pause_presentation = NULL;
 /* Keys pressed before the pause were released inside the menu, outside the
  * engine's input; a stale held Escape would re-enter the pause at once. */
 memset(d->held, 0, sizeof(d->held)); d->pending_count = 0; d->left_held = 0;
 return refresh_screen(d);
}
/* SAVE_SLOT from the in-game list 14 (macos_live_shell_save_slot): the modern
 * save comes in milestone 3, so *saved reports it as unavailable; the original
 * final 0x42c570 (MenuInit 18 + HUD latch) still runs through rl_shell_resume. */
int rlw_shell_save_slot(RlwDriver *d, int32_t *saved) {
 if (!d || !d->shell || !saved || d->shell_request.kind != RL_SHELL_SAVE_SLOT || !d->pause_presentation) return RL_INVALID_ARGUMENT;
 *saved = RL_UNSUPPORTED_PATH;
 int c = shell_resume(d);
 if (!c) snprintf(d->save_error, sizeof(d->save_error), "Salvar partidas chega no próximo marco do cliente Windows; o jogo continua sem salvar.");
 return c;
}
/* Read-only view of the open session's memory (exported copy). */
int rlw_shell_peek(RlwDriver *d, uint32_t address, uint32_t size, uint8_t *out) {
 if (!d || !d->shell || !out || !size || size > 4096) return RL_INVALID_ARGUMENT;
 RlMundo world = 0; int c = rl_shell_export_world(d->shell, d->shell_generation, &world); if (c) return save_result(d, c, "Leitura da tela original");
 uint32_t n = 0; uint8_t *b = NULL; c = rl_projetar_tudo(world, NULL, 0, &n);
 if (c == RL_BUFFER_TOO_SMALL) { b = malloc(n); c = b ? rl_projetar_tudo(world, b, n, &n) : RL_IO_ERROR; } else if (!c) c = RL_SAVE_INVALID;
 for (uint32_t i = 0; !c && i < size; i++) c = campaign_byte(b, n, address + i, &out[i]);
 free(b); rl_liberar_mundo(world); return c ? save_result(d, c, "Leitura da tela original") : 0;
}
/* Mouse zone rectangle (left, top, right, bottom) of the current screen, from the
 * session's MouseZone table at 0x5099b4+16*zone (0x479cac). */
int rlw_shell_zone(RlwDriver *d, uint32_t zone, int32_t rect[4]) {
 if (!rect || zone > 255) return RL_INVALID_ARGUMENT;
 uint8_t raw[16]; int c = rlw_shell_peek(d, 0x5099b4 + 16 * zone, 16, raw); if (c) return c;
 for (unsigned i = 0; i < 4; i++) rect[i] = (int32_t)le32(raw + 4 * i);
 return 0;
}
/* Host presentation policy for the objective/briefing pages (modes 17 and 23):
 * the original turns a page only while the button is down and 0x508f7c % 10 == 0
 * (0x42caa9 -> 0x47c7ca). A press is stamped at the next multiple of 10; a
 * release or move that would land on a used multiple is stamped one tick later. */
uint32_t rlw_shell_click_clock(uint32_t mode, uint32_t clock, uint32_t press) {
 if (mode != RL_SHELL_BRIEFING && mode != RL_SHELL_OBJECTIVES) return clock;
 if (press) return clock % 10 ? clock + (10 - clock % 10) : clock;
 return clock % 10 ? clock : clock + 1;
}
/* macos_live_shell_request(): RESET_CAMPAIGN (17 "Novo jogo", 13 "Carregar
 * jogo") and RESUME. */
int rlw_shell_request(RlwDriver *d) {
 if (!d || !d->shell) return RL_INVALID_ARGUMENT;
 uint32_t kind = d->shell_request.kind, mode = d->shell_request.mode;
 if (kind == RL_SHELL_RESET_CAMPAIGN && mode == RL_SHELL_BRIEFING) {
  uint8_t path[300]; uint32_t pn = 0, tn = 0; uint8_t *text = NULL;
  int c = briefing_file(d, 1, path, &pn, &text, &tn);
  if (!c) {
   RlShellFile files[1] = {{{path, pn}, {text, tn}}};
   c = rl_shell_campaign_reset(d->shell, d->shell_generation, files, text ? 1 : 0, &d->shell_generation);
   if (!c) { d->new_campaign = 1; c = shell_entered(d, RL_SHELL_BRIEFING); }
  }
  free(text); return c ? save_result(d, c, "Novo jogo original") : 0;
 }
 if (kind == RL_SHELL_RESET_CAMPAIGN && mode == RL_SHELL_LOAD_MENU) {
  SlotFiles *slots = malloc(sizeof(*slots)); if (!slots) return save_result(d, RL_IO_ERROR, "Carregar jogo original");
  int c = slot_files(d, slots);
  if (!c) c = rl_shell_campaign_reset(d->shell, d->shell_generation, slots->files, slots->count, &d->shell_generation);
  free(slots); if (!c) c = shell_entered(d, RL_SHELL_LOAD_MENU);
  return c ? save_result(d, c, "Carregar jogo original") : 0;
 }
 if (kind == RL_SHELL_RESUME) return shell_resume(d);
 if (kind == RL_SHELL_RESET_CAMPAIGN) {
  snprintf(d->save_error, sizeof(d->save_error), "Pedido original %u/%u (%s) ainda não disponível na tela preparada", kind, mode, "Novo jogo");
  return RL_UNSUPPORTED_PATH;
 }
 return RL_INVALID_ARGUMENT;
}
/* credits_file(): exact <CD root>\\Events\\CREDITS.FLC VFS entry, the imported
 * file's first 128 bytes. *header_length stays 0 when the file is absent. */
static int credits_file(RlwDriver *d, uint8_t *path, uint32_t *pn, uint8_t *header, uint32_t *header_length) {
 uint8_t *world = NULL; uint32_t wn = 0; int c = shell_world_bytes(d, &world, &wn);
 if (!c) c = shell_string(world, wn, 0x509594, path, 256, pn); free(world); if (c) return c;
 memcpy(path + *pn, "\\Events\\CREDITS.FLC", 19); *pn += 19; *header_length = 0;
 char name[4200]; snprintf(name, sizeof(name), "%s" RLW_SEP "events" RLW_SEP "CREDITS.FLC", d->root);
 if (!rlw_exists(name)) return 0;
 FILE *f = rlw_fopen(name, "rb"); if (!f) return RL_IO_ERROR;
 size_t n = fread(header, 1, 128, f); fclose(f); if (n != 128) return RL_IO_ERROR; *header_length = 128; return 0;
}
/* One AffCredits frame. ENDED (the FLC did not open) leaves SHOW main pending in
 * the engine: completed here, so the host only repaints. */
int rlw_shell_credits_frame(RlwDriver *d, uint32_t elapsed_ms, RlShellCredits *out) {
 if (!d || !d->shell || !out || d->shell_mode != RL_SHELL_CREDITS_SCREEN || d->shell_request.kind) return RL_INVALID_ARGUMENT;
 int c = rl_shell_credits_frame(d->shell, d->shell_generation, elapsed_ms, out);
 if (c) return save_result(d, c, "Créditos originais");
 d->shell_generation = out->generation;
 if (out->state == RL_SHELL_CREDITS_ENDED) {
  c = rl_shell_menu_init(d->shell, d->shell_generation, NULL, 0, &d->shell_generation);
  if (!c) c = shell_entered(d, RL_SHELL_MAIN);
  if (c) return save_result(d, c, "Retorno original dos créditos");
 }
 return 0;
}
/* Host's decoded FLC picture over the persistent picture, with the FLC palette. */
int rlw_shell_credits_present(RlwDriver *d, const RlShellCredits *frame, const uint8_t *pixels, const uint8_t *palette) {
 if (!d || !frame || !pixels || !palette || d->shell_mode != RL_SHELL_CREDITS_SCREEN || frame->state != RL_SHELL_CREDITS_DRAW || frame->x + frame->width > 800 || frame->y + frame->height > 600) return RL_INVALID_ARGUMENT;
 for (uint32_t y = 0; y < frame->height; y++) memcpy(d->pixels + (frame->y + y) * 800 + frame->x, pixels + y * frame->width, frame->width);
 memcpy(d->palette, palette, 768); return 0;
}
/* macos_live_shell_show(): SHOW, the pause menu's SAVE_LOAD (lists 12/14) and CREDITS (mode10). */
int rlw_shell_show(RlwDriver *d) {
 if (!d || !d->shell || (d->shell_request.kind != RL_SHELL_SHOW && d->shell_request.kind != RL_SHELL_SAVE_LOAD && d->shell_request.kind != RL_SHELL_CREDITS)) return RL_INVALID_ARGUMENT;
 uint32_t mode = d->shell_request.kind == RL_SHELL_CREDITS ? RL_SHELL_CREDITS_SCREEN : d->shell_request.mode;
 uint8_t path[300], header[128]; uint32_t pn = 0, tn = 0; uint8_t *text = NULL; unsigned number = 0; int c = 0;
 SlotFiles *slots = NULL; RlShellFile files[1]; const RlShellFile *vfs = files; uint32_t count = 0;
 if (is_list(mode)) { slots = malloc(sizeof(*slots)); c = slots ? slot_files(d, slots) : RL_IO_ERROR; if (!c) { vfs = slots->files; count = slots->count; } }
 else if (mode == RL_SHELL_BRIEFING || mode == RL_SHELL_OBJECTIVES) { c = stage_number(d, &number); if (!c) c = briefing_file(d, number, path, &pn, &text, &tn); files[0] = (RlShellFile){{path, pn}, {text, tn}}; count = text ? 1 : 0; }
 else if (mode == RL_SHELL_CREDITS_SCREEN) { c = credits_file(d, path, &pn, header, &tn); files[0] = (RlShellFile){{path, pn}, {header, tn}}; count = tn ? 1 : 0; if (c) snprintf(d->save_error, sizeof(d->save_error), "Falha ao ler o CREDITS.FLC original"); }
 uint32_t from = d->shell_mode;
 if (!c) c = rl_shell_menu_init(d->shell, d->shell_generation, vfs, count, &d->shell_generation);
 if (!c) c = shell_entered(d, mode);
 /* Pause "Reiniciar a missão": the briefing's START_MISSION reloads this stage. */
 if (!c && mode == RL_SHELL_BRIEFING && from == RL_SHELL_PAUSE) d->new_campaign = (int)number;
 free(text); free(slots); return c ? save_result(d, c, "Tela original") : 0;
}
void rlw_shell_error(const RlwDriver *d, char *out, size_t capacity) { if (d && out && capacity) snprintf(out, capacity, "%s", d->save_error); }
