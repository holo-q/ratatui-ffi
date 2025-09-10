# ratatui_ffi

![CI](https://github.com/holo-q/ratatui-ffi/actions/workflows/ci.yml/badge.svg)
[![GitHub Release](https://img.shields.io/github/v/release/holo-q/ratatui-ffi?logo=github)](https://github.com/holo-q/ratatui-ffi/releases)
[![crates.io](https://img.shields.io/crates/v/ratatui_ffi.svg?logo=rust&label=crates.io)](https://crates.io/crates/ratatui_ffi)
[![crates.io downloads](https://img.shields.io/crates/d/ratatui_ffi.svg?logo=rust)](https://crates.io/crates/ratatui_ffi)
[![docs.rs](https://img.shields.io/docsrs/ratatui_ffi?logo=rust)](https://docs.rs/ratatui_ffi)

Native C ABI for [Ratatui], exposing a small cdylib you can consume from C, C#, Python, TypeScript, and others.

## Current Coverage

- Widgets: Paragraph, List (+state), Table (+state), Tabs, Gauge, LineGauge, BarChart, Sparkline, Chart, Scrollbar, Clear, RatatuiLogo, Canvas.
- Layout: `layout_split`, `layout_split_ex` (spacing + per‑side margins), `layout_split_ex2` (adds `Constraint::Ratio`).
- Text/Styles: `Span`, `Line`, per‑span lines; paragraph base style, alignment, wrap(trim), scroll; colors (named/RGB/indexed); modifiers (incl. hidden).
- Block: per‑side borders, border type, padding, title as spans, title alignment across all block‑bearing widgets.
- Terminal: init/clear, batched frame render, raw/alt toggles, cursor get/set/show, size, event poll/injection.
- Headless: text snapshot; compact and full‑fidelity style snapshots; structured cell dump (`FfiCellInfo`).
- Batching: list items, paragraph lines, table rows with multi‑line cells, datasets; reserve helpers.

## Language Bindings
- C#: [holo-q/Ratatui.cs](https://github.com/holo-q/Ratatui.cs)
- Python: [holo-q/ratatui-py](https://github.com/holo-q/ratatui-py)

Status
- Targets the Ratatui workspace API (0.30 beta series). The crate currently depends on the workspace layout, which is why it has a path dependency to `ratatui`.
- If you want to build against crates.io instead, switch the dependency to a crates.io version (see below).

Build
```bash
cargo build --release
# produces target/release/libratatui_ffi.so (Linux), .dylib (macOS), or ratatui_ffi.dll (Windows)
```

Local FFI introspection
- Build Ratatui docs once to enable widget coverage:
  ```bash
  cargo doc -p ratatui
  ```
- Run the introspector to see FFI exports and widget coverage:
  ```bash
  cargo run --quiet --bin ffi_introspect
  ```
  It reports source/binary exports and groups by prefix, and compares widget coverage against Ratatui’s public docs. No files are generated.
  It also prints a module‑group summary (terminal/layout/headless/etc.). For JSON, pass `--json`.

Headless rendering and style snapshots
- Text snapshots: render a composed frame of widgets without a terminal:
  - `ratatui_headless_render_frame(width, height, cmds, len, out_text_utf8)`
  - `ratatui_headless_render_paragraph`, `ratatui_headless_render_list`, `ratatui_headless_render_table`, etc.
- Style snapshots: per-cell style dumps for visual testing:
  - Compact: `ratatui_headless_render_frame_styles` returns rows of "FG2 BG2 MOD4" hex groups (named palette only).
  - Extended: `ratatui_headless_render_frame_styles_ex` returns rows of "FG8 BG8 MOD4" hex groups where FG/BG use the same 32-bit encoding as `FfiStyle` (named, indexed, or RGB).
  - Structured cells: `ratatui_headless_render_frame_cells(width,height,cmds,len,out_cells,cap)` to fill an array of `FfiCellInfo { ch, fg, bg, mods }`.

Throughput helpers
- Tables with many multi-line cells can be appended in batches to reduce FFI overhead:
  - Single row: `ratatui_table_append_row_cells_lines(cells, cell_count)`
  - Batched rows: `ratatui_table_append_rows_cells_lines(rows, row_count)` where each row is an `FfiRowCellsLines` pointing to an array of `FfiCellLines`.

Canvas (custom drawing)
- Build custom charts/maps with the Ratatui Canvas via FFI:
  - Create: `ratatui_canvas_new(x_min, x_max, y_min, y_max)`
  - Configure: `ratatui_canvas_set_bounds`, `ratatui_canvas_set_background_color`, `ratatui_canvas_set_block_title`/`_adv`
  - Add shapes: `ratatui_canvas_add_line`, `ratatui_canvas_add_rect`, `ratatui_canvas_add_points`
  - Render: `ratatui_terminal_draw_canvas_in` or `ratatui_headless_render_canvas`
  - Notes: for 0.29, points use a default marker (no per-points marker selection).

Using from C/C#
- Exported symbols use `extern "C"` and a stable ABI.
- See the C# wrapper in holo-q/ratatui-cs for a reference P/Invoke layer and SafeHandle pattern.

Install (Rust)
```bash
cargo add ratatui_ffi
```

Switching to crates.io
- Current Cargo.toml uses:
  ```toml
  ratatui = { path = "../../ratatui/ratatui" }
  ```
- To build standalone (without the workspace), replace with a version:
  ```toml
  ratatui = "0.29"
  crossterm = "0.27"
  ```
- Note: API has changed in 0.30+ (split crates). If you keep using 0.30 workspace (beta), retain the path dep or pin compatible versions across the split crates.

CI (optional)
- You can add a simple GitHub Actions workflow to build release artifacts for linux-x64, win-x64, osx-x64, osx-arm64 and upload them to releases.
- See holo-q/ratatui-cs for an example of multi-RID builds and packaging.

[Ratatui]: https://github.com/ratatui-org/ratatui
