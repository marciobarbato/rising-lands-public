/* Automated smokes without game data (see rlw_smoke.h). Win32 only. */
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
#include "rlw_markers.h"
#include "rlw_markers_gdi.h"
#include "rlw_smoke.h"
#include "rlw_dpi.h"

/* ---------- --smoke-markers ---------- */

static int failures;
static void check(int ok, const char *what) { printf("  %s %s\n", ok ? "ok  " : "FALHA", what); if (!ok) failures++; }

typedef struct { HDC dc; HBITMAP bitmap; HGDIOBJ old; uint32_t *bits; int width, height; } Canvas;
static int canvas_open(Canvas *c, int width, int height) {
 memset(c, 0, sizeof(*c));
 BITMAPINFO bi; memset(&bi, 0, sizeof(bi));
 bi.bmiHeader.biSize = sizeof(bi.bmiHeader); bi.bmiHeader.biWidth = width; bi.bmiHeader.biHeight = -height;
 bi.bmiHeader.biPlanes = 1; bi.bmiHeader.biBitCount = 32; bi.bmiHeader.biCompression = BI_RGB;
 HDC screen = GetDC(NULL);
 c->dc = CreateCompatibleDC(screen);
 void *bits = NULL;
 c->bitmap = c->dc ? CreateDIBSection(screen, &bi, DIB_RGB_COLORS, &bits, NULL, 0) : NULL;
 ReleaseDC(NULL, screen);
 if (!c->dc || !c->bitmap || !bits) return 1;
 c->old = SelectObject(c->dc, c->bitmap); c->bits = (uint32_t *)bits; c->width = width; c->height = height;
 memset(bits, 0, (size_t)width * (size_t)height * 4);
 return 0;
}
static void canvas_close(Canvas *c) {
 if (c->old) SelectObject(c->dc, c->old);
 if (c->bitmap) DeleteObject(c->bitmap);
 if (c->dc) DeleteDC(c->dc);
 memset(c, 0, sizeof(*c));
}
static uint32_t pixel(const Canvas *c, int x, int y) {
 if (x < 0 || y < 0 || x >= c->width || y >= c->height) return 0xffffffffu;
 return c->bits[(size_t)y * (size_t)c->width + (size_t)x] & 0xffffffu;
}
static int colour_is(uint32_t v, int r, int g, int b) {
 int vr = (int)(v >> 16 & 255), vg = (int)(v >> 8 & 255), vb = (int)(v & 255);
 return abs(vr - r) <= 8 && abs(vg - g) <= 8 && abs(vb - b) <= 8;
}
static int yellow(uint32_t v) { return colour_is(v, 255, 217, 51); }
/* Some pixel of the (2r+1)^2 neighbourhood is the marker colour. */
static int yellow_near(const Canvas *c, int x, int y, int r) {
 for (int dy = -r; dy <= r; dy++) for (int dx = -r; dx <= r; dx++) if (yellow(pixel(c, x + dx, y + dy))) return 1;
 return 0;
}

static uint8_t picture[800 * 600];
static RlwMarkerShape shapes[RLW_MARKER_SHAPES];

