#include "rlw_gamemenu.h"
#include "rlw_autosave.h"
#include "rlw_fs.h"
#include "rlw_savemgr.h"
#include "rlw_saves.h"
#include <stdio.h>
#include <string.h>

static int slot_exists(const char *directory, const char *slot) {
 char path[4200]; return directory && directory[0] && !rlw_saves_path(directory, slot, path, sizeof(path)) && rlw_exists(path);
}
void rlw_gamemenu_scan(RlwGameMenuState *s, const char *directory) {
 s->quick = slot_exists(directory, "rapido"); s->stage = slot_exists(directory, "inicio-fase");
 s->autosave = slot_exists(directory, RLW_AUTOSAVE_SLOT); s->positions = 0;
 for (unsigned n = 1; n <= RLW_GM_POSITIONS; n++) { char slot[32]; snprintf(slot, sizeof(slot), "posicao-%u", n); if (slot_exists(directory, slot)) s->positions |= 1u << n; }
}

/* refreshSaveMenus/validateMenuItem: saves and loads need the folder, no write
 * in progress and live menus; a named save needs play; loads need the file. */
static int usable(const RlwGameMenuState *s) { return s->driver && s->saves && !s->writing && !s->failed; }
/* openOptions(): over the main or pause menu, or in play (not paused: the
 * synthetic Escape must reach the world). */
static int options_ready(const RlwGameMenuState *s) {
 return s->options && s->driver && !s->failed && ((s->playing && !s->paused) || s->shell_mode == 2 || s->shell_mode == 16);
}

RlwGameMenuItem rlw_gamemenu_item(const RlwGameMenuState *s, unsigned id) {
 RlwGameMenuItem r = {0, 0};
 if (id == RLW_GM_QUICK_SAVE) r.enabled = usable(s);
 else if (id == RLW_GM_QUICK_LOAD) r.enabled = usable(s) && s->quick;
 else if (id == RLW_GM_STAGE_LOAD) r.enabled = usable(s) && s->stage;
 else if (id == RLW_GM_AUTOSAVE_LOAD) r.enabled = usable(s) && s->autosave;
 else if (id > RLW_GM_SAVE_POSITION && id <= RLW_GM_SAVE_POSITION + RLW_GM_POSITIONS) r.enabled = usable(s) && s->shell_mode == 0;
 else if (id > RLW_GM_LOAD_POSITION && id <= RLW_GM_LOAD_POSITION + RLW_GM_POSITIONS) r.enabled = usable(s) && (s->positions >> (id - RLW_GM_LOAD_POSITION) & 1u);
 else if (id == RLW_GM_MANAGE) r.enabled = usable(s) && rlw_savemgr_available(s->shell_mode);
 else if (id >= RLW_GM_AUTOSAVE && id < RLW_GM_AUTOSAVE + 4) { r.enabled = 1; r.checked = RLW_AUTOSAVE_CHOICES[id - RLW_GM_AUTOSAVE] == s->autosave_minutes; }
 else if (id == RLW_GM_KEYS) r.enabled = s->keys;
 else if (id == RLW_GM_PAUSE) { r.enabled = s->driver && s->playing && !s->failed; r.checked = 0; }
 else if (id >= RLW_GM_SPEED && id < RLW_GM_SPEED + 4) { r.enabled = s->driver != 0; r.checked = s->speed == id - RLW_GM_SPEED; }
 else if (id == RLW_GM_FASTER) r.enabled = s->driver && s->speed < 3;
 else if (id == RLW_GM_SLOWER) r.enabled = s->driver && s->speed > 0;
 else if (id >= RLW_GM_DIFFICULTY && id < RLW_GM_DIFFICULTY + 3) { r.enabled = s->driver != 0; r.checked = s->difficulty == id - RLW_GM_DIFFICULTY; }
 else if (id == RLW_GM_OPTIONS) r.enabled = options_ready(s);
 else if (id == RLW_GM_MODERN) { r.enabled = s->options; r.checked = s->atalhos != 0; }
 else if (id == RLW_GM_PATHS) { r.enabled = s->options; r.checked = s->caminhos != 0; }
 return r;
}

