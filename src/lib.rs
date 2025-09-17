#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::ffi::{c_char, CStr, CString};
use std::io::{stdout, Stdout, Write};
use std::ptr;

use crossterm::event::{
    Event as CtEvent, KeyCode as CtKeyCode, KeyEvent as CtKeyEvent, KeyModifiers as CtKeyModifiers,
    MouseButton as CtMouseButton, MouseEventKind as CtMouseKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style, Styled};
use ratatui::widgets::canvas::{
    Canvas as RtCanvas, Line as RtCanvasLine, Points as RtCanvasPoints, Rectangle as RtCanvasRect,
};
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
use ffi::widgets::canvas::FfiCanvas;
use ffi::widgets::chart::{FfiBarChart, FfiChart};
use ffi::widgets::gauge::{FfiGauge, FfiLineGauge};
use ffi::widgets::paragraph::FfiParagraph;
use ffi::widgets::scrollbar::{FfiScrollbar, FfiScrollbarOrient};
use ffi::widgets::sparkline::FfiSparkline;
use std::any::Any;
use std::fs::OpenOptions;
use std::panic::{catch_unwind, UnwindSafe};
use std::sync::OnceLock;

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
#[repr(C)]
pub struct FfiTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    entered_alt: bool,
    raw_mode: bool,
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

// ----- Canvas -----
// moved to widgets::canvas

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

// Flat slice for braille DOTS (4x2 -> 8 elements)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FfiU16Slice {
    pub ptr: *const u16,
    pub len: usize,
}

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

#[repr(u32)]
pub enum FfiHighlightSpacing {
    Always = 0,
    Never = 1,
    WhenSelected = 2,
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
    let align = align_of::<T>();
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
    let size = size_of::<T>();
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
    let align = align_of::<T>();
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
            ptr::null_mut()
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

// moved to widgets::list

// moved to widgets::gauge

// moved to widgets::tabs

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
pub enum FfiAlignment {
    Left = 0,
    Center = 1,
    Right = 2,
}

#[repr(u32)]
pub enum FfiDirection {
    Horizontal = 0,
    Vertical = 1,
}

#[repr(u32)]
pub enum FfiFlex {
    Legacy = 0,
    Start = 1,
    End = 2,
    Center = 3,
    SpaceBetween = 4,
    SpaceAround = 5,
}

#[repr(u32)]
pub enum FfiGraphType {
    Scatter = 0,
    Line = 1,
    Bar = 2,
}

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
pub enum FfiRenderDirection {
    LeftToRight = 0,
    RightToLeft = 1,
}

#[repr(u32)]
pub enum FfiListDirection {
    TopToBottom = 0,
    BottomToTop = 1,
    LeftToRight = 2,
}

#[repr(u32)]
pub enum FfiPosition {
    Top = 0,
    Bottom = 1,
}

#[repr(u32)]
pub enum FfiMapResolution {
    Low = 0,
    High = 1,
}

#[repr(u32)]
pub enum FfiSize {
    Tiny = 0,
    Small = 1,
}

#[repr(u32)]
pub enum FfiConstraint {
    Min = 0,
    Max = 1,
    Length = 2,
    Percentage = 3,
    Ratio = 4,
    Fill = 5,
}

#[repr(u32)]
pub enum FfiSpacing {
    Space = 0,
    Overlap = 1,
}

#[repr(u32)]
pub enum FfiMarker {
    Dot = 0,
    Block = 1,
    Bar = 2,
    Braille = 3,
    HalfBlock = 4,
}

#[repr(u32)]
pub enum FfiClearType {
    All = 0,
    AfterCursor = 1,
    BeforeCursor = 2,
    CurrentLine = 3,
    UntilNewLine = 4,
}

#[repr(u32)]
pub enum FfiViewport {
    Fullscreen = 0,
    Inline = 1,
    Fixed = 2,
}

// ----- Ratatui Symbols & Palettes (generated) -----
include!("ffi/generated.rs");
ratatui_const_str_getter!(
    ratatui_symbols_get_double_vertical,
    symbols::line::DOUBLE_VERTICAL
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_vertical,
    symbols::line::THICK_VERTICAL
);
ratatui_const_str_getter!(
    ratatui_symbols_get_horizontal,
    symbols::line::HORIZONTAL
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_horizontal,
    symbols::line::DOUBLE_HORIZONTAL
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_horizontal,
    symbols::line::THICK_HORIZONTAL
);
ratatui_const_str_getter!(
    ratatui_symbols_get_top_right,
    symbols::line::TOP_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_rounded_top_right,
    symbols::line::ROUNDED_TOP_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_top_right,
    symbols::line::DOUBLE_TOP_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_top_right,
    symbols::line::THICK_TOP_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_top_left,
    symbols::line::TOP_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_rounded_top_left,
    symbols::line::ROUNDED_TOP_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_top_left,
    symbols::line::DOUBLE_TOP_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_top_left,
    symbols::line::THICK_TOP_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_bottom_right,
    symbols::line::BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_rounded_bottom_right,
    symbols::line::ROUNDED_BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_bottom_right,
    symbols::line::DOUBLE_BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_bottom_right,
    symbols::line::THICK_BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_bottom_left,
    symbols::line::BOTTOM_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_rounded_bottom_left,
    symbols::line::ROUNDED_BOTTOM_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_bottom_left,
    symbols::line::DOUBLE_BOTTOM_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_bottom_left,
    symbols::line::THICK_BOTTOM_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_vertical_left,
    symbols::line::VERTICAL_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_vertical_left,
    symbols::line::DOUBLE_VERTICAL_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_vertical_left,
    symbols::line::THICK_VERTICAL_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_vertical_right,
    symbols::line::VERTICAL_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_vertical_right,
    symbols::line::DOUBLE_VERTICAL_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_vertical_right,
    symbols::line::THICK_VERTICAL_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_horizontal_down,
    symbols::line::HORIZONTAL_DOWN
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_horizontal_down,
    symbols::line::DOUBLE_HORIZONTAL_DOWN
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_horizontal_down,
    symbols::line::THICK_HORIZONTAL_DOWN
);
ratatui_const_str_getter!(
    ratatui_symbols_get_horizontal_up,
    symbols::line::HORIZONTAL_UP
);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_horizontal_up,
    symbols::line::DOUBLE_HORIZONTAL_UP
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_horizontal_up,
    symbols::line::THICK_HORIZONTAL_UP
);
ratatui_const_str_getter!(ratatui_symbols_get_cross, symbols::line::CROSS);
ratatui_const_str_getter!(
    ratatui_symbols_get_double_cross,
    symbols::line::DOUBLE_CROSS
);
ratatui_const_str_getter!(
    ratatui_symbols_get_thick_cross,
    symbols::line::THICK_CROSS
);

