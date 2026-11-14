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
#include <math.h>

#define RATE 44100u          /* first choice; the device may take 48,000 or 22,050 instead */
#define BLOCKS 4
#define BLOCK_FRAMES 1024u   /* 23 ms each, ~93 ms queued */

struct RlwAudioHost {
 RlwAudio *audio; CRITICAL_SECTION lock;
 HWAVEOUT device; HANDLE event, thread; volatile LONG quit;
 /* Device life: the rate it took, "lost" set by the output thread (a write
  * refused, or no block played for 2 s: device removed or invalidated), the
  * next reopen attempt, frames handed to the device. */
 uint32_t rate; volatile LONG lost; ULONGLONG retry_at; volatile LONG64 rendered;
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
 ULONGLONG progress = GetTickCount64();
 while (!h->quit && !h->lost) {
  for (int i = 0; i < BLOCKS && !h->quit && !h->lost; i++) {
   WAVEHDR *w = &h->headers[i];
   if (w->dwUser && !(w->dwFlags & WHDR_DONE)) continue;   /* still queued */
   rlw_audio_render(h->audio, h->blocks[i], BLOCK_FRAMES);
   w->dwFlags &= ~(DWORD)WHDR_DONE; w->dwUser = 1;
   /* A refused write (MMSYSERR_NODRIVER etc.: the device went away, the waveOut
    * face of WASAPI's AUDCLNT_E_DEVICE_INVALIDATED) hands the device back. */
   if (waveOutWrite(h->device, w, sizeof(*w)) != MMSYSERR_NOERROR) { w->dwUser = 0; InterlockedExchange(&h->lost, 1); break; }
   InterlockedAdd64(&h->rendered, BLOCK_FRAMES); progress = GetTickCount64();
  }
  /* Every block queued and none played for 2 s: a stalled (removed) device. */
  if (GetTickCount64() - progress > 2000) InterlockedExchange(&h->lost, 1);
  WaitForSingleObject(h->event, 50);
 }
 return 0;
}

/* 16-bit stereo PCM at the first rate the device (the WAVE_MAPPER, which
 * follows the default device) accepts; the mixer follows that rate. */
static int open_device(RlwAudioHost *h) {
 static const uint32_t rates[3] = {RATE, 48000u, 22050u};
 if (!waveOutGetNumDevs()) return 1;
 if (!(h->event = CreateEventW(NULL, FALSE, FALSE, NULL))) return 1;
 h->device = NULL;
 for (int r = 0; r < 3 && !h->device; r++) {
  WAVEFORMATEX f; memset(&f, 0, sizeof(f));
  f.wFormatTag = WAVE_FORMAT_PCM; f.nChannels = 2; f.nSamplesPerSec = rates[r]; f.wBitsPerSample = 16;
  f.nBlockAlign = 4; f.nAvgBytesPerSec = rates[r] * 4;
  if (waveOutOpen(&h->device, WAVE_MAPPER, &f, (DWORD_PTR)h->event, 0, CALLBACK_EVENT) != MMSYSERR_NOERROR) h->device = NULL;
  else if (rates[r] != h->rate) { h->rate = rates[r]; rlw_audio_set_rate(h->audio, rates[r]); }
 }
 if (!h->device) return 1;
 h->lost = 0; h->quit = 0;
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
 h->audio = rlw_audio_create(banks, RATE, &lock); h->rate = RATE;
 if (!h->audio) { DeleteCriticalSection(&h->lock); free(h); return NULL; }
 snprintf(folder, sizeof(folder), "%s" RLW_SEP "music", dados ? dados : ".");
 h->player = rlw_cdplayer_create(folder); rlw_cdmusic_init(&h->music);
 if (!dados || !rlw_cdmusic_installed(dados)) snprintf(h->music.notice, sizeof(h->music.notice), "Música do CD indisponível: importe a imagem .BIN/.CUE do CD");
 /* The device carries the effects and the CD music; either may be absent. */
 QueryPerformanceFrequency(&h->frequency); QueryPerformanceCounter(&h->sink_last);
 if (open_device(h)) { close_device(h); h->retry_at = GetTickCount64() + 3000; if (rlw_audio_enabled(h->audio)) rlw_audio_set_availability(h->audio, "sem dispositivo"); }
 return h;
}

void rlw_audio_host_close(RlwAudioHost *h) {
 if (!h) return;
 close_device(h);
 rlw_audio_free(h->audio); rlw_cdplayer_free(h->player); free(h->cd_events); DeleteCriticalSection(&h->lock); free(h);
}
RlwAudio *rlw_audio_host_audio(RlwAudioHost *h) { return h ? h->audio : NULL; }

