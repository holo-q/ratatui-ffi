#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cbindgen >/dev/null 2>&1; then
  echo "cbindgen not found. Install with: cargo install cbindgen" >&2
  exit 1
fi

mkdir -p include
cbindgen --config cbindgen.toml --crate ratatui_ffi --output include/ratatui_ffi.h
echo "Wrote include/ratatui_ffi.h"

