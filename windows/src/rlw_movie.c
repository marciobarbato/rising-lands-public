/* Original movies: AVI container, MS ADPCM, Cinepak (see rlw_movie.h).
 * Line-by-line port of NativeDefeatMovie.swift (NativeAVIMovie, NativeMSADPCM,
 * NativeCinepakDecoder); same tolerances and error cases. */
#include "rlw_movie.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ---------------- MS ADPCM ---------------- */
static const int32_t ADAPTATION[16] = {230, 230, 230, 230, 307, 409, 512, 614, 768, 614, 512, 409, 307, 230, 230, 230};
static int32_t s16le(const uint8_t *p) { return (int16_t)(uint16_t)(p[0] | (p[1] << 8)); }

int64_t rlw_msadpcm_decode(const uint8_t *bytes, size_t length, const RlwAudioFormat *f, int16_t *out, uint64_t capacity) {
 uint32_t channels = f->channels, header = 7 * channels, pairs = f->coefficient_count;
 if (channels < 1 || channels > 2 || !f->block_align) return -1;
 uint64_t written = 0;
#define PUT(v) do { if (out && written < capacity) out[written] = (int16_t)(v); written++; } while (0)
 for (size_t block = 0; block + header <= length; block += f->block_align) {
  size_t end = block + f->block_align < length ? block + f->block_align : length;
  /* A short final block holds fewer samples, as its byte count says. */
  int64_t samples = (int64_t)(end - block - header) * 2 / channels + 2;
  if (f->samples_per_block > 0 && samples > (int64_t)f->samples_per_block) samples = f->samples_per_block;
  int32_t c1[2] = {0, 0}, c2[2] = {0, 0}, delta[2] = {0, 0}, s1[2] = {0, 0}, s2[2] = {0, 0};
  for (uint32_t ch = 0; ch < channels; ch++) {
   uint32_t predictor = bytes[block + ch];
   if (predictor >= pairs) return -1;
   c1[ch] = f->coefficients[2 * predictor]; c2[ch] = f->coefficients[2 * predictor + 1];
   delta[ch] = s16le(bytes + block + channels + 2 * ch);
   s1[ch] = s16le(bytes + block + 3 * channels + 2 * ch);
   s2[ch] = s16le(bytes + block + 5 * channels + 2 * ch);
  }
  for (uint32_t ch = 0; ch < channels; ch++) PUT(s2[ch]);
  for (uint32_t ch = 0; ch < channels; ch++) PUT(s1[ch]);
  size_t p = block + header;
  for (int64_t i = 0; i < (samples - 2) * (int64_t)channels / 2; i++) {
   if (p >= end) break;
   int32_t byte = bytes[p++];
   for (int half = 0; half < 2; half++) {
    uint32_t ch = half ? channels - 1 : 0; int32_t nibble = half ? byte & 15 : byte >> 4;
    int32_t predicted = (int32_t)(((int64_t)s1[ch] * c1[ch] + (int64_t)s2[ch] * c2[ch]) / 256);
    predicted += (nibble >= 8 ? nibble - 16 : nibble) * delta[ch];
    if (predicted < -32768) predicted = -32768;
    if (predicted > 32767) predicted = 32767;
    s2[ch] = s1[ch]; s1[ch] = predicted;
    int64_t d = ((int64_t)ADAPTATION[nibble] * delta[ch]) >> 8;
    if (d > 0x7fffffff / 768) d = 0x7fffffff / 768;
    if (d < 16) d = 16;
    delta[ch] = (int32_t)d;
    PUT(predicted);
   }
  }
 }
#undef PUT
 return (int64_t)written;
}

/* ---------------- Cinepak ---------------- */
#define MAX_STRIPS 32
#define BOOK (256 * 16)
struct RlwCinepak { uint32_t width, height; uint8_t *pixels, *v4, *v1; };
RlwCinepak *rlw_cinepak_create(uint32_t width, uint32_t height) {
 if (!width || !height || width > 4096 || height > 4096) return NULL;
 RlwCinepak *c = calloc(1, sizeof(*c)); if (!c) return NULL;
 c->width = width; c->height = height;
 c->pixels = calloc((size_t)width * height, 4); c->v4 = calloc(MAX_STRIPS, BOOK); c->v1 = calloc(MAX_STRIPS, BOOK);
 if (!c->pixels || !c->v4 || !c->v1) { rlw_cinepak_free(c); return NULL; }
 for (size_t i = 3; i < (size_t)width * height * 4; i += 4) c->pixels[i] = 255;
 return c;
}
void rlw_cinepak_free(RlwCinepak *c) { if (c) { free(c->pixels); free(c->v4); free(c->v1); free(c); } }
const uint8_t *rlw_cinepak_pixels(const RlwCinepak *c) { return c ? c->pixels : NULL; }

