#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::ffi::{c_char, CStr, CString};
use std::io::{stdout, Stdout, Write};
use std::ptr;

use crossterm::event::{
    self, Event as CtEvent, KeyCode as CtKeyCode, KeyEvent as CtKeyEvent,
    KeyModifiers as CtKeyModifiers, MouseButton as CtMouseButton, MouseEvent as CtMouseEvent,
    MouseEventKind as CtMouseKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style, Styled};
use ratatui::symbols::Marker as RtMarker;
use ratatui::widgets::canvas::{
    Canvas as RtCanvas, Line as RtCanvasLine, Points as RtCanvasPoints, Rectangle as RtCanvasRect,
};
use ratatui::widgets::LegendPosition as RtLegendPosition;
use ratatui::widgets::ListDirection as RtListDirection;
use ratatui::widgets::{
    Axis as RtAxis, BarChart as RtBarChart, Block, BorderType as RtBorderType, Borders, Cell,
    Chart as RtChart, Clear as RtClear, Dataset as RtDataset, Gauge, GraphType as RtGraphType,
    HighlightSpacing as RtHighlightSpacing, LineGauge as RtLineGauge, List, ListItem,
    Padding as RtPadding, Paragraph, RatatuiLogo as RtRatatuiLogo, Row, Sparkline as RtSparkline,
    Table, Tabs,
};
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
use ratatui::widgets::{
    Scrollbar as RtScrollbar, ScrollbarOrientation as RtScrollbarOrientation,
    ScrollbarState as RtScrollbarState,
};
mod ffi;
use std::collections::VecDeque;
use std::sync::Mutex;

// ----- Panic guard helpers -----
use std::any::Any;
use std::fs::OpenOptions;
use std::panic::{catch_unwind, UnwindSafe};
use std::sync::OnceLock;

fn panic_message(e: Box<dyn Any + Send + 'static>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

fn maybe_backtrace() -> Option<String> {
    let enabled = std::env::var("RUST_BACKTRACE")
        .map(|v| v != "0")
        .unwrap_or(false);
    if enabled {
        Some(format!("{}", std::backtrace::Backtrace::force_capture()))
    } else {
        None
    }
}

static LOG_FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();

fn logger() -> Option<&'static Mutex<std::fs::File>> {
    LOG_FILE
        .get_or_init(|| {
            if let Ok(path) = std::env::var("RATATUI_FFI_LOG") {
                if !path.is_empty() {
                    // By default, recreate (truncate) the log on each run so users don't have to delete it.
                    // Set RATATUI_FFI_LOG_APPEND=1 to append instead.
                    let append = std::env::var("RATATUI_FFI_LOG_APPEND")
                        .map(|v| v != "0" && !v.is_empty())
                        .unwrap_or(false);
                    let mut opts = OpenOptions::new();
                    opts.create(true).write(true);
                    if append {
                        opts.append(true);
                    } else {
                        opts.truncate(true);
                    }
                    if let Ok(f) = opts.open(path) {
                        return Some(Mutex::new(f));
                    }
                }
            }
            None
        })
        .as_ref()
}

fn log_line(s: &str) {
    if let Some(m) = logger() {
        if let Ok(mut f) = m.lock() {
            let _ = writeln!(f, "{}", s);
        }
    }
}

fn slice_checked<'a, T>(ptr: *const T, len: usize, ctx: &str) -> Option<&'a [T]> {
    if ptr.is_null() {
        eprintln!("ratatui_ffi {}: null pointer", ctx);
        log_line(&format!("{}: null pointer", ctx));
        return None;
    }
    let align = std::mem::align_of::<T>();
    let addr = ptr as usize;
    if align != 0 && (addr % align) != 0 {
        eprintln!(
            "ratatui_ffi {}: misaligned pointer addr={:#x} align={}",
            ctx, addr, align
        );
        log_line(&format!(
            "{}: misaligned pointer addr={:#x} align={}",
            ctx, addr, align
        ));
        return None;
    }
    let size = std::mem::size_of::<T>();
    if size == 0 {
        unsafe {
            return Some(std::slice::from_raw_parts(ptr, len));
        }
    }
    if let Some(total) = len.checked_mul(size) {
        if total > isize::MAX as usize {
            eprintln!(
                "ratatui_ffi {}: slice too large len={} size={} total>{}",
                ctx,
                len,
                size,
                isize::MAX
            );
            log_line(&format!(
                "{}: slice too large len={} size={}",
                ctx, len, size
            ));
            return None;
        }
    } else {
        eprintln!(
            "ratatui_ffi {}: size overflow len={} size={}",
            ctx, len, size
        );
        log_line(&format!("{}: size overflow len={} size={}", ctx, len, size));
        return None;
    }
    unsafe { Some(std::slice::from_raw_parts(ptr, len)) }
}

fn ptr_checked<'a, T>(ptr: *const T, ctx: &str) -> Option<&'a T> {
    if ptr.is_null() {
        eprintln!("ratatui_ffi {}: null handle", ctx);
        log_line(&format!("{}: null handle", ctx));
        return None;
    }
    let align = std::mem::align_of::<T>();
    let addr = ptr as usize;
    if align != 0 && (addr % align) != 0 {
        eprintln!(
            "ratatui_ffi {}: misaligned handle addr={:#x} align={}",
            ctx, addr, align
        );
        log_line(&format!(
            "{}: misaligned handle addr={:#x} align={}",
            ctx, addr, align
        ));
        return None;
    }
    unsafe { Some(&*ptr) }
}

fn guard_bool<F: FnOnce() -> bool + UnwindSafe>(name: &str, f: F) -> bool {
    let trace = std::env::var("RATATUI_FFI_TRACE").is_ok();
    if trace {
        eprintln!("ratatui_ffi ENTER {}", name);
        log_line(&format!("ENTER {}", name));
    }
    let out = match catch_unwind(f) {
        Ok(v) => v,
        Err(e) => {
            let msg = panic_message(e);
            eprintln!("ratatui_ffi PANIC {}: {}", name, msg);
            log_line(&format!("PANIC {}: {}", name, msg));
            if let Some(bt) = maybe_backtrace() {
                eprintln!("{}", bt);
            }
            false
        }
    };
    if trace {
        eprintln!("ratatui_ffi EXIT  {} -> {}", name, out);
        log_line(&format!("EXIT  {} -> {}", name, out));
    }
    out
}

fn guard_ptr<T, F: FnOnce() -> *mut T + UnwindSafe>(name: &str, f: F) -> *mut T {
    let trace = std::env::var("RATATUI_FFI_TRACE").is_ok();
    if trace {
        eprintln!("ratatui_ffi ENTER {}", name);
        log_line(&format!("ENTER {}", name));
    }
    let out = match catch_unwind(f) {
        Ok(v) => v,
        Err(e) => {
            let msg = panic_message(e);
            eprintln!("ratatui_ffi PANIC {}: {}", name, msg);
            log_line(&format!("PANIC {}: {}", name, msg));
            if let Some(bt) = maybe_backtrace() {
                eprintln!("{}", bt);
            }
            std::ptr::null_mut()
        }
    };
    if trace {
        eprintln!("ratatui_ffi EXIT  {} -> {:?}", name, out);
        log_line(&format!("EXIT  {} -> {:?}", name, out));
    }
    out
}

fn guard_void<F: FnOnce() + UnwindSafe>(name: &str, f: F) {
    let trace = std::env::var("RATATUI_FFI_TRACE").is_ok();
    if trace {
        eprintln!("ratatui_ffi ENTER {}", name);
        log_line(&format!("ENTER {}", name));
    }
    if let Err(e) = catch_unwind(f) {
        let msg = panic_message(e);
        eprintln!("ratatui_ffi PANIC {}: {}", name, msg);
        log_line(&format!("PANIC {}: {}", name, msg));
        if let Some(bt) = maybe_backtrace() {
            eprintln!("{}", bt);
        }
    }
    if trace {
        eprintln!("ratatui_ffi EXIT  {}", name);
        log_line(&format!("EXIT  {}", name));
    }
}

#[repr(C)]
pub struct FfiTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    entered_alt: bool,
    raw_mode: bool,
}

#[repr(C)]
pub struct FfiParagraph {
    lines: Vec<Line<'static>>,     // content
    block: Option<Block<'static>>, // optional block with borders/title
    align: Option<ratatui::layout::Alignment>,
    wrap_trim: Option<bool>,
    scroll_x: Option<u16>,
    scroll_y: Option<u16>,
    base_style: Option<Style>,
}

#[repr(C)]
pub struct FfiList {
    items: Vec<Line<'static>>,
    block: Option<Block<'static>>,
    selected: Option<usize>,
    highlight_style: Option<Style>,
    highlight_symbol: Option<String>,
    direction: Option<RtListDirection>,
    scroll_offset: Option<usize>,
    highlight_spacing: Option<RtHighlightSpacing>,
}

#[repr(C)]
pub struct FfiListState {
    selected: Option<usize>,
    offset: usize,
}

#[repr(C)]
pub struct FfiTableState {
    selected: Option<usize>,
    offset: usize,
}

#[repr(C)]
pub struct FfiGauge {
    ratio: f32,
    label: Option<String>,
    block: Option<Block<'static>>,
    style: Option<Style>,
    label_style: Option<Style>,
    gauge_style: Option<Style>,
}

#[repr(C)]
pub struct FfiLineGauge {
    ratio: f32,
    label: Option<String>,
    label_line: Option<Line<'static>>,
    block: Option<Block<'static>>,
    style: Option<Style>,
}

#[repr(C)]
pub struct FfiTabs {
    titles: Vec<String>,
    selected: u16,
    block: Option<Block<'static>>,
    unselected_style: Option<Style>,
    selected_style: Option<Style>,
    divider: Option<String>,
    // Optional styled divider span; preferred if set
    divider_span: Option<Span<'static>>,
    titles_spans: Option<Vec<Line<'static>>>,
}
#[repr(C)]
pub struct FfiTabsStyles {
    pub unselected: FfiStyle,
    pub selected: FfiStyle,
}

#[repr(C)]
pub struct FfiBarChart {
    values: Vec<u64>,
    labels: Vec<String>,
    block: Option<Block<'static>>,
    bar_width: Option<u16>,
    bar_gap: Option<u16>,
    bar_style: Option<Style>,
    value_style: Option<Style>,
    label_style: Option<Style>,
}

#[repr(C)]
pub struct FfiSparkline {
    values: Vec<u64>,
    block: Option<Block<'static>>,
    max: Option<u64>,
    style: Option<Style>,
}

#[repr(C)]
pub struct FfiChartDataset {
    name: String,
    points: Vec<(f64, f64)>,
    style: Option<Style>,
    kind: u32,
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
    datasets: Vec<FfiChartDataset>,
    x_title: Option<String>,
    y_title: Option<String>,
    block: Option<Block<'static>>,
    x_min: Option<f64>,
    x_max: Option<f64>,
    y_min: Option<f64>,
    y_max: Option<f64>,
    legend_pos: Option<u32>,
    hidden_legend_kinds: Option<[u32; 2]>,
    hidden_legend_values: Option<[u16; 2]>,
    chart_style: Option<Style>,
    x_axis_style: Option<Style>,
    y_axis_style: Option<Style>,
    x_labels: Option<Vec<Line<'static>>>,
    y_labels: Option<Vec<Line<'static>>>,
    x_labels_align: Option<ratatui::layout::Alignment>,
    y_labels_align: Option<ratatui::layout::Alignment>,
}

// ----- Canvas -----

#[repr(C)]
pub struct FfiCanvasLine {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub style: FfiStyle,
}

#[repr(C)]
pub struct FfiCanvasPoints {
    pub points_xy: *const f64,
    pub len_pairs: usize,
    pub style: FfiStyle,
    pub marker: u32,
}

#[repr(C)]
pub struct FfiCanvasRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub style: FfiStyle,
    pub filled: bool,
}

#[repr(C)]
pub struct FfiCanvas {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
    background: Option<Color>,
    block: Option<Block<'static>>,
    marker: Option<RtMarker>,
    lines: Vec<FfiCanvasLine>,
    rects: Vec<FfiCanvasRect>,
    pts: Vec<(Vec<(f64, f64)>, Color)>,
}

