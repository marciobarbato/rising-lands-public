/* Portable unit tests for the Windows host's CD reader/importer and SHA-256.
 * Builds its own synthetic ISO 9660 image (no game data): plain ISO, raw
 * 2,352-byte MODE1 (through a .CUE) and MODE2/form1 images, and a folder source.
 * Run: windows/build.sh test  (or compile with -DRLW_TESTING, see build.sh). */
#include "rlw_cd.h"
#include "rlw_fs.h"
#include "rlw_sha256.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern int rlw_cd_test_ignore_pins;
static int failures = 0;
#define CHECK(cond) do { if (!(cond)) { fprintf(stderr, "FAIL %s:%d: %s\n", __FILE__, __LINE__, #cond); failures++; } } while (0)

typedef struct { const char *dir, *name; uint32_t size; } Spec;
static const Spec FILES[] = {
 {"INSTALHD", "RISING.EXE", 5000}, {"INSTALHD", "RISING.INI", 100}, {"INSTALHD", "RISING.RUL", 200},
 {"INSTALHD", "RISING.SBL", 300}, {"INSTALHD", "TABLEAU.COL", 400}, {"INSTALHD", "COULEUR.MAP", 500},
 {"INSTALHD", "PLATGAME.PIE", 2100}, {"INSTALHD", "MONSTRE.INI", 60}, {"INSTALHD", "TEXTES.TXT", 700},
 {"INSTALHD", "EVENTS.INI", 80},
 {"INSTALHD/GRAPHICS", "BLOCS.CMP", 4097}, {"INSTALHD/GRAPHICS", "TROOPS.CMP", 10}, {"INSTALHD/GRAPHICS", "VEHICULE.CMP", 11},
 {"INSTALHD/GRAPHICS", "CREATURE.CMP", 12}, {"INSTALHD/GRAPHICS", "BUILDING.CMP", 13}, {"INSTALHD/GRAPHICS", "OTHERS.CMP", 14},
 {"WORLD", "MONDE01.ETA", 3000}, {"WORLD", "MONDE02.ETA", 1500}, {"WORLD", "LEIAME.TXT", 9},
 {"EVENTS", "LABO1.FLC", 2500},
 /* Milestone 2: menu banks, backgrounds, a briefing and a movie. */
 {"INSTALHD/GRAPHICS", "MENU.CMP", 20}, {"INSTALHD/GRAPHICS", "INVENTE.CMP", 21}, {"INSTALHD", "TITRE.PIE", 2200},
 {"INSTALHD", "PAGEMIS.PIE", 30}, {"INSTALHD", "PAGEMIS2.PIE", 31}, {"INSTALHD", "CREDITS.PIE", 32},
 {"INSTALHD", "ECRANFIN.LZS", 33}, {"WORLD", "MONDE01.TXT", 44}, {"EVENTS", "INTROFR.AVI", 5000},
};
#define NFILES (sizeof(FILES) / sizeof(FILES[0]))
static const char *DIRS[] = {"", "INSTALHD", "INSTALHD/GRAPHICS", "WORLD", "EVENTS"};
#define NDIRS 5

static uint8_t content_byte(const Spec *s, uint32_t i) {
 uint32_t h = 2166136261u; for (const char *p = s->name; *p; p++) h = (h ^ (uint8_t)*p) * 16777619u;
 return (uint8_t)((h >> (i % 24)) + i * 31);
}
static uint8_t *content(const Spec *s) {
 uint8_t *p = malloc(s->size); for (uint32_t i = 0; i < s->size; i++) p[i] = content_byte(s, i); return p;
}

static void put_both32(uint8_t *p, uint32_t v) { for (int i = 0; i < 4; i++) { p[i] = (uint8_t)(v >> (8 * i)); p[7 - i] = (uint8_t)(v >> (8 * i)); } }
static uint32_t record(uint8_t *out, uint32_t lba, uint32_t size, int dir, const char *name, uint32_t name_length) {
 uint32_t length = 33 + name_length; if (length & 1) length++;
 memset(out, 0, length); out[0] = (uint8_t)length; put_both32(out + 2, lba); put_both32(out + 10, size);
 out[25] = dir ? 2 : 0; out[28] = 1; out[31] = 1; out[32] = (uint8_t)name_length; memcpy(out + 33, name, name_length);
 return length;
}

