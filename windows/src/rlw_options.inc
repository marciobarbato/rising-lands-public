/* Port-only "Opções" screen consumer (rising_options.h), included by
 * rlw_driver.c. Port of macos/src/macos_options.inc: same
 * engine calls and order. No game rules and no stored settings here. */
int rlw_options_sync(RlwDriver *d, const RlOpcoesValores *values) {
 if (!d || !values) return RL_INVALID_ARGUMENT;
 if (rl_opcoes_versao() != RL_OPCOES_VERSION) return RL_UNSUPPORTED_PATH;
 if (d->options) return rl_opcoes_definir(d->options, values);
 /* The original menu background, decoded from the imported CD file. */
 uint32_t n = 0; uint8_t *f = file(d->root, "TITRE.PIE", &n), *p = malloc(480000);
 int c = f && p ? rl_shell_imagem_cd(f, n, p, 480000) : RL_IO_ERROR; free(f);
 if (!c) c = rl_opcoes_criar(values, (RlShellBytes){p, 480000}, &d->options);
 free(p); if (c) { d->options = 0; return c; }
 /* An open main/pause picture gets its button right away. */
 if (d->shell && !d->shell_request.kind && (d->shell_mode == RL_SHELL_MAIN || d->shell_mode == RL_SHELL_PAUSE)) (void)shell_copy_picture(d);
 return 0;
}
uint32_t rlw_options_open(const RlwDriver *d) { return d && d->options && d->options_open ? 1 : 0; }
int rlw_options_input(RlwDriver *d, const RlShellInput *input, uint32_t keys, RlOpcoesEvento *out) {
 if (!d || !input || !out) return RL_INVALID_ARGUMENT;
 memset(out, 0, sizeof(*out)); out->item = UINT32_MAX;
 if (!d->options || !d->shell || d->shell_request.kind) return 0;
 /* Shell ticks advance the counters by one; the screen animates only by them. */
 uint32_t ticks = d->options_open ? input->animation_84 - d->shell_input.animation_84 : 0;
 RlOpcoesEntrada e = {RL_OPCOES_VERSION, keys & 63u, input->mouse_x, input->mouse_y, input->mouse_ae, input->mouse_af, ticks > 8 ? 1 : ticks, 0};
 int c = rl_opcoes_entrada(d->options, d->shell, &e, out);
 if (c) { memset(out, 0, sizeof(*out)); out->item = UINT32_MAX; return save_result(d, c, "Tela de opções"); }
 d->options_open = out->open != 0;
 if (!out->consumed) return 0;
 if (out->open) c = rl_opcoes_desenhar(d->options, d->shell, d->pixels, 480000, d->palette, 768);
 else c = shell_copy_picture(d);
 return c ? save_result(d, c, "Tela de opções") : 0;
}
int rlw_options_button(RlwDriver *d, int32_t rect[4]) {
 if (!d || !rect || !d->options || !d->shell) return RL_INVALID_ARGUMENT;
 return rl_opcoes_botao(d->options, d->shell, rect);
}
/* macos_live_options_show(): Jogo > Opções…, the same screen the button opens,
 * through the explicit rl_opcoes_abrir. Only over main2/pause16 without a
 * pending request; the shell session is not touched. Already open: nothing. */
int rlw_options_show(RlwDriver *d) {
 if (!d || !d->options || !d->shell) return RL_UNSUPPORTED_PATH;
 if (d->shell_request.kind || (d->shell_mode != RL_SHELL_MAIN && d->shell_mode != RL_SHELL_PAUSE)) return RL_INVALID_ARGUMENT;
 if (d->options_open) return 0;
 int c = rl_opcoes_abrir(d->options, d->shell);
 if (c) return save_result(d, c, "Tela de opções");
 d->options_open = 1;
 c = rl_opcoes_desenhar(d->options, d->shell, d->pixels, 480000, d->palette, 768);
 return c ? save_result(d, c, "Tela de opções") : 0;
}
/* After rlw_options_sync from the host's own menu: an open screen shows the new
 * values at once (nothing to do when closed). */
int rlw_options_redraw(RlwDriver *d) {
 if (!d || !d->options || !d->shell || !d->options_open || d->shell_request.kind) return 0;
 int c = rl_opcoes_desenhar(d->options, d->shell, d->pixels, 480000, d->palette, 768);
 return c ? save_result(d, c, "Tela de opções") : 0;
}

/* ---- Port-only "Redistribuir descobertas" (rising_research.h) ----
 * The engine owns every rule; the driver only gates the calls on the host's
 * option and the briefing (mode 17, no pending request) and repaints. */
static int research_active(const RlwDriver *d) {
 return d && d->research && d->shell && !d->shell_request.kind && d->shell_mode == RL_SHELL_BRIEFING && !d->options_open;
}
int rlw_research_enable(RlwDriver *d, int on) {
 if (!d) return 0;
 int want = on && rl_redistribuir_versao() == RL_REDISTRIBUIR_VERSION;
 if (want == d->research) return 0;
 d->research = want;
 /* A briefing on screen gets (or loses) its overlay on a fresh copy. */
 if (!d->shell || d->shell_request.kind || d->shell_mode != RL_SHELL_BRIEFING) return 0;
 return shell_copy_picture(d) ? 0 : 1;
}
int rlw_research_target(RlwDriver *d, uint32_t x, uint32_t y, uint32_t *ramo, int32_t *delta) {
 if (!ramo || !delta) return RL_INVALID_ARGUMENT;
 *ramo = 0; *delta = 0;
 if (!research_active(d)) return 0;
 uint32_t r = 0; int32_t s = 0;
 int c = rl_redistribuir_alvo(d->shell, x, y, &r, &s);
 if (c) return save_result(d, c, "Redistribuir descobertas");
 *ramo = r; *delta = s; return 0;
}
int rlw_research_step(RlwDriver *d, uint32_t ramo, int32_t delta, uint32_t *changed) {
 if (!changed) return RL_INVALID_ARGUMENT;
 *changed = 0;
 if (!research_active(d)) return 0;
 uint64_t generation = d->shell_generation; uint32_t mudou = 0;
 int c = rl_redistribuir_passo(d->shell, d->shell_generation, ramo, delta, &generation, &mudou);
 if (c) return save_result(d, c, "Redistribuir descobertas");
 if (!mudou) return 0;
 /* The session memory changed: the briefing picture (pillars) is painted again
  * from its CD background, then copied with both overlays (shell_paint). */
 d->shell_generation = generation; *changed = 1;
 c = shell_paint(d);
 return c ? save_result(d, c, "Redistribuir descobertas") : 0;
}
