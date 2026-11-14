/* Shift+click order queue markers: reader and layout (see rlw_markers.h). */
#include "rlw_markers.h"
#include <string.h>

const uint32_t rlw_marker_addresses[RLW_MARKER_RANGES] = {0x4e2f78u, 0x508041u, 0x508720u, 0x508728u};
const uint32_t rlw_marker_sizes[RLW_MARKER_RANGES] = {1u, 1u, 8u, 4u * 512u};
#define MARKER_BYTES (1 + 1 + 8 + 4 * 512)

static int u16(const uint8_t *b, int o) { return (int)b[o] | (int)b[o + 1] << 8; }
static int round_half(double v) { return (int)(v < 0 ? v - 0.5 : v + 0.5); }

void rlw_markers_clear(RlwMarkers *m) { if (m) memset(m, 0, sizeof(*m)); }

int rlw_markers_equal(const RlwMarkers *a, const RlwMarkers *b) {
 if (a->path_count != b->path_count || a->map_width != b->map_width || a->map_height != b->map_height) return 0;
 for (int p = 0; p < a->path_count; p++) {
  const RlwMarkerPath *x = &a->paths[p], *y = &b->paths[p];
  if (x->count != y->count) return 0;
  for (int k = 0; k < x->count; k++) if (x->points[k].x != y->points[k].x || x->points[k].y != y->points[k].y) return 0;
 }
 return 1;
}

int rlw_markers_any_queued(RlwOrderQueue queue, void *ctx) {
 for (uint32_t unit = 0; unit < RLW_MARKER_UNIT_LIMIT; unit++) {
  uint32_t count = 0;
  if (queue(ctx, unit, NULL, 0, &count) == 0 && count > 0) return 1;
 }
 return 0;
}

int rlw_markers_read(RlwPeek peek, RlwOrderQueue queue, void *ctx, RlwMarkers *out) {
 uint8_t b[MARKER_BYTES];
 rlw_markers_clear(out);
 if (peek(ctx, rlw_marker_addresses, rlw_marker_sizes, RLW_MARKER_RANGES, b)) return 1;
 if (b[0] != 0x12) return 0;
 int local = b[1] & 3, vx = u16(b, 2), vy = u16(b, 4), vw = u16(b, 6), vh = u16(b, 8);
 int list = 10 + 512 * local, count = u16(b, list);
 if (count > RLW_MARKER_MAX_UNITS) count = RLW_MARKER_MAX_UNITS;
 out->map_width = vw * 32; out->map_height = vh * 32;
 for (int k = 0; k < count; k++) {
  uint32_t unit = (uint32_t)u16(b, list + 2 + 2 * k), total = 0;
  uint16_t cells[RLW_MARKER_MAX_POINTS];
  if (unit >= RLW_MARKER_UNIT_LIMIT || queue(ctx, unit, cells, RLW_MARKER_MAX_POINTS, &total) || !total) continue;
  RlwMarkerPath *path = &out->paths[out->path_count++];
  path->count = total < RLW_MARKER_MAX_POINTS ? (int)total : RLW_MARKER_MAX_POINTS;
  for (int i = 0; i < path->count; i++) {
   path->points[i].x = ((int)(cells[i] & 255) - vx) * 32 + 16;
   path->points[i].y = ((int)(cells[i] >> 8) - vy) * 32 + 16;
  }
 }
 return 0;
}

static int push(RlwMarkerShape *out, int n, int capacity, int kind, int x0, int y0, int x1, int y1) {
 if (n < capacity) { out[n].kind = kind; out[n].x0 = x0; out[n].y0 = y0; out[n].x1 = x1; out[n].y1 = y1; return n + 1; }
 return n;
}

int rlw_markers_layout(const RlwMarkers *m, int left, int top, int width, int height,
                       RlwMarkerShape *out, int capacity, RlwMarkerRect *clip) {
 RlwMarkerRect empty = {0, 0, 0, 0};
 if (clip) *clip = empty;
 if (!m || m->path_count <= 0 || width <= 0 || height <= 0) return 0;
 double sx = width / 800.0, sy = height / 600.0, flag = 5 * sx < 3 ? 3 : 5 * sx;
 RlwMarkerRect map = {left, top, left + round_half(m->map_width * sx), top + round_half(m->map_height * sy)};
 if (map.right > left + width) map.right = left + width;
 if (map.bottom > top + height) map.bottom = top + height;
 if (map.right <= map.left || map.bottom <= map.top) return 0;
 if (clip) *clip = map;
 int n = 0, half = round_half(flag / 2), pole = round_half(2.2 * flag), flag_top = pole, flag_bottom = round_half(1.2 * flag),
     flag_width = round_half(1.2 * flag);
 for (int p = 0; p < m->path_count; p++) {
  const RlwMarkerPath *path = &m->paths[p];
  int px[RLW_MARKER_MAX_POINTS], py[RLW_MARKER_MAX_POINTS];
  for (int k = 0; k < path->count && k < RLW_MARKER_MAX_POINTS; k++) {
   px[k] = left + round_half(path->points[k].x * sx); py[k] = top + round_half(path->points[k].y * sy);
  }
  for (int k = 1; k < path->count && k < RLW_MARKER_MAX_POINTS; k++)
   n = push(out, n, capacity, RLW_SHAPE_DASH, px[k - 1], py[k - 1], px[k], py[k]);
  for (int k = 0; k < path->count && k < RLW_MARKER_MAX_POINTS; k++) {
   n = push(out, n, capacity, RLW_SHAPE_LINE, px[k], py[k], px[k], py[k] - pole);
   n = push(out, n, capacity, RLW_SHAPE_RECT, px[k], py[k] - flag_top, px[k] + flag_width, py[k] - flag_bottom);
   n = push(out, n, capacity, RLW_SHAPE_ELLIPSE, px[k] - half, py[k] - half, px[k] + half, py[k] + half);
  }
 }
 return n;
}

int rlw_markers_pen_width(unsigned dpi) {
 int w = (int)((3u * dpi + 96u) / 192u);
 return w < 1 ? 1 : w;
}