void rlw_gamemenu_label(const RlwGameMenuState *s, unsigned id, char *out, size_t capacity) {
 static const char *const speeds[4] = {"0,5x \xc2\xb7 15 quadros/s", "1x \xc2\xb7 30 quadros/s", "2x \xc2\xb7 60 quadros/s", "4x \xc2\xb7 120 quadros/s"};
 static const char *const levels[3] = {"F\xc3\xa1" "cil", "Normal", "Dif\xc3\xad" "cil"};
 const char *text = "";
 if (id == RLW_GM_QUICK_SAVE) text = "Salvar r\xc3\xa1pido\tF5";
 else if (id == RLW_GM_QUICK_LOAD) text = "Carregar r\xc3\xa1pido\tF9";
 else if (id == RLW_GM_STAGE_LOAD) text = "In\xc3\xad" "cio da fase \xc2\xb7 salvamento autom\xc3\xa1tico";
 else if (id == RLW_GM_MANAGE) text = "Gerenciar partidas salvas\xe2\x80\xa6";
 else if (id == RLW_GM_KEYS) text = "Configurar teclas\xe2\x80\xa6";
 else if (id == RLW_GM_PAUSE) text = s && s->paused ? "Continuar\tP" : "Pausar\tP";
 else if (id >= RLW_GM_SPEED && id < RLW_GM_SPEED + 4) text = speeds[id - RLW_GM_SPEED];
 else if (id == RLW_GM_FASTER) text = "Mais r\xc3\xa1pido\t+";
 else if (id == RLW_GM_SLOWER) text = "Mais devagar\t\xe2\x88\x92";
 else if (id >= RLW_GM_DIFFICULTY && id < RLW_GM_DIFFICULTY + 3) text = levels[id - RLW_GM_DIFFICULTY];
 else if (id == RLW_GM_OPTIONS) text = "Op\xc3\xa7\xc3\xb5" "es\xe2\x80\xa6";
 else if (id == RLW_GM_MODERN) text = "Atalhos modernos";
 else if (id == RLW_GM_PATHS) text = "Caminhos melhores";
 else if (id >= RLW_GM_AUTOSAVE && id < RLW_GM_AUTOSAVE + 4) { rlw_autosave_title(RLW_AUTOSAVE_CHOICES[id - RLW_GM_AUTOSAVE], out, capacity); return; }
 snprintf(out, capacity, "%s", text);
}

RlwGameMenuCommand rlw_gamemenu_decode(const RlwGameMenuState *s, unsigned id) {
 RlwGameMenuCommand c = {0, 0};
 if (!rlw_gamemenu_item(s, id).enabled) return c;
 if (id > RLW_GM_SAVE_POSITION && id <= RLW_GM_SAVE_POSITION + RLW_GM_POSITIONS) { c.action = RLW_GM_SAVE_POSITION; c.arg = id - RLW_GM_SAVE_POSITION; }
 else if (id > RLW_GM_LOAD_POSITION && id <= RLW_GM_LOAD_POSITION + RLW_GM_POSITIONS) { c.action = RLW_GM_LOAD_POSITION; c.arg = id - RLW_GM_LOAD_POSITION; }
 else if (id >= RLW_GM_AUTOSAVE && id < RLW_GM_AUTOSAVE + 4) { c.action = RLW_GM_AUTOSAVE; c.arg = RLW_AUTOSAVE_CHOICES[id - RLW_GM_AUTOSAVE]; }
 else if (id >= RLW_GM_SPEED && id < RLW_GM_SPEED + 4) { c.action = RLW_GM_SPEED; c.arg = id - RLW_GM_SPEED; }
 else if (id == RLW_GM_FASTER) { c.action = RLW_GM_SPEED; c.arg = s->speed + 1; }
 else if (id == RLW_GM_SLOWER) { c.action = RLW_GM_SPEED; c.arg = s->speed - 1; }
 else if (id >= RLW_GM_DIFFICULTY && id < RLW_GM_DIFFICULTY + 3) { c.action = RLW_GM_DIFFICULTY; c.arg = id - RLW_GM_DIFFICULTY; }
 else if (id == RLW_GM_MODERN) { c.action = id; c.arg = !s->atalhos; }
 else if (id == RLW_GM_PATHS) { c.action = id; c.arg = !s->caminhos; }
 else if (id == RLW_GM_PAUSE) { c.action = id; c.arg = !s->paused; }
 else c.action = id;
 return c;
}

/* 30 frames, as optionsAfterPause's deadline. */
int rlw_options_request_start(RlwOptionsRequest *r, uint64_t frames) {
 if (r->pending) return 0;
 r->pending = 1; r->deadline = frames + 30; return 1;
}
int rlw_options_request_shell(RlwOptionsRequest *r) { int show = r->pending; r->pending = 0; return show; }
int rlw_options_request_frame(RlwOptionsRequest *r, uint64_t frames, int playing) {
 if (!r->pending || !playing || frames < r->deadline) return 0;
 r->pending = 0; return 1;
}
