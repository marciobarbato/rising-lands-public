#include "rlw_icon.h"
#include "rlw_fs.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

enum { RT_ICON_TYPE = 3, RT_GROUP_ICON_TYPE = 14 };
typedef struct { const uint8_t *b; size_t n; uint32_t sections, section_table, rsrc; size_t rsrc_off, rsrc_end; } Pe;

static int u16(const Pe *p, size_t o, uint32_t *v) {
 if (o + 2 > p->n) return -1;
 *v = p->b[o] | (uint32_t)p->b[o + 1] << 8; return 0;
}
static int u32(const Pe *p, size_t o, uint32_t *v) {
 if (o + 4 > p->n) return -1;
 *v = p->b[o] | (uint32_t)p->b[o + 1] << 8 | (uint32_t)p->b[o + 2] << 16 | (uint32_t)p->b[o + 3] << 24; return 0;
}
/* RVA -> file offset; a section spans max(VirtualSize, SizeOfRawData) in memory
 * (old linkers leave VirtualSize 0) but only its raw bytes exist in the file. */
static int rva_offset(const Pe *p, uint32_t rva, uint32_t size, size_t *out) {
 for (uint32_t i = 0; i < p->sections; i++) {
  size_t s = p->section_table + 40u * i; uint32_t vsize, va, raw_size, raw;
  if (u32(p, s + 8, &vsize) || u32(p, s + 12, &va) || u32(p, s + 16, &raw_size) || u32(p, s + 20, &raw)) return -1;
  uint32_t span = vsize > raw_size ? vsize : raw_size;
  if (rva < va || (uint64_t)rva >= (uint64_t)va + span) continue;
  uint64_t start = (uint64_t)raw + (rva - va);
  if ((uint64_t)(rva - va) + size > raw_size || start + size > p->n) return -1;
  *out = (size_t)start; return 0;
 }
 return -1;
}
static int open_pe(Pe *p, const uint8_t *b, size_t n) {
 memset(p, 0, sizeof(*p)); p->b = b; p->n = n;
 uint32_t lfanew, sig, sections, optional, magic, count, rva, size;
 if (n < 64 || b[0] != 'M' || b[1] != 'Z' || u32(p, 0x3c, &lfanew) || u32(p, lfanew, &sig) || sig != 0x00004550) return -1;
 if (u16(p, lfanew + 6, &sections) || u16(p, lfanew + 20, &optional) || u16(p, lfanew + 24, &magic) || sections == 0 || sections > 96) return -1;
 size_t dirs = lfanew + 24 + (magic == 0x10b ? 96u : magic == 0x20b ? 112u : 0u), count_at = dirs - 4;
 if (dirs == lfanew + 24 || u32(p, count_at, &count) || count < 3 || dirs + 8 * 3 > lfanew + 24 + (size_t)optional) return -1;
 if (u32(p, dirs + 16, &rva) || u32(p, dirs + 20, &size) || !rva || size < 16) return -1;
 p->sections = sections; p->section_table = lfanew + 24 + optional; p->rsrc = rva;
 if ((uint64_t)p->section_table + 40u * sections > n) return -1;
 /* The directory may be smaller than declared in the file; bound it by the raw section. */
 size_t off; uint32_t usable = size;
 while (usable >= 16 && rva_offset(p, rva, usable, &off)) usable /= 2;
 if (usable < 16) return -1;
 p->rsrc_off = off; p->rsrc_end = off + usable; return 0;
}
/* One directory level: the entry with this id (any_entry: the first one,
 * named or numbered). Returns its offset field, or -1. */
