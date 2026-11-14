/* Portable checks of the saved-games manager (rlw_savemgr.c: list, rename,
 * Recycle Bin through an injected fake) and the periodic autosave
 * (rlw_autosave.c), with the host compiler; no game data.
 *   test_savemgr <empty work folder> */
#include "rlw_fs.h"
#include "rlw_saves.h"
#include "rlw_savemgr.h"
#include "rlw_autosave.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "test_savemgr: %s:%d: %s\n", __FILE__, __LINE__, #c); failures++; } } while (0)

/* A companion with the RLVIEW4 header, any presentation, the mission and difficulty. */
static int write_view(const char *path, const char *mission, uint32_t difficulty) {
 size_t n = 72u + 751364u + 28u; uint8_t *b = calloc(1, n); if (!b) return -1;
 memcpy(b, "RLVIEW4\n", 8); snprintf((char *)b + 72 + 751364, 16, "%s", mission);
 for (unsigned i = 0; i < 4; i++) b[72 + 751364 + 16 + i] = (uint8_t)(difficulty >> (8 * i));
 int c = rlw_durable_write(path, b, n, NULL, 0); free(b); return c;
}
/* The Recycle Bin of the tests: a move into <work>/lixeira, or a refusal. */
typedef struct { char folder[4200]; unsigned moved, refuse_at; } FakeTrash;
static int fake_trash(void *ctx, const char *path, char *reason, size_t capacity) {
 FakeTrash *t = ctx; t->moved++;
 if (t->refuse_at && t->moved == t->refuse_at) { snprintf(reason, capacity, "recusado"); return 1; }
 const char *base = path; for (const char *p = path; *p; p++) if (*p == '/' || *p == '\\') base = p + 1;
 char target[4400]; snprintf(target, sizeof(target), "%s" RLW_SEP "%s", t->folder, base);
 return rename(path, target) != 0;
}
static void put(const char *dir, const char *name, const char *text) {
 char path[4400]; snprintf(path, sizeof(path), "%s" RLW_SEP "%s", dir, name);
 CHECK(rlw_durable_write(path, (const uint8_t *)text, strlen(text), NULL, 0) == 0);
}

