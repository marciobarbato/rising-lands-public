/* waveOut output for RlwAudio and the driver pump (see rlw_audio_win32.h). */
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <mmsystem.h>
#include "rlw_audio_win32.h"
#include "rlw_fs.h"
#include "rlw_cdmusic.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define RATE 44100u
#define BLOCKS 4
#define BLOCK_FRAMES 1024u   /* 23 ms each, ~93 ms queued */

struct RlwAudioHost {
 RlwAudio *audio; CRITICAL_SECTION lock;
 HWAVEOUT device; HANDLE event, thread; volatile LONG quit;
 WAVEHDR headers[BLOCKS]; int16_t blocks[BLOCKS][BLOCK_FRAMES * 2];
 RlwDriver *driver; uint64_t world;
 /* Without a device the mixer still runs on the game thread, in real time, so
  * voices end and the mapper's queue keeps moving (NativeAudio's ended(id)). */
 LARGE_INTEGER sink_last, frequency; int16_t sink[BLOCK_FRAMES * 2];
 RlwSoundEvent events[256]; RlwAudioRequest requests[256];
 /* CD music (NativeCDMusicHost + NativeCDMusicPlayer). */
 RlwCdMusic music; RlwCdPlayer *player; uint64_t music_world; uint32_t last_screen;
 RlFrameEvent *cd_events; uint32_t cd_capacity;
};

static void host_lock(void *ctx) { EnterCriticalSection(&((RlwAudioHost *)ctx)->lock); }
static void host_unlock(void *ctx) { LeaveCriticalSection(&((RlwAudioHost *)ctx)->lock); }

static DWORD WINAPI output_thread(LPVOID p) {
 RlwAudioHost *h = p;
 SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL);
 while (!h->quit) {
  for (int i = 0; i < BLOCKS && !h->quit; i++) {
   WAVEHDR *w = &h->headers[i];
   if (w->dwUser && !(w->dwFlags & WHDR_DONE)) continue;   /* still queued */
   rlw_audio_render(h->audio, h->blocks[i], BLOCK_FRAMES);
   w->dwFlags &= ~(DWORD)WHDR_DONE; w->dwUser = 1;
   if (waveOutWrite(h->device, w, sizeof(*w)) != MMSYSERR_NOERROR) w->dwUser = 0;
  }
  WaitForSingleObject(h->event, 50);
 }
 return 0;
}

static int open_device(RlwAudioHost *h) {
 WAVEFORMATEX f; memset(&f, 0, sizeof(f));
 f.wFormatTag = WAVE_FORMAT_PCM; f.nChannels = 2; f.nSamplesPerSec = RATE; f.wBitsPerSample = 16;
 f.nBlockAlign = 4; f.nAvgBytesPerSec = RATE * 4;
 if (!(h->event = CreateEventW(NULL, FALSE, FALSE, NULL))) return 1;
 if (waveOutOpen(&h->device, WAVE_MAPPER, &f, (DWORD_PTR)h->event, 0, CALLBACK_EVENT) != MMSYSERR_NOERROR) { h->device = NULL; return 1; }
 for (int i = 0; i < BLOCKS; i++) {
  WAVEHDR *w = &h->headers[i]; memset(w, 0, sizeof(*w));
  w->lpData = (LPSTR)h->blocks[i]; w->dwBufferLength = sizeof(h->blocks[i]);
  if (waveOutPrepareHeader(h->device, w, sizeof(*w)) != MMSYSERR_NOERROR) return 1;
 }
 h->thread = CreateThread(NULL, 0, output_thread, h, 0, NULL);
 return h->thread ? 0 : 1;
}

static void close_device(RlwAudioHost *h) {
 if (h->thread) { InterlockedExchange(&h->quit, 1); SetEvent(h->event); WaitForSingleObject(h->thread, INFINITE); CloseHandle(h->thread); h->thread = NULL; }
 if (h->device) {
  waveOutReset(h->device);
  for (int i = 0; i < BLOCKS; i++) if (h->headers[i].dwFlags & WHDR_PREPARED) waveOutUnprepareHeader(h->device, &h->headers[i], sizeof(h->headers[i]));
  waveOutClose(h->device); h->device = NULL;
 }
 if (h->event) { CloseHandle(h->event); h->event = NULL; }
}

RlwAudioHost *rlw_audio_host_open(const char *dados) {
 RlwAudioHost *h = calloc(1, sizeof(*h)); if (!h) return NULL;
 InitializeCriticalSection(&h->lock);
 char folder[4096]; snprintf(folder, sizeof(folder), "%s" RLW_SEP "sound", dados ? dados : ".");
 RlwSoundBanks *banks = dados ? rlw_sound_banks_open(folder, NULL, 0) : NULL;
 RlwAudioLock lock = {h, host_lock, host_unlock};
 h->audio = rlw_audio_create(banks, RATE, &lock);
 if (!h->audio) { DeleteCriticalSection(&h->lock); free(h); return NULL; }
 snprintf(folder, sizeof(folder), "%s" RLW_SEP "music", dados ? dados : ".");
 h->player = rlw_cdplayer_create(folder); rlw_cdmusic_init(&h->music);
 if (!dados || !rlw_cdmusic_installed(dados)) snprintf(h->music.notice, sizeof(h->music.notice), "Música do CD indisponível: importe a imagem .BIN/.CUE do CD");
 /* The device carries the effects and the CD music; either may be absent. */
 if (open_device(h)) { close_device(h); if (rlw_audio_enabled(h->audio)) rlw_audio_set_availability(h->audio, "sem dispositivo"); }
 QueryPerformanceFrequency(&h->frequency); QueryPerformanceCounter(&h->sink_last);
 return h;
}

