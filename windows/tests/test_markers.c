/* Portable unit tests (no game data, no Win32) for the Shift+click order queue
 * markers (rlw_markers.c): the reader over a synthetic memory map through the
 * rlw_driver_peek contract and a fake rl_fila_ordens, and the window layout.
 * The reader case is the macOS ModernHotkeysTests.orderQueueMarkers case. */
#include "rlw_markers.h"
#include <stdio.h>
#include <string.h>

static int failures;
#define CHECK(c) do { if (!(c)) { fprintf(stderr, "%s:%d: CHECK(%s)\n", __FILE__, __LINE__, #c); failures++; } } while (0)

static uint8_t memory[1 + 1 + 8 + 2048];
static int peeks, refuse_peek;
static int fake_peek(void *ctx, const uint32_t *a, const uint32_t *s, uint32_t n, uint8_t *out) {
 (void)ctx; peeks++;
 if (refuse_peek || n != RLW_MARKER_RANGES) return 1;
 for (uint32_t i = 0; i < n; i++) if (a[i] != rlw_marker_addresses[i] || s[i] != rlw_marker_sizes[i]) return 1;
 memcpy(out, memory, sizeof(memory)); return 0;
}

/* Fake rl_fila_ordens: unit -> cells (y*256+x). */
static struct { uint32_t unit; int count; uint16_t cells[12]; } queues[4];
static int queue_count, queue_calls;
static int fake_queue(void *ctx, uint32_t unit, uint16_t *cells, uint32_t capacity, uint32_t *count) {
 (void)ctx; queue_calls++; *count = 0;
 if (unit >= 800) return 1;
 for (int q = 0; q < queue_count; q++) if (queues[q].unit == unit) {
  *count = (uint32_t)queues[q].count;
  for (uint32_t i = 0; i < capacity && i < (uint32_t)queues[q].count; i++) cells[i] = queues[q].cells[i];
 }
 return 0;
}
static uint16_t cell(int x, int y) { return (uint16_t)(y << 8 | x); }

static void setup(void) {
 memset(memory, 0, sizeof(memory));
 memory[0] = 0x12; memory[1] = 2;
 memory[2] = 10; memory[4] = 20; memory[6] = 25; memory[8] = 17;
 int list = 10 + 512 * 2;
 memory[list] = 3; memory[list + 2] = 7; memory[list + 4] = 9; memory[list + 6] = 0x10; memory[list + 7] = 0x01;
 memset(queues, 0, sizeof(queues)); queue_count = 2;
 queues[0].unit = 7; queues[0].count = 2; queues[0].cells[0] = cell(11, 21); queues[0].cells[1] = cell(40, 30);
 queues[1].unit = 0x110; queues[1].count = 1; queues[1].cells[0] = cell(10, 21);
 peeks = 0; refuse_peek = 0; queue_calls = 0;
}

static void test_reader(void) {
 static RlwMarkers m, again;
 setup();
 CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &m) == 0);
 CHECK(peeks == 1);                                   /* one snapshot per refresh */
 CHECK(m.map_width == 800 && m.map_height == 544);    /* map region from the view size */
 CHECK(m.path_count == 2);                            /* unit 9 has no waypoints */
 CHECK(m.paths[0].count == 2 && m.paths[0].points[0].x == 48 && m.paths[0].points[0].y == 48);
 CHECK(m.paths[0].points[1].x == 976 && m.paths[0].points[1].y == 336);
 CHECK(m.paths[1].count == 1 && m.paths[1].points[0].x == 16 && m.paths[1].points[0].y == 48);
 CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &again) == 0 && rlw_markers_equal(&m, &again));
 again.paths[1].points[0].y++; CHECK(!rlw_markers_equal(&m, &again));
 /* Another local player reads its own list (empty here). */
 memory[1] = 1; CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &m) == 0 && m.path_count == 0 && m.map_width == 800);
 memory[1] = 2 | 4; CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &m) == 0 && m.path_count == 2); /* & 3 */
 /* No markers outside a mission; a refused peek fails with nothing to draw. */
 memory[0] = 0x16; CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &m) == 0 && m.path_count == 0 && m.map_width == 0);
 memory[0] = 0x12; refuse_peek = 1; CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &m) != 0 && m.path_count == 0);
 refuse_peek = 0;
 /* At most 8 waypoints per unit and 64 units; unit indices >= 800 are not asked. */
 queue_count = 1; queues[0].count = 12;
 for (int i = 0; i < 12; i++) queues[0].cells[i] = cell(10 + i, 20);
 int list = 10 + 512 * 2; memory[list] = 200; memory[list + 1] = 0;
 for (int k = 0; k < 200; k++) { memory[list + 2 + 2 * k] = 7; memory[list + 3 + 2 * k] = 0; }
 memory[list + 2] = 0x40; memory[list + 3] = 0x03; /* unit 832 */
 queue_calls = 0;
 CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &m) == 0);
 CHECK(m.path_count == RLW_MARKER_MAX_UNITS - 1 && queue_calls == RLW_MARKER_MAX_UNITS - 1);
 CHECK(m.paths[0].count == RLW_MARKER_MAX_POINTS && m.paths[0].points[7].x == 7 * 32 + 16);
 /* A unit whose queue call fails is skipped. */
 memory[list + 4] = 0x20; memory[list + 5] = 0x03; /* 800 */
 CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &m) == 0 && m.path_count == RLW_MARKER_MAX_UNITS - 2);
}

