#include "rlw_savemgr.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* NativeSaveLibrary.presentationSize: surface + terrain keys + payloads + cursor. */
#define VIEW_PRESENTATION_SIZE (480000u + 1024u + 270336u + 4u)

static const char *slot_name(int kind, unsigned number, char *out, size_t capacity) {
 switch (kind) {
 case RLW_SAVE_NAMED: snprintf(out, capacity, "posicao-%u", number); break;
 case RLW_SAVE_QUICK: snprintf(out, capacity, "rapido"); break;
 case RLW_SAVE_STAGE: snprintf(out, capacity, "inicio-fase"); break;
 default: snprintf(out, capacity, "autosave"); break;
 }
 return out;
}
const char *rlw_savemgr_default_name(int kind, unsigned number, char *out, size_t capacity) {
 switch (kind) {
 case RLW_SAVE_NAMED: snprintf(out, capacity, "Posi\xc3\xa7\xc3\xa3o %u", number); break;
 case RLW_SAVE_QUICK: snprintf(out, capacity, "Salvamento r\xc3\xa1pido"); break;
 case RLW_SAVE_STAGE: snprintf(out, capacity, "In\xc3\xad" "cio da fase \xc2\xb7 salvamento autom\xc3\xa1tico"); break;
 default: snprintf(out, capacity, "Salvamento autom\xc3\xa1tico"); break;
 }
 return out;
}
int rlw_savemgr_available(uint32_t shell_mode) { return shell_mode != 12 && shell_mode != 13 && shell_mode != 14; }
int rlw_savemgr_can_rename(const RlwSavedGame *game) { return game && game->kind == RLW_SAVE_NAMED; }

int rlw_savemgr_view_stage(const char *view, char mission[16], uint32_t *difficulty) {
 FILE *f = rlw_fopen(view, "rb"); if (!f) return -1;
 uint8_t magic[8], tail[20];
 int bad = fread(magic, 1, 8, f) != 8 || (memcmp(magic, "RLVIEW3\n", 8) && memcmp(magic, "RLVIEW4\n", 8));
 if (!bad) bad = rlw_seek(f, 72u + VIEW_PRESENTATION_SIZE) || fread(tail, 1, 20, f) != 20;
 fclose(f); if (bad) return -1;
 char name[17] = {0}; memcpy(name, tail, 16); size_t n = strlen(name);
 if (n < 11 || n > 12 || strncmp(name, "MONDE", 5) || strcmp(name + n - 4, ".ETA")) return -1;
 for (size_t i = 0; i < n; i++) if ((unsigned char)name[i] >= 0x80) return -1;
 memcpy(mission, name, 16);
 *difficulty = (uint32_t)tail[16] | ((uint32_t)tail[17] << 8) | ((uint32_t)tail[18] << 16) | ((uint32_t)tail[19] << 24);
 return 0;
}

typedef struct { const char *prefix; char names[RLW_SAVEMGR_MAX_VIEWS][300]; unsigned count; } ViewScan;
static int scan_view(const char *name, int is_dir, void *ctx) {
 ViewScan *s = ctx; size_t n = strlen(name), p = strlen(s->prefix);
 if (is_dir || n <= p + 5 || strncmp(name, s->prefix, p) || strcmp(name + n - 5, ".view") || strstr(name, ".tmp.")) return 0;
 if (s->count < RLW_SAVEMGR_MAX_VIEWS && n < sizeof(s->names[0])) snprintf(s->names[s->count++], sizeof(s->names[0]), "%s", name);
 return 0;
}
static int compare_names(const void *a, const void *b) { return strcmp((const char *)a, (const char *)b); }

void rlw_savemgr_list(const char *directory, RlwSaveList *out) {
 memset(out, 0, sizeof(*out));
 RlwSaveNames names; rlw_saves_names_read(directory, &names);
 for (unsigned k = 0; k < RLW_SAVEMGR_MAX_GAMES; k++) {
  int kind = k < 10 ? RLW_SAVE_NAMED : (int)(k - 9); unsigned number = k < 10 ? k + 1 : 0;
  RlwSavedGame *g = &out->game[out->count]; memset(g, 0, sizeof(*g));
  g->kind = kind; g->number = number; slot_name(kind, number, g->slot, sizeof(g->slot));
  if (rlw_saves_path(directory, g->slot, g->file, sizeof(g->file))) continue;
  g->mtime = rlw_file_mtime(g->file); if (g->mtime < 0) continue;
  ViewScan scan; memset(&scan, 0, sizeof(scan)); char prefix[64]; snprintf(prefix, sizeof(prefix), "%s.rls.", g->slot); scan.prefix = prefix;
  rlw_list_dir(directory, scan_view, &scan);
  qsort(scan.names, scan.count, sizeof(scan.names[0]), compare_names);
  for (unsigned i = 0; i < scan.count; i++) snprintf(g->views[g->view_count++], sizeof(g->views[0]), "%s" RLW_SEP "%s", directory, scan.names[i]);
  if (kind == RLW_SAVE_NAMED && names.name[number][0]) snprintf(g->name, sizeof(g->name), "%s", names.name[number]);
  else rlw_savemgr_default_name(kind, number, g->name, sizeof(g->name));
  for (unsigned i = 0; i < g->view_count && !g->mission[0]; i++) if (rlw_savemgr_view_stage(g->views[i], g->mission, &g->difficulty)) g->mission[0] = 0;
  out->count++;
 }
}

