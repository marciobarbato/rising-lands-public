/* UTF-8 file helpers for the Windows host (also build on POSIX for unit tests).
 * Paths are UTF-8; on Windows they are converted to UTF-16 for the wide APIs. */
#ifndef RLW_FS_H
#define RLW_FS_H
#include <stdint.h>
#include <stdio.h>
FILE *rlw_fopen(const char *path, const char *mode);
int rlw_seek(FILE *f, uint64_t offset);
int64_t rlw_file_size(FILE *f);
/* Whole file, at most `limit` bytes; NULL on error. Caller frees. */
uint8_t *rlw_read_file(const char *path, uint32_t *length, uint32_t limit);
int rlw_write_file(const char *path, const uint8_t *data, uint32_t length);
int rlw_mkdir(const char *path);              /* 0 when created or already a directory */
int rlw_mkdirs(const char *path);             /* mkdir -p */
int rlw_is_dir(const char *path);
int rlw_exists(const char *path);
int rlw_rename_replace_dir(const char *from, const char *to); /* removes `to` first */
int rlw_remove_tree(const char *path);
/* Lists direct children of a directory: calls fn(name, is_dir, ctx) per entry
 * (no "." or ".."). Returns 0 or -1. */
int rlw_list_dir(const char *path, int (*fn)(const char *, int, void *), void *ctx);
/* Per-user data folder: %LOCALAPPDATA%\RisingLands-ptbr (Windows) or
 * $XDG_DATA_HOME|~/.local/share/RisingLands-ptbr (POSIX tests). */
int rlw_user_data_dir(char *out, size_t capacity);
#ifdef _WIN32
#define RLW_SEP "\\"
#else
#define RLW_SEP "/"
#endif
#endif
