use crate::*;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

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
    if tbl.is_null() { return; }
    unsafe { drop(Box::from_raw(tbl)); }
}

#[no_mangle]
pub extern "C" fn ratatui_table_state_new() -> *mut FfiTableState {
    Box::into_raw(Box::new(FfiTableState { selected: None, offset: 0 }))
}

#[no_mangle]
pub extern "C" fn ratatui_table_state_free(st: *mut FfiTableState) {
    if st.is_null() { return; }
    unsafe { drop(Box::from_raw(st)); }
}

crate::ratatui_set_selected_i32_fn!(ratatui_table_state_set_selected, FfiTableState, selected);

#[no_mangle]
pub extern "C" fn ratatui_table_state_set_offset(st: *mut FfiTableState, offset: usize) {
    if st.is_null() { return; }
    unsafe { (&mut *st).offset = offset; }
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_table_state_in(
    term: *mut FfiTerminal,
    tbl: *const FfiTable,
    rect: FfiRect,
    st: *const FfiTableState,
) -> bool {
    crate::guard_bool("ratatui_terminal_draw_table_state_in", || {
        if term.is_null() || tbl.is_null() || st.is_null() { return false; }
        let t = unsafe { &mut *term };
        let tb = unsafe { &*tbl };
        let ss = unsafe { &*st };
        let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
        let header_row = if let Some(hs) = &tb.headers_spans {
            let mut r = Row::new(hs.iter().cloned().map(Cell::from).collect::<Vec<_>>());
            if let Some(hsty) = &tb.header_style { r = r.style(hsty.clone()); }
            Some(r)
        } else if tb.headers.is_empty() { None } else {
            Some(Row::new(tb.headers.iter().cloned().map(Cell::from).collect::<Vec<_>>()))
        };
        let rows: Vec<Row> = if let Some(rows_cells) = &tb.rows_cells_lines {
            rows_cells.iter().map(|cells| {
                let mut rc: Vec<Cell> = Vec::with_capacity(cells.len());
                for cell_lines in cells.iter() {
                    let text = ratatui::text::Text::from(cell_lines.clone());
                    rc.push(Cell::from(text));
                }
                let mut row = Row::new(rc);
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
        let col_count = if let Some(w) = &tb.widths_pct { w.len().max(1) }
                        else if !tb.rows.is_empty() { tb.rows.iter().map(|r| r.len()).max().unwrap_or(1) }
                        else { tb.headers.len().max(1) };
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
        let mut state = ratatui::widgets::TableState::default();
        if let Some(sel) = ss.selected { state.select(Some(sel)); }
        state = state.with_offset(ss.offset);
        let res = t.terminal.draw(|frame| { frame.render_stateful_widget(widget.clone(), area, &mut state); });
        res.is_ok()
    })
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_headers(tbl: *mut FfiTable, tsv_utf8: *const c_char) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    if tsv_utf8.is_null() { t.headers.clear(); return; }
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() { t.headers = s.split('\t').map(|x| x.to_string()).collect(); }
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_headers_spans(tbl: *mut FfiTable, spans: *const FfiSpan, len: usize) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    if spans.is_null() || len == 0 { t.headers_spans = None; return; }
    if let Some(sp) = spans_from_ffi(spans, len) { t.headers_spans = Some(vec![Line::from(sp)]); }
}

#[no_mangle]
pub extern "C" fn ratatui_table_append_row(tbl: *mut FfiTable, tsv_utf8: *const c_char) {
    if tbl.is_null() || tsv_utf8.is_null() { return; }
    let t = unsafe { &mut *tbl };
    let c_str = unsafe { CStr::from_ptr(tsv_utf8) };
    if let Ok(s) = c_str.to_str() { t.rows.push(s.split('\t').map(|x| x.to_string()).collect()); }
}

#[no_mangle]
pub extern "C" fn ratatui_table_append_row_spans(tbl: *mut FfiTable, spans: *const FfiSpan, len: usize) {
    if tbl.is_null() || spans.is_null() || len == 0 { return; }
    let t = unsafe { &mut *tbl };
    if let Some(sp) = spans_from_ffi(spans, len) { t.rows_spans.get_or_insert(Vec::new()).push(vec![Line::from(sp)]); }
}

#[no_mangle]
pub extern "C" fn ratatui_table_append_row_cells_lines(tbl: *mut FfiTable, cells: *const FfiLineSpans, len: usize) {
    if tbl.is_null() || cells.is_null() || len == 0 { return; }
    let t = unsafe { &mut *tbl };
    let slice = unsafe { std::slice::from_raw_parts(cells, len) };
    let mut row: Vec<Vec<Line<'static>>> = Vec::with_capacity(len);
    for ls in slice.iter() {
        let mut lines = Vec::new();
        if ls.spans.is_null() || ls.len == 0 { lines.push(Line::default()); }
        else if let Some(sp) = spans_from_ffi(ls.spans, ls.len) { lines.push(Line::from(sp)); }
        row.push(lines);
    }
    if t.rows_cells_lines.is_none() { t.rows_cells_lines = Some(Vec::new()); }
    t.rows_cells_lines.as_mut().unwrap().push(row);
}

crate::ratatui_block_title_fn!(ratatui_table_set_block_title, FfiTable);
crate::ratatui_block_title_spans_fn!(ratatui_table_set_block_title_spans, FfiTable);
crate::ratatui_block_adv_fn!(ratatui_table_set_block_adv, FfiTable);
crate::ratatui_block_title_alignment_fn!(ratatui_table_set_block_title_alignment, FfiTable);
crate::ratatui_set_selected_i32_fn!(ratatui_table_set_selected, FfiTable, selected);
crate::ratatui_set_style_fn!(ratatui_table_set_row_highlight_style, FfiTable, row_highlight_style);

#[no_mangle]
pub extern "C" fn ratatui_table_set_highlight_symbol(tbl: *mut FfiTable, sym_utf8: *const c_char) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.highlight_symbol = if sym_utf8.is_null() { None } else { unsafe { CStr::from_ptr(sym_utf8) }.to_str().ok().map(|s| s.to_string()) };
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_column_highlight_style(tbl: *mut FfiTable, style: FfiStyle) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.column_highlight_style = Some(style_from_ffi(style));
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_cell_highlight_style(tbl: *mut FfiTable, style: FfiStyle) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.cell_highlight_style = Some(style_from_ffi(style));
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_highlight_spacing(tbl: *mut FfiTable, spacing: u32) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.highlight_spacing = Some(match spacing { 1 => ratatui::widgets::HighlightSpacing::Never, 2 => ratatui::widgets::HighlightSpacing::WhenSelected, _ => ratatui::widgets::HighlightSpacing::Always });
}

crate::ratatui_set_style_fn!(ratatui_table_set_header_style, FfiTable, header_style);

#[no_mangle]
pub extern "C" fn ratatui_table_set_row_height(tbl: *mut FfiTable, height: u16) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.row_height = Some(height);
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_column_spacing(tbl: *mut FfiTable, spacing: u16) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    t.column_spacing = Some(spacing);
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_widths_percentages(tbl: *mut FfiTable, widths: *const u16, len: usize) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    if widths.is_null() || len == 0 { t.widths_pct = None; return; }
    let slice = unsafe { std::slice::from_raw_parts(widths, len) };
    t.widths_pct = Some(slice.to_vec());
}

#[no_mangle]
pub extern "C" fn ratatui_table_set_widths(tbl: *mut FfiTable, kinds: *const u32, vals: *const u16, len: usize) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    if kinds.is_null() || vals.is_null() || len == 0 { t.widths_pct = None; return; }
    let ks = unsafe { std::slice::from_raw_parts(kinds, len) };
    let vs = unsafe { std::slice::from_raw_parts(vals, len) };
    let all_pct = ks.iter().all(|&k| k == 1);
    if all_pct { t.widths_pct = Some(vs.to_vec()); t.widths_constraints = None; return; }
    // fallback: approximate to percentages
    let sum: u32 = vs.iter().map(|&v| v as u32).sum();
    if sum == 0 { t.widths_pct = None; return; }
    let out: Vec<u16> = vs.iter().map(|&v| (((v as u32) * 100) / sum) as u16).collect();
    t.widths_pct = Some(out);
}

#[no_mangle]
pub extern "C" fn ratatui_terminal_draw_table_in(term: *mut FfiTerminal, tbl: *const FfiTable, rect: FfiRect) -> bool {
    crate::guard_bool("ratatui_terminal_draw_table_in", || {
        if term.is_null() || tbl.is_null() { return false; }
        let t = unsafe { &mut *term };
        let tb = unsafe { &*tbl };
        let area = Rect { x: rect.x, y: rect.y, width: rect.width, height: rect.height };
        // build rows via headless logic
        let header_row = if let Some(hs) = &tb.headers_spans { Some(Row::new(hs.iter().cloned().map(Cell::from).collect::<Vec<_>>())) }
                         else if tb.headers.is_empty() { None } else { Some(Row::new(tb.headers.iter().cloned().map(Cell::from).collect::<Vec<_>>())) };
        let rows: Vec<Row> = if let Some(rows_cells) = &tb.rows_cells_lines {
            rows_cells.iter().map(|cells| {
                let mut rc: Vec<Cell> = Vec::with_capacity(cells.len());
                for cell_lines in cells.iter() { let text = ratatui::text::Text::from(cell_lines.clone()); rc.push(Cell::from(text)); }
                Row::new(rc)
            }).collect()
        } else if let Some(rss) = &tb.rows_spans {
            rss.iter().map(|r| Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>())).collect()
        } else {
            tb.rows.iter().map(|r| Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>())).collect()
        };
        let col_count = if let Some(w) = &tb.widths_pct { w.len().max(1) }
                        else if !tb.rows.is_empty() { tb.rows.iter().map(|r| r.len()).max().unwrap_or(1) }
                        else { tb.headers.len().max(1) };
        let widths: Vec<Constraint> = if let Some(ws) = &tb.widths_pct { ws.iter().map(|p| Constraint::Percentage(*p)).collect() }
                                      else { std::iter::repeat(Constraint::Percentage((100 / col_count.max(1)) as u16)).take(col_count.max(1)).collect() };
        let mut widget = Table::new(rows, widths);
        if let Some(cs) = tb.column_spacing { widget = widget.column_spacing(cs); }
        if let Some(hr) = header_row { widget = widget.header(hr); }
        if let Some(b) = &tb.block { widget = widget.block(b.clone()); }
        if let Some(sty) = &tb.row_highlight_style { widget = widget.row_highlight_style(sty.clone()); }
        if let Some(sym) = &tb.highlight_symbol { widget = widget.highlight_symbol(sym.clone()); }
        if let Some(sty) = &tb.column_highlight_style { widget = widget.column_highlight_style(sty.clone()); }
        if let Some(sty) = &tb.cell_highlight_style { widget = widget.cell_highlight_style(sty.clone()); }
        if let Some(sp) = &tb.highlight_spacing { widget = widget.highlight_spacing(sp.clone()); }
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
    })
}