static void paint_case(const char *name, const RlwMarkers *m, const BITMAPINFO *bmi, int cw, int ch, int left, int top, int width, int height) {
 Canvas c; char what[256];
 printf("%s (%dx%d em %d,%d)\n", name, width, height, left, top);
 if (canvas_open(&c, cw, ch)) { check(0, "DIB section"); return; }
 check(rlw_markers_paint(c.dc, m, left, top, width, height, picture, bmi) == 1, "rlw_markers_paint desenhou");
 GdiFlush();
 RlwMarkerRect clip;
 int n = rlw_markers_layout(m, left, top, width, height, shapes, RLW_MARKER_SHAPES, &clip);
 int rects = 0, dots = 0, poles = 0, dashes = 0;
 for (int i = 0; i < n; i++) {
  const RlwMarkerShape *s = &shapes[i];
  int cx = (s->x0 + s->x1) / 2, cy = (s->y0 + s->y1) / 2;
  int inside = cx >= clip.left && cx < clip.right && cy >= clip.top && cy < clip.bottom;
  if (!inside) continue;
  if (s->kind == RLW_SHAPE_RECT) rects += yellow(pixel(&c, cx, cy));
  else if (s->kind == RLW_SHAPE_ELLIPSE) dots += yellow(pixel(&c, cx, cy));
  else if (s->kind == RLW_SHAPE_LINE) poles += yellow_near(&c, s->x0, (s->y0 + s->y1) / 2, 1);
  else {
   /* Dashed: along the segment some samples are the colour and some are not. */
   int on = 0, total = 0;
   for (int t = 10; t <= 90; t++) {
    int x = s->x0 + (s->x1 - s->x0) * t / 100, y = s->y0 + (s->y1 - s->y0) * t / 100;
    if (x < clip.left || x >= clip.right || y < clip.top || y >= clip.bottom) continue;
    total++; on += yellow(pixel(&c, x, y - 1)) || yellow(pixel(&c, x, y)) || yellow(pixel(&c, x, y + 1));
   }
   dashes += total > 0 && on * 10 >= total * 2 && on < total;
  }
 }
 snprintf(what, sizeof(what), "bandeiras preenchidas (%d)", rects); check(rects == 2, what);
 snprintf(what, sizeof(what), "pontos preenchidos (%d)", dots); check(dots == 2, what);
 snprintf(what, sizeof(what), "mastros (%d)", poles); check(poles == 2, what);
 snprintf(what, sizeof(what), "linha tracejada entre os pontos (%d)", dashes); check(dashes == 1, what);
 /* Below the map region (the HUD) and outside the picture nothing changes. */
 int hud_marked = 0, hud_wrong = 0, bars = 0;
 for (int y = clip.bottom; y < top + height; y++)
  for (int x = left; x < left + width; x++) { uint32_t v = pixel(&c, x, y); hud_marked += yellow(v); hud_wrong += !colour_is(v, 128, 128, 128); }
 for (int y = 0; y < ch; y++) for (int x = 0; x < cw; x++)
  if (x < left || x >= left + width || y < top || y >= top + height) bars += pixel(&c, x, y) != 0;
 check(hud_marked == 0 && hud_wrong == 0, "HUD abaixo do mapa intacto (marcador abaixo do mapa recortado)");
 check(bars == 0, "nada fora do retângulo da imagem");
 check(colour_is(pixel(&c, left + width * 7 / 8, top + height / 2), 0, 96, 0), "mapa sem marcador intacto");
 canvas_close(&c);
}

int rlw_smoke_markers(void) {
 failures = 0;
 BITMAPINFO *bmi = calloc(1, sizeof(BITMAPINFOHEADER) + 256 * sizeof(RGBQUAD));
 if (!bmi) return 1;
 bmi->bmiHeader.biSize = sizeof(BITMAPINFOHEADER); bmi->bmiHeader.biWidth = 800; bmi->bmiHeader.biHeight = -600;
 bmi->bmiHeader.biPlanes = 1; bmi->bmiHeader.biBitCount = 8; bmi->bmiHeader.biCompression = BI_RGB; bmi->bmiHeader.biClrUsed = 256;
 bmi->bmiColors[1].rgbGreen = 96;
 bmi->bmiColors[2].rgbRed = bmi->bmiColors[2].rgbGreen = bmi->bmiColors[2].rgbBlue = 128;
 /* Map (index 1) over the view's 17 rows of cells, HUD (index 2) below. */
 memset(picture, 1, 800 * 544); memset(picture + 800 * 544, 2, 800 * 56);
 RlwMarkers m; rlw_markers_clear(&m);
 m.map_width = 800; m.map_height = 544;
 m.path_count = 2;
 m.paths[0].count = 2; m.paths[0].points[0].x = 112; m.paths[0].points[0].y = 112; m.paths[0].points[1].x = 368; m.paths[0].points[1].y = 112;
 m.paths[1].count = 1; m.paths[1].points[0].x = 400; m.paths[1].points[0].y = 576; /* under the map: clipped */
 /* A flag and dot of path 1 are fully under the map; the counts above expect 2. */
 paint_case("1:1", &m, bmi, 800, 600, 0, 0, 800, 600);
 paint_case("2x com barras laterais", &m, bmi, 1760, 1200, 80, 0, 1600, 1200);
 paint_case("janela pequena", &m, bmi, 400, 340, 0, 20, 400, 300);
 /* Nothing to mark: nothing is drawn, the caller blits as before. */
 Canvas c; RlwMarkers none; rlw_markers_clear(&none);
 printf("sem marcadores\n");
 if (!canvas_open(&c, 800, 600)) {
  check(rlw_markers_paint(c.dc, &none, 0, 0, 800, 600, picture, bmi) == 0 && pixel(&c, 400, 300) == 0, "nada desenhado");
  canvas_close(&c);
 } else check(0, "DIB section");
 free(bmi);
 printf("smoke markers %s: %d falha(s)\n", failures ? "FALHOU" : "ok", failures);
 return failures ? 1 : 0;
}

