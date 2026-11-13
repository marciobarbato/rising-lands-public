/* "Atalhos de teclado" window of the Windows host (Win32 only). Port of
 * NativeHotkeyPreferencesController (NativeHotkeyPreferences.swift): one row per
 * action of rlw_hotkeys.h, edit/add/remove alternatives by pressing the new
 * combination, restore defaults, apply or cancel. Edits a draft
 * (RlwEditor); only Aplicar saves and installs it, through the host. */
#ifndef RLW_KEYS_DIALOG_H
#define RLW_KEYS_DIALOG_H
#include <windows.h>
#include "rlw_hotkeys.h"

typedef struct {
 void *ctx;
 /* Saves the candidate (teclas.ini) and installs it in the router; 0 = done,
  * nonzero = nothing changed (the window says the save failed). */
 int (*commit)(void *ctx, const RlwBindings *candidate);
 /* After the window closed and the owner was enabled again. */
 void (*closed)(void *ctx);
} RlwKeysDialogHost;

/* Opens the window over owner (disabled while it is open), or brings an open one
 * forward. 0 or -1 when the window could not be created. */
int rlw_keys_dialog_open(HWND owner, const RlwBindings *applied, const RlwKeysDialogHost *host);
int rlw_keys_dialog_is_open(void);
/* Call for every message before TranslateMessage/DispatchMessage: 1 = handled
 * (keyboard navigation inside the window). */
int rlw_keys_dialog_message(MSG *message);
#endif