// border.rs quadrants and one-eighths
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_left,
    symbols::border::QUADRANT_TOP_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_right,
    symbols::border::QUADRANT_TOP_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_bottom_left,
    symbols::border::QUADRANT_BOTTOM_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_bottom_right,
    symbols::border::QUADRANT_BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_half,
    symbols::border::QUADRANT_TOP_HALF
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_bottom_half,
    symbols::border::QUADRANT_BOTTOM_HALF
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_left_half,
    symbols::border::QUADRANT_LEFT_HALF
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_right_half,
    symbols::border::QUADRANT_RIGHT_HALF
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_left_bottom_left_bottom_right,
    symbols::border::QUADRANT_TOP_LEFT_BOTTOM_LEFT_BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_left_top_right_bottom_left,
    symbols::border::QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_left_top_right_bottom_right,
    symbols::border::QUADRANT_TOP_LEFT_TOP_RIGHT_BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_right_bottom_left_bottom_right,
    symbols::border::QUADRANT_TOP_RIGHT_BOTTOM_LEFT_BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_left_bottom_right,
    symbols::border::QUADRANT_TOP_LEFT_BOTTOM_RIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_top_right_bottom_left,
    symbols::border::QUADRANT_TOP_RIGHT_BOTTOM_LEFT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_quadrant_block,
    symbols::border::QUADRANT_BLOCK
);

ratatui_const_str_getter!(
    ratatui_symbols_get_one_eighth_top_eight,
    symbols::border::ONE_EIGHTH_TOP_EIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_one_eighth_bottom_eight,
    symbols::border::ONE_EIGHTH_BOTTOM_EIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_one_eighth_left_eight,
    symbols::border::ONE_EIGHTH_LEFT_EIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_one_eighth_right_eight,
    symbols::border::ONE_EIGHTH_RIGHT_EIGHT
);

