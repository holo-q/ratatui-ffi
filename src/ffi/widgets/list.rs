use crate::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

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
    if let Some(d) = l.direction { widget = widget.direction(d); }
    if let Some(b) = &l.block { widget = widget.block(b.clone()); }
    if let Some(sty) = &l.highlight_style { widget = widget.highlight_style(sty.clone()); }
    if let Some(sym) = &l.highlight_symbol { widget = widget.highlight_symbol(sym.as_str()); }
    if let Some(sp) = &l.highlight_spacing { widget = widget.highlight_spacing(sp.clone()); }
    if l.selected.is_some() || l.scroll_offset.is_some() {
        let mut state = ratatui::widgets::ListState::default();
        if let Some(sel) = l.selected { state.select(Some(sel)); }
        if let Some(off) = l.scroll_offset { state = state.with_offset(off); }
        ratatui::widgets::StatefulWidget::render(widget, area, &mut buf, &mut state);
    } else {
        ratatui::widgets::Widget::render(widget, area, &mut buf);
    }
    let mut s = String::new();
    for y in 0..height { for x in 0..width { s.push_str(buf[(x, y)].symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

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
    if lst.is_null() { return; }
    unsafe { drop(Box::from_raw(lst)); }
}

// ListState FFI
#[no_mangle]
pub extern "C" fn ratatui_list_state_new() -> *mut FfiListState {
    Box::into_raw(Box::new(FfiListState { selected: None, offset: 0 }))
}

#[no_mangle]
pub extern "C" fn ratatui_list_state_free(st: *mut FfiListState) {
    if st.is_null() { return; }
    unsafe { drop(Box::from_raw(st)); }
}

crate::ratatui_set_selected_i32_fn!(ratatui_list_state_set_selected, FfiListState, selected);

#[no_mangle]
pub extern "C" fn ratatui_list_state_set_offset(st: *mut FfiListState, offset: usize) {
    if st.is_null() { return; }
    unsafe { (&mut *st).offset = offset; }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_list_state_in(
    term: *mut FfiTerminal,
    lst: *const FfiList,
    rect: FfiRect,
    st: *const FfiListState,
) -> bool {
    crate::guard_bool("ratatui_terminal_draw_list_state_in", || {
        if term.is_null() || lst.is_null() || st.is_null() { return false; }
        let t = unsafe { &mut *term };
        let l = unsafe { &*lst };
        let s = unsafe { &*st };
        let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
        let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
        let mut widget = List::new(items);
        if let Some(d) = l.direction { widget = widget.direction(d); }
        if let Some(b) = &l.block { widget = widget.block(b.clone()); }
        if let Some(sty) = &l.highlight_style { widget = widget.highlight_style(sty.clone()); }
        if let Some(sym) = &l.highlight_symbol { widget = widget.highlight_symbol(sym.as_str()); }
        if let Some(sp) = &l.highlight_spacing { widget = widget.highlight_spacing(sp.clone()); }
        let mut state = ratatui::widgets::ListState::default();
        if let Some(sel) = s.selected { state.select(Some(sel)); }
        state = state.with_offset(s.offset);
        let res = t.terminal.draw(|frame| { frame.render_stateful_widget(widget.clone(), area, &mut state); });
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
    if lst.is_null() || st.is_null() || out_text_utf8.is_null() { return false; }
    let l = unsafe { &*lst };
    let s = unsafe { &*st };
    let area = Rect { x: 0, y: 0, width, height };
    let mut buf = Buffer::empty(area);
    let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
    let mut widget = List::new(items);
    if let Some(d) = l.direction { widget = widget.direction(d); }
    if let Some(b) = &l.block { widget = widget.block(b.clone()); }
    if let Some(sty) = &l.highlight_style { widget = widget.highlight_style(sty.clone()); }
    if let Some(sym) = &l.highlight_symbol { widget = widget.highlight_symbol(sym.as_str()); }
    if let Some(sp) = &l.highlight_spacing { widget = widget.highlight_spacing(sp.clone()); }
    let mut state = ratatui::widgets::ListState::default();
    if let Some(sel) = s.selected { state.select(Some(sel)); }
    state = state.with_offset(s.offset);
    ratatui::widgets::StatefulWidget::render(widget, area, &mut buf, &mut state);
    let mut s = String::new();
    for y in 0..height { for x in 0..width { s.push_str(buf[(x, y)].symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
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
pub extern "C" fn ratatui_list_append_item_spans(lst: *mut FfiList, spans: *const FfiSpan, len: usize) {
    if lst.is_null() || spans.is_null() { return; }
    let l = unsafe { &mut *lst };
    if let Some(sp) = spans_from_ffi(spans, len) { l.items.push(Line::from(sp)); }
}

#[no_mangle]
pub extern "C" fn ratatui_list_append_items_spans(lst: *mut FfiList, items: *const FfiLineSpans, len: usize) {
    if lst.is_null() || items.is_null() || len == 0 { return; }
    let l = unsafe { &mut *lst };
    let slice = unsafe { std::slice::from_raw_parts(items, len) };
    for it in slice.iter() {
        if it.spans.is_null() || it.len == 0 { l.items.push(Line::default()); continue; }
        if let Some(sp) = spans_from_ffi(it.spans, it.len) { l.items.push(Line::from(sp)); }
        else { l.items.push(Line::default()); }
    }
}

crate::ratatui_block_title_fn!(ratatui_list_set_block_title, FfiList);
crate::ratatui_block_title_spans_fn!(ratatui_list_set_block_title_spans, FfiList);
crate::ratatui_block_adv_fn!(ratatui_list_set_block_adv, FfiList);
crate::ratatui_block_title_alignment_fn!(ratatui_list_set_block_title_alignment, FfiList);
crate::ratatui_set_selected_i32_fn!(ratatui_list_set_selected, FfiList, selected);
crate::ratatui_set_style_fn!(ratatui_list_set_highlight_style, FfiList, highlight_style);

#[no_mangle]
pub extern "C" fn ratatui_list_set_highlight_symbol(lst: *mut FfiList, sym_utf8: *const c_char) {
    if lst.is_null() { return; }
    let l = unsafe { &mut *lst };
    l.highlight_symbol = if sym_utf8.is_null() { None } else { unsafe { CStr::from_ptr(sym_utf8) }.to_str().ok().map(|s| s.to_string()) };
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_direction(lst: *mut FfiList, dir: u32) {
    if lst.is_null() { return; }
    let l = unsafe { &mut *lst };
    l.direction = Some(match dir { 1 => RtListDirection::BottomToTop, _ => RtListDirection::TopToBottom });
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_scroll_offset(lst: *mut FfiList, offset: usize) {
    if lst.is_null() { return; }
    let l = unsafe { &mut *lst };
    l.scroll_offset = Some(offset);
}

#[no_mangle]
pub extern "C" fn ratatui_list_set_highlight_spacing(lst: *mut FfiList, spacing: u32) {
    if lst.is_null() { return; }
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
    crate::guard_bool("ratatui_terminal_draw_list_in", || {
        if term.is_null() || lst.is_null() { return false; }
        let t = unsafe { &mut *term };
        let l = unsafe { &*lst };
        let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
        let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
        let mut widget = List::new(items);
        if let Some(b) = &l.block { widget = widget.block(b.clone()); }
        if let Some(sty) = &l.highlight_style { widget = widget.highlight_style(sty.clone()); }
        if let Some(sym) = &l.highlight_symbol { widget = widget.highlight_symbol(sym.as_str()); }
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

crate::ratatui_reserve_vec_fn!(ratatui_list_reserve_items, FfiList, items);

