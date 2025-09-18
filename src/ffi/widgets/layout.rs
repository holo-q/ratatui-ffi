// Module split placeholder: Layout helpers
// Move from src/lib.rs:
// - ratatui_layout_split, ratatui_layout_split_ex, ratatui_layout_split_ex2
// Types used: FfiDirection, FfiFlex, etc.

// use crate::*; // enable when moving implementations

use crate::{FfiEvent, FfiRect, FfiTerminal, INJECTED_EVENTS};
use crate::{FfiKeyCode, FfiKeyMods, FfiMouseKind};
use crossterm::event::{
    Event as CtEvent, KeyCode as CtKeyCode, KeyEvent as CtKeyEvent, KeyModifiers as CtKeyModifiers,
    MouseButton as CtMouseButton, MouseEvent as CtMouseEvent, MouseEventKind as CtMouseKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{event, execute};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use std::io::stdout;

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

#[allow(dead_code)]
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
        return crate::fill_ffi_event(evt, out_event);
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
    crate::fill_ffi_event(evt, out_event)
}

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
