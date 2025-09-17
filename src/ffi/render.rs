use crate::*;
use ratatui::widgets::{Clear as RtClear, Gauge, List, ListItem, Paragraph, Tabs};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::{Line, Span};
use ratatui::widgets::canvas::{Canvas as RtCanvas, Line as RtCanvasLine, Points as RtCanvasPoints, Rectangle as RtCanvasRect};
use ratatui::widgets::{Table, Row, Cell};
use ratatui::widgets::{Scrollbar as RtScrollbar, ScrollbarOrientation as RtScrollbarOrientation, ScrollbarState as RtScrollbarState};
use ratatui::widgets::{BarChart as RtBarChart};
use ratatui::widgets::{Chart as RtChart, Dataset as RtDataset, Axis as RtAxis, GraphType as RtGraphType, LegendPosition as RtLegendPosition};

pub fn draw_frame(term: &mut FfiTerminal, slice: &[FfiDrawCmd]) -> bool {
    let res = term.terminal.draw(|frame| {
        let full = frame.area();
        for cmd in slice.iter() {
            let x = cmd.rect.x.min(full.width.saturating_sub(1));
            let y = cmd.rect.y.min(full.height.saturating_sub(1));
            let max_w = full.width.saturating_sub(x);
            let max_h = full.height.saturating_sub(y);
            let w = cmd.rect.width.min(max_w);
            let h = cmd.rect.height.min(max_h);
            if w == 0 || h == 0 { continue; }
            let area = Rect { x, y, width: w, height: h };
            match cmd.kind {
                x if x == FfiWidgetKind::Paragraph as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(p) = crate::ptr_checked(cmd.handle as *const FfiParagraph, "draw_frame:Paragraph") else { continue; };
                    let mut w = Paragraph::new(p.lines.clone());
                    if let Some(b) = &p.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::List as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(l) = crate::ptr_checked(cmd.handle as *const FfiList, "draw_frame:List") else { continue; };
                    let items: Vec<ListItem> = l.items.iter().cloned().map(ListItem::new).collect();
                    let mut w = List::new(items);
                    if let Some(d) = l.direction { w = w.direction(d); }
                    if let Some(b) = &l.block { w = w.block(b.clone()); }
                    if let Some(sp) = &l.highlight_spacing { w = w.highlight_spacing(sp.clone()); }
                    if l.selected.is_some() || l.scroll_offset.is_some() {
                        let mut state = ratatui::widgets::ListState::default();
                        if let Some(sel) = l.selected { state.select(Some(sel)); }
                        if let Some(off) = l.scroll_offset { state = state.with_offset(off); }
                        frame.render_stateful_widget(w, area, &mut state);
                    } else {
                        frame.render_widget(w, area);
                    }
                }
                x if x == FfiWidgetKind::Table as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(tb) = crate::ptr_checked(cmd.handle as *const FfiTable, "draw_frame:Table") else { continue; };
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
                    frame.render_widget(widget, area);
                }
                x if x == FfiWidgetKind::Gauge as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(g) = crate::ptr_checked(cmd.handle as *const FfiGauge, "draw_frame:Gauge") else { continue; };
                    let mut w = Gauge::default().ratio(g.ratio as f64);
                    if let Some(label) = &g.label { w = w.label(label.clone()); }
                    if let Some(b) = &g.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Tabs as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(tbs) = crate::ptr_checked(cmd.handle as *const FfiTabs, "draw_frame:Tabs") else { continue; };
                    let titles: Vec<Line> = tbs.titles.iter().cloned().map(|s| Line::from(Span::raw(s))).collect();
                    let mut w = Tabs::new(titles).select(tbs.selected as usize);
                    if let Some(b) = &tbs.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::BarChart as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(bc) = crate::ptr_checked(cmd.handle as *const FfiBarChart, "draw_frame:BarChart") else { continue; };
                    let data: Vec<(&str, u64)> = bc.labels.iter().map(|s| s.as_str()).zip(bc.values.iter().cloned()).collect();
                    let mut w = RtBarChart::default().data(&data);
                    if let Some(wd) = bc.bar_width { w = w.bar_width(wd); }
                    if let Some(gp) = bc.bar_gap { w = w.bar_gap(gp); }
                    if let Some(st) = &bc.bar_style { w = w.bar_style(st.clone()); }
                    if let Some(st) = &bc.value_style { w = w.value_style(st.clone()); }
                    if let Some(st) = &bc.label_style { w = w.label_style(st.clone()); }
                    if let Some(b) = &bc.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Canvas as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(cv) = crate::ptr_checked(cmd.handle as *const FfiCanvas, "draw_frame:Canvas") else { continue; };
                    let mut w = RtCanvas::default().x_bounds([cv.x_min, cv.x_max]).y_bounds([cv.y_min, cv.y_max]);
                    if let Some(bg) = cv.background { w = w.background_color(bg); }
                    if let Some(b) = &cv.block { w = w.block(b.clone()); }
                    if let Some(mk) = cv.marker { w = w.marker(mk); }
                    w = w.paint(|p| {
                        for l in &cv.lines {
                            let col = crate::color_from_u32(l.style.fg).unwrap_or(Color::White);
                            p.draw(&RtCanvasLine { x1: l.x1, y1: l.y1, x2: l.x2, y2: l.y2, color: col });
                        }
                        for r in &cv.rects {
                            let col = crate::color_from_u32(r.style.fg).unwrap_or(Color::White);
                            p.draw(&RtCanvasRect { x: r.x, y: r.y, width: r.w, height: r.h, color: col });
                        }
                        for (pts, col) in &cv.pts {
                            p.draw(&RtCanvasPoints { coords: &pts[..], color: *col });
                        }
                    });
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Chart as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(ch) = crate::ptr_checked(cmd.handle as *const FfiChart, "draw_frame:Chart") else { continue; };
                    let mut datasets: Vec<RtDataset> = Vec::new();
                    for ds in &ch.datasets {
                        let mut d = RtDataset::default().name(ds.name.clone()).data(&ds.points);
                        if let Some(sty) = &ds.style { d = d.style(sty.clone()); }
                        d = d.graph_type(match ds.kind { 1 => RtGraphType::Bar, 2 => RtGraphType::Scatter, _ => RtGraphType::Line });
                        datasets.push(d);
                    }
                    let mut chart = RtChart::new(datasets);
                    let x_axis = {
                        let mut ax = RtAxis::default();
                        if let Some(t) = &ch.x_title { ax = ax.title(t.clone()); }
                        if let (Some(min), Some(max)) = (ch.x_min, ch.x_max) { ax = ax.bounds([min, max]); }
                        if let Some(st) = &ch.x_axis_style { ax = ax.style(st.clone()); }
                        if let Some(lbls) = &ch.x_labels { ax = ax.labels(lbls.clone()); }
                        if let Some(al) = ch.x_labels_align { ax = ax.labels_alignment(al); }
                        ax
                    };
                    let y_axis = {
                        let mut ay = RtAxis::default();
                        if let Some(t) = &ch.y_title { ay = ay.title(t.clone()); }
                        if let (Some(min), Some(max)) = (ch.y_min, ch.y_max) { ay = ay.bounds([min, max]); }
                        if let Some(st) = &ch.y_axis_style { ay = ay.style(st.clone()); }
                        if let Some(lbls) = &ch.y_labels { ay = ay.labels(lbls.clone()); }
                        if let Some(al) = ch.y_labels_align { ay = ay.labels_alignment(al); }
                        ay
                    };
                    chart = chart.x_axis(x_axis).y_axis(y_axis);
                    if let Some(st) = &ch.chart_style { chart = chart.style(st.clone()); }
                    if let Some(b) = &ch.block { chart = chart.block(b.clone()); }
                    frame.render_widget(chart, area);
                }
                x if x == FfiWidgetKind::LineGauge as u32 => {
                    if cmd.handle.is_null() { continue; }
                    let Some(lg) = crate::ptr_checked(cmd.handle as *const FfiLineGauge, "draw_frame:LineGauge") else { continue; };
                    let mut w = RtLineGauge::default().ratio(lg.ratio as f64);
                    if let Some(label) = &lg.label { w = w.label(label.clone()); }
                    if let Some(b) = &lg.block { w = w.block(b.clone()); }
                    frame.render_widget(w, area);
                }
                x if x == FfiWidgetKind::Clear as u32 => {
                    frame.render_widget(RtClear, area);
                }
                x if x == FfiWidgetKind::RatatuiLogo as u32 => {
                    frame.render_widget(ratatui::widgets::RatatuiLogo::default(), area);
                }
                _ => {}
            }
        }
    });
    res.is_ok()
}

