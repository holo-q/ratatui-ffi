use crate::{FfiRect, FfiStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug)]
pub struct Caps {
    pub max_width: u16,
    pub max_height: u16,
    pub max_area: u32,
    pub max_text_len: u32,
    pub max_batch_items: u32,
}

static SAFETY_ENABLED: AtomicBool = AtomicBool::new(false);
static CAPS: OnceLock<Mutex<Caps>> = OnceLock::new();
static LAST_ERROR: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn caps() -> &'static Mutex<Caps> {
    CAPS.get_or_init(|| {
        Mutex::new(Caps {
            max_width: 400,
            max_height: 200,
            max_area: 4_000_000,
            max_text_len: 8192,
            max_batch_items: 100_000,
        })
    })
}

fn last_error() -> &'static Mutex<Option<String>> {
    LAST_ERROR.get_or_init(|| Mutex::new(None))
}

pub fn safety_enabled() -> bool {
    if SAFETY_ENABLED.load(Ordering::Relaxed) {
        return true;
    }
    // Enable via env var without recompiling
    if std::env::var("RATATUI_FFI_SAFETY").map(|v| v != "0").unwrap_or(false) {
        SAFETY_ENABLED.store(true, Ordering::Relaxed);
        return true;
    }
    false
}

pub fn record_error(msg: &str) {
    if let Ok(mut slot) = last_error().lock() {
        *slot = Some(msg.to_string());
    }
}

pub fn check_rect_dims(rect: FfiRect) -> bool {
    if !safety_enabled() {
        return true;
    }
    let caps = caps().lock().unwrap();
    if rect.width == 0 || rect.height == 0 {
        record_error("rect has zero width/height");
        return false;
    }
    if rect.width > caps.max_width || rect.height > caps.max_height {
        record_error("rect exceeds max dimensions");
        return false;
    }
    let area = (rect.width as u32) * (rect.height as u32);
    if area > caps.max_area {
        record_error("rect area exceeds cap");
        return false;
    }
    true
}

pub fn check_rect_in_viewport(rect: FfiRect, viewport: FfiRect) -> bool {
    if !safety_enabled() {
        return true;
    }
    // basic bounds check
    let vx = viewport.x as u32;
    let vy = viewport.y as u32;
    let vw = viewport.width as u32;
    let vh = viewport.height as u32;
    let x = rect.x as u32;
    let y = rect.y as u32;
    let w = rect.width as u32;
    let h = rect.height as u32;
    if x < vx || y < vy || x.saturating_add(w) > vx + vw || y.saturating_add(h) > vy + vh {
        record_error("rect does not fit within viewport");
        return false;
    }
    true
}

pub fn check_text_len(len: usize) -> bool {
    if !safety_enabled() {
        return true;
    }
    let caps = caps().lock().unwrap();
    if (len as u32) > caps.max_text_len {
        record_error("text length exceeds cap");
        return false;
    }
    true
}

pub fn check_batch_len(n: usize) -> bool {
    if !safety_enabled() {
        return true;
    }
    let caps = caps().lock().unwrap();
    if (n as u32) > caps.max_batch_items {
        record_error("batch length exceeds cap");
        return false;
    }
    true
}

#[no_mangle]
pub extern "C" fn ratatui_ffi_set_safety(enabled: bool) {
    SAFETY_ENABLED.store(enabled, Ordering::Relaxed);
}

#[no_mangle]
pub extern "C" fn ratatui_ffi_last_error() -> FfiStr {
    if let Ok(slot) = last_error().lock() {
        if let Some(s) = slot.as_ref() {
            return FfiStr {
                ptr: s.as_ptr(),
                len: s.len(),
            };
        }
    }
    FfiStr {
        ptr: std::ptr::null(),
        len: 0,
    }
}

#[no_mangle]
pub extern "C" fn ratatui_ffi_clear_last_error() {
    if let Ok(mut slot) = last_error().lock() {
        *slot = None;
    }
}

#[no_mangle]
pub extern "C" fn ratatui_ffi_set_caps(
    max_width: u16,
    max_height: u16,
    max_area: u32,
    max_text_len: u32,
    max_batch_items: u32,
) {
    if let Ok(mut c) = caps().lock() {
        c.max_width = max_width.max(1);
        c.max_height = max_height.max(1);
        c.max_area = max_area.max(1);
        c.max_text_len = max_text_len.max(1);
        c.max_batch_items = max_batch_items.max(1);
    }
}
