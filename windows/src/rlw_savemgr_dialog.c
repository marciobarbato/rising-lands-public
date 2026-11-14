/* Partida > Gerenciar partidas salvas… (Win32), port of NativeSaveManager.swift.
 * In-memory dialog templates (no resource file), a report-style list view and
 * the Recycle Bin through SHFileOperationW. Logic: rlw_savemgr.c. */
#include "rlw_savemgr_dialog.h"
#include "rlw_savemgr.h"
#include <commctrl.h>
#include <shellapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>

enum { ID_LIST = 300, ID_EMPTY = 301, ID_NOTE = 302, ID_RENAME = 201, ID_DELETE = 202, ID_TEXT = 100, ID_FIELD = 101 };

static wchar_t *widen(const char *s) {
 int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
 wchar_t *w = malloc((size_t)(n > 0 ? n : 1) * sizeof(wchar_t));
 if (w) { if (n > 0) MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n); else w[0] = 0; }
 return w;
}
static char *narrow(const wchar_t *w) {
 int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
 char *s = malloc((size_t)(n > 0 ? n : 1));
 if (s) { if (n > 0) WideCharToMultiByte(CP_UTF8, 0, w, -1, s, n, NULL, NULL); else s[0] = 0; }
 return s;
}
static void message(HWND owner, const char *text, const char *title) {
 wchar_t *t = widen(text), *c = widen(title); MessageBoxW(owner, t ? t : L"", c ? c : L"", MB_ICONWARNING | MB_OK); free(t); free(c);
}

/* ---- dialog templates ---- */
static WORD *put_text(WORD *p, const wchar_t *text) { do { *p++ = (WORD)*text; } while (*text++); return p; }
static WORD *put_item(WORD *p, DWORD style, short x, short y, short cx, short cy, WORD id, WORD atom, const wchar_t *klass, const wchar_t *text) {
 p = (WORD *)(((uintptr_t)p + 3) & ~(uintptr_t)3);
 DLGITEMTEMPLATE *item = (DLGITEMTEMPLATE *)p; item->style = style | WS_CHILD | WS_VISIBLE; item->dwExtendedStyle = 0;
 item->x = x; item->y = y; item->cx = cx; item->cy = cy; item->id = id;
 p = (WORD *)(item + 1);
 if (klass) p = put_text(p, klass); else { *p++ = 0xffff; *p++ = atom; }
 p = put_text(p, text); *p++ = 0; return p;
}
static WORD *put_header(DWORD *buffer, WORD count, short cx, short cy, const wchar_t *title) {
 DLGTEMPLATE *t = (DLGTEMPLATE *)buffer;
 t->style = DS_MODALFRAME | DS_CENTER | DS_SETFONT | WS_POPUP | WS_CAPTION | WS_SYSMENU; t->cdit = count; t->cx = cx; t->cy = cy;
 WORD *p = (WORD *)(t + 1); *p++ = 0; *p++ = 0; p = put_text(p, title);
 *p++ = 9; return put_text(p, L"Segoe UI");
}