#[no_mangle]
pub extern "C" fn ratatui_canvas_new(
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
) -> *mut FfiCanvas {
    Box::into_raw(Box::new(FfiCanvas {
        x_min,
        x_max,
        y_min,
        y_max,
        background: None,
        block: None,
        marker: None,
        lines: Vec::new(),
        rects: Vec::new(),
        pts: Vec::new(),
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_canvas_free(c: *mut FfiCanvas) {
    if c.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(c));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_canvas_set_bounds(
    c: *mut FfiCanvas,
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
) {
    if c.is_null() {
        return;
    }
    unsafe {
        let cv = &mut *c;
        cv.x_min = x_min;
        cv.x_max = x_max;
        cv.y_min = y_min;
        cv.y_max = y_max;
    }
}

#[no_mangle]
pub extern "C" fn ratatui_canvas_set_background_color(c: *mut FfiCanvas, color: u32) {
    if c.is_null() {
        return;
    }
    unsafe {
        (&mut *c).background = color_from_u32(color);
    }
}

crate::ratatui_block_title_fn!(ratatui_canvas_set_block_title, FfiCanvas);
crate::ratatui_block_title_spans_fn!(ratatui_canvas_set_block_title_spans, FfiCanvas);

#[no_mangle]
pub extern "C" fn ratatui_canvas_set_block_adv(
    c: *mut FfiCanvas,
    borders_bits: u8,
    border_type: u32,
    pad_l: u16,
    pad_t: u16,
    pad_r: u16,
    pad_b: u16,
    title_spans: *const FfiSpan,
    title_len: usize,
) {
    if c.is_null() {
        return;
    }
    let cv = unsafe { &mut *c };
    cv.block = Some(build_block_from_adv(
        borders_bits,
        border_type,
        pad_l,
        pad_t,
        pad_r,
        pad_b,
        title_spans,
        title_len,
    ));
}

#[no_mangle]
pub extern "C" fn ratatui_canvas_set_marker(c: *mut FfiCanvas, marker: u32) {
    if c.is_null() {
        return;
    }
    let cv = unsafe { &mut *c };
    cv.marker = Some(match marker {
        1 => RtMarker::Braille,
        2 => RtMarker::Block,
        3 => RtMarker::HalfBlock,
        _ => RtMarker::Dot,
    });
}

#[no_mangle]
pub extern "C" fn ratatui_canvas_add_line(
    c: *mut FfiCanvas,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    style: FfiStyle,
) {
    if c.is_null() {
        return;
    }
    let cv = unsafe { &mut *c };
    cv.lines.push(FfiCanvasLine {
        x1,
        y1,
        x2,
        y2,
        style,
    });
}

#[no_mangle]
pub extern "C" fn ratatui_canvas_add_rect(
    c: *mut FfiCanvas,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    style: FfiStyle,
    filled: bool,
) {
    if c.is_null() {
        return;
    }
    let cv = unsafe { &mut *c };
    cv.rects.push(FfiCanvasRect {
        x,
        y,
        w,
        h,
        style,
        filled,
    });
}

#[no_mangle]
pub extern "C" fn ratatui_canvas_add_points(
    c: *mut FfiCanvas,
    points_xy: *const f64,
    len_pairs: usize,
    style: FfiStyle,
    marker: u32,
) {
    if c.is_null() {
        return;
    }
    let cv = unsafe { &mut *c };
    if points_xy.is_null() || len_pairs == 0 {
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(points_xy, len_pairs * 2) };
    let mut pts: Vec<(f64, f64)> = Vec::with_capacity(len_pairs);
    for i in 0..len_pairs {
        pts.push((slice[i * 2], slice[i * 2 + 1]));
    }
    let col = color_from_u32(style.fg).unwrap_or(Color::White);
    let _ = marker; // marker not supported in ratatui 0.29 Points shape; ignored
    cv.pts.push((pts, col));
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_canvas_in(
    term: *mut FfiTerminal,
    c: *const FfiCanvas,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_canvas_in", || {
        if term.is_null() || c.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let cv = unsafe { &*c };
        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let mut w = RtCanvas::default()
            .x_bounds([cv.x_min, cv.x_max])
            .y_bounds([cv.y_min, cv.y_max]);
        if let Some(bg) = cv.background {
            w = w.background_color(bg);
        }
        if let Some(b) = &cv.block {
            w = w.block(b.clone());
        }
        if let Some(mk) = cv.marker {
            w = w.marker(mk);
        }
        w = w.paint(|p| {
            for l in &cv.lines {
                let col = color_from_u32(l.style.fg).unwrap_or(Color::White);
                p.draw(&RtCanvasLine {
                    x1: l.x1,
                    y1: l.y1,
                    x2: l.x2,
                    y2: l.y2,
                    color: col,
                });
            }
            for r in &cv.rects {
                let col = color_from_u32(r.style.fg).unwrap_or(Color::White);
                p.draw(&RtCanvasRect {
                    x: r.x,
                    y: r.y,
                    width: r.w,
                    height: r.h,
                    color: col,
                });
            }
            for (pts, col) in &cv.pts {
                p.draw(&RtCanvasPoints {
                    coords: &pts[..],
                    color: *col,
                });
            }
        });
        let res = t.terminal.draw(|frame| {
            frame.render_widget(w.clone(), area);
        });
        res.is_ok()
    })
}

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
#[repr(u32)]
pub enum FfiScrollbarOrient {
    Vertical = 0,
    Horizontal = 1,
}

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
#[repr(C)]
pub struct FfiScrollbar {
    orient: u32,
    position: u16,
    content_len: u16,
    viewport_len: u16,
    block: Option<Block<'static>>,
    side: Option<u32>,
}

#[repr(C)]
pub struct FfiRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[repr(C)]
pub struct FfiCellInfo {
    pub ch: u32,
    pub fg: u32,
    pub bg: u32,
    pub mods: u16,
}

#[repr(u32)]
pub enum FfiEventKind {
    None = 0,
    Key = 1,
    Resize = 2,
    Mouse = 3,
}

#[repr(u32)]
pub enum FfiKeyCode {
    Char = 0,
    Enter = 1,
    Left = 2,
    Right = 3,
    Up = 4,
    Down = 5,
    Esc = 6,
    Backspace = 7,
    Tab = 8,
    Delete = 9,
    Home = 10,
    End = 11,
    PageUp = 12,
    PageDown = 13,
    Insert = 14,
    F1 = 100,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct FfiKeyMods: u8 {
        const NONE = 0;
        const SHIFT = 1<<0;
        const ALT = 1<<1;
        const CTRL = 1<<2;
    }
}

#[repr(C)]
pub struct FfiKeyEvent {
    pub code: u32,
    pub ch: u32,
    pub mods: u8,
}

#[repr(C)]
pub struct FfiEvent {
    pub kind: u32,
    pub key: FfiKeyEvent,
    pub width: u16,
    pub height: u16,
    pub mouse_x: u16,
    pub mouse_y: u16,
    pub mouse_kind: u32,
    pub mouse_btn: u32,
    pub mouse_mods: u8,
}

#[repr(u32)]
pub enum FfiMouseKind {
    Down = 1,
    Up = 2,
    Drag = 3,
    Moved = 4,
    ScrollUp = 5,
    ScrollDown = 6,
}

#[repr(u32)]
pub enum FfiMouseButton {
    Left = 1,
    Right = 2,
    Middle = 3,
    None = 0,
}

#[no_mangle]
pub extern "C" fn ratatui_init_terminal() -> *mut FfiTerminal {
    guard_ptr("ratatui_init_terminal", || {
        let mut out = stdout();
        // Raw mode ON by default to avoid key echo; can disable via RATATUI_FFI_NO_RAW=1
        let want_raw = std::env::var("RATATUI_FFI_NO_RAW").is_err();
        let use_alt = std::env::var("RATATUI_FFI_ALTSCR").is_ok();
        let mut entered_alt = false;
        let mut raw_mode = false;
        if want_raw {
            if enable_raw_mode().is_ok() {
                raw_mode = true;
            }
        }
        if use_alt {
            if execute!(out, EnterAlternateScreen).is_ok() {
                entered_alt = true;
            }
        }
        let backend = CrosstermBackend::new(out);
        match Terminal::new(backend) {
            Ok(mut terminal) => {
                let _ = terminal.hide_cursor();
                // If we are not using the alternate screen, the visible buffer may contain
                // previous shell output. Force a full clear on first draw so the UI doesn't
                // appear additively over existing text.
                let _ = terminal.clear();
                Box::into_raw(Box::new(FfiTerminal {
                    terminal,
                    entered_alt,
                    raw_mode,
                }))
            }
            Err(_) => {
                if raw_mode {
                    let _ = disable_raw_mode();
                }
                ptr::null_mut()
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_clear(term: *mut FfiTerminal) {
    guard_void("ratatui_terminal_clear", || {
        if term.is_null() {
            return;
        }
        let t = unsafe { &mut *term };
        let _ = t.terminal.clear();
    })
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_free(term: *mut FfiTerminal) {
    guard_void("ratatui_terminal_free", || {
        if term.is_null() {
            return;
        }
        // Take ownership and drop after restoring terminal state
        let mut boxed = unsafe { Box::from_raw(term) };
        let _ = boxed.terminal.show_cursor();
        // Leave alternate screen and disable raw mode if we enabled them
        if boxed.entered_alt {
            let _ = execute!(stdout(), LeaveAlternateScreen);
        }
        if boxed.raw_mode {
            let _ = disable_raw_mode();
        }
        // Drop happens here
    })
}

#[repr(u32)]
pub enum FfiAlign {
    Left = 0,
    Center = 1,
    Right = 2,
}


crate::ratatui_block_adv_fn!(ratatui_list_set_block_adv, FfiList);

crate::ratatui_block_adv_fn!(ratatui_table_set_block_adv, FfiTable);

crate::ratatui_block_adv_fn!(ratatui_gauge_set_block_adv, FfiGauge);

crate::ratatui_block_adv_fn!(ratatui_linegauge_set_block_adv, FfiLineGauge);

crate::ratatui_block_adv_fn!(ratatui_tabs_set_block_adv, FfiTabs);

// paragraph externs moved to crate::ffi::widgets::paragraph

// ----- Styles -----

#[repr(u32)]
pub enum FfiColor {
    Reset = 0,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    Gray,
    DarkGray,
    LightRed,
    LightGreen,
    LightYellow,
    LightBlue,
    LightMagenta,
    LightCyan,
    White,
    Indexed,
    Rgb,
}

// Additional FFI enums to mirror Ratatui public enums for coverage and clarity
// These are ABI-safe mirrors; functions may still use u32 where appropriate for stability.

#[repr(u32)]
pub enum FfiAlignment { Left = 0, Center = 1, Right = 2 }

#[repr(u32)]
pub enum FfiDirection { Horizontal = 0, Vertical = 1 }

#[repr(u32)]
pub enum FfiFlex { Legacy = 0, Start = 1, End = 2, Center = 3, SpaceBetween = 4, SpaceAround = 5 }

#[repr(u32)]
pub enum FfiGraphType { Scatter = 0, Line = 1, Bar = 2 }

#[repr(u32)]
pub enum FfiLegendPosition {
    Top = 0,
    TopRight = 1,
    TopLeft = 2,
    Left = 3,
    Right = 4,
    Bottom = 5,
    BottomRight = 6,
    BottomLeft = 7,
}

#[repr(u32)]
pub enum FfiRenderDirection { LeftToRight = 0, RightToLeft = 1 }

#[repr(u32)]
pub enum FfiListDirection { TopToBottom = 0, BottomToTop = 1, LeftToRight = 2 }

#[cfg(feature = "scrollbar")]
#[repr(u32)]
pub enum FfiScrollbarOrientation { VerticalRight = 0, VerticalLeft = 1, HorizontalBottom = 2, HorizontalTop = 3 }

#[cfg(feature = "scrollbar")]
#[repr(u32)]
pub enum FfiScrollDirection { Forward = 0, Backward = 1 }

#[repr(u32)]
pub enum FfiPosition { Top = 0, Bottom = 1 }

#[repr(u32)]
pub enum FfiMapResolution { Low = 0, High = 1 }

#[repr(u32)]
pub enum FfiSize { Tiny = 0, Small = 1 }

#[repr(u32)]
pub enum FfiConstraint { Min = 0, Max = 1, Length = 2, Percentage = 3, Ratio = 4, Fill = 5 }

#[repr(u32)]
pub enum FfiSpacing { Space = 0, Overlap = 1 }

#[repr(u32)]
pub enum FfiMarker { Dot = 0, Block = 1, Bar = 2, Braille = 3, HalfBlock = 4 }

#[repr(u32)]
pub enum FfiClearType { All = 0, AfterCursor = 1, BeforeCursor = 2, CurrentLine = 3, UntilNewLine = 4 }

#[repr(u32)]
pub enum FfiViewport { Fullscreen = 0, Inline = 1, Fixed = 2 }

// ----- Ratatui Symbols & Palettes (generated) -----
include!("ffi/generated.rs");
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_vertical, ratatui::symbols::line::DOUBLE_VERTICAL);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_vertical, ratatui::symbols::line::THICK_VERTICAL);
crate::ratatui_const_str_getter!(ratatui_symbols_get_horizontal, ratatui::symbols::line::HORIZONTAL);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_horizontal, ratatui::symbols::line::DOUBLE_HORIZONTAL);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_horizontal, ratatui::symbols::line::THICK_HORIZONTAL);
crate::ratatui_const_str_getter!(ratatui_symbols_get_top_right, ratatui::symbols::line::TOP_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_rounded_top_right, ratatui::symbols::line::ROUNDED_TOP_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_top_right, ratatui::symbols::line::DOUBLE_TOP_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_top_right, ratatui::symbols::line::THICK_TOP_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_top_left, ratatui::symbols::line::TOP_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_rounded_top_left, ratatui::symbols::line::ROUNDED_TOP_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_top_left, ratatui::symbols::line::DOUBLE_TOP_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_top_left, ratatui::symbols::line::THICK_TOP_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bottom_right, ratatui::symbols::line::BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_rounded_bottom_right, ratatui::symbols::line::ROUNDED_BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_bottom_right, ratatui::symbols::line::DOUBLE_BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_bottom_right, ratatui::symbols::line::THICK_BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bottom_left, ratatui::symbols::line::BOTTOM_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_rounded_bottom_left, ratatui::symbols::line::ROUNDED_BOTTOM_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_bottom_left, ratatui::symbols::line::DOUBLE_BOTTOM_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_bottom_left, ratatui::symbols::line::THICK_BOTTOM_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_vertical_left, ratatui::symbols::line::VERTICAL_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_vertical_left, ratatui::symbols::line::DOUBLE_VERTICAL_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_vertical_left, ratatui::symbols::line::THICK_VERTICAL_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_vertical_right, ratatui::symbols::line::VERTICAL_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_vertical_right, ratatui::symbols::line::DOUBLE_VERTICAL_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_vertical_right, ratatui::symbols::line::THICK_VERTICAL_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_horizontal_down, ratatui::symbols::line::HORIZONTAL_DOWN);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_horizontal_down, ratatui::symbols::line::DOUBLE_HORIZONTAL_DOWN);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_horizontal_down, ratatui::symbols::line::THICK_HORIZONTAL_DOWN);
crate::ratatui_const_str_getter!(ratatui_symbols_get_horizontal_up, ratatui::symbols::line::HORIZONTAL_UP);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_horizontal_up, ratatui::symbols::line::DOUBLE_HORIZONTAL_UP);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_horizontal_up, ratatui::symbols::line::THICK_HORIZONTAL_UP);
crate::ratatui_const_str_getter!(ratatui_symbols_get_cross, ratatui::symbols::line::CROSS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_double_cross, ratatui::symbols::line::DOUBLE_CROSS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_thick_cross, ratatui::symbols::line::THICK_CROSS);

// border.rs quadrants and one-eighths
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_left, ratatui::symbols::border::QUADRANT_TOP_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_right, ratatui::symbols::border::QUADRANT_TOP_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_bottom_left, ratatui::symbols::border::QUADRANT_BOTTOM_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_bottom_right, ratatui::symbols::border::QUADRANT_BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_half, ratatui::symbols::border::QUADRANT_TOP_HALF);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_bottom_half, ratatui::symbols::border::QUADRANT_BOTTOM_HALF);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_left_half, ratatui::symbols::border::QUADRANT_LEFT_HALF);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_right_half, ratatui::symbols::border::QUADRANT_RIGHT_HALF);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_left_bottom_left_bottom_right, ratatui::symbols::border::QUADRANT_TOP_LEFT_BOTTOM_LEFT_BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_left_top_right_bottom_left, ratatui::symbols::border::QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_left_top_right_bottom_right, ratatui::symbols::border::QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_right_bottom_left_bottom_right, ratatui::symbols::border::QUADRANT_TOP_RIGHT_BOTTOM_LEFT_BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_left_bottom_right, ratatui::symbols::border::QUADRANT_TOP_LEFT_BOTTOM_RIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_top_right_bottom_left, ratatui::symbols::border::QUADRANT_TOP_RIGHT_BOTTOM_LEFT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_quadrant_block, ratatui::symbols::border::QUADRANT_BLOCK);

crate::ratatui_const_str_getter!(ratatui_symbols_get_one_eighth_top_eight, ratatui::symbols::border::ONE_EIGHTH_TOP_EIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_one_eighth_bottom_eight, ratatui::symbols::border::ONE_EIGHTH_BOTTOM_EIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_one_eighth_left_eight, ratatui::symbols::border::ONE_EIGHTH_LEFT_EIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_one_eighth_right_eight, ratatui::symbols::border::ONE_EIGHTH_RIGHT_EIGHT);

// line.rs Set getters
crate::ratatui_const_struct_getter!(ratatui_symbols_get_line_normal, FfiLineSet, ratatui::symbols::line::NORMAL, [vertical, horizontal, top_right, top_left, bottom_right, bottom_left, vertical_left, vertical_right, horizontal_down, horizontal_up, cross]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_line_rounded, FfiLineSet, ratatui::symbols::line::ROUNDED, [vertical, horizontal, top_right, top_left, bottom_right, bottom_left, vertical_left, vertical_right, horizontal_down, horizontal_up, cross]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_line_double, FfiLineSet, ratatui::symbols::line::DOUBLE, [vertical, horizontal, top_right, top_left, bottom_right, bottom_left, vertical_left, vertical_right, horizontal_down, horizontal_up, cross]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_line_thick, FfiLineSet, ratatui::symbols::line::THICK, [vertical, horizontal, top_right, top_left, bottom_right, bottom_left, vertical_left, vertical_right, horizontal_down, horizontal_up, cross]);

// border.rs Set getters
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_plain, FfiBorderSet, ratatui::symbols::border::PLAIN, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_rounded, FfiBorderSet, ratatui::symbols::border::ROUNDED, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_double, FfiBorderSet, ratatui::symbols::border::DOUBLE, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_thick, FfiBorderSet, ratatui::symbols::border::THICK, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_quadrant_outside, FfiBorderSet, ratatui::symbols::border::QUADRANT_OUTSIDE, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_quadrant_inside, FfiBorderSet, ratatui::symbols::border::QUADRANT_INSIDE, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_one_eighth_wide, FfiBorderSet, ratatui::symbols::border::ONE_EIGHTH_WIDE, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_one_eighth_tall, FfiBorderSet, ratatui::symbols::border::ONE_EIGHTH_TALL, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_proportional_wide, FfiBorderSet, ratatui::symbols::border::PROPORTIONAL_WIDE, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_proportional_tall, FfiBorderSet, ratatui::symbols::border::PROPORTIONAL_TALL, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_border_full, FfiBorderSet, ratatui::symbols::border::FULL, [top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom]);

