/* Autodesk FLC/FLI decoder (host presentation only), a C port of
 * macos/src/NativeFLC.swift: an 8-bit frame buffer plus a
 * 256-colour RGB palette, advanced one frame chunk at a time. Chunks COLOR_256
 * (4), COLOR_64 (11), BLACK (13), BRUN (15), COPY (16), LC (12) and SS2 (7);
 * others (PSTAMP 18, ...) are skipped. Used for the original credits screen. */
#ifndef RLW_FLC_H
#define RLW_FLC_H
#include <stddef.h>
#include <stdint.h>
typedef struct {
 uint8_t *data; uint32_t size;
 uint32_t frames, width, height;
 double delay_ms;            /* FLI speed (1/70 s) converted to milliseconds */
 uint8_t *pixels;            /* width x height */
 uint8_t palette[768];       /* 256 x RGB, 8 bits per component */
 int palette_changed;
 uint32_t shown;             /* frame chunks decoded so far */
 uint32_t offset, first_frame;
 char error[160];
} RlwFlc;
/* Takes ownership of `data` (malloc'd) on success and on failure. 0 or nonzero
 * with flc->error set. */
int rlw_flc_init(RlwFlc *flc, uint8_t *data, uint32_t size);
int rlw_flc_open(RlwFlc *flc, const char *path);
int rlw_flc_next(RlwFlc *flc);
void rlw_flc_rewind(RlwFlc *flc);
void rlw_flc_free(RlwFlc *flc);
#endif