static int64_t entry(const Pe *p, size_t dir, uint32_t id, int any_entry) {
 uint32_t named, ids, name, value;
 if (dir + 16 > p->rsrc_end || u16(p, dir + 12, &named) || u16(p, dir + 14, &ids)) return -1;
 for (uint32_t i = 0; i < named + ids; i++) {
  size_t e = dir + 16 + 8u * i;
  if (e + 8 > p->rsrc_end || u32(p, e, &name) || u32(p, e + 4, &value)) return -1;
  if (any_entry || (!(name & 0x80000000u) && name == id)) return value;
 }
 return -1;
}
static size_t subdir(const Pe *p, int64_t value) { return value >= 0 && (value & 0x80000000) ? p->rsrc_off + (size_t)(value & 0x7fffffff) : 0; }
/* type/name(or first)/first language -> data bytes. */
static int resource(const Pe *p, uint32_t type, uint32_t name, int first_name, const uint8_t **data, uint32_t *size) {
 size_t names = subdir(p, entry(p, p->rsrc_off, type, 0));
 if (!names) return -1;
 size_t languages = subdir(p, entry(p, names, name, first_name));
 if (!languages) return -1;
 int64_t leaf = entry(p, languages, 0, 1);
 if (leaf < 0 || (leaf & 0x80000000)) return -1;
 size_t d = p->rsrc_off + (size_t)leaf; uint32_t rva, len; size_t off;
 if (d + 16 > p->rsrc_end || u32(p, d, &rva) || u32(p, d + 4, &len) || !len || len > 16u * 1024u * 1024u || rva_offset(p, rva, len, &off)) return -1;
 *data = p->b + off; *size = len; return 0;
}

int rlw_icon_from_pe(const uint8_t *image, size_t n, uint8_t **out, size_t *out_size) {
 *out = NULL; *out_size = 0;
 Pe p; const uint8_t *group; uint32_t group_size;
 if (open_pe(&p, image, n) || resource(&p, RT_GROUP_ICON_TYPE, 0, 1, &group, &group_size) || group_size < 6) return -1;
 uint32_t count = group[4] | (uint32_t)group[5] << 8;
 if (group[2] != 1 || group[3] != 0 || !count || count > 64 || 6 + 14u * count > group_size) return -1;
 const uint8_t *images[64]; uint32_t sizes[64]; size_t total = 6 + 16u * count;
 for (uint32_t i = 0; i < count; i++) {
  const uint8_t *e = group + 6 + 14u * i; uint32_t id = e[12] | (uint32_t)e[13] << 8;
  if (resource(&p, RT_ICON_TYPE, id, 0, &images[i], &sizes[i])) return -1;
  total += sizes[i];
 }
 uint8_t *ico = malloc(total);
 if (!ico) return -1;
 memcpy(ico, group, 6); size_t at = 6 + 16u * count;
 for (uint32_t i = 0; i < count; i++) {
  const uint8_t *e = group + 6 + 14u * i; uint8_t *d = ico + 6 + 16u * i;
  memcpy(d, e, 8); /* width, height, colours, reserved, planes, bit count */
  for (unsigned k = 0; k < 4; k++) { d[8 + k] = (uint8_t)(sizes[i] >> (8 * k)); d[12 + k] = (uint8_t)(at >> (8 * k)); }
  memcpy(ico + at, images[i], sizes[i]); at += sizes[i];
 }
 *out = ico; *out_size = total; return 0;
}

int rlw_icon_prepare(const char *data_root, char *ico_path, size_t capacity, int *created) {
 *created = 0; char exe[4200];
 if (!data_root || !data_root[0] || snprintf(ico_path, capacity, "%s" RLW_SEP "rising.ico", data_root) >= (int)capacity) return -1;
 if (rlw_exists(ico_path)) return 0;
 if (snprintf(exe, sizeof(exe), "%s" RLW_SEP "Dados" RLW_SEP "RISING.EXE", data_root) >= (int)sizeof(exe)) return -1;
 uint32_t n = 0; uint8_t *image = rlw_read_file(exe, &n, 16u * 1024u * 1024u);
 if (!image) return -1;
 uint8_t *ico = NULL; size_t size = 0; int c = rlw_icon_from_pe(image, n, &ico, &size); free(image);
 if (!c) c = rlw_durable_write(ico_path, ico, size, NULL, 0) ? -1 : 0;
 free(ico);
 if (!c) *created = 1;
 return c;
}
