/* Port of NativeHotkeyBindings.swift and NativeHotkeyInputRouter.swift (see
 * rlw_hotkeys.h). Host input only: nothing here reads or writes game state. */
#include "rlw_hotkeys.h"
#include <stdio.h>
#include <string.h>

static const uint32_t ACCEPTED[23] = {13, 16, 17, 18, 27, 32, 37, 38, 39, 40, 46, 88, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 9};
static const uint8_t CHANNEL_VK[RLW_FIRST_MODERN_ACTION] = {13, 27, 32, 37, 38, 39, 40, 46, 88, 9,
                                                           48, 49, 50, 51, 52, 53, 54, 55, 56, 57};
static const char *const NAMES[RLW_ACTION_COUNT] = {
 "enter", "escape", "space", "left", "up", "right", "down", "delete", "x", "tab",
 "digit0", "digit1", "digit2", "digit3", "digit4", "digit5", "digit6", "digit7", "digit8", "digit9",
 "lastAlert", "nextIdleWorker", "nextIdleMilitary", "mainStructure"};
static const char *const LABELS[RLW_ACTION_COUNT] = {
 "Enter", "Esc", "Espaço", "Seta para a esquerda", "Seta para cima", "Seta para a direita", "Seta para baixo",
 "Delete (à frente)", "X", "Tab (minimapa)", "0", "1", "2", "3", "4", "5", "6", "7", "8", "9",
 "Último alerta (moderno)", "Próximo trabalhador ocioso (moderno)", "Próximo militar ocioso (moderno)",
 "Santuário (moderno)"};

int rlw_accepted_vk(uint32_t vk) {
 for (unsigned i = 0; i < 23; i++) if (ACCEPTED[i] == vk) return 1;
 return 0;
}
uint32_t rlw_action_vk(int action) { return action >= 0 && action < RLW_FIRST_MODERN_ACTION ? CHANNEL_VK[action] : 0; }
int rlw_action_is_modern(int action) { return action >= RLW_FIRST_MODERN_ACTION && action < RLW_ACTION_COUNT; }
const char *rlw_action_name(int action) { return action >= 0 && action < RLW_ACTION_COUNT ? NAMES[action] : ""; }
const char *rlw_action_label(int action) { return action >= 0 && action < RLW_ACTION_COUNT ? LABELS[action] : ""; }

int rlw_chord_check(const RlwChord *c) {
 uint32_t vk = c->vk;
 if (c->mods & ~RLW_MOD_ALL) return RLW_BIND_INVALID_MODIFIERS;
 /* Mouse buttons, modifiers, locks, IME, Windows/menu keys, browser/media keys,
  * Print Screen: not keyDown channels a chord can use. */
 if (vk <= 7 || (vk >= 0x10 && vk <= 0x12) || vk == 0x14 || (vk >= 0x15 && vk <= 0x1a) || (vk >= 0x1c && vk <= 0x1f) ||
     vk == 0x2c || (vk >= 0x5b && vk <= 0x5f) || vk == 0x90 || vk == 0x91 || (vk >= 0xa0 && vk <= 0xb7) ||
     vk == 0xe5 || vk == 0xe7 || vk >= 0xf6) return RLW_BIND_INVALID_KEY;
 /* The host's own keys, with any modifiers (rlw_main.c key()): P and Pause,
  * + and - (both rows), F5, F9, F11; Alt+F4 closes the window. */
 if (vk == 'P' || vk == 0x13 || vk == 0x6b || vk == 0xbb || vk == 0x6d || vk == 0xbd || vk == 0x74 || vk == 0x78 || vk == 0x7a)
  return RLW_BIND_RESERVED;
 if (vk == 0x73 && ((c->mods & RLW_MOD_ALT) || c->optional)) return RLW_BIND_RESERVED;
 return RLW_BIND_OK;
}

/* Chord.matches / Chord.overlaps. */
static int matches(const RlwChord *c, uint32_t vk, uint32_t mods) {
 if (c->vk != vk || (mods & ~RLW_MOD_ALL)) return 0;
 return c->optional ? (mods & c->mods) == c->mods : mods == c->mods;
}
static int overlaps(const RlwChord *a, const RlwChord *b) {
 for (uint32_t bits = 0; bits <= RLW_MOD_ALL; bits++) if (matches(a, b->vk, bits) && matches(b, a->vk, bits)) return 1;
 return 0;
}

