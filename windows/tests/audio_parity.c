/* Sound-event parity runner (front 2, audio): the engine's ordered sound
 * requests as each host driver publishes them, for
 *  - the Windows host driver (rlw_driver.c: rlw_driver_sound_*), or
 *  - with -DPARITY_MACOS, the macOS host driver (macos/src, read-only:
 *    macos_live_sound_*).
 * Capture is enabled right after the driver is created (as the apps do) and the
 * queue is drained after every host tick. One line per event:
 *   "<tick> <kind> <frame> <phase> <site> <eax> <edx> <ebx> <ecx> <volume> <has_volume>"
 * (kind 0 Manage, 1 PostSound, 2 StopSound, 3 StopAllSound), the CD music requests of the
 * world's last step (frame boundary 102, MCICD_Play) as "<tick> cd <site> <track> <loop>", then a summary line
 * with the event counts per kind and the dropped counter. Identical files prove
 * the Windows driver hands its audio exactly the requests the macOS app plays.
 * Needs an imported CD (Dados folder); no game data is part of this test.
 *   audio_parity <Dados> <ticks> <events.txt>      MONDE01 with the fixed input (rlw_demo.h), then
 *                                                  a deterministic click soak (selections and orders)
 *   audio_parity <Dados> menu <events.txt>         the scripted menu walk (rlw_menu_demo.c)
 * Windows build only, with the original sound banks (<folder> holds {A,B,P}.{HED,SND}):
 *   audio_parity samples <folder> <samples.txt>    every data header decoded: "<bank> <entry> <variant> <frames> <sha256>"
 *   audio_parity <Dados> <ticks> <events.txt> --wav <folder> <out.wav>
 *                                                  the play run also through RlwAudio (mapper + mixer), rendered
 *                                                  at 44,100 Hz, 1/30 s per tick; prints the WAV's statistics
 *   audio_parity music <source .cue/.bin> <data root>   rlw_cdmusic_import (pinned track WAVs)
 *   audio_parity movies <file.AVI>...              rlw_movie.c decodes every frame and the audio, one line per
 *                                                  movie as tests/movie_parity.swift prints it */
#include "rlw_demo.h"
#include "rlw_menu_demo.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef PARITY_MACOS
#include "MacOSLiveDriver.h"
typedef MacOSSoundEvent SoundEvent;
#else
#include "rlw_driver.h"
#include "rlw_menu_ops.h"
#include "rlw_audio.h"
#include "rlw_cdmusic.h"
#include "rlw_movie.h"
#include "rlw_sha256.h"
#include "rlw_fs.h"
#include <math.h>
typedef RlwSoundEvent SoundEvent;
static RlwAudio *g_audio; static int16_t *g_wav; static uint64_t g_wav_frames, g_wav_capacity;
#endif

/* After the fixed demo input: every 6 frames from frame 80 a click, left and
 * right in turn, at positions from a fixed LCG over the play area
 * (selections and orders make the units answer). Same input for both drivers. */
typedef struct { uint32_t kind, message; int32_t x, y; } SoakEvent;
static int soak(uint64_t frame, SoakEvent out[2]) {
 if (frame < 80 || (frame - 80) % 6) return 0;
 uint32_t k = (uint32_t)((frame - 80) / 6), seed = 0x2545f491u;
 for (uint32_t i = 0; i <= k; i++) seed = seed * 1664525u + 1013904223u;
 int32_t x = (int32_t)(40 + (seed >> 8) % 560), y = (int32_t)(60 + (seed >> 20) % 440);
 uint32_t down = k % 2 ? 0x204u : 0x201u, up = k % 2 ? 0x205u : 0x202u;
 out[0] = (SoakEvent){1, down, x, y}; out[1] = (SoakEvent){1, up, x, y};
 return 2;
}

static FILE *g_out; static unsigned long long g_tick, g_kinds[4], g_other;

