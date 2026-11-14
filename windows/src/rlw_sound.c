/* Original sound banks: HED/SND reading and sample decoding (see rlw_sound.h). */
#include "rlw_sound.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

const uint32_t RLW_SOUND_SELECTORS[3] = {0x41u, 0x42u, 0x50u};
const RlwSoundFile RLW_SOUND_FILES[6] = {
 {"A.HED", 372u, "e98c8cbe1c7734b638c008c905b68bdb2023f185327c3b7ec86b88d5e7707072"},
 {"A.SND", 525824u, "ec4874144eca50ddab6bc8fbb5f9db86d06d77960bd6a3e7ec313b2c9288f60b"},
 {"B.HED", 2872u, "48bf0fbc8c7f93963c3ff2d9ad79963fade15ae165a685f6f30e2e440107bd4a"},
 {"B.SND", 1675264u, "9b617f4bbb47c5de05ea9a29af1dae03f53ab77b57ed8c8b15582c76126079f3"},
 {"P.HED", 10312u, "05170234f723c1e9e10c0edf494a432f7dc4fbd4a5c2b917f953b50cace4c66e"},
 {"P.SND", 10337891u, "c1193ed24382ba755e0e026c65e51dfc27264e0b480722e7c5779b918a94a977"},
};

#define MAX_HEADERS 4096u
typedef struct { uint32_t key; RlwSample sample; int state; } Slot; /* state 0 not tried, 1 ok, 2 failed */
struct RlwSoundBanks {
 uint8_t *hed[3], *snd[3]; uint32_t hed_length[3], snd_length[3];
 Slot slots[MAX_HEADERS]; uint32_t slot_count;
};

static uint32_t le32(const uint8_t *p) { return p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24); }
static uint32_t le16(const uint8_t *p) { return p[0] | ((uint32_t)p[1] << 8); }

/* The checks of macos_audio_load_bank: magic, body = length - 16, count fits. */
static int bank_valid(const uint8_t *hed, uint32_t length) {
 if (!hed || length < 16 || memcmp(hed, "ABCD", 4)) return 0;
 uint32_t body = le32(hed + 4), count = le32(hed + 12);
 return body == length - 16 && body <= 0x10000u && count <= 0xffffu && (uint64_t)count * 12 <= body;
}

static RlwSoundBanks *adopt(uint8_t *hed[3], const uint32_t hed_length[3], uint8_t *snd[3], const uint32_t snd_length[3]) {
 RlwSoundBanks *b = calloc(1, sizeof(*b));
 int ok = b != NULL;
 for (unsigned i = 0; i < 3; i++) ok = ok && bank_valid(hed[i], hed_length[i]) && snd[i];
 if (!ok) { for (unsigned i = 0; i < 3; i++) { free(hed[i]); free(snd[i]); } free(b); return NULL; }
 for (unsigned i = 0; i < 3; i++) { b->hed[i] = hed[i]; b->snd[i] = snd[i]; b->hed_length[i] = hed_length[i]; b->snd_length[i] = snd_length[i]; }
 return b;
}

RlwSoundBanks *rlw_sound_banks_open(const char *folder, char *error, size_t capacity) {
 uint8_t *hed[3] = {0}, *snd[3] = {0}; uint32_t hl[3] = {0}, sl[3] = {0};
 static const char letters[3] = {'A', 'B', 'P'};
 for (unsigned i = 0; i < 3; i++) {
  char path[4096];
  snprintf(path, sizeof(path), "%s" RLW_SEP "%c.HED", folder, letters[i]); hed[i] = rlw_read_file(path, &hl[i], 1u << 20);
  snprintf(path, sizeof(path), "%s" RLW_SEP "%c.SND", folder, letters[i]); snd[i] = rlw_read_file(path, &sl[i], 64u << 20);
  if (!hed[i] || !snd[i]) {
   if (error && capacity) snprintf(error, capacity, "Bancos de som ausentes (%c.HED/%c.SND em %s)", letters[i], letters[i], folder);
   for (unsigned k = 0; k < 3; k++) { free(hed[k]); free(snd[k]); }
   return NULL;
  }
 }
 RlwSoundBanks *b = adopt(hed, hl, snd, sl);
 if (!b && error && capacity) snprintf(error, capacity, "Bancos de som inválidos em %s", folder);
 return b;
}

RlwSoundBanks *rlw_sound_banks_from_memory(const uint8_t *const hed[3], const uint32_t hed_length[3],
                                           const uint8_t *const snd[3], const uint32_t snd_length[3]) {
 uint8_t *h[3] = {0}, *s[3] = {0};
 for (unsigned i = 0; i < 3; i++) {
  h[i] = malloc(hed_length[i] ? hed_length[i] : 1); s[i] = malloc(snd_length[i] ? snd_length[i] : 1);
  if (h[i] && hed[i]) memcpy(h[i], hed[i], hed_length[i]);
  if (s[i] && snd[i]) memcpy(s[i], snd[i], snd_length[i]);
 }
 return adopt(h, hed_length, s, snd_length);
}

void rlw_sound_banks_free(RlwSoundBanks *b) {
 if (!b) return;
 for (unsigned i = 0; i < 3; i++) { free(b->hed[i]); free(b->snd[i]); }
 for (uint32_t i = 0; i < b->slot_count; i++) free(b->slots[i].sample.pcm);
 free(b);
}

const uint8_t *rlw_sound_banks_hed(const RlwSoundBanks *b, unsigned bank, uint32_t *length) {
 if (!b || bank > 2) return NULL;
 if (length) *length = b->hed_length[bank];
 return b->hed[bank];
}

