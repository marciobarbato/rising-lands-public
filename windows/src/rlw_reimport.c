#include "rlw_reimport.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <string.h>

const char RLW_REIMPORT_NOTICE[] = "Para ouvir sons e m\xc3\xbasica, use Jogo \xe2\x86\x92 Importar o CD novamente\xe2\x80\xa6";

int rlw_reimport_command_line(const char *exe, const char *source, char *out, size_t capacity) {
 if (!exe || !source || !exe[0] || !source[0] || strchr(exe, '"') || strchr(source, '"')) return -1;
 int n = snprintf(out, capacity, "\"%s\" --cd \"%s\" --reimport", exe, source);
 return n < 0 || (size_t)n >= capacity ? -1 : 0;
}

static int marker(const char *data_root, char *out, size_t capacity) {
 return !data_root || !data_root[0] || snprintf(out, capacity, "%s" RLW_SEP "aviso-reimportar", data_root) >= (int)capacity ? -1 : 0;
}
int rlw_reimport_notice_due(const char *data_root, int sound_installed, int music_installed) {
 char path[4200], dados[4200];
 if (sound_installed && music_installed) return 0;
 if (marker(data_root, path, sizeof(path)) || snprintf(dados, sizeof(dados), "%s" RLW_SEP "Dados", data_root) >= (int)sizeof(dados)) return 0;
 return rlw_is_dir(dados) && !rlw_exists(path);
}
int rlw_reimport_notice_done(const char *data_root) {
 char path[4200]; static const char text[] = "aviso de sons e musica mostrado\r\n";
 if (marker(data_root, path, sizeof(path))) return -1;
 return rlw_durable_write(path, (const uint8_t *)text, sizeof(text) - 1, NULL, 0) ? -1 : 0;
}
