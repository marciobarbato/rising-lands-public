/* Keyboard bindings and the physical-input router of the Windows host.
 * Port of macos/src/NativeHotkeyBindings.swift and
 * NativeHotkeyInputRouter.swift: the same actions, the same 23-key whitelist of
 * game channels, the same alias/modifier rules and the same "Atalhos modernos"
 * double tap. Physical keys are Windows virtual keys (the macOS app uses its
 * key codes). Portable C, no Win32: the unit tests run on any OS. */
#ifndef RLW_HOTKEYS_H
#define RLW_HOTKEYS_H
#include <stddef.h>
#include <stdint.h>

/* NativeHotkeyBindings.Action, in the same order. The first 20 are the game's
 * input channels (each emits exactly one of the 23 accepted virtual keys; Shift,
 * Control and Alt stay independent channels outside the bindings). The last four
 * are "Atalhos modernos" commands with no channel of their own. */
enum {
 RLW_ACT_ENTER, RLW_ACT_ESCAPE, RLW_ACT_SPACE, RLW_ACT_LEFT, RLW_ACT_UP, RLW_ACT_RIGHT, RLW_ACT_DOWN,
 RLW_ACT_DELETE, RLW_ACT_X, RLW_ACT_TAB,
 RLW_ACT_DIGIT0, RLW_ACT_DIGIT1, RLW_ACT_DIGIT2, RLW_ACT_DIGIT3, RLW_ACT_DIGIT4,
 RLW_ACT_DIGIT5, RLW_ACT_DIGIT6, RLW_ACT_DIGIT7, RLW_ACT_DIGIT8, RLW_ACT_DIGIT9,
 RLW_ACT_LAST_ALERT, RLW_ACT_NEXT_IDLE_WORKER, RLW_ACT_NEXT_IDLE_MILITARY, RLW_ACT_MAIN_STRUCTURE,
 RLW_ACTION_COUNT
};
#define RLW_FIRST_MODERN_ACTION RLW_ACT_LAST_ALERT

/* Gameplay modifiers (Windows has no Command; the host's own keys are reserved instead). */
#define RLW_MOD_SHIFT 1u
#define RLW_MOD_CONTROL 2u
#define RLW_MOD_ALT 4u
#define RLW_MOD_ALL 7u
#define RLW_MAX_ALIASES 8

/* One physical chord. optional = the legacy defaults' "modificadores opcionais":
 * held Shift/Control/Alt are accepted on top of mods. */
typedef struct { uint8_t vk, mods, optional; } RlwChord;
typedef struct { uint8_t count[RLW_ACTION_COUNT]; RlwChord chords[RLW_ACTION_COUNT][RLW_MAX_ALIASES]; } RlwBindings;

/* BindingError. first/second name the actions of a conflict. */
enum {
 RLW_BIND_OK, RLW_BIND_INVALID_FILE, RLW_BIND_UNSUPPORTED_VERSION, RLW_BIND_DUPLICATE_ACTION,
 RLW_BIND_INVALID_KEY, RLW_BIND_INVALID_MODIFIERS, RLW_BIND_RESERVED, RLW_BIND_CONFLICT, RLW_BIND_TOO_MANY,
 /* NativeHotkeyPreferencesSession.EditorError and the save failure. */
 RLW_EDIT_INVALID_SELECTION, RLW_EDIT_FINISH_CAPTURE, RLW_EDIT_BUSY, RLW_EDIT_WRITE_FAILED
};
typedef struct { int code, first, second; } RlwBindError;

/* The 23 virtual keys the driver accepts (macos_live_event's kind 2). */
int rlw_accepted_vk(uint32_t vk);
/* Channel virtual key of an action; 0 for the modern actions. */
uint32_t rlw_action_vk(int action);
int rlw_action_is_modern(int action);
/* File name (the macOS Action raw value) and Portuguese label. */
const char *rlw_action_name(int action);
const char *rlw_action_label(int action);
/* Keys a chord may never use: modifier/lock/system keys (RLW_BIND_INVALID_KEY)
 * and the host's own keys (RLW_BIND_RESERVED): P/Pause, +/-, F5, F9, F11, Alt+F4. */
int rlw_chord_check(const RlwChord *c);