void rlw_audio_host_close(RlwAudioHost *h) {
 if (!h) return;
 close_device(h);
 rlw_audio_free(h->audio); rlw_cdplayer_free(h->player); free(h->cd_events); DeleteCriticalSection(&h->lock); free(h);
}
RlwAudio *rlw_audio_host_audio(RlwAudioHost *h) { return h ? h->audio : NULL; }

/* ---- CD music ---- */
static void music_action(RlwAudioHost *h, int action) {
 if (action == RLW_CDMUSIC_NONE) return;
 rlw_audio_set_music(h->audio, NULL, NULL, 0); rlw_cdplayer_close(h->player);
 if (action != RLW_CDMUSIC_PLAY) return;
 uint32_t track = h->music.track;
 if (rlw_cdplayer_open(h->player, track)) { rlw_cdmusic_failed(&h->music, track); return; }
 rlw_audio_set_music(h->audio, rlw_cdplayer_pull, h->player, 44100);
 rlw_cdmusic_started(&h->music, track);
}
/* pumpCDMusic: the MCICD_Play boundaries (kind 2, id 102) of the world's last step. */
static void music_requests(RlwAudioHost *h, RlwDriver *d) {
 uint32_t n = 0; int c = rlw_driver_cd_events(d, h->cd_events, h->cd_capacity, &n);
 if (c == RL_BUFFER_TOO_SMALL && n > h->cd_capacity && n <= 65536) {
  RlFrameEvent *grown = realloc(h->cd_events, (size_t)n * sizeof(*grown));
  if (grown) { h->cd_events = grown; h->cd_capacity = n; c = rlw_driver_cd_events(d, h->cd_events, h->cd_capacity, &n); }
 }
 if (c) return;
 for (uint32_t i = 0; i < n && i < h->cd_capacity; i++) {
  const RlFrameEvent *e = &h->cd_events[i];
  if (e->kind == 2 && e->id == 102) music_action(h, rlw_cdmusic_request(&h->music, e->site, e->registers[0], e->registers[1] & 255));
 }
}
/* A new world: a stage start, load or next stage arms the music again
 * (beginStage); a world resumed from the pause menu keeps the current track.
 * Its creation's frame events (the menus' MCICD_Play) are read before play. */
static int music_world(RlwAudioHost *h, RlwDriver *d) {
 uint64_t world = rlw_driver_sound_world(d);
 if (world == h->music_world) return 0;
 if (h->last_screen != 16) music_action(h, rlw_cdmusic_begin_stage(&h->music));
 else if (!h->music.stage_active) h->music.stage_active = 1;
 h->music_world = world;
 music_requests(h, d);
 return 1;
}

void rlw_audio_host_attach(RlwAudioHost *h, RlwDriver *d) {
 if (!h || !d) return;
 if (h->driver != d) {
  h->driver = d; h->world = rlw_driver_sound_world(d);
  if (rlw_audio_enabled(h->audio)) { (void)rlw_driver_sound_capture(d, 1); rlw_audio_reset(h->audio); }
 }
 (void)music_world(h, d);
}

void rlw_audio_host_leave_stage(RlwAudioHost *h) { if (h) music_action(h, rlw_cdmusic_leave_stage(&h->music)); }
const RlwCdMusic *rlw_audio_host_music(const RlwAudioHost *h) { return h ? &h->music : NULL; }

void rlw_audio_host_pump(RlwAudioHost *h, RlwDriver *d) {
 if (!h || !d) return;
 if (h->driver != d) rlw_audio_host_attach(h, d);
 if (rlw_audio_enabled(h->audio)) {
  uint64_t world = rlw_driver_sound_world(d);
  if (world != h->world) { h->world = world; rlw_audio_reset(h->audio); }
  uint32_t n;
  do {
   n = rlw_driver_sound_events(d, h->events, 256);
   for (uint32_t i = 0; i < n; i++) {
    const RlwSoundEvent *e = &h->events[i];
    h->requests[i] = (RlwAudioRequest){e->kind, e->frame, e->phase, e->site, e->eax, e->edx, e->ebx, e->ecx, e->volume, e->has_volume};
   }
   rlw_audio_requests(h->audio, h->requests, n);
  } while (n == 256);
 }
 if (!music_world(h, d)) music_requests(h, d);
 if (rlw_cdplayer_ended(h->player)) { music_action(h, RLW_CDMUSIC_STOP); rlw_cdmusic_finished(&h->music); }
 RlwStatus s; rlw_driver_status(d, &s);
 if (s.screen == 19 && h->last_screen != 19) music_action(h, rlw_cdmusic_result(&h->music));
 if (s.stopped) music_action(h, rlw_cdmusic_leave_stage(&h->music));
 h->last_screen = s.screen;
 if (!h->device) {
  LARGE_INTEGER now; QueryPerformanceCounter(&now);
  uint64_t frames = (uint64_t)(now.QuadPart - h->sink_last.QuadPart) * RATE / (uint64_t)h->frequency.QuadPart;
  if (frames > RATE) frames = RATE;
  if (frames) {
   h->sink_last = now;
   while (frames) { uint32_t k = frames > BLOCK_FRAMES ? BLOCK_FRAMES : (uint32_t)frames; rlw_audio_render(h->audio, h->sink, k); frames -= k; }
  }
 }
}
