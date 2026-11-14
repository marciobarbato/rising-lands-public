/* CD music: disc table, track WAVs, streaming player and policy (see rlw_cdmusic.h). */
#include "rlw_cdmusic.h"
#include "rlw_fs.h"
#include "rlw_sha256.h"
#include <ctype.h>
#include <stdlib.h>
#include <string.h>

const RlwCdMusicPin RLW_CDMUSIC_PINS[2] = {
 {2, 534569616ull, 99654240ull, "bee34ef8aef0a58933f90d63766eafc3f8daf447aae64ea35157d84c6850596a"},
 {3, 634223856ull, 71195040ull, "80b28a77ea82e454aa6babeb788746d2a141a932f4d101d7c4a8ecc51870ae3d"},
};

/* ---------------- Disc ---------------- */
static const char *skip(const char *s, const char *end) { while (s < end && (*s == ' ' || *s == '\t')) s++; return s; }
static int word(const char *s, const char *end, const char *w) {
 size_t n = strlen(w); if ((size_t)(end - s) < n) return 0;
 for (size_t i = 0; i < n; i++) if (toupper((unsigned char)s[i]) != w[i]) return 0;
 return s + n == end || s[n] == ' ' || s[n] == '\t';
}

int rlw_cdmusic_parse_cue(const char *text, size_t length, uint64_t image_size, RlwCdToc *out) {
 if (!text || !out) return 1;
 memset(out, 0, sizeof(*out));
 int files = 0; RlwCdTrack *current = NULL;
 const char *p = text, *stop = text + length;
 while (p < stop) {
  const char *e = p; while (e < stop && *e != '\n' && *e != '\r') e++;
  const char *s = skip(p, e);
  if (word(s, e, "FILE")) {
   if (++files > 1) return 1;
   const char *q = memchr(s, '"', (size_t)(e - s)), *r = q ? memchr(q + 1, '"', (size_t)(e - q - 1)) : NULL;
   if (!q || !r || r - q - 1 >= (ptrdiff_t)sizeof(out->file)) return 1;
   memcpy(out->file, q + 1, (size_t)(r - q - 1)); out->file[r - q - 1] = 0;
  } else if (word(s, e, "TRACK")) {
   if (!files || out->count == 99) return 1;
   char *after = NULL; unsigned long n = strtoul(s + 5, &after, 10);
   if (!n || n > 99 || after == s + 5) return 1;
   const char *t = skip(after, e);
   current = &out->tracks[out->count++]; current->number = (uint32_t)n; current->audio = word(t, e, "AUDIO");
   current->offset = UINT64_MAX;
  } else if (word(s, e, "INDEX") && current) {
   char *after = NULL; unsigned long index = strtoul(s + 5, &after, 10);
   unsigned mm = 0, ss = 0, ff = 0; char tail[2];
   char msf[16] = {0}; const char *m = skip(after, e); size_t n = (size_t)(e - m) < sizeof(msf) - 1 ? (size_t)(e - m) : sizeof(msf) - 1;
   memcpy(msf, m, n);
   if (sscanf(msf, "%u:%u:%u%1s", &mm, &ss, &ff, tail) != 3 || ss > 59 || ff > 74) return 1;
   if (index == 1) current->offset = ((uint64_t)mm * 60 * 75 + ss * 75 + ff) * 2352u;
  }
  p = e; while (p < stop && (*p == '\n' || *p == '\r')) p++;
 }
 if (!files || !out->count) return 1;
 for (uint32_t i = 0; i < out->count; i++) {
  uint64_t start = out->tracks[i].offset, next = i + 1 < out->count ? out->tracks[i + 1].offset : image_size;
  if (start == UINT64_MAX || next == UINT64_MAX || next < start || next > image_size) return 1;
  out->tracks[i].length = next - start;
 }
 return 0;
}

const RlwCdTrack *rlw_cdmusic_track(const RlwCdToc *toc, uint32_t number) {
 if (!toc) return NULL;
 for (uint32_t i = 0; i < toc->count; i++) if (toc->tracks[i].number == number) return &toc->tracks[i];
 return NULL;
}

