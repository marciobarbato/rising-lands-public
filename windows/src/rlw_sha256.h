/* Small portable SHA-256 (FIPS 180-4) for the Windows host's CD import check.
 * Host transport only; no game data. */
#ifndef RLW_SHA256_H
#define RLW_SHA256_H
#include <stddef.h>
#include <stdint.h>
typedef struct { uint32_t h[8]; uint64_t length; uint8_t block[64]; uint32_t used; } RlwSha256;
void rlw_sha256_init(RlwSha256 *c);
void rlw_sha256_update(RlwSha256 *c, const void *data, size_t length);
void rlw_sha256_final(RlwSha256 *c, uint8_t out[32]);
void rlw_sha256(const void *data, size_t length, uint8_t out[32]);
/* Lower-case hex of 32 bytes into out[65]. */
void rlw_sha256_hex(const uint8_t digest[32], char out[65]);
#endif
