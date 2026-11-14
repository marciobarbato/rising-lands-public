/* Portable unit tests for the Windows host's sound (front 2): IMA ADPCM
 * decoding, synthetic sound banks (memory and folder), the mixer (volume,
 * loops, ends, release, music stream, a rendered WAV's statistics) and the
 * play-mode host over MacOSAudioMapper (PostSound -> Manage -> voice;
 * StopAllSound -> silence). No game data: every sample is synthetic.
 *   test_audio <work folder> */
#include "rlw_audio.h"
#include "rlw_sound.h"
#include "rlw_fs.h"
#include "rlw_sound_import.h"
#include "rlw_cdmusic.h"
#include "rlw_eventflc.h"
#include "rlw_movie.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "%s:%d: CHECK(%s)\n", __FILE__, __LINE__, #c); failures++; } } while (0)

static void put32(uint8_t *p, uint32_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); p[2] = (uint8_t)(v >> 16); p[3] = (uint8_t)(v >> 24); }
static void put16(uint8_t *p, uint32_t v) { p[0] = (uint8_t)v; p[1] = (uint8_t)(v >> 8); }

/* Reference IMA ADPCM encoder (the decoder's inverse), for synthetic data. */
static const int STEPS[89] = {7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73,
 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963,
 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132,
 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767};
static const int INDEX[16] = {-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8};
static int encode_nibble(int sample, int *predictor, int *index) {
 int step = STEPS[*index], diff = sample - *predictor, nibble = 0;
 if (diff < 0) { nibble = 8; diff = -diff; }
 int delta = step >> 3;
 if (diff >= step) { nibble |= 4; diff -= step; delta += step; }
 if (diff >= step >> 1) { nibble |= 2; diff -= step >> 1; delta += step >> 1; }
 if (diff >= step >> 2) { nibble |= 1; delta += step >> 2; }
 *predictor += nibble & 8 ? -delta : delta;
 if (*predictor > 32767) *predictor = 32767;
 if (*predictor < -32768) *predictor = -32768;
 *index += INDEX[nibble]; if (*index < 0) *index = 0; if (*index > 88) *index = 88;
 return nibble;
}
/* Blocks of `block` bytes: predictor, index, pad, then (block-4)*2 samples. */
static uint32_t ima_encode(const int16_t *pcm, uint32_t frames, uint32_t block, uint8_t *out) {
 uint32_t at = 0, n = 0, per = (block - 4) * 2 + 1;
 while (at < frames) {
  int predictor = pcm[at], index = 0;
  put16(out + n, (uint16_t)(int16_t)predictor); out[n + 2] = 0; out[n + 3] = 0; n += 4; at++;
  for (uint32_t k = 0; k < block - 4; k++) {
   int lo = encode_nibble(at < frames ? pcm[at] : 0, &predictor, &index); at++;
   int hi = encode_nibble(at < frames ? pcm[at] : 0, &predictor, &index); at++;
   out[n++] = (uint8_t)(lo | (hi << 4));
  }
  (void)per;
 }
 return n;
}

static void test_ima(void) {
 int16_t pcm[2000]; for (int i = 0; i < 2000; i++) pcm[i] = (int16_t)(8000 * sin(i * 0.05));
 uint8_t enc[4096]; uint32_t n = ima_encode(pcm, 2000, 512, enc);
 int64_t count = rlw_sound_ima_mono16(enc, n, 512, NULL, 0);
 CHECK(count == (int64_t)(n / 512) * 1017);
 int16_t *dec = malloc((size_t)count * sizeof(int16_t));
 CHECK(rlw_sound_ima_mono16(enc, n, 512, dec, (uint64_t)count) == count);
 double err = 0; for (int i = 0; i < 2000; i++) err += fabs((double)dec[i] - pcm[i]);
 CHECK(err / 2000 < 200);              /* close to the source */
 CHECK(dec[0] == pcm[0] && dec[1017] == pcm[1017]);  /* block headers are emitted samples */
 free(dec);
 uint8_t bad[8] = {0x10, 0, 89, 0, 0x77, 0x77, 0x77, 0x77};
 CHECK(rlw_sound_ima_mono16(bad, 8, 8, NULL, 0) == 0);  /* step index above 88 stops */
 CHECK(rlw_sound_ima_mono16(bad, 8, 3, NULL, 0) == -1); /* block < 4 */
 uint8_t tail[6] = {0x34, 0x12, 0, 0, 0x01, 0x10};
 int16_t t[8]; CHECK(rlw_sound_ima_mono16(tail, 6, 512, t, 8) == 5 && t[0] == 0x1234);
}

/* Synthetic banks: A holds sound 0x20800 (IMA, 1 variant) and 0x30100 (PCM, 2 variants); B and P one PCM sound each. */
typedef struct { uint8_t hed[3][256]; uint32_t hed_length[3]; uint8_t snd[3][16384]; uint32_t snd_length[3]; uint32_t key_ima, key_pcm0, key_pcm1; } Banks;
static void make_banks(Banks *b) {
 memset(b, 0, sizeof(*b));
 int16_t tone[1500]; for (int i = 0; i < 1500; i++) tone[i] = (int16_t)(6000 * sin(i * 0.1));
 for (unsigned k = 0; k < 3; k++) {
  uint8_t *h = b->hed[k], *body = h + 16; uint32_t count = k == 0 ? 2 : 1, headers = 12 * count;
  memcpy(h, "ABCD", 4);
  uint32_t snd = 0, nh = 0;
  if (k == 0) {
   put32(body, 0x20800); put16(body + 4, 0); put16(body + 6, 1); put32(body + 8, headers);
   put32(body + 12, 0x30100); put16(body + 16, 0); put16(body + 18, 2); put32(body + 20, headers + 16);
   uint32_t n = ima_encode(tone, 1500, 512, b->snd[0]);
   uint8_t *d = body + headers; put32(d, 0); put32(d + 4, n); d[8] = 1; d[9] = 16; put16(d + 10, 22050); d[12] = 4; put16(d + 14, 512);
   snd = n; nh = 3;
   for (unsigned v = 0; v < 2; v++) {
    d = body + headers + 16 * (1 + v); uint32_t len = 600;
    for (uint32_t i = 0; i < len / 2; i++) put16(b->snd[0] + snd + 2 * i, (uint16_t)(int16_t)(v ? -4000 : 4000));
    put32(d, snd); put32(d + 4, len); d[8] = 1; d[9] = 16; put16(d + 10, 22050); d[12] = 0; put16(d + 14, 0); snd += len;
   }
  } else {
   put32(body, 0x10000 + k); put16(body + 4, 0); put16(body + 6, 1); put32(body + 8, headers);
   uint8_t *d = body + headers; for (uint32_t i = 0; i < 100; i++) b->snd[k][i] = (uint8_t)(128 + (i & 1 ? 40 : -40));
   put32(d, 0); put32(d + 4, 100); d[8] = 1; d[9] = 8; put16(d + 10, 11025); d[12] = 0; snd = 100; nh = 1;
  }
  uint32_t bodylen = headers + 16 * nh;
  put32(h + 4, bodylen); put32(h + 8, snd); put32(h + 12, count);
  b->hed_length[k] = 16 + bodylen; b->snd_length[k] = snd;
 }
 b->key_ima = 24; b->key_pcm0 = 40; b->key_pcm1 = 56;
}
static RlwSoundBanks *open_banks(const Banks *b) {
 const uint8_t *h[3] = {b->hed[0], b->hed[1], b->hed[2]}, *s[3] = {b->snd[0], b->snd[1], b->snd[2]};
 return rlw_sound_banks_from_memory(h, b->hed_length, s, b->snd_length);
}
static int keys_seen;
static void count_key(void *ctx, unsigned bank, uint32_t entry, uint32_t variant, uint32_t key) { (void)ctx; (void)bank; (void)entry; (void)variant; (void)key; keys_seen++; }

