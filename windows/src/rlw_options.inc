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
