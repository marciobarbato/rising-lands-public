/* Host transport only: reads the player's CD; no original game data is bundled. */
#include "rlw_cd.h"
#include "rlw_fs.h"
#include "rlw_sha256.h"
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define MAX_CD_FILE (128u * 1024u * 1024u)

struct RlwCd {
 FILE *image;           /* NULL for a folder source */
 char *folder;
 char label[260];
 uint32_t sector_size, data_offset;
 uint32_t root_lba, root_length;
};

static uint32_t le32(const uint8_t *p) { return p[0] | (uint32_t)p[1] << 8 | (uint32_t)p[2] << 16 | (uint32_t)p[3] << 24; }

static void normalized(const char *in, size_t n, char *out, size_t capacity) {
 size_t k = 0;
 for (size_t i = 0; i < n && in[i] && in[i] != ';' && k + 1 < capacity; i++) out[k++] = (char)toupper((unsigned char)in[i]);
 while (k && out[k - 1] == '.') k--;
 out[k] = 0;
}

static const char *base_name(const char *path) {
 const char *a = strrchr(path, '/'), *b = strrchr(path, '\\'), *s = a > b ? a : b;
 return s ? s + 1 : path;
}

static int read_at(RlwCd *cd, uint64_t offset, void *out, size_t n) {
 return rlw_seek(cd->image, offset) == 0 && fread(out, 1, n, cd->image) == n ? 0 : -1;
}

static uint8_t *extent(RlwCd *cd, uint32_t lba, uint32_t length) {
 uint32_t sectors = (length + 2047) / 2048;
 uint8_t *out = malloc((size_t)sectors * 2048 + 1);
 if (!out) return NULL;
 for (uint32_t i = 0; i < sectors; i++)
  if (read_at(cd, (uint64_t)(lba + i) * cd->sector_size + cd->data_offset, out + (size_t)i * 2048, 2048)) { free(out); return NULL; }
 return out;
}

typedef struct { uint32_t lba, length; int dir; } Record;
/* Calls fn for every record of a directory extent; stop when fn returns 1. */
static int each_record(RlwCd *cd, uint32_t lba, uint32_t length, int (*fn)(const char *, Record, void *), void *ctx) {
 uint8_t *data = extent(cd, lba, length); if (!data) return -1;
 uint32_t offset = 0; int rc = 0;
 while (offset < length) {
  uint32_t size = data[offset];
  if (size == 0) { offset = (offset / 2048 + 1) * 2048; continue; }
  if (offset + size > length || size < 34) break;
  const uint8_t *r = data + offset; uint32_t name_length = r[32];
  if (name_length && 33 + name_length <= size && !(name_length == 1 && (r[33] == 0 || r[33] == 1))) {
   char name[256]; normalized((const char *)r + 33, name_length, name, sizeof(name));
   Record rec = {le32(r + 2), le32(r + 10), (r[25] & 2) != 0};
   if (fn(name, rec, ctx)) { rc = 1; break; }
  }
  offset += size;
 }
 free(data); return rc;
}

typedef struct { const char *want; Record found; int ok; } FindCtx;
static int find_record(const char *name, Record r, void *ctx) {
 FindCtx *f = ctx; if (strcmp(name, f->want)) return 0;
 f->found = r; f->ok = 1; return 1;
}

static int resolve_image(RlwCd *cd, const char *path, Record *out) {
 Record cur = {cd->root_lba, cd->root_length, 1};
 char component[256]; const char *p = path;
 while (*p) {
  const char *slash = strchr(p, '/'); size_t n = slash ? (size_t)(slash - p) : strlen(p);
  if (n) {
   if (!cur.dir) return -1;
   normalized(p, n, component, sizeof(component));
   FindCtx f = {component, {0, 0, 0}, 0};
   if (each_record(cd, cur.lba, cur.length, find_record, &f) < 0 || !f.ok) return -1;
   cur = f.found;
  }
  p += n; if (*p == '/') p++;
 }
 *out = cur; return 0;
}

