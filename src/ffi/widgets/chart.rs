// Widget split placeholder: Chart
// Move from src/lib.rs:
// - FFI externs: ratatui_chart_new, ratatui_chart_free
// - Adders: ratatui_chart_add_line, ratatui_chart_add_dataset_with_type, ratatui_chart_add_datasets
// - Setters: ratatui_chart_set_axes_titles, ratatui_chart_set_bounds, ratatui_chart_set_legend_position,
//            ratatui_chart_set_hidden_legend_constraints, ratatui_chart_set_axis_styles,
//            ratatui_chart_set_x_labels_spans, ratatui_chart_set_y_labels_spans,
//            ratatui_chart_set_labels_alignment
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_chart_set_block_title, FfiChart)
//   ratatui_block_title_spans_fn!(ratatui_chart_set_block_title_spans, FfiChart)
//   ratatui_block_adv_fn!(ratatui_chart_set_block_adv, FfiChart)
// - Draw helpers: ratatui_terminal_draw_chart_in, ratatui_headless_render_chart
// Types used: FfiChart, FfiChartDataset, FfiChartDatasetSpec

// use crate::*; // enable when moving implementations

use crate::{
    ratatui_block_adv_fn, ratatui_block_title_fn, ratatui_block_title_spans_fn,
    ratatui_set_style_fn, FfiLineSpans, FfiRect, FfiStyle, FfiTerminal,
};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::prelude::{Line, Style};
use ratatui::widgets::{
    Axis as RtAxis, Block, Chart as RtChart, Dataset as RtDataset, GraphType as RtGraphType,
    LegendPosition as RtLegendPosition,
};
use std::ffi::{c_char, CStr, CString};

#[repr(C)]
pub struct FfiChartDataset {
    pub name: String,
    pub points: Vec<(f64, f64)>,
    pub style: Option<Style>,
    pub kind: u32,
}

#[repr(C)]
pub struct FfiChartDatasetSpec {
    pub name_utf8: *const c_char,
    pub points_xy: *const f64,
    pub len_pairs: usize,
    pub style: FfiStyle,
    pub kind: u32,
}

#[repr(C)]
pub struct FfiChart {
    pub datasets: Vec<FfiChartDataset>,
    pub x_title: Option<String>,
    pub y_title: Option<String>,
    pub block: Option<Block<'static>>,
    pub x_min: Option<f64>,
    pub x_max: Option<f64>,
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
    pub legend_pos: Option<u32>,
    pub hidden_legend_kinds: Option<[u32; 2]>,
    pub hidden_legend_values: Option<[u16; 2]>,
    pub chart_style: Option<Style>,
    pub x_axis_style: Option<Style>,
    pub y_axis_style: Option<Style>,
    pub x_labels: Option<Vec<Line<'static>>>,
    pub y_labels: Option<Vec<Line<'static>>>,
    pub x_labels_align: Option<Alignment>,
    pub y_labels_align: Option<Alignment>,
}

// ----- Chart -----

ratatui_set_style_fn!(ratatui_chart_set_style, FfiChart, chart_style);
ratatui_block_title_fn!(ratatui_chart_set_block_title, FfiChart);
ratatui_block_title_spans_fn!(ratatui_chart_set_block_title_spans, FfiChart);
ratatui_block_adv_fn!(ratatui_chart_set_block_adv, FfiChart);

