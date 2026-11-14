/* Sound import from the player's CD (see rlw_sound_import.h). */
#include "rlw_sound_import.h"
#include "rlw_sound.h"
#include "rlw_cd.h"
#include "rlw_fs.h"
#include "rlw_sha256.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef RLW_TESTING
int rlw_sound_test_ignore_pins = 0;
#define PINS_ENFORCED (!rlw_sound_test_ignore_pins)
#else
#define PINS_ENFORCED 1
#endif

static void set_error(char *error, size_t capacity, const char *text, const char *name) {
 if (error && capacity) snprintf(error, capacity, text, name);
}

int rlw_sound_import(const char *source, const char *data_root, char *error, size_t capacity) {
 char cd_error[512] = {0};
 RlwCd *cd = rlw_cd_open(source, cd_error, sizeof(cd_error));
 if (!cd) { set_error(error, capacity, "Som: %s", cd_error); return 1; }
 char dados[4096], staging[4096], final_dir[4096];
 snprintf(dados, sizeof(dados), "%s" RLW_SEP "Dados", data_root);
 snprintf(staging, sizeof(staging), "%s" RLW_SEP "sound.importando-%lu", dados, (unsigned long)time(NULL));
 snprintf(final_dir, sizeof(final_dir), "%s" RLW_SEP "sound", dados);
 int rc = 1;
 rlw_remove_tree(staging);
 if (rlw_mkdirs(staging)) { set_error(error, capacity, "Não foi possível criar %s.", staging); goto done; }
 for (unsigned i = 0; i < 6; i++) {
  char path[64], target[4096]; snprintf(path, sizeof(path), "INSTALHD/SOUND/%s", RLW_SOUND_FILES[i].name);
  uint32_t n = 0; uint8_t *data = rlw_cd_read(cd, path, &n);
  if (!data) { set_error(error, capacity, "O arquivo %s não foi encontrado no CD escolhido.", path); goto done; }
  if (PINS_ENFORCED) {
   uint8_t digest[32]; char hex[65]; rlw_sha256(data, n, digest); rlw_sha256_hex(digest, hex);
   if (n != RLW_SOUND_FILES[i].size || strcmp(hex, RLW_SOUND_FILES[i].sha256)) {
    free(data); set_error(error, capacity, "O arquivo %s do CD escolhido é diferente do CD brasileiro original (verificação SHA-256).", path); goto done;
   }
  }
  snprintf(target, sizeof(target), "%s" RLW_SEP "%s", staging, RLW_SOUND_FILES[i].name);
  int bad = rlw_write_file(target, data, n); free(data);
  if (bad) { set_error(error, capacity, "Não foi possível gravar %s.", target); goto done; }
 }
 if (rlw_rename_replace_dir(staging, final_dir)) { set_error(error, capacity, "Não foi possível instalar os sons em %s.", final_dir); goto done; }
 staging[0] = 0; rc = 0;
done:
 if (staging[0]) rlw_remove_tree(staging);
 rlw_cd_close(cd);
 return rc;
}

int rlw_sound_installed(const char *dados) {
 for (unsigned i = 0; i < 6; i++) {
  char path[4096]; snprintf(path, sizeof(path), "%s" RLW_SEP "sound" RLW_SEP "%s", dados, RLW_SOUND_FILES[i].name);
  if (!rlw_exists(path)) return 0;
 }
 return 1;
}
