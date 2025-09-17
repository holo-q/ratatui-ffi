use crate::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

#[no_mangle]
pub extern "C" fn ratatui_gauge_new() -> *mut FfiGauge {
    Box::into_raw(Box::new(FfiGauge {
        ratio: 0.0,
        label: None,
        block: None,
        style: None,
        label_style: None,
        gauge_style: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_gauge_free(g: *mut FfiGauge) {
    if g.is_null() { return; }
    unsafe { drop(Box::from_raw(g)); }
}

#[no_mangle]
pub extern "C" fn ratatui_gauge_set_ratio(g: *mut FfiGauge, ratio: f32) {
    if g.is_null() { return; }
    unsafe { (&mut *g).ratio = ratio.clamp(0.0, 1.0); }
}

#[no_mangle]
pub extern "C" fn ratatui_gauge_set_label(g: *mut FfiGauge, label: *const c_char) {
    if g.is_null() { return; }
    let gg = unsafe { &mut *g };
    gg.label = if label.is_null() { None } else { unsafe { CStr::from_ptr(label) }.to_str().ok().map(|s| s.to_string()) };
}

// Span-based label for Gauge (preferred)
#[no_mangle]
pub extern "C" fn ratatui_gauge_set_label_spans(
    g: *mut FfiGauge,
    spans: *const FfiSpan,
    len: usize,
) {
    if g.is_null() { return; }
    let gg = unsafe { &mut *g };
    if spans.is_null() || len == 0 { gg.label = Some(String::new()); return; }
    let slice = unsafe { std::slice::from_raw_parts(spans, len) };
    let mut s = String::new();
    for sp in slice.iter() {
        if sp.text_utf8.is_null() { continue; }
        if let Ok(txt) = unsafe { CStr::from_ptr(sp.text_utf8) }.to_str() { s.push_str(txt); }
    }
    gg.label = Some(s);
}

#[no_mangle]
pub extern "C" fn ratatui_gauge_set_styles(
    g: *mut FfiGauge,
    style: FfiStyle,
    label_style: FfiStyle,
    gauge_style: FfiStyle,
) {
    if g.is_null() { return; }
    let gg = unsafe { &mut *g };
    gg.style = Some(style_from_ffi(style));
    gg.label_style = Some(style_from_ffi(label_style));
    gg.gauge_style = Some(style_from_ffi(gauge_style));
}

crate::ratatui_block_title_fn!(ratatui_gauge_set_block_title, FfiGauge);
crate::ratatui_block_title_spans_fn!(ratatui_gauge_set_block_title_spans, FfiGauge);
crate::ratatui_block_title_alignment_fn!(ratatui_gauge_set_block_title_alignment, FfiGauge);
crate::ratatui_block_adv_fn!(ratatui_gauge_set_block_adv, FfiGauge);
crate::ratatui_block_title_fn!(ratatui_linegauge_set_block_title, FfiLineGauge);
crate::ratatui_block_title_spans_fn!(ratatui_linegauge_set_block_title_spans, FfiLineGauge);
crate::ratatui_block_title_alignment_fn!(ratatui_linegauge_set_block_title_alignment, FfiLineGauge);
crate::ratatui_block_adv_fn!(ratatui_linegauge_set_block_adv, FfiLineGauge);
crate::ratatui_set_style_fn!(ratatui_linegauge_set_style, FfiLineGauge, style);

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_gauge_in(
    term: *mut FfiTerminal,
    g: *const FfiGauge,
    rect: FfiRect,
) -> bool {
    crate::guard_bool("ratatui_terminal_draw_gauge_in", || {
        if term.is_null() || g.is_null() { return false; }
        let t = unsafe { &mut *term };
        let gg = unsafe { &*g };
        let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
        let mut widget = Gauge::default().ratio(gg.ratio as f64);
        if let Some(st) = &gg.style { widget = widget.style(st.clone()); }
        if let Some(label) = &gg.label { widget = widget.label(label.clone()); }
        if let Some(st) = &gg.label_style { widget = widget.set_style(st.clone()); }
        if let Some(st) = &gg.gauge_style { widget = widget.gauge_style(st.clone()); }
        if let Some(b) = &gg.block { widget = widget.block(b.clone()); }
        let res = t.terminal.draw(|frame| { frame.render_widget(widget.clone(), area); });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_gauge(
    width: u16,
    height: u16,
    g: *const FfiGauge,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if g.is_null() || out_text_utf8.is_null() { return false; }
    let gg = unsafe { &*g };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let mut w = Gauge::default().ratio(gg.ratio as f64);
    if let Some(st) = &gg.style { w = w.style(st.clone()); }
    if let Some(label) = &gg.label { w = w.label(label.clone()); }
    if let Some(st) = &gg.label_style { w = w.set_style(st.clone()); }
    if let Some(st) = &gg.gauge_style { w = w.gauge_style(st.clone()); }
    if let Some(b) = &gg.block { w = w.block(b.clone()); }
    ratatui::widgets::Widget::render(w, area, &mut buf);
    let mut s = String::new();
    for y in 0..height { for x in 0..width { s.push_str(buf[(x, y)].symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

#[repr(C)]
pub struct FfiGauge {
    pub ratio: f32,
    pub label: Option<String>,
    pub block: Option<Block<'static>>,
    pub style: Option<Style>,
    pub label_style: Option<Style>,
    pub gauge_style: Option<Style>,
}

#[repr(C)]
pub struct FfiLineGauge {
    pub ratio: f32,
    pub label: Option<String>,
    pub label_line: Option<Line<'static>>,
    pub block: Option<Block<'static>>,
    pub style: Option<Style>,
}

#[no_mangle]
pub extern "C" fn ratatui_linegauge_new() -> *mut FfiLineGauge {
    Box::into_raw(Box::new(FfiLineGauge {
        ratio: 0.0,
        label: None,
        label_line: None,
        block: None,
        style: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_linegauge_free(g: *mut FfiLineGauge) {
    if g.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(g));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_linegauge_set_ratio(g: *mut FfiLineGauge, ratio: f32) {
    if g.is_null() {
        return;
    }
    unsafe {
        (&mut *g).ratio = ratio;
    }
}

#[no_mangle]
pub extern "C" fn ratatui_linegauge_set_label(g: *mut FfiLineGauge, label_utf8: *const std::ffi::c_char) {
    if g.is_null() {
        return;
    }
    let gg = unsafe { &mut *g };
    gg.label = if label_utf8.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(label_utf8) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    };
}

// Span-based label for LineGauge (preferred; avoids allocations in hot paths)
#[no_mangle]
pub extern "C" fn ratatui_linegauge_set_label_spans(
    g: *mut FfiLineGauge,
    spans: *const FfiSpan,
    len: usize,
) {
    if g.is_null() {
        return;
    }
    let gg = unsafe { &mut *g };
    if spans.is_null() || len == 0 {
        gg.label_line = Some(Line::default());
        gg.label = None;
        return;
    }
    if let Some(sp) = spans_from_ffi(spans, len) {
        gg.label_line = Some(Line::from(sp));
        gg.label = None; // prefer spans over legacy string label
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_linegauge_in(
    term: *mut FfiTerminal,
    g: *const FfiLineGauge,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_linegauge_in", || {
        if term.is_null() || g.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let gg = unsafe { &*g };
        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let mut w = RtLineGauge::default().ratio(gg.ratio as f64);
        if let Some(lbl) = &gg.label_line {
            w = w.label(lbl.clone());
        } else if let Some(label) = &gg.label {
            w = w.label(label.clone());
        }
        if let Some(st) = &gg.style {
            w = w.style(st.clone());
        }
        if let Some(b) = &gg.block {
            w = w.block(b.clone());
        }
        let res = t.terminal.draw(|frame| {
            frame.render_widget(w.clone(), area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_linegauge(
	width: u16,
	height: u16,
	g: *const FfiLineGauge,
	out_text_utf8: *mut *mut std::ffi::c_char,
) -> bool {
    if g.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let gg = unsafe { &*g };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let mut w = RtLineGauge::default().ratio(gg.ratio as f64);
    if let Some(lbl) = &gg.label_line {
        w = w.label(lbl.clone());
    } else if let Some(label) = &gg.label {
        w = w.label(label.clone());
    }
    if let Some(st) = &gg.style {
        w = w.style(st.clone());
    }
    if let Some(b) = &gg.block {
        w = w.block(b.clone());
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

// moved to widgets::layout