/* NativeLiveWindow's legacy keys: every channel on its own virtual key, with
 * optional modifiers. Modern defaults (AoE2 style, keys the original does not
 * read): Home, ".", ",", H, exact chords so their Shift/Control/Alt variants stay free. */
void rlw_bindings_defaults(RlwBindings *b) {
 memset(b, 0, sizeof(*b));
 for (int a = 0; a < RLW_FIRST_MODERN_ACTION; a++) { b->count[a] = 1; b->chords[a][0] = (RlwChord){CHANNEL_VK[a], 0, 1}; }
 static const uint8_t modern[4] = {0x24 /* Home */, 0xbe /* . */, 0xbc /* , */, 'H'};
 for (int i = 0; i < 4; i++) { b->count[RLW_FIRST_MODERN_ACTION + i] = 1; b->chords[RLW_FIRST_MODERN_ACTION + i][0] = (RlwChord){modern[i], 0, 0}; }
}

int rlw_bindings_equal(const RlwBindings *a, const RlwBindings *b) {
 for (int i = 0; i < RLW_ACTION_COUNT; i++) {
  if (a->count[i] != b->count[i]) return 0;
  for (unsigned k = 0; k < (unsigned)a->count[i] && k < RLW_MAX_ALIASES; k++)
   if (a->chords[i][k].vk != b->chords[i][k].vk || a->chords[i][k].mods != b->chords[i][k].mods ||
       (a->chords[i][k].optional != 0) != (b->chords[i][k].optional != 0)) return 0;
 }
 return 1;
}

int rlw_bindings_action(const RlwBindings *b, uint32_t vk, uint32_t mods) {
 for (int a = 0; a < RLW_ACTION_COUNT; a++)
  for (unsigned k = 0; k < (unsigned)b->count[a] && k < RLW_MAX_ALIASES; k++) if (matches(&b->chords[a][k], vk, mods)) return a;
 return -1;
}

int rlw_bindings_validate(const RlwBindings *b, RlwBindError *err) {
 RlwBindError e = {RLW_BIND_OK, -1, -1};
 for (int a = 0; a < RLW_ACTION_COUNT && !e.code; a++) {
  if (b->count[a] > RLW_MAX_ALIASES) { e.code = RLW_BIND_TOO_MANY; e.first = e.second = a; break; }
  for (unsigned k = 0; k < (unsigned)b->count[a] && !e.code; k++) {
   const RlwChord *c = &b->chords[a][k];
   int check = rlw_chord_check(c); if (check) { e.code = check; e.first = e.second = a; break; }
   /* Against every chord seen before it, in action order (the first conflict wins). */
   for (int o = 0; o <= a && !e.code; o++)
    for (unsigned j = 0; j < (o == a ? k : (unsigned)b->count[o]); j++)
     if (overlaps(c, &b->chords[o][j])) { e.code = RLW_BIND_CONFLICT; e.first = o; e.second = a; break; }
  }
 }
 if (err) *err = e;
 return e.code;
}

int rlw_bindings_rebind(RlwBindings *b, int action, const RlwChord *chords, unsigned count, RlwBindError *err) {
 if (action < 0 || action >= RLW_ACTION_COUNT) { if (err) *err = (RlwBindError){RLW_BIND_INVALID_FILE, -1, -1}; return RLW_BIND_INVALID_FILE; }
 if (count > RLW_MAX_ALIASES) { if (err) *err = (RlwBindError){RLW_BIND_TOO_MANY, action, action}; return RLW_BIND_TOO_MANY; }
 RlwBindings candidate = *b;
 candidate.count[action] = (uint8_t)count;
 memset(candidate.chords[action], 0, sizeof(candidate.chords[action]));
 for (unsigned k = 0; k < count; k++) candidate.chords[action][k] = chords[k];
 int c = rlw_bindings_validate(&candidate, err);
 if (!c) *b = candidate;
 return c;
}

