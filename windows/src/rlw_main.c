/* Rising Lands Windows host: import the player's CD, start in the original main
 * menu (milestone 2) or a given stage, present the 800x600 indexed frame with its
 * palette, map mouse/keyboard to the same Win32-style messages as the macOS host,
 * pace 1x = 30 frames/s, pause. The original menus (main, single player, briefing,
 * pause 16, result 19, objectives, credits) run on the engine's prepared shell
 * through rlw_menu.c. Saves (milestone 3): the "Partida" menu (quick save F5,
 * quick load F9, named positions, the stage-start autosave), the original save
 * and load lists, files in the macOS app's format (rlw_saves.h). Win32 + GDI
 * only. The host never writes game state. */
#define WIN32_LEAN_AND_MEAN
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif
#include <windows.h>
#include <mmsystem.h>
#include <windowsx.h>
#include <commdlg.h>
#include <shellapi.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "rlw_cd.h"
#include "rlw_demo.h"
#include "rlw_driver.h"
#include "rlw_fs.h"
#include "rlw_hotkeys.h"
#include "rlw_keys_dialog.h"
#include "rlw_menu.h"
#include "rlw_menu_ops.h"
#include "rlw_modern.h"
#include "rlw_saves.h"
#include "rlw_sha256.h"
#include "rising_engine.h"

/* NativeGameSpeed: 1x = 30 frames/s; 0.5x/2x/4x; one engine frame per tick. */
static const double SPEEDS[4] = {0.5, 1, 2, 4};
static const double NORMAL_FRAME_RATE = 30.0, MAXIMUM_BACKLOG = 0.25;

typedef struct {
 const char *cd, *data, *mission, *hashes, *screenshot, *menu_demo, *save_directory, *save_demo;
 unsigned difficulty, frames, speed;
 int selftest, import_only, headless, fullscreen, no_verify_image, demo_input;
 int difficulty_given, speed_given; /* explicit --difficulty/--speed win over opcoes.ini */
} Options;

static struct {
 HWND hwnd; RlwDriver *driver; Options opt;
 int paused, fullscreen, stopped_reported, focused;
 unsigned speed; double deadline; LARGE_INTEGER frequency;
 WINDOWPLACEMENT placement; RECT view; double scale;
 int32_t last_x, last_y; int left_held;
 BITMAPINFO *bmi; char status_line[256];
 /* Original menus: routing state, next 20 Hz tick, last coalesced move. */
 char dados[4096]; RlwMenu menu; double menu_tick, menu_flush; int menu_failed, quit;
 /* Saves: the player's folder ("" in smoke runs: no autosave), the menu bar,
  * a short notice in the title ("Partida salva"). */
 char saves[4096]; HMENU menubar, save_menu, load_menu; char notice[96]; double notice_until;
 /* The port's "Opções" screen: settings kept in <user data>/opcoes.ini (players'
  * runs only; "" in smoke/demo runs, which keep the original defaults). */
 char options_path[4200]; unsigned atalhos, caminhos;
 /* Keyboard bindings and "Atalhos modernos" (rlw_hotkeys.h, rlw_modern.h): the
  * router, the running command (between frames, on the driver it started on),
  * the player's pointer, a command waiting for the left button's release. */
 RlwRouter router; RlwModernJob job; RlwPlanner planner; RlwDriver *job_driver; int job_active;
 double synthetic_until; int32_t pointer_x, pointer_y; int pending_modern; uint32_t pending_digit;
 RlwSnapshot snapshot;
 /* Bindings file beside opcoes.ini (players' runs only). */
 char keys_path[4200];
} G = {0};

/* ---------- utilities ---------- */

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
static void message_box(const char *text, const char *title, UINT flags) {
 wchar_t *t = widen(text), *c = widen(title);
 MessageBoxW(G.hwnd, t, c, flags); free(t); free(c);
}
static double now_seconds(void) {
 LARGE_INTEGER t; QueryPerformanceCounter(&t);
 return (double)t.QuadPart / (double)G.frequency.QuadPart;
}
static int console_attached;
static void attach_console(void) {
 if (console_attached) return;
 console_attached = 1;
 /* Redirected output (CI pipes, wine, files) is already usable as is. */
 DWORD type = GetFileType(GetStdHandle(STD_OUTPUT_HANDLE));
 if (type == FILE_TYPE_DISK || type == FILE_TYPE_PIPE) return;
 /* Started from a console: print there. A player's double-click opens none. */
 if (AttachConsole(ATTACH_PARENT_PROCESS)) {
  freopen("CONOUT$", "w", stdout); freopen("CONOUT$", "w", stderr);
 }
}

/* ---------- presentation (GDI, 8-bit DIB with the engine palette) ---------- */

static void update_viewport(void) {
 RECT c; GetClientRect(G.hwnd, &c);
 int w = c.right - c.left, h = c.bottom - c.top;
 double sx = w / 800.0, sy = h / 600.0; G.scale = sx < sy ? sx : sy;
 if (G.scale <= 0) G.scale = 1;
 int vw = (int)(800 * G.scale + 0.5), vh = (int)(600 * G.scale + 0.5);
 G.view.left = (w - vw) / 2; G.view.top = (h - vh) / 2;
 G.view.right = G.view.left + vw; G.view.bottom = G.view.top + vh;
}

static void present(HDC dc) {
 if (!G.driver) return;
 const uint8_t *pixels = rlw_driver_pixels(G.driver), *palette = rlw_driver_palette(G.driver);
 for (int i = 0; i < 256; i++) {
  G.bmi->bmiColors[i].rgbRed = palette[3 * i]; G.bmi->bmiColors[i].rgbGreen = palette[3 * i + 1];
  G.bmi->bmiColors[i].rgbBlue = palette[3 * i + 2]; G.bmi->bmiColors[i].rgbReserved = 0;
 }
 RECT c; GetClientRect(G.hwnd, &c);
 HBRUSH black = (HBRUSH)GetStockObject(BLACK_BRUSH);
 RECT bars[4] = {{0, 0, c.right, G.view.top}, {0, G.view.bottom, c.right, c.bottom},
                 {0, G.view.top, G.view.left, G.view.bottom}, {G.view.right, G.view.top, c.right, G.view.bottom}};
 for (int i = 0; i < 4; i++) if (bars[i].right > bars[i].left && bars[i].bottom > bars[i].top) FillRect(dc, &bars[i], black);
 SetStretchBltMode(dc, COLORONCOLOR);
 StretchDIBits(dc, G.view.left, G.view.top, G.view.right - G.view.left, G.view.bottom - G.view.top,
               0, 0, 800, 600, pixels, G.bmi, DIB_RGB_COLORS, SRCCOPY);
}

static void present_now(void) { HDC dc = GetDC(G.hwnd); present(dc); ReleaseDC(G.hwnd, dc); }

static void update_title(void) {
 RlwStatus s; memset(&s, 0, sizeof(s)); if (G.driver) rlw_driver_status(G.driver, &s);
 static const char *labels[4] = {"0,5x", "1x", "2x", "4x"};
 char title[384];
 uint32_t mode = G.driver ? rlw_shell_mode(G.driver) : 0;
 if (mode) snprintf(title, sizeof(title), "Rising Lands — %s", mode == RL_SHELL_PAUSE ? "pausa" : mode == 19 ? "resultado" :
                    mode == RL_SHELL_BRIEFING ? "briefing" : mode == RL_SHELL_OBJECTIVES ? "objetivos" : "menu");
 else snprintf(title, sizeof(title), "Rising Lands — %s · %s%s · quadro %llu",
          G.opt.mission ? G.opt.mission : "campanha", labels[G.speed], G.paused ? " · pausado (P)" : "",
          (unsigned long long)s.frames);
 if (s.stopped || G.menu_failed) snprintf(title, sizeof(title), "Rising Lands — parado");
 else if (G.notice[0] && now_seconds() < G.notice_until) { size_t n = strlen(title); snprintf(title + n, sizeof(title) - n, " · %s", G.notice); }
 wchar_t *w = widen(title); SetWindowTextW(G.hwnd, w); free(w);
}

static void set_fullscreen(int on) {
 DWORD style = (DWORD)GetWindowLongPtrW(G.hwnd, GWL_STYLE);
 if (on && !G.fullscreen) {
  MONITORINFO mi; memset(&mi, 0, sizeof(mi)); mi.cbSize = sizeof(mi);
  if (GetWindowPlacement(G.hwnd, &G.placement) && GetMonitorInfoW(MonitorFromWindow(G.hwnd, MONITOR_DEFAULTTOPRIMARY), &mi)) {
   SetWindowLongPtrW(G.hwnd, GWL_STYLE, style & ~WS_OVERLAPPEDWINDOW); SetMenu(G.hwnd, NULL);
   SetWindowPos(G.hwnd, HWND_TOP, mi.rcMonitor.left, mi.rcMonitor.top, mi.rcMonitor.right - mi.rcMonitor.left,
                mi.rcMonitor.bottom - mi.rcMonitor.top, SWP_NOOWNERZORDER | SWP_FRAMECHANGED);
   G.fullscreen = 1;
  }
 } else if (!on && G.fullscreen) {
  SetWindowLongPtrW(G.hwnd, GWL_STYLE, style | WS_OVERLAPPEDWINDOW); SetMenu(G.hwnd, G.menubar);
  SetWindowPlacement(G.hwnd, &G.placement);
  SetWindowPos(G.hwnd, NULL, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED);
  G.fullscreen = 0;
 }
 update_viewport(); InvalidateRect(G.hwnd, NULL, FALSE);
}

