// Widget split placeholder: Sparkline
// Move from src/lib.rs:
// - FFI externs: ratatui_sparkline_new, ratatui_sparkline_free
// - Setters: ratatui_sparkline_set_values, ratatui_sparkline_set_max, ratatui_sparkline_set_style
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_sparkline_set_block_title, FfiSparkline)
//   ratatui_block_title_spans_fn!(ratatui_sparkline_set_block_title_spans, FfiSparkline)
//   ratatui_block_adv_fn!(ratatui_sparkline_set_block_adv, FfiSparkline)
// - Draw helpers: ratatui_terminal_draw_sparkline_in, ratatui_headless_render_sparkline
// Types used: FfiSparkline

// use crate::*; // enable when moving implementations

