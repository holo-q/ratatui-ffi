// Widget split placeholder: Chart
// Move from src/lib.rs:
// - FFI externs: ratatui_chart_new, ratatui_chart_free
// - Adders: ratatui_chart_add_line, ratatui_chart_add_dataset_with_type, ratatui_chart_add_datasets
// - Setters: ratatui_chart_set_axes_titles, ratatui_chart_set_bounds, ratatui_chart_set_legend_position,
//            ratatui_chart_set_hidden_legend_constraints, ratatui_chart_set_axis_styles,
//            ratatui_chart_set_x_labels_spans, ratatui_chart_set_y_labels_spans,
//            ratatui_chart_set_labels_alignment
// - Block helpers (macros invoked here):
//   ratatui_block_title_fn!(ratatui_chart_set_block_title, FfiChart)
//   ratatui_block_title_spans_fn!(ratatui_chart_set_block_title_spans, FfiChart)
//   ratatui_block_adv_fn!(ratatui_chart_set_block_adv, FfiChart)
// - Draw helpers: ratatui_terminal_draw_chart_in, ratatui_headless_render_chart
// Types used: FfiChart, FfiChartDataset, FfiChartDatasetSpec

// use crate::*; // enable when moving implementations

