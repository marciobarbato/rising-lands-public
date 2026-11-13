/* "Atalhos de teclado" (see rlw_keys_dialog.h). Plain Win32 controls only (no
 * common-controls library): a list box with one row per action, a drop-down of
 * the selected action's alternatives, a capture field that takes the next key
 * combination, and the macOS window's buttons and messages. */
#define WIN32_LEAN_AND_MEAN
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "rlw_keys_dialog.h"

enum { IDC_LIST = 200, IDC_ALIASES, IDC_CHANGE, IDC_ADD, IDC_REMOVE, IDC_RECORDER, IDC_CANCEL_CAPTURE, IDC_FEEDBACK,
       IDC_RESET, IDC_CANCEL, IDC_APPLY, IDC_TITLE, IDC_TEXT };

static struct {
 HWND window, owner, list, aliases, change, add, remove, recorder, cancel_capture, feedback, reset, cancel, apply;
 HFONT font, title_font; int refreshing, error; RlwEditor editor; RlwKeysDialogHost host; char message[512];
} K;

static void set_text(HWND h, const char *utf8) {
 wchar_t w[1024]; if (!MultiByteToWideChar(CP_UTF8, 0, utf8, -1, w, 1024)) w[0] = 0;
 SetWindowTextW(h, w);
}
static LRESULT add_string(HWND h, UINT msg, const char *utf8) {
 wchar_t w[1024]; if (!MultiByteToWideChar(CP_UTF8, 0, utf8, -1, w, 1024)) w[0] = 0;
 return SendMessageW(h, msg, 0, (LPARAM)w);
}

/* Character keys take the current layout's name (GetKeyNameTextW); the others
 * keep fixed Portuguese names, as NativeHotkeyLabels does. */
static void key_label(uint32_t vk, char *out, size_t capacity) {
 int character = (vk >= '0' && vk <= '9') || (vk >= 'A' && vk <= 'Z') || (vk >= 0xba && vk <= 0xc2) || (vk >= 0xdb && vk <= 0xdf) || vk == 0xe2;
 if (character) {
  UINT scan = MapVirtualKeyW(vk, 0 /* MAPVK_VK_TO_VSC */);
  wchar_t name[64];
  if (scan && GetKeyNameTextW((LONG)(scan << 16), name, 64) > 0) {
   CharUpperW(name);
   if (WideCharToMultiByte(CP_UTF8, 0, name, -1, out, (int)capacity, NULL, NULL) > 0) return;
  }
 }
 rlw_vk_label(vk, out, capacity);
}
static void chord_text(const RlwChord *c, char *out, size_t capacity) {
 char key[96]; key_label(c->vk, key, sizeof(key));
 snprintf(out, capacity, "%s%s%s%s%s", (c->mods & RLW_MOD_CONTROL) ? "Ctrl + " : "", (c->mods & RLW_MOD_ALT) ? "Alt + " : "",
          (c->mods & RLW_MOD_SHIFT) ? "Shift + " : "", key, c->optional ? " (modificadores opcionais)" : "");
}
static void summary(int action, char *out, size_t capacity) {
 const RlwBindings *b = &K.editor.draft; size_t n = 0; out[0] = 0;
 if (!b->count[action]) { snprintf(out, capacity, "Sem atalho"); return; }
 for (unsigned k = 0; k < (unsigned)b->count[action] && n < capacity; k++) {
  char one[160]; chord_text(&b->chords[action][k], one, sizeof(one));
  int w = snprintf(out + n, capacity - n, "%s%s", k ? " ou " : "", one); if (w > 0) n += (size_t)w;
 }
}
static int selected_action(void) {
 LRESULT row = SendMessageW(K.list, LB_GETCURSEL, 0, 0);
 return row >= 0 && row < RLW_ACTION_COUNT ? (int)row : -1;
}