static void test_any_queued(void) {
 setup();
 CHECK(rlw_markers_any_queued(fake_queue, NULL));
 queue_count = 0; queue_calls = 0;
 CHECK(!rlw_markers_any_queued(fake_queue, NULL) && queue_calls == 800);
 queue_count = 1; queues[0].unit = 799; queues[0].count = 1;
 CHECK(rlw_markers_any_queued(fake_queue, NULL));
}

static int count_kind(const RlwMarkerShape *s, int n, int kind) { int c = 0; for (int i = 0; i < n; i++) c += s[i].kind == kind; return c; }

static void test_layout(void) {
 static RlwMarkers m; static RlwMarkerShape shapes[RLW_MARKER_SHAPES]; RlwMarkerRect clip;
 setup(); CHECK(rlw_markers_read(fake_peek, fake_queue, NULL, &m) == 0);
 /* 1:1 at the window origin: the map region is the view's 800x544. */
 int n = rlw_markers_layout(&m, 0, 0, 800, 600, shapes, RLW_MARKER_SHAPES, &clip);
 CHECK(n == 1 + 3 * 3);
 CHECK(clip.left == 0 && clip.top == 0 && clip.right == 800 && clip.bottom == 544);
 CHECK(count_kind(shapes, n, RLW_SHAPE_DASH) == 1 && count_kind(shapes, n, RLW_SHAPE_LINE) == 3);
 CHECK(count_kind(shapes, n, RLW_SHAPE_RECT) == 3 && count_kind(shapes, n, RLW_SHAPE_ELLIPSE) == 3);
 /* Dash between the first unit's waypoints, then its flags: flag 5 px, pole 11 px up,
  * flag rect from 11 to 6 px above the point, 6 px wide, dot 5 px across (rounded). */
 CHECK(shapes[0].kind == RLW_SHAPE_DASH && shapes[0].x0 == 48 && shapes[0].y0 == 48 && shapes[0].x1 == 976 && shapes[0].y1 == 336);
 CHECK(shapes[1].kind == RLW_SHAPE_LINE && shapes[1].x0 == 48 && shapes[1].y0 == 48 && shapes[1].x1 == 48 && shapes[1].y1 == 37);
 CHECK(shapes[2].kind == RLW_SHAPE_RECT && shapes[2].x0 == 48 && shapes[2].y0 == 37 && shapes[2].x1 == 54 && shapes[2].y1 == 42);
 CHECK(shapes[3].kind == RLW_SHAPE_ELLIPSE && shapes[3].x0 == 45 && shapes[3].y0 == 45 && shapes[3].x1 == 51 && shapes[3].y1 == 51);
 /* Aspect-fit 2x with letterbox offsets (1600x1200 picture at 160,0). */
 n = rlw_markers_layout(&m, 160, 0, 1600, 1200, shapes, RLW_MARKER_SHAPES, &clip);
 CHECK(clip.left == 160 && clip.top == 0 && clip.right == 1760 && clip.bottom == 1088);
 CHECK(shapes[0].x0 == 160 + 96 && shapes[0].y0 == 96 && shapes[0].x1 == 160 + 1952 && shapes[0].y1 == 672);
 CHECK(shapes[1].y1 == 96 - 22 && shapes[2].x1 - shapes[2].x0 == 12 && shapes[3].x1 - shapes[3].x0 == 10);
 /* Small window: the flag never gets smaller than 3 px. */
 n = rlw_markers_layout(&m, 0, 0, 400, 300, shapes, RLW_MARKER_SHAPES, &clip);
 CHECK(clip.right == 400 && clip.bottom == 272 && shapes[1].y1 == 24 - 7);
 /* A map taller than the picture is clipped to it; capacity is respected. */
 m.map_height = 700; n = rlw_markers_layout(&m, 0, 0, 800, 600, shapes, 4, &clip);
 CHECK(n == 4 && clip.bottom == 600);
 /* Nothing to draw: no shapes, empty clip. */
 rlw_markers_clear(&m); n = rlw_markers_layout(&m, 0, 0, 800, 600, shapes, RLW_MARKER_SHAPES, &clip);
 CHECK(n == 0 && clip.right == 0 && clip.bottom == 0);
 CHECK(rlw_markers_pen_width(96) == 2 && rlw_markers_pen_width(192) == 3 && rlw_markers_pen_width(0) == 1);
}

int main(void) {
 test_reader(); test_any_queued(); test_layout();
 if (failures) { fprintf(stderr, "test_markers: %d failure(s)\n", failures); return 1; }
 printf("test_markers: ok (reader over a synthetic map, any-queued scan, aspect-fit layout)\n");
 return 0;
}