// symbols.rs base
crate::ratatui_const_str_getter!(ratatui_symbols_get_dot, ratatui::symbols::DOT);
// block scalar levels
crate::ratatui_const_str_getter!(ratatui_symbols_get_block_full, ratatui::symbols::block::FULL);
crate::ratatui_const_str_getter!(ratatui_symbols_get_block_seven_eighths, ratatui::symbols::block::SEVEN_EIGHTHS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_block_three_quarters, ratatui::symbols::block::THREE_QUARTERS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_block_five_eighths, ratatui::symbols::block::FIVE_EIGHTHS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_block_half, ratatui::symbols::block::HALF);
crate::ratatui_const_str_getter!(ratatui_symbols_get_block_three_eighths, ratatui::symbols::block::THREE_EIGHTHS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_block_one_quarter, ratatui::symbols::block::ONE_QUARTER);
crate::ratatui_const_str_getter!(ratatui_symbols_get_block_one_eighth, ratatui::symbols::block::ONE_EIGHTH);
// bar scalar levels
crate::ratatui_const_str_getter!(ratatui_symbols_get_bar_full, ratatui::symbols::bar::FULL);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bar_seven_eighths, ratatui::symbols::bar::SEVEN_EIGHTHS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bar_three_quarters, ratatui::symbols::bar::THREE_QUARTERS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bar_five_eighths, ratatui::symbols::bar::FIVE_EIGHTHS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bar_half, ratatui::symbols::bar::HALF);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bar_three_eighths, ratatui::symbols::bar::THREE_EIGHTHS);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bar_one_quarter, ratatui::symbols::bar::ONE_QUARTER);
crate::ratatui_const_str_getter!(ratatui_symbols_get_bar_one_eighth, ratatui::symbols::bar::ONE_EIGHTH);
// braille scalars
crate::ratatui_const_u16_getter!(ratatui_symbols_get_braille_blank, ratatui::symbols::braille::BLANK);
crate::ratatui_const_str_getter!(ratatui_symbols_get_shade_empty, ratatui::symbols::shade::EMPTY);
crate::ratatui_const_str_getter!(ratatui_symbols_get_shade_light, ratatui::symbols::shade::LIGHT);
crate::ratatui_const_str_getter!(ratatui_symbols_get_shade_medium, ratatui::symbols::shade::MEDIUM);
crate::ratatui_const_str_getter!(ratatui_symbols_get_shade_dark, ratatui::symbols::shade::DARK);
crate::ratatui_const_str_getter!(ratatui_symbols_get_shade_full, ratatui::symbols::shade::FULL);
crate::ratatui_const_char_getter!(ratatui_symbols_get_half_block_upper, ratatui::symbols::half_block::UPPER);
crate::ratatui_const_char_getter!(ratatui_symbols_get_half_block_lower, ratatui::symbols::half_block::LOWER);
crate::ratatui_const_char_getter!(ratatui_symbols_get_half_block_full, ratatui::symbols::half_block::FULL);

// block/bar level sets
crate::ratatui_const_struct_getter!(ratatui_symbols_get_block_three_levels, FfiLevelSet, ratatui::symbols::block::THREE_LEVELS, [full, seven_eighths, three_quarters, five_eighths, half, three_eighths, one_quarter, one_eighth, empty]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_block_nine_levels, FfiLevelSet, ratatui::symbols::block::NINE_LEVELS, [full, seven_eighths, three_quarters, five_eighths, half, three_eighths, one_quarter, one_eighth, empty]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_bar_three_levels, FfiLevelSet, ratatui::symbols::bar::THREE_LEVELS, [full, seven_eighths, three_quarters, five_eighths, half, three_eighths, one_quarter, one_eighth, empty]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_bar_nine_levels, FfiLevelSet, ratatui::symbols::bar::NINE_LEVELS, [full, seven_eighths, three_quarters, five_eighths, half, three_eighths, one_quarter, one_eighth, empty]);

// scrollbar sets
crate::ratatui_const_struct_getter!(ratatui_symbols_get_scrollbar_double_vertical, FfiScrollbarSet, ratatui::symbols::scrollbar::DOUBLE_VERTICAL, [track, thumb, begin, end]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_scrollbar_double_horizontal, FfiScrollbarSet, ratatui::symbols::scrollbar::DOUBLE_HORIZONTAL, [track, thumb, begin, end]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_scrollbar_vertical, FfiScrollbarSet, ratatui::symbols::scrollbar::VERTICAL, [track, thumb, begin, end]);
crate::ratatui_const_struct_getter!(ratatui_symbols_get_scrollbar_horizontal, FfiScrollbarSet, ratatui::symbols::scrollbar::HORIZONTAL, [track, thumb, begin, end]);