pub fn render_cmd_to_buffer(cmd: &FfiDrawCmd, buf: &mut Buffer) {
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
            let header_row = if tb.headers.is_empty() { None } else {
                Some(Row::new(tb.headers.iter().cloned().map(Cell::from).collect::<Vec<_>>()))
            };
            let rows: Vec<Row> = tb.rows.iter().map(|r| Row::new(r.iter().cloned().map(Cell::from).collect::<Vec<_>>())).collect();
            let col_count = if !tb.rows.is_empty() { tb.rows.iter().map(|r| r.len()).max().unwrap_or(1) } else { tb.headers.len().max(1) };
            let widths = std::iter::repeat(ratatui::layout::Constraint::Percentage((100 / col_count.max(1)) as u16)).take(col_count.max(1));
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
            let data: Vec<(&str, u64)> = bc.labels.iter().map(|s| s.as_str()).zip(bc.values.iter().cloned()).collect();
            let mut w = RtBarChart::default().data(&data);
            if let Some(b) = &bc.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Canvas as u32 => {
            if cmd.handle.is_null() { return; }
            let cv = unsafe { &*(cmd.handle as *const FfiCanvas) };
            let mut w = RtCanvas::default().x_bounds([cv.x_min, cv.x_max]).y_bounds([cv.y_min, cv.y_max]);
            if let Some(bg) = cv.background { w = w.background_color(bg); }
            if let Some(b) = &cv.block { w = w.block(b.clone()); }
            w = w.paint(|p| {
                for l in &cv.lines {
                    let col = crate::color_from_u32(l.style.fg).unwrap_or(Color::White);
                    p.draw(&RtCanvasLine { x1: l.x1, y1: l.y1, x2: l.x2, y2: l.y2, color: col });
                }
                for r in &cv.rects {
                    let col = crate::color_from_u32(r.style.fg).unwrap_or(Color::White);
                    p.draw(&RtCanvasRect { x: r.x, y: r.y, width: r.w, height: r.h, color: col });
                }
                for (pts, col) in &cv.pts { p.draw(&RtCanvasPoints { coords: &pts[..], color: *col }); }
            });
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Chart as u32 => {
            if cmd.handle.is_null() { return; }
            let ch = unsafe { &*(cmd.handle as *const FfiChart) };
            let mut datasets: Vec<RtDataset> = Vec::new();
            for ds in &ch.datasets {
                let mut d = RtDataset::default().name(ds.name.clone()).data(&ds.points);
                if let Some(sty) = &ds.style { d = d.style(sty.clone()); }
                d = d.graph_type(match ds.kind { 1 => RtGraphType::Bar, 2 => RtGraphType::Scatter, _ => RtGraphType::Line });
                datasets.push(d);
            }
            let mut chart = RtChart::new(datasets);
            let x_axis = { let mut ax = RtAxis::default(); if let Some(ti) = &ch.x_title { ax = ax.title(ti.clone()); } ax };
            let y_axis = { let mut ay = RtAxis::default(); if let Some(ti) = &ch.y_title { ay = ay.title(ti.clone()); } ay };
            chart = chart.x_axis(x_axis).y_axis(y_axis);
            if let Some(b) = &ch.block { chart = chart.block(b.clone()); }
            ratatui::widgets::Widget::render(chart, area, buf);
        }
        x if x == FfiWidgetKind::LineGauge as u32 => {
            if cmd.handle.is_null() { return; }
            let lg = unsafe { &*(cmd.handle as *const FfiLineGauge) };
            let mut w = ratatui::widgets::LineGauge::default().ratio(lg.ratio as f64);
            if let Some(label) = &lg.label { w = w.label(label.clone()); }
            if let Some(b) = &lg.block { w = w.block(b.clone()); }
            ratatui::widgets::Widget::render(w, area, buf);
        }
        x if x == FfiWidgetKind::Clear as u32 => {
            ratatui::widgets::Widget::render(RtClear, area, buf);
        }
        x if x == FfiWidgetKind::RatatuiLogo as u32 => {
            ratatui::widgets::Widget::render(ratatui::widgets::RatatuiLogo::default(), area, buf);
        }
        _ => {}
    }
}
