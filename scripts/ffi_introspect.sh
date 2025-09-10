#!/usr/bin/env bash
set -euo pipefail

# Portable FFI export introspection (no hardcoded capabilities, no files).
# - Lists source-declared exports (#[no_mangle] extern "C").
# - Lists binary exports from the built library if present.
# - Reports mismatches and per-prefix group counts.
# - Optional: --json to emit machine-readable JSON.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
SRC_DIR="$ROOT_DIR/src"
JSON=0

if [[ ${1:-} == "--json" ]]; then JSON=1; fi

if [[ ! -d "$SRC_DIR" ]]; then
  echo "error: cannot find src dir at $SRC_DIR" >&2
  exit 2
fi

extract_from_src() {
  # Scan all Rust sources under src/ for #[no_mangle] extern "C" fn symbols.
  # Allow up to a few lookahead lines to skip cfg attributes, etc.
  awk '
    FNR==1 { state=0 }
    /#\[no_mangle\]/ { state=6; next }
    state>0 {
      if ($0 ~ /extern "C" fn [A-Za-z0-9_]+\(/) {
        match($0, /extern "C" fn ([A-Za-z0-9_]+)/, m);
        if (m[1] != "") print m[1];
        state=0; next
      }
      state--
    }
  ' $(find "$SRC_DIR" -type f -name '*.rs' | sort) | sort -u
}

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

# prefer llvm-nm if available
if command -v llvm-nm >/dev/null 2>&1; then alias nm=llvm-nm; fi

mapfile -t SRC_FUNCS < <(extract_from_src)
declare -A SRC_SET=()
for f in "${SRC_FUNCS[@]}"; do SRC_SET["$f"]=1; done

# locate built library
LIB=""
for cand in \
  "$ROOT_DIR/target/release/libratatui_ffi.so" \
  "$ROOT_DIR/target/release/libratatui_ffi.dylib" \
  "$ROOT_DIR/target/debug/libratatui_ffi.so" \
  "$ROOT_DIR/target/debug/libratatui_ffi.dylib"; do
  [[ -f "$cand" ]] && LIB="$cand" && break
done

BIN_FUNCS=()
if command -v nm >/dev/null 2>&1 && [[ -n "$LIB" ]]; then
  while IFS= read -r line; do BIN_FUNCS+=("$line"); done < <(extract_from_lib "$LIB")
fi
declare -A BIN_SET=()
for f in "${BIN_FUNCS[@]}"; do BIN_SET["$f"]=1; done

# compute groups by prefix after ratatui_
group_key() {
  local name="$1"
  local rest="${name#ratatui_}"
  if [[ "$rest" == headless_render_* ]]; then
    echo "headless"
  else
    echo "${rest%%_*}"
  fi
}

declare -A GROUP_SRC=()
declare -A GROUP_BIN=()
for f in "${SRC_FUNCS[@]}"; do k=$(group_key "$f"); GROUP_SRC["$k"]=$(( ${GROUP_SRC["$k"]:-0} + 1 )); done
for f in "${BIN_FUNCS[@]}"; do k=$(group_key "$f"); GROUP_BIN["$k"]=$(( ${GROUP_BIN["$k"]:-0} + 1 )); done

src_only=()
for f in "${SRC_FUNCS[@]}"; do [[ -n "${BIN_SET[$f]:-}" ]] || src_only+=("$f"); done
bin_only=()
for f in "${BIN_FUNCS[@]}"; do [[ -n "${SRC_SET[$f]:-}" ]] || bin_only+=("$f"); done

if (( JSON == 1 )); then
  printf '{"library":%s,"exports_source":[' "$([[ -n "$LIB" ]] && printf '"%s"' "$LIB" || printf 'null')"
  for i in "${!SRC_FUNCS[@]}"; do printf '%s"%s"' "$([[ $i -gt 0 ]] && echo ,)" "${SRC_FUNCS[$i]}"; done
  printf '],"exports_binary":['
  for i in "${!BIN_FUNCS[@]}"; do printf '%s"%s"' "$([[ $i -gt 0 ]] && echo ,)" "${BIN_FUNCS[$i]}"; done
  printf '],"mismatch":{"source_only":['
  for i in "${!src_only[@]}"; do printf '%s"%s"' "$([[ $i -gt 0 ]] && echo ,)" "${src_only[$i]}"; done
  printf '],"binary_only":['
  for i in "${!bin_only[@]}"; do printf '%s"%s"' "$([[ $i -gt 0 ]] && echo ,)" "${bin_only[$i]}"; done
  printf ']},"groups":{"source":{'
  i=0; for k in "${!GROUP_SRC[@]}"; do printf '%s"%s":%d' "$([[ $i -gt 0 ]] && echo ,)" "$k" "${GROUP_SRC[$k]}"; i=$((i+1)); done
  printf '},"binary":{'
  i=0; for k in "${!GROUP_BIN[@]}"; do printf '%s"%s":%d' "$([[ $i -gt 0 ]] && echo ,)" "$k" "${GROUP_BIN[$k]}"; i=$((i+1)); done
  printf '}}}\n'
  exit 0
fi

echo "== ratatui_ffi exports =="
echo "Functions (source): ${#SRC_FUNCS[@]}"
if [[ -n "$LIB" ]]; then echo "Library: $LIB"; fi
if (( ${#BIN_FUNCS[@]} > 0 )); then echo "Functions (binary): ${#BIN_FUNCS[@]}"; fi

if (( ${#src_only[@]} > 0 )); then
  echo "\nSource-only (not in binary):"
  for f in "${src_only[@]}"; do echo "  $f"; done
fi
if (( ${#bin_only[@]} > 0 )); then
  echo "\nBinary-only (not in source):"
  for f in "${bin_only[@]}"; do echo "  $f"; done
fi

echo "\nGroups (by prefix)"
echo "  Source:"
for k in "${!GROUP_SRC[@]}"; do printf '    %-14s %d\n' "$k" "${GROUP_SRC[$k]}"; done | sort
if (( ${#BIN_FUNCS[@]} > 0 )); then
  echo "  Binary:"
  for k in "${!GROUP_BIN[@]}"; do printf '    %-14s %d\n' "$k" "${GROUP_BIN[$k]}"; done | sort
fi
