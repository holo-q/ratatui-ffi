# ratatui bindings codegen pipeline

One Rust FFI (`ratatui-ffi`) is the ABI truth. Three language bindings
(`Ratatui.cs`, `ratatui-py`, `ratatui-ts`) used to hand-maintain ~220 interop
declarations *each* — and drifted independently (TS ~14 behind, Py ~12 behind,
the C header never even generated). This pipeline kills that: **one typed IR,
emitted from the FFI source, that every binding generates its interop layer
from.** Hand-drift becomes structurally impossible; the only manual work left is
(1) authoring the C-ABI Rust shape for genuinely-new upstream widgets and (2)
ergonomic OO sugar — and the pipeline *reports* both as an explicit worklist.

```
ratatui-ffi/src/ffi/**  ──manifest-gen (nightly-expand + syn)──► ratatui-ffi/bindings.json  (the contract)
                                                      │
                  ┌───────────────────────────────────┼───────────────────────────────────┐
            rustbind (C# emitter)             ratatui-py/tools/gen_ffi.py        ratatui-ts/scripts/gen-native.js
            → src/Ratatui/Interop/Native.cs    → src/ratatui_py/_ffi.py           → src/native.ts
                                                      │
                          workgroup justfile: sync-upstream → manifest → gen → check → report
```

Sibling layout (org convention — clone the tree, path-deps stay):
`repo-com/ratatui/{ratatui-ffi, Ratatui.cs, ratatui-py, ratatui-ts}`. Each
emitter reads the manifest from the sibling: `../ratatui-ffi/bindings.json`.

## The IR contract — `bindings.json`

Emitted by **rustbind's `manifest-gen`** (`repo-tool/rustbind/manifest-gen`), a
crate-agnostic syn-based extractor (the generalized successor of the old local
`ratatui-ffi/codegen`, which was deleted — no duplicate generator). It is a
standalone crate, intentionally *not* a member of the published cdylib package,
so `syn` stays out of crates.io deps. Regenerate (from this dir) with:

```sh
cargo run --manifest-path ../../../repo-tool/rustbind/manifest-gen/Cargo.toml -- \
    --ffi-crate . --upstream-dep ratatui
```

The `--upstream-dep ratatui` flag is the (now parametric) pin that emits
`ratatui_version` into the IR — omit it for crates with no meaningful upstream pin.

### Source acquisition — why nightly expansion, not raw parsing

A mature FFI crate (this one: ~349 exports, v0.2.6) generates a large fraction
of its surface via `macro_rules!` (palette/symbol getters, widget
block/style/title setters) and splits the rest across a module tree
(`src/ffi/widgets/*.rs`, `terminal.rs`, …) plus `include!`d generated files.
`syn` expands **none** of that — `mod foo;`, `include!(…)`, and macro invocations
are all opaque to a raw-source parse, which therefore sees only the literal
`#[no_mangle]` fns (≈half the real ABI). So `manifest-gen` instead expands the
whole crate with nightly rustc (`cargo rustc --lib --profile check --
-Zunpretty=expanded`, nightly selected via `RUSTUP_TOOLCHAIN`): every module
inlined, every macro expanded, one parseable dump carrying the **complete** typed
surface. The one thing expansion *destroys* is `bitflags!` (it lowers to a plain
struct + impl-const block), so the four bitflags are harvested separately from
the raw source tree, where they remain literal macros. Same-named structs
surfaced from two module paths are deduped by name (loud-fail on layout
disagreement). Requires a nightly toolchain on PATH.

Top-level shape:

```json
{
  "schema": 1,
  "ffi_version": "0.2.1",
  "ratatui_version": "0.29",
  "functions":      [ IrFunction, ... ],   // sorted by name, deterministic
  "value_structs":  [ IrValueStruct, ... ],// repr(C) structs w/ full field layout
  "opaque_structs": [ "FfiTerminal", ... ],// handle types — only ever crossed as pointers
  "enums":          [ IrEnum, ... ],        // #[repr(uN)] enums w/ computed discriminants
  "bitflags":       [ IrEnum, ... ]         // bitflags! structs (same shape as enums)
}
```

