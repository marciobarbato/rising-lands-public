/* The game's original icon (the pumpkin), portable C (unit-tested on any host).
 * Nothing of it is in the repository: at run time it is read from the player's
 * imported RISING.EXE (its first RT_GROUP_ICON and the RT_ICON images it names,
 * from the PE resource directory) and written as a standard .ico file in the
 * player's data folder. rlw_icon_win.c puts it on the window and on the
 * Start-menu shortcut. */
#ifndef RLW_ICON_H
#define RLW_ICON_H
#include <stddef.h>
#include <stdint.h>

/* .ico bytes (ICONDIR, entries, images) of the first icon group of a PE image.
 * 0 and *out (caller frees), or -1 when the image is not a PE or has no icon. */
int rlw_icon_from_pe(const uint8_t *pe, size_t size, uint8_t **out, size_t *out_size);
/* <data_root>/rising.ico, built from <data_root>/Dados/RISING.EXE when absent.
 * 0 with the path (and *created = 1 when this call wrote it), -1 when there is
 * no icon to show (missing import, no icon group, write failure). */
int rlw_icon_prepare(const char *data_root, char *ico_path, size_t capacity, int *created);
#endif