// Palette constant getters are emitted by tools/ffi_introspect.rs into ffi/generated.rs

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct FfiStyleMods: u16 {
        const NONE      = 0;
        const BOLD      = 1<<0;
        const ITALIC    = 1<<1;
        const UNDERLINE = 1<<2;
        const DIM       = 1<<3;
        const CROSSED   = 1<<4;
        const REVERSED  = 1<<5;
        const RAPIDBLINK= 1<<6;
        const SLOWBLINK = 1<<7;
        const HIDDEN    = 1<<8;
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiStyle {
    pub fg: u32,
    pub bg: u32,
    pub mods: u16,
}

#[repr(C)]
pub struct FfiSpan {
    pub text_utf8: *const c_char,
    pub style: FfiStyle,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiStr {
    pub ptr: *const u8,
    pub len: usize,
}

// Auto-generated FFI structs for symbol sets (all fields are UTF-8 string slices)
crate::ratatui_define_ffi_str_struct!(FfiLineSet: vertical, horizontal, top_right, top_left, bottom_right, bottom_left, vertical_left, vertical_right, horizontal_down, horizontal_up, cross);
crate::ratatui_define_ffi_str_struct!(FfiBorderSet: top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom);
crate::ratatui_define_ffi_str_struct!(FfiLevelSet: full, seven_eighths, three_quarters, five_eighths, half, three_eighths, one_quarter, one_eighth, empty);
crate::ratatui_define_ffi_str_struct!(FfiScrollbarSet: track, thumb, begin, end);

// Auto-generated FFI structs for color palettes will be included from generated.rs

// (structs generated by macros above)

// Flat slice for braille DOTS (4x2 -> 8 elements)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiU16Slice {
    pub ptr: *const u16,
    pub len: usize,
}

const __RATATUI_BRAILLE_DOTS: [[u16; 2]; 4] = ratatui::symbols::braille::DOTS;
const __RATATUI_BRAILLE_DOTS_FLAT: [u16; 8] = [
    __RATATUI_BRAILLE_DOTS[0][0], __RATATUI_BRAILLE_DOTS[0][1],
    __RATATUI_BRAILLE_DOTS[1][0], __RATATUI_BRAILLE_DOTS[1][1],
    __RATATUI_BRAILLE_DOTS[2][0], __RATATUI_BRAILLE_DOTS[2][1],
    __RATATUI_BRAILLE_DOTS[3][0], __RATATUI_BRAILLE_DOTS[3][1],
];

#[no_mangle]
pub extern "C" fn ratatui_symbols_get_braille_dots_flat() -> FfiU16Slice {
    FfiU16Slice { ptr: __RATATUI_BRAILLE_DOTS_FLAT.as_ptr(), len: __RATATUI_BRAILLE_DOTS_FLAT.len() }
}

// Convenience color helpers for building FfiStyle color values
#[no_mangle]
pub extern "C" fn ratatui_color_rgb(r: u8, g: u8, b: u8) -> u32 {
    0x8000_0000u32 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

#[no_mangle]
pub extern "C" fn ratatui_color_indexed(index: u8) -> u32 {
    0x4000_0000u32 | (index as u32)
}

// Version and capability introspection
#[no_mangle]
pub extern "C" fn ratatui_ffi_version(
    out_major: *mut u32,
    out_minor: *mut u32,
    out_patch: *mut u32,
) -> bool {
    if out_major.is_null() || out_minor.is_null() || out_patch.is_null() {
        return false;
    }
    let ver = env!("CARGO_PKG_VERSION");
    let mut parts = ver.split('.');
    let maj = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let min = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let pat = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    unsafe {
        *out_major = maj;
        *out_minor = min;
        *out_patch = pat;
    }
    true
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct FfiFeatures: u32 {
        const SCROLLBAR        = 1 << 0;
        const CANVAS           = 1 << 1;
        const STYLE_DUMP_EX    = 1 << 2;
        const BATCH_TABLE_ROWS = 1 << 3;
        const BATCH_LIST_ITEMS = 1 << 4;
        const COLOR_HELPERS    = 1 << 5;
        const AXIS_LABELS      = 1 << 6;
        // New span-based setters for labels/dividers/titles
        const SPAN_SETTERS     = 1 << 7;
    }
}

#[no_mangle]
pub extern "C" fn ratatui_ffi_feature_bits() -> u32 {
    let mut bits = FfiFeatures::empty();
    #[cfg(feature = "scrollbar")]
    {
        bits |= FfiFeatures::SCROLLBAR;
    }
    bits |= FfiFeatures::CANVAS;
    bits |= FfiFeatures::STYLE_DUMP_EX;
    bits |= FfiFeatures::BATCH_TABLE_ROWS;
    bits |= FfiFeatures::BATCH_LIST_ITEMS;
    bits |= FfiFeatures::COLOR_HELPERS;
    bits |= FfiFeatures::AXIS_LABELS;
    bits |= FfiFeatures::SPAN_SETTERS;
    // Paragraph and Tabs batching are lightweight; not explicitly flagged.
    bits.bits()
}

fn spans_from_ffi<'a>(spans: *const FfiSpan, len: usize) -> Option<Vec<Span<'static>>> {
    if spans.is_null() {
        return None;
    }
    let slice = unsafe { std::slice::from_raw_parts(spans, len) };
    let mut out: Vec<Span<'static>> = Vec::with_capacity(len);
    for s in slice.iter() {
        if s.text_utf8.is_null() {
            continue;
        }
        let c = unsafe { CStr::from_ptr(s.text_utf8) };
        if let Ok(txt) = c.to_str() {
            out.push(Span::styled(txt.to_string(), style_from_ffi(s.style)));
        }
    }
    Some(out)
}

fn build_block_from_adv(
    borders_bits: u8,
    border_type: u32,
    pad_l: u16,
    pad_t: u16,
    pad_r: u16,
    pad_b: u16,
    title_spans: *const FfiSpan,
    title_len: usize,
) -> Block<'static> {
    let mut block = Block::default().borders(borders_from_bits(borders_bits));
    block = block.padding(RtPadding {
        left: pad_l,
        right: pad_r,
        top: pad_t,
        bottom: pad_b,
    });
    block = match border_type {
        1 => block.border_type(RtBorderType::Thick),
        2 => block.border_type(RtBorderType::Double),
        3 => block.border_type(RtBorderType::Rounded),
        4 => block.border_type(RtBorderType::QuadrantInside),
        5 => block.border_type(RtBorderType::QuadrantOutside),
        _ => block.border_type(RtBorderType::Plain),
    };
    if let Some(sp) = spans_from_ffi(title_spans, title_len) {
        block = block.title(Line::from(sp));
    }
    block
}

fn apply_block_title_alignment(b: Block<'static>, align_code: u32) -> Block<'static> {
    let align = match align_code {
        1 => ratatui::layout::Alignment::Center,
        2 => ratatui::layout::Alignment::Right,
        _ => ratatui::layout::Alignment::Left,
    };
    b.title_alignment(align)
}

// ----- Block title alignment setters (additive; do not break existing APIs) -----

crate::ratatui_block_title_alignment_fn!(ratatui_paragraph_set_block_title_alignment, FfiParagraph);
crate::ratatui_block_title_alignment_fn!(ratatui_list_set_block_title_alignment, FfiList);
crate::ratatui_block_title_alignment_fn!(ratatui_table_set_block_title_alignment, FfiTable);
crate::ratatui_block_title_alignment_fn!(ratatui_gauge_set_block_title_alignment, FfiGauge);
crate::ratatui_block_title_alignment_fn!(ratatui_linegauge_set_block_title_alignment, FfiLineGauge);
crate::ratatui_block_title_alignment_fn!(ratatui_tabs_set_block_title_alignment, FfiTabs);
crate::ratatui_block_title_alignment_fn!(ratatui_barchart_set_block_title_alignment, FfiBarChart);
crate::ratatui_block_title_alignment_fn!(ratatui_chart_set_block_title_alignment, FfiChart);
crate::ratatui_block_title_alignment_fn!(ratatui_sparkline_set_block_title_alignment, FfiSparkline);
crate::ratatui_block_title_alignment_fn!(ratatui_scrollbar_set_block_title_alignment, FfiScrollbar);
crate::ratatui_block_title_alignment_fn!(ratatui_canvas_set_block_title_alignment, FfiCanvas);

#[repr(u32)]
pub enum FfiBorderType {
    Plain = 0,
    Thick = 1,
    Double = 2,
    Rounded = 3,
    QuadrantInside = 4,
    QuadrantOutside = 5,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct FfiBorders: u8 {
        const NONE   = 0;
        const LEFT   = 1<<0;
        const RIGHT  = 1<<1;
        const TOP    = 1<<2;
        const BOTTOM = 1<<3;
    }
}

fn borders_from_bits(bits: u8) -> Borders {
    let fb = FfiBorders::from_bits_truncate(bits);
    let mut b = Borders::NONE;
    if fb.contains(FfiBorders::LEFT) {
        b |= Borders::LEFT;
    }
    if fb.contains(FfiBorders::RIGHT) {
        b |= Borders::RIGHT;
    }
    if fb.contains(FfiBorders::TOP) {
        b |= Borders::TOP;
    }
    if fb.contains(FfiBorders::BOTTOM) {
        b |= Borders::BOTTOM;
    }
    b
}

fn color_from_u32(c: u32) -> Option<Color> {
    if c == 0 {
        return None;
    }
    // High-bit encodings for extended colors
    if (c & 0x8000_0000) != 0 {
        let r = ((c >> 16) & 0xFF) as u8;
        let g = ((c >> 8) & 0xFF) as u8;
        let b = (c & 0xFF) as u8;
        return Some(Color::Rgb(r, g, b));
    }
    if (c & 0x4000_0000) != 0 {
        let idx = (c & 0xFF) as u8;
        return Some(Color::Indexed(idx));
    }
    match c {
        1 => Some(Color::Black),
        2 => Some(Color::Red),
        3 => Some(Color::Green),
        4 => Some(Color::Yellow),
        5 => Some(Color::Blue),
        6 => Some(Color::Magenta),
        7 => Some(Color::Cyan),
        8 => Some(Color::Gray),
        9 => Some(Color::DarkGray),
        10 => Some(Color::LightRed),
        11 => Some(Color::LightGreen),
        12 => Some(Color::LightYellow),
        13 => Some(Color::LightBlue),
        14 => Some(Color::LightMagenta),
        15 => Some(Color::LightCyan),
        16 => Some(Color::White),
        _ => None,
    }
}

fn color_to_u32(c: Color) -> u32 {
    match c {
        Color::Reset => 0,
        Color::Black => 1,
        Color::Red => 2,
        Color::Green => 3,
        Color::Yellow => 4,
        Color::Blue => 5,
        Color::Magenta => 6,
        Color::Cyan => 7,
        Color::Gray => 8,
        Color::DarkGray => 9,
        Color::LightRed => 10,
        Color::LightGreen => 11,
        Color::LightYellow => 12,
        Color::LightBlue => 13,
        Color::LightMagenta => 14,
        Color::LightCyan => 15,
        Color::White => 16,
        Color::Indexed(i) => 0x4000_0000 | (i as u32),
        Color::Rgb(r, g, b) => 0x8000_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32),
    }
}

fn style_from_ffi(s: FfiStyle) -> Style {
    let mut st = Style::default();
    if let Some(fg) = color_from_u32(s.fg) {
        st = st.fg(fg);
    }
    if let Some(bg) = color_from_u32(s.bg) {
        st = st.bg(bg);
    }
    let mods = FfiStyleMods::from_bits_truncate(s.mods);
    if mods.contains(FfiStyleMods::BOLD) {
        st = st.add_modifier(Modifier::BOLD);
    }
    if mods.contains(FfiStyleMods::ITALIC) {
        st = st.add_modifier(Modifier::ITALIC);
    }
    if mods.contains(FfiStyleMods::UNDERLINE) {
        st = st.add_modifier(Modifier::UNDERLINED);
    }
    if mods.contains(FfiStyleMods::DIM) {
        st = st.add_modifier(Modifier::DIM);
    }
    if mods.contains(FfiStyleMods::CROSSED) {
        st = st.add_modifier(Modifier::CROSSED_OUT);
    }
    if mods.contains(FfiStyleMods::REVERSED) {
        st = st.add_modifier(Modifier::REVERSED);
    }
    if mods.contains(FfiStyleMods::RAPIDBLINK) {
        st = st.add_modifier(Modifier::RAPID_BLINK);
    }
    if mods.contains(FfiStyleMods::SLOWBLINK) {
        st = st.add_modifier(Modifier::SLOW_BLINK);
    }
    if mods.contains(FfiStyleMods::HIDDEN) {
        st = st.add_modifier(Modifier::HIDDEN);
    }
    st
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_line(
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
        p.lines.push(Line::from(Span::styled(s.to_string(), st)));
    }
}

// ----- LineGauge -----

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
pub extern "C" fn ratatui_linegauge_set_label(g: *mut FfiLineGauge, label_utf8: *const c_char) {
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

crate::ratatui_block_title_fn!(ratatui_linegauge_set_block_title, FfiLineGauge);
crate::ratatui_block_title_spans_fn!(ratatui_linegauge_set_block_title_spans, FfiLineGauge);

crate::ratatui_set_style_fn!(ratatui_linegauge_set_style, FfiLineGauge, style);

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
    out_text_utf8: *mut *mut c_char,
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

// ----- Clear -----

#[no_mangle]
pub extern "C" fn ratatui_clear_in(term: *mut FfiTerminal, rect: FfiRect) -> bool {
    guard_bool("ratatui_clear_in", || {
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
            frame.render_widget(RtClear, area);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_clear(
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
    ratatui::widgets::Widget::render(RtClear, area, &mut buf);
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

// ----- RatatuiLogo -----

#[no_mangle]
pub extern "C" fn ratatui_ratatuilogo_draw_in(term: *mut FfiTerminal, rect: FfiRect) -> bool {
    guard_bool("ratatui_ratatuilogo_draw_in", || {
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
    guard_bool("ratatui_ratatuilogo_draw_sized_in", || {
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

// ----- Headless rendering helpers (for smoke tests) -----

#[no_mangle]
pub extern "C" fn ratatui_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

// moved to crate::ffi::widgets::paragraph

// ----- Headless List/Table and Composite Frame -----

// moved to crate::ffi::widgets::list

// moved to crate::ffi::widgets::table

#[no_mangle]
pub extern "C" fn ratatui_headless_render_canvas(
    width: u16,
    height: u16,
    c: *const FfiCanvas,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if c.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let cv = unsafe { &*c };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let mut w = RtCanvas::default()
        .x_bounds([cv.x_min, cv.x_max])
        .y_bounds([cv.y_min, cv.y_max]);
    if let Some(bg) = cv.background {
        w = w.background_color(bg);
    }
    if let Some(b) = &cv.block {
        w = w.block(b.clone());
    }
    if let Some(mk) = cv.marker {
        w = w.marker(mk);
    }
    w = w.paint(|p| {
        for l in &cv.lines {
            let col = color_from_u32(l.style.fg).unwrap_or(Color::White);
            p.draw(&RtCanvasLine {
                x1: l.x1,
                y1: l.y1,
                x2: l.x2,
                y2: l.y2,
                color: col,
            });
        }
        for r in &cv.rects {
            let col = color_from_u32(r.style.fg).unwrap_or(Color::White);
            p.draw(&RtCanvasRect {
                x: r.x,
                y: r.y,
                width: r.w,
                height: r.h,
                color: col,
            });
        }
        for (pts, col) in &cv.pts {
            p.draw(&RtCanvasPoints {
                coords: &pts[..],
                color: *col,
            });
        }
    });
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

#[repr(u32)]
pub enum FfiWidgetKind {
    Paragraph = 1,
    List = 2,
    Table = 3,
    Gauge = 4,
    Tabs = 5,
    BarChart = 6,
    Sparkline = 7,
    Chart = 8,
    #[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
    #[cfg(feature = "scrollbar")]
    Scrollbar = 9,
    LineGauge = 10,
    Clear = 11,
    RatatuiLogo = 12,
    Canvas = 13,
}
// extend kinds for new widgets

#[repr(C)]
pub struct FfiDrawCmd {
    pub kind: u32,
    pub handle: *const (),
    pub rect: FfiRect,
}

fn render_cmd_to_buffer(cmd: &FfiDrawCmd, buf: &mut Buffer) {
    let area = Rect {
        x: cmd.rect.x,
        y: cmd.rect.y,
        width: cmd.rect.width,
        height: cmd.rect.height,
    };
    match cmd.kind {
        x if x == FfiWidgetKind::Paragraph as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let p = unsafe { &*(cmd.handle as *const FfiParagraph) };
            let mut w = Paragraph::new(p.lines.clone());
            if let Some(b) = &p.block {
                w = w.block(b.clone());
            }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::List as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let l = unsafe { &*(cmd.handle as *const FfiList) };
            let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
            let mut w = List::new(items);
            if let Some(b) = &l.block {
                w = w.block(b.clone());
            }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Table as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let tb = unsafe { &*(cmd.handle as *const FfiTable) };
            let header_row = if tb.headers.is_empty() {
                None
            } else {
                Some(Row::new(
                    tb.headers
                        .iter()
                        .cloned()
                        .map(Cell::from)
                        .collect::<Vec<_>>(),
                ))
            };
            let rows: Vec<Row> = tb
                .rows
                .iter()
                .map(|r| Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>()))
                .collect();
            let col_count = if !tb.rows.is_empty() {
                tb.rows.iter().map(|r| r.len()).max().unwrap_or(1)
            } else {
                tb.headers.len().max(1)
            };
            let widths = std::iter::repeat(Constraint::Percentage((100 / col_count.max(1)) as u16))
                .take(col_count.max(1));
            let mut w = Table::new(rows, widths);
            if let Some(hr) = header_row {
                w = w.header(hr);
            }
            if let Some(b) = &tb.block {
                w = w.block(b.clone());
            }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Gauge as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let g = unsafe { &*(cmd.handle as *const FfiGauge) };
            let mut w = Gauge::default().ratio(g.ratio as f64);
            if let Some(label) = &g.label {
                w = w.label(label.clone());
            }
            if let Some(b) = &g.block {
                w = w.block(b.clone());
            }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Tabs as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let t = unsafe { &*(cmd.handle as *const FfiTabs) };
            let titles: Vec<Line> = t
                .titles
                .iter()
                .cloned()
                .map(|s| Line::from(Span::raw(s)))
                .collect();
            let mut w = Tabs::new(titles).select(t.selected as usize);
            if let Some(b) = &t.block {
                w = w.block(b.clone());
            }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::BarChart as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let bc = unsafe { &*(cmd.handle as *const FfiBarChart) };
            let area = area; // reuse
            let data: Vec<(&str, u64)> = bc
                .labels
                .iter()
                .map(|s| s.as_str())
                .zip(bc.values.iter().cloned())
                .collect();
            let mut w = RtBarChart::default().data(&data);
            if let Some(wd) = bc.bar_width {
                w = w.bar_width(wd);
            }
            if let Some(gp) = bc.bar_gap {
                w = w.bar_gap(gp);
            }
            if let Some(st) = &bc.bar_style {
                w = w.bar_style(st.clone());
            }
            if let Some(st) = &bc.value_style {
                w = w.value_style(st.clone());
            }
            if let Some(st) = &bc.label_style {
                w = w.label_style(st.clone());
            }
            if let Some(b) = &bc.block {
                w = w.block(b.clone());
            }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Sparkline as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let sp = unsafe { &*(cmd.handle as *const FfiSparkline) };
            let mut w = RtSparkline::default().data(&sp.values);
            if let Some(b) = &sp.block {
                w = w.block(b.clone());
            }
            ratatui::widgets::Widget::render(w, area, buf);
        }

        #[cfg(feature = "scrollbar")]
        x if x == FfiWidgetKind::Scrollbar as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let sc = unsafe { &*(cmd.handle as *const FfiScrollbar) };
            let mut state =
                RtScrollbarState::new(sc.content_len as usize).position(sc.position as usize);
            let orient = if let Some(side) = sc.side {
                match side {
                    0 => RtScrollbarOrientation::VerticalLeft,
                    1 => RtScrollbarOrientation::VerticalRight,
                    2 => RtScrollbarOrientation::HorizontalTop,
                    3 => RtScrollbarOrientation::HorizontalBottom,
                    _ => RtScrollbarOrientation::VerticalRight,
                }
            } else if sc.orient == FfiScrollbarOrient::Horizontal as u32 {
                RtScrollbarOrientation::HorizontalTop
            } else {
                RtScrollbarOrientation::VerticalRight
            };
            let w = RtScrollbar::new(orient);
            ratatui::widgets::StatefulWidget::render(w, area, buf, &mut state);
        }
        x if x == FfiWidgetKind::Chart as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let ch = unsafe { &*(cmd.handle as *const FfiChart) };
            let area = area;
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
            let mut chart = RtChart::new(datasets);
            let x_axis = {
                let mut ax = RtAxis::default();
                if let Some(t) = &ch.x_title {
                    ax = ax.title(t.clone());
                }
                if let (Some(min), Some(max)) = (ch.x_min, ch.x_max) {
                    ax = ax.bounds([min, max]);
                }
                if let Some(st) = &ch.x_axis_style {
                    ax = ax.style(st.clone());
                }
                if let Some(lbls) = &ch.x_labels {
                    ax = ax.labels(lbls.clone());
                }
                if let Some(al) = ch.x_labels_align {
                    ax = ax.labels_alignment(al);
                }
                ax
            };
            let y_axis = {
                let mut ay = RtAxis::default();
                if let Some(t) = &ch.y_title {
                    ay = ay.title(t.clone());
                }
                if let (Some(min), Some(max)) = (ch.y_min, ch.y_max) {
                    ay = ay.bounds([min, max]);
                }
                if let Some(st) = &ch.y_axis_style {
                    ay = ay.style(st.clone());
                }
                if let Some(lbls) = &ch.y_labels {
                    ay = ay.labels(lbls.clone());
                }
                if let Some(al) = ch.y_labels_align {
                    ay = ay.labels_alignment(al);
                }
                ay
            };
            chart = chart.x_axis(x_axis).y_axis(y_axis);
            if let Some(st) = &ch.chart_style {
                chart = chart.style(st.clone());
            }
            if let Some(b) = &ch.block {
                chart = chart.block(b.clone());
            }
            ratatui::widgets::Widget::render(chart, area, buf);
        }
        x if x == FfiWidgetKind::LineGauge as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let lg = unsafe { &*(cmd.handle as *const FfiLineGauge) };
            let mut w = RtLineGauge::default().ratio(lg.ratio as f64);
            if let Some(label) = &lg.label {
                w = w.label(label.clone());
            }
            if let Some(b) = &lg.block {
                w = w.block(b.clone());
            }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Clear as u32 => {
            let w = RtClear;
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::RatatuiLogo as u32 => {
            let w = RtRatatuiLogo::default();
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Canvas as u32 => {
            if cmd.handle.is_null() {
                return;
            }
            let cv = unsafe { &*(cmd.handle as *const FfiCanvas) };
            let mut w = RtCanvas::default()
                .x_bounds([cv.x_min, cv.x_max])
                .y_bounds([cv.y_min, cv.y_max]);
            if let Some(bg) = cv.background {
                w = w.background_color(bg);
            }
            if let Some(b) = &cv.block {
                w = w.block(b.clone());
            }
            w = w.paint(|p| {
                for l in &cv.lines {
                    let col = color_from_u32(l.style.fg).unwrap_or(Color::White);
                    p.draw(&RtCanvasLine {
                        x1: l.x1,
                        y1: l.y1,
                        x2: l.x2,
                        y2: l.y2,
                        color: col,
                    });
                }
                for r in &cv.rects {
                    let col = color_from_u32(r.style.fg).unwrap_or(Color::White);
                    p.draw(&RtCanvasRect {
                        x: r.x,
                        y: r.y,
                        width: r.w,
                        height: r.h,
                        color: col,
                    });
                }
                for (pts, col) in &cv.pts {
                    p.draw(&RtCanvasPoints {
                        coords: &pts[..],
                        color: *col,
                    });
                }
            });
            ratatui::widgets::Widget::render(w, area, buf);
        }
        _ => {}
    }
}

// moved to crate::ffi::headless_frame

// Style dump for headless frames: returns a text grid of hex triplets per cell: fg as 2-hex (00 if none or encoded high bits), bg as 2-hex, mods as 4-hex.
// moved to crate::ffi::headless_frame

// Extended style dump for headless frames.
// Emits per-cell: 8-hex fg color (FfiStyle encoding), 8-hex bg color, 4-hex mods.
// Cells are separated by spaces, rows by newlines. Suitable for precise snapshot tests.
// moved to crate::ffi::headless_frame

// Structured cell dump for headless frames: fills caller-provided buffer with per-cell data.
// Returns number of cells written (min(width*height, cap)). Layout is row-major.
// moved to crate::ffi::headless_frame

// ----- Batched terminal frame drawing -----

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_frame(
    term: *mut FfiTerminal,
    cmds: *const FfiDrawCmd,
    len: usize,
) -> bool {
    guard_bool("ratatui_terminal_draw_frame", || {
        if term.is_null() || cmds.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let Some(slice) = slice_checked(cmds, len, "terminal_draw_frame(slice)") else {
            return false;
        };
        let res = t.terminal.draw(|frame| {
            let full = frame.area();
            for cmd in slice.iter() {
                // Clamp to frame area to avoid invalid regions
                let x = cmd.rect.x.min(full.width.saturating_sub(1));
                let y = cmd.rect.y.min(full.height.saturating_sub(1));
                let max_w = full.width.saturating_sub(x);
                let max_h = full.height.saturating_sub(y);
                let w = cmd.rect.width.min(max_w);
                let h = cmd.rect.height.min(max_h);
                if w == 0 || h == 0 {
                    continue;
                }
                let area = Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                };
                match cmd.kind {
                    x if x == FfiWidgetKind::Paragraph as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(p) =
                            ptr_checked(cmd.handle as *const FfiParagraph, "draw_frame:Paragraph")
                        else {
                            continue;
                        };
                        let mut w = Paragraph::new(p.lines.clone());
                        if let Some(b) = &p.block {
                            w = w.block(b.clone());
                        }
                        frame.render_widget(w, area);
                    }
                    x if x == FfiWidgetKind::List as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(l) = ptr_checked(cmd.handle as *const FfiList, "draw_frame:List")
                        else {
                            continue;
                        };
                        let items: Vec<ListItem> =
                            l.items.iter().cloned().map(ListItem::new).collect();
                        let mut w = List::new(items);
                        if let Some(d) = l.direction {
                            w = w.direction(d);
                        }
                        if let Some(b) = &l.block {
                            w = w.block(b.clone());
                        }
                        if let Some(sp) = &l.highlight_spacing {
                            w = w.highlight_spacing(sp.clone());
                        }
                        if l.selected.is_some() || l.scroll_offset.is_some() {
                            let mut state = ratatui::widgets::ListState::default();
                            if let Some(sel) = l.selected {
                                state.select(Some(sel));
                            }
                            if let Some(off) = l.scroll_offset {
                                state = state.with_offset(off);
                            }
                            frame.render_stateful_widget(w, area, &mut state);
                        } else {
                            frame.render_widget(w, area);
                        }
                    }
                    x if x == FfiWidgetKind::Table as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(tb) =
                            ptr_checked(cmd.handle as *const FfiTable, "draw_frame:Table")
                        else {
                            continue;
                        };
                        let header_row = if let Some(hs) = &tb.headers_spans {
                            let mut r =
                                Row::new(hs.iter().cloned().map(Cell::from).collect::<Vec<_>>());
                            if let Some(hsty) = &tb.header_style {
                                r = r.style(hsty.clone());
                            }
                            Some(r)
                        } else if tb.headers.is_empty() {
                            None
                        } else {
                            Some(Row::new(
                                tb.headers
                                    .iter()
                                    .cloned()
                                    .map(Cell::from)
                                    .collect::<Vec<_>>(),
                            ))
                        };
                        let rows: Vec<Row> = if let Some(rows_cells) = &tb.rows_cells_lines {
                            rows_cells
                                .iter()
                                .map(|cells| {
                                    let mut rc: Vec<Cell> = Vec::with_capacity(cells.len());
                                    for cell_lines in cells.iter() {
                                        let text = ratatui::text::Text::from(cell_lines.clone());
                                        rc.push(Cell::from(text));
                                    }
                                    let mut row = Row::new(rc);
                                    if let Some(h) = tb.row_height {
                                        row = row.height(h);
                                    }
                                    row
                                })
                                .collect()
                        } else if let Some(rss) = &tb.rows_spans {
                            rss.iter()
                                .map(|r| {
                                    let mut row = Row::new(
                                        r.iter().cloned().map(Cell::from).collect::<Vec<_>>(),
                                    );
                                    if let Some(h) = tb.row_height {
                                        row = row.height(h);
                                    }
                                    row
                                })
                                .collect()
                        } else {
                            tb.rows
                                .iter()
                                .map(|r| {
                                    let mut row = Row::new(
                                        r.iter().cloned().map(Cell::from).collect::<Vec<_>>(),
                                    );
                                    if let Some(h) = tb.row_height {
                                        row = row.height(h);
                                    }
                                    row
                                })
                                .collect()
                        };
                        let col_count = if let Some(w) = &tb.widths_pct {
                            w.len().max(1)
                        } else if !tb.rows.is_empty() {
                            tb.rows.iter().map(|r| r.len()).max().unwrap_or(1)
                        } else {
                            tb.headers.len().max(1)
                        };
                        let widths: Vec<Constraint> = if let Some(ws) = &tb.widths_pct {
                            ws.iter().map(|p| Constraint::Percentage(*p)).collect()
                        } else {
                            std::iter::repeat(Constraint::Percentage(
                                (100 / col_count.max(1)) as u16,
                            ))
                            .take(col_count.max(1))
                            .collect()
                        };
                        let mut w = Table::new(rows, widths);
                        if let Some(cs) = tb.column_spacing {
                            w = w.column_spacing(cs);
                        }
                        if let Some(hr) = header_row {
                            w = w.header(hr);
                        }
                        if let Some(b) = &tb.block {
                            w = w.block(b.clone());
                        }
                        if let Some(sty) = &tb.column_highlight_style {
                            w = w.column_highlight_style(sty.clone());
                        }
                        if let Some(sty) = &tb.cell_highlight_style {
                            w = w.cell_highlight_style(sty.clone());
                        }
                        if let Some(sp) = &tb.highlight_spacing {
                            w = w.highlight_spacing(sp.clone());
                        }
                        frame.render_widget(w, area);
                    }
                    x if x == FfiWidgetKind::Gauge as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(g) =
                            ptr_checked(cmd.handle as *const FfiGauge, "draw_frame:Gauge")
                        else {
                            continue;
                        };
                        let mut w = Gauge::default().ratio(g.ratio as f64);
                        if let Some(label) = &g.label {
                            w = w.label(label.clone());
                        }
                        if let Some(b) = &g.block {
                            w = w.block(b.clone());
                        }
                        frame.render_widget(w, area);
                    }
                    x if x == FfiWidgetKind::Tabs as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(tbs) =
                            ptr_checked(cmd.handle as *const FfiTabs, "draw_frame:Tabs")
                        else {
                            continue;
                        };
                        let titles: Vec<Line> = tbs
                            .titles
                            .iter()
                            .cloned()
                            .map(|s| Line::from(Span::raw(s)))
                            .collect();
                        let mut w = Tabs::new(titles).select(tbs.selected as usize);
                        if let Some(b) = &tbs.block {
                            w = w.block(b.clone());
                        }
                        frame.render_widget(w, area);
                    }
                    x if x == FfiWidgetKind::BarChart as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(bc) =
                            ptr_checked(cmd.handle as *const FfiBarChart, "draw_frame:BarChart")
                        else {
                            continue;
                        };
                        let data: Vec<(&str, u64)> = bc
                            .labels
                            .iter()
                            .map(|s| s.as_str())
                            .zip(bc.values.iter().cloned())
                            .collect();
                        let mut w = RtBarChart::default().data(&data);
                        if let Some(wd) = bc.bar_width {
                            w = w.bar_width(wd);
                        }
                        if let Some(gp) = bc.bar_gap {
                            w = w.bar_gap(gp);
                        }
                        if let Some(st) = &bc.bar_style {
                            w = w.bar_style(st.clone());
                        }
                        if let Some(st) = &bc.value_style {
                            w = w.value_style(st.clone());
                        }
                        if let Some(st) = &bc.label_style {
                            w = w.label_style(st.clone());
                        }
                        if let Some(b) = &bc.block {
                            w = w.block(b.clone());
                        }
                        frame.render_widget(w, area);
                    }
                    x if x == FfiWidgetKind::Canvas as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(cv) =
                            ptr_checked(cmd.handle as *const FfiCanvas, "draw_frame:Canvas")
                        else {
                            continue;
                        };
                        let mut w = RtCanvas::default()
                            .x_bounds([cv.x_min, cv.x_max])
                            .y_bounds([cv.y_min, cv.y_max]);
                        if let Some(bg) = cv.background {
                            w = w.background_color(bg);
                        }
                        if let Some(b) = &cv.block {
                            w = w.block(b.clone());
                        }
                        if let Some(mk) = cv.marker {
                            w = w.marker(mk);
                        }
                        w = w.paint(|p| {
                            for l in &cv.lines {
                                let col = color_from_u32(l.style.fg).unwrap_or(Color::White);
                                p.draw(&RtCanvasLine {
                                    x1: l.x1,
                                    y1: l.y1,
                                    x2: l.x2,
                                    y2: l.y2,
                                    color: col,
                                });
                            }
                            for r in &cv.rects {
                                let col = color_from_u32(r.style.fg).unwrap_or(Color::White);
                                p.draw(&RtCanvasRect {
                                    x: r.x,
                                    y: r.y,
                                    width: r.w,
                                    height: r.h,
                                    color: col,
                                });
                            }
                            for (pts, col) in &cv.pts {
                                p.draw(&RtCanvasPoints {
                                    coords: &pts[..],
                                    color: *col,
                                });
                            }
                        });
                        frame.render_widget(w, area);
                    }
                    x if x == FfiWidgetKind::Sparkline as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(sp) =
                            ptr_checked(cmd.handle as *const FfiSparkline, "draw_frame:Sparkline")
                        else {
                            continue;
                        };
                        let mut w = RtSparkline::default().data(&sp.values);
                        if let Some(m) = sp.max {
                            w = w.max(m);
                        }
                        if let Some(st) = &sp.style {
                            w = w.style(st.clone());
                        }
                        if let Some(b) = &sp.block {
                            w = w.block(b.clone());
                        }
                        frame.render_widget(w, area);
                    }
                    x if x == FfiWidgetKind::Chart as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(ch) =
                            ptr_checked(cmd.handle as *const FfiChart, "draw_frame:Chart")
                        else {
                            continue;
                        };
                        let mut datasets: Vec<RtDataset> = Vec::new();
                        for ds in &ch.datasets {
                            let mut d = RtDataset::default().name(ds.name.clone()).data(&ds.points);
                            if let Some(sty) = &ds.style {
                                d = d.style(sty.clone());
                            }
                            d = d.graph_type(RtGraphType::Line);
                            datasets.push(d);
                        }
                        let mut chart = RtChart::new(datasets);
                        let x_axis = {
                            let mut ax = RtAxis::default();
                            if let Some(t) = &ch.x_title {
                                ax = ax.title(t.clone());
                            }
                            ax
                        };
                        let y_axis = {
                            let mut ay = RtAxis::default();
                            if let Some(t) = &ch.y_title {
                                ay = ay.title(t.clone());
                            }
                            ay
                        };
                        chart = chart.x_axis(x_axis).y_axis(y_axis);
                        if let Some(b) = &ch.block {
                            chart = chart.block(b.clone());
                        }
                        frame.render_widget(chart, area);
                    }
                    x if x == FfiWidgetKind::LineGauge as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let Some(lg) =
                            ptr_checked(cmd.handle as *const FfiLineGauge, "draw_frame:LineGauge")
                        else {
                            continue;
                        };
                        let mut w = RtLineGauge::default().ratio(lg.ratio as f64);
                        if let Some(label) = &lg.label {
                            w = w.label(label.clone());
                        }
                        if let Some(b) = &lg.block {
                            w = w.block(b.clone());
                        }
                        frame.render_widget(w, area);
                    }
                    x if x == FfiWidgetKind::Clear as u32 => {
                        frame.render_widget(RtClear, area);
                    }
                    x if x == FfiWidgetKind::RatatuiLogo as u32 => {
                        frame.render_widget(RtRatatuiLogo::default(), area);
                    }
                    #[cfg(feature = "scrollbar")]
                    x if x == FfiWidgetKind::Scrollbar as u32 => {
                        if cmd.handle.is_null() {
                            continue;
                        }
                        let sc = unsafe { &*(cmd.handle as *const FfiScrollbar) };
                        let mut state = RtScrollbarState::new(sc.content_len as usize)
                            .position(sc.position as usize);
                        let orient = if sc.orient == FfiScrollbarOrient::Horizontal as u32 {
                            RtScrollbarOrientation::HorizontalTop
                        } else {
                            RtScrollbarOrientation::VerticalRight
                        };
                        let w = RtScrollbar::new(orient);
                        frame.render_stateful_widget(w, area, &mut state);
                    }
                    _ => {}
                }
            }
        });
        res.is_ok()
    })
}

