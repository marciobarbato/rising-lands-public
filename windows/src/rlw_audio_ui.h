/* Sound in the Windows host's window (front 2, audio), Win32 only: the one
 * sound host of the process, the "Áudio" menu (mute and level for effects and
 * CD music, as the macOS Áudio menu) and the sound import after the CD import.
 * rlw_main.c only calls these hooks; all logic lives in the rlw_audio* files. */
#ifndef RLW_AUDIO_UI_H
#define RLW_AUDIO_UI_H
#include <stddef.h>
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include "rlw_audio.h"
#include "rlw_driver.h"

/* After a successful rlw_cd_import from `source`: imports the sound banks too.
 * A failure is reported on stderr / in the Áudio menu; the game still runs. */
void rlw_audio_ui_import(const char *source, const char *data_root);
/* Opens the sound host over <dados>/sound and reads the Áudio settings from
 * options_path ("" = defaults, never stored). store() rewrites opcoes.ini
 * (rlw_main.c's options_store, which appends rlw_audio_ui_ini). */
void rlw_audio_ui_open(const char *dados, const char *options_path, void (*store)(void));
void rlw_audio_ui_close(void);
/* Around every play-mode rlw_driver_advance. */
void rlw_audio_ui_attach(RlwDriver *d);
void rlw_audio_ui_pump(RlwDriver *d);
/* The in-game FLC event's picture (centred FLC frame over the game, with the
 * FLC palette) when it replaces the engine's this frame; 0 = present the
 * driver's own pixels and palette. */
int rlw_audio_ui_picture(RlwDriver *d, const uint8_t **pixels, const uint8_t **palette);
/* Original movies (NativeDefeatMovie): the host request of an introduction,
 * defeat or campaign movie. 1 = the movie plays (complete it when
 * rlw_audio_ui_movie_step returns 1, with its completion; 2 = a new picture
 * to present, 0 = nothing new); 0 = it cannot be
 * opened (the host completes the request as before). While one plays, the
 * window's input goes to rlw_audio_ui_movie_message (Escape key-up ends it; the
 * mouse does not) and present() to rlw_audio_ui_movie_present. */
int rlw_audio_ui_movie_begin(const char *dados, const RlShellEvent *e);
int rlw_audio_ui_movie_active(void);
int rlw_audio_ui_movie_step(uint32_t *completion);
int rlw_audio_ui_movie_message(UINT msg, WPARAM wp);
int rlw_audio_ui_movie_present(HDC dc, const RECT *client, const RECT *view);
/* Movies, credits, quitting: stops the CD music (NativeCDMusicHost.leaveStage). */
void rlw_audio_ui_leave_stage(void);
/* After a successful load + rlw_driver_load_resume (rlw_main.c load_slot). */
void rlw_audio_ui_loaded(RlwDriver *d);
/* Every menu-loop pass while a shell session is open (the credits stop the CD music). */
void rlw_audio_ui_menu_tick(RlwDriver *d);
/* --smoke-audio (CI): opens the sound host without game data, plays a synthetic
 * tone and prints one line; 0 with or without an audio device. */
int rlw_audio_ui_smoke(void);
/* The "Áudio" popup appended to the menu bar; commands return 1 when handled. */
void rlw_audio_ui_menu(HMENU menubar);
int rlw_audio_ui_command(unsigned id);
/* The four opcoes.ini lines of the current settings (rlw_audio_settings_format). */
size_t rlw_audio_ui_ini(char *out, size_t capacity);
/* The host (NULL before open), for the event FLC's per-frame sound ticks. */
RlwAudio *rlw_audio_ui_audio(void);
#endif
