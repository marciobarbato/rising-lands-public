/* Portable mixer and play-mode sound host (see rlw_audio.h). */
#include "rlw_audio.h"
#include "MacOSAudioMapper.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ---------------- Mixer ---------------- */
typedef struct {
 uint32_t id; const RlwSample *sample;
 uint64_t position, step;   /* 32.16 fixed point in source frames */
 float gain; int playing, loop;
} Voice;
struct RlwMixer {
 uint32_t rate; Voice voices[RLW_MIXER_VOICES];
 uint32_t finished[RLW_MIXER_VOICES * 4]; uint32_t finished_count;
 float effects_gain, music_gain;
 RlwMusicPull music; void *music_ctx; uint32_t music_rate;
 int16_t music_chunk[2 * 512]; uint32_t music_have, music_read; int music_primed;
 int32_t music_cur[2], music_next[2]; uint64_t music_frac, music_step;
 float *mix; uint32_t mix_frames;
 uint64_t clipped;
};

float rlw_audio_gain(int32_t hundredths_db) {
 if (hundredths_db <= -10000) return 0.0f;
 if (hundredths_db > 0) hundredths_db = 0;
 return (float)pow(10.0, (double)hundredths_db / 2000.0);
}

RlwMixer *rlw_mixer_create(uint32_t output_rate) {
 if (output_rate < 8000 || output_rate > 192000) return NULL;
 RlwMixer *m = calloc(1, sizeof(*m)); if (!m) return NULL;
 m->rate = output_rate; m->effects_gain = 1.0f; m->music_gain = 1.0f;
 return m;
}
void rlw_mixer_free(RlwMixer *m) { if (m) { free(m->mix); free(m); } }
uint32_t rlw_mixer_rate(const RlwMixer *m) { return m ? m->rate : 0; }