/* ---- the confirmation / name prompt (NSAlert with an optional text field) ---- */
static struct { const wchar_t *text; wchar_t field[96]; int has_field; } PROMPT;
static INT_PTR CALLBACK prompt_proc(HWND dialog, UINT msg, WPARAM wp, LPARAM lp) {
 (void)lp;
 switch (msg) {
 case WM_INITDIALOG:
  SetDlgItemTextW(dialog, ID_TEXT, PROMPT.text);
  if (PROMPT.has_field) {
   SetDlgItemTextW(dialog, ID_FIELD, PROMPT.field); SendDlgItemMessageW(dialog, ID_FIELD, EM_LIMITTEXT, 90, 0);
   SendDlgItemMessageW(dialog, ID_FIELD, EM_SETSEL, 0, -1); SetFocus(GetDlgItem(dialog, ID_FIELD)); return FALSE;
  }
  return TRUE;
 case WM_COMMAND:
  if (LOWORD(wp) == IDOK) { if (PROMPT.has_field) GetDlgItemTextW(dialog, ID_FIELD, PROMPT.field, 96); EndDialog(dialog, 1); return TRUE; }
  if (LOWORD(wp) == IDCANCEL) { EndDialog(dialog, 0); return TRUE; }
  break;
 }
 return FALSE;
}
/* 1 when the first button was chosen; `field` (UTF-8 in/out) when not NULL. */
static int prompt(HWND owner, const char *title, const char *text, const wchar_t *ok, char *field, size_t field_capacity) {
 wchar_t *wtitle = widen(title), *wtext = widen(text); if (!wtitle || !wtext) { free(wtitle); free(wtext); return 0; }
 PROMPT.text = wtext; PROMPT.has_field = field != NULL; PROMPT.field[0] = 0;
 if (field) { wchar_t *w = widen(field); if (w) { wcsncpy(PROMPT.field, w, 95); PROMPT.field[95] = 0; free(w); } }
 static DWORD buffer[2048]; memset(buffer, 0, sizeof(buffer));
 short text_h = field ? 34 : 58, y = (short)(8 + text_h + 6);
 WORD *p = put_header(buffer, (WORD)(field ? 4 : 3), 270, (short)(y + (field ? 40 : 22)), wtitle);
 p = put_item(p, SS_LEFT, 8, 8, 254, text_h, ID_TEXT, 0x0082, NULL, L"");
 if (field) { p = put_item(p, WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL, 8, y, 254, 13, ID_FIELD, 0x0081, NULL, L""); y = (short)(y + 20); }
 p = put_item(p, WS_TABSTOP | BS_DEFPUSHBUTTON, 120, y, 86, 14, IDOK, 0x0080, NULL, ok);
 put_item(p, WS_TABSTOP | BS_PUSHBUTTON, 210, y, 52, 14, IDCANCEL, 0x0080, NULL, L"Cancelar");
 INT_PTR r = DialogBoxIndirectParamW(GetModuleHandleW(NULL), (DLGTEMPLATE *)buffer, owner, prompt_proc, 0);
 if (r == 1 && field) { char *s = narrow(PROMPT.field); snprintf(field, field_capacity, "%s", s ? s : ""); free(s); }
 free(wtitle); free(wtext); return r == 1;
}

/* ---- the Recycle Bin: never a permanent delete ---- */
/* FOF_ALLOWUNDO recycles; FOF_WANTNUKEWARNING asks before a file that cannot be
 * recycled (another drive, too large) would be destroyed, so the player can
 * cancel; no other confirmation (the manager already asked). */
static int recycle(void *ctx, const char *path, char *reason, size_t capacity) {
 wchar_t *w = widen(path); if (!w) { snprintf(reason, capacity, "sem memória"); return 1; }
 size_t n = wcslen(w); wchar_t *from = calloc(n + 2, sizeof(wchar_t)); /* double NUL */
 if (!from) { free(w); snprintf(reason, capacity, "sem memória"); return 1; }
 memcpy(from, w, n * sizeof(wchar_t)); free(w);
 SHFILEOPSTRUCTW op; memset(&op, 0, sizeof(op));
 op.hwnd = (HWND)ctx; op.wFunc = FO_DELETE; op.pFrom = from;
 op.fFlags = FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_WANTNUKEWARNING | FOF_SILENT | FOF_NOERRORUI;
 int c = SHFileOperationW(&op); free(from);
 if (c) { snprintf(reason, capacity, "erro %d do Windows", c); return 1; }
 if (op.fAnyOperationsAborted) { snprintf(reason, capacity, "operação cancelada"); return 1; }
 return 0;
}

/* ---- the manager window ---- */
static struct { const char *directory; RlwSaveList list; int changed; HWND dialog; } M;