void rlw_bind_error_message(const RlwBindError *err, char *out, size_t capacity) {
 if (!capacity) return;
 switch (err ? err->code : RLW_BIND_INVALID_FILE) {
 case RLW_BIND_OK: out[0] = 0; break;
 case RLW_BIND_RESERVED:
  snprintf(out, capacity, "Esta tecla é reservada ao aplicativo (pausa, velocidade, salvar e carregar rápido, tela cheia, Alt+F4). Escolha outra.");
  break;
 case RLW_BIND_CONFLICT:
  if (err->first == err->second) snprintf(out, capacity, "Esse atalho já está na lista desta ação.");
  else snprintf(out, capacity, "Conflito de atalhos entre %s e %s. Escolha outra combinação.", rlw_action_label(err->first), rlw_action_label(err->second));
  break;
 case RLW_BIND_INVALID_KEY: case RLW_BIND_INVALID_MODIFIERS:
  snprintf(out, capacity, "Esta tecla ou combinação não pode ser usada. Tente outra."); break;
 case RLW_BIND_TOO_MANY: snprintf(out, capacity, "Esta ação já tem o número máximo de atalhos."); break;
 case RLW_EDIT_INVALID_SELECTION: snprintf(out, capacity, "Selecione um atalho da lista."); break;
 case RLW_EDIT_FINISH_CAPTURE: snprintf(out, capacity, "Conclua ou cancele a captura antes de aplicar."); break;
 case RLW_EDIT_BUSY: snprintf(out, capacity, "Aguarde a aplicação dos atalhos."); break;
 case RLW_EDIT_WRITE_FAILED: snprintf(out, capacity, "Não foi possível salvar os atalhos. As alterações não foram aplicadas."); break;
 default: snprintf(out, capacity, "Não foi possível usar estes atalhos. As alterações não foram aplicadas."); break;
 }
}

void rlw_vk_label(uint32_t vk, char *out, size_t capacity) {
 static const struct { uint8_t vk; const char *name; } special[] = {
  {0x08, "Backspace"}, {0x09, "Tab"}, {0x0d, "Enter"}, {0x1b, "Esc"}, {0x20, "Espaço"}, {0x21, "Page Up"},
  {0x22, "Page Down"}, {0x23, "End"}, {0x24, "Home"}, {0x25, "←"}, {0x26, "↑"}, {0x27, "→"}, {0x28, "↓"},
  {0x2d, "Insert"}, {0x2e, "Delete"}, {0x6a, "Num *"}, {0x6c, "Num ,"}, {0x6e, "Num ."}, {0x6f, "Num /"},
  {0xba, ";"}, {0xbc, ","}, {0xbe, "."}, {0xbf, "/"}, {0xc0, "`"}, {0xc1, "/"}, {0xc2, "Num ."}, {0xdb, "["},
  {0xdc, "\\"}, {0xdd, "]"}, {0xde, "'"}, {0xe2, "\\"}};
 if (!capacity) return;
 if ((vk >= '0' && vk <= '9') || (vk >= 'A' && vk <= 'Z')) { snprintf(out, capacity, "%c", (char)vk); return; }
 if (vk >= 0x60 && vk <= 0x69) { snprintf(out, capacity, "Num %u", vk - 0x60); return; }
 if (vk >= 0x70 && vk <= 0x87) { snprintf(out, capacity, "F%u", vk - 0x6f); return; }
 for (unsigned i = 0; i < sizeof(special) / sizeof(special[0]); i++) if ((uint32_t)special[i].vk == vk) { snprintf(out, capacity, "%s", special[i].name); return; }
 snprintf(out, capacity, "Tecla sem nome");
}

/* ---------- bindings file ---------- */