static Voice *voice(RlwMixer *m, uint32_t id) {
 if (!m || !id) return NULL;
 for (unsigned i = 0; i < RLW_MIXER_VOICES; i++) if (m->voices[i].id == id) return &m->voices[i];
 return NULL;
}
static void unfinish(RlwMixer *m, uint32_t id) {
 uint32_t keep = 0;
 for (uint32_t i = 0; i < m->finished_count; i++) if (m->finished[i] != id) m->finished[keep++] = m->finished[i];
 m->finished_count = keep;
}
static void finish(RlwMixer *m, uint32_t id) {
 unfinish(m, id);
 if (m->finished_count < sizeof(m->finished) / sizeof(m->finished[0])) m->finished[m->finished_count++] = id;
}
int rlw_mixer_buffer(RlwMixer *m, uint32_t id, const RlwSample *sample) {
 if (!m || !id) return 1;
 Voice *v = voice(m, id);
 if (!v) for (unsigned i = 0; i < RLW_MIXER_VOICES && !v; i++) if (!m->voices[i].id) v = &m->voices[i];
 if (!v) return 1;
 unfinish(m, id);
 memset(v, 0, sizeof(*v)); v->id = id; v->sample = sample; v->gain = 1.0f;
 if (sample && sample->rate) v->step = ((uint64_t)sample->rate << 16) / m->rate;
 return 0;
}
int rlw_mixer_duplicate(RlwMixer *m, uint32_t id, uint32_t source) {
 Voice *s = voice(m, source);
 return rlw_mixer_buffer(m, id, s ? s->sample : NULL);
}
void rlw_mixer_play(RlwMixer *m, uint32_t id, int32_t volume, int loop) {
 Voice *v = voice(m, id); if (!v) return;
 unfinish(m, id);
 v->gain = rlw_audio_gain(volume); v->loop = loop != 0; v->position = 0;
 /* A missing sample, or none at all: the play ends at once (NativeAudio's ended(id)). */
 if (!v->sample || !v->sample->frames || !v->step) { v->playing = 0; finish(m, id); return; }
 v->playing = 1;
}
void rlw_mixer_volume(RlwMixer *m, uint32_t id, int32_t volume) { Voice *v = voice(m, id); if (v) v->gain = rlw_audio_gain(volume); }
void rlw_mixer_stop(RlwMixer *m, uint32_t id) { Voice *v = voice(m, id); if (v) v->playing = 0; }
void rlw_mixer_release(RlwMixer *m, uint32_t id) { Voice *v = voice(m, id); if (v) { unfinish(m, id); memset(v, 0, sizeof(*v)); } }
void rlw_mixer_silence(RlwMixer *m) { if (m) { memset(m->voices, 0, sizeof(m->voices)); m->finished_count = 0; } }
static float clamp01(float g) { return g == g ? (g < 0 ? 0 : g > 1 ? 1 : g) : 1.0f; }
void rlw_mixer_set_effects_gain(RlwMixer *m, float gain) { if (m) m->effects_gain = clamp01(gain); }
void rlw_mixer_set_music_gain(RlwMixer *m, float gain) { if (m) m->music_gain = clamp01(gain); }
void rlw_mixer_set_music(RlwMixer *m, RlwMusicPull fn, void *ctx, uint32_t rate) {
 if (!m) return;
 m->music = rate >= 8000 && rate <= 192000 ? fn : NULL; m->music_ctx = ctx; m->music_rate = rate;
 m->music_have = m->music_read = 0; m->music_primed = 0; m->music_frac = 0;
 m->music_step = m->music ? ((uint64_t)rate << 16) / m->rate : 0;
}
uint32_t rlw_mixer_finished(RlwMixer *m, uint32_t *ids, uint32_t capacity) {
 if (!m || !ids) return 0;
 uint32_t n = m->finished_count < capacity ? m->finished_count : capacity;
 memcpy(ids, m->finished, n * sizeof(*ids));
 memmove(m->finished, m->finished + n, (m->finished_count - n) * sizeof(*ids)); m->finished_count -= n;
 return n;
}
uint32_t rlw_mixer_playing(const RlwMixer *m) {
 uint32_t n = 0; if (m) for (unsigned i = 0; i < RLW_MIXER_VOICES; i++) n += m->voices[i].playing != 0;
 return n;
}
uint64_t rlw_mixer_clipped(const RlwMixer *m) { return m ? m->clipped : 0; }

/* Next stereo frame of the music stream; 0 at its end. */
static int music_frame(RlwMixer *m, int32_t out[2]) {
 if (m->music_read == m->music_have) {
  m->music_have = m->music(m->music_ctx, m->music_chunk, 512); m->music_read = 0;
  if (!m->music_have) return 0;
 }
 out[0] = m->music_chunk[2 * m->music_read]; out[1] = m->music_chunk[2 * m->music_read + 1]; m->music_read++;
 return 1;
}