typedef struct { const char *want; char match[1024]; int ok; } FolderFind;
static int folder_match(const char *name, int is_dir, void *ctx) {
 (void)is_dir; FolderFind *f = ctx; char a[256];
 normalized(name, strlen(name), a, sizeof(a));
 if (strcmp(a, f->want)) return 0;
 snprintf(f->match, sizeof(f->match), "%s", name); f->ok = 1; return 1;
}

static int resolve_folder(const char *root, const char *path, char *out, size_t capacity) {
 char current[4096]; snprintf(current, sizeof(current), "%s", root);
 char component[256]; const char *p = path;
 while (*p) {
  const char *slash = strchr(p, '/'); size_t n = slash ? (size_t)(slash - p) : strlen(p);
  if (n) {
   normalized(p, n, component, sizeof(component));
   FolderFind f = {component, {0}, 0};
   if (rlw_list_dir(current, folder_match, &f) < 0 || !f.ok) return -1;
   size_t used = strlen(current);
   if (snprintf(current + used, sizeof(current) - used, RLW_SEP "%s", f.match) >= (int)(sizeof(current) - used)) return -1;
  }
  p += n; if (*p == '/') p++;
 }
 return snprintf(out, capacity, "%s", current) >= (int)capacity ? -1 : 0;
}

static int has_suffix(const char *s, const char *suffix) {
 size_t a = strlen(s), b = strlen(suffix);
 if (a < b) return 0;
 for (size_t i = 0; i < b; i++) if (tolower((unsigned char)s[a - b + i]) != tolower((unsigned char)suffix[i])) return 0;
 return 1;
}

static void set_error(char *error, size_t capacity, const char *fmt, const char *arg) {
 if (error && capacity) snprintf(error, capacity, fmt, arg);
}

static RlwCd *open_image(const char *path, char *error, size_t capacity) {
 RlwCd *cd = calloc(1, sizeof(*cd)); if (!cd) return NULL;
 snprintf(cd->label, sizeof(cd->label), "%s", base_name(path));
 cd->image = rlw_fopen(path, "rb");
 static const uint32_t layouts[3][2] = {{2352, 24}, {2352, 16}, {2048, 0}};
 uint8_t pvd[2048];
 for (int i = 0; cd->image && i < 3; i++) {
  cd->sector_size = layouts[i][0]; cd->data_offset = layouts[i][1];
  if (read_at(cd, 16ull * cd->sector_size + cd->data_offset, pvd, sizeof(pvd)) == 0 && pvd[0] == 1 && !memcmp(pvd + 1, "CD001", 5)) {
   cd->root_lba = le32(pvd + 156 + 2); cd->root_length = le32(pvd + 156 + 10);
   return cd;
  }
 }
 set_error(error, capacity, "“%s” não parece ser o CD do Rising Lands. Escolha a imagem RISING_LANDS_BR_.BIN (ou o arquivo .CUE ou .ISO) ou a pasta com os arquivos do CD.", cd->label);
 rlw_cd_close(cd); return NULL;
}