/* Sector plan: 16 PVD, 17 terminator, 18..22 directories, files from 23. */
static uint8_t *build_iso(uint32_t *sectors_out) {
 uint32_t file_lba[NFILES], next = 18 + NDIRS;
 for (unsigned i = 0; i < NFILES; i++) { file_lba[i] = next; next += (FILES[i].size + 2047) / 2048; }
 uint8_t *iso = calloc(next, 2048);
 uint8_t *pvd = iso + 16 * 2048; pvd[0] = 1; memcpy(pvd + 1, "CD001", 5); pvd[6] = 1;
 put_both32(pvd + 80, next);
 record(pvd + 156, 18, 2048, 1, "\0", 1);
 uint8_t *term = iso + 17 * 2048; term[0] = 255; memcpy(term + 1, "CD001", 5); term[6] = 1;
 for (unsigned d = 0; d < NDIRS; d++) {
  uint8_t *p = iso + (18 + d) * 2048; uint32_t off = 0;
  /* parent: the longest proper prefix directory (root for top level) */
  unsigned parent = 0; const char *slash = strrchr(DIRS[d], '/');
  if (slash) for (unsigned k = 0; k < NDIRS; k++) if (strlen(DIRS[k]) == (size_t)(slash - DIRS[d]) && !strncmp(DIRS[k], DIRS[d], strlen(DIRS[k]))) parent = k;
  off += record(p + off, 18 + d, 2048, 1, "\0", 1);
  off += record(p + off, 18 + parent, 2048, 1, "\1", 1);
  for (unsigned k = 1; k < NDIRS; k++) {
   const char *ks = strrchr(DIRS[k], '/'); size_t plen = ks ? (size_t)(ks - DIRS[k]) : 0;
   if (plen != strlen(DIRS[d]) || strncmp(DIRS[k], DIRS[d], plen)) continue;
   const char *leaf = ks ? ks + 1 : DIRS[k];
   off += record(p + off, 18 + k, 2048, 1, leaf, (uint32_t)strlen(leaf));
  }
  for (unsigned i = 0; i < NFILES; i++) {
   if (strcmp(FILES[i].dir, DIRS[d])) continue;
   char name[64]; snprintf(name, sizeof(name), "%s;1", FILES[i].name);
   off += record(p + off, file_lba[i], FILES[i].size, 0, name, (uint32_t)strlen(name));
  }
 }
 for (unsigned i = 0; i < NFILES; i++) { uint8_t *c = content(&FILES[i]); memcpy(iso + (size_t)file_lba[i] * 2048, c, FILES[i].size); free(c); }
 *sectors_out = next; return iso;
}

static void write_raw(const char *path, const uint8_t *iso, uint32_t sectors, uint32_t header) {
 FILE *f = rlw_fopen(path, "wb"); uint8_t sector[2352];
 for (uint32_t s = 0; s < sectors; s++) {
  memset(sector, 0, sizeof(sector)); memset(sector + 1, 0xff, 10); sector[15] = header == 16 ? 1 : 2;
  memcpy(sector + header, iso + (size_t)s * 2048, 2048); fwrite(sector, 1, 2352, f);
 }
 fclose(f);
}

static int lower_mixed(char *s) { for (int i = 1; s[i]; i++) if (s[i] >= 'A' && s[i] <= 'Z') s[i] = (char)(s[i] + 32); return 0; }

static void make_folder(const char *root) {
 for (unsigned d = 1; d < NDIRS; d++) {
  char rel[64]; snprintf(rel, sizeof(rel), "%s", DIRS[d]); lower_mixed(rel); /* "Instalhd/graphics" */
  for (char *c = rel; *c; c++) if (*c == '/') { c[1] = (char)(c[1] - 32); }
  char full[1024]; snprintf(full, sizeof(full), "%s/%s", root, rel); rlw_mkdirs(full);
 }
 for (unsigned i = 0; i < NFILES; i++) {
  char rel[64], name[32]; snprintf(rel, sizeof(rel), "%s", FILES[i].dir); lower_mixed(rel);
  for (char *c = rel; *c; c++) if (*c == '/') { c[1] = (char)(c[1] - 32); }
  snprintf(name, sizeof(name), "%s", FILES[i].name); lower_mixed(name);
  char full[1024]; snprintf(full, sizeof(full), "%s/%s/%s", root, rel, name);
  uint8_t *c = content(&FILES[i]); CHECK(rlw_write_file(full, c, FILES[i].size) == 0); free(c);
 }
}