/* macos_audio_sample: the key must be one of an entry's variant headers. */
static int key_known(const RlwSoundBanks *b, uint32_t key) {
 uint32_t bank = key >> 24, offset = key & 0xffffffu;
 if (bank > 2) return 0;
 const uint8_t *body = b->hed[bank] + 16; uint32_t count = le32(b->hed[bank] + 12);
 for (uint32_t i = 0; i < count; i++) {
  uint32_t first = le32(body + 12 * i + 8), n = le16(body + 12 * i + 6);
  if (offset >= first && offset < first + 16 * n && (offset - first) % 16 == 0) return 1;
 }
 return 0;
}

void rlw_sound_each(const RlwSoundBanks *b, void (*fn)(void *, unsigned, uint32_t, uint32_t, uint32_t), void *ctx) {
 if (!b || !fn) return;
 for (unsigned bank = 0; bank < 3; bank++) {
  const uint8_t *body = b->hed[bank] + 16; uint32_t count = le32(b->hed[bank] + 12), length = b->hed_length[bank] - 16;
  for (uint32_t i = 0; i < count; i++) {
   uint32_t first = le32(body + 12 * i + 8), n = le16(body + 12 * i + 6);
   for (uint32_t v = 0; v < n; v++) if ((uint64_t)first + 16 * v + 16 <= length) fn(ctx, bank, i, v, (bank << 24) | (first + 16 * v));
  }
 }
}

static const int IMA_INDEX[16] = {-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8};
static const int IMA_STEPS[89] = {7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73,
 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132,
 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767};

int64_t rlw_sound_ima_mono16(const uint8_t *source, uint32_t length, uint32_t block, int16_t *out, uint64_t capacity) {
 uint64_t written = 0; uint32_t remaining = length, position = 0;
 if (remaining >= 4 && block < 4) return -1;
#define EMIT(v) do { if (written < capacity && out) out[written] = (int16_t)(v); written++; } while (0)
 while (remaining >= 4) {
  uint32_t count = remaining < block ? remaining : block;
  remaining -= count;
  int predictor = (int16_t)(uint16_t)(source[position] | (source[position + 1] << 8));
  int index = source[position + 2];
  position += 4;
  if (index > 88) return (int64_t)written;
  EMIT(predictor);
  for (uint32_t k = 0; k < count - 4; k++) {
   uint8_t byte = source[position + k];
   for (int half = 0; half < 2; half++) {
    int nibble = half ? byte >> 4 : byte & 15, step = IMA_STEPS[index], difference = step >> 3;
    if (nibble & 4) difference += step;
    if (nibble & 2) difference += step >> 1;
    if (nibble & 1) difference += step >> 2;
    predictor += nibble & 8 ? -difference : difference;
    if (predictor > 32767) predictor = 32767;
    if (predictor < -32768) predictor = -32768;
    index += IMA_INDEX[nibble]; if (index < 0) index = 0; if (index > 88) index = 88;
    EMIT(predictor);
   }
  }
  position += count - 4;
 }
#undef EMIT
 return (int64_t)written;
}

static int decode(const RlwSoundBanks *b, uint32_t key, RlwSample *out) {
 uint32_t bank = key >> 24, offset = key & 0xffffffu;
 if (!key_known(b, key) || (uint64_t)offset + 16 > b->hed_length[bank] - 16) return 1;
 const uint8_t *h = b->hed[bank] + 16 + offset;
 uint32_t at = le32(h), size = le32(h + 4), channels = h[8], bits = h[9], rate = le16(h + 10), format = h[12], block = le16(h + 14);
 if ((uint64_t)at + size > b->snd_length[bank] || !rate || !channels) return 1;
 const uint8_t *data = b->snd[bank] + at;
 if (format == 4) {
  if (channels != 1 || bits != 16) return 1;
  int64_t n = rlw_sound_ima_mono16(data, size, block, NULL, 0);
  if (n <= 0) return 1;
  out->pcm = malloc((size_t)n * sizeof(int16_t)); if (!out->pcm) return 1;
  rlw_sound_ima_mono16(data, size, block, out->pcm, (uint64_t)n);
  out->frames = (uint32_t)n; out->rate = rate; return 0;
 }
 if (format == 0 && (bits == 8 || bits == 16) && channels <= 2) {
  uint32_t align = channels * bits / 8, n = size / align;
  if (!n) return 1;
  out->pcm = malloc((size_t)n * sizeof(int16_t)); if (!out->pcm) return 1;
  for (uint32_t i = 0; i < n; i++) {
   int sum = 0;
   for (uint32_t c = 0; c < channels; c++) {
    const uint8_t *p = data + (size_t)i * align + c * (bits / 8);
    sum += bits == 16 ? (int16_t)(uint16_t)(p[0] | (p[1] << 8)) : ((int)p[0] - 128) * 256;
   }
   out->pcm[i] = (int16_t)(sum / (int)channels);
  }
  out->frames = n; out->rate = rate; return 0;
 }
 return 1;
}

const RlwSample *rlw_sound_sample(RlwSoundBanks *b, uint32_t key) {
 if (!b) return NULL;
 for (uint32_t i = 0; i < b->slot_count; i++) if (b->slots[i].key == key) return b->slots[i].state == 1 ? &b->slots[i].sample : NULL;
 if (b->slot_count == MAX_HEADERS) return NULL;
 Slot *s = &b->slots[b->slot_count++]; memset(s, 0, sizeof(*s)); s->key = key;
 s->state = decode(b, key, &s->sample) ? 2 : 1;
 return s->state == 1 ? &s->sample : NULL;
}