static uint8_t clamp8(int v) { return (uint8_t)(v < 0 ? 0 : v > 255 ? 255 : v); }
static uint32_t be32(const uint8_t *p) { return (uint32_t)p[0] << 24 | (uint32_t)p[1] << 16 | (uint32_t)p[2] << 8 | p[3]; }

static void codebook(uint8_t *book, uint32_t strip, uint32_t id, const uint8_t *f, size_t start, size_t end) {
 size_t n = id & 4 ? 4 : 6, p = start; int partial = id & 1; uint32_t flag = 0, mask = 0;
 for (uint32_t entry = 0; entry < 256; entry++) {
  if (partial) {
   mask >>= 1;
   if (!mask) { if (p + 4 > end) return; flag = be32(f + p); p += 4; mask = 0x80000000u; }
   if (!(flag & mask)) continue;
  }
  if (p + n > end) return;
  size_t base = (size_t)strip * BOOK + entry * 16;
  int u = n == 6 ? (int8_t)f[p + 4] : 0, v = n == 6 ? (int8_t)f[p + 5] : 0;
  for (int k = 0; k < 4; k++) {
   int y = f[p + k];
   book[base + 4 * k] = clamp8(y + v * 2);
   book[base + 4 * k + 1] = clamp8(y - u / 2 - v);
   book[base + 4 * k + 2] = clamp8(y + u * 2);
   book[base + 4 * k + 3] = 255;
  }
  p += n;
 }
}

static void put(RlwCinepak *c, const uint8_t *book, size_t from, uint32_t x, uint32_t y) {
 if (y >= c->height) return;
 size_t o = ((size_t)y * c->width + x) * 4;
 c->pixels[o] = book[from]; c->pixels[o + 1] = book[from + 1]; c->pixels[o + 2] = book[from + 2];
}

static int vectors(RlwCinepak *c, uint32_t strip, uint32_t id, uint32_t y1, uint32_t y2, const uint8_t *f, size_t start, size_t end,
                   char *error, size_t capacity) {
 size_t p = start, cb = (size_t)strip * BOOK; uint32_t flag = 0, mask = 0;
#define READ() do { if (p + 4 > end) { snprintf(error, capacity, "vetores Cinepak truncados"); return 1; } flag = be32(f + p); p += 4; mask = 0x80000000u; } while (0)
 for (uint32_t y = y1; y < y2; y += 4) {
  for (uint32_t x = 0; x < c->width; x += 4) {
   int coded = 1;
   if (id & 1) { mask >>= 1; if (!mask) READ(); coded = (flag & mask) != 0; }
   if (!coded) continue;
   int single = (id & 2) != 0;
   if (!single) { mask >>= 1; if (!mask) READ(); single = (flag & mask) == 0; }
   if (single) {
    if (p >= end) { snprintf(error, capacity, "vetor V1 truncado"); return 1; }
    size_t e = cb + (size_t)f[p] * 16; p++;
    for (uint32_t k = 0; k < 4; k++) {
     uint32_t bx = x + (k & 1) * 2, by = y + (k >> 1) * 2;
     for (uint32_t d = 0; d < 4; d++) put(c, c->v1, e + 4 * k, bx + (d & 1), by + (d >> 1));
    }
   } else {
    if (p + 4 > end) { snprintf(error, capacity, "vetor V4 truncado"); return 1; }
    for (uint32_t k = 0; k < 4; k++) {
     size_t e = cb + (size_t)f[p + k] * 16; uint32_t bx = x + (k & 1) * 2, by = y + (k >> 1) * 2;
     for (uint32_t d = 0; d < 4; d++) put(c, c->v4, e + 4 * d, bx + (d & 1), by + (d >> 1));
    }
    p += 4;
   }
  }
 }
#undef READ
 return 0;
}

static int decode_strip(RlwCinepak *c, uint32_t strip, uint32_t y1, uint32_t y2, const uint8_t *f, size_t start, size_t end,
                        char *error, size_t capacity) {
 size_t p = start;
 while (p + 4 <= end) {
  uint32_t id = f[p]; int64_t raw = (int64_t)((uint32_t)f[p + 1] << 16 | (uint32_t)f[p + 2] << 8 | f[p + 3]) - 4;
  int64_t room = (int64_t)(end - p - 4); size_t size = (size_t)(raw < 0 ? 0 : raw > room ? room : raw);
  p += 4;
  switch (id) {
  case 0x20: case 0x21: case 0x24: case 0x25: codebook(c->v4, strip, id, f, p, p + size); break;
  case 0x22: case 0x23: case 0x26: case 0x27: codebook(c->v1, strip, id, f, p, p + size); break;
  case 0x30: case 0x31: case 0x32: return vectors(c, strip, id, y1, y2, f, p, p + size, error, capacity);
  default: break;
  }
  p += size;
 }
 return 0;
}

