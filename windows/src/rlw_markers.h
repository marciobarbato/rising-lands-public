/* Shift+click order queue markers of the Windows host. Port of
 * NativeOrderQueueMarkers (macos/src/NativeModernHotkeys.swift)
 * and of NativeLiveWindow.swift's stepOrderQueueMarkers/layoutWaypoints.
 *
 * With the engine rule RL_REGRA_FILA_ORDENS (bit 1 of regras, on with "Atalhos
 * modernos" for worlds created afterwards), a Shift+right click queues an order
 * after the current one. The host shows the pending waypoints of the local
 * player's selected units over the map part of the picture: a dashed line through
 * each unit's waypoints and, on every waypoint, a pole with a flag and a dot.
 * Read-only (one projection through rlw_driver_peek plus rl_fila_ordens per
 * unit), drawn by the host after the frame, never into the game image, a save or
 * a recording. Portable C: the reader and the layout run in the unit tests on any
 * OS; rlw_main.c only paints the shapes with GDI. */
#ifndef RLW_MARKERS_H
#define RLW_MARKERS_H
#include <stdint.h>
#include "rlw_modern.h" /* RlwPeek */

/* Mission state 0x4e2f78 (0x12 in a mission), local player 0x508041, view
 * (x, y, w, h) 0x508720, the four selection lists 0x508728 (u16 count + 255
 * unit indices, 512 bytes each). */
#define RLW_MARKER_RANGES 4
extern const uint32_t rlw_marker_addresses[RLW_MARKER_RANGES];
extern const uint32_t rlw_marker_sizes[RLW_MARKER_RANGES];
/* At most this many selected units are drawn, each with at most this many waypoints. */
#define RLW_MARKER_MAX_UNITS 64
#define RLW_MARKER_MAX_POINTS 8
/* Units rl_fila_ordens is asked about (the original's unit table). */
#define RLW_MARKER_UNIT_LIMIT 800u
/* The macOS host refreshes every 4 frames. */
#define RLW_MARKER_PERIOD 4u

typedef struct { int x, y; } RlwMarkerPoint;
typedef struct { int count; RlwMarkerPoint points[RLW_MARKER_MAX_POINTS]; } RlwMarkerPath;
typedef struct {
 /* Waypoint centres in original 800x600 pixels per selected unit with waypoints, oldest first. */
 int path_count; RlwMarkerPath paths[RLW_MARKER_MAX_UNITS];
 /* The map part of the picture (the view's cells, from the top-left), where markers may show. */
 int map_width, map_height;
} RlwMarkers;

/* rl_fila_ordens over the driver (rlw_driver_order_queue): 0 = success, *count
 * the total; cells may be NULL with capacity 0. */
typedef int (*RlwOrderQueue)(void *ctx, uint32_t unit, uint16_t *cells, uint32_t capacity, uint32_t *count);

void rlw_markers_clear(RlwMarkers *m);
int rlw_markers_equal(const RlwMarkers *a, const RlwMarkers *b);
/* NativeLiveWindow.anyOrderQueued: 1 when some unit has a waypoint. */
int rlw_markers_any_queued(RlwOrderQueue queue, void *ctx);
/* NativeOrderQueueMarkers.read: one peek of the four ranges, then the queue of
 * each selected unit. 0 on success (empty outside a mission); nonzero when the
 * peek fails (out is then empty). */
int rlw_markers_read(RlwPeek peek, RlwOrderQueue queue, void *ctx, RlwMarkers *out);

/* layoutWaypoints in window pixels (y down) for the picture's aspect-fit rect
 * (left, top, width, height): scale width/800 by height/600, the map region
 * clipped from the picture's top-left, flag size max(3, 5 * scale x). */
enum { RLW_SHAPE_LINE, RLW_SHAPE_DASH, RLW_SHAPE_RECT, RLW_SHAPE_ELLIPSE };
typedef struct { int kind, x0, y0, x1, y1; } RlwMarkerShape;
/* Per waypoint: pole (line), flag (filled rect), dot (filled ellipse); between
 * waypoints a dashed line. Rects/ellipses are [x0, x1) x [y0, y1). */
#define RLW_MARKER_SHAPES (RLW_MARKER_MAX_UNITS * (RLW_MARKER_MAX_POINTS - 1 + 3 * RLW_MARKER_MAX_POINTS))
typedef struct { int left, top, right, bottom; } RlwMarkerRect;
/* Returns the number of shapes written (at most capacity); *clip receives the map
 * region in window pixels (empty when there is nothing to draw). */
int rlw_markers_layout(const RlwMarkers *m, int left, int top, int width, int height,
                       RlwMarkerShape *out, int capacity, RlwMarkerRect *clip);
/* Line width in device pixels (macOS: 1.5 points, independent of the picture's
 * scale) for the window's DPI (96 = 100 %). */
int rlw_markers_pen_width(unsigned dpi);
#endif