void rlw_bindings_defaults(RlwBindings *b);
int rlw_bindings_equal(const RlwBindings *a, const RlwBindings *b);
/* First action whose chords accept (vk, mods), or -1. */
int rlw_bindings_action(const RlwBindings *b, uint32_t vk, uint32_t mods);
/* Every chord valid, no two chords of the whole set overlap. */
int rlw_bindings_validate(const RlwBindings *b, RlwBindError *err);
/* Replaces every alias of one action; b changes only if the result is valid. */
int rlw_bindings_rebind(RlwBindings *b, int action, const RlwChord *chords, unsigned count, RlwBindError *err);
/* User-facing message (no key codes or paths), NativeHotkeyPreferencesSession.message. */
void rlw_bind_error_message(const RlwBindError *err, char *out, size_t capacity);
/* Fallback key name (US layout); the Win32 host prefers GetKeyNameTextW. */
void rlw_vk_label(uint32_t vk, char *out, size_t capacity);

/* Bindings file (teclas.ini beside opcoes.ini): "versao=1", then one line per
 * action, "<name>=<vk>/<mods>/<optional> ..." (an empty value unbinds it).
 * format writes every action; parse checks the version, rejects unknown or
 * repeated actions and validates the whole set before writing *out. An action
 * missing from the file takes its default unless that chord is already in use
 * (NativeHotkeyBindings.load's migration). */
void rlw_bindings_format(const RlwBindings *b, char *out, size_t capacity);
int rlw_bindings_parse(const char *text, size_t length, RlwBindings *out, RlwBindError *err);

/* NativeHotkeyPreferencesSession: an editable draft over the applied set. */
typedef struct {
 RlwBindings applied, draft;
 int capturing, capture_action, capture_alias; /* alias -1 appends an alternative */
 int applying;
} RlwEditor;
void rlw_editor_init(RlwEditor *e, const RlwBindings *applied);
int rlw_editor_has_changes(const RlwEditor *e);
int rlw_editor_begin_capture(RlwEditor *e, int action, int alias, RlwBindError *err);
/* 1 = recorded (capture ends), 0 = nothing to record (no capture or a repeat),
 * <0 = refused: err says why and the capture stays for a retry. The chord is
 * exact (no optional modifiers), as on macOS. */
int rlw_editor_record(RlwEditor *e, uint32_t vk, uint32_t mods, int repeat, RlwBindError *err);
void rlw_editor_cancel_capture(RlwEditor *e);
int rlw_editor_remove_alias(RlwEditor *e, int action, int alias, RlwBindError *err);
int rlw_editor_reset_defaults(RlwEditor *e, RlwBindError *err);
/* Discards the draft. */
void rlw_editor_cancel(RlwEditor *e);
/* commit saves and installs the candidate (0 = done; nonzero = nothing changed,
 * RLW_EDIT_WRITE_FAILED). Only then does the draft become the applied set. */
int rlw_editor_apply(RlwEditor *e, int (*commit)(void *ctx, const RlwBindings *candidate), void *ctx, RlwBindError *err);

/* NativeHotkeyInputRouter. Modern commands: */
enum { RLW_MODERN_NONE, RLW_MODERN_CENTER_GROUP, RLW_MODERN_LAST_ALERT, RLW_MODERN_NEXT_IDLE_WORKER,
       RLW_MODERN_NEXT_IDLE_MILITARY, RLW_MODERN_MAIN_STRUCTURE };
/* Two taps of the same digit within this interval centre on the group. */
#define RLW_DOUBLE_TAP_SECONDS 0.35
typedef struct {
 void *ctx;
 /* A game channel down/up (one of the 23 keys). */
 void (*emit)(void *ctx, uint32_t vk, int down);
 /* An "Atalhos modernos" command; digit_vk = 49..57 for RLW_MODERN_CENTER_GROUP. */
 void (*modern)(void *ctx, int command, uint32_t digit_vk);
} RlwRouterSink;
typedef struct {
 RlwBindings bindings;
 int modern_enabled;
 struct { int8_t action; uint8_t used, modern; } presses[256];
 uint16_t references[RLW_ACTION_COUNT];
 uint32_t last_digit; double last_digit_time;
 RlwRouterSink sink;
} RlwRouter;

void rlw_router_init(RlwRouter *r, const RlwBindings *bindings, RlwRouterSink sink);
/* Non-modifier physical keys only (Shift/Control/Alt stay the host's channels).
 * Returns 1 when handled (the key is bound or consumed), 0 otherwise. A repeat
 * never emits. */
int rlw_router_key_down(RlwRouter *r, uint32_t vk, uint32_t mods, int repeat, double now);
int rlw_router_key_up(RlwRouter *r, uint32_t vk);
/* Forgets presses, alias counts and the last tap without emitting (the host
 * releases the driver's held keys itself, as on focus loss). */
void rlw_router_reset(RlwRouter *r);
/* prepareInstallation's commit: a different set resets, then installs. */
void rlw_router_install(RlwRouter *r, const RlwBindings *bindings);
#endif
