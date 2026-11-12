#ifndef RISING_OPTIONS_H
#define RISING_OPTIONS_H
#include "rising_shell.h"
#ifdef __cplusplus
extern "C" {
#endif
/* "Opções" extension v1: a port-only screen (RISING.EXE has none), additive to
 * ABI1.6 and prepared shell v1, whose functions and structures are unchanged.
 * See options-screen.md. Same conventions as rising_shell.h: integer handles,
 * all buffers copied, errors leave handle state and outputs unchanged,
 * rl_erro_mensagem holds the diagnostic. Handles come from the common allocator.
 *
 * The engine stores no setting: the host passes its values (rl_opcoes_criar,
 * rl_opcoes_definir) and reads every change back (RlOpcoesEvento.values).
 * Drawing, input and hit tests READ a prepared shell session (MenuInit fonts
 * 0x4e2f74..76, MENU.CMP, mode, counter 0x508f84) and never change it: not its
 * surface, memory, generation or request. No RNG, clock or game memory use.
 * Without these calls the shell and its pictures stay byte for byte the same. */
#define RL_OPCOES_VERSION 1u
typedef uint64_t RlOpcoes;
/* Items, in screen order; bit (1<<item) of disponivel offers it. */
enum { RL_OPCOES_ATALHOS=0, RL_OPCOES_TECLAS=1, RL_OPCOES_CAMINHOS=2,
 RL_OPCOES_DIFICULDADE=3, RL_OPCOES_VELOCIDADE=4, RL_OPCOES_VOLTAR=5 };
#define RL_OPCOES_TODOS 0x3fu
typedef struct {
 uint32_t version;     /* RL_OPCOES_VERSION */
 uint32_t atalhos;     /* modern hotkeys (+ order queue) 0/1 */
 uint32_t caminhos;    /* better paths, rule bit0 0/1 */
 uint32_t dificuldade; /* RL_DIFICULDADE_* 0..2 (stage-start preset) */
 uint32_t velocidade;  /* host pace 0=0,5x 1=1x 2=2x 3=4x */
 uint32_t disponivel;  /* offered items mask (<= RL_OPCOES_TODOS); VOLTAR always */
 uint32_t idioma;      /* 0 pt-BR, 1 English (prepared for the language option) */
 uint32_t reserved;    /* 0 */
} RlOpcoesValores;
/* Keys: RL_SHELL_ENTER/UP/DOWN/ESCAPE plus LEFT/RIGHT (completed presses). */
enum { RL_OPCOES_ESQUERDA=16, RL_OPCOES_DIREITA=32 };
typedef struct {
 uint32_t version;              /* RL_OPCOES_VERSION */
 uint32_t keys;                 /* <= 63 */
 uint32_t mouse_x, mouse_y;     /* original 800x600 space, as RlShellInput */
 uint32_t mouse_ae, mouse_af;   /* held / released (a click = released while held) */
 uint32_t ticks;                /* animation steps since the previous input */
 uint32_t reserved;             /* 0 */
} RlOpcoesEntrada;
enum { RL_OPCOES_NADA=0, RL_OPCOES_ABRIU=1, RL_OPCOES_MUDOU=2,
 RL_OPCOES_CONFIGURAR_TECLAS=3, RL_OPCOES_FECHOU=4 };
typedef struct {
 uint32_t kind;     /* RL_OPCOES_NADA..FECHOU */
 uint32_t consumed; /* 1: do NOT give this input to rl_shell_input */
 uint32_t item;     /* MUDOU/CONFIGURAR_TECLAS: the item; else UINT32_MAX */
 uint32_t open;     /* screen open after this input */
 RlOpcoesValores values; /* current values (apply/store them on MUDOU) */
} RlOpcoesEvento;

uint32_t rl_opcoes_versao(void);
/* background: the decoded original menu background (TITRE.PIE through
 * rl_shell_imagem_cd), 480000 indices. One handle per host; it outlives shells. */
int32_t rl_opcoes_criar(const RlOpcoesValores *values, RlShellBytes background,
 RlOpcoes *output);
int32_t rl_opcoes_definir(RlOpcoes options, const RlOpcoesValores *values);
int32_t rl_opcoes_valores(RlOpcoes options, RlOpcoesValores *output);
int32_t rl_opcoes_aberta(RlOpcoes options, uint32_t *open);
/* Explicit open/close (the button opens it through rl_opcoes_entrada). Open
 * needs a shell in main2 or pause16 without a pending request. */
int32_t rl_opcoes_abrir(RlOpcoes options, RlShell shell);
int32_t rl_opcoes_fechar(RlOpcoes options);
/* Call BEFORE rl_shell_input with the same input (no shell generation needed).
 * Closed: only a press that starts on the button, and its release, are
 * consumed (the release on it opens the screen: ABRIU); every other input
 * returns consumed=0 and goes to the shell unchanged. A pending shell request
 * or a screen other than main2/pause16 consumes nothing. Open: every input is
 * consumed; the shell is not fed, so it stays exactly as it was. */
int32_t rl_opcoes_entrada(RlOpcoes options, RlShell shell,
 const RlOpcoesEntrada *input, RlOpcoesEvento *output);
/* Closed, over main2/pause16: draws the "OPÇÕES" button into the host's COPY
 * of the shell picture (rl_shell_copy output); hovered = selected font. Other
 * screens, or open: pixels untouched. Call after every shell copy. */
int32_t rl_opcoes_sobrepor(RlOpcoes options, RlShell shell, uint8_t *pixels,
 uint32_t capacity);
/* Open: the whole screen (background, MENU.CMP animations, title, items) and
 * the shell's palette. Closed: RL_INVALID_ARGUMENT. Deterministic. */
int32_t rl_opcoes_desenhar(RlOpcoes options, RlShell shell, uint8_t *pixels,
 uint32_t pixel_capacity, uint8_t *palette, uint32_t palette_capacity);
/* The button's rectangle {left, top, right, bottom} (exclusive), 800x600. */
int32_t rl_opcoes_botao(RlOpcoes options, RlShell shell, int32_t rect[4]);
int32_t rl_opcoes_liberar(RlOpcoes options);
#ifdef __cplusplus
}
#endif
#endif
