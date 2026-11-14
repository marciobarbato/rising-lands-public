/* GDI painting of the Shift+click order queue markers (rlw_markers.h), Win32 only. */
#ifndef RLW_MARKERS_GDI_H
#define RLW_MARKERS_GDI_H
#include <windows.h>
#include "rlw_markers.h"

/* Presents the 800x600 indexed picture stretched to (left, top, width, height)
 * with the markers over its map part, through an off-screen bitmap so a frame
 * never shows without its markers. Returns 0 (nothing drawn) when there is
 * nothing to mark or a GDI object cannot be created: the caller then blits the
 * picture as usual. */
int rlw_markers_paint(HDC dc, const RlwMarkers *m, int left, int top, int width, int height,
                      const void *pixels, const BITMAPINFO *bmi);
#endif
