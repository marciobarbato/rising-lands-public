/* Play-mode sound for the Windows host (front 2, audio): portable mixer and the
 * glue from the engine's sound requests to it. Port of NativeAudio.swift.
 *
 * The driver publishes the engine's ordered sound requests
 * (rlw_driver_sound_events); MacOSAudioMapper (macos/src, reused
 * unchanged) reproduces the original initialized DirectSound queue and turns
 * them into device commands on abstract buffer ids (create from a sample key,
 * duplicate, play with a DirectSound volume and loop flag, volume, stop,
 * release). RlwAudio applies those commands to a software mixer whose voices
 * play the original samples decoded from the imported banks (rlw_sound.c), and
 * answers the mapper's "buffer stopped" questions. Nothing here reads or writes
 * game state.
 *
 * Threads: the game thread calls rlw_audio_reset/_requests/_event_tick/_set_*;
 * the output device's thread calls rlw_audio_render. Pass a lock pair (the
 * Win32 output does) to serialize them; tests pass none and render inline. */
#ifndef RLW_AUDIO_H
#define RLW_AUDIO_H
#include <stddef.h>
#include <stdint.h>
#include "rlw_sound.h"

/* ---- Mixer: voices over shared mono PCM16, stereo PCM16 output ---- */
typedef struct RlwMixer RlwMixer;
#define RLW_MIXER_VOICES 64
RlwMixer *rlw_mixer_create(uint32_t output_rate);
void rlw_mixer_free(RlwMixer *m);
uint32_t rlw_mixer_rate(const RlwMixer *m);
/* Buffer `id` (non-zero) gets a sample (NULL = missing sample: its plays end at
 * once). The sample must outlive the buffer. 0 or 1 when no voice is free. */
int rlw_mixer_buffer(RlwMixer *m, uint32_t id, const RlwSample *sample);
/* DuplicateSoundBuffer: id gets source's sample. */
int rlw_mixer_duplicate(RlwMixer *m, uint32_t id, uint32_t source);
/* Play from position 0 at a DirectSound volume (hundredths of dB, -10000..0). */
void rlw_mixer_play(RlwMixer *m, uint32_t id, int32_t volume, int loop);
void rlw_mixer_volume(RlwMixer *m, uint32_t id, int32_t volume);
void rlw_mixer_stop(RlwMixer *m, uint32_t id);
void rlw_mixer_release(RlwMixer *m, uint32_t id);
void rlw_mixer_silence(RlwMixer *m);   /* releases every buffer */
/* Linear gains 0..1 (the Áudio menu's effects and music levels; mute = 0). */
void rlw_mixer_set_effects_gain(RlwMixer *m, float gain);
void rlw_mixer_set_music_gain(RlwMixer *m, float gain);
/* Music stream (CD audio): a stereo PCM16 source at its own rate pulled by the
 * mixer; fn writes up to `frames` stereo frames and returns the count (fewer =
 * the stream ended). NULL fn removes it. */
typedef uint32_t (*RlwMusicPull)(void *ctx, int16_t *stereo, uint32_t frames);
void rlw_mixer_set_music(RlwMixer *m, RlwMusicPull fn, void *ctx, uint32_t rate);
/* Mixes `frames` stereo frames into out (interleaved L,R). */
void rlw_mixer_render(RlwMixer *m, int16_t *out, uint32_t frames);
/* Buffers whose (non-looping) play reached its end since the last call. */
uint32_t rlw_mixer_finished(RlwMixer *m, uint32_t *ids, uint32_t capacity);
uint32_t rlw_mixer_playing(const RlwMixer *m);   /* voices currently playing */
uint64_t rlw_mixer_clipped(const RlwMixer *m);   /* output samples clamped so far */
/* DirectSound attenuation to a linear gain (NativeAudio.gain). */
float rlw_audio_gain(int32_t hundredths_db);

/* ---- Play-mode sound host (NativeAudioPlayback) ---- */
typedef struct RlwAudio RlwAudio;
typedef struct { void *ctx; void (*lock)(void *ctx); void (*unlock)(void *ctx); } RlwAudioLock;
/* Same record as RlwSoundEvent / MacOSSoundEvent. */
typedef struct { uint32_t kind, frame, phase, site, eax, edx, ebx, ecx, volume, has_volume; } RlwAudioRequest;
typedef struct {
 uint64_t requests, commands, plays_started, missing_samples, event_ticks, finished;
 uint64_t posts, stops, manages, stop_alls, domain_faults, dropped_commands;
} RlwAudioStats;
/* banks: adopted (freed with the host); NULL = no sound ("ausente"). */
RlwAudio *rlw_audio_create(RlwSoundBanks *banks, uint32_t output_rate, const RlwAudioLock *lock);
void rlw_audio_free(RlwAudio *a);
int rlw_audio_enabled(const RlwAudio *a);
/* "pacote" (banks loaded), "ausente" or "sem dispositivo" (set by the output). */
const char *rlw_audio_availability(const RlwAudio *a);
void rlw_audio_set_availability(RlwAudio *a, const char *text);
/* A fresh mapper for a fresh world; silences every voice. */
void rlw_audio_reset(RlwAudio *a);
/* Game thread, after every play-mode advance: the drained requests in order. */
void rlw_audio_requests(RlwAudio *a, const RlwAudioRequest *events, uint32_t count);
/* One modal-event display tick of EvenementAffiche (0x45827f): while the start
 * sound's reference 0x496cd0 is held, PostSound(0x41, 0x40200, 0x496cd4, 1) at
 * volume 0x100 (NativeAudioPlayback.eventTick). */
void rlw_audio_event_tick(RlwAudio *a, uint32_t frame);
void rlw_audio_set_effects_gain(RlwAudio *a, float gain);
void rlw_audio_set_music_gain(RlwAudio *a, float gain);
void rlw_audio_set_music(RlwAudio *a, RlwMusicPull fn, void *ctx, uint32_t rate);
/* Output thread. */
void rlw_audio_render(RlwAudio *a, int16_t *out, uint32_t frames);
void rlw_audio_stats(const RlwAudio *a, RlwAudioStats *out);
RlwMixer *rlw_audio_mixer(RlwAudio *a);

/* ---- Áudio menu settings (NativeAudioSettings) ----
 * Effects and CD music, each a mute toggle and one of four levels (100, 75, 50,
 * 25 %); defaults 100 % and not muted. Kept in opcoes.ini as
 * efeitos_volume=, efeitos_mudo=, musica_volume=, musica_mudo= (whole percent,
 * 0/1); a missing or invalid key keeps its default. */
typedef struct { unsigned effects_volume, effects_muted, music_volume, music_muted; } RlwAudioSettings;
extern const unsigned RLW_AUDIO_LEVELS[4];
void rlw_audio_settings_default(RlwAudioSettings *s);
/* Reads the four keys from opcoes.ini text (other lines ignored). */
void rlw_audio_settings_parse(RlwAudioSettings *s, const char *text, size_t length);
/* Appends the four "key=value\r\n" lines; returns the length written (0 when it does not fit). */
size_t rlw_audio_settings_format(const RlwAudioSettings *s, char *out, size_t capacity);
float rlw_audio_settings_effects_gain(const RlwAudioSettings *s);
float rlw_audio_settings_music_gain(const RlwAudioSettings *s);
#endif