void rlw_mixer_render(RlwMixer *m, int16_t *out, uint32_t frames) {
 if (!m || !out) return;
 if (frames > m->mix_frames) {
  float *grown = realloc(m->mix, (size_t)frames * 2 * sizeof(float));
  if (!grown) { memset(out, 0, (size_t)frames * 2 * sizeof(int16_t)); return; }
  m->mix = grown; m->mix_frames = frames;
 }
 float *mix = m->mix; memset(mix, 0, (size_t)frames * 2 * sizeof(float));
 /* Effects: mono, centred (DirectSound buffers without pan), linear interpolation. */
 for (unsigned i = 0; i < RLW_MIXER_VOICES; i++) {
  Voice *v = &m->voices[i]; if (!v->playing) continue;
  const int16_t *pcm = v->sample->pcm; uint64_t length = (uint64_t)v->sample->frames << 16;
  float g = v->gain * m->effects_gain;
  for (uint32_t f = 0; f < frames; f++) {
   if (v->position >= length) {
    if (!v->loop) { v->playing = 0; finish(m, v->id); break; }
    v->position %= length;
   }
   uint64_t index = v->position >> 16; uint32_t frac = (uint32_t)(v->position & 0xffff);
   int32_t a = pcm[index], b = index + 1 < v->sample->frames ? pcm[index + 1] : v->loop ? pcm[0] : a;
   float s = (float)a + (float)(b - a) * (float)frac / 65536.0f;
   mix[2 * f] += s * g; mix[2 * f + 1] += s * g;
   v->position += v->step;
  }
 }
 /* Music: stereo at its own rate, linear interpolation between stream frames. */
 if (m->music) {
  if (!m->music_primed) {
   if (!music_frame(m, m->music_cur)) { m->music = NULL; }
   else { if (!music_frame(m, m->music_next)) memcpy(m->music_next, m->music_cur, sizeof(m->music_cur)); m->music_primed = 1; }
  }
  for (uint32_t f = 0; m->music && f < frames; f++) {
   float t = (float)m->music_frac / 65536.0f;
   for (int c = 0; c < 2; c++) mix[2 * f + c] += ((float)m->music_cur[c] + (float)(m->music_next[c] - m->music_cur[c]) * t) * m->music_gain;
   m->music_frac += m->music_step;
   while (m->music_frac >= 0x10000) {
    m->music_frac -= 0x10000; memcpy(m->music_cur, m->music_next, sizeof(m->music_cur));
    if (!music_frame(m, m->music_next)) { m->music = NULL; break; }
   }
  }
 }
 for (uint32_t k = 0; k < 2 * frames; k++) {
  float s = mix[k];
  if (s > 32767.0f) { s = 32767.0f; m->clipped++; } else if (s < -32768.0f) { s = -32768.0f; m->clipped++; }
  out[k] = (int16_t)lrintf(s);
 }
}

/* ---------------- Play-mode sound host ---------------- */
struct RlwAudio {
 RlwSoundBanks *banks; MacOSAudioMapper *mapper; RlwMixer *mixer;
 RlwAudioLock lock; char availability[32];
 MacOSAudioCommand commands[256];
 RlwAudioStats stats;
};
static void lock(RlwAudio *a) { if (a->lock.lock) a->lock.lock(a->lock.ctx); }
static void unlock(RlwAudio *a) { if (a->lock.unlock) a->lock.unlock(a->lock.ctx); }

RlwAudio *rlw_audio_create(RlwSoundBanks *banks, uint32_t output_rate, const RlwAudioLock *lk) {
 RlwAudio *a = calloc(1, sizeof(*a));
 if (!a || !(a->mixer = rlw_mixer_create(output_rate))) { free(a); rlw_sound_banks_free(banks); return NULL; }
 if (lk) a->lock = *lk;
 a->banks = banks;
 snprintf(a->availability, sizeof(a->availability), "%s", banks ? "pacote" : "ausente");
 rlw_audio_reset(a);
 return a;
}
void rlw_audio_free(RlwAudio *a) {
 if (!a) return;
 if (a->mapper) macos_audio_free(a->mapper);
 rlw_mixer_free(a->mixer); rlw_sound_banks_free(a->banks); free(a);
}
int rlw_audio_enabled(const RlwAudio *a) { return a && a->mapper != NULL; }
const char *rlw_audio_availability(const RlwAudio *a) { return a ? a->availability : "ausente"; }
void rlw_audio_set_availability(RlwAudio *a, const char *text) { if (a && text) snprintf(a->availability, sizeof(a->availability), "%s", text); }
RlwMixer *rlw_audio_mixer(RlwAudio *a) { return a ? a->mixer : NULL; }