static void put32(uint8_t *p, uint32_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24); }
void rlw_cdmusic_wav_header(uint8_t out[44], uint32_t pcm_bytes) {
 memcpy(out, "RIFF", 4); put32(out + 4, pcm_bytes + 36); memcpy(out + 8, "WAVEfmt ", 8); put32(out + 16, 16);
 out[20] = 1; out[21] = 0; out[22] = 2; out[23] = 0; put32(out + 24, 44100); put32(out + 28, 176400);
 out[32] = 4; out[33] = 0; out[34] = 16; out[35] = 0; memcpy(out + 36, "data", 4); put32(out + 40, pcm_bytes);
}

int rlw_cdmusic_write_wav(FILE *image, const RlwCdTrack *track, const char *path, char sha256_hex[65]) {
 if (!image || !track || !track->audio || track->length > 0xffffffffull - 44 || rlw_seek(image, track->offset)) return 1;
 FILE *out = rlw_fopen(path, "wb"); if (!out) return 1;
 uint8_t header[44]; rlw_cdmusic_wav_header(header, (uint32_t)track->length);
 RlwSha256 c; rlw_sha256_init(&c); rlw_sha256_update(&c, header, 44);
 int bad = fwrite(header, 1, 44, out) != 44;
 uint8_t *piece = malloc(1u << 20); if (!piece) bad = 1;
 for (uint64_t left = track->length; !bad && left;) {
  size_t n = left < (1u << 20) ? (size_t)left : (1u << 20);
  if (fread(piece, 1, n, image) != n || fwrite(piece, 1, n, out) != n) bad = 1;
  else { rlw_sha256_update(&c, piece, n); left -= n; }
 }
 free(piece);
 if (fclose(out)) bad = 1;
 if (bad) { rlw_remove_file(path); return 1; }
 if (sha256_hex) { uint8_t digest[32]; rlw_sha256_final(&c, digest); rlw_sha256_hex(digest, sha256_hex); }
 return 0;
}

