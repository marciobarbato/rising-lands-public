/* Original sound banks (front 2, audio), portable C.
 *
 * The CD's INSTALHD/SOUND/{A,B,P}.HED + .SND, imported unchanged into
 * <Dados>/sound/. A HED is "ABCD", body size, data size, entry count, then the
 * body: count x 12-byte entries (id u32, word u16, variants u16, first data
 * header u32) and 16-byte data headers (SND offset u32, size u32, channels u8,
 * bits u8, rate u16, format u8, byte 0x0d, block u16). Format 4 is the
 * original's IMA ADPCM (imaadpcmDecode4Bit_M16 0x48b8c9, mono 16-bit), format 0
 * plain PCM. The macOS app imports the same samples as WAVs through
 * NativeCDImport.imaMono16; this decodes them in memory from the same bytes.
 *
 * Sample keys are MacOSAudioMapper's: bank index (0 A, 1 B, 2 P) << 24 | the
 * data header's offset in the HED body. */
#ifndef RLW_SOUND_H
#define RLW_SOUND_H
#include <stddef.h>
#include <stdint.h>

typedef struct RlwSoundBanks RlwSoundBanks;
typedef struct { int16_t *pcm; uint32_t frames, rate; } RlwSample;  /* mono PCM16 */

/* Bank selectors of PlaySnd_PostSound per bank index. */
extern const uint32_t RLW_SOUND_SELECTORS[3];
/* SHA-256 of the six original files (private research repository). */
typedef struct { const char *name; uint32_t size; const char *sha256; } RlwSoundFile;
extern const RlwSoundFile RLW_SOUND_FILES[6];

/* Reads <folder>/{A,B,P}.{HED,SND}. NULL (with a PT-BR reason) when a file is
 * missing or not a valid bank. Samples are decoded once, on first use. */
RlwSoundBanks *rlw_sound_banks_open(const char *folder, char *error, size_t capacity);
/* The same over bytes in memory (tests); copies them. */
RlwSoundBanks *rlw_sound_banks_from_memory(const uint8_t *const hed[3], const uint32_t hed_length[3],
                                           const uint8_t *const snd[3], const uint32_t snd_length[3]);
void rlw_sound_banks_free(RlwSoundBanks *b);
/* Exact HED bytes of a bank (for macos_audio_load_bank). */
const uint8_t *rlw_sound_banks_hed(const RlwSoundBanks *b, unsigned bank, uint32_t *length);
/* Decoded sample for a mapper key; NULL when the key names no data header or
 * the data is out of range. The sample stays owned by the banks. */
const RlwSample *rlw_sound_sample(RlwSoundBanks *b, uint32_t key);
/* Every data header of a bank, in body order: calls fn(bank, entry, variant, key). */
void rlw_sound_each(const RlwSoundBanks *b, void (*fn)(void *ctx, unsigned bank, uint32_t entry, uint32_t variant, uint32_t key), void *ctx);

/* imaadpcmDecode4Bit_M16 0x48b8c9 (NativeCDImport.imaMono16): blocks of
 * min(remaining, block) bytes while at least 4 remain; header i16 predictor,
 * u8 step index (above 88 stops), then low and high nibble of each byte.
 * Writes at most capacity samples; returns the count the full decode has, or
 * -1 when block < 4 with data to decode. */
int64_t rlw_sound_ima_mono16(const uint8_t *source, uint32_t length, uint32_t block, int16_t *out, uint64_t capacity);
#endif
