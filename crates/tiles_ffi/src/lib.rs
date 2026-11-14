use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use std::ffi::{c_void, CStr, CString};
use std::num::NonZeroIsize;
use std::os::raw::c_char;
use std::path::PathBuf;
use tiles_app::{AppCommand, EngineSession};
use tiles_core::Trs;
use tiles_render::TilesRenderer;

#[repr(C)]
pub struct NativeWindowInfo {
    pub backend: u32, // 1=Windows HWND, 2=macOS NSView*, 3=Linux X11 Window
    pub window_id: u64,
    pub display_ptr: *mut c_void,
    pub x11_screen: i32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
pub struct TrsC {
    pub tx: f64,
    pub ty: f64,
    pub tz: f64,
    pub rx: f64,
    pub ry: f64,
    pub rz: f64,
    pub sx: f64,
    pub sy: f64,
    pub sz: f64,
}

pub struct EngineHandle {
    session: Option<EngineSession>,
}

fn build_raw_handles(info: &NativeWindowInfo) -> Option<(RawDisplayHandle, RawWindowHandle)> {
    match info.backend {
        #[cfg(target_os = "windows")]
        1 => {
            use raw_window_handle::{Win32WindowHandle, WindowsDisplayHandle};
            let hwnd = NonZeroIsize::new(info.window_id as isize)?;
            Some((
                RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
            ))
        }
        #[cfg(target_os = "macos")]
        2 => {
            use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle};
            let view = NonNull::new(info.window_id as *mut c_void)?;
            Some((
                RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
                RawWindowHandle::AppKit(AppKitWindowHandle::new(view)),
            ))
        }
        #[cfg(target_os = "linux")]
        3 => {
            use raw_window_handle::{XlibDisplayHandle, XlibWindowHandle};
            let win = XlibWindowHandle::new(info.window_id as std::ffi::c_ulong);
            let disp = XlibDisplayHandle::new(NonNull::new(info.display_ptr), info.x11_screen);
            Some((RawDisplayHandle::Xlib(disp), RawWindowHandle::Xlib(win)))
        }
        _ => None,
    }
}

#[no_mangle]
pub extern "C" fn engine_create() -> *mut EngineHandle {
    Box::into_raw(Box::new(EngineHandle { session: None }))
}

#[no_mangle]
pub unsafe extern "C" fn engine_destroy(handle: *mut EngineHandle) {
    if !handle.is_null() {
        drop(Box::from_raw(handle));
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_attach_window(
    handle: *mut EngineHandle,
    info: NativeWindowInfo,
) -> bool {
    if handle.is_null() {
        return false;
    }
    let h = &mut *handle;
    if h.session.is_some() {
        return true;
    }
    let Some((raw_disp, raw_win)) = build_raw_handles(&info) else {
        return false;
    };
    let gpu_budget = 1536 * 1024 * 1024;
    let Ok(renderer) = TilesRenderer::new(
        raw_disp,
        raw_win,
        info.width.max(1),
        info.height.max(1),
        gpu_budget,
    ) else {
        return false;
    };
    let Ok(session) = EngineSession::start(Some(renderer)) else {
        return false;
    };
    h.session = Some(session);
    true
}

#[no_mangle]
pub unsafe extern "C" fn engine_resize(handle: *mut EngineHandle, width: u32, height: u32) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::Resize { width, height });
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_open_tileset(
    handle: *mut EngineHandle,
    path_utf8: *const c_char,
) -> bool {
    if path_utf8.is_null() {
        return false;
    }
    let Ok(s) = CStr::from_ptr(path_utf8).to_str() else {
        return false;
    };
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::OpenTileset(PathBuf::from(s)));
        return true;
    }
    false
}

#[no_mangle]
pub unsafe extern "C" fn engine_save_tileset(
    handle: *mut EngineHandle,
    override_path_utf8: *const c_char,
) {
    let path_opt = if override_path_utf8.is_null() {
        None
    } else {
        CStr::from_ptr(override_path_utf8)
            .to_str()
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
    };
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::SaveTileset(path_opt));
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_camera_orbit(handle: *mut EngineHandle, dx: f64, dy: f64) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::Orbit { dx, dy });
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_camera_pan(handle: *mut EngineHandle, dx: f64, dy: f64) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::Pan { dx, dy });
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_camera_zoom(handle: *mut EngineHandle, delta_steps: f64) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::Zoom { delta: delta_steps });
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_pick_at(handle: *mut EngineHandle, x: f64, y: f64) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::PickAt { x, y });
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_select_tile(handle: *mut EngineHandle, tile_id: u64) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::SelectTile(if tile_id == 0 {
            None
        } else {
            Some(tile_id)
        }));
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_focus_tile(handle: *mut EngineHandle, tile_id: u64) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::FocusTile(if tile_id == 0 {
            None
        } else {
            Some(tile_id)
        }));
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_set_max_sse(handle: *mut EngineHandle, max_sse: f64) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::SetMaxSse(max_sse));
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_set_show_all_bboxes(handle: *mut EngineHandle, show: bool) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::SetShowAllBBoxes(show));
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_set_tile_trs(
    handle: *mut EngineHandle,
    tile_id: u64,
    trs: TrsC,
) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::SetTileTrs {
            tile_id,
            trs: Trs {
                translation: [trs.tx, trs.ty, trs.tz],
                rotation_deg: [trs.rx, trs.ry, trs.rz],
                scale: [trs.sx, trs.sy, trs.sz],
            },
        });
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_set_tile_visible(
    handle: *mut EngineHandle,
    tile_id: u64,
    visible: bool,
) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::SetTileVisible { tile_id, visible });
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_undo(handle: *mut EngineHandle) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::Undo);
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_redo(handle: *mut EngineHandle) {
    if let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) {
        sess.send(AppCommand::Redo);
    }
}

#[no_mangle]
pub unsafe extern "C" fn engine_poll_event_json(handle: *mut EngineHandle) -> *mut c_char {
    let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) else {
        return std::ptr::null_mut();
    };
    let Some(evt) = sess.poll_event() else {
        return std::ptr::null_mut();
    };
    let Ok(json) = serde_json::to_string(&evt) else {
        return std::ptr::null_mut();
    };
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn engine_get_tile_children_json(
    handle: *mut EngineHandle,
    parent_id: u64,
) -> *mut c_char {
    let Some(sess) = handle.as_ref().and_then(|h| h.session.as_ref()) else {
        return std::ptr::null_mut();
    };
    let list = if parent_id == 0 {
        sess.root_summary().into_iter().collect()
    } else {
        sess.tile_children(parent_id)
    };
    let Ok(json) = serde_json::to_string(&list) else {
        return std::ptr::null_mut();
    };
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "C" fn engine_free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}