int rlw_cinepak_decode(RlwCinepak *c, const uint8_t *f, size_t length, char *error, size_t capacity) {
 char scratch[8]; if (!error) { error = scratch; capacity = sizeof(scratch); }
 if (length < 10) { snprintf(error, capacity, "quadro Cinepak curto"); return 1; }
 uint32_t flags = f[0], strips = (uint32_t)f[8] << 8 | f[9]; if (strips > MAX_STRIPS) strips = MAX_STRIPS;
 size_t p = 10; uint32_t y0 = 0;
 for (uint32_t strip = 0; strip < strips; strip++) {
  if (p + 12 > length) { snprintf(error, capacity, "faixa Cinepak truncada"); return 1; }
  int64_t size = (int64_t)((uint32_t)f[p + 1] << 16 | (uint32_t)f[p + 2] << 8 | f[p + 3]) - 12;
  if (size < 0) { snprintf(error, capacity, "faixa Cinepak inválida"); return 1; }
  uint32_t y1 = y0, y2 = y0 + ((uint32_t)f[p + 8] << 8 | f[p + 9]);
  p += 12;
  if ((uint64_t)size > length - p) size = (int64_t)(length - p);
  if (strip > 0 && !(flags & 1)) {
   memcpy(c->v4 + (size_t)strip * BOOK, c->v4 + (size_t)(strip - 1) * BOOK, BOOK);
   memcpy(c->v1 + (size_t)strip * BOOK, c->v1 + (size_t)(strip - 1) * BOOK, BOOK);
  }
  if (decode_strip(c, strip, y1, y2, f, p, p + (size_t)size, error, capacity)) return 1;
  p += (size_t)size; y0 = y2;
 }
 return 0;
}

/* ---------------- AVI ---------------- */
typedef struct { size_t start, length; } Range;
struct RlwMovie {
 uint8_t *data; size_t size;
 uint32_t width, height, period; char codec[5];
 int has_audio; RlwAudioFormat audio;
 Range *video; uint32_t video_count, video_capacity;
 Range *sound; uint32_t sound_count, sound_capacity;
 int16_t *pcm; uint64_t pcm_samples, pcm_at;   /* interleaved */
 RlwCinepak *decoder; uint32_t decoded; char error[200];
};
static uint32_t u16le(const uint8_t *p) { return p[0] | ((uint32_t)p[1] << 8); }
static uint32_t u32le(const uint8_t *p) { return u16le(p) | (u16le(p + 2) << 16); }
static int push(Range **list, uint32_t *count, uint32_t *capacity, size_t start, size_t length) {
 if (*count == *capacity) {
  uint32_t grown = *capacity ? *capacity * 2 : 1024; Range *r = realloc(*list, grown * sizeof(*r));
  if (!r) return 1;
  *list = r; *capacity = grown;
 }
 (*list)[(*count)++] = (Range){start, length}; return 0;
}
typedef struct { char types[32][5]; uint32_t type_count; int video_stream, audio_stream; } Walk;
static int walk(RlwMovie *m, Walk *w, size_t start, size_t end, int in_movi, char *error, size_t capacity) {
 const uint8_t *b = m->data; size_t p = start;
 while (p + 8 <= end) {
  const uint8_t *id = b + p; uint32_t size = u32le(b + p + 4); size_t body = p + 8;
  if (body + size > end) { snprintf(error, capacity, "bloco %.4s truncado", (const char *)id); return 1; }
  if (!memcmp(id, "LIST", 4) || !memcmp(id, "RIFF", 4)) {
   if (size < 4) { snprintf(error, capacity, "lista vazia"); return 1; }
   if (walk(m, w, body + 4, body + size, in_movi || !memcmp(b + body, "movi", 4), error, capacity)) return 1;
  } else if (in_movi) {
   int stream = id[0] >= '0' && id[0] <= '9' && id[1] >= '0' && id[1] <= '9' ? (id[0] - '0') * 10 + (id[1] - '0') : -1;
   if (stream >= 0 && stream == w->video_stream && (!memcmp(id + 2, "dc", 2) || !memcmp(id + 2, "db", 2))) { if (push(&m->video, &m->video_count, &m->video_capacity, body, size)) return 1; }
   else if (stream >= 0 && stream == w->audio_stream && !memcmp(id + 2, "wb", 2)) { if (push(&m->sound, &m->sound_count, &m->sound_capacity, body, size)) return 1; }
  } else if (!memcmp(id, "avih", 4) && size >= 40) {
   m->period = u32le(b + body); m->width = u32le(b + body + 32); m->height = u32le(b + body + 36);
  } else if (!memcmp(id, "strh", 4) && size >= 8) {
   if (w->type_count < 32) { memcpy(w->types[w->type_count], b + body, 4); w->types[w->type_count][4] = 0; }
   w->type_count++;
  } else if (!memcmp(id, "strf", 4) && w->type_count && w->type_count <= 32) {
   int index = (int)w->type_count - 1; const char *type = w->types[index];
   if (!strcmp(type, "vids") && w->video_stream < 0 && size >= 20) {
    w->video_stream = index; memcpy(m->codec, b + body + 16, 4); m->codec[4] = 0;
    m->width = u32le(b + body + 4); m->height = u32le(b + body + 8);
   } else if (!strcmp(type, "auds") && w->audio_stream < 0 && size >= 16) {
    w->audio_stream = index; RlwAudioFormat *a = &m->audio; memset(a, 0, sizeof(*a));
    a->tag = u16le(b + body); a->channels = u16le(b + body + 2); a->rate = u32le(b + body + 4);
    a->block_align = u16le(b + body + 12); a->bits = u16le(b + body + 14);
    if (a->tag == 2) {
     if (size < 22) { snprintf(error, capacity, "formato ADPCM incompleto"); return 1; }
     a->samples_per_block = u16le(b + body + 18); uint32_t count = u16le(b + body + 20);
     if (size < 22 + 4 * count || count < 1 || count > 32) { snprintf(error, capacity, "coeficientes ADPCM ausentes"); return 1; }
     for (uint32_t i = 0; i < count * 2; i++) a->coefficients[i] = (int16_t)(uint16_t)u16le(b + body + 22 + 2 * i);
     a->coefficient_count = count;
    }
    m->has_audio = 1;
   }
  }
  p = body + size + (size & 1);
 }
 return 0;
}

