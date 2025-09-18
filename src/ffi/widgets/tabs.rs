use crate::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Span};
use ratatui::style::Style;
use ratatui::widgets::{Block, Tabs};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[repr(C)]
pub struct FfiTabs {
    pub titles: Vec<String>,
    pub selected: u16,
    pub block: Option<Block<'static>>,
    pub unselected_style: Option<Style>,
    pub selected_style: Option<Style>,
    pub divider: Option<String>,
    pub divider_span: Option<Span<'static>>,
    pub titles_spans: Option<Vec<Line<'static>>>,
}

#[repr(C)]
pub struct FfiTabsStyles {
    pub unselected: FfiStyle,
    pub selected: FfiStyle,
}

#[no_mangle]
pub extern "C" fn ratatui_tabs_new() -> *mut FfiTabs {
    Box::into_raw(Box::new(FfiTabs {
        titles: Vec::new(),
        selected: 0,
        block: None,
        unselected_style: None,
        selected_style: None,
        divider: None,
        divider_span: None,
        titles_spans: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_tabs_free(t: *mut FfiTabs) {
    if t.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(t));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_tabs_set_titles(t: *mut FfiTabs, tsv_utf8: *const c_char) {
    if t.is_null() || tsv_utf8.is_null() {
        return;
    }
    let tt = unsafe { &mut *t };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() {
        tt.titles = s.split('\t').map(|x| x.to_string()).collect();
    }
}

#[no_mangle]
pub extern "C" fn ratatui_tabs_clear_titles(t: *mut FfiTabs) {
    if t.is_null() {
        return;
    }
    unsafe {
        (&mut *t).titles.clear();
    }
}

#[no_mangle]
pub extern "C" fn ratatui_tabs_add_title_spans(t: *mut FfiTabs, spans: *const FfiSpan, len: usize) {
    if t.is_null() || spans.is_null() {
        return;
    }
    let tt = unsafe { &mut *t };
    if let Some(sp) = spans_from_ffi(spans, len) {
        if tt.titles_spans.is_none() {
            tt.titles_spans = Some(Vec::new());
        }
        tt.titles_spans.as_mut().unwrap().push(Line::from(sp));
    }
}

// Set all tab titles from lines (spans per title), replacing any existing titles
#[no_mangle]
pub extern "C" fn ratatui_tabs_set_titles_spans(
    t: *mut FfiTabs,
    lines: *const FfiLineSpans,
    len: usize,
) {
    if t.is_null() || lines.is_null() || len == 0 {
        return;
    }
    let tt = unsafe { &mut *t };
    tt.titles.clear();
    let slice = unsafe { std::slice::from_raw_parts(lines, len) };
    let mut out: Vec<Line<'static>> = Vec::with_capacity(len);
    for ls in slice.iter() {
        if ls.spans.is_null() || ls.len == 0 {
            out.push(Line::default());
            continue;
        }
        if let Some(sp) = spans_from_ffi(ls.spans, ls.len) {
            out.push(Line::from(sp));
        } else {
            out.push(Line::default());
        }
    }
    tt.titles_spans = Some(out);
}

#[no_mangle]
pub extern "C" fn ratatui_tabs_set_selected(t: *mut FfiTabs, selected: u16) {
    if t.is_null() {
        return;
    }
    unsafe {
        (&mut *t).selected = selected;
    }
}

crate::ratatui_block_title_fn!(ratatui_tabs_set_block_title, FfiTabs);
crate::ratatui_block_title_spans_fn!(ratatui_tabs_set_block_title_spans, FfiTabs);
crate::ratatui_block_title_alignment_fn!(ratatui_tabs_set_block_title_alignment, FfiTabs);
crate::ratatui_block_adv_fn!(ratatui_tabs_set_block_adv, FfiTabs);

#[no_mangle]
pub extern "C" fn ratatui_tabs_set_styles(
    t: *mut FfiTabs,
    unselected: FfiStyle,
    selected: FfiStyle,
) {
    if t.is_null() {
        return;
    }
    let tt = unsafe { &mut *t };
    tt.unselected_style = Some(style_from_ffi(unselected));
    tt.selected_style = Some(style_from_ffi(selected));
}

#[no_mangle]
pub extern "C" fn ratatui_tabs_set_divider(t: *mut FfiTabs, divider_utf8: *const c_char) {
    if t.is_null() || divider_utf8.is_null() {
        return;
    }
    let tt = unsafe { &mut *t };
    tt.divider_span = None; // legacy string path overrides styled divider
    let c_str = unsafe { CStr::from_ptr(divider_utf8) };
    if let Ok(s) = c_str.to_str() {
        tt.divider = Some(s.to_string());
    }
}

// Span-based divider
#[no_mangle]
pub extern "C" fn ratatui_tabs_set_divider_spans(
    t: *mut FfiTabs,
    spans: *const FfiSpan,
    len: usize,
) {
    if t.is_null() {
        return;
    }
    let tt = unsafe { &mut *t };
    tt.divider = None;
    tt.divider_span = None;
    if spans.is_null() || len == 0 {
        return;
    }
    if len == 1 {
        let s = unsafe { &*spans };
        if !s.text_utf8.is_null() {
            if let Ok(txt) = unsafe { CStr::from_ptr(s.text_utf8) }.to_str() {
                tt.divider_span = Some(Span::styled(txt.to_string(), style_from_ffi(s.style)));
                return;
            }
        }
    }
    if let Some(sp) = spans_from_ffi(spans, len) {
        let joined = sp
            .into_iter()
            .map(|spn| spn.content.into_owned())
            .collect::<Vec<_>>()
            .join("");
        tt.divider = Some(joined);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_tabs_in(
    term: *mut FfiTerminal,
    t: *const FfiTabs,
    rect: FfiRect,
) -> bool {
    crate::guard_bool("ratatui_terminal_draw_tabs_in", || {
        if term.is_null() || t.is_null() {
            return false;
        }
        let termi = unsafe { &mut *term };
        let tabs = unsafe { &*t };
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
        let titles: Vec<Line> = if let Some(lines) = &tabs.titles_spans {
            lines.clone()
        } else {
            tabs.titles
                .iter()
                .cloned()
                .map(|s| Line::from(Span::raw(s)))
                .collect()
        };
        let mut widget = Tabs::new(titles).select(tabs.selected as usize);
        if let Some(sty) = &tabs.unselected_style {
            widget = widget.style(sty.clone());
        }
        if let Some(hsty) = &tabs.selected_style {
            widget = widget.highlight_style(hsty.clone());
        }
        if let Some(dsp) = &tabs.divider_span {
            widget = widget.divider(dsp.clone());
        } else if let Some(div) = &tabs.divider {
            if !div.is_empty() {
                widget = widget.divider(Span::raw(div.clone()));
            }
        }
        if let Some(b) = &tabs.block {
            widget = widget.block(b.clone());
        }
        let res = termi.terminal.draw(|frame| {
            frame.render_widget(widget.clone(), area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_tabs(
    width: u16,
    height: u16,
    t: *const FfiTabs,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if t.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let tabs = unsafe { &*t };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let titles: Vec<Line> = if let Some(lines) = &tabs.titles_spans {
        lines.clone()
    } else {
        tabs.titles
            .iter()
            .cloned()
            .map(|s| Line::from(Span::raw(s)))
            .collect()
    };
    let mut widget = Tabs::new(titles).select(tabs.selected as usize);
    if let Some(sty) = &tabs.unselected_style {
        widget = widget.style(sty.clone());
    }
    if let Some(hsty) = &tabs.selected_style {
        widget = widget.highlight_style(hsty.clone());
    }
    if let Some(dsp) = &tabs.divider_span {
        widget = widget.divider(dsp.clone());
    } else if let Some(div) = &tabs.divider {
        if !div.is_empty() {
            widget = widget.divider(Span::raw(div.clone()));
        }
    }
    if let Some(b) = &tabs.block {
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
