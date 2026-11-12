/* C port of NativeFLC.swift; same chunk handling and the same tolerances. */
#include "rlw_flc.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t u16(const uint8_t *d, uint32_t o) { return (uint32_t)d[o] | (uint32_t)d[o + 1] << 8; }
static uint32_t u32(const uint8_t *d, uint32_t o) { return u16(d, o) | u16(d, o + 2) << 16; }

int rlw_flc_init(RlwFlc *f, uint8_t *data, uint32_t size) {
 memset(f, 0, sizeof(*f)); f->data = data; f->size = size;
 if (!data || size < 128) { snprintf(f->error, sizeof(f->error), "FLC header truncated"); return 1; }
 uint32_t magic = u16(data, 4);
 if (magic != 0xAF12 && magic != 0xAF11) { snprintf(f->error, sizeof(f->error), "not an FLC/FLI (magic %04x)", magic); return 1; }
 f->frames = u16(data, 6); f->width = u16(data, 8); f->height = u16(data, 10);
 if (!f->frames || !f->width || !f->height || f->width > 800 || f->height > 600 || (u16(data, 12) != 8 && u16(data, 12) != 0)) {
  snprintf(f->error, sizeof(f->error), "unsupported FLC geometry"); return 1;
 }
 uint32_t speed = u32(data, 16);
 f->delay_ms = magic == 0xAF11 ? (double)speed * 1000.0 / 70.0 : (double)speed;
 uint32_t frame1 = magic == 0xAF12 ? u32(data, 80) : 0;
 f->first_frame = frame1 >= 128 && frame1 < size ? frame1 : 128;
 f->offset = f->first_frame;
 f->pixels = calloc((size_t)f->width * f->height, 1);
 if (!f->pixels) { snprintf(f->error, sizeof(f->error), "out of memory"); return 1; }
 return 0;
}

int rlw_flc_open(RlwFlc *f, const char *path) {
 uint32_t n = 0; uint8_t *data = rlw_read_file(path, &n, 256u * 1024u * 1024u);
 if (!data) { memset(f, 0, sizeof(*f)); snprintf(f->error, sizeof(f->error), "cannot read %.120s", path); return 1; }
 return rlw_flc_init(f, data, n);
}

void rlw_flc_free(RlwFlc *f) { if (!f) return; free(f->data); free(f->pixels); memset(f, 0, sizeof(*f)); }
void rlw_flc_rewind(RlwFlc *f) { f->offset = f->first_frame; f->shown = 0; }

typedef struct { const uint8_t *d; uint32_t p, end; int bad; } Reader;
static uint32_t byte(Reader *r) { if (r->p >= r->end) { r->bad = 1; return 0; } return r->d[r->p++]; }
static uint32_t word(Reader *r) { uint32_t lo = byte(r); return lo | byte(r) << 8; }

static int fail(RlwFlc *f, uint32_t kind, const char *what) {
 snprintf(f->error, sizeof(f->error), "frame %u chunk %u: %s", f->shown + 1, kind, what); return 1;
}

