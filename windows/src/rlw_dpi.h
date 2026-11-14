/* DPI handling of the Windows host (mixed mode), Win32 only.
 *
 * The game window is per-monitor DPI aware (v2, Windows 10 1703+): on a screen
 * scaled above 100 % the 800x600 picture is stretched once, by StretchDIBits,
 * into real device pixels (sharp, the same nearest-neighbour aspect-fit as at
 * 100 %), instead of being stretched again and blurred by the window manager;
 * fullscreen covers the monitor's real resolution. Mouse coordinates and the
 * viewport are both in device pixels, so the mapping is unchanged.
 *
 * Windows built with fixed pixel layouts (CreateWindowEx dialogs such as the
 * key window) are created inside rlw_dpi_begin_unaware() and stay DPI unaware:
 * Windows scales them as it did before. Template dialogs (DialogBox*), message
 * boxes and the file dialog scale themselves under per-monitor v2. On older
 * Windows every call is a no-op and the process stays unaware, as before. */
#ifndef RLW_DPI_H
#define RLW_DPI_H
#include <windows.h>
#ifndef WM_DPICHANGED
#define WM_DPICHANGED 0x02E0
#endif

typedef struct { HANDLE previous; int active; } RlwDpiScope;
/* Windows created by this thread inside the scope are per-monitor v2 aware. */
RlwDpiScope rlw_dpi_begin_per_monitor(void);
/* Windows created by this thread inside the scope are DPI unaware. */
RlwDpiScope rlw_dpi_begin_unaware(void);
void rlw_dpi_end(RlwDpiScope scope);
/* The window's DPI (96 = 100 %), 96 when unknown. */
unsigned rlw_dpi_of(HWND window);
/* 1 when the window is per-monitor v2 aware, 0 when not (or unknown). */
int rlw_dpi_window_is_per_monitor(HWND window);
/* Outer size of a window whose client area is client_w x client_h at 100 %,
 * scaled to the window's DPI (AdjustWindowRectExForDpi). */
void rlw_dpi_window_size(HWND window, int client_w, int client_h, DWORD style, BOOL menu, DWORD ex, int *w, int *h);
/* WM_DPICHANGED for a windowed (not fullscreen) window: move to the suggested rect. */
void rlw_dpi_changed(HWND window, LPARAM suggested);
/* Borderless fullscreen: the window covers the monitor it is on. 0 or -1. */
int rlw_fit_monitor(HWND window);
#endif