/* ---------- input (NativeLiveWindow / NativeHotkeyInputRouter parity) ---------- */

static void report_stop(void) {
 RlwStatus s; rlw_driver_status(G.driver, &s);
 if (!s.stopped || G.stopped_reported) return;
 G.stopped_reported = 1; update_title();
 char text[700]; snprintf(text, sizeof(text), "O jogo parou.\n\n%s", s.message);
 attach_console(); fprintf(stderr, "stop: %s\n", s.message);
 if (!G.opt.headless && !G.opt.frames) message_box(text, "Rising Lands", MB_ICONWARNING | MB_OK);
}

static void send_event(uint32_t kind, uint32_t msg, int32_t x, int32_t y, uint32_t wp, uint32_t vk, uint32_t down) {
 if (!G.driver) return;
 if (rlw_driver_event(G.driver, kind, msg, x, y, wp, vk, down)) report_stop();
}

/* Window point -> original pixel; NULL-equivalent (0) for the letterbox bars. */
static int original_pixel(int px, int py, int clamp, int32_t *x, int32_t *y) {
 double fx = (px - G.view.left) / G.scale, fy = (py - G.view.top) / G.scale;
 int ix = (int)(fx < 0 ? fx - 1 : fx), iy = (int)(fy < 0 ? fy - 1 : fy); /* floor */
 if (clamp) {
  *x = ix < 0 ? 0 : ix > 799 ? 799 : ix; *y = iy < 0 ? 0 : iy > 599 ? 599 : iy; return 1;
 }
 if (px < G.view.left || px >= G.view.right || py < G.view.top || py >= G.view.bottom || ix < 0 || ix > 799 || iy < 0 || iy > 599) return 0;
 *x = ix; *y = iy; return 1;
}

static uint32_t mouse_wparam(void) {
 return ((GetKeyState(VK_SHIFT) & 0x8000) ? 4u : 0u) | ((GetKeyState(VK_CONTROL) & 0x8000) ? 8u : 0u);
}

/* Shift/Control/Alt are independent channels (VK 16/17/18), synchronized before
 * each mouse event exactly like synchronizeModifiers(). */
static void synchronize_modifiers(void) {
 static const int vks[3] = {VK_SHIFT, VK_CONTROL, VK_MENU};
 for (int i = 0; i < 3; i++) {
  int down = (GetKeyState(vks[i]) & 0x8000) != 0;
  if (G.driver && down != rlw_driver_key_held(G.driver, (uint32_t)(16 + i))) send_event(2, 0, 0, 0, 0, (uint32_t)(16 + i), (uint32_t)down);
 }
}

static int synthetic_active(void);
static void start_modern(int command, uint32_t digit);
static void mouse(uint32_t msg, LPARAM lp, int held, int hover) {
 int32_t x, y;
 if (!original_pixel(GET_X_LPARAM(lp), GET_Y_LPARAM(lp), held || hover, &x, &y)) return;
 G.pointer_x = x; G.pointer_y = y;
 /* While a synthetic mouse command runs, the player's mouse only moves the
  * remembered pointer, so no player event interleaves with it. */
 if (synthetic_active()) return;
 synchronize_modifiers();
 if (msg == 0x201 || msg == 0x203) G.left_held = 1; else if (msg == 0x202) G.left_held = 0;
 G.last_x = x; G.last_y = y;
 send_event(1, msg, x, y, mouse_wparam(), 0, 0);
 /* A modern command waits for the player's held button to be released. */
 if (msg == 0x202 && G.pending_modern) { int command = G.pending_modern; G.pending_modern = 0; start_modern(command, G.pending_digit); }
}

/* Focus loss: release a held press at its last pixel, release every held key,
 * and pull a pointer resting in an edge-scroll zone 2 px inward. */
static void release_all(void) {
 if (!G.driver) return;
 rlw_router_reset(&G.router); G.pending_modern = 0;
 if (G.left_held) { G.left_held = 0; send_event(1, 0x202, G.last_x, G.last_y, 0, 0, 0); }
 for (uint32_t vk = 0; vk < 256; vk++) if (rlw_driver_key_held(G.driver, vk)) send_event(2, 0, 0, 0, 0, vk, 0);
 int32_t x = G.last_x < 2 ? 2 : G.last_x > 797 ? 797 : G.last_x, y = G.last_y < 2 ? 2 : G.last_y > 597 ? 597 : G.last_y;
 if (x != G.last_x || y != G.last_y) { G.last_x = x; G.last_y = y; send_event(1, 0x200, x, y, 0, 0, 0); }
 if (GetCapture() == G.hwnd) ReleaseCapture();
}

static int accepted_vk(uint32_t vk) { return rlw_accepted_vk(vk); }
static uint32_t held_modifiers(void) {
 return ((GetKeyState(VK_SHIFT) & 0x8000) ? RLW_MOD_SHIFT : 0u) | ((GetKeyState(VK_CONTROL) & 0x8000) ? RLW_MOD_CONTROL : 0u) |
        ((GetKeyState(VK_MENU) & 0x8000) ? RLW_MOD_ALT : 0u);
}
/* The router's game channel: no duplicate down, no up without down. */
static void router_emit(void *ctx, uint32_t vk, int down) {
 (void)ctx; if (!G.driver || down == rlw_driver_key_held(G.driver, vk)) return;
 send_event(2, 0, 0, 0, 0, vk, (uint32_t)down);
}
static void router_modern(void *ctx, int command, uint32_t digit) { (void)ctx; start_modern(command, digit); }

/* ---------- "Atalhos modernos" (NativeLiveWindow's startModern/stepModern) ---------- */

/* Ignored in menus/briefing/pause (shell), while paused or stopped. */
static int modern_allowed(void) {
 if (!G.driver || G.menu_failed || G.paused || rlw_shell_mode(G.driver) != 0) return 0;
 RlwStatus s; rlw_driver_status(G.driver, &s); return !s.stopped;
}
static int synthetic_active(void) { return G.job_active && rlw_job_uses_mouse(&G.job) && now_seconds() < G.synthetic_until; }
static int modern_peek(void *ctx, const uint32_t *a, const uint32_t *sz, uint32_t n, uint8_t *out) { (void)ctx; return rlw_driver_peek(G.driver, a, sz, n, out); }
static const RlwSnapshot *modern_snapshot(void *ctx) { (void)ctx; return rlw_snapshot_read(modern_peek, NULL, &G.snapshot) ? NULL : &G.snapshot; }
/* The player's own input path (rlw_driver_event) for synthetic input. */
static int modern_deliver(void *ctx, const RlwStep *step) {
 (void)ctx; if (!modern_allowed()) return 1;
 int c = step->type == RLW_STEP_KEY ? rlw_driver_event(G.driver, 2, 0, 0, 0, 0, step->code, step->down)
                                    : rlw_driver_event(G.driver, 1, step->code, step->x, step->y, 0, 0, 0);
 if (c) { report_stop(); return 1; }
 return 0;
}
static uint32_t modern_pending(void *ctx) { (void)ctx; return rlw_driver_pending_input(G.driver); }
static uint64_t modern_frames(void *ctx) { (void)ctx; RlwStatus s; rlw_driver_status(G.driver, &s); return s.frames; }
static void modern_pointer(void *ctx, int *x, int *y) { (void)ctx; *x = G.pointer_x; *y = G.pointer_y; }
static const RlwModernHost MODERN_HOST = {NULL, modern_deliver, modern_pending, modern_frames, modern_snapshot, modern_pointer};

/* After every advance: the next stage of the running command. Menus, a stop, a
 * pause or a replaced driver end it; nothing more is delivered. */
static void step_modern(void) {
 if (!G.job_active) return;
 if (G.job_driver != G.driver) { G.job_active = 0; return; }
 int allowed = modern_allowed();
 if (allowed) rlw_job_pump(&G.job, &MODERN_HOST);
 if (allowed && G.job.state == RLW_JOB_RUNNING) return;
 G.job_active = 0;
}
static void start_modern(int command, uint32_t digit) {
 if (G.left_held) { G.pending_modern = command; G.pending_digit = digit; return; }
 if (G.job_active || !modern_allowed()) return;
 RlwStep steps[5]; int n = 0;
 if (command == RLW_MODERN_CENTER_GROUP) n = rlw_modern_center_group(digit, steps);
 else if (command == RLW_MODERN_LAST_ALERT) n = rlw_modern_last_alert(steps);
 if (n) rlw_job_steps(&G.job, steps, n);
 else {
  RlwTarget target; const RlwSnapshot *s = modern_snapshot(NULL);
  if (!s || !rlw_planner_choose(&G.planner, command, s, &target)) return;
  rlw_job_target(&G.job, target); G.synthetic_until = now_seconds() + 3;
 }
 G.job_active = 1; G.job_driver = G.driver;
 step_modern();
}