// removed duplicate ratatui_table_append_rows_cells_lines (old signature)

#[no_mangle]
pub extern "C" fn ratatui_table_reserve_rows(tbl: *mut FfiTable, additional: usize) {
    if tbl.is_null() { return; }
    let t = unsafe { &mut *tbl };
    if let Some(rr) = &mut t.rows_cells_lines { rr.reserve(additional); }
    else if let Some(rs) = &mut t.rows_spans { rs.reserve(additional); }
    else { t.rows.reserve(additional); }
}

#[repr(C)]
pub struct FfiRowCellsLines { pub cells: *const FfiCellLines, pub len: usize }

#[no_mangle]
pub extern "C" fn ratatui_table_append_rows_cells_lines(
    tbl: *mut FfiTable,
    rows: *const FfiRowCellsLines,
    row_count: usize,
) {
    if tbl.is_null() || rows.is_null() || row_count == 0 { return; }
    let t = unsafe { &mut *tbl };
    let rows_slice = unsafe { std::slice::from_raw_parts(rows, row_count) };
    for r in rows_slice.iter() {
        if r.cells.is_null() || r.len == 0 { continue; }
        let cells_slice = unsafe { std::slice::from_raw_parts(r.cells, r.len) };
        let mut row: Vec<Vec<Line<'static>>> = Vec::with_capacity(cells_slice.len());
        for cell in cells_slice.iter() {
            if cell.lines.is_null() || cell.len == 0 { row.push(Vec::new()); continue; }
            let line_specs = unsafe { std::slice::from_raw_parts(cell.lines, cell.len) };
            let mut lines: Vec<Line<'static>> = Vec::with_capacity(cell.len);
            for ls in line_specs.iter() {
                if ls.spans.is_null() || ls.len == 0 { lines.push(Line::default()); continue; }
                if let Some(sp) = spans_from_ffi(ls.spans, ls.len) { lines.push(Line::from(sp)); }
                else { lines.push(Line::default()); }
            }
            row.push(lines);
        }
        if t.rows_cells_lines.is_none() { t.rows_cells_lines = Some(Vec::new()); }
        t.rows_cells_lines.as_mut().unwrap().push(row);
    }
}
