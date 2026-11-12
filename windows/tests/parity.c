/* Driver parity runner: the same stage, frames and synthetic input through
 *  - the Windows host driver (rlw_driver.c), built natively for any OS, or
 *  - with -DPARITY_MACOS, the macOS host driver (macos/src, read-only)
 * and one line per host tick: "<frames> <screen> <sha256(pixels||palette)>".
 * Identical files prove the Windows port keeps the macOS call order, input
 * mapping and presentation inputs (the modo fiel invariant). Needs an imported
 * CD (Dados folder); no game data is part of this test.
 *   parity <Dados> <frames> <hashes.txt> [--demo-input]
 * Menu parity (milestone 2): the scripted walk of rlw_menu_demo.c from the
 * original main menu through briefing, play, pause and restart:
 *   parity <Dados> menu <walk.txt>
 * Save parity (milestone 3): the save walk of rlw_menu_demo.c from MONDE01 in
 * play mode (autosave, save, load and replay, pause list save/load), writing in
 * an empty folder; the slot and .view files must be byte-identical too:
 *   parity <Dados> save <walk.txt> <folder> */
#include "rlw_demo.h"
#include "rlw_menu_demo.h"
#include "rlw_sha256.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef PARITY_MACOS
#include "MacOSLiveDriver.h"
#else
#include "rlw_driver.h"
#include "rlw_menu_ops.h"
#endif

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
static int op_advance(void *driver) { return macos_live_advance(D, NULL, NULL); }
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
static const RlwDemoOps OPS = {{op_mode, op_state, op_input, op_draw, op_show, op_prepare, op_request, op_load, op_slots, op_zone,
 macos_live_shell_click_clock, op_credits_frame, op_credits_present, op_error, op_options_input, op_options_open, op_options_sync, op_options_button}, op_advance, op_event, op_begin, op_screen, op_frames, op_pixels, op_palette, op_stop,
 op_save, op_autosave, op_load_file, op_save_slot};
#else
#define OPS RLW_WINDOWS_OPS
#endif
#undef D

static int menu_walk(const char *root, const char *path) {
 FILE *out = fopen(path, "w"); if (!out) return 2;
 char error[1024] = {0};
#ifdef PARITY_MACOS
 static const char logical[] = "C:\\RISING";
 MacOSLiveDriver *d = macos_live_create_menu_host_v1(NULL, root, 1, (const uint8_t *)logical, (uint32_t)strlen(logical), error, sizeof(error));
 const char *name = "macos";
#else
 RlwDriver *d = rlw_driver_create_menu(root, 1, error, sizeof(error));
 const char *name = "windows";
#endif
 if (!d) { fprintf(stderr, "%s menu create: %s\n", name, error); fclose(out); return 3; }
 int rc = rlw_menu_demo_run(&OPS, d, out);
 fclose(out);
#ifdef PARITY_MACOS
 macos_live_free(d);
#else
 rlw_driver_free(d);
#endif
 printf("%s: menu walk %s\n", name, rc ? "ended early (see the file)" : "complete");
 return 0;
}

static int save_walk(const char *root, const char *path, const char *directory) {
 FILE *out = fopen(path, "w"); if (!out) return 2;
 char error[1024] = {0};
#ifdef PARITY_MACOS
 static const char logical[] = "C:\\RISING";
 MacOSLiveDriver *d = macos_live_create_mission_host_v1(NULL, root, "MONDE01.ETA", 1, (const uint8_t *)logical, (uint32_t)strlen(logical), error, sizeof(error));
 if (d && macos_live_validation(d, 0)) { MacOSLiveStatus s; macos_live_status(d, &s); snprintf(error, sizeof(error), "%s", s.message); macos_live_free(d); d = NULL; }
 const char *name = "macos";
#else
 RlwDriver *d = rlw_driver_create(root, "MONDE01.ETA", 1, error, sizeof(error));
 const char *name = "windows";
#endif
 if (!d) { fprintf(stderr, "%s save create: %s\n", name, error); fclose(out); return 3; }
 int rc = rlw_save_demo_run(&OPS, d, directory, out);
 fclose(out);
#ifdef PARITY_MACOS
 macos_live_free(d);
#else
 rlw_driver_free(d);
#endif
 printf("%s: save walk %s\n", name, rc ? "ended early (see the file)" : "complete");
 return 0;
}

