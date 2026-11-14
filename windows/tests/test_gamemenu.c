/* Portable checks of the "Jogo" menu model (rlw_gamemenu.c): labels, enabled
 * and checked state, decoded commands, the save-folder scan and the Opções
 * request during play, with the host compiler; no game data.
 *   test_gamemenu <empty work folder> */
#include "rlw_fs.h"
#include "rlw_gamemenu.h"
#include <stdio.h>
#include <string.h>

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "test_gamemenu: %s:%d: %s\n", __FILE__, __LINE__, #c); failures++; } } while (0)

static RlwGameMenuState playing(void) {
 RlwGameMenuState s; memset(&s, 0, sizeof(s));
 s.driver = 1; s.playing = 1; s.saves = 1; s.keys = 1; s.options = 1; s.speed = 1; s.difficulty = 1; s.atalhos = 1;
 return s;
}
static int enabled(const RlwGameMenuState *s, unsigned id) { return rlw_gamemenu_item(s, id).enabled; }
static int checked(const RlwGameMenuState *s, unsigned id) { return rlw_gamemenu_item(s, id).checked; }

static void put(const char *dir, const char *name) {
 char path[4400]; snprintf(path, sizeof(path), "%s" RLW_SEP "%s", dir, name);
 CHECK(rlw_durable_write(path, (const uint8_t *)"x", 1, NULL, 0) == 0);
}