static uint32_t drain(void *driver, SoundEvent *buffer, uint32_t capacity) {
#ifdef PARITY_MACOS
 return macos_live_sound_events((MacOSLiveDriver *)driver, buffer, capacity);
#else
 return rlw_driver_sound_events((RlwDriver *)driver, buffer, capacity);
#endif
}
static void log_sounds(void *driver) {
 SoundEvent events[256]; uint32_t n;
 do {
  n = drain(driver, events, 256);
  for (uint32_t i = 0; i < n; i++) {
   const SoundEvent *e = &events[i];
   if (e->kind < 4) g_kinds[e->kind]++; else g_other++;
   fprintf(g_out, "%llu %u %u %u 0x%x 0x%x 0x%x 0x%x 0x%x 0x%x %u\n", g_tick, e->kind, e->frame, e->phase, e->site,
           e->eax, e->edx, e->ebx, e->ecx, e->volume, e->has_volume);
#ifndef PARITY_MACOS
   if (g_audio) { RlwAudioRequest r = {e->kind, e->frame, e->phase, e->site, e->eax, e->edx, e->ebx, e->ecx, e->volume, e->has_volume}; rlw_audio_requests(g_audio, &r, 1); }
#endif
  }
 } while (n == 256);
}
static unsigned long long g_cd;
static void log_cd(void *driver) {
 static RlFrameEvent events[8192]; uint32_t n = 0;
#ifdef PARITY_MACOS
 int c = macos_live_cd_events((MacOSLiveDriver *)driver, events, 8192, &n);
#else
 int c = rlw_driver_cd_events((RlwDriver *)driver, events, 8192, &n);
#endif
 if (c) { fprintf(g_out, "%llu cd-error %d\n", g_tick, c); return; }
 for (uint32_t i = 0; i < n && i < 8192; i++)
  if (events[i].kind == 2 && events[i].id == 102) { g_cd++; fprintf(g_out, "%llu cd 0x%x %u %u\n", g_tick, events[i].site, events[i].registers[0], events[i].registers[1] & 255); }
}
static uint64_t dropped(void *driver) {
#ifdef PARITY_MACOS
 return macos_live_sound_dropped((MacOSLiveDriver *)driver);
#else
 return rlw_driver_sound_dropped((RlwDriver *)driver);
#endif
}
static void summary(void *driver, const char *what) {
 fprintf(g_out, "# %s ticks=%llu manage=%llu post=%llu stop=%llu stop_all=%llu other=%llu dropped=%llu cd=%llu\n", what, g_tick,
         g_kinds[0], g_kinds[1], g_kinds[2], g_kinds[3], g_other, (unsigned long long)dropped(driver), g_cd);
}

