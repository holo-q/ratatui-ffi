use crate::*;
use std::ffi::CString;

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