typedef struct { int count; int saw_monde02; } ListResult;
static int count_files(const char *name, uint32_t size, void *ctx) {
 ListResult *r = ctx; r->count++; if (!strcmp(name, "MONDE02.ETA") && size == 1500) r->saw_monde02 = 1; return 0;
}

static void check_source(const char *path, int raw) {
 char error[512] = {0};
 RlwCd *cd = rlw_cd_open(path, error, sizeof(error));
 CHECK(cd != NULL); if (!cd) { fprintf(stderr, "  %s: %s\n", path, error); return; }
 CHECK(rlw_cd_is_raw(cd) == raw);
 for (unsigned i = 0; i < NFILES; i++) {
  char p[128]; snprintf(p, sizeof(p), "%s/%s", FILES[i].dir, FILES[i].name);
  uint32_t n = 0; uint8_t *data = rlw_cd_read(cd, p, &n), *want = content(&FILES[i]);
  CHECK(data && n == FILES[i].size && !memcmp(data, want, n));
  free(data); free(want);
 }
 uint32_t n = 0; uint8_t *lower = rlw_cd_read(cd, "instalhd/graphics/blocs.cmp", &n);
 CHECK(lower && n == 4097); free(lower);
 CHECK(rlw_cd_read(cd, "INSTALHD/NADA.BIN", &n) == NULL);
 CHECK(rlw_cd_read(cd, "INSTALHD", &n) == NULL);
 ListResult r = {0, 0}; CHECK(rlw_cd_list_files(cd, "WORLD", count_files, &r) == 0);
 CHECK(r.count == 4 && r.saw_monde02);
 rlw_cd_close(cd);
}

static int file_equals(const char *path, const Spec *s) {
 uint32_t n = 0; uint8_t *data = rlw_read_file(path, &n, 1u << 24), *want = content(s);
 int ok = data && n == s->size && !memcmp(data, want, n); free(data); free(want); return ok;
}