### IrType — the language-neutral type encoding (tagged union on `kind`)

```
{ "kind": "prim",   "name": "u8|u16|u32|u64|usize|i8|i16|i32|i64|f32|f64|bool" }
{ "kind": "char" }                               // c_char — only ever under a ptr (UTF-8 byte)
{ "kind": "void" }                               // unit return, or void* pointee
{ "kind": "ptr", "mutable": bool, "elem": IrType }
{ "kind": "struct", "name": "FfiStyle", "opaque": bool }   // opaque=false ⇒ value-struct w/ layout
```

Recursion examples (read straight off the manifest):
- `*const c_char`  → `ptr(const, char)`
- `*mut *mut c_char` → `ptr(mut, ptr(mut, char))`
- `*mut FfiTerminal` → `ptr(mut, struct FfiTerminal opaque=true)`
- `*const FfiSpan` → `ptr(const, struct FfiSpan opaque=false)`
- `FfiStyle` (by value) → `struct FfiStyle opaque=false`
- `*const ()` (Rust void*) → `ptr(const, void)`

### IrFunction / IrValueStruct / IrEnum

```
IrFunction    { name, params:[{name,type:IrType}], ret:IrType,
                cfg_feature?: "scrollbar", doc?: "..." }
IrValueStruct { name, fields:[{name,type:IrType}] }      // field ORDER is the C layout — preserve it
IrEnum        { name, repr:"u32", variants:[{name,value:i64}] }
```

`cfg_feature` marks an export gated behind a Cargo feature. Emit it
unconditionally; the runtime `ratatui_ffi_feature_bits()` reports whether the
loaded library actually has it (see FfiFeatures bitflags).

## Per-language type mapping

The pointer policy is uniform per binding (every binding already treats all
pointers as one opaque pointer type — match that). Value-structs are passed
**by value** when the IrType is `struct(opaque=false)` directly, and as the
opaque pointer when under `ptr`.

| IrType            | C# (P/Invoke)                       | Python (ctypes)        | TS (ffi-napi/ref)        |
|-------------------|-------------------------------------|------------------------|--------------------------|
| prim u8           | `byte`                              | `c_ubyte`              | `'uint8'`                |
| prim u16          | `ushort`                            | `c_ushort`             | `'uint16'`               |
| prim u32          | `uint`                              | `c_uint`               | `'uint32'`               |
| prim u64          | `ulong`                             | `c_ulonglong`          | `'uint64'`               |
| prim usize        | `UIntPtr` (nuint)                   | `c_size_t`             | `'size_t'`               |
| prim i32          | `int`                               | `c_int`                | `'int32'`                |
| prim f32 / f64    | `float` / `double`                  | `c_float` / `c_double` | `'float'` / `'double'`   |
| prim bool         | `bool` + `[MarshalAs(I1)]`          | `c_bool`               | `'bool'`                 |
| ptr→char          | `IntPtr` (UTF-8 in/out; free w/ `ratatui_string_free`) | `c_char_p` | `ref.types.CString` |
| ptr→anything else | `IntPtr`                            | `c_void_p` (or `POINTER(Struct)` if you prefer typed) | `voidPtr` |
| void return       | `void`                              | (no restype / None)    | `'void'`                 |
| struct(value) by value | the generated struct type      | the generated Structure| the generated Struct     |

Match whatever the existing hand-written interop file already does where it is
ABI-correct; the generated file becomes canonical, so silent hand-drift in the
old file is *normalized away* (and reported, see below).

## Emitter responsibilities (every language)

1. **Generate the interop file in full** from the manifest: value-struct
   definitions (field order preserved), every function declaration, enum +
   bitflags named integer constants, and the ffi/ratatui version stamped in a
   header banner: `// GENERATED from bindings.json by <emitter> — DO NOT EDIT.
   Regenerate with `just gen`. ffi=<v> ratatui=<v>`.