/* Game thread: a lost device is closed (the game goes on silently, the mixer
 * then runs in real time on the game thread so voices still end), and a
 * missing one is retried every 3 s: a device plugged in, or a new default
 * device, takes over without a restart. */
void rlw_audio_host_maintain(RlwAudioHost *h) {
 if (!h) return;
 ULONGLONG now_ms = GetTickCount64();
 if (h->device && h->lost) {
  close_device(h); h->retry_at = now_ms + 1000;
  if (rlw_audio_enabled(h->audio)) rlw_audio_set_availability(h->audio, "sem dispositivo");
  QueryPerformanceCounter(&h->sink_last);
 }
 if (!h->device && now_ms >= h->retry_at) {
  if (open_device(h)) { close_device(h); h->retry_at = now_ms + 3000; }
  else if (rlw_audio_enabled(h->audio)) rlw_audio_set_availability(h->audio, "pacote");
 }
 if (!h->device) {
  LARGE_INTEGER now; QueryPerformanceCounter(&now);
  uint64_t frames = (uint64_t)(now.QuadPart - h->sink_last.QuadPart) * h->rate / (uint64_t)h->frequency.QuadPart;
  if (frames > h->rate) frames = h->rate;
  if (frames) {
   h->sink_last = now;
   while (frames) { uint32_t k = frames > BLOCK_FRAMES ? BLOCK_FRAMES : (uint32_t)frames; rlw_audio_render(h->audio, h->sink, k); frames -= k; }
  }
 }
}
int rlw_audio_host_device(const RlwAudioHost *h, uint32_t *rate) { if (rate) *rate = h ? h->rate : 0; return h && h->device && !h->lost; }

/* --smoke-audio (CI, no game data): the host without banks, a synthetic 440 Hz
 * tone through the mixer for 0.5 s on the device, or rendered on the game
 * thread without one; exit 0 either way. */
int rlw_audio_host_smoke(char *report, size_t capacity) {
 RlwAudioHost *h = rlw_audio_host_open(NULL);
 if (!h) { snprintf(report, capacity, "audio smoke: sem memória"); return 1; }
 static int16_t tone[22050];
 for (int i = 0; i < 22050; i++) tone[i] = (int16_t)(9000.0 * sin(6.283185307179586 * 440.0 * i / 22050.0));
 RlwSample sample = {tone, 22050, 22050};
 RlwMixer *m = rlw_audio_mixer(h->audio);
 EnterCriticalSection(&h->lock); rlw_mixer_buffer(m, 0x7fff0001u, &sample); rlw_mixer_play(m, 0x7fff0001u, -600, 0); LeaveCriticalSection(&h->lock);
 int device = h->device != NULL; uint32_t rate = h->rate; uint64_t frames = 0;
 if (device) { Sleep(500); frames = (uint64_t)h->rendered; }
 else { static int16_t out[2 * 22050]; rlw_audio_render(h->audio, out, rate / 2 > 22050 ? 22050 : rate / 2); frames = rate / 2 > 22050 ? 22050 : rate / 2; }
 uint32_t finished[8]; EnterCriticalSection(&h->lock); uint32_t ended = rlw_mixer_finished(m, finished, 8); uint64_t clipped = rlw_mixer_clipped(m); LeaveCriticalSection(&h->lock);
 snprintf(report, capacity, "audio smoke ok: dispositivo %s, %u Hz, %llu quadros enviados, tom %s, %llu cortes",
          device ? (h->lost ? "perdido" : "waveOut") : "ausente (silencioso)", rate, (unsigned long long)frames, ended ? "terminado" : "tocando", (unsigned long long)clipped);
 rlw_audio_host_close(h);
 return 0;
}

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
/* After a successful load (and rlw_driver_load_resume): the macOS app pumps the
 * loaded world's frame events (a pause-list save's MenuInit 18) and then calls
 * beginStage, in that order on its main queue: the track stops and the stage
 * is re-armed; the game's next MCICD_Play poll picks the track again. */
void rlw_audio_host_loaded(RlwAudioHost *h, RlwDriver *d) {
 if (!h || !d) return;
 if (h->driver != d) rlw_audio_host_attach(h, d);
 h->music_world = rlw_driver_sound_world(d);
 music_requests(h, d);
 music_action(h, rlw_cdmusic_begin_stage(&h->music));
 RlwStatus s; rlw_driver_status(d, &s); h->last_screen = s.screen;
}
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
 rlw_audio_host_maintain(h);
}