/* ---------- "Opções" settings (opcoes.ini beside Dados and Partidas) ---------- */
static void options_store(void) {
 if (!G.options_path[0]) return;
 char text[160]; int n = snprintf(text, sizeof(text), "atalhos=%u\r\ncaminhos=%u\r\ndificuldade=%u\r\nvelocidade=%u\r\n",
                                  G.atalhos, G.caminhos, G.opt.difficulty, G.speed);
 if (n > 0) (void)rlw_durable_write(G.options_path, (const uint8_t *)text, (size_t)n, NULL, 0);
}
/* Missing file or keys keep the defaults (set by the caller: the macOS app's). */
static void options_load(void) {
 FILE *f = G.options_path[0] ? rlw_fopen(G.options_path, "rb") : NULL; if (!f) return;
 char line[96];
 while (fgets(line, sizeof(line), f)) {
  unsigned v = 0; if (sscanf(line, "%*[^=]=%u", &v) != 1) continue;
  if (!strncmp(line, "atalhos=", 8) && v <= 1) G.atalhos = v;
  else if (!strncmp(line, "caminhos=", 9) && v <= 1) G.caminhos = v;
  else if (!strncmp(line, "dificuldade=", 12) && v <= 2 && !G.opt.difficulty_given) G.opt.difficulty = v;
  else if (!strncmp(line, "velocidade=", 11) && v <= 3 && !G.opt.speed_given) G.opt.speed = v;
 }
 fclose(f);
}
/* NativeEngineRules.current(): "Atalhos modernos" = the Shift+click order queue
 * (bit 1), "Caminhos melhores" = modern unit paths (bit 0). Smoke/demo runs keep
 * both off (no opcoes.ini), so the original rules stay byte for byte. */
static uint32_t options_rules(void) { return rlw_options_rules(G.atalhos, G.caminhos); }
static void apply_rules(void) { rlw_driver_default_rules(options_rules()); (void)rlw_driver_set_rules(G.driver, options_rules()); }

/* ---------- key bindings (teclas.ini) and the "Atalhos de teclado" window ---------- */
/* loadHotkeys(): a missing file keeps the defaults; an invalid one too (reported). */
static void keys_load(void) {
 if (!G.keys_path[0] || !rlw_exists(G.keys_path)) return;
 uint32_t n = 0; uint8_t *text = rlw_read_file(G.keys_path, &n, 65536);
 RlwBindings b; RlwBindError e = {RLW_BIND_INVALID_FILE, -1, -1};
 if (text && !rlw_bindings_parse((const char *)text, n, &b, &e)) rlw_router_install(&G.router, &b);
 else { char why[512]; rlw_bind_error_message(&e, why, sizeof(why)); attach_console(); fprintf(stderr, "teclas.ini: %s\n", why); }
 free(text);
}
/* prepareInstallation + save: written first; only a saved set is installed. */
static int keys_commit(void *ctx, const RlwBindings *candidate) {
 (void)ctx; char text[4096]; rlw_bindings_format(candidate, text, sizeof(text));
 if (rlw_durable_write(G.keys_path, (const uint8_t *)text, strlen(text), NULL, 0)) return 1;
 release_all(); rlw_router_install(&G.router, candidate);
 return 0;
}
static void keys_closed(void *ctx) { (void)ctx; G.deadline = now_seconds(); SetFocus(G.hwnd); }
static void show_keys(void) {
 if (!G.keys_path[0] || !G.hwnd) return;
 RlwKeysDialogHost host = {NULL, keys_commit, keys_closed};
 if (rlw_keys_dialog_open(G.hwnd, &G.router.bindings, &host)) message_box("Não foi possível abrir a janela de atalhos.", "Rising Lands", MB_ICONWARNING | MB_OK);
}

static void set_speed(unsigned speed) { G.speed = speed > 3 ? 3 : speed; G.deadline = now_seconds(); update_title(); options_store(); }
static void command(unsigned id);
enum { ID_QUICK_SAVE = 100, ID_QUICK_LOAD, ID_STAGE_LOAD, ID_SAVE_POSITION = 110, ID_LOAD_POSITION = 120, ID_KEYS = 130 };

/* Host keys never reach the game: F11 (fullscreen; also never delivered per the
 * host contract), P/Pause (pause), +/- (speed), F5/F9 (quick save/load, the
 * macOS app's Cmd+S/Cmd+L). Shift/Control/Alt are the game's own channels.
 * Every other key goes through the bindings (rlw_hotkeys.h): a bound action
 * emits one of the 23 accepted keys, a modern action starts its command when
 * "Atalhos modernos" is on. Returns 1 when handled. */
static int key(WPARAM wp, LPARAM lp, int down) {
 uint32_t vk = (uint32_t)wp;
 if (down && (lp & (1 << 30))) { /* auto-repeat: never emits */
  uint32_t k = vk == VK_LSHIFT || vk == VK_RSHIFT ? 16 : vk;
  if (k == VK_F11 || k == VK_F5 || k == VK_F9 || (k >= 16 && k <= 18)) return 1;
  return rlw_router_key_down(&G.router, k, held_modifiers(), 1, now_seconds()) || accepted_vk(k);
 }
 if (vk == VK_F11) { if (down) set_fullscreen(!G.fullscreen); return 1; }
 if (vk == VK_F5 || vk == VK_F9) { if (down) command(vk == VK_F5 ? ID_QUICK_SAVE : ID_QUICK_LOAD); return 1; }
 if (vk == VK_PAUSE || vk == 'P') { if (down) { G.paused = !G.paused; G.deadline = now_seconds(); update_title(); } return 1; }
 if (vk == VK_ADD || vk == VK_OEM_PLUS) { if (down && G.speed < 3) set_speed(G.speed + 1); return 1; }
 if (vk == VK_SUBTRACT || vk == VK_OEM_MINUS) { if (down && G.speed > 0) set_speed(G.speed - 1); return 1; }
 if (vk == VK_LSHIFT || vk == VK_RSHIFT) vk = VK_SHIFT;
 if (vk == VK_LCONTROL || vk == VK_RCONTROL) vk = VK_CONTROL;
 if (vk == VK_LMENU || vk == VK_RMENU) vk = VK_MENU;
 if (!G.driver) return 0;
 if (vk < 16 || vk > 18) {
  int handled = down ? rlw_router_key_down(&G.router, vk, held_modifiers(), 0, now_seconds()) : rlw_router_key_up(&G.router, vk);
  return handled || accepted_vk(vk);
 }
 if (!down && (GetKeyState((int)vk) & 0x8000)) return 1; /* the other side still held */
 if (down == rlw_driver_key_held(G.driver, vk)) return 1; /* no duplicate down, no up without down */
 send_event(2, 0, 0, 0, 0, vk, (uint32_t)down);
 return 1;
}

/* ---------- original menus (NativeLiveWindow shell paths) ---------- */

static int in_menu(void) { return G.driver && !G.menu_failed && rlw_shell_mode(G.driver) != 0; }
/* publishSlots(): captions of posicao-1..10 for the original save lists. */
static void publish_slots(void *ctx, uint8_t table[330]) { (void)ctx; if (G.saves[0]) rlw_saves_caption_table(G.saves, table); else memset(table, 0, 330); }

static void menu_attach(void) {
 char credits[4200]; snprintf(credits, sizeof(credits), "%s" RLW_SEP "events" RLW_SEP "CREDITS.FLC", G.dados);
 rlw_menu_close(&G.menu); rlw_menu_init(&G.menu, &RLW_WINDOWS_OPS.menu, G.driver, credits);
 G.menu.publish_slots = publish_slots;
 /* The port's "Opções" button over the main and pause menus (players' runs). */
 if (G.options_path[0]) {
  RlOpcoesValores v; memset(&v, 0, sizeof(v));
  v.version = RL_OPCOES_VERSION; v.atalhos = G.atalhos; v.caminhos = G.caminhos; v.dificuldade = G.opt.difficulty; v.velocidade = G.speed;
  /* "Configurar teclas" opens the "Atalhos de teclado" window (show_keys). */
  v.disponivel = RL_OPCOES_TODOS; v.idioma = 0;
  (void)rlw_options_sync(G.driver, &v);
 }
 /* startShell(): the first shellTick comes 50 ms after the session opens. */
 G.menu_tick = now_seconds() + 0.05; G.menu_flush = 0;
}

static void menu_failed(const char *why) {
 G.menu_failed = 1; update_title();
 attach_console(); fprintf(stderr, "menu: %s\n", why);
 char text[700]; snprintf(text, sizeof(text), "O jogo parou.\n\n%s", why);
 if (!G.opt.headless && !G.opt.frames) message_box(text, "Rising Lands", MB_ICONWARNING | MB_OK);
}

/* reopenMenu(): a main-menu item the host cannot complete yet boots the main
 * menu again, as at launch (the pending session is discarded). */
static void reopen_menu(const char *title, const char *detail) {
 message_box(detail, title, MB_ICONINFORMATION | MB_OK);
 char error[1024] = {0};
 RlwDriver *fresh = rlw_driver_create_menu(G.dados, G.opt.difficulty, error, sizeof(error));
 if (!fresh) { menu_failed(error); return; }
 rlw_driver_free(G.driver); G.driver = fresh; menu_attach(); present_now(); update_title();
}

/* Movie name from the original CD-root path (\events\introfr.avi -> INTROFR.AVI). */
static int movie_exists(const RlShellEvent *e) {
 char name[64] = {0}; uint32_t start = 0, n = e->path_length < sizeof(e->path) ? e->path_length : (uint32_t)sizeof(e->path);
 for (uint32_t i = 0; i < n; i++) if (e->path[i] == '\\' || e->path[i] == '/') start = i + 1;
 uint32_t k = 0; for (uint32_t i = start; i < n && k + 1 < sizeof(name); i++) { char c = (char)e->path[i]; name[k++] = c >= 'a' && c <= 'z' ? (char)(c - 32) : c; }
 char path[4200]; snprintf(path, sizeof(path), "%s" RLW_SEP "movies" RLW_SEP "%s", G.dados, name);
 return k > 0 && rlw_exists(path);
}

static void back_to_game(void) {
 G.left_held = 0; G.deadline = now_seconds(); G.paused = 0;
 if (GetCapture() == G.hwnd) ReleaseCapture();
 present_now(); update_title();
}

