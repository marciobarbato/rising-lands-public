#include "rlw_saves.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

int rlw_saves_path(const char *directory, const char *slot, char *out, size_t capacity) {
 return snprintf(out, capacity, "%s" RLW_SEP "%s.rls", directory, slot) >= (int)capacity ? -1 : 0;
}

/* ---- nomes.plist: the subset PropertyListSerialization writes for a
 * [String: String] dictionary (keys "1".."10"). ---- */
static void unescape(const char *s, size_t n, char *out, size_t capacity) {
 static const struct { const char *entity; char c; } table[] = {{"&amp;", '&'}, {"&lt;", '<'}, {"&gt;", '>'}, {"&quot;", '"'}, {"&apos;", '\''}};
 size_t k = 0;
 for (size_t i = 0; i < n && k + 1 < capacity;) {
  int done = 0;
  if (s[i] == '&') for (unsigned e = 0; e < 5 && !done; e++) {
   size_t m = strlen(table[e].entity);
   if (i + m <= n && !memcmp(s + i, table[e].entity, m)) { out[k++] = table[e].c; i += m; done = 1; }
  }
  if (!done) out[k++] = s[i++];
 }
 out[k] = 0;
}
void rlw_saves_names_read(const char *directory, RlwSaveNames *names) {
 memset(names, 0, sizeof(*names));
 char path[4200]; if (snprintf(path, sizeof(path), "%s" RLW_SEP "nomes.plist", directory) >= (int)sizeof(path)) return;
 uint32_t n = 0; char *text = (char *)rlw_read_file(path, &n, 1024 * 1024); if (!text) return;
 const char *p = text;
 while ((p = strstr(p, "<key>"))) {
  p += 5; const char *end = strstr(p, "</key>"); if (!end) break;
  char key[8]; unescape(p, (size_t)(end - p), key, sizeof(key)); p = end + 6;
  while (*p == ' ' || *p == '\t' || *p == '\r' || *p == '\n') p++;
  char value[RLW_SAVE_NAME_CAPACITY] = "";
  if (!strncmp(p, "<string/>", 9)) p += 9;
  else if (!strncmp(p, "<string>", 8)) {
   p += 8; end = strstr(p, "</string>"); if (!end) break;
   unescape(p, (size_t)(end - p), value, sizeof(value)); p = end + 9;
  } else continue;
  char *tail = NULL; long slot = strtol(key, &tail, 10);
  if (tail && !*tail && slot >= 1 && slot <= 10) snprintf(names->name[slot], sizeof(names->name[slot]), "%s", value);
 }
 free(text);
}
static size_t escape(const char *s, char *out, size_t capacity) {
 size_t k = 0;
 for (; *s; s++) {
  const char *e = *s == '&' ? "&amp;" : *s == '<' ? "&lt;" : *s == '>' ? "&gt;" : NULL;
  size_t m = e ? strlen(e) : 1;
  if (k + m + 1 > capacity) break;
  if (e) { memcpy(out + k, e, m); k += m; } else out[k++] = *s;
 }
 out[k] = 0; return k;
}
int rlw_saves_names_write(const char *directory, const RlwSaveNames *names) {
 char path[4200]; if (snprintf(path, sizeof(path), "%s" RLW_SEP "nomes.plist", directory) >= (int)sizeof(path)) return -1;
 static char text[64 * 1024]; size_t k = 0;
 k += (size_t)snprintf(text + k, sizeof(text) - k, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
      "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n"
      "<plist version=\"1.0\">\n<dict>\n");
 /* Keys in the order PropertyListSerialization sorts them (as strings). */
 static const unsigned order[10] = {1, 10, 2, 3, 4, 5, 6, 7, 8, 9};
 for (unsigned i = 0; i < 10; i++) {
  unsigned slot = order[i]; if (!names->name[slot][0]) continue;
  char value[RLW_SAVE_NAME_CAPACITY * 6]; escape(names->name[slot], value, sizeof(value));
  k += (size_t)snprintf(text + k, sizeof(text) - k, "\t<key>%u</key>\n\t<string>%s</string>\n", slot, value);
 }
 k += (size_t)snprintf(text + k, sizeof(text) - k, "</dict>\n</plist>\n");
 if (k >= sizeof(text)) return -1;
 return rlw_durable_write(path, (const uint8_t *)text, k, NULL, 0);
}

/* ---- captions ---- */
size_t rlw_utf8_to_cp1252(const char *text, uint8_t *out, size_t capacity) {
 /* CP1252 0x80..0x9f. */
 static const uint16_t high[32] = {0x20ac, 0, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0, 0x017d, 0,
                                   0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0, 0x017e, 0x0178};
 const uint8_t *s = (const uint8_t *)text; size_t k = 0;
 while (*s && k < capacity) {
  uint32_t c = *s++; int extra = c >= 0xf0 ? 3 : c >= 0xe0 ? 2 : c >= 0xc0 ? 1 : 0;
  if (extra) { c &= 0x3fu >> extra; for (int i = 0; i < extra && (*s & 0xc0) == 0x80; i++) c = (c << 6) | (*s++ & 0x3fu); }
  uint8_t b = '?';
  if (c < 0x80 || (c >= 0xa0 && c <= 0xff)) b = (uint8_t)c;
  else for (unsigned i = 0; i < 32; i++) if (high[i] && high[i] == c) b = (uint8_t)(0x80 + i);
  out[k++] = b;
 }
 return k;
}
static int slot_time(const char *directory, unsigned n, struct tm *out) {
 char slot[32], path[4200]; snprintf(slot, sizeof(slot), "posicao-%u", n);
 if (rlw_saves_path(directory, slot, path, sizeof(path))) return 0;
 int64_t t = rlw_file_mtime(path); if (t < 0) return 0;
 time_t when = (time_t)t; struct tm *local = localtime(&when); if (!local) return 0;
 *out = *local; return 1;
}
static void slot_name(const RlwSaveNames *names, unsigned n, char *out, size_t capacity) {
 if (names->name[n][0]) snprintf(out, capacity, "%s", names->name[n]); else snprintf(out, capacity, "Posição %u", n);
}
void rlw_saves_caption_table(const char *directory, uint8_t table[330]) {
 RlwSaveNames names; rlw_saves_names_read(directory, &names);
 memset(table, 0, 330);
 for (unsigned slot = 0; slot < 10; slot++) {
  struct tm when; if (!slot_time(directory, slot + 1, &when)) continue;
  char name[RLW_SAVE_NAME_CAPACITY], caption[RLW_SAVE_NAME_CAPACITY + 32];
  slot_name(&names, slot + 1, name, sizeof(name));
  snprintf(caption, sizeof(caption), "%s · %02d/%02d %02d:%02d", name, when.tm_mday, when.tm_mon + 1, when.tm_hour, when.tm_min);
  table[33 * slot] = 1; rlw_utf8_to_cp1252(caption, table + 33 * slot + 1, 31);
 }
}
void rlw_saves_title(const char *directory, unsigned n, char *out, size_t capacity) {
 RlwSaveNames names; rlw_saves_names_read(directory, &names);
 char name[RLW_SAVE_NAME_CAPACITY]; slot_name(&names, n >= 1 && n <= 10 ? n : 1, name, sizeof(name));
 struct tm when;
 if (n >= 1 && n <= 10 && slot_time(directory, n, &when))
  snprintf(out, capacity, "%s · %02d/%02d/%04d %02d:%02d", name, when.tm_mday, when.tm_mon + 1, when.tm_year + 1900, when.tm_hour, when.tm_min);
 else snprintf(out, capacity, "%s · vazia", name);
}