static void refresh(const char *message, int error) {
 if (!K.window || K.refreshing) return;
 K.refreshing = 1;
 if (message != K.message) snprintf(K.message, sizeof(K.message), "%s", message);
 int row = selected_action(); if (row < 0) row = 0;
 LRESULT alias = SendMessageW(K.aliases, CB_GETCURSEL, 0, 0);
 SendMessageW(K.list, WM_SETREDRAW, FALSE, 0);
 SendMessageW(K.list, LB_RESETCONTENT, 0, 0);
 for (int a = 0; a < RLW_ACTION_COUNT; a++) {
  char line[1024], chords[900]; summary(a, chords, sizeof(chords));
  snprintf(line, sizeof(line), "%s\t%s", rlw_action_label(a), chords);
  add_string(K.list, LB_ADDSTRING, line);
 }
 SendMessageW(K.list, LB_SETCURSEL, (WPARAM)row, 0);
 SendMessageW(K.list, WM_SETREDRAW, TRUE, 0); InvalidateRect(K.list, NULL, TRUE);
 SendMessageW(K.aliases, CB_RESETCONTENT, 0, 0);
 unsigned count = K.editor.draft.count[row];
 for (unsigned k = 0; k < count; k++) {
  char one[160], item[200]; chord_text(&K.editor.draft.chords[row][k], one, sizeof(one));
  snprintf(item, sizeof(item), "%u. %s", k + 1, one); add_string(K.aliases, CB_ADDSTRING, item);
 }
 if (!count) add_string(K.aliases, CB_ADDSTRING, "Sem atalho");
 SendMessageW(K.aliases, CB_SETCURSEL, (WPARAM)(alias >= 0 && (unsigned)alias < count ? alias : 0), 0);
 int capturing = K.editor.capturing;
 EnableWindow(K.aliases, !capturing && count);
 EnableWindow(K.change, !capturing && count);
 EnableWindow(K.add, !capturing);
 EnableWindow(K.remove, !capturing && count);
 EnableWindow(K.cancel_capture, capturing);
 EnableWindow(K.reset, !capturing);
 EnableWindow(K.apply, !capturing && rlw_editor_has_changes(&K.editor));
 InvalidateRect(K.recorder, NULL, TRUE);
 K.error = error; set_text(K.feedback, K.message);
 K.refreshing = 0;
}
static void show_error(const RlwBindError *e) { char m[512]; rlw_bind_error_message(e, m, sizeof(m)); refresh(m, 1); }

static void close_window(void) {
 if (!K.window) return;
 HWND w = K.window;
 rlw_editor_cancel(&K.editor);
 K.window = NULL;
 EnableWindow(K.owner, TRUE); SetForegroundWindow(K.owner);
 DestroyWindow(w);
 if (K.host.closed) K.host.closed(K.host.ctx);
}

static void begin_capture(int append) {
 int action = selected_action(); if (action < 0) return;
 int alias = append ? -1 : (int)SendMessageW(K.aliases, CB_GETCURSEL, 0, 0);
 RlwBindError e;
 if (rlw_editor_begin_capture(&K.editor, action, alias, &e)) { show_error(&e); return; }
 char m[512]; snprintf(m, sizeof(m), "Pressione a nova combinação para %s. Use Cancelar captura para sair.", rlw_action_label(action));
 refresh(m, 0); SetFocus(K.recorder);
}

