#include "rlw_autosave.h"
#include "rlw_fs.h"
#include "rlw_saves.h"
#include <stdio.h>
#include <string.h>
#include <time.h>

const unsigned RLW_AUTOSAVE_CHOICES[4] = {0, 5, 10, 15};

int rlw_autosave_valid_minutes(unsigned minutes) { return minutes == 0 || minutes == 5 || minutes == 10 || minutes == 15; }
uint64_t rlw_autosave_frames(unsigned minutes) { return rlw_autosave_valid_minutes(minutes) ? (uint64_t)minutes * RLW_AUTOSAVE_FRAMES_PER_MINUTE : 0; }
int rlw_autosave_option(const char *line, unsigned *minutes) {
 unsigned v = 0; char tail = 0;
 if (strncmp(line, "autosave=", 9) || sscanf(line + 9, "%u%c", &v, &tail) < 1 || (tail && tail != '\r' && tail != '\n') || !rlw_autosave_valid_minutes(v)) return 0;
 *minutes = v; return 1;
}
void rlw_autosave_title(unsigned minutes, char *out, size_t capacity) {
 if (!minutes) snprintf(out, capacity, "Desligado"); else snprintf(out, capacity, "A cada %u minutos", minutes);
}
void rlw_autosave_caption(const char *directory, char *out, size_t capacity) {
 char path[4200]; int64_t t = -1;
 if (directory && directory[0] && !rlw_saves_path(directory, RLW_AUTOSAVE_SLOT, path, sizeof(path))) t = rlw_file_mtime(path);
 time_t when = (time_t)t; struct tm *local = t >= 0 ? localtime(&when) : NULL;
 if (!local) { snprintf(out, capacity, "Salvamento autom\xc3\xa1tico \xc2\xb7 vazio"); return; }
 snprintf(out, capacity, "Salvamento autom\xc3\xa1tico \xc2\xb7 %02d/%02d %02d:%02d", local->tm_mday, local->tm_mon + 1, local->tm_hour, local->tm_min);
}

void rlw_autosave_init(RlwAutosave *a, uint64_t interval_frames) { a->interval_frames = interval_frames; a->elapsed = 0; }
int rlw_autosave_enabled(const RlwAutosave *a) { return a->interval_frames > 0; }
int rlw_autosave_due(const RlwAutosave *a) { return rlw_autosave_enabled(a) && a->elapsed >= a->interval_frames; }
int rlw_autosave_frame(RlwAutosave *a, int eligible) {
 if (!rlw_autosave_enabled(a) || !eligible) return 0;
 if (a->elapsed < a->interval_frames) a->elapsed++;
 return rlw_autosave_due(a);
}
void rlw_autosave_set_interval(RlwAutosave *a, uint64_t interval_frames) {
 a->interval_frames = interval_frames;
 if (!interval_frames) a->elapsed = 0; else if (a->elapsed > interval_frames) a->elapsed = interval_frames;
}
void rlw_autosave_saved(RlwAutosave *a) { a->elapsed = 0; }
void rlw_autosave_failed(RlwAutosave *a) { a->elapsed = 0; }
void rlw_autosave_restart(RlwAutosave *a) { a->elapsed = 0; }
