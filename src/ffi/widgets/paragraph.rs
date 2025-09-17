use crate::*;
use std::ffi::CString;
use std::ffi::CStr;
use std::os::raw::c_char;

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

#[no_mangle]
pub extern "C" fn ratatui_paragraph_new(text_utf8: *const c_char) -> *mut FfiParagraph {
    if text_utf8.is_null() { return std::ptr::null_mut(); }
    let c_str = unsafe { CStr::from_ptr(text_utf8) };
    let text = match c_str.to_str() { Ok(s) => s.to_owned(), Err(_) => return std::ptr::null_mut() };
    let mut lines: Vec<Line<'static>> = Vec::new();
    for l in text.split('\n') { lines.push(Line::from(Span::raw(l.to_string()))); }
    Box::into_raw(Box::new(FfiParagraph { lines, block: None, align: None, wrap_trim: None, scroll_x: None, scroll_y: None, base_style: None }))
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_new_empty() -> *mut FfiParagraph {
    Box::into_raw(Box::new(FfiParagraph { lines: Vec::new(), block: None, align: None, wrap_trim: None, scroll_x: None, scroll_y: None, base_style: None }))
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_span(para: *mut FfiParagraph, text_utf8: *const c_char, style: FfiStyle) {
    if para.is_null() || text_utf8.is_null() { return; }
    let p = unsafe { &mut *para };
    let c_str = unsafe { CStr::from_ptr(text_utf8) };
    if let Ok(s) = c_str.to_str() {
        let st = style_from_ffi(style);
        if let Some(last) = p.lines.last_mut() { last.spans.push(Span::styled(s.to_string(), st)); }
        else { p.lines.push(Line::from(Span::styled(s.to_string(), st))); }
    }
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_spans(para: *mut FfiParagraph, spans: *const FfiSpan, len: usize) {
    if para.is_null() || spans.is_null() || len == 0 { return; }
    let p = unsafe { &mut *para };
    if let Some(sp) = spans_from_ffi(spans, len) {
        if let Some(last) = p.lines.last_mut() { last.spans.extend(sp); }
        else { p.lines.push(Line::from(sp)); }
    }
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_line_spans(para: *mut FfiParagraph, spans: *const FfiSpan, len: usize) {
    if para.is_null() || spans.is_null() || len == 0 { return; }
    let p = unsafe { &mut *para };
    if let Some(sp) = spans_from_ffi(spans, len) { p.lines.push(Line::from(sp)); }
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_lines_spans(para: *mut FfiParagraph, lines: *const FfiLineSpans, len: usize) {
    if para.is_null() || lines.is_null() || len == 0 { return; }
    let p = unsafe { &mut *para };
    let slice = unsafe { std::slice::from_raw_parts(lines, len) };
    for ls in slice.iter() {
        if ls.spans.is_null() || ls.len == 0 { p.lines.push(Line::default()); continue; }
        if let Some(sp) = spans_from_ffi(ls.spans, ls.len) { p.lines.push(Line::from(sp)); }
        else { p.lines.push(Line::default()); }
    }
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_line_break(para: *mut FfiParagraph) {
    if para.is_null() { return; }
    let p = unsafe { &mut *para };
    p.lines.push(Line::default());
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_set_alignment(para: *mut FfiParagraph, align: u32) {
    if para.is_null() { return; }
    let p = unsafe { &mut *para };
    p.align = Some(match align { 1 => ratatui::layout::Alignment::Center, 2 => ratatui::layout::Alignment::Right, _ => ratatui::layout::Alignment::Left });
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_set_wrap(para: *mut FfiParagraph, trim: bool) {
    if para.is_null() { return; }
    let p = unsafe { &mut *para };
    p.wrap_trim = Some(trim);
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_set_scroll(para: *mut FfiParagraph, x: u16, y: u16) {
    if para.is_null() { return; }
    let p = unsafe { &mut *para };
    p.scroll_x = Some(x);
    p.scroll_y = Some(y);
}

crate::ratatui_set_style_fn!(ratatui_paragraph_set_style, FfiParagraph, base_style);
crate::ratatui_block_title_fn!(ratatui_paragraph_set_block_title, FfiParagraph);
crate::ratatui_block_title_spans_fn!(ratatui_paragraph_set_block_title_spans, FfiParagraph);
crate::ratatui_block_adv_fn!(ratatui_paragraph_set_block_adv, FfiParagraph);

#[no_mangle]
pub extern "C" fn ratatui_paragraph_free(para: *mut FfiParagraph) {
    if para.is_null() { return; }
    unsafe { drop(Box::from_raw(para)) };
}