static void menu_result(int r);

/* ---------- saves (NativeLiveWindow saveSlot/commitSave/loadSlot, main.swift menus) ---------- */

static struct {
 HANDLE thread; RlwSaveJob *job; volatile LONG code;    /* one background write at a time */
 unsigned named; char name[RLW_SAVE_NAME_CAPACITY];      /* position named after its write */
 char retry_slot[32]; unsigned retry_named; char retry_name[RLW_SAVE_NAME_CAPACITY]; int retries; double retry_at;
} SV;

static void notice(const char *text) { snprintf(G.notice, sizeof(G.notice), "%s", text); G.notice_until = now_seconds() + 4; update_title(); }
static int game_running(void) { RlwStatus s; rlw_driver_status(G.driver, &s); return !in_menu() && !G.menu_failed && !G.paused && !s.stopped; }

static void commit_finished(int code, RlwSaveJob *job) {
 char error[512]; rlw_save_job_error(job, error, sizeof(error)); rlw_save_job_free(job);
 if (code) { message_box(error, "Não foi possível salvar a partida", MB_ICONWARNING | MB_OK); SV.named = 0; return; }
 if (SV.named) {
  RlwSaveNames names; rlw_saves_names_read(G.saves, &names);
  snprintf(names.name[SV.named], sizeof(names.name[SV.named]), "%s", SV.name); SV.named = 0;
  if (rlw_saves_names_write(G.saves, &names)) message_box("A partida foi salva, mas o nome da posição não pôde ser gravado.", "Salvar partida", MB_ICONWARNING | MB_OK);
 }
 notice("Partida salva");
}
static DWORD WINAPI commit_thread(LPVOID unused) { (void)unused; InterlockedExchange(&SV.code, rlw_save_commit(SV.job)); return 0; }
/* The captured job is written off the game loop; success only after flush and
 * replace. wait = 1 before a load or quit (never leave a captured save unwritten). */
static void save_poll(int wait) {
 if (!SV.thread || WaitForSingleObject(SV.thread, wait ? INFINITE : 0) != WAIT_OBJECT_0) return;
 CloseHandle(SV.thread); SV.thread = NULL; RlwSaveJob *job = SV.job; SV.job = NULL;
 commit_finished((int)SV.code, job);
}
static void start_commit(RlwSaveJob *job, unsigned named, const char *name) {
 SV.job = job; SV.code = 0; SV.named = named; snprintf(SV.name, sizeof(SV.name), "%s", name ? name : "");
 SV.thread = CreateThread(NULL, 0, commit_thread, NULL, 0, NULL);
 if (!SV.thread) { SV.job = NULL; commit_finished(rlw_save_commit(job), job); }
}
static void save_failed(const char *title) {
 char why[512]; rlw_driver_save_error(G.driver, why, sizeof(why));
 message_box(why[0] ? why : "Erro desconhecido", title, MB_ICONWARNING | MB_OK);
}
/* saveSlot(): releases the held keys first (their releases are queued input,
 * so the save is busy until a frame took them); a busy save is retried every
 * 80 ms while the game runs, 30 times at most. */
static void save_attempt(const char *slot, unsigned named, const char *name, int retries) {
 SV.retry_slot[0] = 0;
 save_poll(1);
 if (!G.saves[0] || rlw_mkdirs(G.saves)) { message_box("A pasta das partidas salvas está indisponível.", "Não foi possível salvar a partida", MB_ICONWARNING | MB_OK); return; }
 char path[4200]; if (rlw_saves_path(G.saves, slot, path, sizeof(path))) return;
 RlwSaveJob *job = NULL; int c = rlw_driver_save_begin(G.driver, path, &job);
 if (c == RL_SAVE_BUSY && game_running() && retries > 0) {
  snprintf(SV.retry_slot, sizeof(SV.retry_slot), "%s", slot); SV.retry_named = named;
  snprintf(SV.retry_name, sizeof(SV.retry_name), "%s", name ? name : ""); SV.retries = retries - 1; SV.retry_at = now_seconds() + 0.08;
  return;
 }
 if (c) { save_failed("Não foi possível concluir a operação"); return; }
 start_commit(job, named, name);
}
static void save_now(const char *slot, unsigned named, const char *name) {
 if (!in_menu()) release_all();
 save_attempt(slot, named, name, 30);
}
/* loadSlot(): waits for a save still being written, then replaces world,
 * renderer and picture together; a failure leaves the current game as it was.
 * A save made from the pause list reopens the pause at the next host tick. */
static int load_slot(const char *slot, int report) {
 char path[4200];
 if (!G.saves[0] || rlw_saves_path(G.saves, slot, path, sizeof(path))) return RL_INVALID_ARGUMENT;
 SV.retry_slot[0] = 0; save_poll(1);
 if (!in_menu()) release_all();
 int c = rlw_driver_load(G.driver, path);
 if (c) { if (report) save_failed("Não foi possível carregar a partida"); return c; }
 rlw_menu_reset(&G.menu); G.menu_failed = 0; G.stopped_reported = 0; G.left_held = 0;
 if (GetCapture() == G.hwnd) ReleaseCapture();
 G.deadline = now_seconds(); notice("Partida carregada"); present_now(); update_title();
 return 0;
}
/* The stage-start autosave (advance stageStart / finishShellCampaign). */
static void stage_autosave(void) {
 if (!G.saves[0]) return;
 char path[4200]; int c = rlw_mkdirs(G.saves) || rlw_saves_path(G.saves, "inicio-fase", path, sizeof(path)) ? RL_IO_ERROR : rlw_driver_autosave(G.driver, path);
 if (c) { G.paused = 1; update_title(); if (G.opt.frames) { attach_console(); fprintf(stderr, "autosave: %d\n", c); } else save_failed("Falha ao salvar automaticamente"); }
}

/* Position name (namedSave): the macOS app's alert with a text field, as an
 * in-memory dialog template (no resource file). */
static wchar_t name_prompt[160], name_value[96];
static INT_PTR CALLBACK name_dialog(HWND dialog, UINT msg, WPARAM wp, LPARAM lp) {
 (void)lp;
 switch (msg) {
 case WM_INITDIALOG:
  SetDlgItemTextW(dialog, 100, name_prompt); SetDlgItemTextW(dialog, 101, name_value);
  SendDlgItemMessageW(dialog, 101, EM_LIMITTEXT, 80, 0); SendDlgItemMessageW(dialog, 101, EM_SETSEL, 0, -1);
  SetFocus(GetDlgItem(dialog, 101)); return FALSE;
 case WM_COMMAND:
  if (LOWORD(wp) == IDOK) { GetDlgItemTextW(dialog, 101, name_value, 96); EndDialog(dialog, 1); return TRUE; }
  if (LOWORD(wp) == IDCANCEL) { EndDialog(dialog, 0); return TRUE; }
  break;
 }
 return FALSE;
}
static WORD *dialog_text(WORD *p, const wchar_t *text) { do { *p++ = (WORD)*text; } while (*text++); return p; }
static WORD *dialog_item(WORD *p, DWORD style, short x, short y, short cx, short cy, WORD id, WORD atom, const wchar_t *text) {
 p = (WORD *)(((uintptr_t)p + 3) & ~(uintptr_t)3);
 DLGITEMTEMPLATE *item = (DLGITEMTEMPLATE *)p; item->style = style | WS_CHILD | WS_VISIBLE; item->dwExtendedStyle = 0;
 item->x = x; item->y = y; item->cx = cx; item->cy = cy; item->id = id;
 p = (WORD *)(item + 1); *p++ = 0xffff; *p++ = atom; p = dialog_text(p, text); *p++ = 0; return p;
}
static int ask_name(unsigned n, char *out, size_t capacity) {
 char path[4200], slot[32]; snprintf(slot, sizeof(slot), "posicao-%u", n);
 int exists = !rlw_saves_path(G.saves, slot, path, sizeof(path)) && rlw_exists(path);
 swprintf(name_prompt, 160, exists ? L"O salvamento desta posição será substituído. Dê um nome à partida:" : L"Dê um nome à partida:");
 RlwSaveNames names; rlw_saves_names_read(G.saves, &names);
 if (names.name[n][0]) { wchar_t *w = widen(names.name[n]); swprintf(name_value, 96, L"%ls", w ? w : L""); free(w); }
 else {
  char mission[16]; snprintf(mission, sizeof(mission), "%s", rlw_driver_mission(G.driver)); char *dot = strchr(mission, '.'); if (dot) *dot = 0;
  wchar_t *w = widen(mission); swprintf(name_value, 96, L"%ls · posição %u", w ? w : L"", n); free(w);
 }
 static DWORD buffer[1024]; memset(buffer, 0, sizeof(buffer));
 DLGTEMPLATE *t = (DLGTEMPLATE *)buffer;
 t->style = DS_MODALFRAME | DS_CENTER | DS_SETFONT | WS_POPUP | WS_CAPTION | WS_SYSMENU; t->cdit = 4; t->cx = 240; t->cy = 74;
 WORD *p = (WORD *)(t + 1); *p++ = 0; *p++ = 0;
 wchar_t title[64]; swprintf(title, 64, L"Salvar partida · posição %u", n); p = dialog_text(p, title);
 *p++ = 9; p = dialog_text(p, L"Segoe UI");
 p = dialog_item(p, SS_LEFT, 8, 8, 224, 18, 100, 0x0082, L"");
 p = dialog_item(p, WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL, 8, 28, 224, 13, 101, 0x0081, L"");
 p = dialog_item(p, WS_TABSTOP | BS_DEFPUSHBUTTON, 128, 52, 50, 14, IDOK, 0x0080, L"Salvar");
 dialog_item(p, WS_TABSTOP | BS_PUSHBUTTON, 182, 52, 50, 14, IDCANCEL, 0x0080, L"Cancelar");
 if (DialogBoxIndirectParamW(GetModuleHandleW(NULL), t, G.hwnd, name_dialog, 0) != 1) return 0;
 char *name = narrow(name_value); if (!name) return 0;
 char *b = name; while (*b == ' ' || *b == '\t') b++;
 size_t len = strlen(b); while (len && (b[len - 1] == ' ' || b[len - 1] == '\t')) b[--len] = 0;
 if (*b) snprintf(out, capacity, "%s", b); else snprintf(out, capacity, "Posição %u", n);
 free(name); return 1;
}