static void test_banks(const char *work) {
 Banks b; make_banks(&b);
 RlwSoundBanks *banks = open_banks(&b); CHECK(banks != NULL); if (!banks) return;
 const RlwSample *s = rlw_sound_sample(banks, b.key_ima);
 CHECK(s && s->rate == 22050 && s->frames == 2 * 1017);   /* 1500 samples -> 2 blocks of 1,017 */
 const RlwSample *p0 = rlw_sound_sample(banks, b.key_pcm0), *p1 = rlw_sound_sample(banks, b.key_pcm1);
 CHECK(p0 && p0->frames == 300 && p0->pcm[0] == 4000 && p1 && p1->pcm[10] == -4000);
 const RlwSample *eight = rlw_sound_sample(banks, (1u << 24) | 12);
 CHECK(eight && eight->rate == 11025 && eight->frames == 100 && eight->pcm[0] == -40 * 256);
 CHECK(rlw_sound_sample(banks, b.key_ima + 4) == NULL);     /* not a header offset */
 CHECK(rlw_sound_sample(banks, (3u << 24) | 12) == NULL);   /* no bank 3 */
 CHECK(rlw_sound_sample(banks, b.key_ima) == s);            /* cached */
 keys_seen = 0; rlw_sound_each(banks, count_key, NULL); CHECK(keys_seen == 5);
 rlw_sound_banks_free(banks);
 /* A corrupt HED is refused. */
 Banks bad = b; bad.hed[1][0] = 'X'; CHECK(open_banks(&bad) == NULL);
 /* From a folder, as <Dados>/sound. */
 char folder[4096]; snprintf(folder, sizeof(folder), "%s" RLW_SEP "sound", work); rlw_remove_tree(folder); rlw_mkdirs(folder);
 static const char letters[3] = {'A', 'B', 'P'};
 for (unsigned k = 0; k < 3; k++) {
  char path[4200];
  snprintf(path, sizeof(path), "%s" RLW_SEP "%c.HED", folder, letters[k]); rlw_write_file(path, b.hed[k], b.hed_length[k]);
  snprintf(path, sizeof(path), "%s" RLW_SEP "%c.SND", folder, letters[k]); if (k < 2) rlw_write_file(path, b.snd[k], b.snd_length[k]);
 }
 char error[256] = {0};
 CHECK(rlw_sound_banks_open(folder, error, sizeof(error)) == NULL && strstr(error, "P.HED/P.SND"));
 char path[4200]; snprintf(path, sizeof(path), "%s" RLW_SEP "P.SND", folder); rlw_write_file(path, b.snd[2], b.snd_length[2]);
 banks = rlw_sound_banks_open(folder, error, sizeof(error)); CHECK(banks != NULL);
 uint32_t n = 0; CHECK(banks && rlw_sound_banks_hed(banks, 2, &n) && n == b.hed_length[2]);
 rlw_sound_banks_free(banks);
}

typedef struct { double rms; int peak; uint64_t nonzero; } Stats;
static Stats stats(const int16_t *s, uint64_t n) {
 Stats r = {0, 0, 0}; double sum = 0;
 for (uint64_t i = 0; i < n; i++) { int v = abs(s[i]); if (v > r.peak) r.peak = v; if (v) r.nonzero++; sum += (double)s[i] * s[i]; }
 r.rms = n ? sqrt(sum / (double)n) : 0; return r;
}
static int write_wav(const char *path, const int16_t *s, uint32_t frames, uint32_t rate) {
 uint32_t bytes = frames * 4; uint8_t *w = malloc(44 + (size_t)bytes); if (!w) return 1;
 memcpy(w, "RIFF", 4); put32(w + 4, 36 + bytes); memcpy(w + 8, "WAVEfmt ", 8); put32(w + 16, 16); put16(w + 20, 1); put16(w + 22, 2);
 put32(w + 24, rate); put32(w + 28, rate * 4); put16(w + 32, 4); put16(w + 34, 16); memcpy(w + 36, "data", 4); put32(w + 40, bytes);
 memcpy(w + 44, s, bytes); int rc = rlw_write_file(path, w, 44 + bytes); free(w); return rc;
}

static const int16_t *music_data; static uint32_t music_frames, music_at;
static uint32_t music_pull(void *ctx, int16_t *out, uint32_t frames) {
 (void)ctx; uint32_t n = music_frames - music_at < frames ? music_frames - music_at : frames;
 memcpy(out, music_data + 2 * music_at, (size_t)n * 4); music_at += n; return n;
}

