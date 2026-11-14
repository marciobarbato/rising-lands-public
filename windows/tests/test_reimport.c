/* Portable checks of Jogo > Importar o CD novamente… (rlw_reimport.c): the
 * relaunch command line and the one-time sound/music notice; no game data.
 *   test_reimport <empty work folder> */
#include "rlw_fs.h"
#include "rlw_reimport.h"
#include <stdio.h>
#include <string.h>

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "test_reimport: %s:%d: %s\n", __FILE__, __LINE__, #c); failures++; } } while (0)

int main(int argc, char **argv) {
 if (argc < 2) { fprintf(stderr, "usage: test_reimport <work folder>\n"); return 64; }
 char line[600];
 CHECK(rlw_reimport_command_line("C:\\Jogos\\Rising Lands\\RisingLands.exe", "D:\\CD\\RISING_LANDS_BR_.CUE", line, sizeof(line)) == 0);
 CHECK(!strcmp(line, "\"C:\\Jogos\\Rising Lands\\RisingLands.exe\" --cd \"D:\\CD\\RISING_LANDS_BR_.CUE\" --reimport"));
 CHECK(rlw_reimport_command_line("a.exe", "x\"y.cue", line, sizeof(line)) == -1);
 CHECK(rlw_reimport_command_line("a.exe", "", line, sizeof(line)) == -1);
 CHECK(rlw_reimport_command_line("a.exe", "b.cue", line, 10) == -1);
 CHECK(!strcmp(RLW_REIMPORT_NOTICE, "Para ouvir sons e m\xc3\xbasica, use Jogo \xe2\x86\x92 Importar o CD novamente\xe2\x80\xa6"));

 char root[4200], dados[4300]; snprintf(root, sizeof(root), "%s" RLW_SEP "reimportar", argv[1]); rlw_remove_tree(root);
 CHECK(rlw_mkdirs(root) == 0);
 CHECK(!rlw_reimport_notice_due(root, 0, 0)); /* no Dados yet: the first start imports everything */
 snprintf(dados, sizeof(dados), "%s" RLW_SEP "Dados", root); CHECK(rlw_mkdirs(dados) == 0);
 CHECK(!rlw_reimport_notice_due(root, 1, 1)); /* all there */
 CHECK(rlw_reimport_notice_due(root, 0, 1) && rlw_reimport_notice_due(root, 1, 0) && rlw_reimport_notice_due(root, 0, 0));
 CHECK(rlw_reimport_notice_done(root) == 0);
 CHECK(!rlw_reimport_notice_due(root, 0, 0)); /* once */
 CHECK(!rlw_reimport_notice_due("", 0, 0) && rlw_reimport_notice_done("") == -1);
 rlw_remove_tree(root);
 if (failures) return 1;
 printf("test_reimport ok: relaunch command line, one-time sound/music notice\n");
 return 0;
}