2. **Prove parity** against the prior hand-written file: the generated symbol
   set must be a superset of the old one. Report, as three lists:
   - `added`   — fns the manifest has that the old file lacked (the drift gap we just closed)
   - `removed` — fns the old file had that the manifest lacks (investigate: stale? non-ratatui helper?)
   - `changed` — fns whose signature differs (investigate ABI correctness)
3. **Compile/load check**: build the binding (C#: `rk build`; Py: import +
   ctypes prototype load; TS: `npm run build` / tsc) so the regenerated file is
   proven to compile and the library loads.
4. **Residue report** — the manual-work worklist: list every FFI function that
   has **no ergonomic wrapper** in the hand-written OO layer (i.e. only appears
   in the generated interop file). Write it to `tools/residue.txt` (or
   `--report <path>`). This is the "fold all manual work into codegen down to
   the limit, and report the rest" directive made concrete.

The interop layer is 100% generated. The ergonomic OO layer is now *also*
generated for the regular patterns — see below.

## Stage 2 — ergonomic wrapper generation (fold the OO layer too)

The residue report (Stage 1) lists FFI fns with no ergonomic wrapper. Stage 2
shrinks that set by **generating idiomatic wrappers for the regular patterns**,
driven by the same manifest. The directive: fold manual work into codegen *down
to the limit* — and the limit is "would a human have written this wrapper this
way?" Prefer leaving a fn raw over emitting an awkward wrapper; a smaller, honest
residue beats a padded one.

### Grouping

Every export is `ratatui_<group>_<verb…>`. The `<group>` token maps to the
existing wrapper class/module for that widget (`paragraph` → `Paragraph`,
`table` → `Table`, …). Generated wrappers attach to that type **without
colliding** with hand-written members: C# `partial class` in a `*.Generated.cs`;
Python a generated mixin/`_generated.py` the hand class inherits or a monkey-patch
module; TS declaration-merging / a generated base. **Never overwrite a
hand-written method** — detect name collision and skip (the hand version wins),
and report each skip.

### Verb taxonomy (what to generate vs leave raw)

| FFI verb shape                       | Wrapper                                            |
|--------------------------------------|----------------------------------------------------|
| `_new` / `_new_empty` / `_new_*`     | constructor — usually hand-written already; skip if present |
| `_free`                              | disposal — owned by SafeHandle/`__del__`/registry; **never wrap** |
| `_set_<prop>(h, …)`                  | fluent setter returning self (the ratatui builder idiom), named `<prop>` |
| `_append_<x>` / `_add_<x>`           | `Append<X>` / `Add<X>` method                      |
| `headless_render_<group>(…)`         | static `RenderHeadless` helper on the widget (handy for tests) |
| `_reserve_*` / capacity hints        | low-level perf — **leave raw**, report             |
| `ratatui_color_*`, `string_free`, `ffi_version`, `feature_bits` | not widget-bound; case-by-case, mostly already have helpers |
| `_draw_in(term, rect, …)`            | drawing is orchestrated by Terminal/Frame; defer to existing draw machinery unless the hand layer clearly wants a per-widget `DrawIn` |

Each `_set_*`/`_append_*` wrapper marshals via the Stage-1 interop layer (build
the FfiSpan/array, call the extern, manage UTF-8 lifetime + `ratatui_string_free`
for out-strings). Mirror exactly how the hand-written wrappers already do this —
the hand idiom is the template; consistency with it is the quality bar.

### Wiring + reporting

- Wire generation into the repo's `just gen` so a single regen produces both the
  interop layer and the wrapper layer. `just gen` stays the one verb.
- After generation, the residue report splits into: **wrapped-now** (newly
  generated), **deliberately-raw** (free/reserve/internal — with a one-line
  reason each), and **still-unwrapped** (the genuine remaining worklist). The
  goal is `still-unwrapped → 0` with every exclusion justified, not a silently
  padded count.