static void test_mixer(const char *work) {
 CHECK(rlw_audio_gain(0) == 1.0f && rlw_audio_gain(-10000) == 0.0f && fabsf(rlw_audio_gain(-2000) - 0.1f) < 1e-4f);
 int16_t ones[220]; for (int i = 0; i < 220; i++) ones[i] = 1000;
 RlwSample s = {ones, 220, 22050};
 RlwMixer *m = rlw_mixer_create(44100); CHECK(m != NULL); if (!m) return;
 int16_t out[2 * 1000];
 CHECK(rlw_mixer_buffer(m, 7, &s) == 0);
 rlw_mixer_play(m, 7, 0, 0);
 rlw_mixer_render(m, out, 1000);
 CHECK(out[0] == 1000 && out[1] == 1000 && out[2 * 439] == 1000 && out[2 * 440] == 0);  /* 220 frames at 22,050 -> 440 at 44,100 */
 uint32_t ids[8]; CHECK(rlw_mixer_finished(m, ids, 8) == 1 && ids[0] == 7);
 CHECK(rlw_mixer_finished(m, ids, 8) == 0);
 rlw_mixer_play(m, 7, -2000, 1); rlw_mixer_render(m, out, 1000);
 CHECK(out[2 * 999] == 100 && rlw_mixer_playing(m) == 1 && rlw_mixer_finished(m, ids, 8) == 0);  /* loops at -20 dB */
 rlw_mixer_volume(m, 7, -10000); rlw_mixer_render(m, out, 10); CHECK(out[0] == 0);
 rlw_mixer_stop(m, 7); CHECK(rlw_mixer_playing(m) == 0);
 CHECK(rlw_mixer_duplicate(m, 8, 7) == 0); rlw_mixer_play(m, 8, 0, 0); rlw_mixer_set_effects_gain(m, 0.5f);
 rlw_mixer_render(m, out, 4); CHECK(out[0] == 500);
 rlw_mixer_set_effects_gain(m, 1.0f);
 CHECK(rlw_mixer_buffer(m, 9, NULL) == 0); rlw_mixer_play(m, 9, 0, 0);          /* missing sample ends at once */
 CHECK(rlw_mixer_finished(m, ids, 8) == 1 && ids[0] == 9);
 rlw_mixer_release(m, 8); rlw_mixer_render(m, out, 4); CHECK(out[0] == 0);
 /* Finishing then replaying before the answer is read drops the stale answer. */
 rlw_mixer_play(m, 7, 0, 0); rlw_mixer_render(m, out, 1000); rlw_mixer_play(m, 7, 0, 0);
 CHECK(rlw_mixer_finished(m, ids, 8) == 0);
 rlw_mixer_silence(m); CHECK(rlw_mixer_playing(m) == 0);
 /* Another output rate (the device took 22,050): the same 220 frames now last 220 output frames. */
 rlw_mixer_set_rate(m, 22050); CHECK(rlw_mixer_rate(m) == 22050);
 rlw_mixer_buffer(m, 5, &s); rlw_mixer_play(m, 5, 0, 0); rlw_mixer_render(m, out, 300);
 CHECK(out[2 * 219] == 1000 && out[2 * 220] == 0 && rlw_mixer_finished(m, ids, 8) == 1);
 rlw_mixer_buffer(m, 5, &s); rlw_mixer_play(m, 5, 0, 1); rlw_mixer_set_rate(m, 44100); rlw_mixer_render(m, out, 10);
 CHECK(out[0] == 1000); rlw_mixer_silence(m);
 /* Clipping is clamped and counted. */
 int16_t loud[100]; for (int i = 0; i < 100; i++) loud[i] = 30000;
 RlwSample l = {loud, 100, 44100};
 rlw_mixer_buffer(m, 1, &l); rlw_mixer_buffer(m, 2, &l); rlw_mixer_play(m, 1, 0, 0); rlw_mixer_play(m, 2, 0, 0);
 rlw_mixer_render(m, out, 10); CHECK(out[0] == 32767 && rlw_mixer_clipped(m) == 20);
 rlw_mixer_silence(m);
 /* Music: a stereo stream at 44,100 Hz, resampled 1:1, gain applied, ends with its data. */
 static int16_t music[2 * 300]; for (int i = 0; i < 300; i++) { music[2 * i] = 2000; music[2 * i + 1] = -2000; }
 music_data = music; music_frames = 300; music_at = 0;
 rlw_mixer_set_music(m, music_pull, NULL, 44100); rlw_mixer_set_music_gain(m, 0.5f);
 rlw_mixer_render(m, out, 1000);
 CHECK(out[0] == 1000 && out[1] == -1000 && out[2 * 298] == 1000 && out[2 * 400] == 0);
 rlw_mixer_set_music_gain(m, 1.0f);
 /* A rendered scene: a 440 Hz tone (looping, -6 dB), a decaying effect and 22,050 Hz music, 2 s.
  * The WAV is written for listening; its statistics are the test. */
 int16_t tone[100]; for (int i = 0; i < 100; i++) tone[i] = (int16_t)(12000 * sin(2 * 3.14159265358979 * i / 100.0));
 int16_t hit[11025]; for (int i = 0; i < 11025; i++) hit[i] = (int16_t)(9000 * exp(-i / 2000.0) * sin(i * 0.3));
 RlwSample st = {tone, 100, 44000}, sh = {hit, 11025, 22050};
 static int16_t slow[2 * 22050]; for (int i = 0; i < 22050; i++) { slow[2 * i] = (int16_t)(3000 * sin(i * 0.02)); slow[2 * i + 1] = slow[2 * i]; }
 music_data = slow; music_frames = 22050; music_at = 0; rlw_mixer_set_music(m, music_pull, NULL, 22050);
 rlw_mixer_buffer(m, 3, &st); rlw_mixer_buffer(m, 4, &sh); rlw_mixer_play(m, 3, -600, 1);
 uint32_t total = 2 * 44100; int16_t *scene = malloc((size_t)total * 4);
 for (uint32_t at = 0; at < total; at += 441) { if (at == 22050) rlw_mixer_play(m, 4, 0, 0); rlw_mixer_render(m, scene + 2 * at, 441); }
 Stats st2 = stats(scene, 2 * (uint64_t)total);
 CHECK(st2.peak < 32767 && st2.peak > 5000 && st2.rms > 2000 && st2.nonzero > total);
 char path[4096]; snprintf(path, sizeof(path), "%s" RLW_SEP "mixer-scene.wav", work);
 CHECK(write_wav(path, scene, total, 44100) == 0);
 uint32_t length = 0; uint8_t *wav = rlw_read_file(path, &length, 1u << 24); CHECK(wav && length == 44 + total * 4); free(wav);
 printf("mixer scene: %u frames (2.00 s), peak %d, rms %.0f, clipped %llu\n", total, st2.peak, st2.rms, (unsigned long long)rlw_mixer_clipped(m) - 20);
 CHECK(rlw_mixer_clipped(m) == 20);
 free(scene); rlw_mixer_free(m);
}