void rlw_bindings_format(const RlwBindings *b, char *out, size_t capacity) {
 size_t n = 0;
 if (!capacity) return;
 out[0] = 0;
#define PUT(...) do { int w = snprintf(out + n, n < capacity ? capacity - n : 0, __VA_ARGS__); if (w > 0) n += (size_t)w; } while (0)
 PUT("versao=1\r\n");
 for (int a = 0; a < RLW_ACTION_COUNT; a++) {
  PUT("%s=", NAMES[a]);
  for (unsigned k = 0; k < (unsigned)b->count[a] && k < RLW_MAX_ALIASES; k++)
   PUT("%s%u/%u/%u", k ? " " : "", (unsigned)b->chords[a][k].vk, (unsigned)b->chords[a][k].mods, b->chords[a][k].optional ? 1u : 0u);
  PUT("\r\n");
 }
#undef PUT
 if (n >= capacity) out[capacity - 1] = 0;
}

static int set_error(RlwBindError *err, int code, int first, int second) { if (err) { err->code = code; err->first = first; err->second = second; } return code; }

int rlw_bindings_parse(const char *text, size_t length, RlwBindings *out, RlwBindError *err) {
 RlwBindings candidate; memset(&candidate, 0, sizeof(candidate));
 int seen[RLW_ACTION_COUNT] = {0}, version = -1;
 size_t i = 0;
 while (i < length) {
  size_t end = i; while (end < length && text[end] != '\n') end++;
  size_t stop = end; if (stop > i && text[stop - 1] == '\r') stop--;
  char line[512]; size_t len = stop - i;
  if (len >= sizeof(line)) return set_error(err, RLW_BIND_INVALID_FILE, -1, -1);
  memcpy(line, text + i, len); line[len] = 0; i = end + 1;
  if (!len || line[0] == '#') continue;
  char *eq = strchr(line, '='); if (!eq) return set_error(err, RLW_BIND_INVALID_FILE, -1, -1);
  *eq = 0; const char *value = eq + 1;
  if (!strcmp(line, "versao")) {
   unsigned v = 0; char extra; if (sscanf(value, "%u%c", &v, &extra) != 1) return set_error(err, RLW_BIND_INVALID_FILE, -1, -1);
   version = (int)v; if (version != 1) return set_error(err, RLW_BIND_UNSUPPORTED_VERSION, -1, -1);
   continue;
  }
  int action = -1; for (int a = 0; a < RLW_ACTION_COUNT; a++) if (!strcmp(line, NAMES[a])) action = a;
  if (action < 0 || version != 1) return set_error(err, RLW_BIND_INVALID_FILE, -1, -1);
  if (seen[action]) return set_error(err, RLW_BIND_DUPLICATE_ACTION, action, action);
  seen[action] = 1;
  const char *p = value;
  while (*p) {
   while (*p == ' ' || *p == '\t') p++;
   if (!*p) break;
   unsigned vk, mods, optional; int used = 0;
   if (sscanf(p, "%u/%u/%u%n", &vk, &mods, &optional, &used) != 3 || vk > 255 || mods > 255 || optional > 1 ||
       (p[used] && p[used] != ' ' && p[used] != '\t')) return set_error(err, RLW_BIND_INVALID_FILE, -1, -1);
   if (candidate.count[action] >= RLW_MAX_ALIASES) return set_error(err, RLW_BIND_TOO_MANY, action, action);
   candidate.chords[action][candidate.count[action]++] = (RlwChord){(uint8_t)vk, (uint8_t)mods, (uint8_t)optional};
   p += used;
  }
 }
 if (version != 1) return set_error(err, version < 0 ? RLW_BIND_INVALID_FILE : RLW_BIND_UNSUPPORTED_VERSION, -1, -1);
 /* Files saved before an action existed keep every saved choice; the new action
  * takes its default unless that physical key is already in use. */
 RlwBindings defaults; rlw_bindings_defaults(&defaults);
 for (int a = 0; a < RLW_ACTION_COUNT; a++) {
  if (seen[a]) continue;
  for (unsigned k = 0; k < (unsigned)defaults.count[a]; k++) {
   int used = 0;
   for (int o = 0; o < RLW_ACTION_COUNT && !used; o++)
    if (seen[o]) for (unsigned j = 0; j < (unsigned)candidate.count[o]; j++) if (overlaps(&candidate.chords[o][j], &defaults.chords[a][k])) { used = 1; break; }
   if (!used) candidate.chords[a][candidate.count[a]++] = defaults.chords[a][k];
  }
 }
 int c = rlw_bindings_validate(&candidate, err);
 if (!c) *out = candidate;
 return c;
}

