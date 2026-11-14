/* Saves (milestone 3), included by rlw_driver.c before rlw_shell.inc. Port of
 * macos/src/macos_live_driver.c's view companion, save job,
 * serialize/snapshot_capture/snapshot_file, save_allowed, macos_live_save*,
 * macos_live_autosave and macos_live_load: same engine calls, checks, order
 * and file formats, so a save written by either host loads in the other.
 *
 * The slot file is exactly the engine's rl_salvar_bytes / rl_autosalvar_bytes
 * output (the bytes rl_salvar_arquivo would write). The host-owned presentation
 * lives in a content-addressed companion "<slot>.<sha256 of slot>.view":
 * "RLVIEW4\n", sha256(slot), sha256(payload), then the payload: surface
 * (480000) + terrain cache keys (1024) + payloads (270336) + cursor (4), the
 * mission name (16), the difficulty (4) and the host pointer x/y (4 + 4). The
 * companion is written first, then the slot replaces the old one atomically;
 * until then the old slot still names its old companion. */
#define VIEW_V3_SIZE (PRESENTATION_SIZE + 20u)
#define VIEW_SIZE (VIEW_V3_SIZE + 8u)
static const char VIEW_ERROR[] = "Não foi possível ler ou gravar a apresentação da partida. Mantenha o arquivo .rls e seu arquivo .view juntos.";

struct RlwSaveJob { char path[4096]; uint8_t *bytes; uint32_t n; uint8_t hash[32]; uint8_t header[72]; uint8_t *payload; char error[512]; };

static int view_error(RlwDriver *d) { snprintf(d->save_error, sizeof(d->save_error), "%s", VIEW_ERROR); return RL_IO_ERROR; }
static int file_digest(const char *path, uint8_t *out) {
 FILE *f = rlw_fopen(path, "rb"); if (!f) return 1;
 RlwSha256 context; rlw_sha256_init(&context); uint8_t bytes[8192]; size_t n, total = 0;
 while ((n = fread(bytes, 1, sizeof(bytes), f))) { total += n; if (total > 160u * 1024u * 1024u) { fclose(f); return 1; } rlw_sha256_update(&context, bytes, n); }
 int bad = ferror(f); fclose(f); if (bad) return 1; rlw_sha256_final(&context, out); return 0;
}
static int view_path(const char *path, const uint8_t *hash, char *out) {
 char hex[65]; rlw_sha256_hex(hash, hex);
 return snprintf(out, 4096, "%s.%s.view", path, hex) >= 4096;
}
/* read_view(): RLVIEW4 (and RLVIEW3, without the host pointer); RLVIEW1/2 carry
 * the old HUD and are refused with their files kept. */
static uint8_t *read_view(RlwDriver *d, const char *path, char *mission, uint32_t *difficulty, int32_t *host_x, int32_t *host_y, int *has_host, int *code) {
 *code = RL_IO_ERROR; *has_host = 0;
 uint8_t hash[32], header[72], actual[32]; char target[4096];
 if (file_digest(path, hash) || view_path(path, hash, target)) { view_error(d); return NULL; }
 FILE *f = rlw_fopen(target, "rb"); if (!f) { view_error(d); return NULL; }
 int bad = fread(header, 1, sizeof(header), f) != sizeof(header); uint32_t size = 0;
 int legacy = !bad && (!memcmp(header, "RLVIEW1\n", 8) || !memcmp(header, "RLVIEW2\n", 8));
 if (!bad) { if (!memcmp(header, "RLVIEW4\n", 8)) size = VIEW_SIZE; else if (!memcmp(header, "RLVIEW3\n", 8) || !memcmp(header, "RLVIEW2\n", 8)) size = VIEW_V3_SIZE; else if (!memcmp(header, "RLVIEW1\n", 8)) size = PRESENTATION_SIZE; else bad = 1; }
 uint8_t *data = bad ? NULL : malloc(size); bad = bad || !data;
 if (!bad) bad = fread(data, 1, size, f) != size || fgetc(f) != EOF || ferror(f);
 fclose(f);
 if (!bad) { rlw_sha256(data, size, actual); bad = memcmp(header + 8, hash, 32) || memcmp(header + 40, actual, 32); }
 if (!bad && le_u32(data + 751360) > 255) bad = 1;
 if (!bad && size >= VIEW_V3_SIZE) {
  memcpy(mission, data + PRESENTATION_SIZE, 16); memcpy(difficulty, data + PRESENTATION_SIZE + 16, 4);
  bad = mission[15] != 0 || strlen(mission) < 11 || strlen(mission) > 12 || strncmp(mission, "MONDE", 5) || strcmp(mission + strlen(mission) - 4, ".ETA") || *difficulty > 2;
 }
 if (bad) { free(data); view_error(d); return NULL; }
 if (size == VIEW_SIZE) { memcpy(host_x, data + VIEW_V3_SIZE, 4); memcpy(host_y, data + VIEW_V3_SIZE + 4, 4); *has_host = 1; }
 if (legacy) {
  free(data); *code = RL_SAVE_VERSION;
  snprintf(d->save_error, sizeof(d->save_error), "Esta apresentação usa o HUD antigo (RLVIEW1/2). Abra a partida na versão que a salvou; os arquivos foram preservados.");
  return NULL;
 }
 return data;
}

