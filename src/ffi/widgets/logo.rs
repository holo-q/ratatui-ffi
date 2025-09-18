// Widget split placeholder: RatatuiLogo
// Move from src/lib.rs:
// - Draw helpers: ratatui_ratatuilogo_draw_in, ratatui_ratatuilogo_draw_sized_in
// - Headless: ratatui_headless_render_ratatuilogo, ratatui_headless_render_ratatuilogo_sized

// use crate::*; // enable when moving implementations

use crate::{FfiRect, FfiTerminal};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::RatatuiLogo as RtRatatuiLogo;
use std::ffi::{c_char, CString};

#[no_mangle]
pub extern "C" fn ratatui_ratatuilogo_draw_in(term: *mut FfiTerminal, rect: FfiRect) -> bool {
    crate::guard_bool("ratatui_ratatuilogo_draw_in", || {
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
        let res = t.terminal.draw(|frame| {
            frame.render_widget(RtRatatuiLogo::default(), area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_ratatuilogo(
    width: u16,
    height: u16,
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
    ratatui::widgets::Widget::render(RtRatatuiLogo::default(), area, &mut buf);
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

#[no_mangle]
pub extern "C" fn ratatui_ratatuilogo_draw_sized_in(
    term: *mut FfiTerminal,
    rect: FfiRect,
    size: u32,
) -> bool {
    crate::guard_bool("ratatui_ratatuilogo_draw_sized_in", || {
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
        let logo = match size {
            1 => RtRatatuiLogo::small(),
            2 => RtRatatuiLogo::default(),
            3 => RtRatatuiLogo::tiny(),
            _ => RtRatatuiLogo::default(),
        };
        let res = t.terminal.draw(|frame| {
            frame.render_widget(logo, area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_ratatuilogo_sized(
    width: u16,
    height: u16,
    size: u32,
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
    let logo = match size {
        1 => RtRatatuiLogo::small(),
        2 => RtRatatuiLogo::default(),
        3 => RtRatatuiLogo::tiny(),
        _ => RtRatatuiLogo::default(),
    };
    ratatui::widgets::Widget::render(logo, area, &mut buf);
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