static void test_host(void) {
 Banks b; make_banks(&b);
 RlwAudio *a = rlw_audio_create(open_banks(&b), 44100, NULL);
 CHECK(a && rlw_audio_enabled(a) && !strcmp(rlw_audio_availability(a), "pacote")); if (!a) return;
 /* PostSound(0x41, 0x20800, ref 0x49a1c8, flags 0) at volume 0x100, then the WinMain Manage tick. */
 RlwAudioRequest post = {1, 10, 3, 0x41b785, 0x41, 0x20800, 0x49a1c8, 0, 0x100, 1}, manage = {0, 11, 2, 0x460220, 0, 0, 0, 0, 0, 0};
 RlwAudioRequest seq[2] = {post, manage};
 rlw_audio_requests(a, seq, 2);
 RlwAudioStats s; rlw_audio_stats(a, &s);
 CHECK(s.posts == 1 && s.manages == 1 && s.commands >= 2 && s.plays_started == 1 && s.missing_samples == 0);
 CHECK(rlw_mixer_playing(rlw_audio_mixer(a)) == 1);
 int16_t out[2 * 4096]; rlw_audio_render(a, out, 4096);
 Stats st = stats(out, 2 * 4096); CHECK(st.peak > 1000);
 rlw_audio_render(a, out, 4096);                       /* 2,034 frames at 22,050 Hz end within 4,068 output frames */
 rlw_audio_requests(a, &manage, 1); rlw_audio_stats(a, &s);
 CHECK(s.finished == 1 && rlw_mixer_playing(rlw_audio_mixer(a)) == 0);
 /* Again, then StopAllSound (kind 3) silences it. */
 rlw_audio_requests(a, seq, 2); CHECK(rlw_mixer_playing(rlw_audio_mixer(a)) == 1);
 RlwAudioRequest stop_all = {3, 20, 3, 0x429335, 0, 0, 0, 0, 0, 0};
 rlw_audio_requests(a, &stop_all, 1); rlw_audio_stats(a, &s);
 CHECK(s.stop_alls == 1 && rlw_mixer_playing(rlw_audio_mixer(a)) == 0);
 rlw_audio_render(a, out, 64); CHECK(stats(out, 128).peak == 0);
 /* An unknown sound is refused without a voice; effects gain 0 mutes. */
 RlwAudioRequest unknown = {1, 30, 3, 0x41b785, 0x41, 0x99999, 0x49a1d0, 0, 0x100, 1};
 RlwAudioRequest seq2[2] = {unknown, manage}; rlw_audio_requests(a, seq2, 2);
 CHECK(rlw_mixer_playing(rlw_audio_mixer(a)) == 0);
 rlw_audio_set_effects_gain(a, 0); rlw_audio_requests(a, seq, 2); rlw_audio_render(a, out, 512); CHECK(stats(out, 1024).peak == 0);
 /* The event loop tick posts nothing while the start sound's reference is free. */
 rlw_audio_event_tick(a, 40); rlw_audio_stats(a, &s); CHECK(s.event_ticks == 0);
 rlw_audio_reset(a); CHECK(rlw_mixer_playing(rlw_audio_mixer(a)) == 0 && rlw_audio_enabled(a));
 /* The event start sound holds reference 0x496cd0: the display tick posts the loop request. */
 RlwAudioRequest start = {1, 50, 3, 0x45827f, 0x41, 0x20800, 0x496cd0, 0, 0x100, 1}, seq3[2] = {start, manage};
 rlw_audio_requests(a, seq3, 2); rlw_audio_event_tick(a, 51); rlw_audio_stats(a, &s);
 CHECK(s.event_ticks == 1 && s.posts == 2);
 rlw_audio_free(a);
 RlwAudio *none = rlw_audio_create(NULL, 44100, NULL);
 CHECK(none && !rlw_audio_enabled(none) && !strcmp(rlw_audio_availability(none), "ausente"));
 rlw_audio_requests(none, seq, 2); rlw_audio_free(none);
}

/* rlw_sound_import from a CD folder: all six files to Dados/sound, or nothing. */
static void test_import(const char *work) {
 Banks b; make_banks(&b);
 char cd[4096], sound[4200], root[4096], dados[4200];
 snprintf(cd, sizeof(cd), "%s" RLW_SEP "cd", work); snprintf(root, sizeof(root), "%s" RLW_SEP "data", work);
 snprintf(sound, sizeof(sound), "%s" RLW_SEP "INSTALHD" RLW_SEP "SOUND", cd); snprintf(dados, sizeof(dados), "%s" RLW_SEP "Dados", root);
 rlw_remove_tree(cd); rlw_remove_tree(root); rlw_mkdirs(sound); rlw_mkdirs(dados);
 static const char letters[3] = {'A', 'B', 'P'};
 for (unsigned k = 0; k < 3; k++) {
  char path[4300];
  snprintf(path, sizeof(path), "%s" RLW_SEP "%c.HED", sound, letters[k]); rlw_write_file(path, b.hed[k], b.hed_length[k]);
  if (k < 2) { snprintf(path, sizeof(path), "%s" RLW_SEP "%c.SND", sound, letters[k]); rlw_write_file(path, b.snd[k], b.snd_length[k]); }
 }
 char error[512] = {0};
 rlw_sound_test_ignore_pins = 1;
 CHECK(rlw_sound_import(cd, root, error, sizeof(error)) != 0 && strstr(error, "P.SND") && !rlw_sound_installed(dados));
 char path[4300]; snprintf(path, sizeof(path), "%s" RLW_SEP "P.SND", sound); rlw_write_file(path, b.snd[2], b.snd_length[2]);
 CHECK(rlw_sound_import(cd, root, error, sizeof(error)) == 0 && rlw_sound_installed(dados));
 char folder[4300]; snprintf(folder, sizeof(folder), "%s" RLW_SEP "sound", dados);
 RlwSoundBanks *banks = rlw_sound_banks_open(folder, error, sizeof(error)); CHECK(banks && rlw_sound_sample(banks, b.key_ima)); rlw_sound_banks_free(banks);
 /* With the pins on, synthetic banks are not the Brazilian CD's: refused, the old import stays. */
 rlw_sound_test_ignore_pins = 0;
 CHECK(rlw_sound_import(cd, root, error, sizeof(error)) != 0 && strstr(error, "SHA-256") && rlw_sound_installed(dados));
}

