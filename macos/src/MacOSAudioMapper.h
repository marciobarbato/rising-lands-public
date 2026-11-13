#ifndef MACOS_AUDIO_MAPPER_H
#define MACOS_AUDIO_MAPPER_H
#include <stdint.h>
/* App-side playback mapper for the engine's ordered sound requests.
 *
 * The engine (ABI 1.6, RlPlayerInputEvent kind 10) reports every
 * PlaySnd_PostSound/StopSound request with its full argument registers and
 * stack volume. Its game state models the original's unopened sound library:
 * caller handles stay -1 and nothing is played. This mapper is a shadow of the
 * original's *initialized* request queue (Audio's validated
 * pesquisas-paralelas/audio/fila_audio.py: waiting/playing records, ten
 * physical and ten logical DirectSound slots, entry-word-256 dictator, volume
 * table 0x498f14) running over its own private image. It never reads or writes
 * game memory, so play/validation game state is unchanged by audio.
 *
 * Output is an ordered list of device commands on abstract buffer ids. The
 * device answers only "this buffer stopped playing" (DSB_IsStopped status).
 * All functions are synchronous and allocation-free after creation; callers
 * serialize every call on one thread. */
typedef struct MacOSAudioMapper MacOSAudioMapper;

/* Same layout is produced by macos_live_sound_events. kind 0 PlaySnd_Manage
 * (the WinMain loop tick), 1 PlaySnd_PostSound, 2 PlaySnd_StopSound,
 * 3 PlaySnd_StopAllSound (0x48aac5 -> 0x48a4a8; registers unused).
 * eax/edx/ebx/ecx are the request registers; volume is the pushed stack word. */
typedef struct {
 uint32_t kind,frame,phase,site,eax,edx,ebx,ecx,volume,has_volume;
} MacOSSoundEvent;

enum { MACOS_AUDIO_CREATE=1, MACOS_AUDIO_DUPLICATE=2, MACOS_AUDIO_PLAY=3,
       MACOS_AUDIO_VOLUME=4, MACOS_AUDIO_STOP=5, MACOS_AUDIO_RELEASE=6 };
/* CREATE: buffer <- sample key (bank<<24 | data-header offset in the HED body).
 * DUPLICATE: buffer <- source buffer (DuplicateSoundBuffer, same sample).
 * PLAY: from position 0 with DirectSound volume (hundredths of dB, -10000..0)
 * and loop (Play flags DSBPLAY_LOOPING). VOLUME: SetVolume. STOP, RELEASE. */
typedef struct {
 uint32_t op,buffer,source;int32_t volume;uint32_t loop,frame,phase,sequence;
} MacOSAudioCommand;

typedef struct {
 uint64_t requests,posts,stops,manages,absent,unsupported_selector,commands,dropped_commands;
 uint64_t plays,domain_faults,handle_overflow,buffer_overflow,stop_alls;
} MacOSAudioStats;

uint32_t macos_audio_abi_version(void);
MacOSAudioMapper *macos_audio_create(void);
void macos_audio_free(MacOSAudioMapper *);
/* Exact original HED bytes for selector 0x41 (A), 0x42 (B) or 0x50 (P). */
int32_t macos_audio_load_bank(MacOSAudioMapper *,uint32_t selector,const uint8_t *hed,uint32_t length);
/* Applies one request in order; returns the original routine's EAX-equivalent
 * (PostSound: 1 admitted/updated, 0 refused/absent, EDX for a bad selector). */
uint32_t macos_audio_request(MacOSAudioMapper *,const MacOSSoundEvent *);
/* Device answer: the buffer finished (status no longer playing/looping). */
void macos_audio_device_stopped(MacOSAudioMapper *,uint32_t buffer);
/* Drains pending commands in order. Returns the count written. */
uint32_t macos_audio_commands(MacOSAudioMapper *,MacOSAudioCommand *,uint32_t capacity);
/* Sample key -> bank letter index (0 A,1 B,2 P), HED entry index and variant. */
int32_t macos_audio_sample(MacOSAudioMapper *,uint32_t key,uint32_t *bank,uint32_t *index,uint32_t *variant);
void macos_audio_stats(MacOSAudioMapper *,MacOSAudioStats *);
/* The shadow word at a caller reference address (a handle such as the event's
 * 0x496cd0): the queue index it holds, or 0xffffffff when free. Read only. */
uint32_t macos_audio_reference(MacOSAudioMapper *,uint32_t address);
/* Original 17-entry volume table (initialized bytes at 0x498f14), for tests. */
int32_t macos_audio_volume_table(uint32_t index);
#endif
