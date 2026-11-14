/* Sound import from the player's CD (front 2, audio), portable C.
 *
 * Complements rlw_cd_import (which copies the game, menu, briefing and movie
 * files): copies INSTALHD/SOUND/{A,B,P}.{HED,SND} unchanged into
 * <data_root>/Dados/sound, each checked against its size and SHA-256
 * (RLW_SOUND_FILES), through a staging folder renamed in only when all six
 * verified. A failure keeps the game playable without sound. Nothing from the
 * CD goes into the repository. */
#ifndef RLW_SOUND_IMPORT_H
#define RLW_SOUND_IMPORT_H
#include <stddef.h>

/* source: the same .cue/.bin/.iso/folder given to rlw_cd_import. Returns 0, or
 * nonzero with a PT-BR message. */
int rlw_sound_import(const char *source, const char *data_root, char *error, size_t capacity);
/* 1 when <dados>/sound holds the six bank files. */
int rlw_sound_installed(const char *dados);
#ifdef RLW_TESTING
extern int rlw_sound_test_ignore_pins; /* synthetic banks in unit tests */
#endif
#endif
