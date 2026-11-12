/* The Windows driver behind the portable menu routing and the scripted walk. */
#include "rlw_menu_ops.h"
#include "rlw_driver.h"
#include <stdio.h>

#define D ((RlwDriver *)driver)
static uint32_t op_mode(void *driver) { return rlw_shell_mode(D); }
static int op_state(void *driver, RlShellInput *o) { return rlw_shell_state(D, o); }
static int op_input(void *driver, const RlShellInput *i, RlShellEvent *o) { return rlw_shell_input(D, i, o); }
static int op_draw(void *driver, uint32_t a) { return rlw_shell_draw(D, a); }
static int op_show(void *driver) { return rlw_shell_show(D); }
static int op_prepare(void *driver) { return rlw_shell_prepare(D); }
static int op_request(void *driver) { return rlw_shell_request(D); }
static int op_load(void *driver) { return rlw_shell_load(D); }
static int op_slots(void *driver, const uint8_t *t, uint32_t n) { return rlw_shell_slots(D, t, n); }
static int op_zone(void *driver, uint32_t z, int32_t r[4]) { return rlw_shell_zone(D, z, r); }
static int op_credits_frame(void *driver, uint32_t ms, RlShellCredits *o) { return rlw_shell_credits_frame(D, ms, o); }
static int op_credits_present(void *driver, const RlShellCredits *f, const uint8_t *p, const uint8_t *c) { return rlw_shell_credits_present(D, f, p, c); }
static void op_error(void *driver, char *o, size_t n) { rlw_shell_error(D, o, n); }
static int op_advance(void *driver) { return rlw_driver_advance(D); }
static int op_event(void *driver, uint32_t k, uint32_t m, int32_t x, int32_t y, uint32_t w, uint32_t v, uint32_t dn) { return rlw_driver_event(D, k, m, x, y, w, v, dn); }
static int op_begin(void *driver) { return rlw_shell_begin(D); }
static uint32_t op_screen(void *driver) { RlwStatus s; rlw_driver_status(D, &s); return s.screen; }
static uint64_t op_frames(void *driver) { RlwStatus s; rlw_driver_status(D, &s); return s.frames; }
static const uint8_t *op_pixels(void *driver) { return rlw_driver_pixels(D); }
static const uint8_t *op_palette(void *driver) { return rlw_driver_palette(D); }
static void op_stop(void *driver, char *o, size_t n) { RlwStatus s; rlw_driver_status(D, &s); snprintf(o, n, "%s", s.message); }
const RlwDemoOps RLW_WINDOWS_OPS = {{op_mode, op_state, op_input, op_draw, op_show, op_prepare, op_request, op_load, op_slots, op_zone,
 rlw_shell_click_clock, op_credits_frame, op_credits_present, op_error}, op_advance, op_event, op_begin, op_screen, op_frames, op_pixels, op_palette, op_stop};
#undef D
