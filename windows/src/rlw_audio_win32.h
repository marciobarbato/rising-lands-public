/* Windows sound output (front 2, audio): the RlwAudio mixer on a waveOut
 * device, fed from the driver's sound requests. Win32 only.
 *
 * Why waveOut (winmm.dll): it is a system DLL on every Windows (x64 and ARM64),
 * needs no COM, headers or import libraries beyond what mingw-w64 and MSVC
 * already ship (the host already links winmm for timeBeginPeriod), and its
 * ~90 ms of buffering is fine for these 22,050 Hz effects and CD music. On
 * current Windows it is a thin layer over the same shared-mode WASAPI engine.
 * XAudio2 would need C++-style COM callbacks from C; WASAPI more code for no
 * audible gain here. */
#ifndef RLW_AUDIO_WIN32_H
#define RLW_AUDIO_WIN32_H
#include "rlw_audio.h"
#include "rlw_driver.h"
#include "rlw_cdmusic.h"

typedef struct RlwAudioHost RlwAudioHost;
/* dados: the imported Dados folder (its sound/ subfolder holds the banks).
 * Never NULL unless out of memory: without banks or device the host stays
 * silent and says why in rlw_audio_availability. */
RlwAudioHost *rlw_audio_host_open(const char *dados);
void rlw_audio_host_close(RlwAudioHost *h);
RlwAudio *rlw_audio_host_audio(RlwAudioHost *h);
/* Before rlw_driver_advance: enables the driver's sound capture when the host
 * sees this driver for the first time. */
void rlw_audio_host_attach(RlwAudioHost *h, RlwDriver *d);
/* After rlw_driver_advance in play: drains the requests into the mapper and
 * mixer; a new world first resets the playback. */
void rlw_audio_host_pump(RlwAudioHost *h, RlwDriver *d);
/* Movies, credits, quitting (NativeCDMusicHost.leaveStage): stops the CD music. */
void rlw_audio_host_leave_stage(RlwAudioHost *h);
/* The CD music state (notice, track) for the window. */
const RlwCdMusic *rlw_audio_host_music(const RlwAudioHost *h);
#endif
