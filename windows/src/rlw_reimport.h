/* Jogo > Importar o CD novamente… (port of the macOS player's reimportCD),
 * portable part (unit-tested on any host). The running game cannot replace
 * Dados safely (the sound host, menus and movies read files there, and Windows
 * cannot remove an open file), so the CD is chosen in the running game, which
 * then closes and starts itself again with `--cd <image> --reimport`: the
 * ordinary first-start import (game files, sound banks, CD music, then the
 * icon) runs before anything opens a file in Dados. A failed re-import keeps
 * the installed data. */
#ifndef RLW_REIMPORT_H
#define RLW_REIMPORT_H
#include <stddef.h>

/* "\"<exe>\" --cd \"<source>\" --reimport" (UTF-8). 0, or -1 when it does not
 * fit or a path holds a quote (Windows paths cannot). */
int rlw_reimport_command_line(const char *exe, const char *source, char *out, size_t capacity);
/* The one-time startup notice "Para ouvir sons e música, use Jogo → Importar o
 * CD novamente…": due when <data_root>/Dados exists, the sound banks or the CD
 * music are missing and the notice was not shown before. */
int rlw_reimport_notice_due(const char *data_root, int sound_installed, int music_installed);
/* Records that the notice was shown (<data_root>/aviso-reimportar). 0 or -1. */
int rlw_reimport_notice_done(const char *data_root);
extern const char RLW_REIMPORT_NOTICE[];
#endif
