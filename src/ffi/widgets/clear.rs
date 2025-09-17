use crate::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Clear as RtClear;
use std::ffi::CString;

#[no_mangle]
pub extern "C" fn ratatui_clear_in(term: *mut FfiTerminal, rect: FfiRect) -> bool {
    crate::guard_bool("ratatui_clear_in", || {
        if term.is_null() { return false; }
        let t = unsafe { &mut *term };
        let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
        let res = t.terminal.draw(|frame| { frame.render_widget(RtClear, area); });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_clear(
    width: u16,
    height: u16,
    out_text_utf8: *mut *mut std::os::raw::c_char,
) -> bool {
    if out_text_utf8.is_null() { return false; }
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    ratatui::widgets::Widget::render(RtClear, area, &mut buf);
    let mut s = String::new();
    for y in 0..height { for x in 0..width { s.push_str(buf[(x, y)].symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}
