// Widget split placeholder: Canvas
// Move from src/lib.rs:
// - Types: FfiCanvas, FfiCanvasLine, FfiCanvasRect, FfiCanvasPoints
// - FFI externs: ratatui_canvas_new, ratatui_canvas_free
// - Setters: ratatui_canvas_set_bounds, ratatui_canvas_set_background_color, ratatui_canvas_set_marker
// - Block helpers (macros and manual):
//   ratatui_block_title_fn!(ratatui_canvas_set_block_title, FfiCanvas)
//   ratatui_block_title_spans_fn!(ratatui_canvas_set_block_title_spans, FfiCanvas)
//   ratatui_canvas_set_block_adv (manual using build_block_from_adv)
// - Adders: ratatui_canvas_add_line, ratatui_canvas_add_rect, ratatui_canvas_add_points
// - Draw helpers: ratatui_terminal_draw_canvas_in, ratatui_headless_render_canvas

// use crate::*; // enable when moving implementations

use crossterm::event::{Event as CtEvent, KeyModifiers as CtKeyModifiers, MouseButton as CtMouseButton, MouseEvent as CtMouseEvent, MouseEventKind as CtMouseKind};
use ratatui::prelude::{Color, Widget};
use ratatui::widgets::Block;
use ratatui::symbols::Marker as RtMarker;
use ratatui::layout::Rect;
use ratatui::widgets::canvas::{Canvas as RtCanvas, Line as RtCanvasLine, Points as RtCanvasPoints, Rectangle as RtCanvasRect};
use std::ffi::{c_char, CString};
use ratatui::buffer::Buffer;
use crate::{FfiKeyMods, FfiMouseKind, FfiRect, FfiSpan, FfiStyle, FfiTerminal, INJECTED_EVENTS};

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
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub background: Option<Color>,
    pub block: Option<Block<'static>>,
    pub marker: Option<RtMarker>,
    pub lines: Vec<FfiCanvasLine>,
    pub rects: Vec<FfiCanvasRect>,
    pub pts: Vec<(Vec<(f64, f64)>, Color)>,
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
        (&mut *c).background = crate::color_from_u32(color);
    }
}

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
    cv.block = Some(crate::build_block_from_adv(
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
    let col = crate::color_from_u32(style.fg).unwrap_or(Color::White);
    let _ = marker; // marker not supported in ratatui 0.29 Points shape; ignored
    cv.pts.push((pts, col));
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_canvas_in(
    term: *mut FfiTerminal,
    c: *const FfiCanvas,
    rect: FfiRect,
) -> bool {
    crate::guard_bool("ratatui_terminal_draw_canvas_in", || {
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
                let col = crate::color_from_u32(l.style.fg).unwrap_or(Color::White);
                p.draw(&RtCanvasLine {
                    x1: l.x1,
                    y1: l.y1,
                    x2: l.x2,
                    y2: l.y2,
                    color: col,
                });
            }
            for r in &cv.rects {
                let col = crate::color_from_u32(r.style.fg).unwrap_or(Color::White);
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
            let col = crate::color_from_u32(l.style.fg).unwrap_or(Color::White);
            p.draw(&RtCanvasLine {
                x1: l.x1,
                y1: l.y1,
                x2: l.x2,
                y2: l.y2,
                color: col,
            });
        }
        for r in &cv.rects {
            let col = crate::color_from_u32(r.style.fg).unwrap_or(Color::White);
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
