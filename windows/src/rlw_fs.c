#include "rlw_fs.h"
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#include <shlobj.h>
static wchar_t *wide(const char *s) {
 int n = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s, -1, NULL, 0);
 if (n <= 0) return NULL;
 wchar_t *w = malloc((size_t)n * sizeof(wchar_t));
 if (w && MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, s, -1, w, n) != n) { free(w); w = NULL; }
 return w;
}
static int narrow(const wchar_t *w, char *out, size_t capacity) {
 int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, out, (int)capacity, NULL, NULL);
 return n > 0 ? 0 : -1;
}
#else
#include <dirent.h>
#include <errno.h>
#include <sys/stat.h>
#include <unistd.h>
#endif

FILE *rlw_fopen(const char *path, const char *mode) {
#ifdef _WIN32
 wchar_t *p = wide(path), *m = wide(mode);
 FILE *f = p && m ? _wfopen(p, m) : NULL;
 free(p); free(m); return f;
#else
 return fopen(path, mode);
#endif
}

int rlw_seek(FILE *f, uint64_t offset) {
#ifdef _WIN32
 return _fseeki64(f, (long long)offset, SEEK_SET);
#else
 return fseeko(f, (off_t)offset, SEEK_SET);
#endif
}

int64_t rlw_file_size(FILE *f) {
#ifdef _WIN32
 if (_fseeki64(f, 0, SEEK_END)) return -1;
 long long n = _ftelli64(f);
#else
 if (fseeko(f, 0, SEEK_END)) return -1;
 long long n = (long long)ftello(f);
#endif
 if (n < 0 || rlw_seek(f, 0)) return -1;
 return n;
}

uint8_t *rlw_read_file(const char *path, uint32_t *length, uint32_t limit) {
 FILE *f = rlw_fopen(path, "rb"); if (!f) return NULL;
 int64_t n = rlw_file_size(f);
 if (n <= 0 || (uint64_t)n > limit) { fclose(f); return NULL; }
 uint8_t *p = malloc((size_t)n + 1); /* NUL after the data; length excludes it */
 if (p && fread(p, 1, (size_t)n, f) != (size_t)n) { free(p); p = NULL; }
 if (p) p[n] = 0;
 fclose(f);
 if (p) *length = (uint32_t)n;
 return p;
}

int rlw_write_file(const char *path, const uint8_t *data, uint32_t length) {
 FILE *f = rlw_fopen(path, "wb"); if (!f) return -1;
 int bad = (length && fwrite(data, 1, length, f) != length) || fflush(f);
 if (fclose(f)) bad = 1;
 return bad ? -1 : 0;
}

int rlw_is_dir(const char *path) {
#ifdef _WIN32
 wchar_t *p = wide(path); if (!p) return 0;
 DWORD a = GetFileAttributesW(p); free(p);
 return a != INVALID_FILE_ATTRIBUTES && (a & FILE_ATTRIBUTE_DIRECTORY);
#else
 struct stat s; return stat(path, &s) == 0 && S_ISDIR(s.st_mode);
#endif
}

int rlw_exists(const char *path) {
#ifdef _WIN32
 wchar_t *p = wide(path); if (!p) return 0;
 DWORD a = GetFileAttributesW(p); free(p); return a != INVALID_FILE_ATTRIBUTES;
#else
 struct stat s; return stat(path, &s) == 0;
#endif
}

int rlw_mkdir(const char *path) {
#ifdef _WIN32
 wchar_t *p = wide(path); if (!p) return -1;
 BOOL ok = CreateDirectoryW(p, NULL); free(p);
 return ok || rlw_is_dir(path) ? 0 : -1;
#else
 return mkdir(path, 0755) == 0 || rlw_is_dir(path) ? 0 : -1;
#endif
}