// line.rs Set getters
ratatui_const_struct_getter!(
    ratatui_symbols_get_line_normal,
    FfiLineSet,
    symbols::line::NORMAL,
    [
        vertical,
        horizontal,
        top_right,
        top_left,
        bottom_right,
        bottom_left,
        vertical_left,
        vertical_right,
        horizontal_down,
        horizontal_up,
        cross
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_line_rounded,
    FfiLineSet,
    symbols::line::ROUNDED,
    [
        vertical,
        horizontal,
        top_right,
        top_left,
        bottom_right,
        bottom_left,
        vertical_left,
        vertical_right,
        horizontal_down,
        horizontal_up,
        cross
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_line_double,
    FfiLineSet,
    symbols::line::DOUBLE,
    [
        vertical,
        horizontal,
        top_right,
        top_left,
        bottom_right,
        bottom_left,
        vertical_left,
        vertical_right,
        horizontal_down,
        horizontal_up,
        cross
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_line_thick,
    FfiLineSet,
    symbols::line::THICK,
    [
        vertical,
        horizontal,
        top_right,
        top_left,
        bottom_right,
        bottom_left,
        vertical_left,
        vertical_right,
        horizontal_down,
        horizontal_up,
        cross
    ]
);

// border.rs Set getters
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_plain,
    FfiBorderSet,
    symbols::border::PLAIN,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_rounded,
    FfiBorderSet,
    symbols::border::ROUNDED,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_double,
    FfiBorderSet,
    symbols::border::DOUBLE,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_thick,
    FfiBorderSet,
    symbols::border::THICK,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_quadrant_outside,
    FfiBorderSet,
    symbols::border::QUADRANT_OUTSIDE,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_quadrant_inside,
    FfiBorderSet,
    symbols::border::QUADRANT_INSIDE,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_one_eighth_wide,
    FfiBorderSet,
    symbols::border::ONE_EIGHTH_WIDE,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_one_eighth_tall,
    FfiBorderSet,
    symbols::border::ONE_EIGHTH_TALL,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_proportional_wide,
    FfiBorderSet,
    symbols::border::PROPORTIONAL_WIDE,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_proportional_tall,
    FfiBorderSet,
    symbols::border::PROPORTIONAL_TALL,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_border_full,
    FfiBorderSet,
    symbols::border::FULL,
    [
        top_left,
        top_right,
        bottom_left,
        bottom_right,
        vertical_left,
        vertical_right,
        horizontal_top,
        horizontal_bottom
    ]
);

// symbols.rs base
ratatui_const_str_getter!(ratatui_symbols_get_dot, symbols::DOT);
// block scalar levels
ratatui_const_str_getter!(
    ratatui_symbols_get_block_full,
    symbols::block::FULL
);
ratatui_const_str_getter!(
    ratatui_symbols_get_block_seven_eighths,
    symbols::block::SEVEN_EIGHTHS
);
ratatui_const_str_getter!(
    ratatui_symbols_get_block_three_quarters,
    symbols::block::THREE_QUARTERS
);
ratatui_const_str_getter!(
    ratatui_symbols_get_block_five_eighths,
    symbols::block::FIVE_EIGHTHS
);
ratatui_const_str_getter!(
    ratatui_symbols_get_block_half,
    symbols::block::HALF
);
ratatui_const_str_getter!(
    ratatui_symbols_get_block_three_eighths,
    symbols::block::THREE_EIGHTHS
);
ratatui_const_str_getter!(
    ratatui_symbols_get_block_one_quarter,
    symbols::block::ONE_QUARTER
);
ratatui_const_str_getter!(
    ratatui_symbols_get_block_one_eighth,
    symbols::block::ONE_EIGHTH
);
// bar scalar levels
ratatui_const_str_getter!(ratatui_symbols_get_bar_full, symbols::bar::FULL);
ratatui_const_str_getter!(
    ratatui_symbols_get_bar_seven_eighths,
    symbols::bar::SEVEN_EIGHTHS
);
ratatui_const_str_getter!(
    ratatui_symbols_get_bar_three_quarters,
    symbols::bar::THREE_QUARTERS
);
ratatui_const_str_getter!(
    ratatui_symbols_get_bar_five_eighths,
    symbols::bar::FIVE_EIGHTHS
);
ratatui_const_str_getter!(ratatui_symbols_get_bar_half, symbols::bar::HALF);
ratatui_const_str_getter!(
    ratatui_symbols_get_bar_three_eighths,
    symbols::bar::THREE_EIGHTHS
);
ratatui_const_str_getter!(
    ratatui_symbols_get_bar_one_quarter,
    symbols::bar::ONE_QUARTER
);
ratatui_const_str_getter!(
    ratatui_symbols_get_bar_one_eighth,
    symbols::bar::ONE_EIGHTH
);
// braille scalars
ratatui_const_u16_getter!(
    ratatui_symbols_get_braille_blank,
    symbols::braille::BLANK
);
ratatui_const_str_getter!(
    ratatui_symbols_get_shade_empty,
    symbols::shade::EMPTY
);
ratatui_const_str_getter!(
    ratatui_symbols_get_shade_light,
    symbols::shade::LIGHT
);
ratatui_const_str_getter!(
    ratatui_symbols_get_shade_medium,
    symbols::shade::MEDIUM
);
ratatui_const_str_getter!(
    ratatui_symbols_get_shade_dark,
    symbols::shade::DARK
);
ratatui_const_str_getter!(
    ratatui_symbols_get_shade_full,
    symbols::shade::FULL
);
ratatui_const_char_getter!(
    ratatui_symbols_get_half_block_upper,
    symbols::half_block::UPPER
);
ratatui_const_char_getter!(
    ratatui_symbols_get_half_block_lower,
    symbols::half_block::LOWER
);
ratatui_const_char_getter!(
    ratatui_symbols_get_half_block_full,
    symbols::half_block::FULL
);

// block/bar level sets
ratatui_const_struct_getter!(
    ratatui_symbols_get_block_three_levels,
    FfiLevelSet,
    symbols::block::THREE_LEVELS,
    [
        full,
        seven_eighths,
        three_quarters,
        five_eighths,
        half,
        three_eighths,
        one_quarter,
        one_eighth,
        empty
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_block_nine_levels,
    FfiLevelSet,
    symbols::block::NINE_LEVELS,
    [
        full,
        seven_eighths,
        three_quarters,
        five_eighths,
        half,
        three_eighths,
        one_quarter,
        one_eighth,
        empty
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_bar_three_levels,
    FfiLevelSet,
    symbols::bar::THREE_LEVELS,
    [
        full,
        seven_eighths,
        three_quarters,
        five_eighths,
        half,
        three_eighths,
        one_quarter,
        one_eighth,
        empty
    ]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_bar_nine_levels,
    FfiLevelSet,
    symbols::bar::NINE_LEVELS,
    [
        full,
        seven_eighths,
        three_quarters,
        five_eighths,
        half,
        three_eighths,
        one_quarter,
        one_eighth,
        empty
    ]
);

// scrollbar sets
ratatui_const_struct_getter!(
    ratatui_symbols_get_scrollbar_double_vertical,
    FfiScrollbarSet,
    symbols::scrollbar::DOUBLE_VERTICAL,
    [track, thumb, begin, end]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_scrollbar_double_horizontal,
    FfiScrollbarSet,
    symbols::scrollbar::DOUBLE_HORIZONTAL,
    [track, thumb, begin, end]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_scrollbar_vertical,
    FfiScrollbarSet,
    symbols::scrollbar::VERTICAL,
    [track, thumb, begin, end]
);
ratatui_const_struct_getter!(
    ratatui_symbols_get_scrollbar_horizontal,
    FfiScrollbarSet,
    symbols::scrollbar::HORIZONTAL,
    [track, thumb, begin, end]
);

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


// Auto-generated FFI structs for symbol sets (all fields are UTF-8 string slices)
ratatui_define_ffi_str_struct!(FfiLineSet: vertical, horizontal, top_right, top_left, bottom_right, bottom_left, vertical_left, vertical_right, horizontal_down, horizontal_up, cross);
ratatui_define_ffi_str_struct!(FfiBorderSet: top_left, top_right, bottom_left, bottom_right, vertical_left, vertical_right, horizontal_top, horizontal_bottom);
ratatui_define_ffi_str_struct!(FfiLevelSet: full, seven_eighths, three_quarters, five_eighths, half, three_eighths, one_quarter, one_eighth, empty);
ratatui_define_ffi_str_struct!(FfiScrollbarSet: track, thumb, begin, end);

// Auto-generated FFI structs for color palettes will be included from generated.rs

// (structs generated by macros above)

const __RATATUI_BRAILLE_DOTS: [[u16; 2]; 4] = symbols::braille::DOTS;
const __RATATUI_BRAILLE_DOTS_FLAT: [u16; 8] = [
    __RATATUI_BRAILLE_DOTS[0][0],
    __RATATUI_BRAILLE_DOTS[0][1],
    __RATATUI_BRAILLE_DOTS[1][0],
    __RATATUI_BRAILLE_DOTS[1][1],
    __RATATUI_BRAILLE_DOTS[2][0],
    __RATATUI_BRAILLE_DOTS[2][1],
    __RATATUI_BRAILLE_DOTS[3][0],
    __RATATUI_BRAILLE_DOTS[3][1],
];

#[no_mangle]
pub extern "C" fn ratatui_symbols_get_braille_dots_flat() -> FfiU16Slice {
    FfiU16Slice {
        ptr: __RATATUI_BRAILLE_DOTS_FLAT.as_ptr(),
        len: __RATATUI_BRAILLE_DOTS_FLAT.len(),
    }
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
        1 => Alignment::Center,
        2 => Alignment::Right,
        _ => Alignment::Left,
    };
    b.title_alignment(align)
}

// ----- Block title alignment setters (additive; do not break existing APIs) -----

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

// ----- LineGauge -----

// moved to widgets::gauge (linegauge)

// moved to widgets::clear

// ----- RatatuiLogo -----

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
pub extern "C" fn ratatui_inject_resize(width: u16, height: u16) {
    INJECTED_EVENTS
        .lock()
        .unwrap()
        .push_back(CtEvent::Resize(width, height));
}


// ----- Layout helpers -----

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

// moved to widgets::paragraph and widgets::chart
