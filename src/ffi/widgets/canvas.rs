// Widget split placeholder: Canvas
// Move from src/lib.rs:
// - Types: FfiCanvas, FfiCanvasLine, FfiCanvasRect, FfiCanvasPoints
// - FFI externs: ratatui_canvas_new, ratatui_canvas_free
// - Setters: ratatui_canvas_set_bounds, ratatui_canvas_set_background_color, ratatui_canvas_set_marker
// - Block helpers (macros and manual):
//   ratatui_block_title_fn!(ratatui_canvas_set_block_title, FfiCanvas)
//   ratatui_block_title_spans_fn!(ratatui_canvas_set_block_title_spans, FfiCanvas)
//   ratatui_canvas_set_block_adv (manual using build_block_from_adv)
// - Adders: ratatui_canvas_add_line, ratatui_canvas_add_rect, ratatui_canvas_add_points
// - Draw helpers: ratatui_terminal_draw_canvas_in, ratatui_headless_render_canvas

// use crate::*; // enable when moving implementations