#ifdef PARITY_MACOS
#define D ((MacOSLiveDriver *)driver)
static uint32_t op_mode(void *driver) { return macos_live_shell_mode(D); }
static int op_state(void *driver, RlShellInput *o) { return macos_live_shell_state(D, o); }
static int op_input(void *driver, const RlShellInput *i, RlShellEvent *o) { return macos_live_shell_input(D, i, o); }
static int op_draw(void *driver, uint32_t a) { return macos_live_shell_draw(D, a); }
static int op_show(void *driver) { return macos_live_shell_show(D); }
static int op_prepare(void *driver) { return macos_live_shell_prepare(D); }
static int op_request(void *driver) { return macos_live_shell_request(D); }
static int op_load(void *driver) { return macos_live_shell_load(D); }
static int op_slots(void *driver, const uint8_t *t, uint32_t n) { return macos_live_shell_slots(D, t, n); }
static int op_zone(void *driver, uint32_t z, int32_t r[4]) { return macos_live_shell_zone(D, z, r); }
static int op_credits_frame(void *driver, uint32_t ms, RlShellCredits *o) { return macos_live_shell_credits_frame(D, ms, o); }
static int op_credits_present(void *driver, const RlShellCredits *f, const uint8_t *p, const uint8_t *c) { return macos_live_shell_credits_present(D, f, p, c); }
static void op_error(void *driver, char *o, size_t n) { macos_live_save_error(D, o, (uint32_t)n); }
static int op_options_input(void *driver, const RlShellInput *i, uint32_t k, RlOpcoesEvento *o) { return macos_live_options_input(D, i, k, o); }
static uint32_t op_options_open(void *driver) { return macos_live_options_open(D); }
static int op_options_sync(void *driver, const RlOpcoesValores *v) { return macos_live_options_sync(D, v); }
static int op_options_button(void *driver, int32_t r[4]) { return macos_live_options_button(D, r); }
static int op_advance_raw(void *driver) { return macos_live_advance(D, NULL, NULL); }
static int op_event(void *driver, uint32_t k, uint32_t m, int32_t x, int32_t y, uint32_t w, uint32_t v, uint32_t dn) { return macos_live_event(D, k, m, x, y, w, v, dn); }
static int op_begin(void *driver) { return macos_live_shell_begin(D); }
static uint32_t op_screen(void *driver) { uint32_t s = 0; macos_live_campaign_screen(D, &s); return s; }
static uint64_t op_frames(void *driver) { MacOSLiveStatus s; macos_live_status(D, &s); return s.frames; }
static const uint8_t *op_pixels(void *driver) { return macos_live_pixels(D); }
static const uint8_t *op_palette(void *driver) { return macos_live_palette(D); }
static void op_stop(void *driver, char *o, size_t n) { MacOSLiveStatus s; macos_live_status(D, &s); snprintf(o, n, "%s", s.message); }
static int op_save(void *driver, const char *p) { return macos_live_save(D, p); }
static int op_autosave(void *driver, const char *p) { return macos_live_autosave(D, p); }
static int op_load_file(void *driver, const char *p) { return macos_live_load(D, p); }
static int op_save_slot(void *driver, const char *p, int32_t *saved) { return macos_live_shell_save_slot(D, p, saved); }
static int op_advance(void *driver) { int c = op_advance_raw(driver); g_tick++; log_sounds(driver); if (!c) log_cd(driver); return c; }
static const RlwDemoOps OPS = {{op_mode, op_state, op_input, op_draw, op_show, op_prepare, op_request, op_load, op_slots, op_zone,
 macos_live_shell_click_clock, op_credits_frame, op_credits_present, op_error, op_options_input, op_options_open, op_options_sync, op_options_button}, op_advance, op_event, op_begin, op_screen, op_frames, op_pixels, op_palette, op_stop,
 op_save, op_autosave, op_load_file, op_save_slot};
#undef D
#else
static int op_advance(void *driver) { int c = RLW_WINDOWS_OPS.advance(driver); g_tick++; log_sounds(driver); if (!c) log_cd(driver); return c; }
#endif

static int menu_walk(const char *root, const char *path) {
 g_out = fopen(path, "w"); if (!g_out) return 2;
 char error[1024] = {0};
#ifdef PARITY_MACOS
 static const char logical[] = "C:\\RISING";
 MacOSLiveDriver *d = macos_live_create_menu_host_v1(NULL, root, 1, (const uint8_t *)logical, (uint32_t)strlen(logical), error, sizeof(error));
 const char *name = "macos";
 if (d && macos_live_sound_capture(d, 1)) { snprintf(error, sizeof(error), "sound capture refused"); macos_live_free(d); d = NULL; }
 const RlwDemoOps *ops = &OPS;
#else
 RlwDriver *d = rlw_driver_create_menu(root, 1, error, sizeof(error));
 const char *name = "windows";
 if (d && rlw_driver_sound_capture(d, 1)) { snprintf(error, sizeof(error), "sound capture refused"); rlw_driver_free(d); d = NULL; }
 RlwDemoOps wrapped = RLW_WINDOWS_OPS; wrapped.advance = op_advance; const RlwDemoOps *ops = &wrapped;
#endif
 if (!d) { fprintf(stderr, "%s menu create: %s\n", name, error); fclose(g_out); return 3; }
 /* The walk's own lines are the menu parity's business (parity.sh); only the sounds go to the file. */
 FILE *sink = tmpfile(); if (!sink) { fclose(g_out); return 2; }
 int rc = rlw_menu_demo_run(ops, d, sink);
 fclose(sink);
 summary(d, rc ? "menu-walk-ended-early" : "menu-walk");
 fclose(g_out);
#ifdef PARITY_MACOS
 macos_live_free(d);
#else
 rlw_driver_free(d);
#endif
 printf("%s: menu walk %s, %llu sound events\n", name, rc ? "ended early" : "complete", g_kinds[0] + g_kinds[1] + g_kinds[2] + g_kinds[3] + g_other);
 return 0;
}

