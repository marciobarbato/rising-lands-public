/* Portable checks of the icon reader (rlw_icon.c) over a synthetic PE built
 * here (no game data): resource tree with a named RT_GROUP_ICON and its RT_ICON,
 * a section with VirtualSize 0 as old linkers write, refusals, and the .ico
 * written into a data folder once.
 *   test_icon <empty work folder> */
#include "rlw_fs.h"
#include "rlw_icon.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#ifdef _MSC_VER
#pragma comment(lib, "user32.lib")
#endif
#endif

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "test_icon: %s:%d: %s\n", __FILE__, __LINE__, #c); failures++; } } while (0)

static void w16(uint8_t *b, size_t o, uint32_t v) { b[o] = (uint8_t)v; b[o + 1] = (uint8_t)(v >> 8); }
static void w32(uint8_t *b, size_t o, uint32_t v) { for (unsigned i = 0; i < 4; i++) b[o + i] = (uint8_t)(v >> (8 * i)); }

enum { RAW = 0x200, VA = 0x1000, IMAGE = 744 }; /* a 32x32 4-bit icon DIB, as the CD's */
/* Resource section layout (offsets inside .rsrc). */
enum { L_ROOT = 0, L_ICON_NAMES = 32, L_ICON_LANGS = 56, L_GROUP_NAMES = 80, L_GROUP_LANGS = 104, L_ICON_DATA = 128, L_GROUP_DATA = 144,
       L_GROUP_NAME = 160, L_GROUP_BYTES = 176, L_IMAGE_BYTES = 200, L_RSRC_SIZE = 1024 };
static size_t make_pe(uint8_t *b, uint32_t icon_id, uint32_t group_points_to) {
 size_t n = RAW + L_RSRC_SIZE; memset(b, 0, n);
 b[0] = 'M'; b[1] = 'Z'; w32(b, 0x3c, 0x40);
 w32(b, 0x40, 0x00004550); w16(b, 0x44, 0x14c); w16(b, 0x46, 1); w16(b, 0x54, 224);
 size_t opt = 0x58; w16(b, opt, 0x10b); w32(b, opt + 92, 16); w32(b, opt + 96 + 16, VA); w32(b, opt + 96 + 20, L_RSRC_SIZE);
 size_t sec = opt + 224; memcpy(b + sec, ".rsrc", 5); w32(b, sec + 8, 0 /* VirtualSize 0 */); w32(b, sec + 12, VA); w32(b, sec + 16, L_RSRC_SIZE); w32(b, sec + 20, RAW);
 uint8_t *r = b + RAW;
 /* root: two id entries, RT_ICON (3) and RT_GROUP_ICON (14) */
 w16(r, L_ROOT + 14, 2); w32(r, L_ROOT + 16, 3); w32(r, L_ROOT + 20, 0x80000000u | L_ICON_NAMES); w32(r, L_ROOT + 24, 14); w32(r, L_ROOT + 28, 0x80000000u | L_GROUP_NAMES);
 w16(r, L_ICON_NAMES + 14, 1); w32(r, L_ICON_NAMES + 16, icon_id); w32(r, L_ICON_NAMES + 20, 0x80000000u | L_ICON_LANGS);
 w16(r, L_ICON_LANGS + 14, 1); w32(r, L_ICON_LANGS + 16, 0x409); w32(r, L_ICON_LANGS + 20, L_ICON_DATA);
 w16(r, L_GROUP_NAMES + 12, 1); w32(r, L_GROUP_NAMES + 16, 0x80000000u | L_GROUP_NAME); w32(r, L_GROUP_NAMES + 20, 0x80000000u | L_GROUP_LANGS);
 w16(r, L_GROUP_LANGS + 14, 1); w32(r, L_GROUP_LANGS + 16, 0x409); w32(r, L_GROUP_LANGS + 20, L_GROUP_DATA);
 w32(r, L_ICON_DATA, VA + L_IMAGE_BYTES); w32(r, L_ICON_DATA + 4, IMAGE);
 w32(r, L_GROUP_DATA, VA + L_GROUP_BYTES); w32(r, L_GROUP_DATA + 4, 20);
 w16(r, L_GROUP_NAME, 4); w16(r, L_GROUP_NAME + 2, 'I'); w16(r, L_GROUP_NAME + 4, 'C'); w16(r, L_GROUP_NAME + 6, 'O'); w16(r, L_GROUP_NAME + 8, 'N');
 uint8_t *g = r + L_GROUP_BYTES; w16(g, 2, 1); w16(g, 4, 1);
 g[6] = 32; g[7] = 32; g[8] = 16; w16(g, 10, 1); w16(g, 12, 4); w32(g, 14, IMAGE); w16(g, 18, group_points_to);
 /* BITMAPINFOHEADER (height doubled for the AND mask), 16-colour palette, XOR
  * pixels (a diagonal pattern), AND mask zero (opaque). */
 uint8_t *d = r + L_IMAGE_BYTES; w32(d, 0, 40); w32(d, 4, 32); w32(d, 8, 64); w16(d, 12, 1); w16(d, 14, 4); w32(d, 20, 512 + 128);
 for (unsigned c = 0; c < 16; c++) { d[40 + 4 * c] = (uint8_t)(c * 16); d[41 + 4 * c] = (uint8_t)(255 - c * 16); d[42 + 4 * c] = 0x80; }
 for (unsigned i = 0; i < 512; i++) d[104 + i] = (uint8_t)(((i / 16) + i) & 0xff);
 return n;
}

