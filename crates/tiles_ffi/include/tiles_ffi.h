#ifndef TILES_FFI_H
#define TILES_FFI_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct EngineHandle EngineHandle;

typedef struct NativeWindowInfo {
    uint32_t backend; // 1=Windows HWND, 2=macOS NSView*, 3=Linux X11 Window
    uint64_t window_id;
    void* display_ptr;
    int32_t x11_screen;
    uint32_t width;
    uint32_t height;
} NativeWindowInfo;

typedef struct TrsC {
    double tx;
    double ty;
    double tz;
    double rx;
    double ry;
    double rz;
    double sx;
    double sy;
    double sz;
} TrsC;

EngineHandle* engine_create(void);
void engine_destroy(EngineHandle* handle);

bool engine_attach_window(EngineHandle* handle, NativeWindowInfo info);
void engine_resize(EngineHandle* handle, uint32_t width, uint32_t height);

bool engine_open_tileset(EngineHandle* handle, const char* path_utf8);
void engine_save_tileset(EngineHandle* handle, const char* override_path_utf8);

void engine_camera_orbit(EngineHandle* handle, double dx, double dy);
void engine_camera_pan(EngineHandle* handle, double dx, double dy);
void engine_camera_zoom(EngineHandle* handle, double delta_steps);
void engine_pick_at(EngineHandle* handle, double x, double y);

void engine_select_tile(EngineHandle* handle, uint64_t tile_id);
void engine_focus_tile(EngineHandle* handle, uint64_t tile_id);
void engine_set_max_sse(EngineHandle* handle, double max_sse);
void engine_set_show_all_bboxes(EngineHandle* handle, bool show);
void engine_set_tile_trs(EngineHandle* handle, uint64_t tile_id, TrsC trs);
void engine_set_tile_visible(EngineHandle* handle, uint64_t tile_id, bool visible);
void engine_undo(EngineHandle* handle);
void engine_redo(EngineHandle* handle);

char* engine_poll_event_json(EngineHandle* handle);
char* engine_get_tile_children_json(EngineHandle* handle, uint64_t parent_id);
void engine_free_string(char* ptr);

#ifdef __cplusplus
}
#endif

#endif // TILES_FFI_H