static void command(int id, int code) {
 RlwBindError e;
 switch (id) {
 case IDC_LIST:
  if (code == LBN_SELCHANGE && !K.refreshing) { rlw_editor_cancel_capture(&K.editor); refresh("Escolha Alterar ou Adicionar alternativa e pressione a combinação desejada.", 0); }
  break;
 case IDC_ALIASES: if (code == CBN_SELCHANGE && !K.refreshing && K.editor.capturing) { rlw_editor_cancel_capture(&K.editor); refresh("Captura cancelada.", 0); } break;
 case IDC_CHANGE: begin_capture(0); break;
 case IDC_ADD: begin_capture(1); break;
 case IDC_REMOVE: {
  int action = selected_action(); if (action < 0) break;
  if (rlw_editor_remove_alias(&K.editor, action, (int)SendMessageW(K.aliases, CB_GETCURSEL, 0, 0), &e)) show_error(&e);
  else refresh("Atalho removido do rascunho. Clique em Aplicar para confirmar.", 0);
  break;
 }
 case IDC_CANCEL_CAPTURE: rlw_editor_cancel_capture(&K.editor); refresh("Captura cancelada.", 0); SetFocus(K.list); break;
 case IDC_RESET:
  if (rlw_editor_reset_defaults(&K.editor, &e)) show_error(&e);
  else refresh("Padrões restaurados no rascunho. Clique em Aplicar para confirmar.", 0);
  break;
 case IDC_APPLY:
  if (rlw_editor_apply(&K.editor, K.host.commit, K.host.ctx, &e)) show_error(&e);
  else refresh("Atalhos aplicados e salvos.", 0);
  break;
 case IDC_CANCEL: close_window(); break;
 default: break; /* IDOK/IDCANCEL from Enter/Esc: both are legitimate captured keys, no equivalents */
 }
}

static uint32_t modifiers(void) {
 return ((GetKeyState(VK_SHIFT) & 0x8000) ? RLW_MOD_SHIFT : 0u) | ((GetKeyState(VK_CONTROL) & 0x8000) ? RLW_MOD_CONTROL : 0u) |
        ((GetKeyState(VK_MENU) & 0x8000) ? RLW_MOD_ALT : 0u);
}

/* Local capture field: takes every key while capturing, so captured events never
 * reach the game or the window's own navigation. */
static LRESULT CALLBACK recorder_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
 switch (msg) {
 case WM_GETDLGCODE: return K.editor.capturing ? DLGC_WANTALLKEYS | DLGC_WANTCHARS : 0;
 case WM_KEYDOWN: case WM_SYSKEYDOWN: {
  if (!K.editor.capturing) break;
  if (msg == WM_SYSKEYDOWN && wp == VK_F4) break; /* Alt+F4 still closes */
  uint32_t vk = (uint32_t)wp;
  /* Modifier-only input is not a chord. */
  if (vk == VK_SHIFT || vk == VK_CONTROL || vk == VK_MENU || (vk >= 0xa0 && vk <= 0xa5) || vk == VK_LWIN || vk == VK_RWIN) return 0;
  RlwBindError e;
  int r = rlw_editor_record(&K.editor, vk, modifiers(), (lp & (1 << 30)) != 0, &e);
  if (r > 0) { refresh("Atalho alterado no rascunho. Clique em Aplicar para confirmar.", 0); SetFocus(K.list); }
  else if (r < 0) show_error(&e);
  return 0;
 }
 case WM_KEYUP: case WM_SYSKEYUP: case WM_CHAR: case WM_SYSCHAR: if (K.editor.capturing) return 0; break;
 case WM_KILLFOCUS:
  if (K.editor.capturing && K.window) { rlw_editor_cancel_capture(&K.editor); refresh("Captura cancelada ao sair do campo de captura.", 0); }
  InvalidateRect(hwnd, NULL, TRUE); break;
 case WM_SETFOCUS: InvalidateRect(hwnd, NULL, TRUE); break;
 case WM_LBUTTONDOWN: if (K.editor.capturing) SetFocus(hwnd); return 0;
 case WM_PAINT: {
  PAINTSTRUCT ps; HDC dc = BeginPaint(hwnd, &ps); RECT r; GetClientRect(hwnd, &r);
  FillRect(dc, &r, GetSysColorBrush(COLOR_WINDOW));
  DrawEdge(dc, &r, EDGE_SUNKEN, BF_RECT);
  HGDIOBJ old = SelectObject(dc, K.font ? (HGDIOBJ)K.font : GetStockObject(DEFAULT_GUI_FONT));
  SetBkMode(dc, TRANSPARENT); SetTextColor(dc, GetSysColor(COLOR_WINDOWTEXT));
  RECT t = r; t.left += 8;
  DrawTextW(dc, K.editor.capturing ? L"Aguardando a nova combinação…" : L"Captura de atalho", -1, &t, DT_SINGLELINE | DT_VCENTER | DT_LEFT);
  if (GetFocus() == hwnd) { RECT f = r; InflateRect(&f, -3, -3); DrawFocusRect(dc, &f); }
  SelectObject(dc, old); EndPaint(hwnd, &ps); return 0;
 }
 }
 return DefWindowProcW(hwnd, msg, wp, lp);
}