/* refreshSaveMenus(): positions 1..5 with their names and times; loads only of
 * existing files; the stage-start autosave below the load positions. */
static void refresh_save_menus(void) {
 while (GetMenuItemCount(G.save_menu) > 0) DeleteMenu(G.save_menu, 0, MF_BYPOSITION);
 while (GetMenuItemCount(G.load_menu) > 0) DeleteMenu(G.load_menu, 0, MF_BYPOSITION);
 int usable = G.saves[0] && !SV.thread && !G.menu_failed;
 for (unsigned n = 1; n <= 5; n++) {
  char title[400], slot[32], path[4200]; rlw_saves_title(G.saves, n, title, sizeof(title)); snprintf(slot, sizeof(slot), "posicao-%u", n);
  int exists = !rlw_saves_path(G.saves, slot, path, sizeof(path)) && rlw_exists(path);
  wchar_t *w = widen(title);
  AppendMenuW(G.save_menu, MF_STRING | (usable && !in_menu() ? 0 : MF_GRAYED), ID_SAVE_POSITION + n, w);
  AppendMenuW(G.load_menu, MF_STRING | (usable && exists ? 0 : MF_GRAYED), ID_LOAD_POSITION + n, w);
  free(w);
 }
 char path[4200]; int stage = G.saves[0] && !rlw_saves_path(G.saves, "inicio-fase", path, sizeof(path)) && rlw_exists(path);
 AppendMenuW(G.load_menu, MF_SEPARATOR, 0, NULL);
 AppendMenuW(G.load_menu, MF_STRING | (usable && stage ? 0 : MF_GRAYED), ID_STAGE_LOAD, L"Início da fase · salvamento automático");
 int quick = G.saves[0] && !rlw_saves_path(G.saves, "rapido", path, sizeof(path)) && rlw_exists(path);
 EnableMenuItem(G.menubar, ID_QUICK_SAVE, MF_BYCOMMAND | (usable ? MF_ENABLED : MF_GRAYED));
 EnableMenuItem(G.menubar, ID_QUICK_LOAD, MF_BYCOMMAND | (usable && quick ? MF_ENABLED : MF_GRAYED));
}
static void build_menu(void) {
 G.menubar = CreateMenu(); HMENU game = CreatePopupMenu(); G.save_menu = CreatePopupMenu(); G.load_menu = CreatePopupMenu();
 AppendMenuW(game, MF_STRING, ID_QUICK_SAVE, L"Salvar rápido\tF5");
 AppendMenuW(game, MF_STRING, ID_QUICK_LOAD, L"Carregar rápido\tF9");
 AppendMenuW(game, MF_POPUP, (UINT_PTR)G.save_menu, L"Salvar partida…");
 AppendMenuW(game, MF_POPUP, (UINT_PTR)G.load_menu, L"Carregar partida");
 if (G.keys_path[0]) { AppendMenuW(game, MF_SEPARATOR, 0, NULL); AppendMenuW(game, MF_STRING, ID_KEYS, L"Configurar teclas…"); }
 AppendMenuW(G.menubar, MF_POPUP, (UINT_PTR)game, L"Partida");
 SetMenu(G.hwnd, G.menubar); refresh_save_menus();
}
static void command(unsigned id) {
 if (id == ID_KEYS) { show_keys(); return; }
 if (!G.driver || !G.saves[0]) return;
 char slot[32];
 if (id == ID_QUICK_SAVE) save_now("rapido", 0, NULL);
 else if (id == ID_QUICK_LOAD) { char path[4200]; if (!rlw_saves_path(G.saves, "rapido", path, sizeof(path)) && rlw_exists(path)) load_slot("rapido", 1); }
 else if (id == ID_STAGE_LOAD) load_slot("inicio-fase", 1);
 else if (id > ID_SAVE_POSITION && id <= ID_SAVE_POSITION + 5) {
  unsigned n = id - ID_SAVE_POSITION; char name[RLW_SAVE_NAME_CAPACITY];
  if (in_menu()) { message_box("Salve durante o jogo, entre quadros completos e antes de uma parada do motor.", "Não foi possível concluir a operação", MB_ICONINFORMATION | MB_OK); return; }
  if (ask_name(n, name, sizeof(name))) { snprintf(slot, sizeof(slot), "posicao-%u", n); save_now(slot, n, name); }
 } else if (id > ID_LOAD_POSITION && id <= ID_LOAD_POSITION + 5) { snprintf(slot, sizeof(slot), "posicao-%u", id - ID_LOAD_POSITION); load_slot(slot, 1); }
}

/* Requests the driver cannot complete by itself (sendShell's host cases). */
static void host_request(const RlShellEvent *e) {
 int code = 0;
 switch (e->kind) {
 case RL_SHELL_QUIT: G.quit = 1; PostQuitMessage(0); return;
 case RL_SHELL_MULTIPLAYER: reopen_menu("O jogo em rede não faz parte desta versão", "O menu principal será reaberto do início."); return;
 case RL_SHELL_DEFEAT_MOVIE: case RL_SHELL_CAMPAIGN_MOVIE:
  /* No movie player yet (later milestone): "unavailable" is the ABI's own
   * completion for a host without a decoder; both run the original MenuInit(2). */
  code = rlw_shell_movie_finish(G.driver, RL_SHELL_MOVIE_UNAVAILABLE); break;
 case RL_SHELL_INTRODUCTION:
  /* movieCompletion(): FINISHED whenever the file exists, played or not (the
   * original would have called PlayVideo); UNAVAILABLE only for an absent file. */
  code = rlw_shell_movie_finish(G.driver, movie_exists(e) ? RL_SHELL_MOVIE_FINISHED : RL_SHELL_MOVIE_UNAVAILABLE); break;
 case RL_SHELL_LOAD_SLOT: {
  /* beginSlotLoad(): the modern load replaces the world (and the session); a
   * failure reopens the original main menu. */
  char slot[32]; snprintf(slot, sizeof(slot), "posicao-%u", e->reserved + 1);
  if (load_slot(slot, 0)) { char why[512]; rlw_driver_save_error(G.driver, why, sizeof(why)); reopen_menu("Não foi possível carregar a partida", why[0] ? why : slot); }
  return;
 }
 case RL_SHELL_SAVE_SLOT: {
  /* The modern save of the paused game first (captured now, written off the
   * loop), then the original MenuInit(18) resume: play continues directly. */
  save_poll(1); SV.retry_slot[0] = 0;
  char slot[32], path[4200] = ""; snprintf(slot, sizeof(slot), "posicao-%u", e->reserved + 1);
  if (G.saves[0]) { rlw_mkdirs(G.saves); rlw_saves_path(G.saves, slot, path, sizeof(path)); }
  int32_t saved = 0; RlwSaveJob *job = NULL;
  code = rlw_shell_save_slot_begin(G.driver, path, &saved, &job);
  if (!code) {
   back_to_game();
   if (!saved && job) start_commit(job, 0, NULL); else save_failed("Não foi possível salvar a partida");
   G.deadline = now_seconds(); return;
  }
  break;
 }
 default: break;
 }
 if (code) { char why[512]; rlw_shell_error(G.driver, why, sizeof(why)); menu_failed(why[0] ? why : "Pedido da tela original"); return; }
 menu_result(RLW_MENU_REPAINT);
}

static void menu_result(int r) {
 switch (r) {
 case RLW_MENU_REPAINT: present_now(); update_title(); break;
 case RLW_MENU_GAME: back_to_game(); if (G.menu.game_kind == RL_SHELL_START_MISSION) stage_autosave(); break;
 case RLW_MENU_HOST: host_request(&G.menu.host_event); break;
 case RLW_MENU_FAILED: menu_failed(G.menu.error); present_now(); break;
 case RLW_MENU_OPTIONS: {
  /* A value changed on the "Opções" screen: apply it like the host's own controls. */
  const RlOpcoesEvento *e = &G.menu.options_event;
  if (e->kind == RL_OPCOES_MUDOU) {
   switch (e->item) {
   case RL_OPCOES_ATALHOS:
    /* Rules follow new worlds, and the menu world before its load tail; a
     * running stage or a loaded save keeps its own (rl_regras_definir). */
    G.atalhos = e->values.atalhos; rlw_router_reset(&G.router); G.router.modern_enabled = G.atalhos != 0; apply_rules(); break;
   case RL_OPCOES_CAMINHOS: G.caminhos = e->values.caminhos; apply_rules(); break;
   case RL_OPCOES_DIFICULDADE: G.opt.difficulty = e->values.dificuldade; (void)rlw_driver_set_difficulty(G.driver, G.opt.difficulty); break;
   case RL_OPCOES_VELOCIDADE: set_speed(e->values.velocidade); break;
   default: break;
   }
   options_store();
  } else if (e->kind == RL_OPCOES_CONFIGURAR_TECLAS) show_keys();
  present_now(); update_title(); break;
 }
 default: break;
 }
}

