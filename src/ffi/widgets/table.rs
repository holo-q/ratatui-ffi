use crate::*;
use std::ffi::CString;

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
    let header_row = if let Some(hs) = &tb.headers_spans {
        let mut r = Row::new(hs.iter().cloned().map(Cell::from).collect::<Vec<_>>());
        if let Some(hsty) = &tb.header_style { r = r.style(hsty.clone()); }
        Some(r)
    } else if tb.headers.is_empty() {
        None
    } else {
        Some(Row::new(tb.headers.iter().cloned().map(Cell::from).collect::<Vec<_>>()))
    };
    let rows: Vec<Row> = if let Some(rows_cells) = &tb.rows_cells_lines {
        rows_cells.iter().map(|cells| {
            let mut row_cells: Vec<Cell> = Vec::with_capacity(cells.len());
            for cell_lines in cells.iter() {
                let text = ratatui::text::Text::from(cell_lines.clone());
                row_cells.push(Cell::from(text));
            }
            let mut row = Row::new(row_cells);
            if let Some(h) = tb.row_height { row = row.height(h); }
            row
        }).collect()
    } else if let Some(rss) = &tb.rows_spans {
        rss.iter().map(|r| {
            let mut row = Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>());
            if let Some(h) = tb.row_height { row = row.height(h); }
            row
        }).collect()
    } else {
        tb.rows.iter().map(|r| {
            let mut row = Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>());
            if let Some(h) = tb.row_height { row = row.height(h); }
            row
        }).collect()
    };
    let col_count = if let Some(w) = &tb.widths_pct {
        w.len().max(1)
    } else if !tb.rows.is_empty() {
        tb.rows.iter().map(|r| r.len()).max().unwrap_or(1)
    } else { tb.headers.len().max(1) };
    let widths: Vec<Constraint> = if let Some(ws) = &tb.widths_pct {
        ws.iter().map(|p| Constraint::Percentage(*p)).collect()
    } else {
        std::iter::repeat(Constraint::Percentage((100 / col_count.max(1)) as u16)).take(col_count.max(1)).collect()
    };
    let mut widget = Table::new(rows, widths);
    if let Some(cs) = tb.column_spacing { widget = widget.column_spacing(cs); }
    if let Some(hr) = header_row { widget = widget.header(hr); }
    if let Some(b) = &tb.block { widget = widget.block(b.clone()); }
    if let Some(sty) = &tb.row_highlight_style { widget = widget.row_highlight_style(sty.clone()); }
    if let Some(sym) = &tb.highlight_symbol { widget = widget.highlight_symbol(sym.clone()); }
    if let Some(sty) = &tb.column_highlight_style { widget = widget.column_highlight_style(sty.clone()); }
    if let Some(sty) = &tb.cell_highlight_style { widget = widget.cell_highlight_style(sty.clone()); }
    if let Some(sp) = &tb.highlight_spacing { widget = widget.highlight_spacing(sp.clone()); }
    if let Some(sel) = tb.selected {
        let mut state = ratatui::widgets::TableState::default();
        state.select(Some(sel));
        ratatui::widgets::StatefulWidget::render(widget, area, &mut buf, &mut state);
    } else {
        ratatui::widgets::Widget::render(widget, area, &mut buf);
    }
    let mut s = String::new();
    for y in 0..height { for x in 0..width { s.push_str(buf[(x, y)].symbol()); } if y + 1 < height { s.push('\n'); } }
    match CString::new(s) { Ok(cstr) => { unsafe { *out_text_utf8 = cstr.into_raw(); } true }, Err(_) => false }
}

