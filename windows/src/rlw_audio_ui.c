/* Sound in the Windows host's window (see rlw_audio_ui.h). */
#include "rlw_audio_ui.h"
#include "rlw_audio_win32.h"
#include "rlw_sound_import.h"
#include "rlw_cdmusic.h"
#include "rlw_eventflc.h"
#include "rlw_movie.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Menu ids: a range of their own, far from rlw_main.c's (100..135). */
enum { ID_EFFECTS_MUTE = 0x5a00, ID_EFFECTS_LEVEL = 0x5a10, ID_MUSIC_MUTE = 0x5a20, ID_MUSIC_LEVEL = 0x5a30, ID_STATUS = 0x5a40 };

static struct {
 RlwAudioHost *host; RlwAudioSettings settings; void (*store)(void); int persist;
 HMENU menu; char import_error[256];
 /* In-game FLC events (NativeEventFLCPlayback) and whether its picture replaces the engine's. */
 RlwEventFlc event; int event_ready, overlay, event_forced; RlwDriver *event_driver; uint64_t event_frame;
 /* Original movie (NativeDefeatMovie): modal over the menus until it ends or Escape. */
 RlwMovie *movie; double movie_started; uint32_t movie_kind; int movie_skip, movie_new; uint8_t *movie_bgrx; BITMAPINFO movie_bmi;
} U;

static void apply(void) {
 RlwAudio *a = rlw_audio_host_audio(U.host);
 rlw_audio_set_effects_gain(a, rlw_audio_settings_effects_gain(&U.settings));
 rlw_audio_set_music_gain(a, rlw_audio_settings_music_gain(&U.settings));
}

void rlw_audio_ui_import(const char *source, const char *data_root) {
 char error[512] = {0};
 if (rlw_sound_import(source, data_root, error, sizeof(error))) {
  snprintf(U.import_error, sizeof(U.import_error), "%s", error);
  fprintf(stderr, "importação do som: %s\n", error);
 }
 /* CD music: only a raw .BIN/.CUE image has the audio tracks (2 = skipped). */
 if (rlw_cdmusic_import(source, data_root, error, sizeof(error)) == 1) fprintf(stderr, "importação da música: %s\n", error);
}
void rlw_audio_ui_leave_stage(void) { rlw_audio_host_leave_stage(U.host); }
void rlw_audio_ui_loaded(RlwDriver *d) { rlw_audio_host_loaded(U.host, d); }
/* Menu ticks: the credits screen stops the CD music, as the macOS host's
 * leaveStage when it shows the credits (once per visit). */
static int in_credits;
void rlw_audio_ui_menu_tick(RlwDriver *d) {
 int credits = d && rlw_shell_mode(d) == RL_SHELL_CREDITS_SCREEN;
 if (credits && !in_credits) rlw_audio_host_leave_stage(U.host);
 in_credits = credits;
 rlw_audio_host_maintain(U.host);   /* device lost/back while in the menus too */
}
int rlw_audio_ui_smoke(void) {
 char report[256] = {0}; int rc = rlw_audio_host_smoke(report, sizeof(report));
 printf("%s\n", report); fflush(stdout);
 return rc;
}

void rlw_audio_ui_open(const char *dados, const char *options_path, void (*store)(void)) {
 rlw_audio_settings_default(&U.settings);
 U.persist = options_path && options_path[0]; U.store = store;
 if (U.persist) {
  uint32_t n = 0; uint8_t *text = rlw_read_file(options_path, &n, 1u << 16);
  if (text) { rlw_audio_settings_parse(&U.settings, (const char *)text, n); free(text); }
 }
 if (!U.host) U.host = rlw_audio_host_open(dados);
 char events[4200]; snprintf(events, sizeof(events), "%s" RLW_SEP "events", dados ? dados : ".");
 rlw_eventflc_init(&U.event, events); U.event_ready = 1;
 apply();
}

void rlw_audio_ui_close(void) {
 rlw_audio_host_close(U.host); U.host = NULL;
 if (U.event_ready) rlw_eventflc_free(&U.event);
 U.event_ready = 0; U.overlay = 0;
}
void rlw_audio_ui_attach(RlwDriver *d) {
 if (U.event_ready) rlw_driver_event_flc_host(d, 1);
 rlw_audio_host_attach(U.host, d);
}