/* After each played frame: the original pause (16) or result (19) screen
 * opens the prepared session, exactly where the macOS host does. */
static int open_shell_if_needed(void) {
 RlwStatus s; rlw_driver_status(G.driver, &s);
 if (s.screen != RL_SHELL_PAUSE && s.screen != 19) return 0;
 if (s.screen == 19) {
  RlwCampaignEnd end; int c = rlw_campaign_end(G.driver, &end);
  if (c) { char why[512]; rlw_shell_error(G.driver, why, sizeof(why)); menu_failed(why); return 1; }
 }
 int c = rlw_shell_begin(G.driver);
 if (c) { char why[512]; rlw_shell_error(G.driver, why, sizeof(why)); menu_failed(why[0] ? why : "Tela original"); return 1; }
 release_all(); menu_attach(); present_now(); update_title();
 return 1;
}

static void menu_mouse(uint32_t msg, LPARAM lp) {
 int32_t x, y; original_pixel(GET_X_LPARAM(lp), GET_Y_LPARAM(lp), 1, &x, &y);
 if (msg == 0x201 || msg == 0x203) G.left_held = 1; else if (msg == 0x202) G.left_held = 0;
 G.last_x = x; G.last_y = y;
 menu_result(rlw_menu_mouse(&G.menu, msg, (uint32_t)x, (uint32_t)y, now_seconds() * 1000));
}

static LRESULT CALLBACK window_proc(HWND hwnd, UINT msg, WPARAM wp, LPARAM lp) {
 switch (msg) {
 case WM_SIZE: update_viewport(); InvalidateRect(hwnd, NULL, FALSE); return 0;
 case WM_PAINT: { PAINTSTRUCT ps; HDC dc = BeginPaint(hwnd, &ps); present(dc); EndPaint(hwnd, &ps); return 0; }
 case WM_ERASEBKGND: return 1;
 case WM_SETCURSOR:
  /* In play the game draws its own pointer into the surface; the menus keep
   * the system pointer, as the macOS host does (hidesSystemCursor only while running). */
  if (LOWORD(lp) == HTCLIENT && G.driver && !in_menu()) { SetCursor(NULL); return TRUE; }
  break;
 case WM_MOUSEMOVE: if (in_menu()) menu_mouse(0x200, lp); else mouse(0x200, lp, G.left_held, 1); return 0;
 case WM_LBUTTONDOWN: SetCapture(hwnd); if (in_menu()) menu_mouse(0x201, lp); else mouse(0x201, lp, 0, 0); return 0;
 case WM_LBUTTONDBLCLK: SetCapture(hwnd); if (in_menu()) menu_mouse(0x203, lp); else mouse(0x203, lp, 0, 0); return 0;
 case WM_LBUTTONUP: if (in_menu()) menu_mouse(0x202, lp); else mouse(0x202, lp, G.left_held, 0); if (GetCapture() == hwnd) ReleaseCapture(); return 0;
 case WM_RBUTTONDOWN: case WM_RBUTTONDBLCLK: if (in_menu()) menu_mouse(0x204, lp); else mouse(0x204, lp, 0, 0); return 0;
 case WM_RBUTTONUP: if (in_menu()) menu_mouse(0x205, lp); else mouse(0x205, lp, G.left_held, 0); return 0;
 case WM_MOUSEWHEEL:
  /* Objective/briefing pages: the wheel clicks the original arrow zones. */
  if (in_menu()) menu_result(rlw_menu_page(&G.menu, GET_WHEEL_DELTA_WPARAM(wp) < 0, now_seconds() * 1000));
  return 0;
 case WM_CAPTURECHANGED:
  if ((HWND)lp != hwnd && G.left_held) { G.left_held = 0; if (in_menu()) rlw_menu_cancel(&G.menu); else send_event(1, 0x202, G.last_x, G.last_y, 0, 0, 0); }
  return 0;
 case WM_KEYDOWN: case WM_SYSKEYDOWN:
  if (msg == WM_SYSKEYDOWN && wp == VK_F4) break; /* Alt+F4 closes */
  if (in_menu()) {
   if (wp == VK_F11) { if (!(lp & (1 << 30))) set_fullscreen(!G.fullscreen); return 0; }
   if (wp == VK_F5 || wp == VK_F9) { if (!(lp & (1 << 30))) command(wp == VK_F5 ? ID_QUICK_SAVE : ID_QUICK_LOAD); return 0; }
   if (!(lp & (1 << 30))) menu_result(rlw_menu_key(&G.menu, (uint32_t)wp, 1, now_seconds() * 1000));
   return 0;
  }
  if (key(wp, lp, 1)) return 0;
  break;
 case WM_KEYUP: case WM_SYSKEYUP:
  if (in_menu()) { menu_result(rlw_menu_key(&G.menu, (uint32_t)wp, 0, now_seconds() * 1000)); return 0; }
  if (key(wp, lp, 0)) return 0;
  break;
 case WM_SYSCOMMAND: if ((wp & 0xfff0) == SC_KEYMENU) return 0; break; /* Alt/F10 stay game keys */
 case WM_COMMAND: if (!lp && HIWORD(wp) == 0) { command(LOWORD(wp)); return 0; } break;
 case WM_INITMENUPOPUP: refresh_save_menus(); break;
 /* The menu bar takes the input like a focus change does. */
 case WM_ENTERMENULOOP: if (in_menu()) { rlw_menu_reset(&G.menu); G.left_held = 0; } else release_all(); return 0;
 case WM_EXITMENULOOP: G.deadline = now_seconds(); return 0;
 case WM_SETFOCUS: G.focused = 1; G.deadline = now_seconds(); return 0;
 case WM_KILLFOCUS: G.focused = 0; if (in_menu()) { rlw_menu_reset(&G.menu); G.left_held = 0; } else release_all(); return 0;
 case WM_DESTROY: PostQuitMessage(0); return 0;
 }
 return DefWindowProcW(hwnd, msg, wp, lp);
}

/* ---------- smoke / test helpers ---------- */

static void frame_digest(char hex[65]) {
 RlwSha256 c; uint8_t d[32]; rlw_sha256_init(&c);
 rlw_sha256_update(&c, rlw_driver_pixels(G.driver), 480000);
 rlw_sha256_update(&c, rlw_driver_palette(G.driver), 768);
 rlw_sha256_final(&c, d); rlw_sha256_hex(d, hex);
}

/* Window client area -> 24-bit BMP, to check the presented frame in wine/CI. */
static int save_screenshot(const char *path) {
 RECT c; GetClientRect(G.hwnd, &c); int w = c.right, h = c.bottom; if (w <= 0 || h <= 0) return -1;
 HDC wdc = GetDC(G.hwnd), mdc = CreateCompatibleDC(wdc);
 BITMAPINFO bi; memset(&bi, 0, sizeof(bi));
 bi.bmiHeader.biSize = sizeof(bi.bmiHeader); bi.bmiHeader.biWidth = w; bi.bmiHeader.biHeight = h;
 bi.bmiHeader.biPlanes = 1; bi.bmiHeader.biBitCount = 24; bi.bmiHeader.biCompression = BI_RGB;
 void *bits = NULL; HBITMAP bmp = CreateDIBSection(wdc, &bi, DIB_RGB_COLORS, &bits, NULL, 0);
 HGDIOBJ old = SelectObject(mdc, bmp);
 present(mdc);
 GdiFlush();
 uint32_t stride = ((uint32_t)w * 3 + 3) & ~3u, size = stride * (uint32_t)h;
 BITMAPFILEHEADER fh = {0x4d42, (DWORD)(sizeof(fh) + sizeof(bi.bmiHeader) + size), 0, 0, (DWORD)(sizeof(fh) + sizeof(bi.bmiHeader))};
 bi.bmiHeader.biSizeImage = size;
 FILE *f = rlw_fopen(path, "wb"); int rc = -1;
 if (f) { rc = fwrite(&fh, sizeof(fh), 1, f) == 1 && fwrite(&bi.bmiHeader, sizeof(bi.bmiHeader), 1, f) == 1 && fwrite(bits, 1, size, f) == size ? 0 : -1; if (fclose(f)) rc = -1; }
 SelectObject(mdc, old); DeleteObject(bmp); DeleteDC(mdc); ReleaseDC(G.hwnd, wdc);
 return rc;
}

/* Shared fixed synthetic input (rlw_demo.h), through the same driver path. */
static void demo_input(uint64_t frame) {
 for (unsigned i = 0; i < RLW_DEMO_COUNT; i++)
  if (RLW_DEMO[i].frame == frame)
   send_event(RLW_DEMO[i].kind, RLW_DEMO[i].message, RLW_DEMO[i].x, RLW_DEMO[i].y, RLW_DEMO[i].wparam, RLW_DEMO[i].vk, RLW_DEMO[i].down);
}

/* ---------- startup ---------- */

