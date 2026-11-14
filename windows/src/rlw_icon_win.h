/* The game's original icon on the window and the Start-menu shortcut (Win32). */
#ifndef RLW_ICON_WIN_H
#define RLW_ICON_WIN_H
#include <windows.h>
/* Builds <data_root>/rising.ico from the imported RISING.EXE when absent
 * (rlw_icon_prepare) and sets it as the window's big and small icon. In a
 * player's run, the one time the .ico is made (first run after the import, or
 * of this version): "Rising Lands.lnk" in Start > Programs (an existing one is
 * kept) and the question "Criar um atalho na Área de Trabalho com o ícone do
 * jogo?". Without an imported icon the default stays. 0 when the icon was set.
 * Call it with the window shown (the question is owned by it). */
int rlw_icon_setup(HWND window, const char *data_root, int players_run);
/* Jogo > Criar atalho na Área de Trabalho: (re)creates the desktop shortcut and
 * reports the result. 0 when created. */
int rlw_icon_desktop_shortcut(HWND owner);
#endif
