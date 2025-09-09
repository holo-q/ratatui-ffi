use std::ffi::{c_char, CStr, CString};
use std::io::{stdout, Stdout};
use std::ptr;

use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::event::{self, Event as CtEvent, KeyEvent as CtKeyEvent, KeyCode as CtKeyCode, KeyModifiers as CtKeyModifiers, MouseEvent as CtMouseEvent, MouseEventKind as CtMouseKind, MouseButton as CtMouseButton};
use std::collections::VecDeque;
use std::sync::Mutex;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Rect, Constraint};
use ratatui::buffer::Buffer;
use ratatui::prelude::*;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Table, Row, Cell, Gauge, Tabs, BarChart as RtBarChart, Sparkline as RtSparkline, Scrollbar as RtScrollbar, ScrollbarOrientation as RtScrollbarOrientation, ScrollbarState as RtScrollbarState};

#[repr(C)]
pub struct FfiTerminal {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

#[repr(C)]
pub struct FfiParagraph {
    lines: Vec<Line<'static>>,          // content
    block: Option<Block<'static>>,      // optional block with borders/title
}

#[repr(C)]
pub struct FfiList {
    items: Vec<Line<'static>>,
    block: Option<Block<'static>>,
    selected: Option<usize>,
    highlight_style: Option<Style>,
    highlight_symbol: Option<String>,
}

#[repr(C)]
pub struct FfiGauge { ratio: f32, label: Option<String>, block: Option<Block<'static>> }

#[repr(C)]
pub struct FfiTabs { titles: Vec<String>, selected: u16, block: Option<Block<'static>> }

#[repr(C)]
pub struct FfiBarChart { values: Vec<u64>, labels: Vec<String>, block: Option<Block<'static>> }

#[repr(C)]
pub struct FfiSparkline { values: Vec<u64>, block: Option<Block<'static>> }

#[repr(u32)]
pub enum FfiScrollbarOrient { Vertical = 0, Horizontal = 1 }

#[repr(C)]
pub struct FfiScrollbar { orient: u32, position: u16, content_len: u16, viewport_len: u16, block: Option<Block<'static>> }

#[repr(C)]
pub struct FfiRect { pub x: u16, pub y: u16, pub width: u16, pub height: u16 }

#[repr(u32)]
pub enum FfiEventKind { None = 0, Key = 1, Resize = 2, Mouse = 3 }

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
pub struct FfiKeyEvent { pub code: u32, pub ch: u32, pub mods: u8 }

#[repr(C)]
pub struct FfiEvent { pub kind: u32, pub key: FfiKeyEvent, pub width: u16, pub height: u16, pub mouse_x: u16, pub mouse_y: u16, pub mouse_kind: u32, pub mouse_btn: u32, pub mouse_mods: u8 }

#[repr(u32)]
pub enum FfiMouseKind { Down = 1, Up = 2, Drag = 3, Moved = 4, ScrollUp = 5, ScrollDown = 6 }

#[repr(u32)]
pub enum FfiMouseButton { Left = 1, Right = 2, Middle = 3, None = 0 }

#[no_mangle]
pub extern "C" fn ratatui_init_terminal() -> *mut FfiTerminal {
    let mut out = stdout();
    // Best-effort: try raw mode and alternate screen, but don't fail hard if not available.
    let _ = enable_raw_mode();
    let _ = execute!(out, EnterAlternateScreen);
    let backend = CrosstermBackend::new(out);
    match Terminal::new(backend) {
        Ok(terminal) => Box::into_raw(Box::new(FfiTerminal { terminal })),
        Err(_) => {
            let _ = disable_raw_mode();
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_clear(term: *mut FfiTerminal) {
    if term.is_null() {
        return;
    }
    let t = unsafe { &mut *term };
    let _ = t.terminal.clear();
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_free(term: *mut FfiTerminal) {
    if term.is_null() {
        return;
    }
    // Take ownership and drop after restoring terminal state
    let mut boxed = unsafe { Box::from_raw(term) };
    let _ = boxed.terminal.show_cursor();
    // Leave alternate screen and disable raw mode
    let _ = execute!(stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
    // Drop happens here
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
    Box::into_raw(Box::new(FfiParagraph { lines, block: None }))
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_set_block_title(
    para: *mut FfiParagraph,
    title_utf8: *const c_char,
    show_border: bool,
) {
    if para.is_null() {
        return;
    }
    let p = unsafe { &mut *para };
    let mut block = if show_border { Block::default().borders(Borders::ALL) } else { Block::default() };
    if !title_utf8.is_null() {
        let c_str = unsafe { CStr::from_ptr(title_utf8) };
        if let Ok(title) = c_str.to_str() {
            block = block.title(title.to_string());
        }
    }
    p.block = Some(block);
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_free(para: *mut FfiParagraph) {
    if para.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(para)) };
}

// ----- Styles -----

#[repr(u32)]
pub enum FfiColor {
    Reset = 0,
    Black, Red, Green, Yellow, Blue, Magenta, Cyan, Gray,
    DarkGray, LightRed, LightGreen, LightYellow, LightBlue, LightMagenta, LightCyan, White,
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
    }
}

#[repr(C)]
pub struct FfiStyle { pub fg: u32, pub bg: u32, pub mods: u16 }

fn color_from_u32(c: u32) -> Option<Color> {
    match c {
        0 => None,
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

fn style_from_ffi(s: FfiStyle) -> Style {
    let mut st = Style::default();
    if let Some(fg) = color_from_u32(s.fg) { st = st.fg(fg); }
    if let Some(bg) = color_from_u32(s.bg) { st = st.bg(bg); }
    let mods = FfiStyleMods::from_bits_truncate(s.mods);
    if mods.contains(FfiStyleMods::BOLD) { st = st.add_modifier(Modifier::BOLD); }
    if mods.contains(FfiStyleMods::ITALIC) { st = st.add_modifier(Modifier::ITALIC); }
    if mods.contains(FfiStyleMods::UNDERLINE) { st = st.add_modifier(Modifier::UNDERLINED); }
    if mods.contains(FfiStyleMods::DIM) { st = st.add_modifier(Modifier::DIM); }
    if mods.contains(FfiStyleMods::CROSSED) { st = st.add_modifier(Modifier::CROSSED_OUT); }
    if mods.contains(FfiStyleMods::REVERSED) { st = st.add_modifier(Modifier::REVERSED); }
    if mods.contains(FfiStyleMods::RAPIDBLINK) { st = st.add_modifier(Modifier::RAPID_BLINK); }
    if mods.contains(FfiStyleMods::SLOWBLINK) { st = st.add_modifier(Modifier::SLOW_BLINK); }
    st
}

#[no_mangle]
pub extern "C" fn ratatui_paragraph_append_line(para: *mut FfiParagraph, text_utf8: *const c_char, style: FfiStyle) {
    if para.is_null() || text_utf8.is_null() { return; }
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
    if term.is_null() || para.is_null() {
        return false;
    }
    let t = unsafe { &mut *term };
    let p = unsafe { &*para };
    let lines = p.lines.clone();
    let mut widget = Paragraph::new(lines);
    if let Some(b) = &p.block { widget = widget.block(b.clone()); }
    let res = t.terminal.draw(|frame| {
        let area: Rect = frame.area();
        frame.render_widget(widget.clone(), area);
    });
    res.is_ok()
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_paragraph_in(
    term: *mut FfiTerminal,
    para: *const FfiParagraph,
    rect: FfiRect,
) -> bool {
    if term.is_null() || para.is_null() { return false; }
    let t = unsafe { &mut *term };
    let p = unsafe { &*para };
    let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    let lines = p.lines.clone();
    let mut widget = Paragraph::new(lines);
    if let Some(b) = &p.block { widget = widget.block(b.clone()); }
    let res = t.terminal.draw(|frame| {
        frame.render_widget(widget.clone(), area);
    });
    res.is_ok()
}

// ----- Headless rendering helpers (for smoke tests) -----

#[no_mangle]
pub extern "C" fn ratatui_string_free(ptr: *mut c_char) {
    if ptr.is_null() { return; }
    unsafe { drop(CString::from_raw(ptr)); }
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_paragraph(
    width: u16,
    height: u16,
    para: *const FfiParagraph,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if para.is_null() || out_text_utf8.is_null() { return false; }
    let p = unsafe { &*para };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let mut widget = Paragraph::new(p.lines.clone());
    if let Some(b) = &p.block { widget = widget.block(b.clone()); }
    ratatui::widgets::Widget::render(widget, area, &mut buf);

    // Serialize buffer into UTF-8 lines
    let mut s = String::new();
    for y in 0..height { 
        for x in 0..width { 
            let cell = &buf[(x, y)];
            s.push_str(cell.symbol());
        }
        if y + 1 < height { s.push('\n'); }
    }
    match CString::new(s) {
        Ok(cstr) => {
            unsafe { *out_text_utf8 = cstr.into_raw(); }
            true
        }
        Err(_) => false,
    }
}

// ----- Headless List/Table and Composite Frame -----

#[no_mangle]
pub extern "C" fn ratatui_headless_render_list(
    width: u16,
    height: u16,
    lst: *const FfiList,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if lst.is_null() || out_text_utf8.is_null() { return false; }
    let l = unsafe { &*lst };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
    let mut widget = List::new(items);
    if let Some(b) = &l.block { widget = widget.block(b.clone()); }
    if let Some(sty) = &l.highlight_style { widget = widget.highlight_style(sty.clone()); }
    if let Some(sym) = &l.highlight_symbol { widget = widget.highlight_symbol(sym.clone()); }
    if let Some(sel) = l.selected {
        let mut state = ratatui::widgets::ListState::default();
        state.select(Some(sel));
        ratatui::widgets::StatefulWidget::render(widget, area, &mut buf, &mut state);
    } else {
        ratatui::widgets::Widget::render(widget, area, &mut buf);
    }
    let mut s = String::new();
    for y in 0..height { for x in 0..width { let cell = &buf[(x, y)]; s.push_str(cell.symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_table(
    width: u16,
    height: u16,
    tbl: *const FfiTable,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if tbl.is_null() || out_text_utf8.is_null() { return false; }
    let tb = unsafe { &*tbl };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let header_row = if tb.headers.is_empty() { None } else { Some(Row::new(tb.headers.iter().cloned().map(Cell::from).collect::<Vec<_>>())) };
    let rows: Vec<Row> = tb.rows.iter().map(|r| Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>())).collect();
    let col_count = if !tb.rows.is_empty() { tb.rows.iter().map(|r| r.len()).max().unwrap_or(1) } else { tb.headers.len().max(1) };
    let widths = std::iter::repeat(Constraint::Percentage((100 / col_count.max(1)) as u16)).take(col_count.max(1));
    let mut widget = Table::new(rows, widths);
    if let Some(hr) = header_row { widget = widget.header(hr); }
    if let Some(b) = &tb.block { widget = widget.block(b.clone()); }
    if let Some(sty) = &tb.row_highlight_style { widget = widget.row_highlight_style(sty.clone()); }
    if let Some(sym) = &tb.highlight_symbol { widget = widget.highlight_symbol(sym.clone()); }
    if let Some(sel) = tb.selected {
        let mut state = ratatui::widgets::TableState::default();
        state.select(Some(sel));
        ratatui::widgets::StatefulWidget::render(widget, area, &mut buf, &mut state);
    } else {
        ratatui::widgets::Widget::render(widget, area, &mut buf);
    }
    let mut s = String::new();
    for y in 0..height { for x in 0..width { let cell = &buf[(x, y)]; s.push_str(cell.symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

#[repr(u32)]
pub enum FfiWidgetKind { Paragraph = 1, List = 2, Table = 3, Gauge = 4, Tabs = 5, BarChart = 6, Sparkline = 7, Scrollbar = 8 }
// extend kinds for new widgets

#[repr(C)]
pub struct FfiDrawCmd { pub kind: u32, pub handle: *const (), pub rect: FfiRect }

fn render_cmd_to_buffer(cmd: &FfiDrawCmd, buf: &mut Buffer) {
    let area = Rect { x: cmd.rect.x, y: cmd.rect.y, width: cmd.rect.width, height: cmd.rect.height };
    match cmd.kind {
        x if x == FfiWidgetKind::Paragraph as u32 => {
            if cmd.handle.is_null() { return; }
            let p = unsafe { &*(cmd.handle as *const FfiParagraph) };
            let mut w = Paragraph::new(p.lines.clone());
            if let Some(b) = &p.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::List as u32 => {
            if cmd.handle.is_null() { return; }
            let l = unsafe { &*(cmd.handle as *const FfiList) };
            let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
            let mut w = List::new(items);
            if let Some(b) = &l.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Table as u32 => {
            if cmd.handle.is_null() { return; }
            let tb = unsafe { &*(cmd.handle as *const FfiTable) };
            let header_row = if tb.headers.is_empty() { None } else { Some(Row::new(tb.headers.iter().cloned().map(Cell::from).collect::<Vec<_>>())) };
            let rows: Vec<Row> = tb.rows.iter().map(|r| Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>())).collect();
            let col_count = if !tb.rows.is_empty() { tb.rows.iter().map(|r| r.len()).max().unwrap_or(1) } else { tb.headers.len().max(1) };
            let widths = std::iter::repeat(Constraint::Percentage((100 / col_count.max(1)) as u16)).take(col_count.max(1));
            let mut w = Table::new(rows, widths);
            if let Some(hr) = header_row { w = w.header(hr); }
            if let Some(b) = &tb.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Gauge as u32 => {
            if cmd.handle.is_null() { return; }
            let g = unsafe { &*(cmd.handle as *const FfiGauge) };
            let mut w = Gauge::default().ratio(g.ratio as f64);
            if let Some(label) = &g.label { w = w.label(label.clone()); }
            if let Some(b) = &g.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Tabs as u32 => {
            if cmd.handle.is_null() { return; }
            let t = unsafe { &*(cmd.handle as *const FfiTabs) };
            let titles: Vec<Line> = t.titles.iter().cloned().map(|s| Line::from(Span::raw(s))).collect();
            let mut w = Tabs::new(titles).select(t.selected as usize);
            if let Some(b) = &t.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::BarChart as u32 => {
            if cmd.handle.is_null() { return; }
            let bc = unsafe { &*(cmd.handle as *const FfiBarChart) };
            let area = area; // reuse
            let data: Vec<(&str, u64)> = bc.labels.iter().map(|s| s.as_str()).zip(bc.values.iter().cloned()).collect();
            let mut w = RtBarChart::default().data(&data);
            if let Some(b) = &bc.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Sparkline as u32 => {
            if cmd.handle.is_null() { return; }
            let sp = unsafe { &*(cmd.handle as *const FfiSparkline) };
            let mut w = RtSparkline::default().data(&sp.values);
            if let Some(b) = &sp.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Scrollbar as u32 => {
            if cmd.handle.is_null() { return; }
            let sc = unsafe { &*(cmd.handle as *const FfiScrollbar) };
            let mut state = RtScrollbarState::new(sc.content_len as usize).position(sc.position as usize);
            let orient = if sc.orient == FfiScrollbarOrient::Horizontal as u32 { RtScrollbarOrientation::HorizontalTop } else { RtScrollbarOrientation::VerticalRight };
            let w = RtScrollbar::new(orient);
            ratatui::widgets::StatefulWidget::render(w, area, buf, &mut state);
        }
        _ => {}
    }
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_frame(
    width: u16,
    height: u16,
    cmds: *const FfiDrawCmd,
    len: usize,
    out_text_utf8: *mut *mut c_char,
) -> bool {
    if cmds.is_null() || out_text_utf8.is_null() { return false; }
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let slice = unsafe { std::slice::from_raw_parts(cmds, len) };
    for cmd in slice.iter() { render_cmd_to_buffer(cmd, &mut buf); }
    let mut s = String::new();
    for y in 0..height { for x in 0..width { let cell = &buf[(x, y)]; s.push_str(cell.symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

// ----- Batched terminal frame drawing -----

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_frame(term: *mut FfiTerminal, cmds: *const FfiDrawCmd, len: usize) -> bool {
    if term.is_null() || cmds.is_null() { return false; }
    let t = unsafe { &mut *term };
    let slice = unsafe { std::slice::from_raw_parts(cmds, len) };
    let res = t.terminal.draw(|frame| {
        for cmd in slice.iter() {
            let area = Rect { x: cmd.rect.x, y: cmd.rect.y, width: cmd.rect.width, height: cmd.rect.height };
            match cmd.kind {
                x if x == FfiWidgetKind::Paragraph as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let p = unsafe { &*(cmd.handle as *const FfiParagraph) };
                    let mut w = Paragraph::new(p.lines.clone());
                    if let Some(b) = &p.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::List as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let l = unsafe { &*(cmd.handle as *const FfiList) };
                    let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
                    let mut w = List::new(items);
                    if let Some(b) = &l.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Table as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let tb = unsafe { &*(cmd.handle as *const FfiTable) };
                    let header_row = if tb.headers.is_empty() { None } else { Some(Row::new(tb.headers.iter().cloned().map(Cell::from).collect::<Vec<_>>())) };
                    let rows: Vec<Row> = tb.rows.iter().map(|r| Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>())).collect();
                    let col_count = if !tb.rows.is_empty() { tb.rows.iter().map(|r| r.len()).max().unwrap_or(1) } else { tb.headers.len().max(1) };
                    let widths = std::iter::repeat(Constraint::Percentage((100 / col_count.max(1)) as u16)).take(col_count.max(1));
                    let mut w = Table::new(rows, widths);
                    if let Some(hr) = header_row { w = w.header(hr); }
                    if let Some(b) = &tb.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Gauge as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let g = unsafe { &*(cmd.handle as *const FfiGauge) };
                    let mut w = Gauge::default().ratio(g.ratio as f64);
                    if let Some(label) = &g.label { w = w.label(label.clone()); }
                    if let Some(b) = &g.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Tabs as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let tbs = unsafe { &*(cmd.handle as *const FfiTabs) };
                    let titles: Vec<Line> = tbs.titles.iter().cloned().map(|s| Line::from(Span::raw(s))).collect();
                    let mut w = Tabs::new(titles).select(tbs.selected as usize);
                    if let Some(b) = &tbs.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::BarChart as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let bc = unsafe { &*(cmd.handle as *const FfiBarChart) };
                    let data: Vec<(&str, u64)> = bc.labels.iter().map(|s| s.as_str()).zip(bc.values.iter().cloned()).collect();
                    let mut w = RtBarChart::default().data(&data);
                    if let Some(b) = &bc.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Sparkline as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let sp = unsafe { &*(cmd.handle as *const FfiSparkline) };
                    let mut w = RtSparkline::default().data(&sp.values);
                    if let Some(b) = &sp.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Scrollbar as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let sc = unsafe { &*(cmd.handle as *const FfiScrollbar) };
                    let mut state = RtScrollbarState::new(sc.content_len as usize).position(sc.position as usize);
                    let orient = if sc.orient == FfiScrollbarOrient::Horizontal as u32 { RtScrollbarOrientation::HorizontalTop } else { RtScrollbarOrientation::VerticalRight };
                    let w = RtScrollbar::new(orient);
                    frame.render_stateful_widget(w, area, &mut state);
                }
                _ => {}
            }
        }
    });
    res.is_ok()
}

// ----- Event injection (for automation) -----

static INJECTED_EVENTS: Mutex<VecDeque<CtEvent>> = Mutex::new(VecDeque::new());

#[no_mangle]
pub extern "C" fn ratatui_inject_key(code: u32, ch: u32, mods: u8) {
    let ke = CtKeyEvent::new(
        match code {
            x if x == FfiKeyCode::Char as u32 => CtKeyCode::Char(char::from_u32(ch).unwrap_or('\0')),
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
            (if (mods & FfiKeyMods::SHIFT.bits()) != 0 { CtKeyModifiers::SHIFT.bits() } else { 0 }) |
            (if (mods & FfiKeyMods::ALT.bits()) != 0 { CtKeyModifiers::ALT.bits() } else { 0 }) |
            (if (mods & FfiKeyMods::CTRL.bits()) != 0 { CtKeyModifiers::CONTROL.bits() } else { 0 })
        ),
    );
    INJECTED_EVENTS.lock().unwrap().push_back(CtEvent::Key(ke));
}

#[no_mangle]
pub extern "C" fn ratatui_inject_resize(width: u16, height: u16) {
    INJECTED_EVENTS.lock().unwrap().push_back(CtEvent::Resize(width, height));
}

#[no_mangle]
pub extern "C" fn ratatui_inject_mouse(kind: u32, btn: u32, x: u16, y: u16, mods: u8) {
    let kind = match kind {
        x if x == FfiMouseKind::Down as u32 => CtMouseKind::Down(match btn { 1 => CtMouseButton::Left, 2 => CtMouseButton::Right, 3 => CtMouseButton::Middle, _ => CtMouseButton::Left }),
        x if x == FfiMouseKind::Up as u32 => CtMouseKind::Up(match btn { 1 => CtMouseButton::Left, 2 => CtMouseButton::Right, 3 => CtMouseButton::Middle, _ => CtMouseButton::Left }),
        x if x == FfiMouseKind::Drag as u32 => CtMouseKind::Drag(match btn { 1 => CtMouseButton::Left, 2 => CtMouseButton::Right, 3 => CtMouseButton::Middle, _ => CtMouseButton::Left }),
        x if x == FfiMouseKind::Moved as u32 => CtMouseKind::Moved,
        x if x == FfiMouseKind::ScrollUp as u32 => CtMouseKind::ScrollUp,
        x if x == FfiMouseKind::ScrollDown as u32 => CtMouseKind::ScrollDown,
        _ => CtMouseKind::Moved,
    };
    let modifiers = CtKeyModifiers::from_bits_truncate(
        (if (mods & FfiKeyMods::SHIFT.bits()) != 0 { CtKeyModifiers::SHIFT.bits() } else { 0 }) |
        (if (mods & FfiKeyMods::ALT.bits()) != 0 { CtKeyModifiers::ALT.bits() } else { 0 }) |
        (if (mods & FfiKeyMods::CTRL.bits()) != 0 { CtKeyModifiers::CONTROL.bits() } else { 0 })
    );
    INJECTED_EVENTS.lock().unwrap().push_back(CtEvent::Mouse(CtMouseEvent{ kind, column: x, row: y, modifiers }));
}

// ----- Simple List -----

#[no_mangle]
pub extern "C" fn ratatui_list_new() -> *mut FfiList {
    Box::into_raw(Box::new(FfiList { items: Vec::new(), block: None, selected: None, highlight_style: None, highlight_symbol: None }))
}

#[no_mangle]
pub extern "C" fn ratatui_list_free(lst: *mut FfiList) {
    if lst.is_null() { return; }
    unsafe { drop(Box::from_raw(lst)); }
}

#[no_mangle]
pub extern "C" fn ratatui_list_append_item(lst: *mut FfiList, text_utf8: *const c_char, style: FfiStyle) {
    if lst.is_null() || text_utf8.is_null() { return; }
    let l = unsafe { &mut *lst };
    let c_str = unsafe { CStr::from_ptr(text_utf8) };
    if let Ok(s) = c_str.to_str() {
        let st = style_from_ffi(style);
        l.items.push(Line::from(Span::styled(s.to_string(), st)));
    }
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_block_title(lst: *mut FfiList, title_utf8: *const c_char, show_border: bool) {
    if lst.is_null() { return; }
    let l = unsafe { &mut *lst };
    let mut block = if show_border { Block::default().borders(Borders::ALL) } else { Block::default() };
    if !title_utf8.is_null() {
        let c_str = unsafe { CStr::from_ptr(title_utf8) };
        if let Ok(title) = c_str.to_str() { block = block.title(title.to_string()); }
    }
    l.block = Some(block);
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_selected(lst: *mut FfiList, selected: i32) {
    if lst.is_null() { return; }
    let l = unsafe { &mut *lst };
    l.selected = if selected < 0 { None } else { Some(selected as usize) };
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_highlight_style(lst: *mut FfiList, style: FfiStyle) {
    if lst.is_null() { return; }
    let l = unsafe { &mut *lst };
    l.highlight_style = Some(style_from_ffi(style));
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_highlight_symbol(lst: *mut FfiList, sym_utf8: *const c_char) {
    if lst.is_null() { return; }
    let l = unsafe { &mut *lst };
    l.highlight_symbol = if sym_utf8.is_null() { None } else { unsafe { CStr::from_ptr(sym_utf8) }.to_str().ok().map(|s| s.to_string()) };
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_list_in(term: *mut FfiTerminal, lst: *const FfiList, rect: FfiRect) -> bool {
    if term.is_null() || lst.is_null() { return false; }
    let t = unsafe { &mut *term };
    let l = unsafe { &*lst };
    let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
    let mut widget = List::new(items);
    if let Some(b) = &l.block { widget = widget.block(b.clone()); }
    if let Some(sty) = &l.highlight_style { widget = widget.highlight_style(sty.clone()); }
    if let Some(sym) = &l.highlight_symbol { widget = widget.highlight_symbol(sym.clone()); }
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
}

// ----- Gauge -----

#[no_mangle]
pub extern "C" fn ratatui_gauge_new() -> *mut FfiGauge { Box::into_raw(Box::new(FfiGauge { ratio: 0.0, label: None, block: None })) }

#[no_mangle]
pub extern "C" fn ratatui_gauge_free(g: *mut FfiGauge) { if g.is_null() { return; } unsafe { drop(Box::from_raw(g)); } }

#[no_mangle]
pub extern "C" fn ratatui_gauge_set_ratio(g: *mut FfiGauge, ratio: f32) { if g.is_null() { return; } unsafe { (&mut *g).ratio = ratio.clamp(0.0, 1.0); } }

#[no_mangle]
pub extern "C" fn ratatui_gauge_set_label(g: *mut FfiGauge, label: *const c_char) {
    if g.is_null() { return; }
    let gg = unsafe { &mut *g };
    gg.label = if label.is_null() { None } else { unsafe { CStr::from_ptr(label) }.to_str().ok().map(|s| s.to_string()) };
}

#[no_mangle]
pub extern "C" fn ratatui_gauge_set_block_title(g: *mut FfiGauge, title_utf8: *const c_char, show_border: bool) {
    if g.is_null() { return; }
    let gg = unsafe { &mut *g };
    let mut block = if show_border { Block::default().borders(Borders::ALL) } else { Block::default() };
    if !title_utf8.is_null() {
        let c_str = unsafe { CStr::from_ptr(title_utf8) };
        if let Ok(title) = c_str.to_str() { block = block.title(title.to_string()); }
    }
    gg.block = Some(block);
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_gauge_in(term: *mut FfiTerminal, g: *const FfiGauge, rect: FfiRect) -> bool {
    if term.is_null() || g.is_null() { return false; }
    let t = unsafe { &mut *term };
    let gg = unsafe { &*g };
    let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    let mut widget = Gauge::default().ratio(gg.ratio as f64);
    if let Some(label) = &gg.label { widget = widget.label(label.clone()); }
    if let Some(b) = &gg.block { widget = widget.block(b.clone()); }
    let res = t.terminal.draw(|frame| { frame.render_widget(widget.clone(), area); });
    res.is_ok()
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_gauge(width: u16, height: u16, g: *const FfiGauge, out_text_utf8: *mut *mut c_char) -> bool {
    if g.is_null() || out_text_utf8.is_null() { return false; }
    let gg = unsafe { &*g };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let mut w = Gauge::default().ratio(gg.ratio as f64);
    if let Some(label) = &gg.label { w = w.label(label.clone()); }
    if let Some(b) = &gg.block { w = w.block(b.clone()); }
    ratatui::widgets::Widget::render(w, area, &mut buf);
    let mut s = String::new();
    for y in 0..height { for x in 0..width { let cell = &buf[(x, y)]; s.push_str(cell.symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

// ----- Tabs -----

#[no_mangle]
pub extern "C" fn ratatui_tabs_new() -> *mut FfiTabs { Box::into_raw(Box::new(FfiTabs { titles: Vec::new(), selected: 0, block: None })) }

#[no_mangle]
pub extern "C" fn ratatui_tabs_free(t: *mut FfiTabs) { if t.is_null() { return; } unsafe { drop(Box::from_raw(t)); } }

#[no_mangle]
pub extern "C" fn ratatui_tabs_set_titles(t: *mut FfiTabs, tsv_utf8: *const c_char) {
    if t.is_null() || tsv_utf8.is_null() { return; }
    let tt = unsafe { &mut *t };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() { tt.titles = s.split('\t').map(|x| x.to_string()).collect(); }
}

#[no_mangle]
pub extern "C" fn ratatui_tabs_set_selected(t: *mut FfiTabs, selected: u16) { if t.is_null() { return; } unsafe { (&mut *t).selected = selected; } }

#[no_mangle]
pub extern "C" fn ratatui_tabs_set_block_title(t: *mut FfiTabs, title_utf8: *const c_char, show_border: bool) {
    if t.is_null() { return; }
    let tt = unsafe { &mut *t };
    let mut block = if show_border { Block::default().borders(Borders::ALL) } else { Block::default() };
    if !title_utf8.is_null() {
        let c_str = unsafe { CStr::from_ptr(title_utf8) };
        if let Ok(title) = c_str.to_str() { block = block.title(title.to_string()); }
    }
    tt.block = Some(block);
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_tabs_in(term: *mut FfiTerminal, t: *const FfiTabs, rect: FfiRect) -> bool {
    if term.is_null() || t.is_null() { return false; }
    let termi = unsafe { &mut *term };
    let tabs = unsafe { &*t };
    let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    let titles: Vec<Line> = tabs.titles.iter().cloned().map(|s| Line::from(Span::raw(s))).collect();
    let mut widget = Tabs::new(titles).select(tabs.selected as usize);
    if let Some(b) = &tabs.block { widget = widget.block(b.clone()); }
    let res = termi.terminal.draw(|frame| { frame.render_widget(widget.clone(), area); });
    res.is_ok()
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_tabs(width: u16, height: u16, t: *const FfiTabs, out_text_utf8: *mut *mut c_char) -> bool {
    if t.is_null() || out_text_utf8.is_null() { return false; }
    let tabs = unsafe { &*t };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let titles: Vec<Line> = tabs.titles.iter().cloned().map(|s| Line::from(Span::raw(s))).collect();
    let mut widget = Tabs::new(titles).select(tabs.selected as usize);
    if let Some(b) = &tabs.block { widget = widget.block(b.clone()); }
    ratatui::widgets::Widget::render(widget, area, &mut buf);
    let mut s = String::new();
    for y in 0..height { for x in 0..width { let cell = &buf[(x, y)]; s.push_str(cell.symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

// ----- BarChart -----

#[no_mangle]
pub extern "C" fn ratatui_barchart_new() -> *mut FfiBarChart { Box::into_raw(Box::new(FfiBarChart { values: Vec::new(), labels: Vec::new(), block: None })) }

#[no_mangle]
pub extern "C" fn ratatui_barchart_free(b: *mut FfiBarChart) { if b.is_null() { return; } unsafe { drop(Box::from_raw(b)); } }

#[no_mangle]
pub extern "C" fn ratatui_barchart_set_values(b: *mut FfiBarChart, values: *const u64, len: usize) {
    if b.is_null() || values.is_null() { return; }
    let bc = unsafe { &mut *b };
    let slice = unsafe { std::slice::from_raw_parts(values, len) };
    bc.values = slice.to_vec();
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_set_labels(b: *mut FfiBarChart, tsv_utf8: *const c_char) {
    if b.is_null() || tsv_utf8.is_null() { return; }
    let bc = unsafe { &mut *b };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() { bc.labels = s.split('\t').map(|x| x.to_string()).collect(); }
}

#[no_mangle]
pub extern "C" fn ratatui_barchart_set_block_title(b: *mut FfiBarChart, title_utf8: *const c_char, show_border: bool) {
    if b.is_null() { return; }
    let bc = unsafe { &mut *b };
    let mut block = if show_border { Block::default().borders(Borders::ALL) } else { Block::default() };
    if !title_utf8.is_null() { if let Ok(title) = unsafe { CStr::from_ptr(title_utf8) }.to_str() { block = block.title(title.to_string()); }}
    bc.block = Some(block);
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_barchart_in(term: *mut FfiTerminal, b: *const FfiBarChart, rect: FfiRect) -> bool {
    if term.is_null() || b.is_null() { return false; }
    let t = unsafe { &mut *term };
    let bc = unsafe { &*b };
    let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    let data: Vec<(&str, u64)> = bc.labels.iter().map(|s| s.as_str()).zip(bc.values.iter().cloned()).collect();
    let mut w = RtBarChart::default().data(&data);
    if let Some(bl) = &bc.block { w = w.block(bl.clone()); }
    let res = t.terminal.draw(|frame| { frame.render_widget(w.clone(), area); });
    res.is_ok()
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_barchart(width: u16, height: u16, b: *const FfiBarChart, out_text_utf8: *mut *mut c_char) -> bool {
    if b.is_null() || out_text_utf8.is_null() { return false; }
    let bc = unsafe { &*b };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let data: Vec<(&str, u64)> = bc.labels.iter().map(|s| s.as_str()).zip(bc.values.iter().cloned()).collect();
    let mut w = RtBarChart::default().data(&data);
    if let Some(bl) = &bc.block { w = w.block(bl.clone()); }
    ratatui::widgets::Widget::render(w, area, &mut buf);
    let mut s = String::new();
    for y in 0..height { for x in 0..width { let cell = &buf[(x, y)]; s.push_str(cell.symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

// ----- Sparkline -----

#[no_mangle]
pub extern "C" fn ratatui_sparkline_new() -> *mut FfiSparkline { Box::into_raw(Box::new(FfiSparkline { values: Vec::new(), block: None })) }

#[no_mangle]
pub extern "C" fn ratatui_sparkline_free(s: *mut FfiSparkline) { if s.is_null() { return; } unsafe { drop(Box::from_raw(s)); } }

#[no_mangle]
pub extern "C" fn ratatui_sparkline_set_values(s: *mut FfiSparkline, values: *const u64, len: usize) {
    if s.is_null() || values.is_null() { return; }
    let sp = unsafe { &mut *s };
    let slice = unsafe { std::slice::from_raw_parts(values, len) };
    sp.values = slice.to_vec();
}

#[no_mangle]
pub extern "C" fn ratatui_sparkline_set_block_title(s: *mut FfiSparkline, title_utf8: *const c_char, show_border: bool) {
    if s.is_null() { return; }
    let sp = unsafe { &mut *s };
    let mut block = if show_border { Block::default().borders(Borders::ALL) } else { Block::default() };
    if !title_utf8.is_null() { if let Ok(title) = unsafe { CStr::from_ptr(title_utf8) }.to_str() { block = block.title(title.to_string()); }}
    sp.block = Some(block);
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_sparkline_in(term: *mut FfiTerminal, s: *const FfiSparkline, rect: FfiRect) -> bool {
    if term.is_null() || s.is_null() { return false; }
    let t = unsafe { &mut *term };
    let sp = unsafe { &*s };
    let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    let mut w = RtSparkline::default().data(&sp.values);
    if let Some(bl) = &sp.block { w = w.block(bl.clone()); }
    let res = t.terminal.draw(|frame| { frame.render_widget(w.clone(), area); });
    res.is_ok()
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_sparkline(width: u16, height: u16, s: *const FfiSparkline, out_text_utf8: *mut *mut c_char) -> bool {
    if s.is_null() || out_text_utf8.is_null() { return false; }
    let sp = unsafe { &*s };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let mut w = RtSparkline::default().data(&sp.values);
    if let Some(bl) = &sp.block { w = w.block(bl.clone()); }
    ratatui::widgets::Widget::render(w, area, &mut buf);
    let mut s = String::new();
    for y in 0..height { for x in 0..width { let cell = &buf[(x, y)]; s.push_str(cell.symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

// ----- Scrollbar -----

#[no_mangle]
pub extern "C" fn ratatui_scrollbar_new() -> *mut FfiScrollbar { Box::into_raw(Box::new(FfiScrollbar { orient: FfiScrollbarOrient::Vertical as u32, position: 0, content_len: 0, viewport_len: 0, block: None })) }

#[no_mangle]
pub extern "C" fn ratatui_scrollbar_free(s: *mut FfiScrollbar) { if s.is_null() { return; } unsafe { drop(Box::from_raw(s)); } }

#[no_mangle]
pub extern "C" fn ratatui_scrollbar_configure(s: *mut FfiScrollbar, orient: u32, position: u16, content_len: u16, viewport_len: u16) {
    if s.is_null() { return; }
    let sb = unsafe { &mut *s };
    sb.orient = orient; sb.position = position; sb.content_len = content_len; sb.viewport_len = viewport_len;
}

#[no_mangle]
pub extern "C" fn ratatui_scrollbar_set_block_title(s: *mut FfiScrollbar, title_utf8: *const c_char, show_border: bool) {
    if s.is_null() { return; }
    let sb = unsafe { &mut *s };
    let mut block = if show_border { Block::default().borders(Borders::ALL) } else { Block::default() };
    if !title_utf8.is_null() { if let Ok(title) = unsafe { CStr::from_ptr(title_utf8) }.to_str() { block = block.title(title.to_string()); }}
    sb.block = Some(block);
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_scrollbar_in(term: *mut FfiTerminal, s: *const FfiScrollbar, rect: FfiRect) -> bool {
    if term.is_null() || s.is_null() { return false; }
    let t = unsafe { &mut *term };
    let sb = unsafe { &*s };
    let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
    let orient = if sb.orient == FfiScrollbarOrient::Horizontal as u32 { RtScrollbarOrientation::HorizontalTop } else { RtScrollbarOrientation::VerticalRight };
    let mut state = RtScrollbarState::new(sb.content_len as usize).position(sb.position as usize).viewport_content_length(sb.viewport_len as usize);
    let mut w = RtScrollbar::new(orient);
    let res = t.terminal.draw(|frame| { frame.render_stateful_widget(w.clone(), area, &mut state); });
    res.is_ok()
}

#[no_mangle]
pub extern "C" fn ratatui_headless_render_scrollbar(width: u16, height: u16, s: *const FfiScrollbar, out_text_utf8: *mut *mut c_char) -> bool {
    if s.is_null() || out_text_utf8.is_null() { return false; }
    let sb = unsafe { &*s };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let orient = if sb.orient == FfiScrollbarOrient::Horizontal as u32 { RtScrollbarOrientation::HorizontalTop } else { RtScrollbarOrientation::VerticalRight };
    let mut state = RtScrollbarState::new(sb.content_len as usize).position(sb.position as usize).viewport_content_length(sb.viewport_len as usize);
    let mut w = RtScrollbar::new(orient);
    ratatui::widgets::StatefulWidget::render(w, area, &mut buf, &mut state);
    let mut s = String::new();
    for y in 0..height { for x in 0..width { let cell = &buf[(x, y)]; s.push_str(cell.symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

// ----- Simple Table (tab-separated cells) -----

#[repr(C)]
pub struct FfiTable { headers: Vec<String>, rows: Vec<Vec<String>>, block: Option<Block<'static>>, selected: Option<usize>, row_highlight_style: Option<Style>, highlight_symbol: Option<String> }

#[no_mangle]
pub extern "C" fn ratatui_table_new() -> *mut FfiTable {
    Box::into_raw(Box::new(FfiTable { headers: Vec::new(), rows: Vec::new(), block: None, selected: None, row_highlight_style: None, highlight_symbol: None }))
}

#[no_mangle]
pub extern "C" fn ratatui_table_free(tbl: *mut FfiTable) {
    if tbl.is_null() { return; }
    unsafe { drop(Box::from_raw(tbl)); }
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_headers(tbl: *mut FfiTable, tsv_utf8: *const c_char) {
    if tbl.is_null() || tsv_utf8.is_null() { return; }
    let t = unsafe { &mut *tbl };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() {
        t.headers = s.split('\t').map(|x| x.to_string()).collect();
    }
}

#[no_mangle]
pub extern "C" fn ratatui_table_append_row(tbl: *mut FfiTable, tsv_utf8: *const c_char) {
    if tbl.is_null() || tsv_utf8.is_null() { return; }
    let t = unsafe { &mut *tbl };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() {
        let row: Vec<String> = s.split('\t').map(|x| x.to_string()).collect();
        t.rows.push(row);
    }
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_block_title(tbl: *mut FfiTable, title_utf8: *const c_char, show_border: bool) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    let mut block = if show_border { Block::default().borders(Borders::ALL) } else { Block::default() };
    if !title_utf8.is_null() {
        let c_str = unsafe { CStr::from_ptr(title_utf8) };
        if let Ok(title) = c_str.to_str() { block = block.title(title.to_string()); }
    }
    t.block = Some(block);
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_selected(tbl: *mut FfiTable, selected: i32) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.selected = if selected < 0 { None } else { Some(selected as usize) };
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_row_highlight_style(tbl: *mut FfiTable, style: FfiStyle) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.row_highlight_style = Some(style_from_ffi(style));
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_highlight_symbol(tbl: *mut FfiTable, sym_utf8: *const c_char) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.highlight_symbol = if sym_utf8.is_null() { None } else { unsafe { CStr::from_ptr(sym_utf8) }.to_str().ok().map(|s| s.to_string()) };
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_table_in(term: *mut FfiTerminal, tbl: *const FfiTable, rect: FfiRect) -> bool {
    if term.is_null() || tbl.is_null() { return false; }
    let t = unsafe { &mut *term };
    let tb = unsafe { &*tbl };
    let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };

    let header_row = if tb.headers.is_empty() { None } else {
        let cells: Vec<Cell> = tb.headers.iter().map(|h| Cell::from(h.clone())).collect();
        Some(Row::new(cells))
    };
    let rows: Vec<Row> = tb.rows.iter().map(|r| Row::new(r.iter().map(|c| Cell::from(c.clone())).collect::<Vec<_>>())).collect();

    // Even column widths
    let col_count = if !tb.rows.is_empty() { tb.rows.iter().map(|r| r.len()).max().unwrap_or(1) } else { tb.headers.len().max(1) };
    let widths = std::iter::repeat(Constraint::Percentage( (100 / col_count.max(1)) as u16 )).take(col_count.max(1));

    let mut widget = Table::new(rows, widths);
    if let Some(hr) = header_row { widget = widget.header(hr); }
    if let Some(b) = &tb.block { widget = widget.block(b.clone()); }
    if let Some(sty) = &tb.row_highlight_style { widget = widget.row_highlight_style(sty.clone()); }
    if let Some(sym) = &tb.highlight_symbol { widget = widget.highlight_symbol(sym.clone()); }

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
    if out_width.is_null() || out_height.is_null() { return false; }
    match crossterm::terminal::size() {
        Ok((w,h)) => {
            unsafe { *out_width = w; *out_height = h; }
            true
        },
        Err(_) => false,
    }
}

#[no_mangle]
pub extern "C" fn ratatui_next_event(timeout_ms: u64, out_event: *mut FfiEvent) -> bool {
    if out_event.is_null() { return false; }
    let timeout = std::time::Duration::from_millis(timeout_ms);
    // Check injected events first
    if let Some(evt) = INJECTED_EVENTS.lock().unwrap().pop_front() {
        return fill_ffi_event(evt, out_event);
    }
    let has = match event::poll(timeout) { Ok(b) => b, Err(_) => return false };
    if !has { return false; }
    let evt = match event::read() { Ok(e) => e, Err(_) => return false };
    fill_ffi_event(evt, out_event)
}

fn ffi_key_from(k: CtKeyEvent) -> FfiKeyEvent {
    let mods = {
        let mut m = FfiKeyMods::NONE;
        if k.modifiers.contains(CtKeyModifiers::SHIFT) { m |= FfiKeyMods::SHIFT; }
        if k.modifiers.contains(CtKeyModifiers::ALT) { m |= FfiKeyMods::ALT; }
        if k.modifiers.contains(CtKeyModifiers::CONTROL) { m |= FfiKeyMods::CTRL; }
        m.bits()
    };
    match k.code {
        CtKeyCode::Char(c) => FfiKeyEvent { code: FfiKeyCode::Char as u32, ch: c as u32, mods },
        CtKeyCode::Enter => FfiKeyEvent { code: FfiKeyCode::Enter as u32, ch: 0, mods },
        CtKeyCode::Left => FfiKeyEvent { code: FfiKeyCode::Left as u32, ch: 0, mods },
        CtKeyCode::Right => FfiKeyEvent { code: FfiKeyCode::Right as u32, ch: 0, mods },
        CtKeyCode::Up => FfiKeyEvent { code: FfiKeyCode::Up as u32, ch: 0, mods },
        CtKeyCode::Down => FfiKeyEvent { code: FfiKeyCode::Down as u32, ch: 0, mods },
        CtKeyCode::Esc => FfiKeyEvent { code: FfiKeyCode::Esc as u32, ch: 0, mods },
        CtKeyCode::Backspace => FfiKeyEvent { code: FfiKeyCode::Backspace as u32, ch: 0, mods },
        CtKeyCode::Tab => FfiKeyEvent { code: FfiKeyCode::Tab as u32, ch: 0, mods },
        CtKeyCode::Delete => FfiKeyEvent { code: FfiKeyCode::Delete as u32, ch: 0, mods },
        CtKeyCode::Home => FfiKeyEvent { code: FfiKeyCode::Home as u32, ch: 0, mods },
        CtKeyCode::End => FfiKeyEvent { code: FfiKeyCode::End as u32, ch: 0, mods },
        CtKeyCode::PageUp => FfiKeyEvent { code: FfiKeyCode::PageUp as u32, ch: 0, mods },
        CtKeyCode::PageDown => FfiKeyEvent { code: FfiKeyCode::PageDown as u32, ch: 0, mods },
        CtKeyCode::Insert => FfiKeyEvent { code: FfiKeyCode::Insert as u32, ch: 0, mods },
        CtKeyCode::F(n) => {
            let base = FfiKeyCode::F1 as u32;
            let code = base + (n.saturating_sub(1) as u32);
            FfiKeyEvent { code, ch: 0, mods }
        }
        _ => FfiKeyEvent { code: 0, ch: 0, mods },
    }
}

fn fill_ffi_event(evt: CtEvent, out_event: *mut FfiEvent) -> bool {
    let mut out = FfiEvent { kind: FfiEventKind::None as u32, key: FfiKeyEvent { code: 0, ch: 0, mods: 0 }, width: 0, height: 0, mouse_x: 0, mouse_y: 0, mouse_kind: 0, mouse_btn: 0, mouse_mods: 0 };
    match evt {
        CtEvent::Key(k) => { out.kind = FfiEventKind::Key as u32; out.key = ffi_key_from(k); },
        CtEvent::Resize(w,h) => { out.kind = FfiEventKind::Resize as u32; out.width = w; out.height = h; },
        CtEvent::Mouse(m) => {
            out.kind = FfiEventKind::Mouse as u32;
            match m.kind {
                CtMouseKind::Down(btn) => { out.mouse_kind = FfiMouseKind::Down as u32; out.mouse_btn = ffi_mouse_btn(btn); }
                CtMouseKind::Up(btn) => { out.mouse_kind = FfiMouseKind::Up as u32; out.mouse_btn = ffi_mouse_btn(btn); }
                CtMouseKind::Drag(btn) => { out.mouse_kind = FfiMouseKind::Drag as u32; out.mouse_btn = ffi_mouse_btn(btn); }
                CtMouseKind::Moved => { out.mouse_kind = FfiMouseKind::Moved as u32; }
                CtMouseKind::ScrollUp => { out.mouse_kind = FfiMouseKind::ScrollUp as u32; }
                CtMouseKind::ScrollDown => { out.mouse_kind = FfiMouseKind::ScrollDown as u32; }
                _ => {}
            }
            out.mouse_x = m.column;
            out.mouse_y = m.row;
            out.mouse_mods = ffi_mods_to_u8(m.modifiers);
        }
        _ => {}
    }
    unsafe { *out_event = out; }
    true
}

fn ffi_mouse_btn(b: CtMouseButton) -> u32 {
    match b { CtMouseButton::Left => FfiMouseButton::Left as u32, CtMouseButton::Right => FfiMouseButton::Right as u32, CtMouseButton::Middle => FfiMouseButton::Middle as u32 }
}

fn ffi_mods_to_u8(m: CtKeyModifiers) -> u8 {
    let mut out = 0u8;
    if m.contains(CtKeyModifiers::SHIFT) { out |= FfiKeyMods::SHIFT.bits(); }
    if m.contains(CtKeyModifiers::ALT) { out |= FfiKeyMods::ALT.bits(); }
    if m.contains(CtKeyModifiers::CONTROL) { out |= FfiKeyMods::CTRL.bits(); }
    out
}