static int decode(RlwFlc *f, uint32_t kind, uint32_t start, uint32_t end) {
 Reader r = {f->data, start, end, 0};
 uint32_t W = f->width, H = f->height;
 switch (kind) {
 case 4: case 11: {
  uint32_t packets = word(&r), index = 0;
  for (uint32_t k = 0; k < packets && !r.bad; k++) {
   index += byte(&r);
   uint32_t count = byte(&r); if (!count) count = 256;
   for (uint32_t i = 0; i < count && !r.bad; i++) {
    if (index >= 256) return fail(f, kind, "palette overflow");
    for (int c = 0; c < 3; c++) {
     uint32_t v = byte(&r);
     f->palette[index * 3 + c] = kind == 4 ? (uint8_t)v : (uint8_t)((((v & 63) << 2) | ((v & 63) >> 4)) > 255 ? 255 : (((v & 63) << 2) | ((v & 63) >> 4)));
    }
    index++;
   }
  }
  if (r.bad) return fail(f, kind, "truncated");
  f->palette_changed = 1; break;
 }
 case 13: memset(f->pixels, 0, (size_t)W * H); break;
 case 16:
  if (end - start < W * H) return fail(f, kind, "COPY truncated");
  memcpy(f->pixels, f->data + start, (size_t)W * H); break;
 case 15:
  for (uint32_t y = 0; y < H; y++) {
   (void)byte(&r); /* packet count: ignored, the line is filled by width */
   uint32_t x = 0;
   while (x < W) {
    int count = (int)(int8_t)(uint8_t)byte(&r);
    if (r.bad) return fail(f, kind, "truncated");
    if (count >= 0) {
     uint8_t value = (uint8_t)byte(&r); if (r.bad) return fail(f, kind, "truncated");
     if (x + (uint32_t)count > W) return fail(f, kind, "BRUN run past line");
     memset(f->pixels + y * W + x, value, (size_t)count); x += (uint32_t)count;
    } else {
     uint32_t n = (uint32_t)(-count);
     if (x + n > W || r.p + n > end) return fail(f, kind, "BRUN copy past line");
     memcpy(f->pixels + y * W + x, f->data + r.p, n); r.p += n; x += n;
    }
    if (count == 0) break;
   }
  }
  break;
 case 12: {
  uint32_t y = word(&r), lines = word(&r);
  if (r.bad) return fail(f, kind, "truncated");
  for (uint32_t l = 0; l < lines; l++) {
   if (y >= H) return fail(f, kind, "LC line past image");
   uint32_t x = 0, packets = byte(&r);
   for (uint32_t k = 0; k < packets; k++) {
    x += byte(&r);
    int count = (int)(int8_t)(uint8_t)byte(&r);
    if (r.bad) return fail(f, kind, "truncated");
    if (count >= 0) {
     uint32_t n = (uint32_t)count;
     if (x + n > W || r.p + n > end) return fail(f, kind, "LC copy past line");
     memcpy(f->pixels + y * W + x, f->data + r.p, n); r.p += n; x += n;
    } else {
     uint8_t value = (uint8_t)byte(&r); uint32_t n = (uint32_t)(-count);
     if (r.bad) return fail(f, kind, "truncated");
     if (x + n > W) return fail(f, kind, "LC run past line");
     memset(f->pixels + y * W + x, value, n); x += n;
    }
   }
   if (r.bad) return fail(f, kind, "truncated");
   y++;
  }
  break;
 }
 case 7: {
  uint32_t lines = word(&r); int64_t y = 0;
  if (r.bad) return fail(f, kind, "truncated");
  for (uint32_t l = 0; l < lines; l++) {
   uint32_t packets = 0;
   for (;;) {
    uint32_t op = word(&r); if (r.bad) return fail(f, kind, "truncated");
    uint32_t top = op >> 14;
    if (top == 0) { packets = op; break; }
    if (top == 3) { y += 65536 - (int64_t)op; continue; } /* signed negative line skip */
    if (top == 2) {
     if (y < 0 || y >= H) return fail(f, kind, "SS2 last byte past image");
     f->pixels[(uint32_t)y * W + W - 1] = (uint8_t)(op & 0xff); continue;
    }
    return fail(f, kind, "SS2 opcode");
   }
   if (y < 0 || y >= H) return fail(f, kind, "SS2 line past image");
   uint32_t x = 0, row = (uint32_t)y * W;
   for (uint32_t k = 0; k < packets; k++) {
    x += byte(&r);
    int count = (int)(int8_t)(uint8_t)byte(&r);
    if (r.bad) return fail(f, kind, "truncated");
    if (count >= 0) {
     uint32_t n = 2u * (uint32_t)count;
     if (x + n > W || r.p + n > end) return fail(f, kind, "SS2 copy past line");
     memcpy(f->pixels + row + x, f->data + r.p, n); r.p += n; x += n;
    } else {
     uint8_t lo = (uint8_t)byte(&r), hi = (uint8_t)byte(&r); uint32_t n = (uint32_t)(-count);
     if (r.bad) return fail(f, kind, "truncated");
     if (x + 2 * n > W) return fail(f, kind, "SS2 run past line");
     for (uint32_t i = 0; i < n; i++) { f->pixels[row + x + 2 * i] = lo; f->pixels[row + x + 2 * i + 1] = hi; }
     x += 2 * n;
    }
   }
   y++;
  }
  break;
 }
 default: break; /* PSTAMP and other chunks carry no image data for playback. */
 }
 return 0;
}

int rlw_flc_next(RlwFlc *f) {
 f->palette_changed = 0;
 for (;;) {
  if ((uint64_t)f->offset + 16 > f->size) { snprintf(f->error, sizeof(f->error), "frame %u beyond end of file", f->shown + 1); return 1; }
  uint32_t size = u32(f->data, f->offset), type = u16(f->data, f->offset + 4);
  if (size < 16 || (uint64_t)f->offset + size > f->size) { snprintf(f->error, sizeof(f->error), "frame %u size %u invalid", f->shown + 1, size); return 1; }
  if (type == 0xF100) { f->offset += size; continue; }
  if (type != 0xF1FA) { snprintf(f->error, sizeof(f->error), "frame %u chunk type %04x", f->shown + 1, type); return 1; }
  uint32_t count = u16(f->data, f->offset + 6), chunk = f->offset + 16, frame_end = f->offset + size;
  for (uint32_t k = 0; k < count; k++) {
   if (chunk + 6 > frame_end) { snprintf(f->error, sizeof(f->error), "frame %u chunk header beyond frame", f->shown + 1); return 1; }
   uint32_t length = u32(f->data, chunk), kind = u16(f->data, chunk + 4);
   /* Some writers store an odd size and pad; never step past the frame. */
   if (length < 6 || (uint64_t)chunk + length > frame_end) {
    if (length < 6 || (uint64_t)chunk + length - 1 > frame_end) { snprintf(f->error, sizeof(f->error), "frame %u chunk %u size %u", f->shown + 1, kind, length); return 1; }
    length = frame_end - chunk;
   }
   /* The original event files' COPY chunks declare 2 bytes less than their
    * 6 + width x height (the frame size is right): read to the frame. */
   if (kind == 16 && length < 6 + f->width * f->height) { uint32_t want = 6 + f->width * f->height, room = frame_end - chunk; length = want < room ? want : room; }
   if (decode(f, kind, chunk + 6, chunk + length)) return 1;
   chunk += length;
  }
  f->offset += size; f->shown++;
  return 0;
 }
}