/* ---------- --smoke-window ---------- */

static HWND test_window(const wchar_t *title) {
 static int registered;
 HINSTANCE instance = GetModuleHandleW(NULL);
 if (!registered) {
  WNDCLASSEXW wc; memset(&wc, 0, sizeof(wc)); wc.cbSize = sizeof(wc);
  wc.lpfnWndProc = DefWindowProcW; wc.hInstance = instance; wc.lpszClassName = L"RisingLandsSmokeWindow";
  if (!RegisterClassExW(&wc)) return NULL;
  registered = 1;
 }
 RECT r = {0, 0, 800, 600}; AdjustWindowRect(&r, WS_OVERLAPPEDWINDOW, FALSE);
 return CreateWindowExW(0, L"RisingLandsSmokeWindow", title, WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT,
                        r.right - r.left, r.bottom - r.top, NULL, NULL, instance, NULL);
}

int rlw_smoke_window(void) {
 char what[256]; failures = 0;
 RlwDpiScope scope = rlw_dpi_begin_per_monitor();
 int supported = scope.active;
 HWND aware = test_window(L"Rising Lands smoke (per-monitor)");
 rlw_dpi_end(scope);
 scope = rlw_dpi_begin_unaware();
 HWND unaware = test_window(L"Rising Lands smoke (unaware)");
 rlw_dpi_end(scope);
 check(aware && unaware, "janelas criadas");
 if (!aware || !unaware) { if (aware) DestroyWindow(aware); if (unaware) DestroyWindow(unaware); return 1; }
 printf("  contexto por monitor disponível: %s; DPI da janela %u\n", supported ? "sim" : "não (Windows antigo)", rlw_dpi_of(aware));
 if (supported) {
  check(rlw_dpi_window_is_per_monitor(aware) == 1, "janela do jogo ciente de DPI por monitor (v2)");
  check(rlw_dpi_window_is_per_monitor(unaware) == 0, "janela de layout fixo sem DPI (escalada pelo Windows)");
 }
 /* Client area 800x600 scaled to the window's DPI. */
 int ww, wh; unsigned dpi = rlw_dpi_of(aware);
 rlw_dpi_window_size(aware, 800, 600, WS_OVERLAPPEDWINDOW, FALSE, 0, &ww, &wh);
 SetWindowPos(aware, NULL, 0, 0, ww, wh, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
 RECT c; GetClientRect(aware, &c);
 snprintf(what, sizeof(what), "área cliente %ldx%ld para 800x600 a %u dpi", (long)c.right, (long)c.bottom, dpi);
 check(c.right == MulDiv(800, (int)dpi, 96) && c.bottom == MulDiv(600, (int)dpi, 96), what);
 /* Borderless fullscreen covers exactly the window's monitor. */
 SetWindowLongPtrW(aware, GWL_STYLE, (LONG_PTR)(WS_POPUP | WS_VISIBLE));
 ShowWindow(aware, SW_SHOWNOACTIVATE);
 MONITORINFO mi; memset(&mi, 0, sizeof(mi)); mi.cbSize = sizeof(mi);
 GetMonitorInfoW(MonitorFromWindow(aware, MONITOR_DEFAULTTONEAREST), &mi);
 check(rlw_fit_monitor(aware) == 0, "rlw_fit_monitor");
 RECT w; GetWindowRect(aware, &w);
 snprintf(what, sizeof(what), "tela cheia (%ld,%ld)-(%ld,%ld) = monitor (%ld,%ld)-(%ld,%ld)", (long)w.left, (long)w.top, (long)w.right,
          (long)w.bottom, (long)mi.rcMonitor.left, (long)mi.rcMonitor.top, (long)mi.rcMonitor.right, (long)mi.rcMonitor.bottom);
 check(EqualRect(&w, &mi.rcMonitor), what);
 DestroyWindow(aware); DestroyWindow(unaware);
 printf("smoke window %s: %d falha(s)\n", failures ? "FALHOU" : "ok", failures);
 return failures ? 1 : 0;
}

/* ---------- --smoke-import-prompt: dialog watcher ---------- */

static struct {
 HANDLE thread; DWORD ui_thread; volatile LONG stop;
 HWND seen[8]; DWORD since[8]; int seen_count;
 int closed; char titles[512];
} W;

static BOOL CALLBACK watch_window(HWND hwnd, LPARAM lp) {
 (void)lp;
 wchar_t cls[64]; if (!IsWindowVisible(hwnd) || !GetClassNameW(hwnd, cls, 64) || wcscmp(cls, L"#32770")) return TRUE;
 DWORD now = GetTickCount();
 for (int i = 0; i < W.seen_count; i++) if (W.seen[i] == hwnd) {
  /* Shown for 300 ms: close it as the player's Cancel/Esc would. */
  if (W.since[i] && now - W.since[i] >= 300) { W.since[i] = 0; PostMessageW(hwnd, WM_CLOSE, 0, 0); W.closed++; }
  return TRUE;
 }
 if (W.seen_count < 8) {
  W.seen[W.seen_count] = hwnd; W.since[W.seen_count] = now ? now : 1; W.seen_count++;
  wchar_t title[200]; char utf8[400]; int n = GetWindowTextW(hwnd, title, 200); title[n > 0 ? n : 0] = 0;
  WideCharToMultiByte(CP_UTF8, 0, title, -1, utf8, sizeof(utf8), NULL, NULL);
  size_t len = strlen(W.titles);
  snprintf(W.titles + len, sizeof(W.titles) - len, "%s%s", len ? "; " : "", utf8);
 }
 return TRUE;
}

static DWORD WINAPI watcher(LPVOID arg) {
 (void)arg; DWORD start = GetTickCount();
 while (!InterlockedCompareExchange(&W.stop, 0, 0)) {
  EnumThreadWindows(W.ui_thread, watch_window, 0);
  if (GetTickCount() - start > 60000) {
   printf("smoke import prompt: watchdog (60 s), diálogos vistos: %s\n", W.titles); fflush(stdout);
   ExitProcess(9);
  }
  Sleep(100);
 }
 return 0;
}

void rlw_smoke_dialogs_begin(void) {
 memset(&W, 0, sizeof(W)); W.ui_thread = GetCurrentThreadId();
 W.thread = CreateThread(NULL, 0, watcher, NULL, 0, NULL);
}

int rlw_smoke_dialogs_end(void) {
 InterlockedExchange(&W.stop, 1);
 if (W.thread) { WaitForSingleObject(W.thread, 5000); CloseHandle(W.thread); W.thread = NULL; }
 printf("diálogos fechados: %d (%s)\n", W.closed, W.titles);
 return W.closed;
}
