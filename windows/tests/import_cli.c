/* Portable CLI over the Windows host's CD importer (same code as RisingLands.exe):
 *   rlw-import <RISING_LANDS_BR_.BIN|.CUE|.ISO|pasta> <pasta-de-dados> */
#include "rlw_cd.h"
#include <stdio.h>

static void progress(void *ctx, double fraction, const char *message) {
 (void)ctx; printf("importando %3d%% %s\n", (int)(fraction * 100), message); fflush(stdout);
}

int main(int argc, char **argv) {
 if (argc != 3) { fprintf(stderr, "usage: rlw-import <CD> <data-root>\n"); return 64; }
 char error[1024] = {0};
 if (rlw_cd_import(argv[1], argv[2], 1, progress, NULL, error, sizeof(error))) { fprintf(stderr, "%s\n", error); return 1; }
 printf("importado em %s/Dados\n", argv[2]);
 return 0;
}
