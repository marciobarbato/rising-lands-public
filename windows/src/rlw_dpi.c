/* DPI handling of the Windows host (see rlw_dpi.h). The Windows 10 functions are
 * looked up at run time, so the .exe still starts on systems without them. */
#define WIN32_LEAN_AND_MEAN
#ifndef UNICODE
#define UNICODE
#endif
#include "rlw_dpi.h"

typedef HANDLE (WINAPI *SetContextFn)(HANDLE);
typedef UINT (WINAPI *DpiForWindowFn)(HWND);
typedef HANDLE (WINAPI *WindowContextFn)(HWND);
typedef BOOL (WINAPI *ContextsEqualFn)(HANDLE, HANDLE);
typedef BOOL (WINAPI *AdjustForDpiFn)(LPRECT, DWORD, BOOL, DWORD, UINT);

/* DPI_AWARENESS_CONTEXT_UNAWARE (-1) and _PER_MONITOR_AWARE_V2 (-4). */
#define CONTEXT_UNAWARE ((HANDLE)(INT_PTR)-1)
#define CONTEXT_PER_MONITOR_V2 ((HANDLE)(INT_PTR)-4)

static struct { int loaded; SetContextFn set; DpiForWindowFn dpi; WindowContextFn context; ContextsEqualFn equal; AdjustForDpiFn adjust; } F;

static void load(void) {
 if (F.loaded) return;
 F.loaded = 1;
 HMODULE user32 = GetModuleHandleW(L"user32.dll");
 if (!user32) return;
 F.set = (SetContextFn)(void (*)(void))GetProcAddress(user32, "SetThreadDpiAwarenessContext");
 F.dpi = (DpiForWindowFn)(void (*)(void))GetProcAddress(user32, "GetDpiForWindow");
 F.context = (WindowContextFn)(void (*)(void))GetProcAddress(user32, "GetWindowDpiAwarenessContext");
 F.equal = (ContextsEqualFn)(void (*)(void))GetProcAddress(user32, "AreDpiAwarenessContextsEqual");
 F.adjust = (AdjustForDpiFn)(void (*)(void))GetProcAddress(user32, "AdjustWindowRectExForDpi");
}

static RlwDpiScope begin(HANDLE context) {
 RlwDpiScope s; s.previous = NULL; s.active = 0;
 load();
 if (F.set) { s.previous = F.set(context); s.active = s.previous != NULL; }
 return s;
}
RlwDpiScope rlw_dpi_begin_per_monitor(void) { return begin(CONTEXT_PER_MONITOR_V2); }
RlwDpiScope rlw_dpi_begin_unaware(void) { return begin(CONTEXT_UNAWARE); }
void rlw_dpi_end(RlwDpiScope scope) { if (scope.active && F.set) F.set(scope.previous); }

unsigned rlw_dpi_of(HWND window) {
 load();
 UINT dpi = F.dpi && window ? F.dpi(window) : 0;
 return dpi ? dpi : 96;
}

int rlw_dpi_window_is_per_monitor(HWND window) {
 load();
 if (!F.context || !F.equal || !window) return 0;
 HANDLE c = F.context(window);
 return c && F.equal(c, CONTEXT_PER_MONITOR_V2) ? 1 : 0;
}

void rlw_dpi_window_size(HWND window, int client_w, int client_h, DWORD style, BOOL menu, DWORD ex, int *w, int *h) {
 unsigned dpi = rlw_dpi_of(window);
 RECT r; r.left = 0; r.top = 0; r.right = MulDiv(client_w, (int)dpi, 96); r.bottom = MulDiv(client_h, (int)dpi, 96);
 if (!(F.adjust && F.adjust(&r, style, menu, ex, dpi))) AdjustWindowRectEx(&r, style, menu, ex);
 *w = r.right - r.left; *h = r.bottom - r.top;
}

void rlw_dpi_changed(HWND window, LPARAM suggested) {
 const RECT *s = (const RECT *)suggested;
 if (s) SetWindowPos(window, NULL, s->left, s->top, s->right - s->left, s->bottom - s->top, SWP_NOZORDER | SWP_NOACTIVATE);
}

int rlw_fit_monitor(HWND window) {
 MONITORINFO mi; ZeroMemory(&mi, sizeof(mi)); mi.cbSize = sizeof(mi);
 if (!GetMonitorInfoW(MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST), &mi)) return -1;
 return SetWindowPos(window, HWND_TOP, mi.rcMonitor.left, mi.rcMonitor.top, mi.rcMonitor.right - mi.rcMonitor.left,
                     mi.rcMonitor.bottom - mi.rcMonitor.top, SWP_NOOWNERZORDER | SWP_FRAMECHANGED) ? 0 : -1;
}
