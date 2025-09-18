#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::ffi::{c_char, CString};
use std::io::{Stdout, Write};
use std::ptr;

use crossterm::event::{
    Event as CtEvent, KeyCode as CtKeyCode, KeyEvent as CtKeyEvent, KeyModifiers as CtKeyModifiers,
    MouseButton as CtMouseButton, MouseEventKind as CtMouseKind,
};
// terminal enter/exit helpers live in ffi::terminal now
use ratatui::backend::CrosstermBackend;
use ratatui::prelude::*;
mod ffi;
#[allow(unused_imports)]
pub use crate::ffi::types::*;
#[allow(unused_imports)]
pub use crate::ffi::util::*;
#[allow(unused_imports)]
pub use crate::ffi::{FfiList, FfiListState, FfiTable, FfiTableState, FfiTabs, FfiTabsStyles};
use std::collections::VecDeque;
use std::sync::Mutex;

use ffi::widgets::gauge::{FfiGauge, FfiLineGauge};
use ffi::widgets::paragraph::FfiParagraph;
use std::any::Any;
use std::fs::OpenOptions;
use std::panic::{catch_unwind, UnwindSafe};
use std::sync::OnceLock;

include!("ffi/generated.rs");

// ABI-stable mirrors of upstream enums. These are generated to ensure parity.
include!("ffi/generated_enums.rs");

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

#[repr(C)]
pub struct FfiDrawCmd {
    pub kind: u32,
    pub handle: *const (),
    pub rect: FfiRect,
}

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

#[no_mangle]
pub extern "C" fn ratatui_color_rgb(r: u8, g: u8, b: u8) -> u32 {
    0x8000_0000u32 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

#[no_mangle]
pub extern "C" fn ratatui_color_indexed(index: u8) -> u32 {
    0x4000_0000u32 | (index as u32)
}

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

#[no_mangle]
pub extern "C" fn ratatui_string_free(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

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
        return ffi::render::draw_frame(t, slice);
    })
}

static INJECTED_EVENTS: Mutex<VecDeque<CtEvent>> = Mutex::new(VecDeque::new());

#[no_mangle]
pub extern "C" fn ratatui_inject_resize(width: u16, height: u16) {
    INJECTED_EVENTS
        .lock()
        .unwrap()
        .push_back(CtEvent::Resize(width, height));
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