/* Partida > Gerenciar partidas salvas… (rlw_savemgr.c): list, rename, trash. */
static void test_manager(const char *work) {
 char dir[4200], trash_dir[4200]; snprintf(dir, sizeof(dir), "%s" RLW_SEP "gerenciar", work); snprintf(trash_dir, sizeof(trash_dir), "%s" RLW_SEP "lixeira", work);
 rlw_remove_tree(dir); rlw_remove_tree(trash_dir); CHECK(rlw_mkdirs(dir) == 0); CHECK(rlw_mkdirs(trash_dir) == 0);
 RlwSaveList list; rlw_savemgr_list(dir, &list); CHECK(list.count == 0);
 /* posicao-3 named with one companion, posicao-10, rapido, inicio-fase, autosave;
  * a temporary companion and a foreign file are ignored. */
 put(dir, "posicao-3.rls", "a"); put(dir, "posicao-10.rls", "b"); put(dir, "rapido.rls", "c"); put(dir, "inicio-fase.rls", "d"); put(dir, "autosave.rls", "e");
 char view[4400]; snprintf(view, sizeof(view), "%s" RLW_SEP "posicao-3.rls.aaaa.view", dir); CHECK(write_view(view, "MONDE07B.ETA", 2) == 0);
 snprintf(view, sizeof(view), "%s" RLW_SEP "rapido.rls.bbbb.view", dir); CHECK(write_view(view, "MONDE01.ETA", 1) == 0);
 put(dir, "posicao-3.rls.cccc.tmp.view", "x"); put(dir, "posicao-30.rls", "f"); put(dir, "posicao-10.rls.dddd.view", "RLVIEW9");
 RlwSaveNames names; memset(&names, 0, sizeof(names)); snprintf(names.name[3], sizeof(names.name[3]), "Ilha");
 CHECK(rlw_saves_names_write(dir, &names) == 0);
 rlw_savemgr_list(dir, &list);
 CHECK(list.count == 5);
 CHECK(list.game[0].kind == RLW_SAVE_NAMED && list.game[0].number == 3 && !strcmp(list.game[0].name, "Ilha") && list.game[0].view_count == 1);
 CHECK(!strcmp(list.game[0].mission, "MONDE07B.ETA") && list.game[0].difficulty == 2);
 CHECK(list.game[1].number == 10 && !strcmp(list.game[1].name, "Posi\xc3\xa7\xc3\xa3o 10") && list.game[1].view_count == 1 && !list.game[1].mission[0]);
 CHECK(list.game[2].kind == RLW_SAVE_QUICK && !strcmp(list.game[2].name, "Salvamento r\xc3\xa1pido"));
 CHECK(list.game[3].kind == RLW_SAVE_STAGE && !strcmp(list.game[4].slot, "autosave") && list.game[4].kind == RLW_SAVE_PERIODIC);
 char text[300], error[600];
 rlw_savemgr_stage_title(&list.game[0], text, sizeof(text)); CHECK(!strcmp(text, "Fase 7B \xc2\xb7 Dif\xc3\xad" "cil"));
 rlw_savemgr_stage_title(&list.game[2], text, sizeof(text)); CHECK(!strcmp(text, "Fase 1 \xc2\xb7 Normal"));
 rlw_savemgr_stage_title(&list.game[1], text, sizeof(text)); CHECK(!strcmp(text, "Fase desconhecida"));
 rlw_savemgr_date(0, text, sizeof(text)); CHECK(strlen(text) == 16 && text[2] == '/' && text[5] == '/' && text[13] == ':');
 CHECK(rlw_savemgr_can_rename(&list.game[0]) && !rlw_savemgr_can_rename(&list.game[2]));
 CHECK(rlw_savemgr_available(0) && rlw_savemgr_available(16) && rlw_savemgr_available(2) && !rlw_savemgr_available(12) && !rlw_savemgr_available(13) && !rlw_savemgr_available(14));
 /* Names: trimmed, 80 characters at most, never empty. */
 char name[RLW_SAVE_NAME_CAPACITY];
 CHECK(rlw_savemgr_valid_name("  \t\n ", name, sizeof(name), error, sizeof(error)) == -1 && !strcmp(error, "Digite um nome para a partida."));
 CHECK(rlw_savemgr_valid_name("  Norte \n", name, sizeof(name), error, sizeof(error)) == 0 && !strcmp(name, "Norte"));
 char long_name[400] = ""; for (int i = 0; i < 90; i++) strcat(long_name, "\xc3\xa9");
 CHECK(rlw_savemgr_valid_name(long_name, name, sizeof(name), error, sizeof(error)) == 0 && strlen(name) == 160);
 /* Rename: only positions, written to nomes.plist (the original lists read it). */
 CHECK(rlw_savemgr_rename(dir, &list.game[2], "x", error, sizeof(error)) == -1 && strstr(error, "1 a 10"));
 CHECK(rlw_savemgr_rename(dir, &list.game[1], " Dezena ", error, sizeof(error)) == 0);
 rlw_saves_names_read(dir, &names); CHECK(!strcmp(names.name[10], "Dezena") && !strcmp(names.name[3], "Ilha"));
 CHECK(rlw_savemgr_rename(dir, &list.game[1], "", error, sizeof(error)) == -1);
 rlw_saves_names_read(dir, &names); CHECK(!strcmp(names.name[10], "Dezena"));
 /* Trash: the .rls then its companion move; the name goes; nothing deleted. */
 FakeTrash t; memset(&t, 0, sizeof(t)); snprintf(t.folder, sizeof(t.folder), "%s", trash_dir);
 CHECK(rlw_savemgr_trash(dir, &list.game[0], fake_trash, &t, error, sizeof(error)) == 0 && t.moved == 2);
 char moved[4400]; snprintf(moved, sizeof(moved), "%s" RLW_SEP "posicao-3.rls", trash_dir); CHECK(rlw_exists(moved));
 snprintf(moved, sizeof(moved), "%s" RLW_SEP "posicao-3.rls.aaaa.view", trash_dir); CHECK(rlw_exists(moved));
 rlw_saves_names_read(dir, &names); CHECK(names.name[3][0] == 0 && !strcmp(names.name[10], "Dezena"));
 CHECK(rlw_savemgr_trash(dir, &list.game[0], fake_trash, &t, error, sizeof(error)) == -1 && strstr(error, "n\xc3\xa3o est\xc3\xa1 mais"));
 CHECK(rlw_savemgr_rename(dir, &list.game[0], "y", error, sizeof(error)) == -1);
 /* A refused move stops with the file named; the slot stays. */
 t.moved = 0; t.refuse_at = 1;
 CHECK(rlw_savemgr_trash(dir, &list.game[2], fake_trash, &t, error, sizeof(error)) == -1 && strstr(error, "rapido.rls") && strstr(error, "recusado"));
 rlw_savemgr_list(dir, &list); CHECK(list.count == 4 && list.game[0].number == 10 && !strcmp(list.game[0].name, "Dezena"));
 rlw_remove_tree(dir); rlw_remove_tree(trash_dir);
}

