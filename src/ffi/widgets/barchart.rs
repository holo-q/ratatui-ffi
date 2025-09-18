// Widget split placeholder: BarChart
// Move the following from src/lib.rs into this module:
// - FFI externs: ratatui_barchart_new, ratatui_barchart_free
// - Setters: ratatui_barchart_set_values, ratatui_barchart_set_labels, ratatui_barchart_set_labels_spans,
//   ratatui_barchart_set_bar_width, ratatui_barchart_set_bar_gap, ratatui_barchart_set_styles
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_barchart_set_block_title, FfiBarChart)
//   ratatui_block_title_spans_fn!(ratatui_barchart_set_block_title_spans, FfiBarChart)
//   ratatui_block_adv_fn!(ratatui_barchart_set_block_adv, FfiBarChart)
// - Draw helpers: ratatui_terminal_draw_barchart_in, ratatui_headless_render_barchart
// Types used: FfiBarChart

// use crate::*; // enable when moving implementations

use crate::ffi::widgets::chart::FfiChart;
use crate::{
    ratatui_block_adv_fn, ratatui_block_title_fn, ratatui_block_title_spans_fn, FfiLineSpans,
    FfiRect, FfiStyle, FfiTerminal,
};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::Style;
use ratatui::widgets::{BarChart as RtBarChart, Block};
use std::ffi::{c_char, CStr, CString};

#[repr(C)]
pub struct FfiBarChart {
    pub values: Vec<u64>,
    pub labels: Vec<String>,
    pub block: Option<Block<'static>>,
    pub bar_width: Option<u16>,
    pub bar_gap: Option<u16>,
    pub bar_style: Option<Style>,
    pub value_style: Option<Style>,
    pub label_style: Option<Style>,
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_new() -> *mut FfiBarChart {
    Box::into_raw(Box::new(FfiBarChart {
        values: Vec::new(),
        labels: Vec::new(),
        block: None,
        bar_width: None,
        bar_gap: None,
        bar_style: None,
        value_style: None,
        label_style: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_free(b: *mut FfiBarChart) {
    if b.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(b));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_set_values(b: *mut FfiBarChart, values: *const u64, len: usize) {
    if b.is_null() || values.is_null() {
        return;
    }
    let bc = unsafe { &mut *b };
    let slice = unsafe { std::slice::from_raw_parts(values, len) };
    bc.values = slice.to_vec();
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_set_labels(b: *mut FfiBarChart, tsv_utf8: *const c_char) {
    if b.is_null() || tsv_utf8.is_null() {
        return;
    }
    let bc = unsafe { &mut *b };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() {
        bc.labels = s.split('\t').map(|x| x.to_string()).collect();
    }
}

// Span-based labels: one FfiLineSpans per label; text is concatenated per label
#[no_mangle]
pub extern "C" fn ratatui_barchart_set_labels_spans(
    b: *mut FfiBarChart,
    lines: *const FfiLineSpans,
    len: usize,
) {
    if b.is_null() {
        return;
    }
    let bc = unsafe { &mut *b };
    if lines.is_null() || len == 0 {
        bc.labels.clear();
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(lines, len) };
    let mut labels: Vec<String> = Vec::with_capacity(len);
    for ls in slice.iter() {
        if ls.spans.is_null() || ls.len == 0 {
            labels.push(String::new());
            continue;
        }
        let spans = unsafe { std::slice::from_raw_parts(ls.spans, ls.len) };
        let mut s = String::new();
        for sp in spans.iter() {
            if sp.text_utf8.is_null() {
                continue;
            }
            if let Ok(txt) = unsafe { CStr::from_ptr(sp.text_utf8) }.to_str() {
                s.push_str(txt);
            }
        }
        labels.push(s);
    }
    bc.labels = labels;
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_set_bar_width(b: *mut FfiBarChart, width: u16) {
    if b.is_null() {
        return;
    }
    unsafe {
        (&mut *b).bar_width = Some(width);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_set_bar_gap(b: *mut FfiBarChart, gap: u16) {
    if b.is_null() {
        return;
    }
    unsafe {
        (&mut *b).bar_gap = Some(gap);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_set_styles(
    b: *mut FfiBarChart,
    bar: FfiStyle,
    value: FfiStyle,
    label: FfiStyle,
) {
    if b.is_null() {
        return;
    }
    let bc = unsafe { &mut *b };
    bc.bar_style = Some(crate::style_from_ffi(bar));
    bc.value_style = Some(crate::style_from_ffi(value));
    bc.label_style = Some(crate::style_from_ffi(label));
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_barchart_in(
    term: *mut FfiTerminal,
    b: *const FfiBarChart,
    rect: FfiRect,
) -> bool {
    crate::guard_bool("ratatui_terminal_draw_barchart_in", || {
        if term.is_null() || b.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let bc = unsafe { &*b };
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
        let data: Vec<(&str, u64)> = bc
            .labels
            .iter()
            .map(|s| s.as_str())
            .zip(bc.values.iter().cloned())
            .collect();
        let mut w = RtBarChart::default().data(&data);
        if let Some(bl) = &bc.block {
            w = w.block(bl.clone());
        }
        let res = t.terminal.draw(|frame| {
            frame.render_widget(w.clone(), area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_barchart(
    width: u16,
    height: u16,
    b: *const FfiBarChart,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if b.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let bc = unsafe { &*b };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let data: Vec<(&str, u64)> = bc
        .labels
        .iter()
        .map(|s| s.as_str())
        .zip(bc.values.iter().cloned())
        .collect();
    let mut w = RtBarChart::default().data(&data);
    if let Some(bl) = &bc.block {
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

#[no_mangle]
pub extern "C" fn ratatui_chart_set_labels_alignment(c: *mut FfiChart, x_align: u32, y_align: u32) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    ch.x_labels_align = Some(match x_align {
        1 => Alignment::Center,
        2 => Alignment::Right,
        _ => Alignment::Left,
    });
    ch.y_labels_align = Some(match y_align {
        1 => Alignment::Center,
        2 => Alignment::Right,
        _ => Alignment::Left,
    });
}

// ----- BarChart -----

ratatui_block_title_fn!(ratatui_barchart_set_block_title, FfiBarChart);
ratatui_block_title_spans_fn!(ratatui_barchart_set_block_title_spans, FfiBarChart);
ratatui_block_adv_fn!(ratatui_barchart_set_block_adv, FfiBarChart);