/* The save job: captured on the game thread, committed by any thread without
 * the driver (companion first, then the slot through a durable replace). */
void rlw_save_job_free(RlwSaveJob *j) { if (!j) return; free(j->bytes); free(j->payload); free(j); }
static int job_view_error(RlwSaveJob *j) { snprintf(j->error, sizeof(j->error), "%s", VIEW_ERROR); return RL_IO_ERROR; }
int rlw_save_commit(RlwSaveJob *j) {
 if (!j) return RL_INVALID_ARGUMENT; j->error[0] = 0;
 uint8_t old_hash[32]; int had_old = file_digest(j->path, old_hash) == 0;
 char target[4096]; if (view_path(j->path, j->hash, target)) return job_view_error(j);
 if (rlw_durable_write(target, j->header, sizeof(j->header), j->payload, VIEW_SIZE)) return job_view_error(j);
 if (rlw_durable_write(j->path, j->bytes, j->n, NULL, 0)) {
  snprintf(j->error, sizeof(j->error), "Não foi possível salvar · %.3900s. A partida salva anterior foi mantida.", j->path); return RL_IO_ERROR;
 }
 if (had_old && memcmp(old_hash, j->hash, 32)) { char old_path[4096]; if (!view_path(j->path, old_hash, old_path)) rlw_remove_file(old_path); }
 return 0;
}
void rlw_save_job_error(const RlwSaveJob *j, char *out, size_t capacity) { if (j && out && capacity) snprintf(out, capacity, "%s", j->error); }

/* serialize(): one serialisation in steady state (the buffer keeps the last
 * size plus slack; only a grown world repeats after RL_BUFFER_TOO_SMALL). */
static int serialize(RlwDriver *d, int stage, uint8_t **out, uint32_t *size) {
 uint32_t capacity = d->save_capacity ? d->save_capacity : 4u * 1024u * 1024u;
 for (int attempt = 0; attempt < 3; attempt++) {
  uint8_t *bytes = malloc(capacity); if (!bytes) return RL_IO_ERROR; uint32_t n = 0;
  int c = stage ? rl_autosalvar_bytes(d->world, bytes, capacity, &n) : rl_salvar_bytes(d->world, &d->input, bytes, capacity, &n);
  if (!c) { d->save_capacity = n + n / 8 + 65536; *out = bytes; *size = n; return 0; }
  free(bytes); if (c != RL_BUFFER_TOO_SMALL || n <= capacity) return c; capacity = n + n / 8 + 65536;
 }
 return RL_SAVE_INVALID;
}
/* snapshot_capture(): ordinary saves accept this boundary's draw first (the
 * retained pixels may precede logic); a game paused in the original menu
 * (mode16, saved from its list) keeps its retained gameplay picture; stage
 * autosaves use the presentation captured with the stage's first draw. */
