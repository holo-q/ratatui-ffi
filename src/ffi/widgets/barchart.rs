// Widget split placeholder: BarChart
// Move the following from src/lib.rs into this module:
// - FFI externs: ratatui_barchart_new, ratatui_barchart_free
// - Setters: ratatui_barchart_set_values, ratatui_barchart_set_labels, ratatui_barchart_set_labels_spans,
//   ratatui_barchart_set_bar_width, ratatui_barchart_set_bar_gap, ratatui_barchart_set_styles
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_barchart_set_block_title, FfiBarChart)
//   ratatui_block_title_spans_fn!(ratatui_barchart_set_block_title_spans, FfiBarChart)
//   ratatui_block_adv_fn!(ratatui_barchart_set_block_adv, FfiBarChart)
// - Draw helpers: ratatui_terminal_draw_barchart_in, ratatui_headless_render_barchart
// Types used: FfiBarChart

// use crate::*; // enable when moving implementations