static void usage(void) {
 attach_console();
 printf("RisingLands.exe [--cd CAMINHO] [--data PASTA] [--mission MONDE01.ETA] [--difficulty 0|1|2]\n"
        "                [--speed 0|1|2|3] [--fullscreen]\n"
        "  Sem --mission o jogo abre no menu principal original.\n"
        "  testes: --selftest | --import-only | --frames N [--headless] [--hashes ARQ] [--screenshot ARQ.bmp] [--demo-input]\n"
        "          --menu-demo ARQ (percurso fixo pelos menus, sem janela; igual ao parity.sh)\n"
        "          --save-demo ARQ [--save-directory PASTA_VAZIA] (percurso fixo de salvar/carregar; igual ao parity.sh)\n"
        "  --save-directory PASTA: partidas salvas (padrão: Partidas na pasta de dados do usuário).\n"
        "  --cd: RISING_LANDS_BR_.BIN, .CUE, .ISO ou a pasta do CD (importa na primeira execução).\n"
        "  Teclas do host: P/Pause pausa, +/- velocidade, F5 salvar rápido, F9 carregar rápido, F11 tela cheia.\n"
        "  As demais 23 teclas vão ao jogo.\n"
        "  Atalhos modernos (Opções): 1-9 duas vezes centraliza o grupo, Home último alerta,\n"
        "  . trabalhador ocioso, , militar ocioso, H santuário.\n");
}

static int parse(int argc, char **argv, Options *o) {
 memset(o, 0, sizeof(*o)); o->mission = NULL; o->difficulty = 1; o->speed = 1;
 for (int i = 1; i < argc; i++) {
  const char *a = argv[i], *v = i + 1 < argc ? argv[i + 1] : NULL;
#define VALUE() do { if (!v) return -1; i++; } while (0)
  if (!strcmp(a, "--cd")) { VALUE(); o->cd = v; }
  else if (!strcmp(a, "--data")) { VALUE(); o->data = v; }
  else if (!strcmp(a, "--mission")) { VALUE(); o->mission = v; }
  else if (!strcmp(a, "--difficulty")) { VALUE(); o->difficulty = (unsigned)atoi(v); o->difficulty_given = 1; }
  else if (!strcmp(a, "--speed")) { VALUE(); o->speed = (unsigned)atoi(v); o->speed_given = 1; }
  else if (!strcmp(a, "--frames")) { VALUE(); o->frames = (unsigned)strtoul(v, NULL, 10); }
  else if (!strcmp(a, "--hashes")) { VALUE(); o->hashes = v; }
  else if (!strcmp(a, "--screenshot")) { VALUE(); o->screenshot = v; }
  else if (!strcmp(a, "--menu-demo")) { VALUE(); o->menu_demo = v; }
  else if (!strcmp(a, "--save-demo")) { VALUE(); o->save_demo = v; }
  else if (!strcmp(a, "--save-directory")) { VALUE(); o->save_directory = v; }
  else if (!strcmp(a, "--selftest")) o->selftest = 1;
  else if (!strcmp(a, "--import-only")) o->import_only = 1;
  else if (!strcmp(a, "--headless")) o->headless = 1;
  else if (!strcmp(a, "--fullscreen")) o->fullscreen = 1;
  else if (!strcmp(a, "--no-verify-image")) o->no_verify_image = 1;
  else if (!strcmp(a, "--demo-input")) o->demo_input = 1;
  else return -1;
#undef VALUE
 }
 /* Frame-count smoke runs keep milestone 1's stage start (MONDE01) unless told otherwise. */
 if (!o->mission && (o->frames || o->headless) && !o->menu_demo && !o->save_demo) o->mission = "MONDE01.ETA";
 return o->difficulty > 2 || o->speed > 3 ? -1 : 0;
}

static char *choose_cd(void) {
 wchar_t file[4096] = L"";
 OPENFILENAMEW ofn; memset(&ofn, 0, sizeof(ofn));
 ofn.lStructSize = sizeof(ofn); ofn.hwndOwner = G.hwnd;
 ofn.lpstrFilter = L"CD do Rising Lands (*.bin;*.cue;*.iso)\0*.bin;*.cue;*.iso\0Todos os arquivos\0*.*\0";
 ofn.lpstrFile = file; ofn.nMaxFile = 4096;
 ofn.lpstrTitle = L"Escolha o CD brasileiro do Rising Lands (RISING_LANDS_BR_.BIN, .CUE ou .ISO)";
 ofn.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
 return GetOpenFileNameW(&ofn) ? narrow(file) : NULL;
}

static void import_progress(void *ctx, double fraction, const char *message) {
 (void)ctx; if (G.opt.headless || G.opt.import_only) { attach_console(); printf("importando %3d%% %s\n", (int)(fraction * 100), message); fflush(stdout); }
}

static int ensure_import(const char *data_root) {
 if (!G.opt.cd && rlw_cd_installed(data_root)) return 0;
 char *chosen = NULL; const char *source = G.opt.cd;
 if (!source) {
  if (G.opt.headless || G.opt.frames) { attach_console(); fprintf(stderr, "CD não importado: use --cd CAMINHO\n"); return 1; }
  message_box("Na primeira execução, escolha o CD brasileiro original do Rising Lands: a imagem RISING_LANDS_BR_.BIN (ou o .CUE ou .ISO). Nenhum dado do jogo acompanha este aplicativo.",
              "Rising Lands — importar o CD", MB_ICONINFORMATION | MB_OK);
  source = chosen = choose_cd();
  if (!source) return 1;
 }
 HCURSOR old = SetCursor(LoadCursor(NULL, IDC_WAIT));
 char error[1024] = {0};
 int rc = rlw_cd_import(source, data_root, !G.opt.no_verify_image, import_progress, NULL, error, sizeof(error));
 SetCursor(old); free(chosen);
 if (rc) {
  attach_console(); fprintf(stderr, "importação: %s\n", error);
  if (!G.opt.headless && !G.opt.import_only) message_box(error, "Rising Lands — importação", MB_ICONERROR | MB_OK);
 }
 return rc;
}

static int any_entry(const char *name, int is_dir, void *ctx) { (void)name; (void)is_dir; (void)ctx; return 1; }

static int run_headless(FILE *hashes) {
 for (unsigned i = 0; i < G.opt.frames + 1; i++) {
  RlwStatus s; rlw_driver_status(G.driver, &s);
  if (G.opt.demo_input && i) demo_input(s.frames + 1);
  if (rlw_driver_advance(G.driver)) { report_stop(); return 2; }
  rlw_driver_status(G.driver, &s);
  if (hashes) { char hex[65]; frame_digest(hex); fprintf(hashes, "%llu %u %s\n", (unsigned long long)s.frames, s.screen, hex); }
 }
 return 0;
}