static void test_settings(void) {
 RlwAudioSettings s; rlw_audio_settings_default(&s);
 CHECK(s.effects_volume == 100 && !s.effects_muted && rlw_audio_settings_effects_gain(&s) == 1.0f && rlw_audio_settings_music_gain(&s) == 1.0f);
 const char *ini = "atalhos=1\r\nefeitos_volume=50\r\nefeitos_mudo=1\nmusica_volume=33\r\nmusica_mudo=1\r\nvelocidade=1\r\nmusica_volume=x\r\n";
 rlw_audio_settings_parse(&s, ini, strlen(ini));
 CHECK(s.effects_volume == 50 && s.effects_muted == 1 && s.music_volume == 100 && s.music_muted == 1);  /* 33 % is no level */
 CHECK(rlw_audio_settings_effects_gain(&s) == 0.0f && rlw_audio_settings_music_gain(&s) == 0.0f);
 s.effects_muted = 0; CHECK(rlw_audio_settings_effects_gain(&s) == 0.5f);
 char text[128]; size_t n = rlw_audio_settings_format(&s, text, sizeof(text));
 CHECK(n > 0 && !strcmp(text, "efeitos_volume=50\r\nefeitos_mudo=0\r\nmusica_volume=100\r\nmusica_mudo=1\r\n"));
 RlwAudioSettings back; rlw_audio_settings_default(&back); rlw_audio_settings_parse(&back, text, n);
 CHECK(!memcmp(&back, &s, sizeof(s)));
 CHECK(rlw_audio_settings_format(&s, text, 10) == 0 && text[0] == 0);
}

static void test_cdmusic(const char *work) {
 /* The Brazilian disc's layout (the same INDEX values as its CUE) gives exactly the pinned windows. */
 const char *cue = "FILE \"DISC.BIN\" BINARY\r\n  TRACK 01 MODE2/2352\r\n    INDEX 01 00:00:00\r\n  TRACK 02 AUDIO\r\n"
                   "    INDEX 00 50:28:33\r\n    INDEX 01 50:30:33\r\n  TRACK 03 AUDIO\r\n    INDEX 00 59:53:28\r\n    INDEX 01 59:55:28\r\n";
 RlwCdToc toc;
 CHECK(rlw_cdmusic_parse_cue(cue, strlen(cue), RLW_CDMUSIC_IMAGE_BYTES, &toc) == 0 && toc.count == 3 && !strcmp(toc.file, "DISC.BIN"));
 for (unsigned i = 0; i < 2; i++) {
  const RlwCdTrack *t = rlw_cdmusic_track(&toc, RLW_CDMUSIC_PINS[i].track);
  CHECK(t && t->audio && t->offset == RLW_CDMUSIC_PINS[i].offset && t->length == RLW_CDMUSIC_PINS[i].pcm_bytes);
 }
 CHECK(!rlw_cdmusic_track(&toc, 1)->audio && rlw_cdmusic_track(&toc, 4) == NULL);
 CHECK(rlw_cdmusic_parse_cue(cue, strlen(cue), 1000, &toc) != 0);                 /* image too small */
 const char *two = "FILE \"A.BIN\" BINARY\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nFILE \"B.BIN\" BINARY\n";
 CHECK(rlw_cdmusic_parse_cue(two, strlen(two), 1u << 20, &toc) != 0);               /* one image only */
 /* A small synthetic image: data (2 sectors), track 2 (3 sectors), track 3 (2 sectors). */
 char folder[4096], bin[4200], cuepath[4200], root[4200], dados[4300];
 snprintf(folder, sizeof(folder), "%s" RLW_SEP "disc", work); rlw_remove_tree(folder); rlw_mkdirs(folder);
 snprintf(bin, sizeof(bin), "%s" RLW_SEP "SMALL.BIN", folder); snprintf(cuepath, sizeof(cuepath), "%s" RLW_SEP "SMALL.CUE", folder);
 snprintf(root, sizeof(root), "%s" RLW_SEP "musicdata", work); snprintf(dados, sizeof(dados), "%s" RLW_SEP "Dados", root);
 rlw_remove_tree(root); rlw_mkdirs(dados);
 uint32_t size = 7 * 2352; uint8_t *image = malloc(size);
 for (uint32_t i = 0; i < size; i++) image[i] = (uint8_t)(i * 7 + (i >> 9));
 rlw_write_file(bin, image, size);
 const char *small = "FILE \"SMALL.BIN\" BINARY\n TRACK 01 MODE1/2352\n  INDEX 01 00:00:00\n TRACK 02 AUDIO\n  INDEX 01 00:00:02\n"
                     " TRACK 03 AUDIO\n  INDEX 00 00:00:04\n  INDEX 01 00:00:05\n";
 rlw_write_file(cuepath, (const uint8_t *)small, (uint32_t)strlen(small));
 char error[512] = {0};
 CHECK(rlw_cdmusic_import(cuepath, root, error, sizeof(error)) == 1 && strstr(error, "SHA-256") && !rlw_cdmusic_installed(dados));
 rlw_cdmusic_test_ignore_pins = 1;
 CHECK(rlw_cdmusic_import(cuepath, root, error, sizeof(error)) == 0 && rlw_cdmusic_installed(dados));
 CHECK(rlw_cdmusic_import(bin, root, error, sizeof(error)) == 2);                  /* a bare .bin of another size: no audio */
 char wav[4400]; snprintf(wav, sizeof(wav), "%s" RLW_SEP "music" RLW_SEP "TRACK02.wav", dados);
 uint32_t n = 0; uint8_t *w = rlw_read_file(wav, &n, 1u << 20), header[44]; rlw_cdmusic_wav_header(header, 3 * 2352);
 CHECK(w && n == 44 + 3 * 2352 && !memcmp(w, header, 44) && !memcmp(w + 44, image + 2 * 2352, 3 * 2352)); free(w);
 /* Player: streams the track's PCM, then ends. */
 char music[4400]; snprintf(music, sizeof(music), "%s" RLW_SEP "music", dados);
 RlwCdPlayer *p = rlw_cdplayer_create(music);
 CHECK(rlw_cdplayer_open(p, 4) != 0 && rlw_cdplayer_open(p, 3) == 0 && rlw_cdplayer_track(p) == 3);
 int16_t frames[2 * 2000]; uint32_t got = rlw_cdplayer_pull(p, frames, 1000), more = rlw_cdplayer_pull(p, frames + 2000, 1000);
 CHECK(got == 1000 && more == 2 * 2352 / 4 - 1000 && !memcmp(frames, image + 5 * 2352, 2 * 2352));
 CHECK(!rlw_cdplayer_ended(p) && rlw_cdplayer_pull(p, frames, 10) == 0 && rlw_cdplayer_ended(p));
 /* Through the mixer: the stream is heard at the music gain, then silence. */
 CHECK(rlw_cdplayer_open(p, 2) == 0);
 RlwMixer *m = rlw_mixer_create(44100); rlw_mixer_set_music(m, rlw_cdplayer_pull, p, 44100);
 int16_t out[2 * 2000]; rlw_mixer_render(m, out, 2000);
 int16_t first[2]; memcpy(first, image + 2 * 2352, 4);
 CHECK(out[0] == first[0] && out[1] == first[1] && out[2 * 1999] == 0 && rlw_cdplayer_ended(p));
 rlw_mixer_free(m); rlw_cdplayer_free(p);
 rlw_cdmusic_test_ignore_pins = 0; free(image);
 /* Policy (NativeCDMusicHost). */
 RlwCdMusic c; rlw_cdmusic_init(&c);
 CHECK(rlw_cdmusic_request(&c, 0x42d32d, 2, 0) == RLW_CDMUSIC_NONE && c.requests == 0);   /* no stage yet */
 CHECK(rlw_cdmusic_begin_stage(&c) == RLW_CDMUSIC_STOP && c.stage_active);
 CHECK(rlw_cdmusic_request(&c, 0x4684bd, 3, 0) == RLW_CDMUSIC_PLAY && c.track == 3);
 rlw_cdmusic_started(&c, 3);
 CHECK(rlw_cdmusic_request(&c, 0x42d32d, 2, 0) == RLW_CDMUSIC_NONE && c.suppressed == 1);  /* suppressed while playing */
 CHECK(rlw_cdmusic_request(&c, 0x4684bd, 2, 0) == RLW_CDMUSIC_PLAY);                      /* MenuInit's site replaces */
 rlw_cdmusic_failed(&c, 2);
 CHECK(rlw_cdmusic_request(&c, 0x4684bd, 2, 0) == RLW_CDMUSIC_NONE && c.failures == 1);   /* failed track not retried */
 CHECK(rlw_cdmusic_request(&c, 0x4684bd, 7, 0) == RLW_CDMUSIC_NONE && c.failures == 2);   /* unsupported track */
 CHECK(rlw_cdmusic_request(&c, 0x4684bd, 3, 1) == RLW_CDMUSIC_NONE && !c.automatic);      /* loop flag: automatic off */
 CHECK(rlw_cdmusic_request(&c, 0x4684bd, 3, 0) == RLW_CDMUSIC_NONE);
 rlw_cdmusic_begin_stage(&c); CHECK(c.automatic && !c.rejected);
 CHECK(rlw_cdmusic_request(&c, 0x123456, 3, 0) == RLW_CDMUSIC_NONE && !c.automatic);      /* unknown site */
 rlw_cdmusic_begin_stage(&c); rlw_cdmusic_request(&c, 0x42d32d, 3, 0); rlw_cdmusic_started(&c, 3);
 CHECK(rlw_cdmusic_result(&c) == RLW_CDMUSIC_NONE && c.playing && !c.stage_active);      /* result keeps the track */
 CHECK(rlw_cdmusic_request(&c, 0x42d32d, 2, 0) == RLW_CDMUSIC_NONE);
 rlw_cdmusic_finished(&c); CHECK(!c.playing && c.finishes == 1);
 rlw_cdmusic_begin_stage(&c); rlw_cdmusic_request(&c, 0x42d32d, 2, 0); rlw_cdmusic_started(&c, 2);
 CHECK(rlw_cdmusic_leave_stage(&c) == RLW_CDMUSIC_STOP && !c.playing && !c.stage_active && c.starts == 3);
}