#[no_mangle]
pub extern "C" fn ratatui_chart_new() -> *mut FfiChart {
    Box::into_raw(Box::new(FfiChart {
        datasets: Vec::new(),
        x_title: None,
        y_title: None,
        block: None,
        x_min: None,
        x_max: None,
        y_min: None,
        y_max: None,
        legend_pos: None,
        hidden_legend_kinds: None,
        hidden_legend_values: None,
        chart_style: None,
        x_axis_style: None,
        y_axis_style: None,
        x_labels: None,
        y_labels: None,
        x_labels_align: None,
        y_labels_align: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_chart_free(c: *mut FfiChart) {
    if c.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(c));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_chart_add_line(
    c: *mut FfiChart,
    name_utf8: *const c_char,
    points_xy: *const f64,
    len_pairs: usize,
    style: FfiStyle,
) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    let name = if name_utf8.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(name_utf8) }
            .to_str()
            .unwrap_or("")
            .to_string()
    };
    let sty = crate::style_from_ffi(style);
    let pts = if points_xy.is_null() || len_pairs == 0 {
        Vec::new()
    } else {
        let slice = unsafe { std::slice::from_raw_parts(points_xy, len_pairs * 2) };
        let mut pts = Vec::with_capacity(len_pairs);
        for i in 0..len_pairs {
            pts.push((slice[i * 2], slice[i * 2 + 1]));
        }
        pts
    };
    ch.datasets.push(FfiChartDataset {
        name,
        points: pts,
        style: Some(sty),
        kind: 0,
    });
}

#[no_mangle]
pub extern "C" fn ratatui_chart_add_dataset_with_type(
    c: *mut FfiChart,
    name_utf8: *const c_char,
    points_xy: *const f64,
    len_pairs: usize,
    style: FfiStyle,
    kind: u32,
) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    let name = if name_utf8.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(name_utf8) }
            .to_str()
            .unwrap_or("")
            .to_string()
    };
    let sty = crate::style_from_ffi(style);
    let pts = if points_xy.is_null() || len_pairs == 0 {
        Vec::new()
    } else {
        let slice = unsafe { std::slice::from_raw_parts(points_xy, len_pairs * 2) };
        let mut pts = Vec::with_capacity(len_pairs);
        for i in 0..len_pairs {
            pts.push((slice[i * 2], slice[i * 2 + 1]));
        }
        pts
    };
    ch.datasets.push(FfiChartDataset {
        name,
        points: pts,
        style: Some(sty),
        kind,
    });
}

#[no_mangle]
pub extern "C" fn ratatui_chart_add_datasets(
    c: *mut FfiChart,
    specs: *const FfiChartDatasetSpec,
    len: usize,
) {
    if c.is_null() || specs.is_null() || len == 0 {
        return;
    }
    let ch = unsafe { &mut *c };
    let slice = unsafe { std::slice::from_raw_parts(specs, len) };
    for s in slice.iter() {
        let name = if s.name_utf8.is_null() {
            String::new()
        } else {
            unsafe { CStr::from_ptr(s.name_utf8) }
                .to_str()
                .unwrap_or("")
                .to_string()
        };
        let pts = if s.points_xy.is_null() || s.len_pairs == 0 {
            Vec::new()
        } else {
            let slice2 = unsafe { std::slice::from_raw_parts(s.points_xy, s.len_pairs * 2) };
            let mut pts = Vec::with_capacity(s.len_pairs);
            for i in 0..s.len_pairs {
                pts.push((slice2[i * 2], slice2[i * 2 + 1]));
            }
            pts
        };
        ch.datasets.push(FfiChartDataset {
            name,
            points: pts,
            style: Some(crate::style_from_ffi(s.style)),
            kind: s.kind,
        });
    }
}

#[no_mangle]
pub extern "C" fn ratatui_chart_set_axes_titles(
    c: *mut FfiChart,
    x_utf8: *const c_char,
    y_utf8: *const c_char,
) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    ch.x_title = if x_utf8.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(x_utf8) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    };
    ch.y_title = if y_utf8.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(y_utf8) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    };
}

#[no_mangle]
pub extern "C" fn ratatui_chart_set_bounds(
    c: *mut FfiChart,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    ch.x_min = Some(x_min);
    ch.x_max = Some(x_max);
    ch.y_min = Some(y_min);
    ch.y_max = Some(y_max);
}

#[no_mangle]
pub extern "C" fn ratatui_chart_set_legend_position(c: *mut FfiChart, pos: u32) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    ch.legend_pos = Some(pos);
}

#[no_mangle]
pub extern "C" fn ratatui_chart_set_hidden_legend_constraints(
    c: *mut FfiChart,
    kinds2: *const u32,
    values2: *const u16,
) {
    if c.is_null() || kinds2.is_null() || values2.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    let kinds = unsafe { std::slice::from_raw_parts(kinds2, 2) };
    let vals = unsafe { std::slice::from_raw_parts(values2, 2) };
    ch.hidden_legend_kinds = Some([kinds[0], kinds[1]]);
    ch.hidden_legend_values = Some([vals[0], vals[1]]);
}