#ifdef RLW_TESTING
int rlw_cdmusic_test_ignore_pins = 0;
#define MUSIC_PINS_ENFORCED (!rlw_cdmusic_test_ignore_pins)
#else
#define MUSIC_PINS_ENFORCED 1
#endif
static int ends_with(const char *s, const char *suffix) {
 size_t n = strlen(s), k = strlen(suffix); if (n < k) return 0;
 for (size_t i = 0; i < k; i++) if (tolower((unsigned char)s[n - k + i]) != suffix[i]) return 0;
 return 1;
}
int rlw_cdmusic_import(const char *source, const char *data_root, char *error, size_t capacity) {
 RlwCdToc toc; memset(&toc, 0, sizeof(toc));
 FILE *image = NULL; int rc = 1;
 char staging[4096] = {0}, final_dir[4096];
 if (error && capacity) error[0] = 0;
 if (!source || !data_root) return 1;
 if (ends_with(source, ".cue")) {
  uint32_t n = 0; uint8_t *text = rlw_read_file(source, &n, 1u << 16);
  if (!text) { if (error) snprintf(error, capacity, "Música: não foi possível ler %s.", source); return 1; }
  RlwCdToc names; int bad = rlw_cdmusic_parse_cue((const char *)text, n, UINT64_MAX - 1, &names);
  if (bad) { free(text); if (error) snprintf(error, capacity, "Música: CUE inválido."); return 1; }
  char bin[4400]; const char *slash = strrchr(source, '/'), *back = strrchr(source, '\\');
  if (back && (!slash || back > slash)) slash = back;
  if (slash) snprintf(bin, sizeof(bin), "%.*s%s", (int)(slash - source + 1), source, names.file); else snprintf(bin, sizeof(bin), "%s", names.file);
  image = rlw_fopen(bin, "rb");
  int64_t size = image ? rlw_file_size(image) : -1;
  if (size <= 0 || rlw_cdmusic_parse_cue((const char *)text, n, (uint64_t)size, &toc)) { free(text); if (image) fclose(image); if (error) snprintf(error, capacity, "Música: imagem %s ausente ou diferente do CUE.", bin); return 1; }
  free(text);
 } else {
  image = rlw_fopen(source, "rb");
  int64_t size = image ? rlw_file_size(image) : -1;
  if (size != (int64_t)RLW_CDMUSIC_IMAGE_BYTES) { if (image) fclose(image); return 2; }
  toc.count = 2;
  for (unsigned i = 0; i < 2; i++) toc.tracks[i] = (RlwCdTrack){RLW_CDMUSIC_PINS[i].track, 1, RLW_CDMUSIC_PINS[i].offset, RLW_CDMUSIC_PINS[i].pcm_bytes};
 }
 if (!rlw_cdmusic_track(&toc, 2) || !rlw_cdmusic_track(&toc, 3)) { fclose(image); return 2; }
 char dados[4096]; snprintf(dados, sizeof(dados), "%s" RLW_SEP "Dados", data_root);
 snprintf(staging, sizeof(staging), "%s" RLW_SEP "music.importando", dados);
 snprintf(final_dir, sizeof(final_dir), "%s" RLW_SEP "music", dados);
 rlw_remove_tree(staging);
 if (rlw_mkdirs(staging)) { if (error) snprintf(error, capacity, "Música: não foi possível criar %s.", staging); goto done; }
 for (unsigned i = 0; i < 2; i++) {
  const RlwCdMusicPin *pin = &RLW_CDMUSIC_PINS[i]; char path[4200], hex[65];
  snprintf(path, sizeof(path), "%s" RLW_SEP "TRACK%02u.wav", staging, pin->track);
  if (rlw_cdmusic_write_wav(image, rlw_cdmusic_track(&toc, pin->track), path, hex)) { if (error) snprintf(error, capacity, "Música: não foi possível gravar a faixa %u.", pin->track); goto done; }
  if (MUSIC_PINS_ENFORCED && strcmp(hex, pin->sha256)) { if (error) snprintf(error, capacity, "Música: a faixa %u do CD escolhido é diferente do CD brasileiro original (SHA-256).", pin->track); goto done; }
 }
 if (rlw_rename_replace_dir(staging, final_dir)) { if (error) snprintf(error, capacity, "Música: não foi possível instalar %s.", final_dir); goto done; }
 staging[0] = 0; rc = 0;
done:
 if (staging[0]) rlw_remove_tree(staging);
 fclose(image);
 return rc;
}
int rlw_cdmusic_installed(const char *dados) {
 for (unsigned i = 0; i < 2; i++) {
  char path[4200]; snprintf(path, sizeof(path), "%s" RLW_SEP "music" RLW_SEP "TRACK%02u.wav", dados, RLW_CDMUSIC_PINS[i].track);
  if (!rlw_exists(path)) return 0;
 }
 return 1;
}

/* ---------------- Player ---------------- */
struct RlwCdPlayer { char folder[4096]; FILE *file; uint32_t track; uint64_t left; volatile int ended; };
RlwCdPlayer *rlw_cdplayer_create(const char *folder) {
 RlwCdPlayer *p = calloc(1, sizeof(*p)); if (!p) return NULL;
 snprintf(p->folder, sizeof(p->folder), "%s", folder ? folder : ".");
 return p;
}
void rlw_cdplayer_close(RlwCdPlayer *p) { if (p) { if (p->file) fclose(p->file); p->file = NULL; p->track = 0; p->left = 0; p->ended = 0; } }
void rlw_cdplayer_free(RlwCdPlayer *p) { rlw_cdplayer_close(p); free(p); }
int rlw_cdplayer_open(RlwCdPlayer *p, uint32_t track) {
 if (!p) return 1;
 const RlwCdMusicPin *pin = NULL;
 for (unsigned i = 0; i < 2; i++) if (RLW_CDMUSIC_PINS[i].track == track) pin = &RLW_CDMUSIC_PINS[i];
 rlw_cdplayer_close(p);
 if (!pin) return 1;
 char path[4200]; snprintf(path, sizeof(path), "%s" RLW_SEP "TRACK%02u.wav", p->folder, track);
 FILE *f = rlw_fopen(path, "rb"); if (!f) return 1;
 /* The full SHA-256 was checked when the track was imported; here its size and header. */
 int64_t size = rlw_file_size(f); uint8_t header[44], expected[44];
 uint64_t pcm = MUSIC_PINS_ENFORCED ? pin->pcm_bytes : (size >= 44 ? (uint64_t)size - 44 : 0);
 rlw_cdmusic_wav_header(expected, (uint32_t)pcm);
 if (size != (int64_t)(pcm + 44) || fread(header, 1, 44, f) != 44 || memcmp(header, expected, 44)) { fclose(f); return 1; }
 p->file = f; p->track = track; p->left = pcm; p->ended = 0;
 return 0;
}
uint32_t rlw_cdplayer_pull(void *player, int16_t *stereo, uint32_t frames) {
 RlwCdPlayer *p = player;
 if (!p || !p->file || !p->left) { if (p && p->file) p->ended = 1; return 0; }
 uint64_t want = (uint64_t)frames * 4; if (want > p->left) want = p->left;
 uint8_t *bytes = (uint8_t *)stereo;
 size_t got = fread(bytes, 1, (size_t)want, p->file); got -= got % 4;
 p->left -= got;
 /* PCM16 little endian on disk; the host is little endian (x86-64). */
 if (!got) { p->left = 0; p->ended = 1; }
 return (uint32_t)(got / 4);
}
uint32_t rlw_cdplayer_track(const RlwCdPlayer *p) { return p ? p->track : 0; }
int rlw_cdplayer_ended(const RlwCdPlayer *p) { return p && p->file && p->ended; }