void rlw_savemgr_stage_title(const RlwSavedGame *game, char *out, size_t capacity) {
 const char *m = game ? game->mission : "";
 size_t n = strlen(m);
 if (n < 11 || m[5] < '0' || m[5] > '9' || m[6] < '0' || m[6] > '9') { snprintf(out, capacity, "Fase desconhecida"); return; }
 int number = (m[5] - '0') * 10 + (m[6] - '0');
 char variant[8] = {0}; size_t v = 0; for (size_t i = 7; i < n && m[i] != '.' && v + 1 < sizeof(variant); i++) variant[v++] = m[i];
 static const char *const levels[3] = {"F\xc3\xa1" "cil", "Normal", "Dif\xc3\xad" "cil"};
 if (game->difficulty <= 2) snprintf(out, capacity, "Fase %d%s \xc2\xb7 %s", number, variant, levels[game->difficulty]);
 else snprintf(out, capacity, "Fase %d%s", number, variant);
}

void rlw_savemgr_date(int64_t mtime, char *out, size_t capacity) {
 time_t when = (time_t)mtime; struct tm *t = localtime(&when);
 if (!t) { snprintf(out, capacity, "?"); return; }
 snprintf(out, capacity, "%02d/%02d/%04d %02d:%02d", t->tm_mday, t->tm_mon + 1, t->tm_year + 1900, t->tm_hour, t->tm_min);
}

static int is_space(unsigned char c) { return c == ' ' || c == '\t' || c == '\r' || c == '\n' || c == '\v' || c == '\f'; }
int rlw_savemgr_valid_name(const char *text, char *out, size_t capacity, char *error, size_t error_capacity) {
 const char *b = text ? text : ""; while (is_space((unsigned char)*b)) b++;
 size_t n = strlen(b); while (n && is_space((unsigned char)b[n - 1])) n--;
 /* The first 80 characters: count UTF-8 lead bytes. */
 size_t k = 0; unsigned characters = 0;
 while (k < n) {
  size_t next = k + 1; while (next < n && ((unsigned char)b[next] & 0xc0) == 0x80) next++;
  if (characters == RLW_SAVEMGR_NAME_CHARACTERS || next >= capacity) break;
  k = next; characters++;
 }
 if (!k) { if (error && error_capacity) snprintf(error, error_capacity, "Digite um nome para a partida."); if (capacity) out[0] = 0; return -1; }
 memcpy(out, b, k); out[k] = 0; return 0;
}

int rlw_savemgr_rename(const char *directory, const RlwSavedGame *game, const char *text, char *error, size_t error_capacity) {
 if (!rlw_savemgr_can_rename(game) || game->number < 1 || game->number > 10) { snprintf(error, error_capacity, "S\xc3\xb3 as partidas das posi\xc3\xa7\xc3\xb5" "es 1 a 10 t\xc3\xaam nome."); return -1; }
 if (!rlw_exists(game->file)) { snprintf(error, error_capacity, "A partida \xe2\x80\x9c%s\xe2\x80\x9d n\xc3\xa3o est\xc3\xa1 mais na pasta de partidas salvas.", game->name); return -1; }
 char name[RLW_SAVE_NAME_CAPACITY]; if (rlw_savemgr_valid_name(text, name, sizeof(name), error, error_capacity)) return -1;
 RlwSaveNames names; rlw_saves_names_read(directory, &names);
 snprintf(names.name[game->number], sizeof(names.name[game->number]), "%s", name);
 if (rlw_mkdirs(directory) || rlw_saves_names_write(directory, &names)) { snprintf(error, error_capacity, "N\xc3\xa3o foi poss\xc3\xadvel gravar o nome da partida (nomes.plist)."); return -1; }
 return 0;
}

static const char *base_name(const char *path) {
 const char *b = path; for (const char *p = path; *p; p++) if (*p == '/' || *p == '\\') b = p + 1; return b;
}
int rlw_savemgr_trash(const char *directory, const RlwSavedGame *game, RlwTrashFn trash, void *ctx, char *error, size_t error_capacity) {
 if (!game || !trash) { snprintf(error, error_capacity, "Partida inv\xc3\xa1lida."); return -1; }
 if (!rlw_exists(game->file)) { snprintf(error, error_capacity, "A partida \xe2\x80\x9c%s\xe2\x80\x9d n\xc3\xa3o est\xc3\xa1 mais na pasta de partidas salvas.", game->name); return -1; }
 for (unsigned i = 0; i <= game->view_count; i++) {
  const char *file = i ? game->views[i - 1] : game->file; char reason[300] = "";
  if (trash(ctx, file, reason, sizeof(reason))) {
   snprintf(error, error_capacity, "N\xc3\xa3o foi poss\xc3\xadvel mover \xe2\x80\x9c%s\xe2\x80\x9d para a Lixeira: %s", base_name(file), reason[0] ? reason : "erro desconhecido");
   return -1;
  }
 }
 if (game->kind == RLW_SAVE_NAMED && game->number >= 1 && game->number <= 10) {
  RlwSaveNames names; rlw_saves_names_read(directory, &names);
  if (names.name[game->number][0]) {
   names.name[game->number][0] = 0;
   if (rlw_saves_names_write(directory, &names)) { snprintf(error, error_capacity, "A partida foi para a Lixeira, mas o nome da posi\xc3\xa7\xc3\xa3o n\xc3\xa3o p\xc3\xb4" "de ser apagado (nomes.plist)."); return -1; }
  }
 }
 return 0;
}
