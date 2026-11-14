/* Saved games as files in the save folder, portable C (no Win32; unit-tested on
 * any host). Port of macos/src/NativeSaveLibrary.swift, the
 * logic behind Partida > Gerenciar partidas salvas… (rlw_savemgr_dialog.c).
 *
 * A slot is "<slot>.rls" (the engine's save bytes) plus its presentation
 * companion "<slot>.rls.<sha256 of the .rls>.view". The named slots
 * posicao-1..posicao-10 are the original lists' save1.sav..save9.sav,save0.sav;
 * their names live in the host file nomes.plist (rlw_saves.h), never in the
 * save, so renaming needs no format or engine change. "rapido" (quick save),
 * "inicio-fase" (stage-start autosave) and "autosave" (periodic autosave,
 * rlw_autosave.h) have fixed names. The host never reads game state here. */
#ifndef RLW_SAVEMGR_H
#define RLW_SAVEMGR_H
#include <stddef.h>
#include <stdint.h>
#include "rlw_saves.h"

enum { RLW_SAVE_NAMED = 0, RLW_SAVE_QUICK = 1, RLW_SAVE_STAGE = 2, RLW_SAVE_PERIODIC = 3 };
#define RLW_SAVEMGR_MAX_VIEWS 4
#define RLW_SAVEMGR_MAX_GAMES 13
#define RLW_SAVEMGR_NAME_CHARACTERS 80   /* NativeSaveLibrary.maximumNameLength */
#define RLW_SAVEMGR_VISIBLE_CHARACTERS 17 /* what the original lists show before the date */

typedef struct {
 int kind;                     /* RLW_SAVE_* */
 unsigned number;              /* 1..10 for RLW_SAVE_NAMED, else 0 */
 char slot[32];                /* "posicao-3", "rapido", "inicio-fase", "autosave" */
 char file[4200];              /* <directory>/<slot>.rls */
 char views[RLW_SAVEMGR_MAX_VIEWS][4300]; unsigned view_count; /* companions, sorted, no temporaries */
 int64_t mtime;                /* seconds since 1970 (UTC) */
 char name[RLW_SAVE_NAME_CAPACITY];
 char mission[16];             /* "MONDE07B.ETA" from the companion; "" when unreadable */
 uint32_t difficulty;          /* 0..2, valid when mission[0] */
} RlwSavedGame;
typedef struct { RlwSavedGame game[RLW_SAVEMGR_MAX_GAMES]; unsigned count; } RlwSaveList;

/* Existing slots: posicao-1..10 in order, then rapido, inicio-fase and autosave
 * (the order of Partida > Carregar partida). */
void rlw_savemgr_list(const char *directory, RlwSaveList *out);
/* "Posição N", "Salvamento rápido", "Início da fase · salvamento automático",
 * "Salvamento automático". */
const char *rlw_savemgr_default_name(int kind, unsigned number, char *out, size_t capacity);
int rlw_savemgr_can_rename(const RlwSavedGame *game);
/* "Fase 7 · Normal", "Fase 7B · Difícil" or "Fase desconhecida". */
void rlw_savemgr_stage_title(const RlwSavedGame *game, char *out, size_t capacity);
/* Local "dd/MM/aaaa HH:mm". */
void rlw_savemgr_date(int64_t mtime, char *out, size_t capacity);
/* Mission and difficulty of an RLVIEW3/RLVIEW4 companion; 0 or -1. */
int rlw_savemgr_view_stage(const char *view, char mission[16], uint32_t *difficulty);
/* Trims whitespace/newlines, keeps the first 80 characters (code points).
 * 0, or -1 for an empty name (message in `error`). */
int rlw_savemgr_valid_name(const char *text, char *out, size_t capacity, char *error, size_t error_capacity);
/* The name the original save and load lists show for posicao-N (nomes.plist).
 * 0, or -1 with a player-facing message. */
int rlw_savemgr_rename(const char *directory, const RlwSavedGame *game, const char *text, char *error, size_t error_capacity);
/* Moves one file to the Recycle Bin (never a permanent delete). 0, or nonzero
 * with `reason`. Windows: SHFileOperationW + FOF_ALLOWUNDO; tests inject a fake. */
typedef int (*RlwTrashFn)(void *ctx, const char *path, char *reason, size_t reason_capacity);
/* The .rls first (the slot leaves every list), then its companions; a named
 * slot also loses its name. 0, or -1 with a player-facing message. */
int rlw_savemgr_trash(const char *directory, const RlwSavedGame *game, RlwTrashFn trash, void *ctx, char *error, size_t error_capacity);
/* The manager stays closed while an original save/load list (RL_SHELL_LOAD_LIST
 * 12, RL_SHELL_LOAD_MENU 13, RL_SHELL_SAVE_LIST 14) is shown: those lists read
 * the captions when they open (AppDelegate.originalSaveListModes). */
int rlw_savemgr_available(uint32_t shell_mode);
#endif