RlwMovie *rlw_movie_from_memory(uint8_t *data, size_t length, char *error, size_t capacity) {
 char scratch[8]; if (!error) { error = scratch; capacity = sizeof(scratch); }
 error[0] = 0;
 RlwMovie *m = calloc(1, sizeof(*m)); if (!m) { free(data); snprintf(error, capacity, "sem memória"); return NULL; }
 m->data = data; m->size = length;
 if (!data || length < 12 || memcmp(data, "RIFF", 4) || memcmp(data + 8, "AVI ", 4)) { snprintf(error, capacity, "não é um AVI"); goto bad; }
 Walk w; memset(&w, 0, sizeof(w)); w.video_stream = w.audio_stream = -1;
 size_t riff_end = 8 + (size_t)u32le(data + 4); if (riff_end > length) riff_end = length;
 if (walk(m, &w, 12, riff_end, 0, error, capacity)) goto bad;
 if (w.video_stream < 0 || !m->video_count) { snprintf(error, capacity, "sem trilha de vídeo"); goto bad; }
 if ((m->codec[0] | 0x20) != 'c' || (m->codec[1] | 0x20) != 'v' || (m->codec[2] | 0x20) != 'i' || (m->codec[3] | 0x20) != 'd') { snprintf(error, capacity, "codec de vídeo %s não suportado", m->codec); goto bad; }
 if (!m->width || !m->height || m->width % 4 || m->width > 4096 || m->height > 4096 || !m->period) { snprintf(error, capacity, "dimensões inválidas"); goto bad; }
 if (m->has_audio) {
  RlwAudioFormat *a = &m->audio;
  if ((a->channels != 1 && a->channels != 2) || !a->rate) { snprintf(error, capacity, "áudio inválido"); goto bad; }
  if (!((a->tag == 2 && a->bits == 4 && a->block_align >= 7 * a->channels) || (a->tag == 1 && a->bits == 16))) { snprintf(error, capacity, "formato de áudio %u não suportado", a->tag); goto bad; }
  size_t total = 0; for (uint32_t i = 0; i < m->sound_count; i++) total += m->sound[i].length;
  uint8_t *stream = malloc(total ? total : 1); if (!stream) { snprintf(error, capacity, "sem memória"); goto bad; }
  size_t at = 0; for (uint32_t i = 0; i < m->sound_count; i++) { memcpy(stream + at, data + m->sound[i].start, m->sound[i].length); at += m->sound[i].length; }
  if (a->tag == 1) {
   m->pcm_samples = total / 2; m->pcm = malloc(m->pcm_samples ? m->pcm_samples * 2 : 2);
   if (m->pcm) for (uint64_t i = 0; i < m->pcm_samples; i++) m->pcm[i] = (int16_t)(uint16_t)u16le(stream + 2 * i);
  } else {
   int64_t n = rlw_msadpcm_decode(stream, total, a, NULL, 0);
   if (n < 0) { free(stream); snprintf(error, capacity, "preditor ADPCM inválido"); goto bad; }
   m->pcm_samples = (uint64_t)n; m->pcm = malloc(n ? (size_t)n * 2 : 2);
   if (m->pcm) rlw_msadpcm_decode(stream, total, a, m->pcm, (uint64_t)n);
  }
  free(stream);
  if (!m->pcm) { snprintf(error, capacity, "sem memória"); goto bad; }
 }
 if (!(m->decoder = rlw_cinepak_create(m->width, m->height))) { snprintf(error, capacity, "sem memória"); goto bad; }
 return m;
bad:
 rlw_movie_free(m); return NULL;
}