// ----- Event injection (for automation) -----

static INJECTED_EVENTS: Mutex<VecDeque<CtEvent>> = Mutex::new(VecDeque::new());

#[no_mangle]
pub extern "C" fn ratatui_inject_key(code: u32, ch: u32, mods: u8) {
    let ke = CtKeyEvent::new(
        match code {
            x if x == FfiKeyCode::Char as u32 => {
                CtKeyCode::Char(char::from_u32(ch).unwrap_or('\0'))
            }
            x if x == FfiKeyCode::Enter as u32 => CtKeyCode::Enter,
            x if x == FfiKeyCode::Left as u32 => CtKeyCode::Left,
            x if x == FfiKeyCode::Right as u32 => CtKeyCode::Right,
            x if x == FfiKeyCode::Up as u32 => CtKeyCode::Up,
            x if x == FfiKeyCode::Down as u32 => CtKeyCode::Down,
            x if x == FfiKeyCode::Esc as u32 => CtKeyCode::Esc,
            x if x == FfiKeyCode::Backspace as u32 => CtKeyCode::Backspace,
            x if x == FfiKeyCode::Tab as u32 => CtKeyCode::Tab,
            x if x == FfiKeyCode::Delete as u32 => CtKeyCode::Delete,
            x if x == FfiKeyCode::Home as u32 => CtKeyCode::Home,
            x if x == FfiKeyCode::End as u32 => CtKeyCode::End,
            x if x == FfiKeyCode::PageUp as u32 => CtKeyCode::PageUp,
            x if x == FfiKeyCode::PageDown as u32 => CtKeyCode::PageDown,
            x if x == FfiKeyCode::Insert as u32 => CtKeyCode::Insert,
            _ => CtKeyCode::Null,
        },
        CtKeyModifiers::from_bits_truncate(
            (if (mods & FfiKeyMods::SHIFT.bits()) != 0 {
                CtKeyModifiers::SHIFT.bits()
            } else {
                0
            }) | (if (mods & FfiKeyMods::ALT.bits()) != 0 {
                CtKeyModifiers::ALT.bits()
            } else {
                0
            }) | (if (mods & FfiKeyMods::CTRL.bits()) != 0 {
                CtKeyModifiers::CONTROL.bits()
            } else {
                0
            }),
        ),
    );
    INJECTED_EVENTS.lock().unwrap().push_back(CtEvent::Key(ke));
}

#[no_mangle]
pub extern "C" fn ratatui_inject_resize(width: u16, height: u16) {
    INJECTED_EVENTS
        .lock()
        .unwrap()
        .push_back(CtEvent::Resize(width, height));
}

#[no_mangle]
pub extern "C" fn ratatui_inject_mouse(kind: u32, btn: u32, x: u16, y: u16, mods: u8) {
    let kind = match kind {
        x if x == FfiMouseKind::Down as u32 => CtMouseKind::Down(match btn {
            1 => CtMouseButton::Left,
            2 => CtMouseButton::Right,
            3 => CtMouseButton::Middle,
            _ => CtMouseButton::Left,
        }),
        x if x == FfiMouseKind::Up as u32 => CtMouseKind::Up(match btn {
            1 => CtMouseButton::Left,
            2 => CtMouseButton::Right,
            3 => CtMouseButton::Middle,
            _ => CtMouseButton::Left,
        }),
        x if x == FfiMouseKind::Drag as u32 => CtMouseKind::Drag(match btn {
            1 => CtMouseButton::Left,
            2 => CtMouseButton::Right,
            3 => CtMouseButton::Middle,
            _ => CtMouseButton::Left,
        }),
        x if x == FfiMouseKind::Moved as u32 => CtMouseKind::Moved,
        x if x == FfiMouseKind::ScrollUp as u32 => CtMouseKind::ScrollUp,
        x if x == FfiMouseKind::ScrollDown as u32 => CtMouseKind::ScrollDown,
        _ => CtMouseKind::Moved,
    };
    let modifiers = CtKeyModifiers::from_bits_truncate(
        (if (mods & FfiKeyMods::SHIFT.bits()) != 0 {
            CtKeyModifiers::SHIFT.bits()
        } else {
            0
        }) | (if (mods & FfiKeyMods::ALT.bits()) != 0 {
            CtKeyModifiers::ALT.bits()
        } else {
            0
        }) | (if (mods & FfiKeyMods::CTRL.bits()) != 0 {
            CtKeyModifiers::CONTROL.bits()
        } else {
            0
        }),
    );
    INJECTED_EVENTS
        .lock()
        .unwrap()
        .push_back(CtEvent::Mouse(CtMouseEvent {
            kind,
            column: x,
            row: y,
            modifiers,
        }));
}

