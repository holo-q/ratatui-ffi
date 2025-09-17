use crate::*;
use std::ffi::CString;

#[no_mangle]
pub extern "C" fn ratatui_headless_render_paragraph(
    width: u16,
    height: u16,
    para: *const FfiParagraph,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if para.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let p = unsafe { &*para };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let mut widget = Paragraph::new(p.lines.clone());
    if let Some(a) = p.align { widget = widget.alignment(a); }
    if let Some(trim) = p.wrap_trim { widget = widget.wrap(ratatui::widgets::Wrap { trim }); }
    if let (Some(sx), Some(sy)) = (p.scroll_x, p.scroll_y) { widget = widget.scroll((sx, sy)); }
    if let Some(st) = &p.base_style { widget = widget.style(st.clone()); }
    if let Some(b) = &p.block { widget = widget.block(b.clone()); }
    ratatui::widgets::Widget::render(widget, area, &mut buf);

    let mut s = String::new();
    for y in 0..height {
        for x in 0..width { s.push_str(buf[(x, y)].symbol()); }
        if y + 1 < height { s.push('\n'); }
    }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

