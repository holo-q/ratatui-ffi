// Widget split placeholder: Scrollbar (feature = "scrollbar")
// Move from src/lib.rs (under cfg(feature = "scrollbar")):
// - Types: FfiScrollbarOrient, FfiScrollbar, FfiScrollbarOrientation, FfiScrollDirection (enums under cfg)
// - FFI externs: ratatui_scrollbar_new, ratatui_scrollbar_free, ratatui_scrollbar_configure,
//                ratatui_scrollbar_set_orientation_side
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_scrollbar_set_block_title, FfiScrollbar)
//   ratatui_block_title_spans_fn!(ratatui_scrollbar_set_block_title_spans, FfiScrollbar)
//   ratatui_block_title_alignment_fn!(ratatui_scrollbar_set_block_title_alignment, FfiScrollbar)
//   ratatui_block_adv_fn!(ratatui_scrollbar_set_block_adv, FfiScrollbar)
// - Draw helpers: ratatui_terminal_draw_scrollbar_in, ratatui_headless_render_scrollbar

// use crate::*; // enable when moving implementations