/* ---------- NativeHotkeyPreferencesSession ---------- */

void rlw_editor_init(RlwEditor *e, const RlwBindings *applied) {
 memset(e, 0, sizeof(*e)); e->applied = *applied; e->draft = *applied; e->capture_action = -1; e->capture_alias = -1;
}
int rlw_editor_has_changes(const RlwEditor *e) { return !rlw_bindings_equal(&e->draft, &e->applied); }

int rlw_editor_begin_capture(RlwEditor *e, int action, int alias, RlwBindError *err) {
 if (e->applying) return set_error(err, RLW_EDIT_BUSY, -1, -1);
 if (action < 0 || action >= RLW_ACTION_COUNT || alias >= (int)e->draft.count[action] || alias < -1)
  return set_error(err, RLW_EDIT_INVALID_SELECTION, -1, -1);
 e->capturing = 1; e->capture_action = action; e->capture_alias = alias;
 return set_error(err, RLW_BIND_OK, -1, -1);
}

int rlw_editor_record(RlwEditor *e, uint32_t vk, uint32_t mods, int repeat, RlwBindError *err) {
 if (e->applying) return -set_error(err, RLW_EDIT_BUSY, -1, -1);
 if (!e->capturing || repeat) return 0;
 int action = e->capture_action;
 RlwChord chords[RLW_MAX_ALIASES]; unsigned n = e->draft.count[action];
 memcpy(chords, e->draft.chords[action], sizeof(chords));
 RlwChord chord = {(uint8_t)(vk > 255 ? 0 : vk), (uint8_t)(mods > 255 ? 255 : mods), 0};
 if (e->capture_alias >= 0) chords[e->capture_alias] = chord;
 else if (n < RLW_MAX_ALIASES) chords[n++] = chord;
 else return -set_error(err, RLW_BIND_TOO_MANY, action, action);
 int c = rlw_bindings_rebind(&e->draft, action, chords, n, err);
 if (c) return -c;
 e->capturing = 0;
 return 1;
}

void rlw_editor_cancel_capture(RlwEditor *e) { if (!e->applying) e->capturing = 0; }

int rlw_editor_remove_alias(RlwEditor *e, int action, int alias, RlwBindError *err) {
 if (e->applying) return set_error(err, RLW_EDIT_BUSY, -1, -1);
 if (action < 0 || action >= RLW_ACTION_COUNT || alias < 0 || alias >= (int)e->draft.count[action])
  return set_error(err, RLW_EDIT_INVALID_SELECTION, -1, -1);
 RlwChord chords[RLW_MAX_ALIASES]; unsigned n = 0;
 for (unsigned k = 0; k < (unsigned)e->draft.count[action]; k++) if ((int)k != alias) chords[n++] = e->draft.chords[action][k];
 int c = rlw_bindings_rebind(&e->draft, action, chords, n, err);
 if (!c) e->capturing = 0;
 return c;
}

int rlw_editor_reset_defaults(RlwEditor *e, RlwBindError *err) {
 if (e->applying) return set_error(err, RLW_EDIT_BUSY, -1, -1);
 rlw_bindings_defaults(&e->draft); e->capturing = 0;
 return set_error(err, RLW_BIND_OK, -1, -1);
}

void rlw_editor_cancel(RlwEditor *e) { if (!e->applying) { e->draft = e->applied; e->capturing = 0; } }

int rlw_editor_apply(RlwEditor *e, int (*commit)(void *ctx, const RlwBindings *candidate), void *ctx, RlwBindError *err) {
 if (e->applying) return set_error(err, RLW_EDIT_BUSY, -1, -1);
 if (e->capturing) return set_error(err, RLW_EDIT_FINISH_CAPTURE, -1, -1);
 if (!rlw_editor_has_changes(e)) return set_error(err, RLW_BIND_OK, -1, -1);
 int c = rlw_bindings_validate(&e->draft, err); if (c) return c;
 e->applying = 1;
 RlwBindings candidate = e->draft;
 c = commit(ctx, &candidate);
 e->applying = 0;
 if (c) return set_error(err, RLW_EDIT_WRITE_FAILED, -1, -1);
 e->applied = candidate;
 return set_error(err, RLW_BIND_OK, -1, -1);
}

