/* The original icon on the window, the Start menu and the desktop (Win32). The
 * .ico comes from rlw_icon_prepare (built at run time from the player's
 * imported RISING.EXE; the repository holds no icon bytes). Shortcuts point
 * their icon at that .ico rather than at RISING.EXE,0: the file is a plain
 * icon any shell reads, while the 1997 Watcom executable is parsed only by us. */
#define COBJMACROS
#include "rlw_icon_win.h"
#include "rlw_icon.h"
#include "rlw_fs.h"
#include <objbase.h>
#include <shlobj.h>
#include <shobjidl.h>
#include <stdio.h>
#include <stdlib.h>

static HICON big_icon, small_icon;
static char root[4096];

static wchar_t *widen(const char *s) {
 int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
 wchar_t *w = malloc((size_t)(n > 0 ? n : 1) * sizeof(wchar_t));
 if (w) { if (n > 0) MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n); else w[0] = 0; }
 return w;
}

/* "<folder>\Rising Lands.lnk": this executable, the original icon. An existing
 * shortcut is kept unless `replace`. 0 when created or present. */
static int shortcut(int csidl, const wchar_t *ico, int replace) {
 wchar_t exe[MAX_PATH], folder[MAX_PATH], link[MAX_PATH + 32], dir[MAX_PATH];
 DWORD n = GetModuleFileNameW(NULL, exe, MAX_PATH);
 if (!n || n >= MAX_PATH) return 1;
 if (FAILED(SHGetFolderPathW(NULL, csidl, NULL, SHGFP_TYPE_CURRENT, folder))) return 1;
 if (_snwprintf(link, MAX_PATH + 32, L"%ls\\Rising Lands.lnk", folder) < 0) return 1;
 link[MAX_PATH + 31] = 0;
 if (!replace && GetFileAttributesW(link) != INVALID_FILE_ATTRIBUTES) return 0;
 wcscpy(dir, exe); wchar_t *slash = wcsrchr(dir, L'\\');
 if (slash) *slash = 0;
 HRESULT init = CoInitializeEx(NULL, COINIT_APARTMENTTHREADED);
 IShellLinkW *shell = NULL; IPersistFile *file = NULL;
 HRESULT h = CoCreateInstance(&CLSID_ShellLink, NULL, CLSCTX_INPROC_SERVER, &IID_IShellLinkW, (void **)&shell);
 if (SUCCEEDED(h)) {
  IShellLinkW_SetPath(shell, exe); IShellLinkW_SetWorkingDirectory(shell, dir);
  IShellLinkW_SetDescription(shell, L"Rising Lands (dados e ícone do CD do jogador)");
  IShellLinkW_SetIconLocation(shell, ico, 0);
  h = IShellLinkW_QueryInterface(shell, &IID_IPersistFile, (void **)&file);
  if (SUCCEEDED(h)) { h = IPersistFile_Save(file, link, TRUE); IPersistFile_Release(file); }
  IShellLinkW_Release(shell);
 }
 if (SUCCEEDED(init)) CoUninitialize();
 return SUCCEEDED(h) ? 0 : 1;
}

int rlw_icon_setup(HWND window, const char *data_root, int players_run) {
 snprintf(root, sizeof(root), "%s", data_root ? data_root : "");
 char path[4200]; int created = 0;
 if (rlw_icon_prepare(root, path, sizeof(path), &created)) return 1;
 wchar_t *w = widen(path);
 if (!w) return 1;
 HICON icon_big = (HICON)LoadImageW(NULL, w, IMAGE_ICON, GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON), LR_LOADFROMFILE);
 HICON icon_small = (HICON)LoadImageW(NULL, w, IMAGE_ICON, GetSystemMetrics(SM_CXSMICON), GetSystemMetrics(SM_CYSMICON), LR_LOADFROMFILE);
 if (icon_big) { SendMessageW(window, WM_SETICON, ICON_BIG, (LPARAM)icon_big); if (big_icon) DestroyIcon(big_icon); big_icon = icon_big; }
 if (icon_small) { SendMessageW(window, WM_SETICON, ICON_SMALL, (LPARAM)icon_small); if (small_icon) DestroyIcon(small_icon); small_icon = icon_small; }
 /* Once, when the .ico was just made (the first run after the import, or the
  * first run of this version): the Start-menu shortcut, and the question about
  * the desktop one (Jogo > Criar atalho na Área de Trabalho does it later). */
 if (created && players_run) {
  (void)shortcut(CSIDL_PROGRAMS, w, 0);
  if (MessageBoxW(window, L"Criar um atalho na Área de Trabalho com o ícone do jogo?", L"Rising Lands", MB_ICONQUESTION | MB_YESNO) == IDYES &&
      shortcut(CSIDL_DESKTOPDIRECTORY, w, 0))
   MessageBoxW(window, L"Não foi possível criar o atalho na Área de Trabalho.", L"Rising Lands", MB_ICONWARNING | MB_OK);
 }
 free(w); return icon_big || icon_small ? 0 : 1;
}

int rlw_icon_desktop_shortcut(HWND owner) {
 char path[4200]; int created = 0;
 if (rlw_icon_prepare(root, path, sizeof(path), &created)) {
  MessageBoxW(owner, L"O ícone do jogo não foi encontrado no CD importado; o atalho não foi criado.", L"Rising Lands", MB_ICONWARNING | MB_OK); return 1;
 }
 wchar_t *w = widen(path);
 int c = w ? shortcut(CSIDL_DESKTOPDIRECTORY, w, 1) : 1; free(w);
 MessageBoxW(owner, c ? L"Não foi possível criar o atalho na Área de Trabalho." : L"Atalho \"Rising Lands\" criado na Área de Trabalho.", L"Rising Lands",
             (c ? MB_ICONWARNING : MB_ICONINFORMATION) | MB_OK);
 return c;
}
