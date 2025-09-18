// Widget split placeholder: LineGauge
// Move from src/lib.rs:
// - FFI externs: ratatui_linegauge_new, ratatui_linegauge_free
// - Setters: ratatui_linegauge_set_ratio, ratatui_linegauge_set_label, ratatui_linegauge_set_label_spans,
//            ratatui_linegauge_set_style
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_linegauge_set_block_title, FfiLineGauge)
//   ratatui_block_title_spans_fn!(ratatui_linegauge_set_block_title_spans, FfiLineGauge)
//   ratatui_block_title_alignment_fn!(ratatui_linegauge_set_block_title_alignment, FfiLineGauge)
//   ratatui_block_adv_fn!(ratatui_linegauge_set_block_adv, FfiLineGauge)
// - Draw helpers: ratatui_terminal_draw_linegauge_in, ratatui_headless_render_linegauge
// Types used: FfiLineGauge

// use crate::*; // enable when moving implementations