int rlw_mkdirs(const char *path) {
 char buffer[4096]; size_t n = strlen(path);
 if (n == 0 || n >= sizeof(buffer)) return -1;
 memcpy(buffer, path, n + 1);
 for (size_t i = 1; i < n; i++) {
  if (buffer[i] == '/' || buffer[i] == '\\') {
   if (i == 2 && buffer[1] == ':') continue; /* drive root C:\ */
   char c = buffer[i]; buffer[i] = 0;
   if (!rlw_is_dir(buffer) && rlw_mkdir(buffer)) return -1;
   buffer[i] = c;
  }
 }
 return rlw_mkdir(buffer);
}

int rlw_list_dir(const char *path, int (*fn)(const char *, int, void *), void *ctx) {
#ifdef _WIN32
 char pattern[4096]; snprintf(pattern, sizeof(pattern), "%s\\*", path);
 wchar_t *p = wide(pattern); if (!p) return -1;
 WIN32_FIND_DATAW data; HANDLE h = FindFirstFileW(p, &data); free(p);
 if (h == INVALID_HANDLE_VALUE) return -1;
 int rc = 0;
 do {
  if (!wcscmp(data.cFileName, L".") || !wcscmp(data.cFileName, L"..")) continue;
  char name[1024]; if (narrow(data.cFileName, name, sizeof(name))) continue;
  if (fn(name, (data.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0, ctx)) { rc = 1; break; }
 } while (FindNextFileW(h, &data));
 FindClose(h); return rc;
#else
 DIR *d = opendir(path); if (!d) return -1;
 struct dirent *e; int rc = 0;
 while ((e = readdir(d))) {
  if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
  char full[4096]; snprintf(full, sizeof(full), "%s/%s", path, e->d_name);
  if (fn(e->d_name, rlw_is_dir(full), ctx)) { rc = 1; break; }
 }
 closedir(d); return rc;
#endif
}

typedef struct { const char *base; int failed; } RemoveCtx;
static int remove_entry(const char *name, int is_dir, void *ctx) {
 RemoveCtx *r = ctx; char full[4096];
 snprintf(full, sizeof(full), "%s" RLW_SEP "%s", r->base, name);
 if (is_dir) { if (rlw_remove_tree(full)) r->failed = 1; return 0; }
#ifdef _WIN32
 wchar_t *p = wide(full); if (!p || !DeleteFileW(p)) r->failed = 1; free(p);
#else
 if (unlink(full)) r->failed = 1;
#endif
 return 0;
}

int rlw_remove_tree(const char *path) {
 if (!rlw_exists(path)) return 0;
 RemoveCtx r = {path, 0};
 if (rlw_list_dir(path, remove_entry, &r) < 0) return -1;
#ifdef _WIN32
 wchar_t *p = wide(path); BOOL ok = p && RemoveDirectoryW(p); free(p);
 return r.failed || !ok ? -1 : 0;
#else
 return r.failed || rmdir(path) ? -1 : 0;
#endif
}

int rlw_rename_replace_dir(const char *from, const char *to) {
 if (rlw_exists(to) && rlw_remove_tree(to)) return -1;
#ifdef _WIN32
 wchar_t *a = wide(from), *b = wide(to);
 BOOL ok = a && b && MoveFileExW(a, b, MOVEFILE_WRITE_THROUGH);
 free(a); free(b); return ok ? 0 : -1;
#else
 return rename(from, to);
#endif
}

int rlw_user_data_dir(char *out, size_t capacity) {
#ifdef _WIN32
 PWSTR base = NULL;
 if (SHGetKnownFolderPath(&FOLDERID_LocalAppData, KF_FLAG_CREATE, NULL, &base) != S_OK) return -1;
 char narrowed[2048]; int rc = narrow(base, narrowed, sizeof(narrowed)); CoTaskMemFree(base);
 if (rc) return -1;
 return snprintf(out, capacity, "%s\\RisingLands-ptbr", narrowed) >= (int)capacity ? -1 : 0;
#else
 const char *xdg = getenv("XDG_DATA_HOME"), *home = getenv("HOME");
 int n = xdg && *xdg ? snprintf(out, capacity, "%s/RisingLands-ptbr", xdg)
                     : snprintf(out, capacity, "%s/.local/share/RisingLands-ptbr", home ? home : ".");
 return n >= (int)capacity ? -1 : 0;
#endif
}
