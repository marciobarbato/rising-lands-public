/* Portable checks of the save folder helpers (rlw_saves.c) and the durable
 * replace (rlw_fs.c), with the host compiler; no game data.
 *   test_saves <empty work folder> */
#include "rlw_fs.h"
#include "rlw_saves.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "test_saves: %s:%d: %s\n", __FILE__, __LINE__, #c); failures++; } } while (0)

typedef struct { unsigned files, temporaries; } Count;
static int count(const char *name, int is_dir, void *ctx) {
 Count *c = ctx; if (is_dir) return 0; c->files++; if (strstr(name, ".tmp.")) c->temporaries++; return 0;
}

int main(int argc, char **argv) {
 if (argc < 2) { fprintf(stderr, "usage: test_saves <work folder>\n"); return 64; }
 char dir[4096]; snprintf(dir, sizeof(dir), "%s" RLW_SEP "saves", argv[1]);
 rlw_remove_tree(dir); CHECK(rlw_mkdirs(dir) == 0);

 /* Durable replace: new content, no temporary left; a missing folder fails cleanly. */
 char path[4200]; CHECK(rlw_saves_path(dir, "posicao-2", path, sizeof(path)) == 0);
 CHECK(strstr(path, "posicao-2.rls") != NULL);
 CHECK(rlw_durable_write(path, (const uint8_t *)"abc", 3, (const uint8_t *)"de", 2) == 0);
 CHECK(rlw_durable_write(path, (const uint8_t *)"xyz", 3, NULL, 0) == 0);
 uint32_t n = 0; uint8_t *data = rlw_read_file(path, &n, 1024);
 CHECK(data && n == 3 && !memcmp(data, "xyz", 3)); free(data);
 char missing[4300]; snprintf(missing, sizeof(missing), "%s" RLW_SEP "absent" RLW_SEP "x.rls", dir);
 CHECK(rlw_durable_write(missing, (const uint8_t *)"a", 1, NULL, 0) != 0);
 CHECK(rlw_file_mtime(path) > 0); CHECK(rlw_file_mtime(missing) == -1);

 /* nomes.plist: round trip with escaping, the macOS key order, absent = unnamed. */
 RlwSaveNames names; rlw_saves_names_read(dir, &names);
 for (unsigned i = 1; i <= 10; i++) CHECK(names.name[i][0] == 0);
 snprintf(names.name[2], sizeof(names.name[2]), "Ilha <norte> & \xc3\xa9");
 snprintf(names.name[10], sizeof(names.name[10]), "D\xc3\xa9" "cima");
 CHECK(rlw_saves_names_write(dir, &names) == 0);
 RlwSaveNames back; rlw_saves_names_read(dir, &back);
 CHECK(!strcmp(back.name[2], names.name[2])); CHECK(!strcmp(back.name[10], names.name[10])); CHECK(back.name[1][0] == 0);
 char plist[4200]; snprintf(plist, sizeof(plist), "%s" RLW_SEP "nomes.plist", dir);
 char *text = (char *)rlw_read_file(plist, &n, 65536);
 CHECK(text && strstr(text, "<key>2</key>\n\t<string>Ilha &lt;norte&gt; &amp; \xc3\xa9</string>"));
 CHECK(text && strstr(text, "<key>10</key>") < strstr(text, "<key>2</key>")); /* string order, as PropertyListSerialization */
 free(text);

 /* Captions for the original lists: present slot 2 named, absent slots empty, CP1252. */
 uint8_t table[330]; memset(table, 0xee, sizeof(table)); rlw_saves_caption_table(dir, table);
 CHECK(table[0] == 0); CHECK(table[33] == 1); CHECK(table[33 * 9] == 0);
 CHECK(!memcmp(table + 34, "Ilha <norte> & \xe9 \xb7 ", 19)); CHECK(table[33 + 32] == 0);
 for (unsigned i = 0; i < 33; i++) CHECK(table[i] == 0);
 /* Unnamed: "Posição N"; 31 bytes at most. */
 RlwSaveNames none; memset(&none, 0, sizeof(none)); CHECK(rlw_saves_names_write(dir, &none) == 0);
 rlw_saves_caption_table(dir, table); CHECK(!memcmp(table + 34, "Posi\xe7\xe3o 2 \xb7 ", 12));
 char title[400]; rlw_saves_title(dir, 1, title, sizeof(title)); CHECK(!strcmp(title, "Posi\xc3\xa7\xc3\xa3o 1 \xc2\xb7 vazia"));
 rlw_saves_title(dir, 2, title, sizeof(title)); CHECK(!strncmp(title, "Posi\xc3\xa7\xc3\xa3o 2 \xc2\xb7 ", 15) && strlen(title) == 15 + 16);

 /* UTF-8 -> CP1252: euro and quotes from 0x80..0x9f, unknown characters '?', capacity kept. */
 uint8_t out[8]; size_t k = rlw_utf8_to_cp1252("\xe2\x82\xac\xe2\x80\x9c" "A\xe2\x98\x85", out, sizeof(out));
 CHECK(k == 4 && out[0] == 0x80 && out[1] == 0x93 && out[2] == 'A' && out[3] == '?');
 CHECK(rlw_utf8_to_cp1252("abcdefghij", out, 3) == 3);

 Count c = {0, 0}; rlw_list_dir(dir, count, &c); CHECK(c.temporaries == 0); CHECK(c.files == 2);
 rlw_remove_tree(dir);
 if (failures) return 1;
 printf("test_saves ok: durable replace, nomes.plist, original list captions, CP1252\n");
 return 0;
}