int main_utf8(int argc, char **argv) {
 QueryPerformanceFrequency(&G.frequency);
 if (parse(argc, argv, &G.opt)) { usage(); return 64; }
 G.speed = G.opt.speed;
 rlw_router_init(&G.router, NULL, (RlwRouterSink){NULL, router_emit, router_modern}); rlw_planner_init(&G.planner);
 G.pointer_x = 400; G.pointer_y = 300;
 char error[1024] = {0};
 if (G.opt.selftest) {
  attach_console();
  if (rlw_engine_check(error, sizeof(error))) { fprintf(stderr, "selftest: %s\n", error); return 1; }
  uint8_t d[32]; char hex[65]; rlw_sha256("abc", 3, d); rlw_sha256_hex(d, hex);
  if (strcmp(hex, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")) { fprintf(stderr, "selftest: sha256\n"); return 1; }
  printf("selftest ok: engine ABI %08x, render contract %08x, mission v%u, save v%u\n",
         rl_versao(), rl_render_contrato(), rl_missao_versao(), rl_salvar_versao());
  return 0;
 }
 char data_root[4096];
 if (G.opt.data) snprintf(data_root, sizeof(data_root), "%s", G.opt.data);
 else if (rlw_user_data_dir(data_root, sizeof(data_root))) { message_box("Pasta de dados do usuário indisponível.", "Rising Lands", MB_ICONERROR); return 1; }
 if (ensure_import(data_root)) return 2;
 if (G.opt.import_only) { attach_console(); printf("importado em %s" RLW_SEP "Dados\n", data_root); return 0; }
 char *dados = G.dados; snprintf(G.dados, sizeof(G.dados), "%s" RLW_SEP "Dados", data_root);
 if (G.opt.menu_demo) {
  RlwDriver *d = rlw_driver_create_menu(dados, G.opt.difficulty, error, sizeof(error));
  attach_console();
  if (!d) { fprintf(stderr, "início: %s\n", error); return 3; }
  FILE *out = rlw_fopen(G.opt.menu_demo, "wb"); if (!out) { fprintf(stderr, "não foi possível criar %s\n", G.opt.menu_demo); rlw_driver_free(d); return 4; }
  int walk = rlw_menu_demo_run(&RLW_WINDOWS_OPS, d, out); fclose(out); rlw_driver_free(d);
  printf("menu demo %s: %s\n", walk ? "terminou antes do fim" : "completo", G.opt.menu_demo);
  return 0;
 }
 if (G.opt.save_demo) {
  /* Never the player's saves: an empty folder of its own (recreated) or the
   * given one, which must be empty. */
  char folder[4200]; attach_console();
  if (G.opt.save_directory) snprintf(folder, sizeof(folder), "%s", G.opt.save_directory);
  else { snprintf(folder, sizeof(folder), "%s" RLW_SEP "teste-saves", data_root); rlw_remove_tree(folder); }
  int empty = !rlw_mkdirs(folder) && rlw_list_dir(folder, any_entry, NULL) == 0;
  if (!empty) { fprintf(stderr, "a pasta do teste precisa estar vazia: %s\n", folder); return 4; }
  RlwDriver *d = rlw_driver_create(dados, "MONDE01.ETA", 1, error, sizeof(error));
  if (!d) { fprintf(stderr, "início: %s\n", error); return 3; }
  FILE *out = rlw_fopen(G.opt.save_demo, "wb"); if (!out) { fprintf(stderr, "não foi possível criar %s\n", G.opt.save_demo); rlw_driver_free(d); return 4; }
  int walk = rlw_save_demo_run(&RLW_WINDOWS_OPS, d, folder, out); fclose(out); rlw_driver_free(d);
  printf("save demo %s: %s\n", walk ? "terminou antes do fim" : "completo", G.opt.save_demo);
  return 0;
 }
 /* The player's saves (none in frame-count smoke runs unless asked). */
 if (G.opt.save_directory) snprintf(G.saves, sizeof(G.saves), "%s", G.opt.save_directory);
 else if (!G.opt.frames && !G.opt.headless) snprintf(G.saves, sizeof(G.saves), "%s" RLW_SEP "Partidas", data_root);
 if (!G.opt.frames && !G.opt.headless) {
  snprintf(G.options_path, sizeof(G.options_path), "%s" RLW_SEP "opcoes.ini", data_root);
  /* The macOS app's defaults (NativeModernHotkeys/NativeModernPaths,
   * NativeDifficulty, NativeGameSpeed): Atalhos modernos on, Caminhos melhores
   * off, Normal, 1x. A key in opcoes.ini wins. */
  G.atalhos = 1; G.caminhos = 0;
  options_load(); G.speed = G.opt.speed; rlw_driver_default_rules(options_rules());
  G.router.modern_enabled = G.atalhos != 0;
  snprintf(G.keys_path, sizeof(G.keys_path), "%s" RLW_SEP "teclas.ini", data_root); keys_load();
 }
 G.driver = G.opt.mission ? rlw_driver_create(dados, G.opt.mission, G.opt.difficulty, error, sizeof(error))
                          : rlw_driver_create_menu(dados, G.opt.difficulty, error, sizeof(error));
 if (!G.driver) {
  attach_console(); fprintf(stderr, "início: %s\n", error);
  if (!G.opt.headless && !G.opt.frames) message_box(error, "Rising Lands", MB_ICONERROR | MB_OK);
  return 3;
 }
 FILE *hashes = NULL;
 if (G.opt.hashes && !(hashes = rlw_fopen(G.opt.hashes, "wb"))) { attach_console(); fprintf(stderr, "não foi possível criar %s\n", G.opt.hashes); return 4; }
 if (G.opt.headless) {
  int rc = run_headless(hashes); if (hashes) fclose(hashes);
  RlwStatus s; rlw_driver_status(G.driver, &s); attach_console();
  char hex[65]; frame_digest(hex);
  printf("headless %s: %llu quadros, tela %u, FLC sem imagem %u, último quadro %s\n", rc ? "parado" : "ok",
         (unsigned long long)s.frames, s.screen, s.flc_skipped, hex);
  rlw_driver_free(G.driver); return rc;
 }

 G.bmi = calloc(1, sizeof(BITMAPINFOHEADER) + 256 * sizeof(RGBQUAD));
 G.bmi->bmiHeader.biSize = sizeof(BITMAPINFOHEADER); G.bmi->bmiHeader.biWidth = 800; G.bmi->bmiHeader.biHeight = -600;
 G.bmi->bmiHeader.biPlanes = 1; G.bmi->bmiHeader.biBitCount = 8; G.bmi->bmiHeader.biCompression = BI_RGB;
 G.bmi->bmiHeader.biClrUsed = 256;
 HINSTANCE instance = GetModuleHandleW(NULL);
 WNDCLASSEXW wc; memset(&wc, 0, sizeof(wc)); wc.cbSize = sizeof(wc);
 wc.style = CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS; wc.lpfnWndProc = window_proc; wc.hInstance = instance;
 wc.hCursor = LoadCursor(NULL, IDC_ARROW); wc.hIcon = LoadIcon(NULL, IDI_APPLICATION);
 wc.lpszClassName = L"RisingLandsWindow";
 RegisterClassExW(&wc);
 RECT r = {0, 0, 800, 600}; AdjustWindowRect(&r, WS_OVERLAPPEDWINDOW, TRUE);
 G.hwnd = CreateWindowExW(0, wc.lpszClassName, L"Rising Lands", WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT,
                          r.right - r.left, r.bottom - r.top, NULL, NULL, instance, NULL);
 if (!G.hwnd) { message_box("Não foi possível criar a janela.", "Rising Lands", MB_ICONERROR); return 5; }
 G.placement.length = sizeof(G.placement);
 build_menu();
 ShowWindow(G.hwnd, SW_SHOW); update_viewport();
 if (G.opt.fullscreen) set_fullscreen(1);
 SetForegroundWindow(G.hwnd); SetFocus(G.hwnd);
 if (rlw_shell_mode(G.driver)) menu_attach();
 update_title();
 timeBeginPeriod(1);
 G.deadline = now_seconds();
 int running = 1, rc = 0; double last_title = 0;
 while (running) {
  MSG m;
  while (PeekMessageW(&m, NULL, 0, 0, PM_REMOVE)) {
   if (m.message == WM_QUIT) { running = 0; break; }
   if (rlw_keys_dialog_message(&m)) continue;
   TranslateMessage(&m); DispatchMessageW(&m);
  }
  if (!running) break;
  if (G.quit) break;
  save_poll(0);
  if (SV.retry_slot[0] && now_seconds() >= SV.retry_at) { char slot[32], name[RLW_SAVE_NAME_CAPACITY]; snprintf(slot, sizeof(slot), "%s", SV.retry_slot); snprintf(name, sizeof(name), "%s", SV.retry_name); save_attempt(slot, SV.retry_named, name, SV.retries); }
  if (G.menu_failed) { MsgWaitForMultipleObjects(0, NULL, FALSE, 50, QS_ALLINPUT); if (G.opt.frames) { rc = 2; break; } continue; }
  if (in_menu()) {
   /* shellTick at 20 Hz (100 Hz in the credits); moves coalesced to 60 Hz. */
   double t = now_seconds(), period = rlw_menu_tick_ms(&G.menu) / 1000.0;
   if (G.menu.move_pending && t - G.menu_flush >= 1.0 / 60) { G.menu_flush = t; menu_result(rlw_menu_flush_move(&G.menu, t * 1000)); continue; }
   if (t >= G.menu_tick) {
    if (t - G.menu_tick > MAXIMUM_BACKLOG) G.menu_tick = t;
    G.menu_tick += period;
    menu_result(rlw_menu_tick(&G.menu, t * 1000));
    continue;
   }
   double until = G.menu_tick - t; if (G.menu.move_pending && G.menu_flush + 1.0 / 60 - t < until) until = G.menu_flush + 1.0 / 60 - t;
   MsgWaitForMultipleObjects(0, NULL, FALSE, (DWORD)(until > 0 ? until * 1000 : 0), QS_ALLINPUT);
   continue;
  }
  RlwStatus s; rlw_driver_status(G.driver, &s);
  int smoke = G.opt.frames != 0;
  if (s.stopped || G.paused) { report_stop(); MsgWaitForMultipleObjects(0, NULL, FALSE, 50, QS_ALLINPUT); if (smoke && s.stopped) { rc = 2; break; } continue; }
  /* NativeFramePacer: one frame when due; debt beyond 0.25 s is forgiven. */
  double t = now_seconds(), rate = smoke ? 0 : NORMAL_FRAME_RATE * SPEEDS[G.speed];
  if (rate > 0 && t < G.deadline) {
   DWORD wait = (DWORD)((G.deadline - t) * 1000); MsgWaitForMultipleObjects(0, NULL, FALSE, wait, QS_ALLINPUT); continue;
  }
  if (rate > 0) { if (t - G.deadline > MAXIMUM_BACKLOG) G.deadline = t; G.deadline += 1 / rate; }
  if (smoke && G.opt.demo_input) demo_input(s.frames + 1);
  int stage_start = rlw_driver_stage_start(G.driver);
  if (rlw_driver_advance(G.driver)) { report_stop(); present_now(); continue; }
  if (stage_start) stage_autosave();
  step_modern();
  present_now();
  rlw_driver_status(G.driver, &s);
  if (!smoke && open_shell_if_needed()) continue;
  if (hashes) { char hex[65]; frame_digest(hex); fprintf(hashes, "%llu %u %s\n", (unsigned long long)s.frames, s.screen, hex); }
  if (t - last_title > 0.5) { update_title(); last_title = t; }
  if (smoke && s.frames >= G.opt.frames) {
   if (G.opt.screenshot && save_screenshot(G.opt.screenshot)) rc = 6;
   char hex[65]; frame_digest(hex); attach_console();
   printf("janela ok: %llu quadros, tela %u, FLC sem imagem %u, último quadro %s\n", (unsigned long long)s.frames, s.screen, s.flc_skipped, hex);
   break;
  }
 }
 timeEndPeriod(1);
 save_poll(1); /* flushSaves(): never leave a captured save unwritten */
 if (hashes) fclose(hashes);
 rlw_menu_close(&G.menu);
 rlw_driver_free(G.driver); G.driver = NULL;
 return rc;
}

int WINAPI wWinMain(HINSTANCE instance, HINSTANCE previous, PWSTR command, int show) {
 (void)instance; (void)previous; (void)command; (void)show;
 int argc = 0; wchar_t **wargv = CommandLineToArgvW(GetCommandLineW(), &argc);
 char **argv = calloc((size_t)argc + 1, sizeof(char *));
 for (int i = 0; i < argc; i++) argv[i] = narrow(wargv[i]);
 LocalFree(wargv);
 int rc = main_utf8(argc, argv);
 for (int i = 0; i < argc; i++) free(argv[i]);
 free(argv);
 if (console_attached) { fflush(stdout); fflush(stderr); }
 return rc;
}
