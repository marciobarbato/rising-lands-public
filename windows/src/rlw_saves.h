/* The player's save folder, portable C (no Win32; unit-tested on any host).
 * Same layout as the macOS app (NativeLiveWindow.saveDirectory), so the folder
 * can be copied between the two: posicao-1..10.rls (named positions; the
 * original lists show them as save1.sav..save9.sav, save0.sav), rapido.rls
 * (quick save), inicio-fase.rls (stage-start autosave), each with its
 * "<slot>.<sha256>.view" companion, and nomes.plist (the position names, an XML
 * property list {"1": "name", ...}). The host never reads game state here. */
#ifndef RLW_SAVES_H
#define RLW_SAVES_H
#include <stddef.h>
#include <stdint.h>

#define RLW_SAVE_NAME_CAPACITY 256 /* UTF-8 bytes, NUL included (80 characters at most) */
typedef struct { char name[11][RLW_SAVE_NAME_CAPACITY]; } RlwSaveNames; /* index 1..10; "" = unnamed */

/* "<directory><sep><slot>.rls". Returns 0, or -1 when it does not fit. */
int rlw_saves_path(const char *directory, const char *slot, char *out, size_t capacity);
/* Reads nomes.plist (absent or unreadable: all unnamed). */
void rlw_saves_names_read(const char *directory, RlwSaveNames *names);
/* Writes nomes.plist durably. Returns 0 or -1. */
int rlw_saves_names_write(const char *directory, const RlwSaveNames *names);
/* publishSlots(): 10 x [present 0/1, 32-byte CP1252 caption "<name> · dd/MM HH:mm"]
 * for posicao-1..10 (unnamed: "Posição N"), from the slot files' local times. */
void rlw_saves_caption_table(const char *directory, uint8_t table[330]);
/* Menu title of position n (1..10): "<name> · dd/MM/aaaa HH:mm" or "<name> · vazia". */
void rlw_saves_title(const char *directory, unsigned n, char *out, size_t capacity);
/* UTF-8 -> CP1252 (characters without a CP1252 code become '?'); writes at
 * most `capacity` bytes, no terminator. Returns the length written. */
size_t rlw_utf8_to_cp1252(const char *text, uint8_t *out, size_t capacity);
#endif
