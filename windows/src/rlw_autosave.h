/* Periodic autosave (Partida > Salvamento automático), portable C, port of the
 * macOS NativeAutosaveInterval and NativeAutosaveSchedule
 * (macos/src/NativePlayerOptions.swift). Host only: the
 * ordinary save of the game in progress goes to its own slot "autosave"
 * (autosave.rls + its .view companion), never to the player's positions or the
 * stage-start autosave. Off by default, as in the original game.
 *
 * Game time, not wall time: one game minute is 1800 frames (60 s at the 1x pace
 * of 30 frames/s), so the interval follows the chosen speed and a paused game
 * (no frames) counts nothing. */
#ifndef RLW_AUTOSAVE_H
#define RLW_AUTOSAVE_H
#include <stddef.h>
#include <stdint.h>

#define RLW_AUTOSAVE_SLOT "autosave"
#define RLW_AUTOSAVE_FRAMES_PER_MINUTE 1800u
/* The menu's choices, in order: off, 5, 10, 15 minutes. */
extern const unsigned RLW_AUTOSAVE_CHOICES[4];

/* 1 for 0, 5, 10 or 15 (opcoes.ini "autosave=<minutes>"). */
int rlw_autosave_valid_minutes(unsigned minutes);
uint64_t rlw_autosave_frames(unsigned minutes);
/* One opcoes.ini line: 1 and *minutes for a valid "autosave=<0|5|10|15>",
 * 0 otherwise (an unknown value keeps the current choice, normally off). */
int rlw_autosave_option(const char *line, unsigned *minutes);
/* "Desligado" or "A cada N minutos" (UTF-8). */
void rlw_autosave_title(unsigned minutes, char *out, size_t capacity);
/* Carregar partida's item: "Salvamento automático · dd/MM HH:mm" or "… · vazio". */
void rlw_autosave_caption(const char *directory, char *out, size_t capacity);

/* When the next autosave is due. frame() is called once per completed game
 * frame and counts only eligible frames (the game in progress: not menus,
 * briefing, pause, result, a stop). Once interval frames have passed the save
 * is due; it stays due, without counting further, until saved() or failed()
 * (an error waits one more full interval). A busy boundary (input still queued
 * for the next frame) retries at the next eligible frame. restart() runs at
 * every stage start and load, so a fresh game never autosaves at once. */
typedef struct { uint64_t interval_frames, elapsed; } RlwAutosave;
void rlw_autosave_init(RlwAutosave *a, uint64_t interval_frames);
int rlw_autosave_enabled(const RlwAutosave *a);
int rlw_autosave_due(const RlwAutosave *a);
/* Returns 1 when the autosave should be attempted at this frame boundary. */
int rlw_autosave_frame(RlwAutosave *a, int eligible);
/* A new interval keeps the frames already played (a shorter one may be due at
 * the next eligible frame); 0 turns it off and forgets them. */
void rlw_autosave_set_interval(RlwAutosave *a, uint64_t interval_frames);
void rlw_autosave_saved(RlwAutosave *a);
void rlw_autosave_failed(RlwAutosave *a);
void rlw_autosave_restart(RlwAutosave *a);
#endif