static int selected(void) {
 int row = (int)SendDlgItemMessageW(M.dialog, ID_LIST, LVM_GETNEXTITEM, (WPARAM)-1, LVNI_SELECTED);
 return row >= 0 && (unsigned)row < M.list.count ? row : -1;
}
static void update_buttons(void) {
 int row = selected();
 EnableWindow(GetDlgItem(M.dialog, ID_RENAME), row >= 0 && rlw_savemgr_can_rename(&M.list.game[row]));
 EnableWindow(GetDlgItem(M.dialog, ID_DELETE), row >= 0);
}
static void set_cell(HWND list, int row, int column, const char *text) {
 wchar_t *w = widen(text); LVITEMW item; memset(&item, 0, sizeof(item));
 item.iSubItem = column; item.pszText = w ? w : L"";
 SendMessageW(list, LVM_SETITEMTEXTW, (WPARAM)row, (LPARAM)&item); free(w);
}
/* reload(select:): the previous selection kept by slot, else the first row. */
static void reload(const char *keep) {
 char previous[32] = ""; int row = selected();
 if (keep) snprintf(previous, sizeof(previous), "%s", keep); else if (row >= 0) snprintf(previous, sizeof(previous), "%s", M.list.game[row].slot);
 rlw_savemgr_list(M.directory, &M.list);
 HWND list = GetDlgItem(M.dialog, ID_LIST); SendMessageW(list, LVM_DELETEALLITEMS, 0, 0);
 int choose = M.list.count ? 0 : -1;
 for (unsigned i = 0; i < M.list.count; i++) {
  const RlwSavedGame *g = &M.list.game[i];
  LVITEMW item; memset(&item, 0, sizeof(item)); item.mask = LVIF_TEXT; item.iItem = (int)i; item.pszText = L"";
  SendMessageW(list, LVM_INSERTITEMW, 0, (LPARAM)&item);
  char stage[64], date[32]; rlw_savemgr_stage_title(g, stage, sizeof(stage)); rlw_savemgr_date(g->mtime, date, sizeof(date));
  set_cell(list, (int)i, 0, g->name); set_cell(list, (int)i, 1, stage); set_cell(list, (int)i, 2, date);
  if (previous[0] && !strcmp(previous, g->slot)) choose = (int)i;
 }
 if (choose >= 0) {
  LVITEMW state; memset(&state, 0, sizeof(state)); state.stateMask = LVIS_SELECTED | LVIS_FOCUSED; state.state = LVIS_SELECTED | LVIS_FOCUSED;
  SendMessageW(list, LVM_SETITEMSTATE, (WPARAM)choose, (LPARAM)&state); SendMessageW(list, LVM_ENSUREVISIBLE, (WPARAM)choose, FALSE);
 }
 ShowWindow(GetDlgItem(M.dialog, ID_EMPTY), M.list.count ? SW_HIDE : SW_SHOW);
 update_buttons();
}
static void report(const char *error) { message(M.dialog, error, "Não foi possível concluir a operação"); }

static void rename_selected(void) {
 int row = selected(); if (row < 0 || !rlw_savemgr_can_rename(&M.list.game[row])) return;
 RlwSavedGame game = M.list.game[row];
 char title[400], text[400], field[RLW_SAVE_NAME_CAPACITY * 2], error[600];
 snprintf(title, sizeof(title), "Renomear “%s”", game.name);
 snprintf(text, sizeof(text), "É o nome que as listas Salvar e Carregar do jogo mostram. Elas exibem os primeiros %d caracteres, seguidos da data.", RLW_SAVEMGR_VISIBLE_CHARACTERS);
 snprintf(field, sizeof(field), "%s", game.name);
 if (!prompt(M.dialog, title, text, L"Renomear", field, sizeof(field))) return;
 if (rlw_savemgr_rename(M.directory, &game, field, error, sizeof(error))) { reload(NULL); report(error); return; }
 M.changed = 1; reload(game.slot);
}
static void delete_selected(void) {
 int row = selected(); if (row < 0) return;
 RlwSavedGame game = M.list.game[row];
 char title[400], text[1200], stage[64], date[32], error[900];
 rlw_savemgr_stage_title(&game, stage, sizeof(stage)); rlw_savemgr_date(game.mtime, date, sizeof(date));
 snprintf(title, sizeof(title), "Apagar “%s”?", game.name);
 snprintf(text, sizeof(text), "%s%s · %s. A partida vai para a Lixeira; enquanto a Lixeira não for esvaziada, ela pode ser recuperada pelo Explorador de Arquivos, junto do arquivo .view.",
          game.kind == RLW_SAVE_STAGE ? "Este é o salvamento automático do início da fase. Sem ele, Carregar partida ▸ Início da fase fica indisponível até a próxima fase começar.\n\n" : "",
          stage, date);
 if (!prompt(M.dialog, title, text, L"Mover para a Lixeira", NULL, 0)) return;
 int c = rlw_savemgr_trash(M.directory, &game, recycle, (void *)M.dialog, error, sizeof(error));
 M.changed = 1; reload(NULL);
 if (c) report(error);
}

