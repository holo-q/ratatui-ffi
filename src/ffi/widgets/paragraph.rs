use crate::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Alignment, Line, Span};
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use std::ffi::CStr;
use std::ffi::CString;
use std::os::raw::c_char;

#[repr(C)]
pub struct FfiParagraph {
    pub lines: Vec<Line<'static>>,     // content
    pub block: Option<Block<'static>>, // optional block with borders/title
    pub align: Option<Alignment>,
    pub wrap_trim: Option<bool>,
    pub scroll_x: Option<u16>,
    pub scroll_y: Option<u16>,
    pub base_style: Option<Style>,
}

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
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let mut widget = Paragraph::new(p.lines.clone());
    if let Some(a) = p.align {
        widget = widget.alignment(a);
    }
    if let Some(trim) = p.wrap_trim {
        widget = widget.wrap(ratatui::widgets::Wrap { trim });
    }
    if let (Some(sx), Some(sy)) = (p.scroll_x, p.scroll_y) {
        widget = widget.scroll((sx, sy));
    }
    if let Some(st) = &p.base_style {
        widget = widget.style(st.clone());
    }
    if let Some(b) = &p.block {
        widget = widget.block(b.clone());
    }
    ratatui::widgets::Widget::render(widget, area, &mut buf);

    let mut s = String::new();
    for y in 0..height {
        for x in 0..width {
            s.push_str(buf[(x, y)].symbol());
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
pub extern "C" fn ratatui_paragraph_new(text_utf8: *const c_char) -> *mut FfiParagraph {
    if text_utf8.is_null() {
        return ptr::null_mut();
    }
    let c_str = unsafe { CStr::from_ptr(text_utf8) };
    let text = match c_str.to_str() {
        Ok(s) => s.to_owned(),
        Err(_) => return ptr::null_mut(),
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    for l in text.split('\n') {
        lines.push(Line::from(Span::raw(l.to_string())));
    }
    Box::into_raw(Box::new(FfiParagraph {
        lines,
        block: None,
        align: None,
        wrap_trim: None,
        scroll_x: None,
        scroll_y: None,
        base_style: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_new_empty() -> *mut FfiParagraph {
    Box::into_raw(Box::new(FfiParagraph {
        lines: Vec::new(),
        block: None,
        align: None,
        wrap_trim: None,
        scroll_x: None,
        scroll_y: None,
        base_style: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_span(
    para: *mut FfiParagraph,
    text_utf8: *const c_char,
    style: FfiStyle,
) {
    if para.is_null() || text_utf8.is_null() {
        return;
    }
    let p = unsafe { &mut *para };
    let c_str = unsafe { CStr::from_ptr(text_utf8) };
    if let Ok(s) = c_str.to_str() {
        let st = style_from_ffi(style);
        if let Some(last) = p.lines.last_mut() {
            last.spans.push(Span::styled(s.to_string(), st));
        } else {
            p.lines.push(Line::from(Span::styled(s.to_string(), st)));
        }
    }
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_spans(
    para: *mut FfiParagraph,
    spans: *const FfiSpan,
    len: usize,
) {
    if para.is_null() || spans.is_null() || len == 0 {
        return;
    }
    let p = unsafe { &mut *para };
    if let Some(sp) = spans_from_ffi(spans, len) {
        if let Some(last) = p.lines.last_mut() {
            last.spans.extend(sp);
        } else {
            p.lines.push(Line::from(sp));
        }
    }
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_line_spans(
    para: *mut FfiParagraph,
    spans: *const FfiSpan,
    len: usize,
) {
    if para.is_null() || spans.is_null() || len == 0 {
        return;
    }
    let p = unsafe { &mut *para };
    if let Some(sp) = spans_from_ffi(spans, len) {
        p.lines.push(Line::from(sp));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_lines_spans(
    para: *mut FfiParagraph,
    lines: *const FfiLineSpans,
    len: usize,
) {
    if para.is_null() || lines.is_null() || len == 0 {
        return;
    }
    let p = unsafe { &mut *para };
    let slice = unsafe { std::slice::from_raw_parts(lines, len) };
    for ls in slice.iter() {
        if ls.spans.is_null() || ls.len == 0 {
            p.lines.push(Line::default());
            continue;
        }
        if let Some(sp) = spans_from_ffi(ls.spans, ls.len) {
            p.lines.push(Line::from(sp));
        } else {
            p.lines.push(Line::default());
        }
    }
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_line_break(para: *mut FfiParagraph) {
    if para.is_null() {
        return;
    }
    let p = unsafe { &mut *para };
    p.lines.push(Line::default());
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_set_alignment(para: *mut FfiParagraph, align: u32) {
    if para.is_null() {
        return;
    }
    let p = unsafe { &mut *para };
    p.align = Some(match align {
        1 => Alignment::Center,
        2 => Alignment::Right,
        _ => Alignment::Left,
    });
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_set_wrap(para: *mut FfiParagraph, trim: bool) {
    if para.is_null() {
        return;
    }
    let p = unsafe { &mut *para };
    p.wrap_trim = Some(trim);
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_set_scroll(para: *mut FfiParagraph, x: u16, y: u16) {
    if para.is_null() {
        return;
    }
    let p = unsafe { &mut *para };
    p.scroll_x = Some(x);
    p.scroll_y = Some(y);
}

ratatui_set_style_fn!(ratatui_paragraph_set_style, FfiParagraph, base_style);
ratatui_block_title_fn!(ratatui_paragraph_set_block_title, FfiParagraph);
ratatui_block_title_spans_fn!(ratatui_paragraph_set_block_title_spans, FfiParagraph);
ratatui_block_adv_fn!(ratatui_paragraph_set_block_adv, FfiParagraph);
ratatui_reserve_vec_fn!(ratatui_paragraph_reserve_lines, FfiParagraph, lines);
ratatui_block_title_alignment_fn!(ratatui_paragraph_set_block_title_alignment, FfiParagraph);

#[no_mangle]
pub extern "C" fn ratatui_paragraph_free(para: *mut FfiParagraph) {
    if para.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(para)) };
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_line(
    para: *mut FfiParagraph,
    text_utf8: *const std::ffi::c_char,
    style: FfiStyle,
) {
    if para.is_null() || text_utf8.is_null() {
        return;
    }
    let p = unsafe { &mut *para };
    let c_str = unsafe { CStr::from_ptr(text_utf8) };
    if let Ok(s) = c_str.to_str() {
        let st = style_from_ffi(style);
        p.lines.push(Line::from(Span::styled(s.to_string(), st)));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_paragraph(
    term: *mut FfiTerminal,
    para: *const FfiParagraph,
) -> bool {
    guard_bool("ratatui_terminal_draw_paragraph", || {
        if term.is_null() || para.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let p = unsafe { &*para };
        let lines = p.lines.clone();
        let mut widget = Paragraph::new(lines);
        if let Some(a) = p.align {
            widget = widget.alignment(a);
        }
        if let Some(trim) = p.wrap_trim {
            widget = widget.wrap(ratatui::widgets::Wrap { trim });
        }
        if let (Some(sx), Some(sy)) = (p.scroll_x, p.scroll_y) {
            widget = widget.scroll((sx, sy));
        }
        if let Some(st) = &p.base_style {
            widget = widget.style(st.clone());
        }
        if let Some(b) = &p.block {
            widget = widget.block(b.clone());
        }
        let res = t.terminal.draw(|frame| {
            let area: Rect = frame.area();
            frame.render_widget(widget.clone(), area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_paragraph_in(
    term: *mut FfiTerminal,
    para: *const FfiParagraph,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_paragraph_in", || {
        if term.is_null() || para.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let p = unsafe { &*para };
        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let lines = p.lines.clone();
        let mut widget = Paragraph::new(lines);
        if let Some(a) = p.align {
            widget = widget.alignment(a);
        }
        if let Some(trim) = p.wrap_trim {
            widget = widget.wrap(ratatui::widgets::Wrap { trim });
        }
        if let (Some(sx), Some(sy)) = (p.scroll_x, p.scroll_y) {
            widget = widget.scroll((sx, sy));
        }
        if let Some(st) = &p.base_style {
            widget = widget.style(st.clone());
        }
        if let Some(b) = &p.block {
            widget = widget.block(b.clone());
        }
        let res = t.terminal.draw(|frame| {
            frame.render_widget(widget.clone(), area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_set_viewport_area(
    term: *mut FfiTerminal,
    rect: FfiRect,
) -> bool {
    let _ = (term, rect);
    false
}