/* ---- In-game FLC events ---- */
static int event_name(void *ctx, uint32_t index, char out[16]) { return rlw_driver_event_flc_name(ctx, index, out); }
static int event_finish(void *ctx) { return rlw_driver_event_flc_finish(ctx); }
static void event_tick(void *ctx) { (void)ctx; rlw_audio_event_tick(rlw_audio_host_audio(U.host), (uint32_t)U.event_frame); }
static double now_ms(void) {
 static LARGE_INTEGER frequency; LARGE_INTEGER t;
 if (!frequency.QuadPart) QueryPerformanceFrequency(&frequency);
 QueryPerformanceCounter(&t); return (double)t.QuadPart * 1000.0 / (double)frequency.QuadPart;
}
static void event_update(RlwDriver *d) {
 U.overlay = 0;
 if (!U.event_ready) return;
 RlEventFlcStateV1 st; if (rlw_driver_event_flc_state(d, &st)) return;
 RlwStatus s; rlw_driver_status(d, &s); U.event_frame = s.frames; U.event_driver = d;
 RlwEventFlcState state = {st.running, st.modal, st.index, st.frames, st.width, st.height, st.speed};
 RlwEventFlcHost host = {d, event_name, event_finish, event_tick};
 U.overlay = rlw_eventflc_update(&U.event, &state, now_ms(), rlw_driver_pixels(d), &host);
 /* An event whose picture cannot be shown still ends at once, as the host did
  * before it had an FLC player (and never waits for a click on a grey view). */
 if (!st.running) U.event_forced = 0;
 else if (U.event.finished && U.event.failure[0] && st.modal && !U.event_forced) { U.event_forced = 1; (void)rlw_driver_event_flc_finish(d); }
}
void rlw_audio_ui_pump(RlwDriver *d) { rlw_audio_host_pump(U.host, d); event_update(d); }
int rlw_audio_ui_picture(RlwDriver *d, const uint8_t **pixels, const uint8_t **palette) {
 if (!U.overlay || d != U.event_driver || rlw_shell_mode(d)) return 0;
 *pixels = U.event.surface; *palette = U.event.flc.palette; return 1;
}
RlwAudio *rlw_audio_ui_audio(void) { return rlw_audio_host_audio(U.host); }
size_t rlw_audio_ui_ini(char *out, size_t capacity) { return rlw_audio_settings_format(&U.settings, out, capacity); }

static void refresh(void) {
 if (!U.menu) return;
 CheckMenuItem(U.menu, ID_EFFECTS_MUTE, MF_BYCOMMAND | (U.settings.effects_muted ? MF_CHECKED : MF_UNCHECKED));
 CheckMenuItem(U.menu, ID_MUSIC_MUTE, MF_BYCOMMAND | (U.settings.music_muted ? MF_CHECKED : MF_UNCHECKED));
 for (unsigned i = 0; i < 4; i++) {
  CheckMenuItem(U.menu, ID_EFFECTS_LEVEL + i, MF_BYCOMMAND | (RLW_AUDIO_LEVELS[i] == U.settings.effects_volume ? MF_CHECKED : MF_UNCHECKED));
  CheckMenuItem(U.menu, ID_MUSIC_LEVEL + i, MF_BYCOMMAND | (RLW_AUDIO_LEVELS[i] == U.settings.music_volume ? MF_CHECKED : MF_UNCHECKED));
 }
}