void rlw_audio_reset(RlwAudio *a) {
 if (!a) return;
 if (a->mapper) macos_audio_free(a->mapper);
 a->mapper = NULL;
 if (a->banks && (a->mapper = macos_audio_create()) != NULL) {
  for (unsigned b = 0; b < 3; b++) {
   uint32_t n = 0; const uint8_t *hed = rlw_sound_banks_hed(a->banks, b, &n);
   if (macos_audio_load_bank(a->mapper, RLW_SOUND_SELECTORS[b], hed, n)) {
    macos_audio_free(a->mapper); a->mapper = NULL; snprintf(a->availability, sizeof(a->availability), "ausente"); break;
   }
  }
 }
 lock(a); rlw_mixer_silence(a->mixer); unlock(a);
}

/* NativeAudioPlayback.drain + apply: commands in order on the mixer. */
static void drain(RlwAudio *a) {
 for (;;) {
  uint32_t n = macos_audio_commands(a->mapper, a->commands, 256);
  if (!n) return;
  a->stats.commands += n;
  /* Decode samples outside the lock (first use of a sample only). */
  const RlwSample *samples[256];
  for (uint32_t i = 0; i < n; i++) {
   samples[i] = NULL;
   if (a->commands[i].op == MACOS_AUDIO_CREATE) {
    samples[i] = rlw_sound_sample(a->banks, a->commands[i].source);
    if (!samples[i]) a->stats.missing_samples++;
   }
  }
  lock(a);
  for (uint32_t i = 0; i < n; i++) {
   const MacOSAudioCommand *c = &a->commands[i];
   switch (c->op) {
   case MACOS_AUDIO_CREATE: rlw_mixer_buffer(a->mixer, c->buffer, samples[i]); break;
   case MACOS_AUDIO_DUPLICATE: rlw_mixer_duplicate(a->mixer, c->buffer, c->source); break;
   case MACOS_AUDIO_PLAY: {
    Voice *v = voice(a->mixer, c->buffer);
    if (v && v->sample) a->stats.plays_started++;
    rlw_mixer_play(a->mixer, c->buffer, c->volume, (int)c->loop);
    break;
   }
   case MACOS_AUDIO_VOLUME: rlw_mixer_volume(a->mixer, c->buffer, c->volume); break;
   case MACOS_AUDIO_STOP: rlw_mixer_stop(a->mixer, c->buffer); break;
   case MACOS_AUDIO_RELEASE: rlw_mixer_release(a->mixer, c->buffer); break;
   default: break;
   }
  }
  unlock(a);
 }
}
/* The device's answers since the last call (buffers that finished playing). */
static void answers(RlwAudio *a) {
 uint32_t ids[RLW_MIXER_VOICES * 4], n;
 lock(a); n = rlw_mixer_finished(a->mixer, ids, RLW_MIXER_VOICES * 4); unlock(a);
 for (uint32_t i = 0; i < n; i++) macos_audio_device_stopped(a->mapper, ids[i]);
 a->stats.finished += n;
}
void rlw_audio_requests(RlwAudio *a, const RlwAudioRequest *events, uint32_t count) {
 if (!a || !a->mapper) return;
 answers(a);
 for (uint32_t i = 0; i < count; i++) {
  MacOSSoundEvent e = {events[i].kind, events[i].frame, events[i].phase, events[i].site, events[i].eax, events[i].edx,
                       events[i].ebx, events[i].ecx, events[i].volume, events[i].has_volume};
  (void)macos_audio_request(a->mapper, &e);
 }
 a->stats.requests += count;
 drain(a);
}
void rlw_audio_event_tick(RlwAudio *a, uint32_t frame) {
 if (!a || !a->mapper || macos_audio_reference(a->mapper, 0x496cd0u) == 0xffffffffu) return;
 answers(a);
 MacOSSoundEvent e = {1, frame, 3, 0x45827fu, 0x41u, 0x40200u, 0x496cd4u, 1, 0x100u, 1};
 (void)macos_audio_request(a->mapper, &e);
 a->stats.requests++; a->stats.event_ticks++;
 drain(a);
}
void rlw_audio_set_effects_gain(RlwAudio *a, float gain) { if (a) { lock(a); rlw_mixer_set_effects_gain(a->mixer, gain); unlock(a); } }
void rlw_audio_set_music_gain(RlwAudio *a, float gain) { if (a) { lock(a); rlw_mixer_set_music_gain(a->mixer, gain); unlock(a); } }
void rlw_audio_set_music(RlwAudio *a, RlwMusicPull fn, void *ctx, uint32_t rate) { if (a) { lock(a); rlw_mixer_set_music(a->mixer, fn, ctx, rate); unlock(a); } }
void rlw_audio_render(RlwAudio *a, int16_t *out, uint32_t frames) {
 if (!a) { if (out) memset(out, 0, (size_t)frames * 2 * sizeof(int16_t)); return; }
 lock(a); rlw_mixer_render(a->mixer, out, frames); unlock(a);
}
void rlw_audio_stats(const RlwAudio *a, RlwAudioStats *out) {
 if (!a || !out) return;
 *out = a->stats;
 if (a->mapper) {
  MacOSAudioStats s; macos_audio_stats(a->mapper, &s);
  out->posts = s.posts; out->stops = s.stops; out->manages = s.manages; out->stop_alls = s.stop_alls;
  out->domain_faults = s.domain_faults; out->dropped_commands = s.dropped_commands;
 }
}

