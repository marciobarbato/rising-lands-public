/* Automated smokes of the Windows host that need no game data (ci/smoke.sh):
 * they run inside the real RisingLands.exe on every CI runner (mingw x64, MSVC
 * x64) and exit with a code.
 *
 * --smoke-markers        paints a synthetic picture with order queue markers
 *                        through the real GDI path (rlw_markers_paint) into a
 *                        memory DC at 1:1, 2x with letterbox and a small window,
 *                        and checks the pixels: flag, dot and dashed line in the
 *                        marker colour, the HUD below the map region untouched.
 * --smoke-import-prompt  first start with an empty --data folder and no --cd,
 *                        the player's path (no --headless): the import notice
 *                        and the CD file dialog must open; a watcher thread
 *                        closes each as the player's Cancel would, so the
 *                        program must end with code 2 and no Dados folder. A
 *                        watchdog ends the process with code 9 after 60 s.
 * --smoke-window         the DPI mixed mode (rlw_dpi.h) on real windows: one
 *                        created per-monitor aware, one created unaware (as the
 *                        key window is), the 800x600 client size at the window's
 *                        DPI, and the borderless fullscreen fit to its monitor. */
#ifndef RLW_SMOKE_H
#define RLW_SMOKE_H

/* 0 = every check passed; prints one line per check to stdout. */
int rlw_smoke_markers(void);
/* 0 = every check passed. */
int rlw_smoke_window(void);
/* Starts the dialog watcher for the calling (UI) thread. */
void rlw_smoke_dialogs_begin(void);
/* Stops it and prints "diálogos fechados: N (title; title)"; returns N. */
int rlw_smoke_dialogs_end(void);
#endif
