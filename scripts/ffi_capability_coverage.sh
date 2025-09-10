#!/usr/bin/env bash
set -euo pipefail

# Capability coverage report for ratatui_ffi.
# Scans source for exported functions and, if a built library is present,
# also checks which symbols are actually exported by the binary.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
SRC_FILE="$ROOT_DIR/src/lib.rs"

if [[ ! -f "$SRC_FILE" ]]; then
  echo "error: cannot find src/lib.rs at $SRC_FILE" >&2
  exit 2
fi

# Extract export names from source (#[no_mangle] pub extern "C" fn NAME()
extract_from_src() {
  awk '/#\[no_mangle\]/{getline; if ($0 ~ /extern "C" fn ([a-zA-Z0-9_]+)/){match($0,/extern "C" fn ([a-zA-Z0-9_]+)/,m); print m[1]}}' "$SRC_FILE" |
    sort -u
}

# Extract export names from built shared library, if available
extract_from_lib() {
  local libpath="$1"
  if [[ ! -f "$libpath" ]]; then
    return 0
  fi
  local os
  os="$(uname -s 2>/dev/null || echo unknown)"
  case "$os" in
    Linux)
      nm -D --defined-only "$libpath" 2>/dev/null | awk '{print $3}' | rg '^ratatui_' || true ;;
    Darwin)
      nm -gUj "$libpath" 2>/dev/null | rg '^ratatui_' || true ;;
    *) ;;
  esac | sort -u
}

have_nm=0
if command -v llvm-nm >/dev/null 2>&1; then
  alias nm=llvm-nm
  have_nm=1
elif command -v nm >/dev/null 2>&1; then
  have_nm=1
fi

mapfile -t SRC_FUNCS < <(extract_from_src)
declare -A SRC_SET=()
for f in "${SRC_FUNCS[@]}"; do SRC_SET["$f"]=1; done

# Locate a built library if any
LIB=""
for cand in \
  "$ROOT_DIR/target/release/libratatui_ffi.so" \
  "$ROOT_DIR/target/release/libratatui_ffi.dylib" \
  "$ROOT_DIR/target/debug/libratatui_ffi.so" \
  "$ROOT_DIR/target/debug/libratatui_ffi.dylib"; do
  [[ -f "$cand" ]] && LIB="$cand" && break
done

BIN_FUNCS=()
if (( have_nm == 1 )) && [[ -n "$LIB" ]]; then
  while IFS= read -r line; do BIN_FUNCS+=("$line"); done < <(extract_from_lib "$LIB")
fi
declare -A BIN_SET=()
for f in "${BIN_FUNCS[@]}"; do BIN_SET["$f"]=1; done

have() { [[ -n "${SRC_SET[$1]:-}" ]]; }
have_bin() { [[ -n "${BIN_SET[$1]:-}" ]]; }

report_cap() {
  local name="$1"; shift
  local -a req=( "$@" )
  local ok_src=1 ok_bin=1
  local bin_count
  bin_count=${#BIN_FUNCS[@]}
  for r in "${req[@]}"; do
    have "$r" || ok_src=0
    if (( bin_count > 0 )); then
      have_bin "$r" || ok_bin=0
    fi
  done
  local src_mark bin_mark
  src_mark=$([[ $ok_src == 1 ]] && echo "✓" || echo "✗")
  if (( bin_count > 0 )); then
    bin_mark=$([[ $ok_bin == 1 ]] && echo "✓" || echo "✗")
  else
    bin_mark="-"
  fi
  printf "%-30s src:%s bin:%s\n" "$name" "$src_mark" "$bin_mark"
}

echo "== ratatui_ffi capability coverage =="
if [[ -n "$LIB" ]]; then
  echo "Library: $LIB"
else
  echo "Library: (not built; bin column will show '-')"
fi
echo "Functions detected in source: ${#SRC_FUNCS[@]}"

echo
echo "-- Core --"
report_cap "terminal"            ratatui_init_terminal ratatui_terminal_free ratatui_terminal_size
report_cap "frame_batch"         ratatui_terminal_draw_frame ratatui_headless_render_frame
report_cap "events"              ratatui_next_event ratatui_inject_key ratatui_inject_mouse ratatui_inject_resize

echo
echo "-- Text / Styles --"
report_cap "paragraph_spans"     ratatui_paragraph_append_span ratatui_paragraph_line_break
report_cap "paragraph_base"      ratatui_paragraph_new ratatui_paragraph_append_line ratatui_paragraph_set_block_title
report_cap "list_basic"          ratatui_list_new ratatui_list_append_item ratatui_list_set_block_title
report_cap "table_basic"         ratatui_table_new ratatui_table_set_headers ratatui_table_append_row ratatui_table_set_block_title
report_cap "tabs_basic"          ratatui_tabs_new ratatui_tabs_set_titles ratatui_tabs_set_selected

echo
echo "-- Widgets --"
report_cap "gauge"               ratatui_gauge_new ratatui_gauge_set_ratio
report_cap "barchart"            ratatui_barchart_new ratatui_barchart_set_values
report_cap "sparkline"           ratatui_sparkline_new ratatui_sparkline_set_values
report_cap "chart_line"          ratatui_chart_new ratatui_chart_add_line

echo
echo "-- Optional (feature) --"
report_cap "scrollbar"           ratatui_headless_render_scrollbar ratatui_scrollbar_free

echo
echo "-- Known missing (expected future) --"
report_cap "paragraph_align"     ratatui_paragraph_set_alignment
report_cap "paragraph_wrap"      ratatui_paragraph_set_wrap
report_cap "paragraph_scroll"    ratatui_paragraph_set_scroll
report_cap "block_padding"       ratatui_block_set_padding
report_cap "block_borders_sides" ratatui_block_set_borders
report_cap "block_border_type"   ratatui_block_set_border_type
report_cap "block_title_spans"   ratatui_paragraph_set_block_title_spans
report_cap "list_item_spans"     ratatui_list_append_item_spans
report_cap "tabs_title_spans"    ratatui_tabs_set_titles_spans
report_cap "table_widths"        ratatui_table_set_widths
report_cap "table_cell_spans"    ratatui_table_set_cell_spans
report_cap "gauge_styles"        ratatui_gauge_set_styles
report_cap "barchart_styles"     ratatui_barchart_set_styles
report_cap "sparkline_style"     ratatui_sparkline_set_style
report_cap "chart_axis_bounds"   ratatui_chart_set_axis_bounds
report_cap "chart_dataset_types" ratatui_chart_add_dataset_with_type
report_cap "layout_api"          ratatui_layout_split
report_cap "frame_cursor"        ratatui_terminal_set_cursor
report_cap "clear_widget"        ratatui_clear_in
report_cap "term_raw_toggle"     ratatui_terminal_enable_raw ratatui_terminal_disable_raw
report_cap "term_alt_toggle"     ratatui_terminal_enter_alt ratatui_terminal_leave_alt

echo
echo "Note: 'src:✓' means functions exist in source; 'bin:✓' means they are exported by the current build. A '-' in bin means no lib found."