RlwCd *rlw_cd_open(const char *path, char *error, size_t capacity) {
 if (!path || !*path) { set_error(error, capacity, "%s", "Nenhum CD escolhido."); return NULL; }
 if (rlw_is_dir(path)) {
  char probe[4096];
  if (resolve_folder(path, "INSTALHD", probe, sizeof(probe))) {
   set_error(error, capacity, "A pasta “%s” não contém os arquivos do CD (INSTALHD).", base_name(path));
   return NULL;
  }
  RlwCd *cd = calloc(1, sizeof(*cd)); if (!cd) return NULL;
  cd->folder = malloc(strlen(path) + 1); if (!cd->folder) { free(cd); return NULL; }
  strcpy(cd->folder, path);
  snprintf(cd->label, sizeof(cd->label), "%s", base_name(path));
  return cd;
 }
 if (has_suffix(path, ".cue")) {
  uint32_t n = 0; uint8_t *text = rlw_read_file(path, &n, 1u << 20);
  if (!text) { set_error(error, capacity, "Não foi possível ler “%s”.", base_name(path)); return NULL; }
  char name[1024] = {0};
  for (uint32_t i = 0; i < n && !name[0];) {
   uint32_t j = i; while (j < n && text[j] != '\n' && text[j] != '\r') j++;
   uint32_t k = i; while (k < j && (text[k] == ' ' || text[k] == '\t')) k++;
   if (j - k > 5 && !strncmp((const char *)text + k, "FILE ", 5)) {
    const char *first = memchr(text + k, '"', j - k), *last = NULL;
    for (uint32_t m = j; m > k; m--) if (text[m - 1] == '"') { last = (const char *)text + m - 1; break; }
    if (first && last && last > first && (size_t)(last - first - 1) < sizeof(name)) memcpy(name, first + 1, (size_t)(last - first - 1));
   }
   i = j; while (i < n && (text[i] == '\n' || text[i] == '\r')) i++;
  }
  free(text);
  if (!name[0] || strstr(name, "..")) { set_error(error, capacity, "“%s” não é uma folha .CUE válida.", base_name(path)); return NULL; }
  char image[4096]; size_t dir = (size_t)(base_name(path) - path);
  if (dir + strlen(name) + 1 > sizeof(image)) return NULL;
  memcpy(image, path, dir); strcpy(image + dir, name);
  return open_image(image, error, capacity);
 }
 return open_image(path, error, capacity);
}

void rlw_cd_close(RlwCd *cd) {
 if (!cd) return;
 if (cd->image) fclose(cd->image);
 free(cd->folder); free(cd);
}

int rlw_cd_is_raw(const RlwCd *cd) { return cd && cd->image && cd->sector_size == 2352; }

int rlw_cd_image_sha256(RlwCd *cd, uint8_t out[32]) {
 if (!cd || !cd->image || rlw_seek(cd->image, 0)) return -1;
 RlwSha256 c; rlw_sha256_init(&c);
 uint8_t *buffer = malloc(1u << 20); if (!buffer) return -1;
 size_t n;
 while ((n = fread(buffer, 1, 1u << 20, cd->image))) rlw_sha256_update(&c, buffer, n);
 int bad = ferror(cd->image); free(buffer); clearerr(cd->image);
 if (bad) return -1;
 rlw_sha256_final(&c, out); return 0;
}

uint8_t *rlw_cd_read(RlwCd *cd, const char *path, uint32_t *length) {
 if (!cd || !path || !length) return NULL;
 if (cd->folder) {
  char full[4096]; if (resolve_folder(cd->folder, path, full, sizeof(full))) return NULL;
  return rlw_read_file(full, length, MAX_CD_FILE);
 }
 Record r; if (resolve_image(cd, path, &r) || r.dir || r.length == 0 || r.length > MAX_CD_FILE) return NULL;
 uint8_t *data = extent(cd, r.lba, r.length);
 if (data) *length = r.length;
 return data;
}

typedef struct { int (*fn)(const char *, uint32_t, void *); void *ctx; const char *folder; } ListCtx;
static int list_image(const char *name, Record r, void *ctx) {
 ListCtx *l = ctx; return r.dir ? 0 : l->fn(name, r.length, l->ctx);
}
static int list_folder(const char *name, int is_dir, void *ctx) {
 ListCtx *l = ctx; if (is_dir) return 0;
 char full[4096]; snprintf(full, sizeof(full), "%s" RLW_SEP "%s", l->folder, name);
 FILE *f = rlw_fopen(full, "rb"); int64_t size = f ? rlw_file_size(f) : -1; if (f) fclose(f);
 char upper[256]; normalized(name, strlen(name), upper, sizeof(upper));
 return size < 0 || size > (int64_t)MAX_CD_FILE ? 0 : l->fn(upper, (uint32_t)size, l->ctx);
}

