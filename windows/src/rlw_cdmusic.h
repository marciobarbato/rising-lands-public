/* CD music for the Windows host (front 2, audio), portable C. Port of
 * NativeCDMusicDisc.swift, NativeCDMusicHost.swift and NativeCDMusicPlayer.swift.
 *
 * Disc: the player's raw BIN holds two Red Book tracks after the data track.
 * The CUE gives each track's INDEX 01; a track runs to the next track's INDEX 01
 * (its pregap included, as NativeCDMusicDisc's windows) or to the end of the
 * image. At import they are written as Dados/music/TRACK02.wav / TRACK03.wav
 * (44-byte RIFF header + the exact 44,100 Hz stereo PCM16), each checked against
 * the macOS player's pinned WAV identity. Nothing from the CD goes into git.
 *
 * Policy (NativeCDMusicHost): presentation only, never game state. While a
 * stage is active, the game's MCICD_Play requests (frame boundary 102 at
 * 0x42d32d or 0x4684bd, loop 0) play the requested original track once; a
 * 0x42d32d request while a track plays is suppressed; any other site or a loop
 * flag turns automatic selection off for the stage; a track that failed is not
 * retried in the stage. beginStage stops and re-arms, result keeps the current
 * track without new requests, leaveStage stops. */
#ifndef RLW_CDMUSIC_H
#define RLW_CDMUSIC_H
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

/* ---- Disc ---- */
typedef struct { uint32_t number, audio; uint64_t offset, length; } RlwCdTrack;
typedef struct { char file[260]; uint32_t count; RlwCdTrack tracks[99]; } RlwCdToc;
/* Parses CUE text (FILE/TRACK/INDEX 01, 2,352-byte sectors); lengths need the
 * image size. 0 or 1 (not a single-file CUE, bad syntax, offsets out of order). */
int rlw_cdmusic_parse_cue(const char *text, size_t length, uint64_t image_size, RlwCdToc *out);
const RlwCdTrack *rlw_cdmusic_track(const RlwCdToc *toc, uint32_t number);
/* The two pinned WAV identities (NativeCDMusicDisc / NativeCDMusicOriginal). */
typedef struct { uint32_t track; uint64_t offset, pcm_bytes; const char *sha256; } RlwCdMusicPin;
extern const RlwCdMusicPin RLW_CDMUSIC_PINS[2];
#define RLW_CDMUSIC_IMAGE_BYTES 705418896ull
/* Writes a track of an open image as a WAV at path: header, then PCM copied in
 * 1 MiB pieces. sha256_hex (65 bytes, may be NULL) receives the WAV's SHA-256.
 * 0 or 1. */
int rlw_cdmusic_write_wav(FILE *image, const RlwCdTrack *track, const char *path, char sha256_hex[65]);
/* The 44-byte header the macOS import writes (PCM, 2 channels, 44,100 Hz, 16 bits). */
void rlw_cdmusic_wav_header(uint8_t out[44], uint32_t pcm_bytes);

/* Import: from a .cue (its FILE) or a raw .bin of the Brazilian image's size,
 * writes Dados/music/TRACK02.wav and TRACK03.wav through a staging folder; each
 * WAV's SHA-256 must equal its pin. 0 done, 2 not a raw image (an .ISO or a
 * folder has no CD audio: skipped), 1 error with a PT-BR message. */
int rlw_cdmusic_import(const char *source, const char *data_root, char *error, size_t capacity);
/* 1 when Dados/music holds both track WAVs. */
int rlw_cdmusic_installed(const char *dados);
#ifdef RLW_TESTING
extern int rlw_cdmusic_test_ignore_pins;
#endif

/* ---- Player: one non-looping track streamed from its WAV ---- */
typedef struct RlwCdPlayer RlwCdPlayer;
/* folder: Dados/music. */
RlwCdPlayer *rlw_cdplayer_create(const char *folder);
void rlw_cdplayer_free(RlwCdPlayer *p);
/* Opens TRACKnn.wav (track 2 or 3), checking its size and header; 0 or 1. */
int rlw_cdplayer_open(RlwCdPlayer *p, uint32_t track);
void rlw_cdplayer_close(RlwCdPlayer *p);
/* RlwMusicPull: stereo frames from the open track; 0 at its end or when closed. */
uint32_t rlw_cdplayer_pull(void *player, int16_t *stereo, uint32_t frames);
uint32_t rlw_cdplayer_track(const RlwCdPlayer *p);   /* 0 when none is open */
int rlw_cdplayer_ended(const RlwCdPlayer *p);         /* the open track reached its end */

/* ---- Policy ---- */
enum { RLW_CDMUSIC_NONE = 0, RLW_CDMUSIC_PLAY = 1, RLW_CDMUSIC_STOP = 2 };
typedef struct {
 int stage_active, automatic, playing; uint32_t track, rejected; /* bit n: track n failed */
 uint64_t requests, suppressed, starts, finishes, failures;
 char notice[128];
} RlwCdMusic;
void rlw_cdmusic_init(RlwCdMusic *m);
/* Each returns the action for the player (PLAY uses m->track). */
int rlw_cdmusic_begin_stage(RlwCdMusic *m);
int rlw_cdmusic_result(RlwCdMusic *m);
int rlw_cdmusic_leave_stage(RlwCdMusic *m);
int rlw_cdmusic_request(RlwCdMusic *m, uint32_t site, uint32_t track, uint32_t loop);
/* Player answers. */
void rlw_cdmusic_started(RlwCdMusic *m, uint32_t track);
void rlw_cdmusic_failed(RlwCdMusic *m, uint32_t track);
void rlw_cdmusic_finished(RlwCdMusic *m);
#endif
