#include "rlw_gamemenu_win.h"
#include "rlw_autosave.h"
#include "rlw_saves.h"
#include <stdio.h>
#include <stdlib.h>

static HMENU save_menu, load_menu;

static wchar_t *widen(const char *s) {
 int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
 wchar_t *w = malloc((size_t)(n > 0 ? n : 1) * sizeof(wchar_t));
 if (w) { if (n > 0) MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n); else w[0] = 0; }
 return w;
}
static void append(HMENU menu, UINT flags, UINT_PTR id, const char *text) {
 wchar_t *w = widen(text); AppendMenuW(menu, flags, id, w ? w : L""); free(w);
}
static void item(HMENU menu, unsigned id) { char text[160]; rlw_gamemenu_label(NULL, id, text, sizeof(text)); append(menu, MF_STRING, id, text); }
static void popup(HMENU parent, HMENU child, const char *title) { append(parent, MF_POPUP, (UINT_PTR)child, title); }

HMENU rlw_gamemenu_create(void) {
 HMENU bar = CreateMenu(), game = CreatePopupMenu(), speed = CreatePopupMenu(), difficulty = CreatePopupMenu(), periodic = CreatePopupMenu();
 save_menu = CreatePopupMenu(); load_menu = CreatePopupMenu();
 item(game, RLW_GM_PAUSE);
 for (unsigned i = 0; i < 4; i++) item(speed, RLW_GM_SPEED + i);
 AppendMenuW(speed, MF_SEPARATOR, 0, NULL); item(speed, RLW_GM_FASTER); item(speed, RLW_GM_SLOWER);
 popup(game, speed, "Velocidade");
 for (unsigned i = 0; i < 3; i++) item(difficulty, RLW_GM_DIFFICULTY + i);
 AppendMenuW(difficulty, MF_SEPARATOR, 0, NULL); append(difficulty, MF_STRING | MF_GRAYED, 0, "Vale a partir do pr\xc3\xb3ximo in\xc3\xad" "cio de fase");
 popup(game, difficulty, "Dificuldade");
 AppendMenuW(game, MF_SEPARATOR, 0, NULL);
 item(game, RLW_GM_QUICK_SAVE); item(game, RLW_GM_QUICK_LOAD);
 popup(game, save_menu, "Salvar partida\xe2\x80\xa6"); popup(game, load_menu, "Carregar partida");
 item(game, RLW_GM_MANAGE);
 for (unsigned i = 0; i < 4; i++) item(periodic, RLW_GM_AUTOSAVE + i);
 AppendMenuW(periodic, MF_SEPARATOR, 0, NULL); append(periodic, MF_STRING | MF_GRAYED, 0, "Conta o tempo de jogo, s\xc3\xb3 com a partida em andamento");
 popup(game, periodic, "Salvamento autom\xc3\xa1tico");
 AppendMenuW(game, MF_SEPARATOR, 0, NULL);
 item(game, RLW_GM_OPTIONS); item(game, RLW_GM_KEYS); item(game, RLW_GM_MODERN); item(game, RLW_GM_PATHS);
 popup(bar, game, "Jogo");
 return bar;
}

static void state(HMENU bar, const RlwGameMenuState *s, unsigned id) {
 RlwGameMenuItem r = rlw_gamemenu_item(s, id);
 EnableMenuItem(bar, id, MF_BYCOMMAND | (r.enabled ? MF_ENABLED : MF_GRAYED));
 CheckMenuItem(bar, id, MF_BYCOMMAND | (r.checked ? MF_CHECKED : MF_UNCHECKED));
}
void rlw_gamemenu_refresh(HMENU bar, const RlwGameMenuState *s, const char *directory) {
 if (!bar || !save_menu || !load_menu) return;
 /* refreshSaveMenus(): positions 1..5 with their names and times, then the two autosaves. */
 while (GetMenuItemCount(save_menu) > 0) DeleteMenu(save_menu, 0, MF_BYPOSITION);
 while (GetMenuItemCount(load_menu) > 0) DeleteMenu(load_menu, 0, MF_BYPOSITION);
 char title[400];
 for (unsigned n = 1; n <= RLW_GM_POSITIONS; n++) {
  rlw_saves_title(directory ? directory : "", n, title, sizeof(title));
  append(save_menu, MF_STRING, RLW_GM_SAVE_POSITION + n, title); append(load_menu, MF_STRING, RLW_GM_LOAD_POSITION + n, title);
 }
 AppendMenuW(load_menu, MF_SEPARATOR, 0, NULL); item(load_menu, RLW_GM_STAGE_LOAD);
 rlw_autosave_caption(directory, title, sizeof(title)); append(load_menu, MF_STRING, RLW_GM_AUTOSAVE_LOAD, title);
 /* Pausar / Continuar. */
 char label[64]; rlw_gamemenu_label(s, RLW_GM_PAUSE, label, sizeof(label)); wchar_t *w = widen(label);
 MENUITEMINFOW info; ZeroMemory(&info, sizeof(info)); info.cbSize = sizeof(info); info.fMask = MIIM_STRING; info.dwTypeData = w ? w : (LPWSTR)L"";
 SetMenuItemInfoW(bar, RLW_GM_PAUSE, FALSE, &info); free(w);
 for (unsigned id = RLW_GM_FIRST; id <= RLW_GM_LAST; id++) {
  char text[8]; rlw_gamemenu_label(s, id, text, sizeof(text));
  int positional = (id > RLW_GM_SAVE_POSITION && id <= RLW_GM_SAVE_POSITION + RLW_GM_POSITIONS) || (id > RLW_GM_LOAD_POSITION && id <= RLW_GM_LOAD_POSITION + RLW_GM_POSITIONS) || id == RLW_GM_AUTOSAVE_LOAD;
  if (text[0] || positional) state(bar, s, id);
 }
}