int main(int argc, char **argv) {
 if (argc < 2) { fprintf(stderr, "usage: test_gamemenu <work folder>\n"); return 64; }
 char text[200];
 RlwGameMenuState s = playing();

 /* Labels: the macOS player menu's texts, the Windows key hints. */
 rlw_gamemenu_label(&s, RLW_GM_PAUSE, text, sizeof(text)); CHECK(!strcmp(text, "Pausar\tP"));
 s.paused = 1; rlw_gamemenu_label(&s, RLW_GM_PAUSE, text, sizeof(text)); CHECK(!strcmp(text, "Continuar\tP")); s.paused = 0;
 rlw_gamemenu_label(&s, RLW_GM_SPEED + 0, text, sizeof(text)); CHECK(!strcmp(text, "0,5x \xc2\xb7 15 quadros/s"));
 rlw_gamemenu_label(&s, RLW_GM_SPEED + 3, text, sizeof(text)); CHECK(!strcmp(text, "4x \xc2\xb7 120 quadros/s"));
 rlw_gamemenu_label(&s, RLW_GM_DIFFICULTY + 2, text, sizeof(text)); CHECK(!strcmp(text, "Dif\xc3\xad" "cil"));
 rlw_gamemenu_label(&s, RLW_GM_OPTIONS, text, sizeof(text)); CHECK(!strcmp(text, "Op\xc3\xa7\xc3\xb5" "es\xe2\x80\xa6"));
 rlw_gamemenu_label(&s, RLW_GM_AUTOSAVE + 1, text, sizeof(text)); CHECK(!strcmp(text, "A cada 5 minutos"));
 rlw_gamemenu_label(&s, RLW_GM_MANAGE, text, sizeof(text)); CHECK(!strcmp(text, "Gerenciar partidas salvas\xe2\x80\xa6"));
 rlw_gamemenu_label(&s, RLW_GM_SHORTCUT, text, sizeof(text)); CHECK(!strcmp(text, "Criar atalho na \xc3\x81rea de Trabalho"));
 rlw_gamemenu_label(&s, RLW_GM_REIMPORT, text, sizeof(text)); CHECK(!strcmp(text, "Importar o CD novamente\xe2\x80\xa6"));
 rlw_gamemenu_label(&s, 99, text, sizeof(text)); CHECK(!text[0]);

 /* In play: everything but the loads of absent files. */
 CHECK(enabled(&s, RLW_GM_PAUSE) && enabled(&s, RLW_GM_OPTIONS) && enabled(&s, RLW_GM_QUICK_SAVE) && enabled(&s, RLW_GM_SAVE_POSITION + 1));
 CHECK(!enabled(&s, RLW_GM_QUICK_LOAD) && !enabled(&s, RLW_GM_LOAD_POSITION + 1) && !enabled(&s, RLW_GM_STAGE_LOAD) && !enabled(&s, RLW_GM_AUTOSAVE_LOAD));
 CHECK(enabled(&s, RLW_GM_MANAGE) && enabled(&s, RLW_GM_KEYS));
 CHECK(checked(&s, RLW_GM_SPEED + 1) && !checked(&s, RLW_GM_SPEED + 2) && checked(&s, RLW_GM_DIFFICULTY + 1));
 CHECK(checked(&s, RLW_GM_MODERN) && !checked(&s, RLW_GM_PATHS) && checked(&s, RLW_GM_AUTOSAVE + 0));
 CHECK(!enabled(&s, RLW_GM_SAVE_POSITION + 6) && !enabled(&s, RLW_GM_LOAD_POSITION) && !enabled(&s, 99) && !enabled(&s, 200));
 /* Paused: Opções… needs the Escape to reach the world. */
 s.paused = 1; CHECK(!enabled(&s, RLW_GM_OPTIONS) && enabled(&s, RLW_GM_PAUSE)); s.paused = 0;
 /* A save being written: no save, load or manager. */
 s.writing = 1; CHECK(!enabled(&s, RLW_GM_QUICK_SAVE) && !enabled(&s, RLW_GM_MANAGE) && enabled(&s, RLW_GM_SPEED)); s.writing = 0;
 /* Pause menu (16): Opções… and the loads, no named save, no Pausar. */
 s.playing = 0; s.shell_mode = 16; s.quick = 1; s.positions = 1u << 3;
 CHECK(enabled(&s, RLW_GM_OPTIONS) && !enabled(&s, RLW_GM_PAUSE) && !enabled(&s, RLW_GM_SAVE_POSITION + 3) && enabled(&s, RLW_GM_LOAD_POSITION + 3) && !enabled(&s, RLW_GM_LOAD_POSITION + 2));
 CHECK(enabled(&s, RLW_GM_QUICK_LOAD) && enabled(&s, RLW_GM_MANAGE));
 /* The original save list (14) and briefing (17): no manager, no Opções…. */
 s.shell_mode = 14; CHECK(!enabled(&s, RLW_GM_MANAGE) && !enabled(&s, RLW_GM_OPTIONS));
 s.shell_mode = 17; CHECK(enabled(&s, RLW_GM_MANAGE) && !enabled(&s, RLW_GM_OPTIONS));
 /* Smoke runs: no save folder, no options. */
 RlwGameMenuState smoke = playing(); smoke.saves = 0; smoke.options = 0; smoke.keys = 0;
 CHECK(enabled(&s, RLW_GM_REIMPORT) && !enabled(&smoke, RLW_GM_REIMPORT) && rlw_gamemenu_decode(&s, RLW_GM_REIMPORT).action == RLW_GM_REIMPORT);
 s.writing = 1; CHECK(!enabled(&s, RLW_GM_REIMPORT)); s.writing = 0;
 CHECK(enabled(&s, RLW_GM_SHORTCUT) && !enabled(&smoke, RLW_GM_SHORTCUT) && rlw_gamemenu_decode(&s, RLW_GM_SHORTCUT).action == RLW_GM_SHORTCUT);
 CHECK(!enabled(&smoke, RLW_GM_QUICK_SAVE) && !enabled(&smoke, RLW_GM_OPTIONS) && !enabled(&smoke, RLW_GM_MODERN) && !enabled(&smoke, RLW_GM_KEYS) && enabled(&smoke, RLW_GM_SPEED));
 /* Stopped menus: nothing that touches the game. */
 s = playing(); s.failed = 1; CHECK(!enabled(&s, RLW_GM_PAUSE) && !enabled(&s, RLW_GM_QUICK_SAVE) && !enabled(&s, RLW_GM_OPTIONS));

 /* Commands. */
 s = playing();
 RlwGameMenuCommand c = rlw_gamemenu_decode(&s, RLW_GM_FASTER); CHECK(c.action == RLW_GM_SPEED && c.arg == 2);
 c = rlw_gamemenu_decode(&s, RLW_GM_SLOWER); CHECK(c.action == RLW_GM_SPEED && c.arg == 0);
 s.speed = 3; CHECK(rlw_gamemenu_decode(&s, RLW_GM_FASTER).action == 0); s.speed = 0; CHECK(rlw_gamemenu_decode(&s, RLW_GM_SLOWER).action == 0); s.speed = 1;
 c = rlw_gamemenu_decode(&s, RLW_GM_SPEED + 3); CHECK(c.action == RLW_GM_SPEED && c.arg == 3);
 c = rlw_gamemenu_decode(&s, RLW_GM_DIFFICULTY + 0); CHECK(c.action == RLW_GM_DIFFICULTY && c.arg == 0);
 c = rlw_gamemenu_decode(&s, RLW_GM_MODERN); CHECK(c.action == RLW_GM_MODERN && c.arg == 0);
 c = rlw_gamemenu_decode(&s, RLW_GM_PATHS); CHECK(c.action == RLW_GM_PATHS && c.arg == 1);
 c = rlw_gamemenu_decode(&s, RLW_GM_PAUSE); CHECK(c.action == RLW_GM_PAUSE && c.arg == 1);
 c = rlw_gamemenu_decode(&s, RLW_GM_AUTOSAVE + 3); CHECK(c.action == RLW_GM_AUTOSAVE && c.arg == 15);
 c = rlw_gamemenu_decode(&s, RLW_GM_SAVE_POSITION + 4); CHECK(c.action == RLW_GM_SAVE_POSITION && c.arg == 4);
 CHECK(rlw_gamemenu_decode(&s, RLW_GM_LOAD_POSITION + 4).action == 0); /* absent file */
 s.positions = 1u << 4; c = rlw_gamemenu_decode(&s, RLW_GM_LOAD_POSITION + 4); CHECK(c.action == RLW_GM_LOAD_POSITION && c.arg == 4);
 CHECK(rlw_gamemenu_decode(&s, RLW_GM_OPTIONS).action == RLW_GM_OPTIONS && rlw_gamemenu_decode(&s, RLW_GM_MANAGE).action == RLW_GM_MANAGE);
 CHECK(rlw_gamemenu_decode(&s, 77).action == 0);

 /* The folder scan. */
 char dir[4200]; snprintf(dir, sizeof(dir), "%s" RLW_SEP "jogo-menu", argv[1]); rlw_remove_tree(dir); CHECK(rlw_mkdirs(dir) == 0);
 RlwGameMenuState scan; memset(&scan, 0, sizeof(scan)); rlw_gamemenu_scan(&scan, dir);
 CHECK(!scan.quick && !scan.stage && !scan.autosave && !scan.positions);
 put(dir, "rapido.rls"); put(dir, "autosave.rls"); put(dir, "posicao-2.rls"); put(dir, "posicao-5.rls"); put(dir, "posicao-7.rls");
 rlw_gamemenu_scan(&scan, dir);
 CHECK(scan.quick && !scan.stage && scan.autosave && scan.positions == ((1u << 2) | (1u << 5)));
 rlw_gamemenu_scan(&scan, ""); CHECK(!scan.quick && !scan.positions);
 rlw_remove_tree(dir);

 /* Opções… during play: Escape down once, the pause shows the screen; without a
  * pause the Escape is released 30 frames later (only in play). */
 RlwOptionsRequest r; memset(&r, 0, sizeof(r));
 CHECK(rlw_options_request_start(&r, 100) && !rlw_options_request_start(&r, 101));
 CHECK(!rlw_options_request_frame(&r, 129, 1) && !rlw_options_request_frame(&r, 140, 0));
 CHECK(rlw_options_request_shell(&r) && !rlw_options_request_shell(&r) && !rlw_options_request_frame(&r, 200, 1));
 CHECK(rlw_options_request_start(&r, 200) && rlw_options_request_frame(&r, 230, 1) && !r.pending && !rlw_options_request_shell(&r));

 if (failures) return 1;
 printf("test_gamemenu ok: Jogo menu labels, state, commands, folder scan, Opções request\n");
 return 0;
}
