// Widget split placeholder: Sparkline
// Move from src/lib.rs:
// - FFI externs: ratatui_sparkline_new, ratatui_sparkline_free
// - Setters: ratatui_sparkline_set_values, ratatui_sparkline_set_max, ratatui_sparkline_set_style
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_sparkline_set_block_title, FfiSparkline)
//   ratatui_block_title_spans_fn!(ratatui_sparkline_set_block_title_spans, FfiSparkline)
//   ratatui_block_adv_fn!(ratatui_sparkline_set_block_adv, FfiSparkline)
// - Draw helpers: ratatui_terminal_draw_sparkline_in, ratatui_headless_render_sparkline
// Types used: FfiSparkline

// use crate::*; // enable when moving implementations

use crate::{
    ratatui_block_adv_fn, ratatui_block_title_fn, ratatui_block_title_spans_fn,
    ratatui_set_style_fn, FfiRect, FfiTerminal,
};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Style;
use ratatui::widgets::{Block, Sparkline as RtSparkline};
use std::ffi::{c_char, CString};

// ----- Sparkline -----

ratatui_set_style_fn!(ratatui_sparkline_set_style, FfiSparkline, style);
ratatui_block_title_fn!(ratatui_sparkline_set_block_title, FfiSparkline);
ratatui_block_title_spans_fn!(ratatui_sparkline_set_block_title_spans, FfiSparkline);
ratatui_block_adv_fn!(ratatui_sparkline_set_block_adv, FfiSparkline);

#[repr(C)]
pub struct FfiSparkline {
    pub values: Vec<u64>,
    pub block: Option<Block<'static>>,
    pub max: Option<u64>,
    pub style: Option<Style>,
}

#[no_mangle]
pub extern "C" fn ratatui_sparkline_new() -> *mut FfiSparkline {
    Box::into_raw(Box::new(FfiSparkline {
        values: Vec::new(),
        block: None,
        max: None,
        style: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_sparkline_free(s: *mut FfiSparkline) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(s));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_sparkline_set_values(
    s: *mut FfiSparkline,
    values: *const u64,
    len: usize,
) {
    if s.is_null() || values.is_null() {
        return;
    }
    let sp = unsafe { &mut *s };
    let slice = unsafe { std::slice::from_raw_parts(values, len) };
    sp.values = slice.to_vec();
}

#[no_mangle]
pub extern "C" fn ratatui_sparkline_set_max(s: *mut FfiSparkline, max: u64) {
    if s.is_null() {
        return;
    }
    unsafe {
        (&mut *s).max = Some(max);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_sparkline_in(
    term: *mut FfiTerminal,
    s: *const FfiSparkline,
    rect: FfiRect,
) -> bool {
    crate::guard_bool("ratatui_terminal_draw_sparkline_in", || {
        if term.is_null() || s.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let sp = unsafe { &*s };
        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        #[cfg(feature = "ffi_safety")]
        {
            if !crate::ffi::safety::check_rect_dims(rect) {
                return false;
            }
        }
        let mut w = RtSparkline::default().data(&sp.values);
        if let Some(bl) = &sp.block {
            w = w.block(bl.clone());
        }
        let res = t.terminal.draw(|frame| {
            frame.render_widget(w.clone(), area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_sparkline(
    width: u16,
    height: u16,
    s: *const FfiSparkline,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if s.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let sp = unsafe { &*s };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let mut w = RtSparkline::default().data(&sp.values);
    if let Some(m) = sp.max {
        w = w.max(m);
    }
    if let Some(st) = &sp.style {
        w = w.style(st.clone());
    }
    if let Some(bl) = &sp.block {
        w = w.block(bl.clone());
    }
    ratatui::widgets::Widget::render(w, area, &mut buf);
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
