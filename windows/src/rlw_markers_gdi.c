/* GDI painting of the Shift+click order queue markers (see rlw_markers_gdi.h).
 * macOS (NativeLiveWindow.layoutWaypoints) strokes and fills one CAShapeLayer
 * path in (1, 0.85, 0.2), 1.5-point lines dashed 4/3, masked to the map region.
 * Here: the same colour and geometry, the connecting lines dashed, poles solid,
 * flags and dots filled, clipped to the map region. */
#define WIN32_LEAN_AND_MEAN
#ifndef UNICODE
#define UNICODE
#endif
#include "rlw_markers_gdi.h"

static RlwMarkerShape shapes[RLW_MARKER_SHAPES];

int rlw_markers_paint(HDC dc, const RlwMarkers *m, int left, int top, int width, int height,
                      const void *pixels, const BITMAPINFO *bmi) {
 RlwMarkerRect clip;
 int n = rlw_markers_layout(m, 0, 0, width, height, shapes, RLW_MARKER_SHAPES, &clip);
 if (n <= 0 || !pixels || !bmi) return 0;
 int dpi = GetDeviceCaps(dc, LOGPIXELSX); if (dpi <= 0) dpi = 96;
 HDC mem = CreateCompatibleDC(dc);
 HBITMAP bitmap = mem ? CreateCompatibleBitmap(dc, width, height) : NULL;
 COLORREF colour = RGB(255, 217, 51);
 int pen_width = rlw_markers_pen_width((unsigned)dpi);
 LOGBRUSH lb; lb.lbStyle = BS_SOLID; lb.lbColor = colour; lb.lbHatch = 0;
 DWORD dash[2]; dash[0] = (DWORD)((4 * dpi + 48) / 96); dash[1] = (DWORD)((3 * dpi + 48) / 96);
 HPEN dashed = bitmap ? ExtCreatePen(PS_GEOMETRIC | PS_USERSTYLE | PS_ENDCAP_FLAT, (DWORD)pen_width, &lb, 2, dash) : NULL;
 HPEN solid = bitmap ? CreatePen(PS_SOLID, pen_width, colour) : NULL;
 HPEN thin = bitmap ? CreatePen(PS_SOLID, 1, colour) : NULL;
 HBRUSH fill = bitmap ? CreateSolidBrush(colour) : NULL;
 int ok = mem && bitmap && dashed && solid && thin && fill;
 if (ok) {
  HGDIOBJ old_bitmap = SelectObject(mem, bitmap);
  SetStretchBltMode(mem, COLORONCOLOR);
  StretchDIBits(mem, 0, 0, width, height, 0, 0, 800, 600, pixels, bmi, DIB_RGB_COLORS, SRCCOPY);
  int saved = SaveDC(mem);
  IntersectClipRect(mem, clip.left, clip.top, clip.right, clip.bottom);
  HGDIOBJ old_pen = SelectObject(mem, solid), old_brush = SelectObject(mem, fill);
  for (int i = 0; i < n; i++) {
   const RlwMarkerShape *s = &shapes[i];
   switch (s->kind) {
   case RLW_SHAPE_DASH: SelectObject(mem, dashed); MoveToEx(mem, s->x0, s->y0, NULL); LineTo(mem, s->x1, s->y1); break;
   case RLW_SHAPE_LINE: SelectObject(mem, solid); MoveToEx(mem, s->x0, s->y0, NULL); LineTo(mem, s->x1, s->y1); break;
   case RLW_SHAPE_RECT: SelectObject(mem, thin); Rectangle(mem, s->x0, s->y0, s->x1, s->y1); break;
   default: SelectObject(mem, thin); Ellipse(mem, s->x0, s->y0, s->x1, s->y1); break;
   }
  }
  SelectObject(mem, old_pen); SelectObject(mem, old_brush);
  RestoreDC(mem, saved);
  BitBlt(dc, left, top, width, height, mem, 0, 0, SRCCOPY);
  SelectObject(mem, old_bitmap);
 }
 if (fill) DeleteObject(fill);
 if (thin) DeleteObject(thin);
 if (solid) DeleteObject(solid);
 if (dashed) DeleteObject(dashed);
 if (bitmap) DeleteObject(bitmap);
 if (mem) DeleteDC(mem);
 return ok;
}
