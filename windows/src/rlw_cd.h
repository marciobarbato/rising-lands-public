/* First-run import of the player's own Brazilian CD (Windows host).
 *
 * Port of the reading half of macos/src/NativeCDImport.swift:
 * an ISO 9660 reader over a raw 2,352-byte MODE1/MODE2 image (.BIN, via .CUE),
 * a 2,048-byte .ISO, or a folder holding the CD's files. Nothing derived from
 * the game is bundled; every imported byte is read from the player's CD here.
 *
 * The import writes <data>/Dados with the names the driver opens (the same
 * layout as the macOS native inputs): the ten mission blocks, EVENTS.INI,
 * asset0..5.bin = GRAPHICS/{BLOCS,TROOPS,VEHICULE,CREATURE,BUILDING,OTHERS}.CMP,
 * asset6.bin = PLATGAME.PIE, every .ETA of WORLD and events/NAME.FLC (EVENTS). */
#ifndef RLW_CD_H
#define RLW_CD_H
#include <stddef.h>
#include <stdint.h>

typedef struct RlwCd RlwCd;

/* path: a .cue, .bin, .iso or a folder that holds INSTALHD. */
RlwCd *rlw_cd_open(const char *path, char *error, size_t capacity);
void rlw_cd_close(RlwCd *cd);
/* 1 when the source is a raw 2,352-byte image (CD audio is only there). */
int rlw_cd_is_raw(const RlwCd *cd);
/* SHA-256 of the whole image file (raw/ISO sources only; -1 for folders). */
int rlw_cd_image_sha256(RlwCd *cd, uint8_t out[32]);
/* CD-relative path with '/' separators, case-insensitive ("INSTALHD/RISING.EXE"). */
uint8_t *rlw_cd_read(RlwCd *cd, const char *path, uint32_t *length);
/* Direct children (files only) of a CD directory: fn(name, size, ctx). */
int rlw_cd_list_files(RlwCd *cd, const char *directory,
                      int (*fn)(const char *name, uint32_t size, void *ctx), void *ctx);

/* SHA-256 of the Internet Archive RISING_LANDS_BR_.BIN (private research repository). */
#define RLW_CD_BIN_SHA256 "c2c09e87927cf5ebbe5d17ea17bffb53c82c2e68385e9259ad0453cc15ea616f"

typedef void (*RlwProgress)(void *ctx, double fraction, const char *message);
/* Imports into <data_root>/Dados atomically (staging folder renamed in only after
 * every file verified). Returns 0, or nonzero with a PT-BR message in error. */
int rlw_cd_import(const char *source, const char *data_root, int verify_image,
                  RlwProgress progress, void *ctx, char *error, size_t capacity);
/* 1 when <data_root>/Dados holds a complete import receipt and every input. */
int rlw_cd_installed(const char *data_root);
#endif