int rlw_cd_list_files(RlwCd *cd, const char *directory, int (*fn)(const char *, uint32_t, void *), void *ctx) {
 if (!cd || !fn) return -1;
 if (cd->folder) {
  char full[4096]; if (resolve_folder(cd->folder, directory, full, sizeof(full))) return -1;
  ListCtx l = {fn, ctx, full}; return rlw_list_dir(full, list_folder, &l) < 0 ? -1 : 0;
 }
 Record r; if (resolve_image(cd, directory, &r) || !r.dir) return -1;
 ListCtx l = {fn, ctx, NULL};
 return each_record(cd, r.lba, r.length, list_image, &l) < 0 ? -1 : 0;
}

/* ---- import ---- */

typedef struct { const char *output, *source, *sha256; } Input;
/* Pinned identities: RISING.EXE (README "Evidence identity"), PLATGAME.PIE
 * (game_hud_background_abi.h), EVENTS.INI (prepare_native_macos.py). The other
 * files are verified through the whole-image hash of a raw BIN and by the
 * engine's own checks at mission start. */
static const Input INPUTS[] = {
 {"RISING.EXE", "INSTALHD/RISING.EXE", "6c556520c084b9300d9ac01900d1505126085c5f64b1ac5e5b615ea41c8543fb"},
 {"RISING.INI", "INSTALHD/RISING.INI", NULL},
 {"RISING.RUL", "INSTALHD/RISING.RUL", NULL},
 {"RISING.SBL", "INSTALHD/RISING.SBL", NULL},
 {"TABLEAU.COL", "INSTALHD/TABLEAU.COL", NULL},
 {"COULEUR.MAP", "INSTALHD/COULEUR.MAP", NULL},
 {"PLATGAME.PIE", "INSTALHD/PLATGAME.PIE", "da6a07c93a6a027cd81ce50e4dfc756848223bd951f4a60e70bab1e9bc3c20a7"},
 {"MONSTRE.INI", "INSTALHD/MONSTRE.INI", NULL},
 {"TEXTES.TXT", "INSTALHD/TEXTES.TXT", NULL},
 {"EVENTS.INI", "INSTALHD/EVENTS.INI", "d31f2859a2445e0f9a0ba299369c4b556eee1ee07b7b8265b9d49b60a1b8496a"},
 {"asset0.bin", "INSTALHD/GRAPHICS/BLOCS.CMP", NULL},
 {"asset1.bin", "INSTALHD/GRAPHICS/TROOPS.CMP", NULL},
 {"asset2.bin", "INSTALHD/GRAPHICS/VEHICULE.CMP", NULL},
 {"asset3.bin", "INSTALHD/GRAPHICS/CREATURE.CMP", NULL},
 {"asset4.bin", "INSTALHD/GRAPHICS/BUILDING.CMP", NULL},
 {"asset5.bin", "INSTALHD/GRAPHICS/OTHERS.CMP", NULL},
 {"asset6.bin", "INSTALHD/PLATGAME.PIE", "da6a07c93a6a027cd81ce50e4dfc756848223bd951f4a60e70bab1e9bc3c20a7"},
 /* Original menus (milestone 2): the prepared shell's two extra banks and the
  * backgrounds the engine decodes at run time (rl_shell_imagem_cd). Pins from
  * the private research repository. */
 {"MENU.CMP", "INSTALHD/GRAPHICS/MENU.CMP", "9a0a407e8eb2be4fc24f1f7ca57f1b0aad67e7e49e0ca09b667e2d0ea4d7e866"},
 {"INVENTE.CMP", "INSTALHD/GRAPHICS/INVENTE.CMP", "a1c2e7155bec34c52d249d4c3aec7e3ce45759d806f880168dd8d8b4b05101aa"},
 {"TITRE.PIE", "INSTALHD/TITRE.PIE", "1afd5fc061a3da09e7731c776b3b86463bedcfcf546ea39114dbfb5eb2807ddc"},
 {"PAGEMIS.PIE", "INSTALHD/PAGEMIS.PIE", "1fda4342af295ed41d91e4f95d29aa9619ae5bb7ced39e75d14c3efad1b8693c"},
 {"PAGEMIS2.PIE", "INSTALHD/PAGEMIS2.PIE", "e36bcf82b58d84a6be16f7fb4ff70c64449594ca5aa71a04579f32ca8de0ce88"},
 {"CREDITS.PIE", "INSTALHD/CREDITS.PIE", NULL},
 {"ECRANFIN.LZS", "INSTALHD/ECRANFIN.LZS", "278e7125cc04546c1eb989b424fa9623811c4a3765a15fe16975450f5b738a4f"},
};
#define INPUT_COUNT (sizeof(INPUTS) / sizeof(INPUTS[0]))
#ifdef RLW_TESTING
int rlw_cd_test_ignore_pins = 0; /* unit tests only: synthetic images have other hashes */
#define PINS_ENFORCED (!rlw_cd_test_ignore_pins)
#else
#define PINS_ENFORCED 1
#endif
static const char RECEIPT[] = "importacao.json";
/* /2 adds the menu files, the briefings and the movies (milestone 2): an older
 * import is not "installed" and the player is asked for the CD again. */