int main(int argc, char **argv) {
 if (argc < 4) { fprintf(stderr, "usage: parity <Dados> <frames>|menu|save <hashes.txt> [--demo-input|<folder>]\n"); return 64; }
 if (!strcmp(argv[2], "menu")) return menu_walk(argv[1], argv[3]);
 if (!strcmp(argv[2], "save")) { if (argc < 5) { fprintf(stderr, "usage: parity <Dados> save <walk.txt> <folder>\n"); return 64; } return save_walk(argv[1], argv[3], argv[4]); }
 const char *root = argv[1]; unsigned frames = (unsigned)strtoul(argv[2], NULL, 10);
 int demo = argc > 4 && !strcmp(argv[4], "--demo-input");
 FILE *out = fopen(argv[3], "w"); if (!out) return 2;
 char error[1024] = {0};
#ifdef PARITY_MACOS
 static const char logical[] = "C:\\RISING";
 MacOSLiveDriver *d = macos_live_create_mission_host_v1(NULL, root, "MONDE01.ETA", 1, (const uint8_t *)logical, (uint32_t)strlen(logical), error, sizeof(error));
 if (d && macos_live_validation(d, 0)) { MacOSLiveStatus s; macos_live_status(d, &s); snprintf(error, sizeof(error), "%s", s.message); macos_live_free(d); d = NULL; }
 const char *name = "macos";
#else
 RlwDriver *d = rlw_driver_create(root, "MONDE01.ETA", 1, error, sizeof(error));
 const char *name = "windows";
#endif
 if (!d) { fprintf(stderr, "%s create: %s\n", name, error); return 3; }
 int rc = 0;
 for (unsigned i = 0; i <= frames; i++) {
  uint64_t done; uint32_t screen = 0; const uint8_t *pixels, *palette; char message[512];
#ifdef PARITY_MACOS
  MacOSLiveStatus s; macos_live_status(d, &s); done = s.frames;
#else
  RlwStatus s; rlw_driver_status(d, &s); done = s.frames;
#endif
  if (demo && i)
   for (unsigned k = 0; k < RLW_DEMO_COUNT; k++) if (RLW_DEMO[k].frame == done + 1) {
    const RlwDemoEvent *e = &RLW_DEMO[k];
#ifdef PARITY_MACOS
    if (macos_live_event(d, e->kind, e->message, e->x, e->y, e->wparam, e->vk, e->down)) rc = 4;
#else
    if (rlw_driver_event(d, e->kind, e->message, e->x, e->y, e->wparam, e->vk, e->down)) rc = 4;
#endif
   }
#ifdef PARITY_MACOS
  if (!rc && macos_live_advance(d, NULL, NULL)) rc = 5;
  macos_live_status(d, &s); done = s.frames; macos_live_campaign_screen(d, &screen);
  pixels = macos_live_pixels(d); palette = macos_live_palette(d); snprintf(message, sizeof(message), "%s", s.message);
#else
  if (!rc && rlw_driver_advance(d)) rc = 5;
  rlw_driver_status(d, &s); done = s.frames; screen = s.screen;
  pixels = rlw_driver_pixels(d); palette = rlw_driver_palette(d); snprintf(message, sizeof(message), "%s", s.message);
#endif
  if (rc) { fprintf(stderr, "%s stop at tick %u: %s\n", name, i, message); break; }
  RlwSha256 c; uint8_t digest[32]; char hex[65];
  rlw_sha256_init(&c); rlw_sha256_update(&c, pixels, 480000); rlw_sha256_update(&c, palette, 768);
  rlw_sha256_final(&c, digest); rlw_sha256_hex(digest, hex);
  fprintf(out, "%llu %u %s\n", (unsigned long long)done, screen, hex);
 }
 fclose(out);
#ifdef PARITY_MACOS
 macos_live_free(d);
#else
 rlw_driver_free(d);
#endif
 if (!rc) printf("%s: %u ticks ok\n", name, frames + 1);
 return rc;
}