static LRESULT CALLBACK window_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
 switch (msg) {
 case WM_COMMAND: if (K.window) command(LOWORD(wp), HIWORD(wp)); return 0;
 case WM_CTLCOLORSTATIC:
  if ((HWND)lp == K.feedback || (HWND)lp == GetDlgItem(hwnd, IDC_TITLE) || (HWND)lp == GetDlgItem(hwnd, IDC_TEXT)) {
   HDC dc = (HDC)wp; SetBkColor(dc, GetSysColor(COLOR_BTNFACE));
   SetTextColor(dc, (HWND)lp == K.feedback && K.error ? RGB(200, 0, 0) : GetSysColor(COLOR_BTNTEXT));
   return (LRESULT)GetSysColorBrush(COLOR_BTNFACE);
  }
  break;
 case WM_ACTIVATE: if (LOWORD(wp) == WA_INACTIVE && K.editor.capturing && K.window) { rlw_editor_cancel_capture(&K.editor); refresh("Captura cancelada ao sair do campo de captura.", 0); } break;
 case WM_CLOSE: close_window(); return 0;
 }
 return DefWindowProcW(hwnd, msg, wp, lp);
}

static HWND child(const wchar_t *cls, const wchar_t *text, DWORD style, int id, int x, int y, int w, int h, HFONT font) {
 HWND c = CreateWindowExW(0, cls, text, WS_CHILD | WS_VISIBLE | style, x, y, w, h, K.window, (HMENU)(INT_PTR)id, GetModuleHandleW(NULL), NULL);
 if (c) SendMessageW(c, WM_SETFONT, (WPARAM)font, FALSE);
 return c;
}

int rlw_keys_dialog_is_open(void) { return K.window != NULL; }
int rlw_keys_dialog_message(MSG *message) { return K.window && IsDialogMessageW(K.window, message); }

