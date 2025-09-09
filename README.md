# ratatui_ffi

![CI](https://github.com/holo-q/ratatui-ffi/actions/workflows/ci.yml/badge.svg)
[![crates.io](https://img.shields.io/crates/v/ratatui_ffi.svg?logo=rust&label=crates.io)](https://crates.io/crates/ratatui_ffi)
[![crates.io downloads](https://img.shields.io/crates/d/ratatui_ffi.svg?logo=rust)](https://crates.io/crates/ratatui_ffi)

Native C ABI for [Ratatui], exposing a small cdylib you can consume from C, C#, and other languages.

Status
- Targets the Ratatui workspace API (0.30 beta series). The crate currently depends on the workspace layout, which is why it has a path dependency to `ratatui`.
- If you want to build against crates.io instead, switch the dependency to a crates.io version (see below).

Build
```bash
cargo build --release
# produces target/release/libratatui_ffi.so (Linux), .dylib (macOS), or ratatui_ffi.dll (Windows)
```

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