static const char SCHEMA[] = "rising-lands/windows-import/2";

typedef struct {
 RlwCd *cd; const char *staging; const char *subdir; const char *suffix; const char *prefix;
 FILE *receipt; int written, failed; char error[512]; const char *cd_dir;
} CopyCtx;

static int write_output(CopyCtx *c, const char *name, const uint8_t *data, uint32_t n) {
 char target[4096];
 if (c->subdir) snprintf(target, sizeof(target), "%s" RLW_SEP "%s" RLW_SEP "%s", c->staging, c->subdir, name);
 else snprintf(target, sizeof(target), "%s" RLW_SEP "%s", c->staging, name);
 if (rlw_write_file(target, data, n)) {
  snprintf(c->error, sizeof(c->error), "Não foi possível gravar os dados importados: %s", target);
  return -1;
 }
 uint8_t digest[32]; char hex[65]; rlw_sha256(data, n, digest); rlw_sha256_hex(digest, hex);
 fprintf(c->receipt, "%s\n    {\"path\": \"%s%s%s\", \"bytes\": %u, \"sha256\": \"%s\"}", c->written ? "," : "",
         c->subdir ? c->subdir : "", c->subdir ? "/" : "", name, n, hex);
 c->written++; return 0;
}

static int copy_listed(const char *name, uint32_t size, void *ctx) {
 CopyCtx *c = ctx; (void)size;
 if (!has_suffix(name, c->suffix)) return 0;
 if (c->prefix && strncmp(name, c->prefix, strlen(c->prefix))) return 0;
 char path[512]; snprintf(path, sizeof(path), "%s/%s", c->cd_dir, name);
 uint32_t n = 0; uint8_t *data = rlw_cd_read(c->cd, path, &n);
 if (!data) { snprintf(c->error, sizeof(c->error), "O arquivo %s não foi encontrado no CD escolhido.", path); c->failed = 1; return 1; }
 int rc = write_output(c, name, data, n); free(data);
 if (rc) { c->failed = 1; return 1; }
 return 0;
}

static void progress_to(RlwProgress p, void *ctx, double f, const char *m) { if (p) p(ctx, f, m); }

