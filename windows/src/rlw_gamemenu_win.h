/* The "Jogo" popup on the Win32 menu bar, built from the portable model in
 * rlw_gamemenu.h. The host owns the bar: other popups (Áudio…) are appended to
 * it by their own files. */
#ifndef RLW_GAMEMENU_WIN_H
#define RLW_GAMEMENU_WIN_H
#include <windows.h>
#include "rlw_gamemenu.h"
/* A menu bar holding the "Jogo" popup (items in the macOS player menu's order). */
HMENU rlw_gamemenu_create(void);
/* Before showing (WM_INITMENUPOPUP) and after any change: position titles from
 * the save folder, enabled/checked state, "Pausar"/"Continuar". */
void rlw_gamemenu_refresh(HMENU bar, const RlwGameMenuState *state, const char *directory);
#endif