/* In-game FLC events over a synthetic 3-frame 4x2 FLC (100 ms per frame). */
static int ev_finished, ev_ticks, ev_name_ok = 1;
static int ev_name(void *ctx, uint32_t index, char out[16]) { (void)ctx; if (!ev_name_ok || index != 5) return 1; memset(out, 0, 16); memcpy(out, "TESTE", 5); return 0; }
static int ev_finish(void *ctx) { (void)ctx; ev_finished++; return 0; }
static void ev_tick(void *ctx) { (void)ctx; ev_ticks++; }
static void test_eventflc(const char *work) {
 char folder[4096], path[4200]; snprintf(folder, sizeof(folder), "%s" RLW_SEP "events", work); rlw_mkdirs(folder);
 snprintf(path, sizeof(path), "%s" RLW_SEP "TESTE.FLC", folder);
 uint8_t f[1024]; memset(f, 0, sizeof(f)); uint32_t o = 128;
 put16(f + 4, 0xAF12); put16(f + 6, 3); put16(f + 8, 4); put16(f + 10, 2); put16(f + 12, 8); put32(f + 16, 100); put32(f + 80, 128);
 for (unsigned k = 0; k < 3; k++) {
  uint32_t frame = o; o += 16; unsigned chunks = 1;
  if (k == 0) { uint32_t c = o; put16(f + o + 4, 4); o += 6; put16(f + o, 1); o += 2; f[o++] = 0; f[o++] = 4;
   for (unsigned i = 0; i < 4; i++) { f[o++] = (uint8_t)(10 * i + 1); f[o++] = (uint8_t)(10 * i + 2); f[o++] = (uint8_t)(10 * i + 3); }
   put32(f + c, o - c); chunks++; }
  uint32_t c = o; put16(f + o + 4, 16); o += 6; memset(f + o, (int)(k + 1), 8); o += 8; put32(f + c, o - c);
  put32(f + frame, o - frame); put16(f + frame + 4, 0xF1FA); put16(f + frame + 6, (uint16_t)chunks);
 }
 put32(f, o); rlw_write_file(path, f, o);
 static uint8_t engine[480000]; memset(engine, 0xEE, sizeof(engine));
 static RlwEventFlc e; rlw_eventflc_init(&e, folder);
 RlwEventFlcHost host = {NULL, ev_name, ev_finish, ev_tick};
 RlwEventFlcState idle = {0, 0, 0, 0, 0, 0, 0}, click = {1, 0, 5, 3, 4, 2, 100}, modal = click; modal.modal = 1;
 CHECK(rlw_eventflc_update(&e, &idle, 0, engine, &host) == 0);
 CHECK(rlw_eventflc_update(&e, &click, 1000, engine, &host) == 1 && e.opened == 1 && ev_ticks == 0);   /* opening: frame 1, no tick */
 uint32_t at = 299 * 800 + 398;                                                                        /* (800-4)/2, (600-2)/2 */
 CHECK(e.surface[at] == 1 && e.surface[at + 3] == 1 && e.surface[at + 800] == 1 && e.surface[at - 1] == 0xEE && e.surface[0] == 0xEE);
 CHECK(e.flc.palette[3] == 11 && e.flc.palette[11] == 33);
 CHECK(rlw_eventflc_update(&e, &modal, 1050, engine, &host) == 1 && e.surface[at] == 1 && ev_ticks == 1);
 CHECK(rlw_eventflc_update(&e, &modal, 1120, engine, &host) == 1 && e.surface[at] == 2 && ev_ticks == 2);
 CHECK(rlw_eventflc_update(&e, &modal, 1250, engine, &host) == 1 && e.surface[at] == 3 && ev_ticks == 2);  /* last frame: no tick */
 CHECK(ev_finished == 0);
 CHECK(rlw_eventflc_update(&e, &modal, 1300, engine, &host) == 0 && ev_finished == 1 && e.terminations == 1 && !e.failure[0]);
 CHECK(rlw_eventflc_update(&e, &modal, 1340, engine, &host) == 0 && ev_finished == 1);                 /* once */
 CHECK(rlw_eventflc_update(&e, &idle, 1400, engine, &host) == 0 && !e.finished);                       /* event over: reset */
 /* Not modal yet at the end (click frame only): no terminar before mode 22. */
 CHECK(rlw_eventflc_update(&e, &click, 2000, engine, &host) == 1 && rlw_eventflc_update(&e, &click, 2500, engine, &host) == 1 && ev_finished == 1);
 CHECK(rlw_eventflc_update(&e, &modal, 2501, engine, &host) == 0 && ev_finished == 2);
 rlw_eventflc_update(&e, &idle, 2600, engine, &host);
 /* A header that differs from the declared state, and a missing name, end without a picture. */
 RlwEventFlcState wrong = modal; wrong.frames = 9;
 CHECK(rlw_eventflc_update(&e, &wrong, 3000, engine, &host) == 0 && e.finished && strstr(e.failure, "cabeçalho"));
 rlw_eventflc_update(&e, &idle, 3100, engine, &host); ev_name_ok = 0;
 CHECK(rlw_eventflc_update(&e, &modal, 3200, engine, &host) == 0 && strstr(e.failure, "sem nome"));
 rlw_eventflc_free(&e);
}