#[no_mangle]
pub extern "C" fn ratatui_chart_set_axis_styles(
    c: *mut FfiChart,
    x_style: FfiStyle,
    y_style: FfiStyle,
) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    ch.x_axis_style = Some(crate::style_from_ffi(x_style));
    ch.y_axis_style = Some(crate::style_from_ffi(y_style));
}

#[no_mangle]
pub extern "C" fn ratatui_chart_set_x_labels_spans(
    c: *mut FfiChart,
    labels: *const FfiLineSpans,
    len: usize,
) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    if labels.is_null() || len == 0 {
        ch.x_labels = None;
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(labels, len) };
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(len);
    for ls in slice.iter() {
        if ls.spans.is_null() || ls.len == 0 {
            lines.push(Line::default());
            continue;
        }
        if let Some(sp) = crate::spans_from_ffi(ls.spans, ls.len) {
            lines.push(Line::from(sp));
        } else {
            lines.push(Line::default());
        }
    }
    ch.x_labels = Some(lines);
}

#[no_mangle]
pub extern "C" fn ratatui_chart_set_y_labels_spans(
    c: *mut FfiChart,
    labels: *const FfiLineSpans,
    len: usize,
) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    if labels.is_null() || len == 0 {
        ch.y_labels = None;
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(labels, len) };
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(len);
    for ls in slice.iter() {
        if ls.spans.is_null() || ls.len == 0 {
            lines.push(Line::default());
            continue;
        }
        if let Some(sp) = crate::spans_from_ffi(ls.spans, ls.len) {
            lines.push(Line::from(sp));
        } else {
            lines.push(Line::default());
        }
    }
    ch.y_labels = Some(lines);
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_chart_in(
    term: *mut FfiTerminal,
    c: *const FfiChart,
    rect: FfiRect,
) -> bool {
    crate::guard_bool("ratatui_terminal_draw_chart_in", || {
        if term.is_null() || c.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let ch = unsafe { &*c };
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
        let mut datasets: Vec<RtDataset> = Vec::new();
        for ds in &ch.datasets {
            let mut d = RtDataset::default().name(ds.name.clone()).data(&ds.points);
            if let Some(sty) = &ds.style {
                d = d.style(sty.clone());
            }
            d = d.graph_type(match ds.kind {
                1 => RtGraphType::Bar,
                2 => RtGraphType::Scatter,
                _ => RtGraphType::Line,
            });
            datasets.push(d);
        }
        let mut w = RtChart::new(datasets);
        let mut x_axis = RtAxis::default();
        let mut y_axis = RtAxis::default();
        if let Some(ti) = &ch.x_title {
            x_axis = x_axis.title(ti.clone());
        }
        if let Some(ti) = &ch.y_title {
            y_axis = y_axis.title(ti.clone());
        }
        if let (Some(min), Some(max)) = (ch.x_min, ch.x_max) {
            x_axis = x_axis.bounds([min, max]);
        }
        if let (Some(min), Some(max)) = (ch.y_min, ch.y_max) {
            y_axis = y_axis.bounds([min, max]);
        }
        if let Some(lbls) = &ch.x_labels {
            x_axis = x_axis.labels(lbls.clone());
        }
        if let Some(lbls) = &ch.y_labels {
            y_axis = y_axis.labels(lbls.clone());
        }
        if let Some(al) = ch.x_labels_align {
            x_axis = x_axis.labels_alignment(al);
        }
        if let Some(al) = ch.y_labels_align {
            y_axis = y_axis.labels_alignment(al);
        }
        w = w.x_axis(x_axis).y_axis(y_axis);
        if let Some(lp) = ch.legend_pos {
            w = w.legend_position(Some(match lp {
                1 => RtLegendPosition::Top,
                2 => RtLegendPosition::Bottom,
                3 => RtLegendPosition::Left,
                4 => RtLegendPosition::Right,
                5 => RtLegendPosition::TopLeft,
                6 => RtLegendPosition::TopRight,
                7 => RtLegendPosition::BottomLeft,
                8 => RtLegendPosition::BottomRight,
                _ => RtLegendPosition::Right,
            }));
        }
        if let (Some(k), Some(v)) = (ch.hidden_legend_kinds, ch.hidden_legend_values) {
            let to_cons = |kind: u32, val: u16| -> Constraint {
                match kind {
                    1 => Constraint::Percentage(val),
                    2 => Constraint::Min(val),
                    _ => Constraint::Length(val),
                }
            };
            w = w.hidden_legend_constraints([to_cons(k[0], v[0]), to_cons(k[1], v[1])].into());
        }
        if let Some(b) = &ch.block {
            w = w.block(b.clone());
        }
        let res = t.terminal.draw(|frame| {
            frame.render_widget(w.clone(), area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_chart(
    width: u16,
    height: u16,
    c: *const FfiChart,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if c.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let ch = unsafe { &*c };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let mut datasets: Vec<RtDataset> = Vec::new();
    for ds in &ch.datasets {
        let mut d = RtDataset::default().name(ds.name.clone()).data(&ds.points);
        if let Some(sty) = &ds.style {
            d = d.style(sty.clone());
        }
        d = d.graph_type(match ds.kind {
            1 => RtGraphType::Bar,
            2 => RtGraphType::Scatter,
            _ => RtGraphType::Line,
        });
        datasets.push(d);
    }
    let mut w = RtChart::new(datasets);
    let mut x_axis = RtAxis::default();
    let mut y_axis = RtAxis::default();
    if let Some(ti) = &ch.x_title {
        x_axis = x_axis.title(ti.clone());
    }
    if let Some(ti) = &ch.y_title {
        y_axis = y_axis.title(ti.clone());
    }
    if let Some(st) = &ch.x_axis_style {
        x_axis = x_axis.style(st.clone());
    }
    if let Some(st) = &ch.y_axis_style {
        y_axis = y_axis.style(st.clone());
    }
    if let (Some(min), Some(max)) = (ch.x_min, ch.x_max) {
        x_axis = x_axis.bounds([min, max]);
    }
    if let (Some(min), Some(max)) = (ch.y_min, ch.y_max) {
        y_axis = y_axis.bounds([min, max]);
    }
    if let Some(lbls) = &ch.x_labels {
        x_axis = x_axis.labels(lbls.clone());
    }
    if let Some(lbls) = &ch.y_labels {
        y_axis = y_axis.labels(lbls.clone());
    }
    if let Some(al) = ch.x_labels_align {
        x_axis = x_axis.labels_alignment(al);
    }
    if let Some(al) = ch.y_labels_align {
        y_axis = y_axis.labels_alignment(al);
    }
    w = w.x_axis(x_axis).y_axis(y_axis);
    if let Some(lp) = ch.legend_pos {
        w = w.legend_position(Some(match lp {
            1 => RtLegendPosition::Top,
            2 => RtLegendPosition::Bottom,
            3 => RtLegendPosition::Left,
            4 => RtLegendPosition::Right,
            5 => RtLegendPosition::TopLeft,
            6 => RtLegendPosition::TopRight,
            7 => RtLegendPosition::BottomLeft,
            8 => RtLegendPosition::BottomRight,
            _ => RtLegendPosition::Right,
        }));
    }
    if let (Some(k), Some(v)) = (ch.hidden_legend_kinds, ch.hidden_legend_values) {
        let to_cons = |kind: u32, val: u16| -> Constraint {
            match kind {
                1 => Constraint::Percentage(val),
                2 => Constraint::Min(val),
                _ => Constraint::Length(val),
            }
        };
        w = w.hidden_legend_constraints([to_cons(k[0], v[0]), to_cons(k[1], v[1])].into());
    }
    if let Some(b) = &ch.block {
        w = w.block(b.clone());
    }
    if let Some(st) = &ch.chart_style {
        w = w.style(st.clone());
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