// ----- Simple List -----

#[no_mangle]
pub extern "C" fn ratatui_list_new() -> *mut FfiList {
    Box::into_raw(Box::new(FfiList {
        items: Vec::new(),
        block: None,
        selected: None,
        highlight_style: None,
        highlight_symbol: None,
        direction: None,
        scroll_offset: None,
        highlight_spacing: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_list_free(lst: *mut FfiList) {
    if lst.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(lst));
    }
}

// ListState FFI
#[no_mangle]
pub extern "C" fn ratatui_list_state_new() -> *mut FfiListState {
    Box::into_raw(Box::new(FfiListState {
        selected: None,
        offset: 0,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_list_state_free(st: *mut FfiListState) {
    if st.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(st));
    }
}

crate::ratatui_set_selected_i32_fn!(ratatui_list_state_set_selected, FfiListState, selected);

#[no_mangle]
pub extern "C" fn ratatui_list_state_set_offset(st: *mut FfiListState, offset: usize) {
    if st.is_null() {
        return;
    }
    unsafe {
        (&mut *st).offset = offset;
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_list_state_in(
    term: *mut FfiTerminal,
    lst: *const FfiList,
    rect: FfiRect,
    st: *const FfiListState,
) -> bool {
    guard_bool("ratatui_terminal_draw_list_state_in", || {
        if term.is_null() || lst.is_null() || st.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let l = unsafe { &*lst };
        let s = unsafe { &*st };
        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
        let mut widget = List::new(items);
        if let Some(d) = l.direction {
            widget = widget.direction(d);
        }
        if let Some(b) = &l.block {
            widget = widget.block(b.clone());
        }
        if let Some(sty) = &l.highlight_style {
            widget = widget.highlight_style(sty.clone());
        }
        if let Some(sym) = &l.highlight_symbol {
            widget = widget.highlight_symbol(sym.as_str());
        }
        if let Some(sp) = &l.highlight_spacing {
            widget = widget.highlight_spacing(sp.clone());
        }
        let mut state = ratatui::widgets::ListState::default();
        if let Some(sel) = s.selected {
            state.select(Some(sel));
        }
        state = state.with_offset(s.offset);
        let res = t.terminal.draw(|frame| {
            frame.render_stateful_widget(widget.clone(), area, &mut state);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_list_state(
    width: u16,
    height: u16,
    lst: *const FfiList,
    st: *const FfiListState,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if lst.is_null() || st.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let l = unsafe { &*lst };
    let s = unsafe { &*st };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
    let mut widget = List::new(items);
    if let Some(d) = l.direction {
        widget = widget.direction(d);
    }
    if let Some(b) = &l.block {
        widget = widget.block(b.clone());
    }
    if let Some(sty) = &l.highlight_style {
        widget = widget.highlight_style(sty.clone());
    }
    if let Some(sym) = &l.highlight_symbol {
        widget = widget.highlight_symbol(sym.as_str());
    }
    if let Some(sp) = &l.highlight_spacing {
        widget = widget.highlight_spacing(sp.clone());
    }
    let mut state = ratatui::widgets::ListState::default();
    if let Some(sel) = s.selected {
        state.select(Some(sel));
    }
    state = state.with_offset(s.offset);
    ratatui::widgets::StatefulWidget::render(widget, area, &mut buf, &mut state);
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
pub extern "C" fn ratatui_list_append_item(
    lst: *mut FfiList,
    text_utf8: *const c_char,
    style: FfiStyle,
) {
    if lst.is_null() || text_utf8.is_null() {
        return;
    }
    let l = unsafe { &mut *lst };
    let c_str = unsafe { CStr::from_ptr(text_utf8) };
    if let Ok(s) = c_str.to_str() {
        let st = style_from_ffi(style);
        l.items.push(Line::from(Span::styled(s.to_string(), st)));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_list_append_item_spans(
    lst: *mut FfiList,
    spans: *const FfiSpan,
    len: usize,
) {
    if lst.is_null() || spans.is_null() {
        return;
    }
    let l = unsafe { &mut *lst };
    if let Some(sp) = spans_from_ffi(spans, len) {
        l.items.push(Line::from(sp));
    }
}

// Batch append list items from Lines (each specified as spans)
#[no_mangle]
pub extern "C" fn ratatui_list_append_items_spans(
    lst: *mut FfiList,
    items: *const FfiLineSpans,
    len: usize,
) {
    if lst.is_null() || items.is_null() || len == 0 {
        return;
    }
    let l = unsafe { &mut *lst };
    let slice = unsafe { std::slice::from_raw_parts(items, len) };
    for it in slice.iter() {
        if it.spans.is_null() || it.len == 0 {
            l.items.push(Line::default());
            continue;
        }
        if let Some(sp) = spans_from_ffi(it.spans, it.len) {
            l.items.push(Line::from(sp));
        } else {
            l.items.push(Line::default());
        }
    }
}

crate::ratatui_block_title_fn!(ratatui_list_set_block_title, FfiList);
crate::ratatui_block_title_spans_fn!(ratatui_list_set_block_title_spans, FfiList);

crate::ratatui_set_selected_i32_fn!(ratatui_list_set_selected, FfiList, selected);

crate::ratatui_set_style_fn!(ratatui_list_set_highlight_style, FfiList, highlight_style);

#[no_mangle]
pub extern "C" fn ratatui_list_set_highlight_symbol(lst: *mut FfiList, sym_utf8: *const c_char) {
    if lst.is_null() {
        return;
    }
    let l = unsafe { &mut *lst };
    l.highlight_symbol = if sym_utf8.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(sym_utf8) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    };
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_direction(lst: *mut FfiList, dir: u32) {
    if lst.is_null() {
        return;
    }
    let l = unsafe { &mut *lst };
    l.direction = Some(match dir {
        1 => RtListDirection::BottomToTop,
        _ => RtListDirection::TopToBottom,
    });
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_scroll_offset(lst: *mut FfiList, offset: usize) {
    if lst.is_null() {
        return;
    }
    let l = unsafe { &mut *lst };
    l.scroll_offset = Some(offset);
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_highlight_spacing(lst: *mut FfiList, spacing: u32) {
    if lst.is_null() {
        return;
    }
    let l = unsafe { &mut *lst };
    l.highlight_spacing = Some(match spacing {
        1 => RtHighlightSpacing::Never,
        2 => RtHighlightSpacing::WhenSelected,
        _ => RtHighlightSpacing::Always,
    });
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_list_in(
    term: *mut FfiTerminal,
    lst: *const FfiList,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_list_in", || {
        if term.is_null() || lst.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let l = unsafe { &*lst };
        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
        let mut widget = List::new(items);
        if let Some(b) = &l.block {
            widget = widget.block(b.clone());
        }
        if let Some(sty) = &l.highlight_style {
            widget = widget.highlight_style(sty.clone());
        }
        if let Some(sym) = &l.highlight_symbol {
            widget = widget.highlight_symbol(sym.as_str());
        }
        let res = t.terminal.draw(|frame| {
            if let Some(sel) = l.selected {
                let mut state = ratatui::widgets::ListState::default();
                state.select(Some(sel));
                frame.render_stateful_widget(widget.clone(), area, &mut state);
            } else {
                frame.render_widget(widget.clone(), area);
            }
        });
        res.is_ok()
    })
}

// ----- Gauge -----

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
    if g.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(g));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_gauge_set_ratio(g: *mut FfiGauge, ratio: f32) {
    if g.is_null() {
        return;
    }
    unsafe {
        (&mut *g).ratio = ratio.clamp(0.0, 1.0);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_gauge_set_label(g: *mut FfiGauge, label: *const c_char) {
    if g.is_null() {
        return;
    }
    let gg = unsafe { &mut *g };
    gg.label = if label.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(label) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    };
}

// Span-based label for Gauge (preferred)
#[no_mangle]
pub extern "C" fn ratatui_gauge_set_label_spans(
    g: *mut FfiGauge,
    spans: *const FfiSpan,
    len: usize,
) {
    if g.is_null() {
        return;
    }
    let gg = unsafe { &mut *g };
    if spans.is_null() || len == 0 {
        gg.label = Some(String::new());
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(spans, len) };
    let mut s = String::new();
    for sp in slice.iter() {
        if sp.text_utf8.is_null() {
            continue;
        }
        if let Ok(txt) = unsafe { CStr::from_ptr(sp.text_utf8) }.to_str() {
            s.push_str(txt);
        }
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
    if g.is_null() {
        return;
    }
    let gg = unsafe { &mut *g };
    gg.style = Some(style_from_ffi(style));
    gg.label_style = Some(style_from_ffi(label_style));
    gg.gauge_style = Some(style_from_ffi(gauge_style));
}

crate::ratatui_block_title_fn!(ratatui_gauge_set_block_title, FfiGauge);
crate::ratatui_block_title_spans_fn!(ratatui_gauge_set_block_title_spans, FfiGauge);

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_gauge_in(
    term: *mut FfiTerminal,
    g: *const FfiGauge,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_gauge_in", || {
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
        let mut widget = Gauge::default().ratio(gg.ratio as f64);
        if let Some(st) = &gg.style {
            widget = widget.style(st.clone());
        }
        if let Some(label) = &gg.label {
            widget = widget.label(label.clone());
        }
        if let Some(st) = &gg.label_style {
            widget = widget.set_style(st.clone());
        }
        if let Some(st) = &gg.gauge_style {
            widget = widget.gauge_style(st.clone());
        }
        if let Some(b) = &gg.block {
            widget = widget.block(b.clone());
        }
        let res = t.terminal.draw(|frame| {
            frame.render_widget(widget.clone(), area);
        });
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
    let mut w = Gauge::default().ratio(gg.ratio as f64);
    if let Some(st) = &gg.style {
        w = w.style(st.clone());
    }
    if let Some(label) = &gg.label {
        w = w.label(label.clone());
    }
    if let Some(st) = &gg.label_style {
        w = w.set_style(st.clone());
    }
    if let Some(st) = &gg.gauge_style {
        w = w.gauge_style(st.clone());
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

// ----- Tabs -----

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

// Span-based divider: concatenates span texts; styles are not preserved
// because ratatui Tabs.divider accepts a single Span.
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
    // Fallback: concatenate multiple spans as plain text
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
    guard_bool("ratatui_terminal_draw_tabs_in", || {
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

// ----- BarChart -----

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

crate::ratatui_block_title_fn!(ratatui_barchart_set_block_title, FfiBarChart);
crate::ratatui_block_title_spans_fn!(ratatui_barchart_set_block_title_spans, FfiBarChart);

crate::ratatui_block_adv_fn!(ratatui_barchart_set_block_adv, FfiBarChart);

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
    bc.bar_style = Some(style_from_ffi(bar));
    bc.value_style = Some(style_from_ffi(value));
    bc.label_style = Some(style_from_ffi(label));
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_barchart_in(
    term: *mut FfiTerminal,
    b: *const FfiBarChart,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_barchart_in", || {
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

// ----- Chart -----

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
    let sty = style_from_ffi(style);
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
    let sty = style_from_ffi(style);
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
            style: Some(style_from_ffi(s.style)),
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

crate::ratatui_set_style_fn!(ratatui_chart_set_style, FfiChart, chart_style);

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
    ch.x_axis_style = Some(style_from_ffi(x_style));
    ch.y_axis_style = Some(style_from_ffi(y_style));
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
        if let Some(sp) = spans_from_ffi(ls.spans, ls.len) {
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
        if let Some(sp) = spans_from_ffi(ls.spans, ls.len) {
            lines.push(Line::from(sp));
        } else {
            lines.push(Line::default());
        }
    }
    ch.y_labels = Some(lines);
}

#[no_mangle]
pub extern "C" fn ratatui_chart_set_labels_alignment(c: *mut FfiChart, x_align: u32, y_align: u32) {
    if c.is_null() {
        return;
    }
    let ch = unsafe { &mut *c };
    ch.x_labels_align = Some(match x_align {
        1 => ratatui::layout::Alignment::Center,
        2 => ratatui::layout::Alignment::Right,
        _ => ratatui::layout::Alignment::Left,
    });
    ch.y_labels_align = Some(match y_align {
        1 => ratatui::layout::Alignment::Center,
        2 => ratatui::layout::Alignment::Right,
        _ => ratatui::layout::Alignment::Left,
    });
}

crate::ratatui_block_title_fn!(ratatui_chart_set_block_title, FfiChart);
crate::ratatui_block_title_spans_fn!(ratatui_chart_set_block_title_spans, FfiChart);

crate::ratatui_block_adv_fn!(ratatui_chart_set_block_adv, FfiChart);

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_chart_in(
    term: *mut FfiTerminal,
    c: *const FfiChart,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_chart_in", || {
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

// ----- Sparkline -----

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

crate::ratatui_set_style_fn!(ratatui_sparkline_set_style, FfiSparkline, style);

crate::ratatui_block_title_fn!(ratatui_sparkline_set_block_title, FfiSparkline);
crate::ratatui_block_title_spans_fn!(ratatui_sparkline_set_block_title_spans, FfiSparkline);

crate::ratatui_block_adv_fn!(ratatui_sparkline_set_block_adv, FfiSparkline);

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_sparkline_in(
    term: *mut FfiTerminal,
    s: *const FfiSparkline,
    rect: FfiRect,
) -> bool {
    guard_bool("ratatui_terminal_draw_sparkline_in", || {
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

// ----- Scrollbar -----

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_scrollbar_new() -> *mut FfiScrollbar {
    Box::into_raw(Box::new(FfiScrollbar {
        orient: FfiScrollbarOrient::Vertical as u32,
        position: 0,
        content_len: 0,
        viewport_len: 0,
        block: None,
        side: None,
    }))
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_scrollbar_free(s: *mut FfiScrollbar) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(s));
    }
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_scrollbar_configure(
    s: *mut FfiScrollbar,
    orient: u32,
    position: u16,
    content_len: u16,
    viewport_len: u16,
) {
    if s.is_null() {
        return;
    }
    let sb = unsafe { &mut *s };
    sb.orient = orient;
    sb.position = position;
    sb.content_len = content_len;
    sb.viewport_len = viewport_len;
}

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
crate::ratatui_block_title_fn!(ratatui_scrollbar_set_block_title, FfiScrollbar);

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
crate::ratatui_block_title_spans_fn!(ratatui_scrollbar_set_block_title_spans, FfiScrollbar);

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_scrollbar_set_orientation_side(s: *mut FfiScrollbar, side: u32) {
    // side mapping: 0=VerticalLeft, 1=VerticalRight, 2=HorizontalTop, 3=HorizontalBottom
    if s.is_null() {
        return;
    }
    let sb = unsafe { &mut *s };
    sb.side = Some(side);
}

#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
crate::ratatui_block_adv_fn!(ratatui_scrollbar_set_block_adv, FfiScrollbar);

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_terminal_draw_scrollbar_in(
    term: *mut FfiTerminal,
    s: *const FfiScrollbar,
    rect: FfiRect,
) -> bool {
    if term.is_null() || s.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    let sb = unsafe { &*s };
    let area = Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    };
    let orient = if let Some(side) = sb.side {
        match side {
            0 => RtScrollbarOrientation::VerticalLeft,
            1 => RtScrollbarOrientation::VerticalRight,
            2 => RtScrollbarOrientation::HorizontalTop,
            3 => RtScrollbarOrientation::HorizontalBottom,
            _ => RtScrollbarOrientation::VerticalRight,
        }
    } else if sb.orient == FfiScrollbarOrient::Horizontal as u32 {
        RtScrollbarOrientation::HorizontalTop
    } else {
        RtScrollbarOrientation::VerticalRight
    };
    let mut state = RtScrollbarState::new(sb.content_len as usize)
        .position(sb.position as usize)
        .viewport_content_length(sb.viewport_len as usize);
    let w = RtScrollbar::new(orient);
    let res = t.terminal.draw(|frame| {
        frame.render_stateful_widget(w.clone(), area, &mut state);
    });
    res.is_ok()
}

#[no_mangle]
#[cfg(feature = "scrollbar")]
#[cfg_attr(docsrs, doc(cfg(feature = "scrollbar")))]
pub extern "C" fn ratatui_headless_render_scrollbar(
    width: u16,
    height: u16,
    s: *const FfiScrollbar,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if s.is_null() || out_text_utf8.is_null() {
        return false;
    }
    let sb = unsafe { &*s };
    let area = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut buf = Buffer::empty(area);
    let orient = if let Some(side) = sb.side {
        match side {
            0 => RtScrollbarOrientation::VerticalLeft,
            1 => RtScrollbarOrientation::VerticalRight,
            2 => RtScrollbarOrientation::HorizontalTop,
            3 => RtScrollbarOrientation::HorizontalBottom,
            _ => RtScrollbarOrientation::VerticalRight,
        }
    } else if sb.orient == FfiScrollbarOrient::Horizontal as u32 {
        RtScrollbarOrientation::HorizontalTop
    } else {
        RtScrollbarOrientation::VerticalRight
    };
    let mut state = RtScrollbarState::new(sb.content_len as usize)
        .position(sb.position as usize)
        .viewport_content_length(sb.viewport_len as usize);
    let w = RtScrollbar::new(orient);
    ratatui::widgets::StatefulWidget::render(w, area, &mut buf, &mut state);
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

// ----- Simple Table (tab-separated cells) -----

#[repr(C)]
pub struct FfiTable {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    block: Option<Block<'static>>,
    selected: Option<usize>,
    row_highlight_style: Option<Style>,
    highlight_symbol: Option<String>,
    widths_pct: Option<Vec<u16>>,
    widths_constraints: Option<Vec<Constraint>>,
    headers_spans: Option<Vec<Line<'static>>>,
    rows_spans: Option<Vec<Vec<Line<'static>>>>,
    // Optional: per-row cells with multi-line Lines per cell
    rows_cells_lines: Option<Vec<Vec<Vec<Line<'static>>>>>,
    header_style: Option<Style>,
    row_height: Option<u16>,
    column_spacing: Option<u16>,
    column_highlight_style: Option<Style>,
    cell_highlight_style: Option<Style>,
    highlight_spacing: Option<ratatui::widgets::HighlightSpacing>,
}

#[no_mangle]
pub extern "C" fn ratatui_table_new() -> *mut FfiTable {
    Box::into_raw(Box::new(FfiTable {
        headers: Vec::new(),
        rows: Vec::new(),
        block: None,
        selected: None,
        row_highlight_style: None,
        highlight_symbol: None,
        widths_pct: None,
        widths_constraints: None,
        headers_spans: None,
        rows_spans: None,
        rows_cells_lines: None,
        header_style: None,
        row_height: None,
        column_spacing: None,
        column_highlight_style: None,
        cell_highlight_style: None,
        highlight_spacing: None,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_table_free(tbl: *mut FfiTable) {
    if tbl.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(tbl));
    }
}

// TableState FFI
#[no_mangle]
pub extern "C" fn ratatui_table_state_new() -> *mut FfiTableState {
    Box::into_raw(Box::new(FfiTableState {
        selected: None,
        offset: 0,
    }))
}

#[no_mangle]
pub extern "C" fn ratatui_table_state_free(st: *mut FfiTableState) {
    if st.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(st));
    }
}

crate::ratatui_set_selected_i32_fn!(ratatui_table_state_set_selected, FfiTableState, selected);

#[no_mangle]
pub extern "C" fn ratatui_table_state_set_offset(st: *mut FfiTableState, offset: usize) {
    if st.is_null() {
        return;
    }
    unsafe {
        (&mut *st).offset = offset;
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_table_state_in(
    term: *mut FfiTerminal,
    tbl: *const FfiTable,
    rect: FfiRect,
    st: *const FfiTableState,
) -> bool {
    guard_bool("ratatui_terminal_draw_table_state_in", || {
        if term.is_null() || tbl.is_null() || st.is_null() {
            return false;
        }
        let t = unsafe { &mut *term };
        let tb = unsafe { &*tbl };
        let ss = unsafe { &*st };
        let area = Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        };
        // reuse headless builder logic to construct rows
        let header_row = if let Some(hs) = &tb.headers_spans {
            let mut r = Row::new(hs.iter().cloned().map(Cell::from).collect::<Vec<_>>());
            if let Some(hsty) = &tb.header_style {
                r = r.style(hsty.clone());
            }
            Some(r)
        } else if tb.headers.is_empty() {
            None
        } else {
            Some(Row::new(
                tb.headers
                    .iter()
                    .cloned()
                    .map(Cell::from)
                    .collect::<Vec<_>>(),
            ))
        };
        let rows: Vec<Row> = if let Some(rows_cells) = &tb.rows_cells_lines {
            rows_cells
                .iter()
                .map(|cells| {
                    let mut rc: Vec<Cell> = Vec::with_capacity(cells.len());
                    for cell_lines in cells.iter() {
                        let text = ratatui::text::Text::from(cell_lines.clone());
                        rc.push(Cell::from(text));
                    }
                    let mut row = Row::new(rc);
                    if let Some(h) = tb.row_height {
                        row = row.height(h);
                    }
                    row
                })
                .collect()
        } else if let Some(rss) = &tb.rows_spans {
            rss.iter()
                .map(|r| {
                    let mut row = Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>());
                    if let Some(h) = tb.row_height {
                        row = row.height(h);
                    }
                    row
                })
                .collect()
        } else {
            tb.rows
                .iter()
                .map(|r| {
                    let mut row = Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>());
                    if let Some(h) = tb.row_height {
                        row = row.height(h);
                    }
                    row
                })
                .collect()
        };
        let col_count = if let Some(w) = &tb.widths_pct {
            w.len().max(1)
        } else if !tb.rows.is_empty() {
            tb.rows.iter().map(|r| r.len()).max().unwrap_or(1)
        } else {
            tb.headers.len().max(1)
        };
        let widths: Vec<Constraint> = if let Some(ws) = &tb.widths_pct {
            ws.iter().map(|p| Constraint::Percentage(*p)).collect()
        } else {
            std::iter::repeat(Constraint::Percentage((100 / col_count.max(1)) as u16))
                .take(col_count.max(1))
                .collect()
        };
        let mut w = Table::new(rows, widths);
        if let Some(cs) = tb.column_spacing {
            w = w.column_spacing(cs);
        }
        if let Some(hr) = header_row {
            w = w.header(hr);
        }
        if let Some(b) = &tb.block {
            w = w.block(b.clone());
        }
        if let Some(sty) = &tb.row_highlight_style {
            w = w.row_highlight_style(sty.clone());
        }
        if let Some(sym) = &tb.highlight_symbol {
            w = w.highlight_symbol(sym.clone());
        }
        if let Some(sty) = &tb.column_highlight_style {
            w = w.column_highlight_style(sty.clone());
        }
        if let Some(sty) = &tb.cell_highlight_style {
            w = w.cell_highlight_style(sty.clone());
        }
        if let Some(sp) = &tb.highlight_spacing {
            w = w.highlight_spacing(sp.clone());
        }
        let mut state = ratatui::widgets::TableState::default();
        if let Some(sel) = ss.selected {
            state.select(Some(sel));
        }
        state = state.with_offset(ss.offset);
        let res = t.terminal.draw(|frame| {
            frame.render_stateful_widget(w.clone(), area, &mut state);
        });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_headers(tbl: *mut FfiTable, tsv_utf8: *const c_char) {
    if tbl.is_null() || tsv_utf8.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() {
        t.headers = s.split('\t').map(|x| x.to_string()).collect();
    }
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_headers_spans(
    tbl: *mut FfiTable,
    spans: *const FfiSpan,
    len: usize,
) {
    if tbl.is_null() || spans.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    if let Some(sp) = spans_from_ffi(spans, len) {
        t.headers_spans = Some(vec![Line::from(sp)]);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_table_append_row(tbl: *mut FfiTable, tsv_utf8: *const c_char) {
    if tbl.is_null() || tsv_utf8.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() {
        let row: Vec<String> = s.split('\t').map(|x| x.to_string()).collect();
        t.rows.push(row);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_table_append_row_spans(
    tbl: *mut FfiTable,
    spans: *const FfiSpan,
    len: usize,
) {
    if tbl.is_null() || spans.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    if let Some(sp) = spans_from_ffi(spans, len) {
        let line = Line::from(sp);
        if t.rows_spans.is_none() {
            t.rows_spans = Some(Vec::new());
        }
        t.rows_spans.as_mut().unwrap().push(vec![line]);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_table_append_row_cells_lines(
    tbl: *mut FfiTable,
    cells: *const FfiCellLines,
    cell_count: usize,
) {
    if tbl.is_null() || cells.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    let cells_slice = unsafe { std::slice::from_raw_parts(cells, cell_count) };
    let mut row: Vec<Vec<Line<'static>>> = Vec::with_capacity(cell_count);
    for cell in cells_slice.iter() {
        if cell.lines.is_null() || cell.len == 0 {
            row.push(Vec::new());
            continue;
        }
        let line_specs = unsafe { std::slice::from_raw_parts(cell.lines, cell.len) };
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(cell.len);
        for ls in line_specs.iter() {
            if ls.spans.is_null() || ls.len == 0 {
                lines.push(Line::default());
                continue;
            }
            if let Some(sp) = spans_from_ffi(ls.spans, ls.len) {
                lines.push(Line::from(sp));
            } else {
                lines.push(Line::default());
            }
        }
        row.push(lines);
    }
    if t.rows_cells_lines.is_none() {
        t.rows_cells_lines = Some(Vec::new());
    }
    t.rows_cells_lines.as_mut().unwrap().push(row);
}

crate::ratatui_block_title_fn!(ratatui_table_set_block_title, FfiTable);
crate::ratatui_block_title_spans_fn!(ratatui_table_set_block_title_spans, FfiTable);

crate::ratatui_set_selected_i32_fn!(ratatui_table_set_selected, FfiTable, selected);

crate::ratatui_set_style_fn!(ratatui_table_set_row_highlight_style, FfiTable, row_highlight_style);

#[no_mangle]
pub extern "C" fn ratatui_table_set_highlight_symbol(tbl: *mut FfiTable, sym_utf8: *const c_char) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    t.highlight_symbol = if sym_utf8.is_null() {
        None
    } else {
        unsafe { CStr::from_ptr(sym_utf8) }
            .to_str()
            .ok()
            .map(|s| s.to_string())
    };
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_column_highlight_style(tbl: *mut FfiTable, style: FfiStyle) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    t.column_highlight_style = Some(style_from_ffi(style));
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_cell_highlight_style(tbl: *mut FfiTable, style: FfiStyle) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    t.cell_highlight_style = Some(style_from_ffi(style));
}

#[repr(u32)]
pub enum FfiHighlightSpacing {
    Always = 0,
    Never = 1,
    WhenSelected = 2,
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_highlight_spacing(tbl: *mut FfiTable, spacing: u32) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    t.highlight_spacing = Some(match spacing {
        1 => ratatui::widgets::HighlightSpacing::Never,
        2 => ratatui::widgets::HighlightSpacing::WhenSelected,
        _ => ratatui::widgets::HighlightSpacing::Always,
    });
}

crate::ratatui_set_style_fn!(ratatui_table_set_header_style, FfiTable, header_style);

#[no_mangle]
pub extern "C" fn ratatui_table_set_row_height(tbl: *mut FfiTable, height: u16) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    t.row_height = Some(height);
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_column_spacing(tbl: *mut FfiTable, spacing: u16) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    t.column_spacing = Some(spacing);
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_widths_percentages(
    tbl: *mut FfiTable,
    widths: *const u16,
    len: usize,
) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    if widths.is_null() || len == 0 {
        t.widths_pct = None;
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(widths, len) };
    t.widths_pct = Some(slice.to_vec());
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_widths(
    tbl: *mut FfiTable,
    kinds: *const u32,
    vals: *const u16,
    len: usize,
) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    if kinds.is_null() || vals.is_null() || len == 0 {
        t.widths_pct = None;
        return;
    }
    let ks = unsafe { std::slice::from_raw_parts(kinds, len) };
    let vs = unsafe { std::slice::from_raw_parts(vals, len) };
    // We keep storing percentages for now if all are Percentage; otherwise compute percentage approximation.
    let all_pct = ks.iter().all(|&k| k == 1);
    if all_pct {
        t.widths_pct = Some(vs.to_vec());
    } else {
        // Approximate: convert Length/Min to relative percentages by normalizing values.
        let sum: u32 = vs.iter().map(|&v| v as u32).sum();
        if sum == 0 {
            t.widths_pct = None;
            return;
        }
        let pct: Vec<u16> = vs
            .iter()
            .map(|&v| ((v as u32 * 100) / sum) as u16)
            .collect();
        t.widths_pct = Some(pct);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_table_in(
    term: *mut FfiTerminal,
    tbl: *const FfiTable,
    rect: FfiRect,
) -> bool {
    if term.is_null() || tbl.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    let tb = unsafe { &*tbl };
    let area = Rect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    };

    let header_row = if tb.headers.is_empty() {
        None
    } else {
        let cells: Vec<Cell> = tb.headers.iter().map(|h| Cell::from(h.clone())).collect();
        Some(Row::new(cells))
    };
    let rows: Vec<Row> = tb
        .rows
        .iter()
        .map(|r| Row::new(r.iter().map(|c| Cell::from(c.clone())).collect::<Vec<_>>()))
        .collect();

    // Even column widths
    let col_count = if !tb.rows.is_empty() {
        tb.rows.iter().map(|r| r.len()).max().unwrap_or(1)
    } else {
        tb.headers.len().max(1)
    };
    let widths = std::iter::repeat(Constraint::Percentage((100 / col_count.max(1)) as u16))
        .take(col_count.max(1));

    let mut widget = Table::new(rows, widths);
    if let Some(hr) = header_row {
        widget = widget.header(hr);
    }
    if let Some(b) = &tb.block {
        widget = widget.block(b.clone());
    }
    if let Some(sty) = &tb.row_highlight_style {
        widget = widget.row_highlight_style(sty.clone());
    }
    if let Some(sym) = &tb.highlight_symbol {
        widget = widget.highlight_symbol(sym.clone());
    }

    let res = t.terminal.draw(|frame| {
        if let Some(sel) = tb.selected {
            let mut state = ratatui::widgets::TableState::default();
            state.select(Some(sel));
            frame.render_stateful_widget(widget.clone(), area, &mut state);
        } else {
            frame.render_widget(widget.clone(), area);
        }
    });
    res.is_ok()
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_size(out_width: *mut u16, out_height: *mut u16) -> bool {
    if out_width.is_null() || out_height.is_null() {
        return false;
    }
    match crossterm::terminal::size() {
        Ok((w, h)) => {
            unsafe {
                *out_width = w;
                *out_height = h;
            }
            true
        }
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_set_cursor_position(
    term: *mut FfiTerminal,
    x: u16,
    y: u16,
) -> bool {
    if term.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    t.terminal.set_cursor_position((x, y)).is_ok()
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_show_cursor(term: *mut FfiTerminal, show: bool) -> bool {
    if term.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    let res = if show {
        t.terminal.show_cursor()
    } else {
        t.terminal.hide_cursor()
    };
    res.is_ok()
}

// Explicit raw/alt toggles
#[no_mangle]
pub extern "C" fn ratatui_terminal_enable_raw(term: *mut FfiTerminal) -> bool {
    let _ = term; // terminal handle not required for raw mode
    match enable_raw_mode() {
        Ok(()) => true,
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_disable_raw(term: *mut FfiTerminal) -> bool {
    let _ = term;
    match disable_raw_mode() {
        Ok(()) => true,
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_enter_alt(term: *mut FfiTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    let res = execute!(stdout(), EnterAlternateScreen);
    if res.is_ok() {
        t.entered_alt = true;
        true
    } else {
        false
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_leave_alt(term: *mut FfiTerminal) -> bool {
    if term.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    let res = execute!(stdout(), LeaveAlternateScreen);
    if res.is_ok() {
        t.entered_alt = false;
        true
    } else {
        false
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_get_cursor_position(
    term: *mut FfiTerminal,
    out_x: *mut u16,
    out_y: *mut u16,
) -> bool {
    if term.is_null() || out_x.is_null() || out_y.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    match t.terminal.get_cursor_position() {
        Ok(pos) => {
            unsafe {
                *out_x = pos.x;
                *out_y = pos.y;
            }
            true
        }
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_set_viewport_area(
    term: *mut FfiTerminal,
    rect: FfiRect,
) -> bool {
    let _ = (term, rect);
    false
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_get_viewport_area(
    term: *mut FfiTerminal,
    out_rect: *mut FfiRect,
) -> bool {
    let _ = term;
    if out_rect.is_null() {
        return false;
    }
    false
}

#[no_mangle]
pub extern "C" fn ratatui_layout_split_ex(
    width: u16,
    height: u16,
    dir: u32, // 0=Vertical, 1=Horizontal
    kinds: *const u32,
    values: *const u16,
    len: usize,
    spacing: u16,
    margin_l: u16,
    margin_t: u16,
    margin_r: u16,
    margin_b: u16,
    out_rects: *mut FfiRect,
    out_cap: usize,
) -> usize {
    if kinds.is_null() || values.is_null() || out_rects.is_null() || len == 0 || out_cap == 0 {
        return 0;
    }
    let kinds_slice = unsafe { std::slice::from_raw_parts(kinds, len) };
    let vals_slice = unsafe { std::slice::from_raw_parts(values, len) };
    let mut constraints: Vec<Constraint> = Vec::with_capacity(len);
    for i in 0..len {
        constraints.push(match kinds_slice[i] {
            1 => Constraint::Percentage(vals_slice[i]),
            2 => Constraint::Min(vals_slice[i] as u16),
            _ => Constraint::Length(vals_slice[i] as u16),
        });
    }
    // Apply margins by shrinking parent rect
    let mut parent = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    if margin_l + margin_r < width {
        parent.x += margin_l;
        parent.width -= margin_l + margin_r;
    }
    if margin_t + margin_b < height {
        parent.y += margin_t;
        parent.height -= margin_t + margin_b;
    }
    let layout = Layout::new(
        if dir == 1 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        },
        constraints,
    )
    .spacing(spacing);
    let chunks = layout.split(parent);
    let n = chunks.len().min(out_cap);
    for i in 0..n {
        let r = chunks[i];
        unsafe {
            *out_rects.add(i) = FfiRect {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
            };
        }
    }
    n
}

// Extended splitter with Ratio constraints: kinds: 0=Length,1=Percentage,2=Min,3=Ratio(numer/denom)
#[no_mangle]
pub extern "C" fn ratatui_layout_split_ex2(
    width: u16,
    height: u16,
    dir: u32, // 0=Vertical, 1=Horizontal
    kinds: *const u32,
    values_a: *const u16,
    values_b: *const u16,
    len: usize,
    spacing: u16,
    margin_l: u16,
    margin_t: u16,
    margin_r: u16,
    margin_b: u16,
    out_rects: *mut FfiRect,
    out_cap: usize,
) -> usize {
    if kinds.is_null() || values_a.is_null() || out_rects.is_null() || len == 0 || out_cap == 0 {
        return 0;
    }
    let kinds_slice = unsafe { std::slice::from_raw_parts(kinds, len) };
    let a_slice = unsafe { std::slice::from_raw_parts(values_a, len) };
    let b_slice = if values_b.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(values_b, len) }
    };
    let mut constraints: Vec<Constraint> = Vec::with_capacity(len);
    for i in 0..len {
        let kind = kinds_slice[i];
        let a = a_slice[i];
        let c = match kind {
            1 => Constraint::Percentage(a),
            2 => Constraint::Min(a as u16),
            3 => {
                let b = if i < b_slice.len() && b_slice[i] != 0 {
                    b_slice[i]
                } else {
                    1
                };
                Constraint::Ratio(a as u32, b as u32)
            }
            _ => Constraint::Length(a as u16),
        };
        constraints.push(c);
    }
    // Apply margins by shrinking parent rect
    let mut parent = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    if margin_l + margin_r < width {
        parent.x += margin_l;
        parent.width -= margin_l + margin_r;
    }
    if margin_t + margin_b < height {
        parent.y += margin_t;
        parent.height -= margin_t + margin_b;
    }
    let layout = Layout::new(
        if dir == 1 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        },
        constraints,
    )
    .spacing(spacing);
    let chunks = layout.split(parent);
    let n = chunks.len().min(out_cap);
    for i in 0..n {
        let r = chunks[i];
        unsafe {
            *out_rects.add(i) = FfiRect {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
            };
        }
    }
    n
}

// ----- Layout helpers -----

#[repr(u32)]
pub enum FfiConstraintKind {
    Length = 0,
    Percentage = 1,
    Min = 2,
}

#[no_mangle]
pub extern "C" fn ratatui_layout_split(
    width: u16,
    height: u16,
    dir: u32, // 0=Vertical, 1=Horizontal
    kinds: *const u32,
    values: *const u16,
    len: usize,
    margin_l: u16,
    margin_t: u16,
    margin_r: u16,
    margin_b: u16,
    out_rects: *mut FfiRect,
    out_cap: usize,
) -> usize {
    if kinds.is_null() || values.is_null() || out_rects.is_null() || len == 0 || out_cap == 0 {
        return 0;
    }
    let kinds_slice = unsafe { std::slice::from_raw_parts(kinds, len) };
    let vals_slice = unsafe { std::slice::from_raw_parts(values, len) };
    let mut constraints: Vec<Constraint> = Vec::with_capacity(len);
    for i in 0..len {
        constraints.push(match kinds_slice[i] {
            1 => Constraint::Percentage(vals_slice[i]),
            2 => Constraint::Min(vals_slice[i] as u16),
            _ => Constraint::Length(vals_slice[i] as u16),
        });
    }
    let parent = Rect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let margin_all = ((margin_l + margin_r + margin_t + margin_b) / 4) as u16;
    let layout = Layout::new(
        if dir == 1 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        },
        constraints,
    )
    .margin(margin_all);
    let chunks = layout.split(parent);
    let n = chunks.len().min(out_cap);
    for i in 0..n {
        let r = chunks[i];
        unsafe {
            *out_rects.add(i) = FfiRect {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
            };
        }
    }
    n
}

#[no_mangle]
pub extern "C" fn ratatui_next_event(timeout_ms: u64, out_event: *mut FfiEvent) -> bool {
    if out_event.is_null() {
        return false;
    }
    let timeout = std::time::Duration::from_millis(timeout_ms);
    // Check injected events first
    if let Some(evt) = INJECTED_EVENTS.lock().unwrap().pop_front() {
        return fill_ffi_event(evt, out_event);
    }
    let has = match event::poll(timeout) {
        Ok(b) => b,
        Err(_) => return false,
    };
    if !has {
        return false;
    }
    let evt = match event::read() {
        Ok(e) => e,
        Err(_) => return false,
    };
    fill_ffi_event(evt, out_event)
}

fn ffi_key_from(k: CtKeyEvent) -> FfiKeyEvent {
    let mods = {
        let mut m = FfiKeyMods::NONE;
        if k.modifiers.contains(CtKeyModifiers::SHIFT) {
            m |= FfiKeyMods::SHIFT;
        }
        if k.modifiers.contains(CtKeyModifiers::ALT) {
            m |= FfiKeyMods::ALT;
        }
        if k.modifiers.contains(CtKeyModifiers::CONTROL) {
            m |= FfiKeyMods::CTRL;
        }
        m.bits()
    };
    match k.code {
        CtKeyCode::Char(c) => FfiKeyEvent {
            code: FfiKeyCode::Char as u32,
            ch: c as u32,
            mods,
        },
        CtKeyCode::Enter => FfiKeyEvent {
            code: FfiKeyCode::Enter as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Left => FfiKeyEvent {
            code: FfiKeyCode::Left as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Right => FfiKeyEvent {
            code: FfiKeyCode::Right as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Up => FfiKeyEvent {
            code: FfiKeyCode::Up as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Down => FfiKeyEvent {
            code: FfiKeyCode::Down as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Esc => FfiKeyEvent {
            code: FfiKeyCode::Esc as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Backspace => FfiKeyEvent {
            code: FfiKeyCode::Backspace as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Tab => FfiKeyEvent {
            code: FfiKeyCode::Tab as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Delete => FfiKeyEvent {
            code: FfiKeyCode::Delete as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Home => FfiKeyEvent {
            code: FfiKeyCode::Home as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::End => FfiKeyEvent {
            code: FfiKeyCode::End as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::PageUp => FfiKeyEvent {
            code: FfiKeyCode::PageUp as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::PageDown => FfiKeyEvent {
            code: FfiKeyCode::PageDown as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::Insert => FfiKeyEvent {
            code: FfiKeyCode::Insert as u32,
            ch: 0,
            mods,
        },
        CtKeyCode::F(n) => {
            let base = FfiKeyCode::F1 as u32;
            let code = base + (n.saturating_sub(1) as u32);
            FfiKeyEvent { code, ch: 0, mods }
        }
        _ => FfiKeyEvent {
            code: 0,
            ch: 0,
            mods,
        },
    }
}

fn fill_ffi_event(evt: CtEvent, out_event: *mut FfiEvent) -> bool {
    let mut out = FfiEvent {
        kind: FfiEventKind::None as u32,
        key: FfiKeyEvent {
            code: 0,
            ch: 0,
            mods: 0,
        },
        width: 0,
        height: 0,
        mouse_x: 0,
        mouse_y: 0,
        mouse_kind: 0,
        mouse_btn: 0,
        mouse_mods: 0,
    };
    match evt {
        CtEvent::Key(k) => {
            out.kind = FfiEventKind::Key as u32;
            out.key = ffi_key_from(k);
        }
        CtEvent::Resize(w, h) => {
            out.kind = FfiEventKind::Resize as u32;
            out.width = w;
            out.height = h;
        }
        CtEvent::Mouse(m) => {
            out.kind = FfiEventKind::Mouse as u32;
            match m.kind {
                CtMouseKind::Down(btn) => {
                    out.mouse_kind = FfiMouseKind::Down as u32;
                    out.mouse_btn = ffi_mouse_btn(btn);
                }
                CtMouseKind::Up(btn) => {
                    out.mouse_kind = FfiMouseKind::Up as u32;
                    out.mouse_btn = ffi_mouse_btn(btn);
                }
                CtMouseKind::Drag(btn) => {
                    out.mouse_kind = FfiMouseKind::Drag as u32;
                    out.mouse_btn = ffi_mouse_btn(btn);
                }
                CtMouseKind::Moved => {
                    out.mouse_kind = FfiMouseKind::Moved as u32;
                }
                CtMouseKind::ScrollUp => {
                    out.mouse_kind = FfiMouseKind::ScrollUp as u32;
                }
                CtMouseKind::ScrollDown => {
                    out.mouse_kind = FfiMouseKind::ScrollDown as u32;
                }
                _ => {}
            }
            out.mouse_x = m.column;
            out.mouse_y = m.row;
            out.mouse_mods = ffi_mods_to_u8(m.modifiers);
        }
        _ => {}
    }
    unsafe {
        *out_event = out;
    }
    true
}

fn ffi_mouse_btn(b: CtMouseButton) -> u32 {
    match b {
        CtMouseButton::Left => FfiMouseButton::Left as u32,
        CtMouseButton::Right => FfiMouseButton::Right as u32,
        CtMouseButton::Middle => FfiMouseButton::Middle as u32,
    }
}

fn ffi_mods_to_u8(m: CtKeyModifiers) -> u8 {
    let mut out = 0u8;
    if m.contains(CtKeyModifiers::SHIFT) {
        out |= FfiKeyMods::SHIFT.bits();
    }
    if m.contains(CtKeyModifiers::ALT) {
        out |= FfiKeyMods::ALT.bits();
    }
    if m.contains(CtKeyModifiers::CONTROL) {
        out |= FfiKeyMods::CTRL.bits();
    }
    out
}
#[repr(C)]
pub struct FfiLineSpans {
    pub spans: *const FfiSpan,
    pub len: usize,
}

#[repr(C)]
pub struct FfiCellLines {
    pub lines: *const FfiLineSpans,
    pub len: usize,
}

#[repr(C)]
pub struct FfiRowCellsLines {
    pub cells: *const FfiCellLines,
    pub len: usize,
}

#[no_mangle]
pub extern "C" fn ratatui_table_append_rows_cells_lines(
    tbl: *mut FfiTable,
    rows: *const FfiRowCellsLines,
    row_count: usize,
) {
    if tbl.is_null() || rows.is_null() || row_count == 0 {
        return;
    }
    let t = unsafe { &mut *tbl };
    let rows_slice = unsafe { std::slice::from_raw_parts(rows, row_count) };
    for r in rows_slice.iter() {
        if r.cells.is_null() || r.len == 0 {
            continue;
        }
        let cells_slice = unsafe { std::slice::from_raw_parts(r.cells, r.len) };
        let mut row: Vec<Vec<Line<'static>>> = Vec::with_capacity(cells_slice.len());
        for cell in cells_slice.iter() {
            if cell.lines.is_null() || cell.len == 0 {
                row.push(Vec::new());
                continue;
            }
            let line_specs = unsafe { std::slice::from_raw_parts(cell.lines, cell.len) };
            let mut lines: Vec<Line<'static>> = Vec::with_capacity(cell.len);
            for ls in line_specs.iter() {
                if ls.spans.is_null() || ls.len == 0 {
                    lines.push(Line::default());
                    continue;
                }
                if let Some(sp) = spans_from_ffi(ls.spans, ls.len) {
                    lines.push(Line::from(sp));
                } else {
                    lines.push(Line::default());
                }
            }
            row.push(lines);
        }
        if t.rows_cells_lines.is_none() {
            t.rows_cells_lines = Some(Vec::new());
        }
        t.rows_cells_lines.as_mut().unwrap().push(row);
    }
}

// Reserve helpers to minimize reallocations on bulk appends
crate::ratatui_reserve_vec_fn!(ratatui_list_reserve_items, FfiList, items);

#[no_mangle]
pub extern "C" fn ratatui_table_reserve_rows(tbl: *mut FfiTable, additional: usize) {
    if tbl.is_null() {
        return;
    }
    let t = unsafe { &mut *tbl };
    if let Some(rr) = &mut t.rows_cells_lines {
        rr.reserve(additional);
    } else if let Some(rs) = &mut t.rows_spans {
        rs.reserve(additional);
    } else {
        t.rows.reserve(additional);
    }
}

crate::ratatui_reserve_vec_fn!(ratatui_paragraph_reserve_lines, FfiParagraph, lines);

crate::ratatui_reserve_vec_fn!(ratatui_chart_reserve_datasets, FfiChart, datasets);