/* Original movies over a synthetic AVI: 8x4 Cinepak (V1 frame, then V4 + V1
 * blocks) and one MS ADPCM block (mono, 10 samples). */
typedef struct { uint8_t b[4096]; uint32_t n; } Buf;
static void bput(Buf *x, const void *p, uint32_t n) { memcpy(x->b + x->n, p, n); x->n += n; }
static void b32(Buf *x, uint32_t v) { uint8_t t[4]; put32(t, v); bput(x, t, 4); }
static void b16(Buf *x, uint32_t v) { uint8_t t[2]; put16(t, v); bput(x, t, 2); }
static uint32_t open_chunk(Buf *x, const char *id) { bput(x, id, 4); b32(x, 0); return x->n; }
static void close_chunk(Buf *x, uint32_t at) { put32(x->b + at - 4, x->n - at); if (x->n & 1) x->b[x->n++] = 0; }
static void cinepak(Buf *x, int second) {
 uint8_t f[128]; uint32_t n = 0;
 f[n++] = 0; f[n++] = 0; f[n++] = 0; f[n++] = 0; f[n++] = 0; f[n++] = 8; f[n++] = 0; f[n++] = 4; f[n++] = 0; f[n++] = 1;  /* flags, size, 8x4, 1 strip */
 uint32_t strip = n; f[n++] = 0x10; n += 3; f[n++] = 0; f[n++] = 0; f[n++] = 0; f[n++] = 0; f[n++] = 0; f[n++] = 4; f[n++] = 0; f[n++] = 8;
 /* V1 codebook, one entry: Y 100 (frame 1) or 200 (frame 2), u = v = 0. */
 f[n++] = 0x22; f[n++] = 0; f[n++] = 0; f[n++] = 10; for (int k = 0; k < 4; k++) f[n++] = second ? 200 : 100; f[n++] = 0; f[n++] = 0;
 if (!second) { f[n++] = 0x32; f[n++] = 0; f[n++] = 0; f[n++] = 6; f[n++] = 0; f[n++] = 0; }  /* all V1: two blocks of entry 0 */
 else {
  f[n++] = 0x20; f[n++] = 0; f[n++] = 0; f[n++] = 10; f[n++] = 10; f[n++] = 20; f[n++] = 30; f[n++] = 40; f[n++] = 0; f[n++] = 0;  /* V4 entry 0 */
  f[n++] = 0x30; f[n++] = 0; f[n++] = 0; f[n++] = 13; f[n++] = 0x80; f[n++] = 0; f[n++] = 0; f[n++] = 0;  /* block 1 V4, block 2 V1 */
  f[n++] = 0; f[n++] = 0; f[n++] = 0; f[n++] = 0; f[n++] = 0;
 }
 uint32_t size = n - strip; f[strip + 1] = 0; f[strip + 2] = (uint8_t)(size >> 8); f[strip + 3] = (uint8_t)size;
 f[1] = 0; f[2] = (uint8_t)(n >> 8); f[3] = (uint8_t)n;
 bput(x, f, n);
}
static void test_movie(void) {
 static const int16_t coef[14] = {256, 0, 512, -256, 0, 0, 192, 64, 240, 0, 460, -208, 392, -232};
 Buf *x = calloc(1, sizeof(Buf));
 uint32_t riff = open_chunk(x, "RIFF"); bput(x, "AVI ", 4);
 uint32_t hdrl = open_chunk(x, "LIST"); bput(x, "hdrl", 4);
 uint32_t avih = open_chunk(x, "avih"); b32(x, 66667); for (int i = 0; i < 7; i++) b32(x, 0); b32(x, 8); b32(x, 4); for (int i = 0; i < 4; i++) b32(x, 0); close_chunk(x, avih);
 uint32_t strl = open_chunk(x, "LIST"); bput(x, "strl", 4);
 uint32_t strh = open_chunk(x, "strh"); bput(x, "vidscvid", 8); for (int i = 0; i < 12; i++) b32(x, 0); close_chunk(x, strh);
 uint32_t strf = open_chunk(x, "strf"); b32(x, 40); b32(x, 8); b32(x, 4); b16(x, 1); b16(x, 24); bput(x, "cvid", 4); for (int i = 0; i < 5; i++) b32(x, 0); close_chunk(x, strf);
 close_chunk(x, strl);
 strl = open_chunk(x, "LIST"); bput(x, "strl", 4);
 strh = open_chunk(x, "strh"); bput(x, "auds", 4); b32(x, 0); for (int i = 0; i < 12; i++) b32(x, 0); close_chunk(x, strh);
 strf = open_chunk(x, "strf"); b16(x, 2); b16(x, 1); b32(x, 22050); b32(x, 11025); b16(x, 11); b16(x, 4); b16(x, 32); b16(x, 10); b16(x, 7);
 for (int i = 0; i < 14; i++) b16(x, (uint16_t)coef[i]); close_chunk(x, strf);
 close_chunk(x, strl); close_chunk(x, hdrl);
 uint32_t movi = open_chunk(x, "LIST"); bput(x, "movi", 4);
 uint32_t c = open_chunk(x, "00dc"); cinepak(x, 0); close_chunk(x, c);
 c = open_chunk(x, "01wb"); uint8_t block[11] = {0, 16, 0, 100, 0, 50, 0, 0, 0, 0, 0}; bput(x, block, 11); close_chunk(x, c);
 c = open_chunk(x, "00dc"); cinepak(x, 1); close_chunk(x, c);
 close_chunk(x, movi); close_chunk(x, riff);
 uint8_t *data = malloc(x->n); memcpy(data, x->b, x->n);
 char error[256] = {0};
 RlwMovie *m = rlw_movie_from_memory(data, x->n, error, sizeof(error));
 CHECK(m != NULL); if (!m) { fprintf(stderr, "movie: %s\n", error); free(x); return; }
 CHECK(rlw_movie_width(m) == 8 && rlw_movie_height(m) == 4 && rlw_movie_frames(m) == 2 && rlw_movie_audio_rate(m) == 22050 && rlw_movie_audio_frames(m) == 10);
 CHECK(fabs(rlw_movie_duration_ms(m) - 133.334) < 0.01);
 CHECK(rlw_movie_advance(m, 0) == 1 && rlw_movie_decoded(m) == 1);
 const uint8_t *px = rlw_movie_pixels(m);
 CHECK(px[0] == 100 && px[1] == 100 && px[2] == 100 && px[3] == 255 && px[4 * (3 * 8 + 7)] == 100);
 CHECK(rlw_movie_advance(m, 30) == 0 && rlw_movie_advance(m, 70) == 1 && rlw_movie_decoded(m) == 2);
 /* Block 1 (V4): each 2x2 quarter shows entry 0's four pixels 10/20/30/40; block 2 (V1): entry 0 = 200. */
 CHECK(px[0] == 10 && px[4 * 1] == 20 && px[4 * 8] == 30 && px[4 * 9] == 40 && px[4 * 2] == 10 && px[4 * (3 * 8 + 3)] == 40 && px[4 * (2 * 8 + 3)] == 20);
 CHECK(px[4 * 4] == 200 && px[4 * (3 * 8 + 7)] == 200);
 int16_t audio[2 * 16]; CHECK(rlw_movie_pull(m, audio, 16) == 10 && audio[0] == 50 && audio[1] == 50 && audio[2] == 100 && audio[19] == 100);
 CHECK(rlw_movie_pull(m, audio, 16) == 0);
 rlw_movie_free(m);
 /* Refusals: not an AVI; a video codec other than Cinepak. */
 uint8_t *junk = calloc(1, 64); CHECK(rlw_movie_from_memory(junk, 64, error, sizeof(error)) == NULL && strstr(error, "não é um AVI"));
 uint8_t *other = malloc(x->n); memcpy(other, x->b, x->n);
 for (uint32_t i = 0; i + 4 <= x->n; i++) if (!memcmp(other + i, "cvid", 4)) memcpy(other + i, "IV50", 4);
 CHECK(rlw_movie_from_memory(other, x->n, error, sizeof(error)) == NULL && strstr(error, "não suportado"));
 /* MS ADPCM stereo: one block per channel pair, s2/s1 first, then high/low nibbles. */
 RlwAudioFormat st = {2, 2, 22050, 16, 4, 0, 7, {0}}; for (int i = 0; i < 14; i++) st.coefficients[i] = coef[i];
 uint8_t two[16] = {0, 0, 16, 0, 16, 0, 1, 0, 2, 0, 3, 0, 4, 0, 0x10, 0x00};
 int16_t out[16]; int64_t got = rlw_msadpcm_decode(two, 16, &st, out, 16);
 CHECK(got == 8 && out[0] == 3 && out[1] == 4 && out[2] == 1 && out[3] == 2 && out[4] == 1 + 16 && out[5] == 2 && out[6] == 17 && out[7] == 2);
 st.coefficient_count = 0; CHECK(rlw_msadpcm_decode(two, 16, &st, NULL, 0) == -1);
 free(x);
}

int main(int argc, char **argv) {
 const char *work = argc > 1 ? argv[1] : ".";
 rlw_mkdirs(work);
 test_ima();
 test_banks(work);
 test_mixer(work);
 test_host();
 test_import(work);
 test_settings();
 test_cdmusic(work);
 test_eventflc(work);
 test_movie();
 if (failures) { fprintf(stderr, "test_audio: %d failures\n", failures); return 1; }
 printf("test_audio ok: IMA ADPCM, sound banks, mixer (volume, loops, ends, music, WAV statistics), host over the mapper, sound import, Áudio settings, CD music (CUE, import, player, policy), event FLC, movies (AVI, Cinepak, MS ADPCM)\n");
 return 0;
}
