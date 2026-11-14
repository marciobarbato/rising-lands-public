/* Original movies for the Windows host (front 2), portable C. Port of
 * NativeDefeatMovie.swift: EVENTS\OVER_FR.AVI, EXTROFR.AVI and INTROFR.AVI are
 * Cinepak 640x480 video at 15 frames/s with Microsoft ADPCM 22,050 Hz audio.
 * The host decodes them itself (no system codec: a 64-bit Cinepak codec is not
 * on every Windows). Decoding is presentation only: the engine owns the
 * request and the return to MenuInit 2 (rlw_shell_movie_finish).
 *
 * PlayVideo 0x47c82c: a movie that cannot be opened is "unavailable"; otherwise
 * it plays to its end, or until the Escape key-up (0x47c96e), and is
 * "finished". Mouse input does not end it. */
#ifndef RLW_MOVIE_H
#define RLW_MOVIE_H
#include <stddef.h>
#include <stdint.h>

/* ---- Codecs ---- */
typedef struct { uint32_t tag, channels, rate, block_align, bits, samples_per_block, coefficient_count; int32_t coefficients[64]; } RlwAudioFormat;
/* WAVE_FORMAT_ADPCM (tag 2), 4 bits, mono or stereo: interleaved PCM16 into out
 * (NULL = count only). Returns the sample count (all channels) or -1 (bad predictor). */
int64_t rlw_msadpcm_decode(const uint8_t *bytes, size_t length, const RlwAudioFormat *format, int16_t *out, uint64_t capacity);

typedef struct RlwCinepak RlwCinepak;
RlwCinepak *rlw_cinepak_create(uint32_t width, uint32_t height);
void rlw_cinepak_free(RlwCinepak *c);
/* One "cvid" frame; codebooks persist per strip across frames. 0 or 1 (error text). */
int rlw_cinepak_decode(RlwCinepak *c, const uint8_t *frame, size_t length, char *error, size_t capacity);
const uint8_t *rlw_cinepak_pixels(const RlwCinepak *c);   /* RGBX, 4 bytes per pixel, X = 255 */

/* ---- Movie ---- */
typedef struct RlwMovie RlwMovie;
/* Reads and parses the AVI and decodes its whole audio track. NULL with a PT-BR reason. */
RlwMovie *rlw_movie_open(const char *path, char *error, size_t capacity);
RlwMovie *rlw_movie_from_memory(uint8_t *data, size_t length, char *error, size_t capacity);  /* takes data */
void rlw_movie_free(RlwMovie *m);
uint32_t rlw_movie_width(const RlwMovie *m);
uint32_t rlw_movie_height(const RlwMovie *m);
uint32_t rlw_movie_frames(const RlwMovie *m);
uint32_t rlw_movie_decoded(const RlwMovie *m);
/* max(video duration, audio duration), as the macOS player waits for both. */
double rlw_movie_duration_ms(const RlwMovie *m);
/* Decodes up to the frame due at elapsed_ms. 1 = a new picture, 0 = none,
 * -1 = decoding failed (rlw_movie_error). */
int rlw_movie_advance(RlwMovie *m, double elapsed_ms);
const uint8_t *rlw_movie_pixels(const RlwMovie *m);       /* RGBX, width x height */
const char *rlw_movie_error(const RlwMovie *m);
/* Audio: rate (0 = none), and a RlwMusicPull of stereo PCM16 frames. */
uint32_t rlw_movie_audio_rate(const RlwMovie *m);
uint64_t rlw_movie_audio_frames(const RlwMovie *m);
uint32_t rlw_movie_pull(void *movie, int16_t *stereo, uint32_t frames);
/* The decoded audio as the stream holds it (interleaved), and the frame period. */
const int16_t *rlw_movie_pcm(const RlwMovie *m, uint64_t *samples);
double rlw_movie_frame_ms(const RlwMovie *m);
#endif