static int snapshot_capture(RlwDriver *d, const char *path, int stage, RlwSaveJob **out) {
 *out = NULL;
 if (!d->game_surface) { snprintf(d->save_error, sizeof(d->save_error), "A apresentação histórica é somente para validação. Salve uma partida no modo Jogar."); return RL_SAVE_BUSY; }
 if (strlen(path) >= 4000) return save_result(d, RL_INVALID_ARGUMENT, "Caminho da partida");
 if (stage && !d->stage_presentation) { snprintf(d->save_error, sizeof(d->save_error), "Não há salvamento automático do início desta fase."); return RL_NO_AUTOSAVE; }
 uint32_t screen = 0; if (rl_missao_tela(d->world, &screen)) screen = 0;
 if (!stage && screen != 16) { int c = first_game_draw(d); if (c) return save_result(d, c, "Não foi possível preparar a apresentação da partida"); }
 RlwSaveJob *j = calloc(1, sizeof(*j)); if (!j) return view_error(d);
 snprintf(j->path, sizeof(j->path), "%s", path);
 int c = serialize(d, stage, &j->bytes, &j->n);
 if (c) { rlw_save_job_free(j); return save_result(d, c, "Não foi possível preparar a partida"); }
 rlw_sha256(j->bytes, j->n, j->hash);
 j->payload = malloc(VIEW_SIZE); if (!j->payload) { rlw_save_job_free(j); return view_error(d); }
 if (stage) memcpy(j->payload, d->stage_presentation, PRESENTATION_SIZE);
 else { c = capture_presentation(d, j->payload); if (c) { rlw_save_job_free(j); return save_result(d, c, "Não foi possível preparar a apresentação"); } }
 memcpy(j->payload + PRESENTATION_SIZE, d->mission, 16); memcpy(j->payload + PRESENTATION_SIZE + 16, &d->difficulty, 4);
 int32_t x = stage ? d->stage_cursor_x : d->cursor_x, y = stage ? d->stage_cursor_y : d->cursor_y;
 memcpy(j->payload + VIEW_V3_SIZE, &x, 4); memcpy(j->payload + VIEW_V3_SIZE + 4, &y, 4);
 memcpy(j->header, "RLVIEW4\n", 8); memcpy(j->header + 8, j->hash, 32); rlw_sha256(j->payload, VIEW_SIZE, j->header + 40);
 d->save_error[0] = 0; *out = j; return 0;
}
static int snapshot_file(RlwDriver *d, const char *path, int stage) {
 RlwSaveJob *j = NULL; int c = snapshot_capture(d, path, stage, &j); if (c) return c;
 c = rlw_save_commit(j); if (c) snprintf(d->save_error, sizeof(d->save_error), "%s", j->error); else d->save_error[0] = 0;
 rlw_save_job_free(j); return c;
}
/* save_allowed(): between complete frames, before a stop, outside a shell
 * session, with no host input still waiting for its frame and no held button. */
static int save_allowed(RlwDriver *d) {
 if (d->shell || d->status.stopped || d->phase_index == UINT32_MAX || d->input_next_phase == 3) { snprintf(d->save_error, sizeof(d->save_error), "Salve durante o jogo, entre quadros completos e antes de uma parada do motor."); return RL_SAVE_BUSY; }
 if (d->pending_count) { snprintf(d->save_error, sizeof(d->save_error), "Há cliques ou teclas aguardando o próximo quadro. Continue o jogo e tente novamente."); return RL_SAVE_BUSY; }
 if (d->left_held) { snprintf(d->save_error, sizeof(d->save_error), "Solte o botão do mouse antes de salvar a partida."); return RL_SAVE_BUSY; }
 return 0;
}
int rlw_driver_save_begin(RlwDriver *d, const char *path, RlwSaveJob **job) {
 if (!d || !path || !job) return RL_INVALID_ARGUMENT; *job = NULL; int c = save_allowed(d); return c ? c : snapshot_capture(d, path, 0, job);
}
int rlw_driver_save(RlwDriver *d, const char *path) {
 if (!d || !path) return RL_INVALID_ARGUMENT; int c = save_allowed(d); return c ? c : snapshot_file(d, path, 0);
}
int rlw_driver_autosave(RlwDriver *d, const char *path) { if (!d || !path) return RL_INVALID_ARGUMENT; return snapshot_file(d, path, 1); }
uint32_t rlw_driver_pending_input(const RlwDriver *d) { return d ? d->pending_count : 0; }
int rlw_driver_stage_start(const RlwDriver *d) { return d && !d->shell && !d->menu_world && d->phase_index == UINT32_MAX; }
const char *rlw_driver_mission(const RlwDriver *d) { return d ? d->mission : ""; }
int rlw_driver_clock(const RlwDriver *d, uint64_t *tick, uint32_t *milliseconds) {
 if (!d || !d->world || !tick || !milliseconds) return RL_INVALID_ARGUMENT; return rl_relogio(d->world, tick, milliseconds);
}
void rlw_driver_save_error(const RlwDriver *d, char *out, size_t capacity) { if (d && out && capacity) snprintf(out, capacity, "%s", d->save_error); }