int main(int argc, char **argv) {
 if (argc < 2) { fprintf(stderr, "usage: test_icon <work folder>\n"); return 64; }
 static uint8_t pe[RAW + L_RSRC_SIZE]; size_t n = make_pe(pe, 1, 1);
 uint8_t *ico = NULL; size_t size = 0;
 CHECK(rlw_icon_from_pe(pe, n, &ico, &size) == 0);
 CHECK(ico && size == 6 + 16 + IMAGE);
 if (ico && size == 6 + 16 + IMAGE) {
  static const uint8_t head[22] = {0, 0, 1, 0, 1, 0, 32, 32, 16, 0, 1, 0, 4, 0, IMAGE & 0xff, IMAGE >> 8, 0, 0, 22, 0, 0, 0};
  CHECK(!memcmp(ico, head, 22)); CHECK(!memcmp(ico + 22, pe + RAW + L_IMAGE_BYTES, IMAGE));
 }
 free(ico);
 /* Refusals: not a PE, a group naming a missing icon, a truncated image, no resources. */
 CHECK(rlw_icon_from_pe((const uint8_t *)"MZ", 2, &ico, &size) == -1 && !ico);
 n = make_pe(pe, 1, 2); CHECK(rlw_icon_from_pe(pe, n, &ico, &size) == -1);
 n = make_pe(pe, 1, 1); CHECK(rlw_icon_from_pe(pe, RAW + L_IMAGE_BYTES + 10, &ico, &size) == -1); /* cut inside the image */
 w32(pe, 0x58 + 96 + 16, 0); CHECK(rlw_icon_from_pe(pe, n, &ico, &size) == -1);
 n = make_pe(pe, 1, 1); pe[0x40] = 'X'; CHECK(rlw_icon_from_pe(pe, n, &ico, &size) == -1);
 /* <data>/rising.ico from <data>/Dados/RISING.EXE, written once. */
 char root[4200], dados[4300], exe[4400], path[4400]; int created = 0;
 snprintf(root, sizeof(root), "%s" RLW_SEP "icone", argv[1]); rlw_remove_tree(root);
 snprintf(dados, sizeof(dados), "%s" RLW_SEP "Dados", root); CHECK(rlw_mkdirs(dados) == 0);
 CHECK(rlw_icon_prepare(root, path, sizeof(path), &created) == -1 && !created); /* no import */
 n = make_pe(pe, 1, 1); snprintf(exe, sizeof(exe), "%s" RLW_SEP "RISING.EXE", dados);
 CHECK(rlw_durable_write(exe, pe, n, NULL, 0) == 0);
 CHECK(rlw_icon_prepare(root, path, sizeof(path), &created) == 0 && created && strstr(path, "rising.ico"));
 uint32_t got = 0; uint8_t *file = rlw_read_file(path, &got, 8192); CHECK(file && got == 6 + 16 + IMAGE && file[18] == 22); free(file);
 CHECK(rlw_icon_prepare(root, path, sizeof(path), &created) == 0 && !created); /* already there: not rebuilt */
#ifdef _WIN32
 /* Smoke on Windows: the shell's own icon loader takes the file, 32 and 16 px. */
 wchar_t wide[4400]; MultiByteToWideChar(CP_UTF8, 0, path, -1, wide, 4400);
 HICON icon_big = (HICON)LoadImageW(NULL, wide, IMAGE_ICON, 32, 32, LR_LOADFROMFILE), icon_small = (HICON)LoadImageW(NULL, wide, IMAGE_ICON, 16, 16, LR_LOADFROMFILE);
 CHECK(icon_big != NULL); CHECK(icon_small != NULL);
 if (icon_big) DestroyIcon(icon_big);
 if (icon_small) DestroyIcon(icon_small);
 printf("test_icon: Windows LoadImageW took the .ico (32 and 16 px)\n");
#endif
 rlw_remove_tree(root);
 if (failures) return 1;
 printf("test_icon ok: PE resource icon group to .ico, refusals, data folder\n");
 return 0;
}