static INT_PTR CALLBACK manager_proc(HWND dialog, UINT msg, WPARAM wp, LPARAM lp) {
 switch (msg) {
 case WM_INITDIALOG: {
  M.dialog = dialog; HWND list = GetDlgItem(dialog, ID_LIST);
  SendMessageW(list, LVM_SETEXTENDEDLISTVIEWSTYLE, 0, LVS_EX_FULLROWSELECT | LVS_EX_GRIDLINES);
  RECT r; GetClientRect(list, &r); int width = r.right - r.left - GetSystemMetrics(SM_CXVSCROLL);
  static const wchar_t *const headings[3] = {L"Nome", L"Fase", L"Data"}; static const int share[3] = {46, 27, 27};
  for (int i = 0; i < 3; i++) {
   LVCOLUMNW column; memset(&column, 0, sizeof(column)); column.mask = LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM;
   column.pszText = (LPWSTR)headings[i]; column.cx = width * share[i] / 100; column.iSubItem = i;
   SendMessageW(list, LVM_INSERTCOLUMNW, (WPARAM)i, (LPARAM)&column);
  }
  SetWindowPos(GetDlgItem(dialog, ID_EMPTY), HWND_TOP, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
  reload(NULL); SetFocus(list); return FALSE;
 }
 case WM_NOTIFY: {
  NMHDR *h = (NMHDR *)lp; if (h->idFrom != ID_LIST) break;
  if (h->code == LVN_ITEMCHANGED) { update_buttons(); return TRUE; }
  if (h->code == NM_DBLCLK) { rename_selected(); return TRUE; }
  if (h->code == LVN_KEYDOWN && ((NMLVKEYDOWN *)lp)->wVKey == VK_DELETE) { delete_selected(); return TRUE; }
  if (h->code == NM_CUSTOMDRAW) {
   /* Fixed names (quick save, autosaves) in grey, as the macOS list. */
   NMLVCUSTOMDRAW *cd = (NMLVCUSTOMDRAW *)lp; LRESULT result = CDRF_DODEFAULT;
   if (cd->nmcd.dwDrawStage == CDDS_PREPAINT) result = CDRF_NOTIFYITEMDRAW;
   else if (cd->nmcd.dwDrawStage == CDDS_ITEMPREPAINT) result = CDRF_NOTIFYSUBITEMDRAW;
   else if (cd->nmcd.dwDrawStage == (CDDS_ITEMPREPAINT | CDDS_SUBITEM)) {
    size_t row = (size_t)cd->nmcd.dwItemSpec;
    cd->clrText = cd->iSubItem == 0 && row < M.list.count && !rlw_savemgr_can_rename(&M.list.game[row]) ? GetSysColor(COLOR_GRAYTEXT) : GetSysColor(COLOR_WINDOWTEXT);
   }
   SetWindowLongPtrW(dialog, DWLP_MSGRESULT, result); return TRUE;
  }
  break;
 }
 case WM_COMMAND:
  if (LOWORD(wp) == ID_RENAME) { rename_selected(); SetFocus(GetDlgItem(dialog, ID_LIST)); return TRUE; }
  if (LOWORD(wp) == ID_DELETE) { delete_selected(); SetFocus(GetDlgItem(dialog, ID_LIST)); return TRUE; }
  if (LOWORD(wp) == IDCANCEL || LOWORD(wp) == IDOK) { EndDialog(dialog, 0); return TRUE; }
  break;
 }
 return FALSE;
}

int rlw_savemgr_dialog_run(HWND owner, const char *directory) {
 INITCOMMONCONTROLSEX controls; controls.dwSize = sizeof(controls); controls.dwICC = ICC_LISTVIEW_CLASSES; InitCommonControlsEx(&controls);
 memset(&M, 0, sizeof(M)); M.directory = directory;
 static DWORD buffer[2048]; memset(buffer, 0, sizeof(buffer));
 WORD *p = put_header(buffer, 6, 330, 214, L"Partidas salvas");
 p = put_item(p, WS_BORDER | WS_TABSTOP | LVS_REPORT | LVS_SINGLESEL | LVS_SHOWSELALWAYS, 7, 7, 316, 146, ID_LIST, 0, L"SysListView32", L"");
 p = put_item(p, SS_CENTER, 7, 76, 316, 10, ID_EMPTY, 0x0082, NULL, L"Nenhuma partida salva.");
 p = put_item(p, SS_LEFT, 7, 158, 316, 20, ID_NOTE, 0x0082, NULL, L"As partidas apagadas vão para a Lixeira e podem ser recuperadas pelo Explorador de Arquivos.");
 p = put_item(p, WS_TABSTOP | BS_PUSHBUTTON, 7, 192, 70, 14, ID_RENAME, 0x0080, NULL, L"Renomear…");
 p = put_item(p, WS_TABSTOP | BS_PUSHBUTTON, 81, 192, 70, 14, ID_DELETE, 0x0080, NULL, L"Apagar…");
 put_item(p, WS_TABSTOP | BS_PUSHBUTTON, 253, 192, 70, 14, IDCANCEL, 0x0080, NULL, L"Fechar");
 DialogBoxIndirectParamW(GetModuleHandleW(NULL), (DLGTEMPLATE *)buffer, owner, manager_proc, 0);
 return M.changed;
}