void rlw_audio_ui_menu(HMENU menubar) {
 U.menu = CreatePopupMenu(); HMENU effects = CreatePopupMenu(), music = CreatePopupMenu();
 for (unsigned i = 0; i < 4; i++) {
  static const wchar_t *const labels[4] = {L"100%", L"75%", L"50%", L"25%"}; const wchar_t *label = labels[i];
  AppendMenuW(effects, MF_STRING, ID_EFFECTS_LEVEL + i, label);
  AppendMenuW(music, MF_STRING, ID_MUSIC_LEVEL + i, label);
 }
 AppendMenuW(U.menu, MF_STRING, ID_EFFECTS_MUTE, L"Silenciar efeitos");
 AppendMenuW(U.menu, MF_POPUP, (UINT_PTR)effects, L"Volume dos efeitos");
 AppendMenuW(U.menu, MF_SEPARATOR, 0, NULL);
 AppendMenuW(U.menu, MF_STRING, ID_MUSIC_MUTE, L"Silenciar música");
 AppendMenuW(U.menu, MF_POPUP, (UINT_PTR)music, L"Volume da música");
 /* Why there is no sound, when there is none (grayed, informative). */
 const char *availability = rlw_audio_availability(rlw_audio_host_audio(U.host));
 const wchar_t *why = !strcmp(availability, "ausente") ? (U.import_error[0] ? L"Som indisponível: a importação dos sons falhou"
                                                                           : L"Som indisponível: importe o CD de novo para incluir os sons")
                    : !strcmp(availability, "sem dispositivo") ? L"Som indisponível: nenhum dispositivo de áudio" : NULL;
 if (why) { AppendMenuW(U.menu, MF_SEPARATOR, 0, NULL); AppendMenuW(U.menu, MF_STRING | MF_GRAYED, ID_STATUS, why); }
 const RlwCdMusic *cd = rlw_audio_host_music(U.host);
 if (cd && !strncmp(cd->notice, "Música do CD indisponível", 26)) {
  if (!why) AppendMenuW(U.menu, MF_SEPARATOR, 0, NULL);
  AppendMenuW(U.menu, MF_STRING | MF_GRAYED, ID_STATUS + 1, L"Música do CD indisponível: importe a imagem .BIN/.CUE do CD");
 }
 AppendMenuW(menubar, MF_POPUP, (UINT_PTR)U.menu, L"Áudio");
 refresh();
}

int rlw_audio_ui_command(unsigned id) {
 if (id == ID_EFFECTS_MUTE) U.settings.effects_muted = !U.settings.effects_muted;
 else if (id == ID_MUSIC_MUTE) U.settings.music_muted = !U.settings.music_muted;
 else if (id >= ID_EFFECTS_LEVEL && id < ID_EFFECTS_LEVEL + 4) U.settings.effects_volume = RLW_AUDIO_LEVELS[id - ID_EFFECTS_LEVEL];
 else if (id >= ID_MUSIC_LEVEL && id < ID_MUSIC_LEVEL + 4) U.settings.music_volume = RLW_AUDIO_LEVELS[id - ID_MUSIC_LEVEL];
 else return 0;
 apply(); refresh();
 if (U.persist && U.store) U.store();
 return 1;
}