#ifndef PARITY_MACOS
static void sample_line(void *ctx, unsigned bank, uint32_t entry, uint32_t variant, uint32_t key) {
 RlwSoundBanks *banks = ((void **)ctx)[0]; FILE *out = ((void **)ctx)[1];
 const RlwSample *s = rlw_sound_sample(banks, key);
 uint8_t *bytes = s ? malloc((size_t)s->frames * 2 + 1) : NULL;
 if (s && bytes) for (uint32_t i = 0; i < s->frames; i++) { bytes[2 * i] = (uint8_t)s->pcm[i]; bytes[2 * i + 1] = (uint8_t)((uint16_t)s->pcm[i] >> 8); }
 uint8_t digest[32]; char hex[65] = "-";
 if (bytes) { rlw_sha256(bytes, (size_t)s->frames * 2, digest); rlw_sha256_hex(digest, hex); }
 fprintf(out, "%c %u %u %u %s\n", "ABP"[bank], entry, variant, s ? s->frames : 0, hex); free(bytes);
}
static int samples(const char *folder, const char *path) {
 char error[512] = {0}; RlwSoundBanks *banks = rlw_sound_banks_open(folder, error, sizeof(error));
 if (!banks) { fprintf(stderr, "%s\n", error); return 3; }
 FILE *out = fopen(path, "w"); if (!out) { rlw_sound_banks_free(banks); return 2; }
 void *ctx[2] = {banks, out}; rlw_sound_each(banks, sample_line, ctx);
 fclose(out); rlw_sound_banks_free(banks); return 0;
}
static void wav_render(uint32_t frames) {
 if (!g_audio) return;
 if (g_wav_frames + frames > g_wav_capacity) {
  uint64_t grown = (g_wav_capacity + frames) * 2; int16_t *p = realloc(g_wav, (size_t)grown * 4); if (!p) return;
  g_wav = p; g_wav_capacity = grown;
 }
 rlw_audio_render(g_audio, g_wav + 2 * g_wav_frames, frames); g_wav_frames += frames;
}
static int wav_finish(const char *path) {
 uint32_t bytes = (uint32_t)(g_wav_frames * 4); uint8_t head[44];
 memcpy(head, "RIFF", 4); uint32_t v[] = {36 + bytes}; memcpy(head + 4, v, 4); memcpy(head + 8, "WAVEfmt ", 8);
 const uint8_t fmt[20] = {16, 0, 0, 0, 1, 0, 2, 0, 0x44, 0xac, 0, 0, 0x10, 0xb1, 2, 0, 4, 0, 16, 0};
 memcpy(head + 16, fmt, 20); memcpy(head + 36, "data", 4); memcpy(head + 40, &bytes, 4);
 int rc = rlw_durable_write(path, head, 44, (const uint8_t *)g_wav, bytes);
 int peak = 0; double sum = 0; uint64_t nonzero = 0;
 for (uint64_t i = 0; i < 2 * g_wav_frames; i++) { int a = abs(g_wav[i]); if (a > peak) peak = a; if (a) nonzero++; sum += (double)g_wav[i] * g_wav[i]; }
 RlwAudioStats st; rlw_audio_stats(g_audio, &st);
 printf("wav: %llu frames (%.2f s), peak %d, rms %.1f, nonzero %llu, clipped %llu, plays %llu, missing %llu, finished %llu\n",
        (unsigned long long)g_wav_frames, g_wav_frames / 44100.0, peak, g_wav_frames ? sqrt(sum / (2.0 * g_wav_frames)) : 0.0,
        (unsigned long long)nonzero, (unsigned long long)rlw_mixer_clipped(rlw_audio_mixer(g_audio)), (unsigned long long)st.plays_started,
        (unsigned long long)st.missing_samples, (unsigned long long)st.finished);
 return rc;
}
#endif

