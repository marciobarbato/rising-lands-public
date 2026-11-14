/* Partida > Gerenciar partidas salvas… (Win32), port of the macOS app's
 * NativeSaveManager: a modal window over the game listing the existing slots
 * (name, stage, date) with Renomear… and Apagar… (to the Recycle Bin, with the
 * .view companion; never a permanent delete). All list/rename/trash logic is
 * rlw_savemgr.c. The window is modal, so the game loop runs no frame while it
 * is open; the caller pauses input and waits for a pending save first. */
#ifndef RLW_SAVEMGR_DIALOG_H
#define RLW_SAVEMGR_DIALOG_H
#ifdef _WIN32
#include <windows.h>
/* Returns 1 when a name or a slot changed (the caller refreshes its menus). */
int rlw_savemgr_dialog_run(HWND owner, const char *directory);
#endif
#endif