int rlw_keys_dialog_open(HWND owner, const RlwBindings *applied, const RlwKeysDialogHost *host) {
 if (K.window) { SetForegroundWindow(K.window); return 0; }
 HINSTANCE instance = GetModuleHandleW(NULL);
 static int registered;
 if (!registered) {
  WNDCLASSEXW wc; memset(&wc, 0, sizeof(wc)); wc.cbSize = sizeof(wc);
  wc.lpfnWndProc = window_proc; wc.hInstance = instance; wc.hCursor = LoadCursor(NULL, IDC_ARROW);
  wc.hbrBackground = (HBRUSH)(COLOR_BTNFACE + 1); wc.lpszClassName = L"RisingLandsKeys";
  if (!RegisterClassExW(&wc)) return -1;
  wc.lpfnWndProc = recorder_proc; wc.hbrBackground = NULL; wc.lpszClassName = L"RisingLandsKeyRecorder";
  if (!RegisterClassExW(&wc)) return -1;
  registered = 1;
 }
 if (!K.font) {
  NONCLIENTMETRICSW m; memset(&m, 0, sizeof(m)); m.cbSize = sizeof(m);
  if (SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, sizeof(m), &m, 0)) {
   K.font = CreateFontIndirectW(&m.lfMessageFont);
   LOGFONTW title = m.lfMessageFont; title.lfHeight = title.lfHeight * 5 / 3; title.lfWeight = FW_BOLD;
   K.title_font = CreateFontIndirectW(&title);
  }
 }
 HFONT font = K.font ? K.font : (HFONT)GetStockObject(DEFAULT_GUI_FONT), title_font = K.title_font ? K.title_font : font;
 rlw_editor_init(&K.editor, applied); K.host = *host; K.owner = owner; K.error = 0; K.message[0] = 0;
 DWORD style = WS_POPUP | WS_CAPTION | WS_SYSMENU, ex = WS_EX_DLGMODALFRAME | WS_EX_CONTROLPARENT;
 RECT r = {0, 0, 700, 546}; AdjustWindowRectEx(&r, style, FALSE, ex);
 int w = r.right - r.left, h = r.bottom - r.top, x = CW_USEDEFAULT, y = CW_USEDEFAULT;
 RECT o; if (owner && GetWindowRect(owner, &o)) { x = o.left + ((o.right - o.left) - w) / 2; y = o.top + ((o.bottom - o.top) - h) / 2; if (y < 0) y = 0; if (x < 0) x = 0; }
 K.window = CreateWindowExW(ex, L"RisingLandsKeys", L"Atalhos de teclado", style, x, y, w, h, owner, NULL, instance, NULL);
 if (!K.window) return -1;
 child(L"STATIC", L"Atalhos de teclado", SS_LEFT, IDC_TITLE, 20, 14, 660, 32, title_font);
 child(L"STATIC", L"Selecione uma ação para editar seus atalhos. As alterações só entram em vigor ao aplicar. "
                  L"P, Pause, + e −, F5, F9, F11 e Alt+F4 ficam reservados ao aplicativo.", SS_LEFT, IDC_TEXT, 20, 50, 660, 40, font);
 K.list = child(L"LISTBOX", L"", WS_BORDER | WS_VSCROLL | WS_TABSTOP | LBS_NOTIFY | LBS_USETABSTOPS | LBS_NOINTEGRALHEIGHT, IDC_LIST, 20, 96, 660, 250, font);
 INT tab = 150; SendMessageW(K.list, LB_SETTABSTOPS, 1, (LPARAM)&tab);
 K.aliases = child(L"COMBOBOX", L"", WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST, IDC_ALIASES, 20, 358, 300, 220, font);
 K.change = child(L"BUTTON", L"Alterar", WS_TABSTOP | BS_PUSHBUTTON, IDC_CHANGE, 330, 356, 90, 28, font);
 K.add = child(L"BUTTON", L"Adicionar alternativa", WS_TABSTOP | BS_PUSHBUTTON, IDC_ADD, 428, 356, 170, 28, font);
 K.remove = child(L"BUTTON", L"Remover", WS_TABSTOP | BS_PUSHBUTTON, IDC_REMOVE, 606, 356, 74, 28, font);
 K.recorder = child(L"RisingLandsKeyRecorder", L"Captura de atalho", WS_TABSTOP, IDC_RECORDER, 20, 396, 480, 30, font);
 K.cancel_capture = child(L"BUTTON", L"Cancelar captura", WS_TABSTOP | BS_PUSHBUTTON, IDC_CANCEL_CAPTURE, 510, 396, 170, 30, font);
 K.feedback = child(L"STATIC", L"", SS_LEFT, IDC_FEEDBACK, 20, 436, 660, 48, font);
 K.reset = child(L"BUTTON", L"Restaurar padrões", WS_TABSTOP | BS_PUSHBUTTON, IDC_RESET, 20, 500, 160, 30, font);
 K.cancel = child(L"BUTTON", L"Cancelar", WS_TABSTOP | BS_PUSHBUTTON, IDC_CANCEL, 440, 500, 110, 30, font);
 K.apply = child(L"BUTTON", L"Aplicar", WS_TABSTOP | BS_PUSHBUTTON, IDC_APPLY, 560, 500, 120, 30, font);
 if (!K.list || !K.aliases || !K.recorder || !K.feedback || !K.apply) { HWND dead = K.window; K.window = NULL; DestroyWindow(dead); return -1; }
 SendMessageW(K.list, LB_SETCURSEL, 0, 0);
 refresh("Escolha uma ação. Atalhos com modificadores opcionais aceitam Shift, Ctrl e Alt.", 0);
 if (owner) EnableWindow(owner, FALSE);
 ShowWindow(K.window, SW_SHOW); SetFocus(K.list);
 return 0;
}