/* ---------------- Áudio settings ---------------- */
const unsigned RLW_AUDIO_LEVELS[4] = {100, 75, 50, 25};
static int level_ok(unsigned v) { for (unsigned i = 0; i < 4; i++) if (RLW_AUDIO_LEVELS[i] == v) return 1; return 0; }
void rlw_audio_settings_default(RlwAudioSettings *s) { if (s) { s->effects_volume = 100; s->effects_muted = 0; s->music_volume = 100; s->music_muted = 0; } }
void rlw_audio_settings_parse(RlwAudioSettings *s, const char *text, size_t length) {
 if (!s || !text) return;
 size_t i = 0;
 while (i < length) {
  size_t j = i; while (j < length && text[j] != '\n' && text[j] != '\r') j++;
  char line[96]; size_t n = j - i < sizeof(line) - 1 ? j - i : sizeof(line) - 1; memcpy(line, text + i, n); line[n] = 0;
  i = j; while (i < length && (text[i] == '\n' || text[i] == '\r')) i++;
  char *eq = strchr(line, '='); if (!eq) continue;
  char *end = NULL; unsigned long v = strtoul(eq + 1, &end, 10); if (end == eq + 1 || (*end && *end != ' ')) continue;
  *eq = 0;
  if (!strcmp(line, "efeitos_volume") && level_ok((unsigned)v)) s->effects_volume = (unsigned)v;
  else if (!strcmp(line, "efeitos_mudo") && v <= 1) s->effects_muted = (unsigned)v;
  else if (!strcmp(line, "musica_volume") && level_ok((unsigned)v)) s->music_volume = (unsigned)v;
  else if (!strcmp(line, "musica_mudo") && v <= 1) s->music_muted = (unsigned)v;
 }
}
size_t rlw_audio_settings_format(const RlwAudioSettings *s, char *out, size_t capacity) {
 if (!s || !out || !capacity) return 0;
 int n = snprintf(out, capacity, "efeitos_volume=%u\r\nefeitos_mudo=%u\r\nmusica_volume=%u\r\nmusica_mudo=%u\r\n",
                  s->effects_volume, s->effects_muted, s->music_volume, s->music_muted);
 if (n < 0 || (size_t)n >= capacity) { out[0] = 0; return 0; }
 return (size_t)n;
}
float rlw_audio_settings_effects_gain(const RlwAudioSettings *s) { return !s || s->effects_muted ? 0.0f : (float)s->effects_volume / 100.0f; }
float rlw_audio_settings_music_gain(const RlwAudioSettings *s) { return !s || s->music_muted ? 0.0f : (float)s->music_volume / 100.0f; }