/* ---------------- Policy (NativeCDMusicHost) ---------------- */
static void say(RlwCdMusic *m, const char *text) { snprintf(m->notice, sizeof(m->notice), "%s", text); }
void rlw_cdmusic_init(RlwCdMusic *m) { memset(m, 0, sizeof(*m)); m->automatic = 1; say(m, "Música do CD · aguardando pedido do jogo."); }
static int stop(RlwCdMusic *m) { int was = m->playing; m->playing = 0; m->track = 0; if (was) say(m, "Música do CD parada."); return RLW_CDMUSIC_STOP; }
int rlw_cdmusic_begin_stage(RlwCdMusic *m) { int a = stop(m); m->rejected = 0; m->automatic = 1; m->stage_active = 1; return a; }
int rlw_cdmusic_result(RlwCdMusic *m) { m->stage_active = 0; return RLW_CDMUSIC_NONE; }
int rlw_cdmusic_leave_stage(RlwCdMusic *m) { m->stage_active = 0; return stop(m); }
int rlw_cdmusic_request(RlwCdMusic *m, uint32_t site, uint32_t track, uint32_t loop) {
 if (!m->stage_active || !m->automatic) return RLW_CDMUSIC_NONE;
 m->requests++;
 if ((site != 0x42d32du && site != 0x4684bdu) || loop != 0) {
  m->automatic = 0; m->failures++; say(m, "Pedidos de música indisponíveis (-1)."); return RLW_CDMUSIC_NONE;
 }
 if (site == 0x42d32du && m->playing) { m->suppressed++; return RLW_CDMUSIC_NONE; }
 if (track < 32 && (m->rejected >> track & 1)) return RLW_CDMUSIC_NONE;
 if (track != 2 && track != 3) {
  if (track < 32) m->rejected |= 1u << track;
  m->failures++; say(m, "Faixa de CD não suportada. Use a faixa original 2 ou 3."); return RLW_CDMUSIC_NONE;
 }
 m->track = track; return RLW_CDMUSIC_PLAY;
}
void rlw_cdmusic_started(RlwCdMusic *m, uint32_t track) {
 m->playing = 1; m->track = track; m->starts++;
 snprintf(m->notice, sizeof(m->notice), "Música do CD · faixa %u", track);
}
void rlw_cdmusic_failed(RlwCdMusic *m, uint32_t track) {
 m->playing = 0; m->track = 0; if (track < 32) m->rejected |= 1u << track; m->failures++;
 snprintf(m->notice, sizeof(m->notice), "Arquivo de música original da faixa %u ausente ou inválido. Importe o CD (imagem .BIN/.CUE) de novo.", track);
}
void rlw_cdmusic_finished(RlwCdMusic *m) { m->playing = 0; m->track = 0; m->finishes++; say(m, "Música do CD encerrada."); }