/* macos_live_load(): a candidate world, renderer and picture are built first;
 * the driver changes only when all succeeded (the open session, if any, is
 * discarded then). A game saved from the original pause list (mode16) keeps
 * its saved picture and the game palette until the pause menu resumes it. */
int rlw_driver_load(RlwDriver *d, const char *path) {
 if (!d || !path) return RL_INVALID_ARGUMENT;
 char mission[16] = {0}; uint32_t difficulty = 0;
 int view_code = 0, has_host = 0; int32_t host_x = 0, host_y = 0;
 uint8_t *view = read_view(d, path, mission, &difficulty, &host_x, &host_y, &has_host, &view_code);
 if (!view) return view_code;
 RlMundo world = 0; RlRenderer renderer = 0; RlFrameInput input; memset(&input, 0, sizeof(input));
 int c = rl_carregar_arquivo((const uint8_t *)path, (uint32_t)strlen(path), &world, &input);
 if (c) { free(view); return save_result(d, c, "Não foi possível carregar"); }
 uint64_t gen = 0; uint32_t phase = 0, frame = 0, mode = 0;
 c = rl_execucao_modo(world, &mode); if (!c) c = rl_render_geracao(world, &gen, &phase, &frame);
 if (!c && (mode != RL_EXECUCAO_JOGO || (phase != 1 && phase != 3))) {
  free(view); rl_liberar_mundo(world);
  snprintf(d->save_error, sizeof(d->save_error), "A partida precisa estar no modo Jogar e em um quadro completo."); return RL_SAVE_BUSY;
 }
 if (!c) c = rl_eventos_reassociar_v1(world, &d->events_host);
 if (!c) c = event_flc_declare(d, world);
 if (!c) c = new_renderer(d, world, view, &renderer);
 uint8_t *pixels = NULL, *palette = NULL; RlRenderResult rendered; memset(&rendered, 0, sizeof(rendered));
 if (!c) { pixels = malloc(sizeof(d->pixels)); palette = malloc(sizeof(d->palette)); if (!pixels || !palette) c = RL_IO_ERROR; }
 uint32_t paused = 0; if (!c && rl_missao_tela(world, &paused)) paused = 0;
 if (!c && paused == 16) { c = rl_mundo_paleta_jogo(world, palette, 768); if (!c) memcpy(pixels, view, 480000); }
 free(view);
 if (!c && paused != 16) c = rl_render_quadro(renderer, gen, 1, pixels, sizeof(d->pixels), palette, sizeof(d->palette), &rendered);
 if (!c && (rendered.abi_status || rendered.disposition == 2)) {
  snprintf(d->save_error, sizeof(d->save_error), "Renderização da partida: %.48s at 0x%08x: %.192s", rendered.stop_callee, rendered.stop_address, rendered.stop_reason);
  free(pixels); free(palette); rl_render_liberar(renderer); rl_liberar_mundo(world); return 5;
 }
 if (c) { free(pixels); free(palette); if (renderer) rl_render_liberar(renderer); rl_liberar_mundo(world); return save_result(d, c, "Não foi possível preparar a partida"); }
 RlMundo old_world = d->world; RlRenderer old_renderer = d->renderer;
 if (d->shell) rl_shell_free(d->shell); d->shell = 0; d->shell_mode = 0; memset(&d->shell_request, 0, sizeof(d->shell_request));
 d->world = world; d->renderer = renderer; d->input = input; memcpy(d->mission, mission, 16); d->difficulty = difficulty;
 d->game_surface = 1;
 memcpy(d->pixels, pixels, sizeof(d->pixels)); memcpy(d->palette, palette, sizeof(d->palette)); free(pixels); free(palette);
 memset(&d->status, 0, sizeof(d->status)); d->status.frames = frame; d->status.generation = gen; d->phase_index = frame * 2; d->input_next_phase = 2; d->started = 1;
 d->pending_count = 0; memset(d->held, 0, sizeof(d->held)); d->cursor_x = has_host ? host_x : input.cursor_x; d->cursor_y = has_host ? host_y : input.cursor_y;
 d->mouse_until = 0; d->left_held = 0; d->left_down_frame = 0;
 if (old_renderer) rl_render_liberar(old_renderer); rl_liberar_mundo(old_world);
 d->menu_world = 0; d->new_campaign = 0; free(d->pause_presentation); d->pause_presentation = NULL;
 save_result(d, 0, "");
 return refresh_screen(d);
}