int main(int argc, char **argv) {
 const char *work = argc > 1 ? argv[1] : "test-work";
 rlw_remove_tree(work); CHECK(rlw_mkdirs(work) == 0);
 /* SHA-256 vectors (FIPS 180-2). */
 uint8_t d[32]; char hex[65];
 rlw_sha256("abc", 3, d); rlw_sha256_hex(d, hex); CHECK(!strcmp(hex, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
 rlw_sha256("", 0, d); rlw_sha256_hex(d, hex); CHECK(!strcmp(hex, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
 const char *v3 = "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
 rlw_sha256(v3, strlen(v3), d); rlw_sha256_hex(d, hex); CHECK(!strcmp(hex, "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"));
 RlwSha256 c; rlw_sha256_init(&c); for (int i = 0; i < 1000000; i++) rlw_sha256_update(&c, "a", 1); rlw_sha256_final(&c, d);
 rlw_sha256_hex(d, hex); CHECK(!strcmp(hex, "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"));

 uint32_t sectors = 0; uint8_t *iso = build_iso(&sectors);
 char iso_path[1024], bin1[1024], bin2[1024], cue[1024], folder[1024], junk[1024];
 snprintf(iso_path, sizeof(iso_path), "%s/teste.iso", work); rlw_write_file(iso_path, iso, sectors * 2048);
 snprintf(bin1, sizeof(bin1), "%s/TESTE MODE1.BIN", work); write_raw(bin1, iso, sectors, 16);
 snprintf(bin2, sizeof(bin2), "%s/teste-mode2.bin", work); write_raw(bin2, iso, sectors, 24);
 snprintf(cue, sizeof(cue), "%s/teste.cue", work);
 const char *sheet = "FILE \"TESTE MODE1.BIN\" BINARY\r\n  TRACK 01 MODE1/2352\r\n    INDEX 01 00:00:00\r\n";
 rlw_write_file(cue, (const uint8_t *)sheet, (uint32_t)strlen(sheet));
 snprintf(folder, sizeof(folder), "%s/pasta-cd", work); make_folder(folder);
 snprintf(junk, sizeof(junk), "%s/nada.bin", work); rlw_write_file(junk, (const uint8_t *)"not a cd", 8);

 check_source(iso_path, 0);
 check_source(cue, 1);
 check_source(bin2, 1);
 check_source(folder, 0);

 char error[512] = {0};
 CHECK(rlw_cd_open(junk, error, sizeof(error)) == NULL && strstr(error, "Rising Lands"));
 error[0] = 0; CHECK(rlw_cd_open(work, error, sizeof(error)) == NULL && strstr(error, "INSTALHD"));

 /* Import with the pinned identities enforced: synthetic data must be refused. */
 char data_root[1024]; snprintf(data_root, sizeof(data_root), "%s/dados-usuario", work);
 error[0] = 0;
 CHECK(rlw_cd_import(cue, data_root, 0, NULL, NULL, error, sizeof(error)) != 0 && strstr(error, "RISING.EXE") && strstr(error, "SHA-256"));
 CHECK(!rlw_cd_installed(data_root));
 /* Whole-image check of a raw BIN against the Brazilian CD hash. */
 error[0] = 0;
 CHECK(rlw_cd_import(bin2, data_root, 1, NULL, NULL, error, sizeof(error)) != 0 && strstr(error, "imagem"));

 /* Successful import (pins bypassed for synthetic data). */
 rlw_cd_test_ignore_pins = 1;
 error[0] = 0;
 int rc = rlw_cd_import(cue, data_root, 0, NULL, NULL, error, sizeof(error));
 CHECK(rc == 0); if (rc) fprintf(stderr, "  import: %s\n", error);
 CHECK(rlw_cd_installed(data_root));
 char p[1024];
 snprintf(p, sizeof(p), "%s/Dados/asset0.bin", data_root); CHECK(file_equals(p, &FILES[10]));
 snprintf(p, sizeof(p), "%s/Dados/asset5.bin", data_root); CHECK(file_equals(p, &FILES[15]));
 snprintf(p, sizeof(p), "%s/Dados/asset6.bin", data_root); CHECK(file_equals(p, &FILES[6]));
 snprintf(p, sizeof(p), "%s/Dados/PLATGAME.PIE", data_root); CHECK(file_equals(p, &FILES[6]));
 snprintf(p, sizeof(p), "%s/Dados/EVENTS.INI", data_root); CHECK(file_equals(p, &FILES[9]));
 snprintf(p, sizeof(p), "%s/Dados/MONDE01.ETA", data_root); CHECK(file_equals(p, &FILES[16]));
 snprintf(p, sizeof(p), "%s/Dados/MONDE02.ETA", data_root); CHECK(file_equals(p, &FILES[17]));
 snprintf(p, sizeof(p), "%s/Dados/events/LABO1.FLC", data_root); CHECK(file_equals(p, &FILES[19]));
 snprintf(p, sizeof(p), "%s/Dados/LEIAME.TXT", data_root); CHECK(!rlw_exists(p));
 snprintf(p, sizeof(p), "%s/Dados/MENU.CMP", data_root); CHECK(file_equals(p, &FILES[20]));
 snprintf(p, sizeof(p), "%s/Dados/TITRE.PIE", data_root); CHECK(file_equals(p, &FILES[22]));
 snprintf(p, sizeof(p), "%s/Dados/ECRANFIN.LZS", data_root); CHECK(file_equals(p, &FILES[26]));
 snprintf(p, sizeof(p), "%s/Dados/MONDE01.TXT", data_root); CHECK(file_equals(p, &FILES[27]));
 snprintf(p, sizeof(p), "%s/Dados/movies/INTROFR.AVI", data_root); CHECK(file_equals(p, &FILES[28]));
 snprintf(p, sizeof(p), "%s/Dados/importacao.json", data_root);
 uint32_t n = 0; uint8_t *receipt = rlw_read_file(p, &n, 1u << 20);
 CHECK(receipt && strstr((char *)receipt, "rising-lands/windows-import/2") && strstr((char *)receipt, "\"count\": 29")); free(receipt);
 /* A failed re-import leaves the previous complete import in place. */
 rlw_cd_test_ignore_pins = 0; error[0] = 0;
 CHECK(rlw_cd_import(folder, data_root, 0, NULL, NULL, error, sizeof(error)) != 0);
 CHECK(rlw_cd_installed(data_root));
 /* A re-import from the folder source replaces it atomically. */
 rlw_cd_test_ignore_pins = 1; error[0] = 0;
 CHECK(rlw_cd_import(folder, data_root, 0, NULL, NULL, error, sizeof(error)) == 0);
 CHECK(rlw_cd_installed(data_root));

 free(iso);
 if (failures) { fprintf(stderr, "%d falha(s)\n", failures); return 1; }
 printf("test_cd ok: SHA-256 vectors, ISO, BIN MODE1 via CUE, BIN MODE2, folder, import (menus, briefings, movies)/refusals\n");
 rlw_remove_tree(work);
 return 0;
}