/* ---------- NativeHotkeyInputRouter ---------- */

void rlw_router_init(RlwRouter *r, const RlwBindings *bindings, RlwRouterSink sink) {
 memset(r, 0, sizeof(*r));
 if (bindings) r->bindings = *bindings; else rlw_bindings_defaults(&r->bindings);
 r->sink = sink;
}

static int modern_command(int action) {
 switch (action) {
 case RLW_ACT_LAST_ALERT: return RLW_MODERN_LAST_ALERT;
 case RLW_ACT_NEXT_IDLE_WORKER: return RLW_MODERN_NEXT_IDLE_WORKER;
 case RLW_ACT_NEXT_IDLE_MILITARY: return RLW_MODERN_NEXT_IDLE_MILITARY;
 case RLW_ACT_MAIN_STRUCTURE: return RLW_MODERN_MAIN_STRUCTURE;
 default: return RLW_MODERN_NONE;
 }
}

int rlw_router_key_down(RlwRouter *r, uint32_t vk, uint32_t mods, int repeat, double now) {
 if (vk > 255) return 0;
 if (r->presses[vk].used) return r->presses[vk].action < 0 && !r->presses[vk].modern ? 0 : 1;
 /* Includes repeats arriving after focus loss or an installation. */
 if (repeat) return 0;
 int action = rlw_bindings_action(&r->bindings, vk, mods);
 if (rlw_action_is_modern(action)) {
  int command = modern_command(action);
  if (!r->modern_enabled || !command) { r->presses[vk].used = 1; r->presses[vk].action = -1; r->presses[vk].modern = 0; return 0; }
  r->last_digit = 0;
  r->presses[vk].used = 1; r->presses[vk].action = -1; r->presses[vk].modern = 1;
  if (r->sink.modern) r->sink.modern(r->sink.ctx, command, 0);
  return 1;
 }
 uint32_t key = rlw_action_vk(action);
 if (action >= 0 && r->modern_enabled && key >= 49 && key <= 57 && mods == 0) {
  if (r->last_digit == key && now - r->last_digit_time <= RLW_DOUBLE_TAP_SECONDS) {
   r->last_digit = 0;
   r->presses[vk].used = 1; r->presses[vk].action = -1; r->presses[vk].modern = 1;
   if (r->sink.modern) r->sink.modern(r->sink.ctx, RLW_MODERN_CENTER_GROUP, key);
   return 1;
  }
  r->last_digit = key; r->last_digit_time = now;
 } else if (action >= 0) r->last_digit = 0;
 r->presses[vk].used = 1; r->presses[vk].action = (int8_t)action; r->presses[vk].modern = 0;
 if (action < 0) return 0;
 if (r->references[action]++ == 0 && r->sink.emit) r->sink.emit(r->sink.ctx, key, 1);
 return 1;
}

int rlw_router_key_up(RlwRouter *r, uint32_t vk) {
 if (vk > 255) return 0;
 int used = r->presses[vk].used, action = r->presses[vk].action, modern = r->presses[vk].modern;
 memset(&r->presses[vk], 0, sizeof(r->presses[vk]));
 if (used && modern) return 1;
 if (!used || action < 0) return 0;
 if (r->references[action] > 1) r->references[action]--;
 else { r->references[action] = 0; if (r->sink.emit) r->sink.emit(r->sink.ctx, rlw_action_vk(action), 0); }
 return 1;
}

void rlw_router_reset(RlwRouter *r) {
 r->last_digit = 0;
 memset(r->presses, 0, sizeof(r->presses));
 memset(r->references, 0, sizeof(r->references));
}

void rlw_router_install(RlwRouter *r, const RlwBindings *bindings) {
 if (rlw_bindings_equal(&r->bindings, bindings)) return;
 rlw_router_reset(r);
 r->bindings = *bindings;
}
