# <img src="./logo.webp" alt="ratatui_ffi logo" width="36"/> ratatui_ffi

![CI](https://github.com/holo-q/ratatui-ffi/actions/workflows/ci.yml/badge.svg)
[![GitHub Release](https://img.shields.io/github/v/release/holo-q/ratatui-ffi?logo=github)](https://github.com/holo-q/ratatui-ffi/releases)
[![crates.io](https://img.shields.io/crates/v/ratatui_ffi.svg?logo=rust&label=crates.io)](https://crates.io/crates/ratatui_ffi)
[![crates.io downloads](https://img.shields.io/crates/d/ratatui_ffi.svg?logo=rust)](https://crates.io/crates/ratatui_ffi)
[![docs.rs](https://img.shields.io/docsrs/ratatui_ffi?logo=rust)](https://docs.rs/ratatui_ffi)

Native C ABI for [Ratatui], shipped as a tiny `cdylib` you can call from C, C#, Python, TypeScript (via FFI), and more. Optimized for hot loops: span‑based setters and batch APIs minimize allocations and marshaling.

## Highlights

- Widgets: Paragraph, List (+state), Table (+state), Tabs, Gauge, LineGauge, BarChart, Sparkline, Chart, Scrollbar, Clear, RatatuiLogo, Canvas.
- Layout: `layout_split`, `layout_split_ex` (spacing + per‑side margins), `layout_split_ex2` (adds `Constraint::Ratio`).
- Text/Styles: `FfiStyle`, `FfiSpan`, `FfiLineSpans`; lines of styled spans; paragraph base style, alignment, wrap(trim), scroll; named/RGB/indexed colors; all modifiers (incl. hidden/blink).
- Blocks: per‑side borders, border type, padding, title alignment, and title as spans across all block‑bearing widgets.
- Terminal: init/clear, batched frame render, raw/alt toggles, cursor get/set/show, size, event poll and injection.
- Headless: ASCII snapshots; compact and extended style dumps; structured cell dump (`FfiCellInfo`).
- Throughput: list/paragraph/table batching; table multi‑line cells; dataset batching; reserve helpers.
- Zero‑alloc paths: span‑based label/title/divider setters for hot code paths.


## Quick Start

### Language Bindings

<!-- Bindings badges -->
[![Python Binding](https://img.shields.io/badge/bindings-Python-3776AB?logo=python&logoColor=white)](https://github.com/holo-q/ratatui-py)
[![Go Binding](https://img.shields.io/badge/bindings-Go-00ADD8?logo=go&logoColor=white)](https://github.com/holo-q/ratatui-go)
[![TypeScript Binding](https://img.shields.io/badge/bindings-TypeScript-3178C6?logo=typescript&logoColor=white)](https://github.com/holo-q/ratatui-ts)

- C#: [holo-q/Ratatui.cs](https://github.com/holo-q/Ratatui.cs)
- Python: [holo-q/ratatui-py](https://github.com/holo-q/ratatui-py)
- Go: [holo-q/ratatui-go](https://github.com/holo-q/ratatui-go)
- TypeScript: [holo-q/ratatui-ts](https://github.com/holo-q/ratatui-ts)

### Building

Build the library:
```bash
cargo build --release
# → target/release/libratatui_ffi.so (Linux), .dylib (macOS), ratatui_ffi.dll (Windows)
```

Use from C (example: Gauge label spans):
```c
FfiStyle white = { .fg = 0x00000010, .bg = 0, .mods = 0 }; // white named
FfiSpan spans[2] = {
  { .text_utf8 = "Load ", .style = white },
  { .text_utf8 = "80%",    .style = white },
};
ratatui_gauge_set_label_spans(gauge, spans, 2);
```

## Aspects

### Span‑Based Setters (Zero‑Alloc Paths)

Preferred over UTF‑8 string setters in hot loops. All functions treat `FfiSpan.text_utf8` as NUL‑terminated UTF‑8 without ownership transfer.

- Tabs: `ratatui_tabs_set_divider_spans(spans, len)`
- Gauge: `ratatui_gauge_set_label_spans(spans, len)`, `ratatui_gauge_set_block_title_spans(spans, len, show_border)`
- LineGauge: `ratatui_linegauge_set_label_spans(spans, len)`
- BarChart: `ratatui_barchart_set_labels_spans(lines, len)`, `ratatui_barchart_set_block_title_spans(spans, len, show_border)`
- Table: `ratatui_table_set_block_title_spans(spans, len, show_border)`
- Paragraph/List/Tabs/LineGauge/Chart/Sparkline/Scrollbar/Canvas: `*_set_block_title_spans(spans, len, show_border)`

Notes and limits:
- Tabs divider: if a single span is provided, style is preserved; otherwise texts are concatenated (ratatui accepts a single `Span`).
- Gauge label: texts are concatenated; use `ratatui_gauge_set_styles(..., label_style, ...)` for label styling.
- BarChart labels: per‑label styling is not supported by ratatui; text‑only, same as TSV path.

### FFI Types

- `FfiStyle { fg: u32, bg: u32, mods: u16 }` with helpers `ratatui_color_rgb`, `ratatui_color_indexed`.
- `FfiSpan { text_utf8: *const c_char, style: FfiStyle }`
- `FfiLineSpans { spans: *const FfiSpan, len: usize }`
- Structured outputs: `FfiCellInfo` (headless), list/table state types, draw commands for batched frames.


### Headless Rendering

- Text snapshots: `ratatui_headless_render_frame`, and per‑widget helpers (`_paragraph`, `_list`, `_table`, ...).
- Style snapshots:
  - Compact: `ratatui_headless_render_frame_styles` → rows of `FG2 BG2 MOD4` hex (named palette).
  - Extended: `ratatui_headless_render_frame_styles_ex` → `FG8 BG8 MOD4` hex (`FfiStyle` encoding).
  - Structured cells: `ratatui_headless_render_frame_cells` → fill array of `FfiCellInfo`.

### Feature Bits (Introspection)

Call `ratatui_ffi_feature_bits()` to detect support at runtime. Bits include:

- `SCROLLBAR`, `CANVAS`, `STYLE_DUMP_EX`, `BATCH_TABLE_ROWS`, `BATCH_LIST_ITEMS`, `COLOR_HELPERS`, `AXIS_LABELS`, `SPAN_SETTERS`.


## Tips

### Hot‑Path Tips

- Prefer span‑based setters and batched APIs to avoid allocations and repeated marshaling.
- Reserve capacity where possible (`ratatui_*_reserve_*`) before large appends.
- Use headless render snapshots in CI for fast, deterministic tests.

### Runtime Behavior & Logging

- By default raw mode is enabled; use `RATATUI_FFI_NO_RAW=1` to disable; `RATATUI_FFI_ALTSCR=1` to use the alternate screen.
- Set `RATATUI_FFI_TRACE=1` to trace `ENTER/EXIT` of FFI calls (stderr and optional file).
- Set `RATATUI_FFI_LOG=<path>` to write logs; truncate per run; use `RATATUI_FFI_LOG_APPEND=1` to append.
- Functions that interact with the terminal are wrapped in panic guards and validate pointers/rects.


## Development

### Introspection Tools

Build Ratatui docs (once) to enable widget coverage detection, then run the introspector:
```bash
cargo doc -p ratatui
cargo run --quiet --bin ffi_introspect
```
Outputs grouped export lists and a module summary. Pass `--json` for JSON.

### C Header Generation

This crate exposes a C ABI and ships a cbindgen config to generate a header for C/C++ consumers.

Generate `include/ratatui_ffi.h` with either:
```bash
# Rusty way (requires cbindgen installed):
cargo run --quiet --bin gen_header

# or Bash helper:
bash tools/gen_header.sh
```
Then include it from C/C++ bindings. CI can generate and attach it to releases.


### CI Notes

Release builds can produce prebuilt binaries for Linux/macOS/Windows. See the GitHub Actions in this repo and the C# binding repo for multi‑RID examples.

[Ratatui]: https://github.com/ratatui-org/ratatui