int rlw_cd_import(const char *source, const char *data_root, int verify_image,
                  RlwProgress progress, void *ctx, char *error, size_t capacity) {
 RlwCd *cd = rlw_cd_open(source, error, capacity); if (!cd) return 1;
 int rc = 1; FILE *receipt = NULL; char staging[4096] = {0}, final_dir[4096], receipt_path[4096];
 uint8_t *data[INPUT_COUNT] = {0}; uint32_t lengths[INPUT_COUNT] = {0};
 int image_verified = 0;
 if (verify_image && rlw_cd_is_raw(cd)) {
  progress_to(progress, ctx, 0.02, "Verificando a imagem do CD (SHA-256)…");
  uint8_t digest[32]; char hex[65];
  if (rlw_cd_image_sha256(cd, digest)) { set_error(error, capacity, "Não foi possível ler a imagem “%s”.", cd->label); goto done; }
  rlw_sha256_hex(digest, hex);
  if (strcmp(hex, RLW_CD_BIN_SHA256)) {
   set_error(error, capacity, "A imagem “%s” é diferente do CD brasileiro original (verificação SHA-256). Use a imagem original, sem modificações.", cd->label);
   goto done;
  }
  image_verified = 1;
 }
 /* Read and verify every required original before writing anything. */
 for (size_t i = 0; i < INPUT_COUNT; i++) {
  progress_to(progress, ctx, 0.3 + 0.3 * (double)i / INPUT_COUNT, INPUTS[i].source);
  data[i] = rlw_cd_read(cd, INPUTS[i].source, &lengths[i]);
  if (!data[i]) { set_error(error, capacity, "O arquivo %s não foi encontrado no CD escolhido.", INPUTS[i].source); goto done; }
  if (INPUTS[i].sha256 && PINS_ENFORCED) {
   uint8_t digest[32]; char hex[65]; rlw_sha256(data[i], lengths[i], digest); rlw_sha256_hex(digest, hex);
   if (strcmp(hex, INPUTS[i].sha256)) {
    set_error(error, capacity, "O arquivo %s do CD escolhido é diferente do CD brasileiro original (verificação SHA-256). Use a imagem original, sem modificações.", INPUTS[i].source);
    goto done;
   }
  }
 }
 if (rlw_mkdirs(data_root)) { set_error(error, capacity, "Não foi possível criar a pasta de dados %s.", data_root); goto done; }
 snprintf(staging, sizeof(staging), "%s" RLW_SEP "Dados.importando-%lu", data_root, (unsigned long)time(NULL));
 rlw_remove_tree(staging);
 char events_dir[4096], movies_dir[4096];
 snprintf(events_dir, sizeof(events_dir), "%s" RLW_SEP "events", staging);
 snprintf(movies_dir, sizeof(movies_dir), "%s" RLW_SEP "movies", staging);
 if (rlw_mkdir(staging) || rlw_mkdir(events_dir) || rlw_mkdir(movies_dir)) { set_error(error, capacity, "Não foi possível criar %s.", staging); goto done; }
 snprintf(receipt_path, sizeof(receipt_path), "%s" RLW_SEP "%s", staging, RECEIPT);
 receipt = rlw_fopen(receipt_path, "wb");
 if (!receipt) { set_error(error, capacity, "Não foi possível gravar %s.", receipt_path); goto done; }
 char label[600]; size_t k = 0;
 for (const char *s = cd->label; *s && k + 2 < sizeof(label); s++) { if (*s == '"' || *s == '\\') label[k++] = '\\'; label[k++] = *s; }
 label[k] = 0;
 fprintf(receipt, "{\n  \"schema\": \"%s\",\n  \"source\": \"%s\",\n  \"rawImage\": %s,\n  \"imageVerified\": %s,\n  \"files\": [",
         SCHEMA, label, rlw_cd_is_raw(cd) ? "true" : "false", image_verified ? "true" : "false");
 CopyCtx c = {cd, staging, NULL, NULL, NULL, receipt, 0, 0, {0}, NULL};
 for (size_t i = 0; i < INPUT_COUNT; i++)
  if (write_output(&c, INPUTS[i].output, data[i], lengths[i])) { set_error(error, capacity, "%s", c.error); goto done; }
 progress_to(progress, ctx, 0.7, "WORLD/*.ETA");
 c.suffix = ".ETA"; c.cd_dir = "WORLD";
 if (rlw_cd_list_files(cd, "WORLD", copy_listed, &c) || c.failed) { set_error(error, capacity, "%s", c.error[0] ? c.error : "A pasta WORLD não foi encontrada no CD escolhido."); goto done; }
 /* Briefings and objectives: WORLD/MONDExx.TXT, read by the shell at their
  * original CD-root paths (an absent briefing is a valid original state). */
 progress_to(progress, ctx, 0.75, "WORLD/MONDE*.TXT");
 c.suffix = ".TXT"; c.prefix = "MONDE";
 if (rlw_cd_list_files(cd, "WORLD", copy_listed, &c) || c.failed) { set_error(error, capacity, "%s", c.error[0] ? c.error : "A pasta WORLD não foi encontrada no CD escolhido."); goto done; }
 c.prefix = NULL;
 progress_to(progress, ctx, 0.8, "EVENTS/*.FLC");
 c.suffix = ".FLC"; c.cd_dir = "EVENTS"; c.subdir = "events";
 if (rlw_cd_list_files(cd, "EVENTS", copy_listed, &c) || c.failed) { set_error(error, capacity, "%s", c.error[0] ? c.error : "A pasta EVENTS não foi encontrada no CD escolhido."); goto done; }
 /* Movies (EVENTS, .AVI), as the macOS movies/ folder: the menus need to know
  * whether the introduction exists; the player comes in a later milestone. */
 progress_to(progress, ctx, 0.85, "EVENTS/*.AVI");
 c.suffix = ".AVI"; c.subdir = "movies";
 if (rlw_cd_list_files(cd, "EVENTS", copy_listed, &c) || c.failed) { set_error(error, capacity, "%s", c.error[0] ? c.error : "A pasta EVENTS não foi encontrada no CD escolhido."); goto done; }
 time_t now = time(NULL); char stamp[32]; strftime(stamp, sizeof(stamp), "%Y-%m-%dT%H:%M:%SZ", gmtime(&now));
 fprintf(receipt, "\n  ],\n  \"count\": %d,\n  \"importedAt\": \"%s\"\n}\n", c.written, stamp);
 int bad = ferror(receipt); if (fclose(receipt)) bad = 1; receipt = NULL;
 if (bad) { set_error(error, capacity, "Não foi possível gravar %s.", receipt_path); goto done; }
 snprintf(final_dir, sizeof(final_dir), "%s" RLW_SEP "Dados", data_root);
 if (rlw_rename_replace_dir(staging, final_dir)) { set_error(error, capacity, "Não foi possível instalar os dados em %s.", final_dir); goto done; }
 staging[0] = 0;
 progress_to(progress, ctx, 1, "Importação concluída.");
 rc = 0;
done:
 if (receipt) fclose(receipt);
 if (staging[0]) rlw_remove_tree(staging);
 for (size_t i = 0; i < INPUT_COUNT; i++) free(data[i]);
 rlw_cd_close(cd);
 return rc;
}

int rlw_cd_installed(const char *data_root) {
 char path[4096];
 snprintf(path, sizeof(path), "%s" RLW_SEP "Dados" RLW_SEP "%s", data_root, RECEIPT);
 uint32_t n = 0; uint8_t *receipt = rlw_read_file(path, &n, 1u << 20);
 if (!receipt) return 0;
 int ok = n > sizeof(SCHEMA) && strstr((const char *)receipt, SCHEMA) != NULL;
 free(receipt);
 for (size_t i = 0; ok && i < INPUT_COUNT; i++) {
  snprintf(path, sizeof(path), "%s" RLW_SEP "Dados" RLW_SEP "%s", data_root, INPUTS[i].output);
  ok = rlw_exists(path);
 }
 snprintf(path, sizeof(path), "%s" RLW_SEP "Dados" RLW_SEP "MONDE01.ETA", data_root);
 return ok && rlw_exists(path);
}
