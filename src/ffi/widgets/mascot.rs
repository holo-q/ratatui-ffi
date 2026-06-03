// RatatuiMascot (new in ratatui 0.30) — mirrors logo.rs. The mascot carries an
// eye-color knob (Default/Red); we pass it as a u32 mapped through FfiMascotEye
// so bindings get named constants for free.

use crate::{FfiRect, FfiTerminal};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{MascotEyeColor as RtMascotEyeColor, RatatuiMascot as RtRatatuiMascot};
use std::ffi::{c_char, CString};

/// Eye-color knob for the mascot — an FFI constant provider (bindings emit these
/// as named ints); not referenced from Rust, hence allow(dead_code).
#[allow(dead_code)]
#[repr(u32)]
pub enum FfiMascotEye {
    Default = 0,
    Red = 1,
}

fn mascot_eye_from_u32(eye: u32) -> RtMascotEyeColor {
    match eye {
        1 => RtMascotEyeColor::Red,
        _ => RtMascotEyeColor::Default,
    }
}

#[no_mangle]
pub extern "C" fn ratatui_ratatuimascot_draw_in(term: *mut FfiTerminal, rect: FfiRect, eye: u32) -> bool {
    crate::guard_bool("ratatui_ratatuimascot_draw_in", || {
        if term.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let mascot = RtRatatuiMascot::new().set_eye(mascot_eye_from_u32(eye));
        let res = t.terminal.draw(|frame| {
            frame.render_widget(mascot, area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_ratatuimascot(
    width: u16,
    height: u16,
    eye: u32,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if out_text_utf8.is_null() {
        return false;
    }
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let mascot = RtRatatuiMascot::new().set_eye(mascot_eye_from_u32(eye));
    ratatui::widgets::Widget::render(mascot, area, &mut buf);
    let mut s = String::new();
    for y in 0..height {
        for x in 0..width {
            let cell = &buf[(x, y)];
            s.push_str(cell.symbol());
        }
        if y + 1 < height {
            s.push('\n');
        }
    }
    match CString::new(s) {
        Ok(cstr) => {
            unsafe {
                *out_text_utf8 = cstr.into_raw();
            }
            true
        }
        Err(_) => false,
    }
}