RlwMovie *rlw_movie_open(const char *path, char *error, size_t capacity) {
 uint32_t n = 0; uint8_t *data = rlw_read_file(path, &n, 512u << 20);
 if (!data) { if (error && capacity) snprintf(error, capacity, "Filme original ausente"); return NULL; }
 return rlw_movie_from_memory(data, n, error, capacity);
}
void rlw_movie_free(RlwMovie *m) {
 if (!m) return;
 rlw_cinepak_free(m->decoder); free(m->video); free(m->sound); free(m->pcm); free(m->data); free(m);
}
uint32_t rlw_movie_width(const RlwMovie *m) { return m ? m->width : 0; }
uint32_t rlw_movie_height(const RlwMovie *m) { return m ? m->height : 0; }
uint32_t rlw_movie_frames(const RlwMovie *m) { return m ? m->video_count : 0; }
uint32_t rlw_movie_decoded(const RlwMovie *m) { return m ? m->decoded : 0; }
uint32_t rlw_movie_audio_rate(const RlwMovie *m) { return m && m->has_audio ? m->audio.rate : 0; }
uint64_t rlw_movie_audio_frames(const RlwMovie *m) { return m && m->has_audio ? m->pcm_samples / m->audio.channels : 0; }
double rlw_movie_duration_ms(const RlwMovie *m) {
 if (!m) return 0;
 double video = (double)m->video_count * m->period / 1000.0, audio = m->has_audio ? (double)rlw_movie_audio_frames(m) * 1000.0 / m->audio.rate : 0;
 return video > audio ? video : audio;
}
int rlw_movie_advance(RlwMovie *m, double elapsed_ms) {
 if (!m) return -1;
 double due = elapsed_ms > 0 ? elapsed_ms * 1000.0 / m->period : 0;
 uint64_t target = (uint64_t)due + 1; if (target > m->video_count) target = m->video_count;
 int advanced = 0;
 while (m->decoded < target) {
  const Range *r = &m->video[m->decoded];
  if (rlw_cinepak_decode(m->decoder, m->data + r->start, r->length, m->error, sizeof(m->error))) return -1;
  m->decoded++; advanced = 1;
 }
 return advanced;
}
const uint8_t *rlw_movie_pixels(const RlwMovie *m) { return m ? rlw_cinepak_pixels(m->decoder) : NULL; }
const char *rlw_movie_error(const RlwMovie *m) { return m ? m->error : ""; }
uint32_t rlw_movie_pull(void *movie, int16_t *stereo, uint32_t frames) {
 RlwMovie *m = movie; if (!m || !m->has_audio) return 0;
 uint32_t channels = m->audio.channels, n = 0;
 while (n < frames && m->pcm_at + channels <= m->pcm_samples) {
  int16_t l = m->pcm[m->pcm_at], r = channels == 2 ? m->pcm[m->pcm_at + 1] : l;
  stereo[2 * n] = l; stereo[2 * n + 1] = r; m->pcm_at += channels; n++;
 }
 return n;
}
const int16_t *rlw_movie_pcm(const RlwMovie *m, uint64_t *samples) { if (samples) *samples = m && m->has_audio ? m->pcm_samples : 0; return m ? m->pcm : NULL; }
double rlw_movie_frame_ms(const RlwMovie *m) { return m ? m->period / 1000.0 : 0; }