/* ---------------- Original movies ---------------- */
/* \events\introfr.avi -> <dados>/movies/INTROFR.AVI (the import keeps the CD's names). */
static void movie_path(const char *dados, const RlShellEvent *e, char *out, size_t capacity) {
 char name[64] = {0}; uint32_t start = 0, n = e->path_length < sizeof(e->path) ? e->path_length : (uint32_t)sizeof(e->path);
 for (uint32_t i = 0; i < n; i++) if (e->path[i] == '\\' || e->path[i] == '/') start = i + 1;
 uint32_t k = 0; for (uint32_t i = start; i < n && k + 1 < sizeof(name); i++) { char c = (char)e->path[i]; name[k++] = c >= 'a' && c <= 'z' ? (char)(c - 32) : c; }
 snprintf(out, capacity, "%s" RLW_SEP "movies" RLW_SEP "%s", dados, k ? name : "-");
}
int rlw_audio_ui_movie_begin(const char *dados, const RlShellEvent *e) {
 if (U.movie || !e || (e->kind != RL_SHELL_DEFEAT_MOVIE && e->kind != RL_SHELL_CAMPAIGN_MOVIE && e->kind != RL_SHELL_INTRODUCTION)) return 0;
 char path[4400], error[256] = {0}; movie_path(dados, e, path, sizeof(path));
 RlwMovie *m = rlw_movie_open(path, error, sizeof(error));
 if (!m) { fprintf(stderr, "filme %s: %s\n", path, error); return 0; }
 uint8_t *bgrx = malloc((size_t)rlw_movie_width(m) * rlw_movie_height(m) * 4);
 if (!bgrx) { rlw_movie_free(m); return 0; }
 rlw_audio_host_leave_stage(U.host);                   /* the CD music stops (leaveStage) */
 RlwAudio *a = rlw_audio_host_audio(U.host);
 if (rlw_movie_audio_rate(m)) { rlw_audio_set_music_gain(a, 1.0f); rlw_audio_set_music(a, rlw_movie_pull, m, rlw_movie_audio_rate(m)); }
 memset(&U.movie_bmi, 0, sizeof(U.movie_bmi));
 U.movie_bmi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER); U.movie_bmi.bmiHeader.biWidth = (LONG)rlw_movie_width(m);
 U.movie_bmi.bmiHeader.biHeight = -(LONG)rlw_movie_height(m); U.movie_bmi.bmiHeader.biPlanes = 1; U.movie_bmi.bmiHeader.biBitCount = 32;
 U.movie_bmi.bmiHeader.biCompression = BI_RGB;
 memset(bgrx, 0, (size_t)rlw_movie_width(m) * rlw_movie_height(m) * 4);
 U.movie = m; U.movie_bgrx = bgrx; U.movie_kind = e->kind; U.movie_skip = 0; U.movie_new = 1; U.movie_started = now_ms();
 return 1;
}
int rlw_audio_ui_movie_active(void) { return U.movie != NULL; }
static void movie_end(void) {
 RlwAudio *a = rlw_audio_host_audio(U.host);
 rlw_audio_set_music(a, NULL, NULL, 0); apply();
 rlw_movie_free(U.movie); U.movie = NULL; free(U.movie_bgrx); U.movie_bgrx = NULL;
}
int rlw_audio_ui_movie_step(uint32_t *completion) {
 if (!U.movie) return 0;
 double elapsed = now_ms() - U.movie_started;
 int r = rlw_movie_advance(U.movie, elapsed);
 if (r > 0) {
  const uint8_t *rgbx = rlw_movie_pixels(U.movie); size_t n = (size_t)rlw_movie_width(U.movie) * rlw_movie_height(U.movie);
  for (size_t i = 0; i < n; i++) { U.movie_bgrx[4 * i] = rgbx[4 * i + 2]; U.movie_bgrx[4 * i + 1] = rgbx[4 * i + 1]; U.movie_bgrx[4 * i + 2] = rgbx[4 * i]; U.movie_bgrx[4 * i + 3] = 0; }
  U.movie_new = 1;
 }
 int failed = r < 0, done = failed || U.movie_skip || elapsed >= rlw_movie_duration_ms(U.movie);
 if (!done) return r > 0 ? 2 : 0;
 if (failed) fprintf(stderr, "filme: falha ao decodificar o AVI original: %s\n", rlw_movie_error(U.movie));
 /* PlayVideo's return: 1 (finished) after the end or Escape; a decoding failure
  * is "unavailable", except the introduction, which counts as shown when its
  * file exists (movieCompletion). */
 *completion = failed && U.movie_kind != RL_SHELL_INTRODUCTION ? RL_SHELL_MOVIE_UNAVAILABLE : RL_SHELL_MOVIE_FINISHED;
 movie_end();
 return 1;
}
int rlw_audio_ui_movie_message(UINT msg, WPARAM wp) {
 if (!U.movie) return 0;
 switch (msg) {
 case WM_KEYUP: case WM_SYSKEYUP: if (wp == VK_ESCAPE) U.movie_skip = 1; return wp != VK_F11;   /* 0x47c96e: Escape key-up */
 case WM_KEYDOWN: case WM_SYSKEYDOWN: return wp != VK_F11 && wp != VK_F4;
 case WM_MOUSEMOVE: case WM_LBUTTONDOWN: case WM_LBUTTONUP: case WM_LBUTTONDBLCLK: case WM_RBUTTONDOWN: case WM_RBUTTONUP:
 case WM_RBUTTONDBLCLK: case WM_MOUSEWHEEL: return 1;                                         /* the mouse does not end it */
 default: return 0;
 }
}
int rlw_audio_ui_movie_present(HDC dc, const RECT *client, const RECT *view) {
 if (!U.movie) return 0;
 FillRect(dc, client, (HBRUSH)GetStockObject(BLACK_BRUSH));
 SetStretchBltMode(dc, HALFTONE); SetBrushOrgEx(dc, 0, 0, NULL);
 StretchDIBits(dc, view->left, view->top, view->right - view->left, view->bottom - view->top, 0, 0,
               (int)rlw_movie_width(U.movie), (int)rlw_movie_height(U.movie), U.movie_bgrx, &U.movie_bmi, DIB_RGB_COLORS, SRCCOPY);
 U.movie_new = 0;
 return 1;
}