int main(int argc, char **argv) {
#ifndef PARITY_MACOS
 if (argc == 4 && !strcmp(argv[1], "samples")) return samples(argv[2], argv[3]);
 if (argc >= 3 && !strcmp(argv[1], "movies")) {
  int bad = 0;
  for (int i = 2; i < argc; i++) {
   char error[256] = {0}; RlwMovie *m = rlw_movie_open(argv[i], error, sizeof(error));
   const char *name = strrchr(argv[i], '/'); name = name ? name + 1 : argv[i];
   if (!m) { printf("%s erro %s\n", name, error); bad = 1; continue; }
   RlwSha256 frames; rlw_sha256_init(&frames); size_t bytes = (size_t)rlw_movie_width(m) * rlw_movie_height(m) * 4;
   for (uint32_t k = 0; k < rlw_movie_frames(m); k++) {
    /* Frame k is due at k periods (+ a hair against rounding): one new picture per step. */
    if (rlw_movie_advance(m, k * rlw_movie_frame_ms(m) + 0.01) < 0) { printf("%s erro %s\n", name, rlw_movie_error(m)); bad = 1; break; }
    rlw_sha256_update(&frames, rlw_movie_pixels(m), bytes);
   }
   uint64_t samples = 0; const int16_t *pcm = rlw_movie_pcm(m, &samples);
   uint8_t d1[32], d2[32]; char h1[65], h2[65]; rlw_sha256_final(&frames, d1); rlw_sha256(pcm, (size_t)samples * 2, d2);
   rlw_sha256_hex(d1, h1); rlw_sha256_hex(d2, h2);
   if (rlw_movie_decoded(m) != rlw_movie_frames(m)) bad = 1;
   printf("%s %u %llu %s %s\n", name, rlw_movie_decoded(m), (unsigned long long)samples, h1, h2);
   rlw_movie_free(m);
  }
  return bad;
 }
 if (argc == 4 && !strcmp(argv[1], "music")) {
  char error[512] = {0}; int c = rlw_cdmusic_import(argv[2], argv[3], error, sizeof(error));
  printf("música: %s\n", c == 0 ? "faixas 2 e 3 = WAVs fixados (SHA-256)" : c == 2 ? "sem faixas de áudio nesta fonte" : error);
  return c == 1;
 }
 const char *wav_path = NULL;
 if (argc >= 7 && !strcmp(argv[4], "--wav")) {
  char error[512] = {0}; RlwSoundBanks *banks = rlw_sound_banks_open(argv[5], error, sizeof(error));
  if (!banks) { fprintf(stderr, "%s\n", error); return 3; }
  g_audio = rlw_audio_create(banks, 44100, NULL); wav_path = argv[6];
  if (!g_audio) return 3;
 }
#endif
 if (argc < 4) { fprintf(stderr, "usage: audio_parity <Dados> <ticks>|menu <events.txt>\n"); return 64; }
 if (!strcmp(argv[2], "menu")) return menu_walk(argv[1], argv[3]);
 const char *root = argv[1]; unsigned ticks = (unsigned)strtoul(argv[2], NULL, 10);
 g_out = fopen(argv[3], "w"); if (!g_out) return 2;
 char error[1024] = {0};
#ifdef PARITY_MACOS
 static const char logical[] = "C:\\RISING";
 MacOSLiveDriver *d = macos_live_create_mission_host_v1(NULL, root, "MONDE01.ETA", 1, (const uint8_t *)logical, (uint32_t)strlen(logical), error, sizeof(error));
 if (d && macos_live_validation(d, 0)) { MacOSLiveStatus s; macos_live_status(d, &s); snprintf(error, sizeof(error), "%s", s.message); macos_live_free(d); d = NULL; }
 if (d && macos_live_sound_capture(d, 1)) { snprintf(error, sizeof(error), "sound capture refused"); macos_live_free(d); d = NULL; }
 const char *name = "macos";
#else
 RlwDriver *d = rlw_driver_create(root, "MONDE01.ETA", 1, error, sizeof(error));
 if (d && rlw_driver_sound_capture(d, 1)) { snprintf(error, sizeof(error), "sound capture refused"); rlw_driver_free(d); d = NULL; }
 const char *name = "windows";
#endif
 if (!d) { fprintf(stderr, "%s create: %s\n", name, error); return 3; }
 int rc = 0;
 for (unsigned i = 0; i <= ticks && !rc; i++) {
  uint64_t done;
#ifdef PARITY_MACOS
  MacOSLiveStatus s; macos_live_status(d, &s); done = s.frames;
#else
  RlwStatus s; rlw_driver_status(d, &s); done = s.frames;
#endif
  if (i)
   for (unsigned k = 0; k < RLW_DEMO_COUNT; k++) if (RLW_DEMO[k].frame == done + 1) {
    const RlwDemoEvent *e = &RLW_DEMO[k];
#ifdef PARITY_MACOS
    if (macos_live_event(d, e->kind, e->message, e->x, e->y, e->wparam, e->vk, e->down)) rc = 4;
#else
    if (rlw_driver_event(d, e->kind, e->message, e->x, e->y, e->wparam, e->vk, e->down)) rc = 4;
#endif
   }
  SoakEvent extra[2]; int count = i ? soak(done + 1, extra) : 0;
  for (int k = 0; k < count && !rc; k++) {
#ifdef PARITY_MACOS
   if (macos_live_event(d, extra[k].kind, extra[k].message, extra[k].x, extra[k].y, 0, 0, 0)) rc = 4;
#else
   if (rlw_driver_event(d, extra[k].kind, extra[k].message, extra[k].x, extra[k].y, 0, 0, 0)) rc = 4;
#endif
  }
#ifdef PARITY_MACOS
  if (!rc && macos_live_advance(d, NULL, NULL)) rc = 5;
  if (rc) { macos_live_status(d, &s); fprintf(stderr, "%s stop at tick %u: %s\n", name, i, s.message); }
#else
  if (!rc && rlw_driver_advance(d)) rc = 5;
  if (rc) { rlw_driver_status(d, &s); fprintf(stderr, "%s stop at tick %u: %s\n", name, i, s.message); }
#endif
  g_tick = i; log_sounds(d); if (!rc) log_cd(d);
#ifndef PARITY_MACOS
  wav_render(1470);
#endif
 }
 summary(d, rc ? "play-stopped" : "play");
 fclose(g_out);
#ifdef PARITY_MACOS
 macos_live_free(d);
#else
 rlw_driver_free(d);
 if (wav_path && wav_finish(wav_path)) rc = 7;
 rlw_audio_free(g_audio); free(g_wav);
#endif
 printf("%s: %u ticks, %llu sound events (%llu PostSound)\n", name, ticks + 1, g_kinds[0] + g_kinds[1] + g_kinds[2] + g_kinds[3] + g_other, g_kinds[1]);
 return rc;
}
