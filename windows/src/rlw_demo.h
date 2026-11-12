/* Fixed synthetic input for smoke and parity runs: hover, a left click, a held
 * drag, a right click, Tab, a right-edge hover (edge-scroll zone) and back.
 * Delivered at the start of the given 1-based frame through the host's normal
 * event path (rlw_driver_event / macos_live_event), never into game state. */
#ifndef RLW_DEMO_H
#define RLW_DEMO_H
#include <stdint.h>
typedef struct { uint64_t frame; uint32_t kind, message; int32_t x, y; uint32_t wparam, vk, down; } RlwDemoEvent;
static const RlwDemoEvent RLW_DEMO[] = {
 {5, 1, 0x200, 400, 300, 0, 0, 0},
 {10, 1, 0x201, 420, 310, 0, 0, 0}, {11, 1, 0x202, 420, 310, 0, 0, 0},
 {20, 1, 0x201, 300, 250, 0, 0, 0}, {22, 1, 0x200, 360, 290, 1, 0, 0}, {26, 1, 0x202, 360, 290, 0, 0, 0},
 {30, 1, 0x204, 450, 320, 0, 0, 0}, {31, 1, 0x205, 450, 320, 0, 0, 0},
 {40, 2, 0, 0, 0, 0, 9, 1}, {41, 2, 0, 0, 0, 0, 9, 0},
 {50, 1, 0x200, 799, 300, 0, 0, 0}, {60, 1, 0x200, 400, 300, 0, 0, 0},
};
#define RLW_DEMO_COUNT (sizeof(RLW_DEMO) / sizeof(RLW_DEMO[0]))
#endif