/* Partida > Salvamento automático (rlw_autosave.c): the macOS PlayerOptionsTests cases. */
static void test_autosave(const char *work) {
 char text[200]; unsigned minutes = 99;
 for (unsigned i = 0; i < 4; i++) CHECK(rlw_autosave_valid_minutes(RLW_AUTOSAVE_CHOICES[i]));
 CHECK(RLW_AUTOSAVE_CHOICES[0] == 0 && RLW_AUTOSAVE_CHOICES[1] == 5 && RLW_AUTOSAVE_CHOICES[2] == 10 && RLW_AUTOSAVE_CHOICES[3] == 15);
 rlw_autosave_title(0, text, sizeof(text)); CHECK(!strcmp(text, "Desligado"));
 rlw_autosave_title(10, text, sizeof(text)); CHECK(!strcmp(text, "A cada 10 minutos"));
 CHECK(rlw_autosave_option("autosave=10\r\n", &minutes) && minutes == 10);
 minutes = 0; CHECK(!rlw_autosave_option("autosave=7\r\n", &minutes) && minutes == 0); /* unknown: stays off */
 CHECK(!rlw_autosave_option("autosave=5x", &minutes) && !rlw_autosave_option("atalhos=1", &minutes) && minutes == 0);
 CHECK(!strcmp(RLW_AUTOSAVE_SLOT, "autosave") && RLW_AUTOSAVE_FRAMES_PER_MINUTE == 30 * 60);
 CHECK(rlw_autosave_frames(5) == 9000 && rlw_autosave_frames(15) == 27000 && rlw_autosave_frames(0) == 0 && rlw_autosave_frames(7) == 0);
 char dir[4200]; snprintf(dir, sizeof(dir), "%s" RLW_SEP "autosave-caption", work); rlw_remove_tree(dir); CHECK(rlw_mkdirs(dir) == 0);
 rlw_autosave_caption(dir, text, sizeof(text)); CHECK(!strcmp(text, "Salvamento autom\xc3\xa1tico \xc2\xb7 vazio"));
 put(dir, "autosave.rls", "x"); rlw_autosave_caption(dir, text, sizeof(text));
 CHECK(!strncmp(text, "Salvamento autom\xc3\xa1tico \xc2\xb7 ", 26) && strlen(text) == 26 + 11 && text[28] == '/' && text[34] == ':');
 rlw_remove_tree(dir);

 RlwAutosave off; rlw_autosave_init(&off, rlw_autosave_frames(0)); int fired = 0;
 for (int i = 0; i < 100000; i++) fired |= rlw_autosave_frame(&off, 1);
 CHECK(!rlw_autosave_enabled(&off) && !fired); /* off never fires */

 RlwAutosave s; rlw_autosave_init(&s, 10); fired = 0;
 for (int i = 0; i < 9; i++) fired |= rlw_autosave_frame(&s, 1);
 CHECK(!fired); /* not before the interval */
 for (int i = 0; i < 50; i++) fired |= rlw_autosave_frame(&s, 0);
 CHECK(!fired && s.elapsed == 9); /* menus, pause, stops do not count */
 CHECK(rlw_autosave_frame(&s, 1)); /* due on the interval's last eligible frame */
 CHECK(rlw_autosave_frame(&s, 1) && s.elapsed == 10); /* busy boundary: stays due */
 CHECK(!rlw_autosave_frame(&s, 0)); /* never at an ineligible frame */
 rlw_autosave_saved(&s); CHECK(!rlw_autosave_due(&s) && s.elapsed == 0);
 fired = 0; for (int i = 0; i < 9; i++) fired |= rlw_autosave_frame(&s, 1);
 CHECK(!fired && rlw_autosave_frame(&s, 1)); /* one interval later */
 rlw_autosave_failed(&s); fired = 0; for (int i = 0; i < 9; i++) fired |= rlw_autosave_frame(&s, 1);
 CHECK(!fired); /* a failure waits a whole interval */
 rlw_autosave_restart(&s); CHECK(s.elapsed == 0);

 RlwAutosave fast; rlw_autosave_init(&fast, rlw_autosave_frames(5)); unsigned saves[4], n = 0;
 for (unsigned frame = 1; frame <= 3 * 9000; frame++) if (rlw_autosave_frame(&fast, 1)) { if (n < 4) saves[n] = frame; n++; rlw_autosave_saved(&fast); }
 CHECK(n == 3 && saves[0] == 9000 && saves[1] == 18000 && saves[2] == 27000);

 RlwAutosave change; rlw_autosave_init(&change, rlw_autosave_frames(15));
 for (int i = 0; i < 12000; i++) rlw_autosave_frame(&change, 1);
 rlw_autosave_set_interval(&change, rlw_autosave_frames(10)); CHECK(change.elapsed == 12000 && !rlw_autosave_due(&change));
 rlw_autosave_set_interval(&change, rlw_autosave_frames(5)); CHECK(change.elapsed == 9000 && rlw_autosave_frame(&change, 1));
 rlw_autosave_set_interval(&change, 0); CHECK(!rlw_autosave_enabled(&change) && change.elapsed == 0 && !rlw_autosave_frame(&change, 1));
 rlw_autosave_set_interval(&change, rlw_autosave_frames(5)); fired = 0;
 for (int i = 0; i < 8999; i++) fired |= rlw_autosave_frame(&change, 1);
 CHECK(!fired && rlw_autosave_frame(&change, 1)); /* turning it on starts a full interval */
}

int main(int argc, char **argv) {
 if (argc < 2) { fprintf(stderr, "usage: test_savemgr <work folder>\n"); return 64; }
 test_manager(argv[1]);
 test_autosave(argv[1]);
 if (failures) return 1;
 printf("test_savemgr ok: saved-games list, rename, Recycle Bin, periodic autosave\n");
 return 0;
}
