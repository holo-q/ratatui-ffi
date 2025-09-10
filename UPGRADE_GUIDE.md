# Ratatui FFI Upgrade Guide

This guide describes a repeatable, local-only workflow to upgrade ratatui_ffi to a new Ratatui release and validate full functional parity before handing off to bindings.

Audience: FFI maintainers. Outcome: updated FFI + clear delta report + compatibility guidance for bindings.

---

## 0. Prep and Principles

- Parity target: functional parity. Anything a Rust Ratatui user can do should be reachable via our FFI with stable, language-agnostic types/functions.
- ABI shape: prefer simple C ABI (flat functions, primitive fields, small enums/bitflags). Hide generics/traits behind handles and setters.
- Consistency: every widget exposed should have:
  - create/free
  - configure (setters for options/styles)
  - terminal draw in rect
  - headless single-widget render
  - batched frame support (FfiWidgetKind)
- Style: keep names `ratatui_<group>_<verb>`; propagate new knobs via `*_set_*` APIs; reuse existing structs (Style/Span/Line/Rect) when possible.

---

## 1. Update and Inspect

1) Pin the new Ratatui version

- Edit `Cargo.toml` dep:
  - For crates.io: `ratatui = "<new_version>"`
  - For workspace/beta: adjust the path dep to the new workspace checkout.

2) Build docs for coverage mapping

```bash
cargo doc -p ratatui
```

3) Build the FFI (debug or release)

```bash
cargo build --release
```

4) Run introspection (quick + rich views)

- Flat summary (source↔binary parity + groups):
  ```bash
  scripts/ffi_introspect.sh
  ```
- Rich view (widgets coverage vs docs + module groups + optional JSON):
  ```bash
  cargo run --quiet --bin ffi_introspect
  cargo run --quiet --bin ffi_introspect -- --json > /tmp/ffi.json
  ```

What to look for
- Mismatches: source-only or binary-only exports → fix immediately.
- Widgets coverage: any ✗ means a new widget or renamed/moved one needs exposing.
- Group drift: new groups or significant count changes hint at new capability areas (e.g., layout, chart, canvas).

---

## 2. Map New/Changed APIs

Use Ratatui release notes + rustdoc pages to identify changes. For each area below, add or adapt FFI.

### A) New Widgets
- Add handle struct (opaque fields or data needed to configure the widget).
- Implement `*_new`, `*_free`, configure setters, terminal draw, headless render.
- Add `FfiWidgetKind::<Widget>` + render path in batched frame and headless-frame.
- If the widget accepts text/styling, support span-based inputs (FfiSpan/FfiLineSpans) from day one.

### B) Block/Style/Text
- If Block gains new knobs (e.g., title alignment, padding variants), add shared helpers and widget-specific setters.
- For text, ensure per-span (`FfiSpan`) content exists for relevant widgets (Paragraph, List, Tabs, Table cells).
- Colors: if Ratatui adds modes, extend our `FfiStyle` encoding; keep existing flags stable (0 reset, 1..16 named, 0x40000000 indexed, 0x80000000 RGB).

### C) Layout
- If new constraints (e.g., `Ratio`) or spacing/flex options appear:
  - Extend `ratatui_layout_split_ex*` (add an `_exN` rather than breaking existing signatures).
  - Keep Direction + per-side margins.

### D) Terminal
- New cursor/viewport/raw/alt helpers: add explicit APIs (`ratatui_terminal_*`).
- If Ratatui exposes viewport APIs publicly in this version, wire `ratatui_terminal_set/get_viewport_area` (previous stubs return false).

### E) Chart/Canvas/Symbols
- Chart: datasets (types), axis labels/alignment, bounds, legend options, styles.
- Canvas: bounds/background, marker, primitives.
- If symbol sets (e.g., for BarChart) are introduced, map as enums + optional raw char.

---

## 3. Wire Common Paths

Every new/changed widget should be wired in three places:
- Single-widget terminal draw (`ratatui_terminal_draw_<widget>_in`).
- Headless single-widget render (`ratatui_headless_render_<widget>`).
- Batched frame:
  - `FfiWidgetKind` enum updated.
  - `render_cmd_to_buffer` and terminal frame renderer updated.

Also update headless style/text dumps if needed to include new behaviors.

---

## 4. Validate Locally

1) Build + run introspection again
```bash
cargo build --release
scripts/ffi_introspect.sh
cargo run --quiet --bin ffi_introspect
```
- Ensure source=binary counts match.
- Ensure widget coverage is all ✓.
- Check group counts for sanity.

2) Quick behavioral snapshots
- Use existing headless demos or compose a standard frame (Paragraph/List/Table/Tabs/Gauge/Chart/Scrollbar/Canvas).
- Capture:
  - `ratatui_headless_render_frame` (text)
  - `ratatui_headless_render_frame_styles_ex` (full styles)
  - `ratatui_headless_render_frame_cells` (structured cells)
- Spot-check visibly changed widgets.

---

## 5. Versioning and Feature Bits

- Bump crate version (SemVer):
  - Patch: purely additive, no signature changes.
  - Minor: new functions/types that bindings must implement.
  - Major: breaking changes to existing signatures/structs (avoid; prefer adding new `_ex` variants).
- Update feature bits in `ratatui_ffi_feature_bits()` to reflect new capabilities (e.g., AXIS_LABELS, CANVAS, STYLE_DUMP_EX, batching flags).
- Document changes in README and consider adding a short “What’s new” section.

---

## 6. Binding Consumers (FYI for Maintainers)

We keep bindings manual (C#, Python, TS). After an FFI upgrade:
- Produce `cargo run --bin ffi_introspect -- --json` and hand it to binding agents for coverage.
- Provide a small “upgrade note” summarizing new/renamed functions and important enums/struct fields.
- Encourage bindings to:
  - add link-through coverage checks against the JSON
  - snapshot test the standard frame (text + styles + cells)

---

## 7. Checklist (Copy/Paste)

- [ ] Update `Cargo.toml` ratatui dep to new version (or path).
- [ ] `cargo doc -p ratatui` (enables widget coverage in the tool).
- [ ] `cargo build --release` (baseline).
- [ ] `scripts/ffi_introspect.sh` and `cargo run --bin ffi_introspect` (record counts; note deltas).
- [ ] Map/implement new widgets (create/free, setters, draw, headless, batch).
- [ ] Extend Block/Text/Style as needed (spans, padding, title alignment, colors, modifiers).
- [ ] Extend Layout (`*_split_ex*`) for new constraints/options.
- [ ] Extend Terminal helpers if Ratatui added APIs.
- [ ] Extend Chart/Canvas/Symbols as needed.
- [ ] Wire batched frame + headless frame renderers.
- [ ] Rebuild + rerun introspection; verify all ✓.
- [ ] Snapshot headless: text, styles_ex, cells.
- [ ] Bump `ratatui_ffi` version + feature bits; update README.
- [ ] Produce introspector JSON for bindings and share the summary.

---

## 8. Notes for Ratatui 0.30+

- 0.30 split crates (style/layout/text/widgets/terminal). Adjust imports accordingly.
- Expect renames or moved types; rely on rustdoc JSON or our introspector’s doc scraping.
- Maintain additive API in FFI (prefer new `_ex` functions over breaking existing signatures).

Happy upgrading!

