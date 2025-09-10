use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn read_file(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn extract_source_exports(src: &str) -> Vec<String> {
    // Simple state machine: when we see #[no_mangle], look ahead for extern "C" fn NAME(
    let mut out = Vec::new();
    let mut lines = src.lines();
    let mut pending = false;
    while let Some(line) = lines.next() {
        let l = line.trim();
        if l.starts_with("#[no_mangle]") {
            pending = true;
            continue;
        }
        if pending {
            let s = l;
            if let Some(idx) = s.find("extern \"C\" fn ") {
                let rest = &s[idx + "extern \"C\" fn ".len()..];
                let name: String = rest.chars().take_while(|&c| c == '_' || c.is_ascii_alphanumeric()).collect();
                if !name.is_empty() {
                    out.push(name);
                }
                pending = false;
            } else {
                // If not on this line, try the next non-empty line
                if let Some(next) = lines.next() {
                    let s2 = next.trim();
                    if let Some(idx2) = s2.find("extern \"C\" fn ") {
                        let rest = &s2[idx2 + "extern \"C\" fn ".len()..];
                        let name: String = rest.chars().take_while(|&c| c == '_' || c.is_ascii_alphanumeric()).collect();
                        if !name.is_empty() {
                            out.push(name);
                        }
                    }
                }
                pending = false;
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn find_library(root: &Path) -> Option<PathBuf> {
    let candidates = [
        root.join("target/release/libratatui_ffi.so"),
        root.join("target/release/libratatui_ffi.dylib"),
        root.join("target/debug/libratatui_ffi.so"),
        root.join("target/debug/libratatui_ffi.dylib"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

fn find_in_path(name: &str) -> Option<String> {
    if let Ok(path) = env::var("PATH") {
        #[cfg(windows)]
        let sep = ';';
        #[cfg(not(windows))]
        let sep = ':';
        for dir in path.split(sep) {
            let p = Path::new(dir).join(name);
            if p.exists() {
                return Some(p.to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn nm_cmd() -> Option<String> {
    find_in_path("llvm-nm").or_else(|| find_in_path("nm"))
}

fn extract_binary_exports(lib: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Some(nm) = nm_cmd() else { return out; };
    let os = env::consts::OS;
    let lib_str = lib.to_string_lossy().to_string();
    let args: Vec<String> = if os == "macos" {
        vec!["-gUj".into(), lib_str.clone()]
    } else {
        vec!["-D".into(), "--defined-only".into(), lib_str.clone()]
    };
    let output = Command::new(nm).args(&args).output();
    if let Ok(o) = output {
        if o.status.success() {
            let text = String::from_utf8_lossy(&o.stdout);
            for line in text.lines() {
                // linux: "0000000000001c50 T ratatui_init_terminal" => last token
                // macos: just names with -j
                let name = if os == "macos" {
                    line.trim().to_string()
                } else {
                    line.split_whitespace().last().unwrap_or("").to_string()
                };
                if name.starts_with("ratatui_") {
                    out.push(name);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn group_key(name: &str) -> String {
    let rest = name.trim_start_matches("ratatui_");
    rest.split('_').next().unwrap_or("").to_string()
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let json = args.get(1).map(|s| s.as_str()) == Some("--json");
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()));
    let src_path = root.join("src/lib.rs");
    let src = read_file(&src_path);
    let src_exports = extract_source_exports(&src);
    let lib = find_library(&root);
    let bin_exports = lib.as_ref().map(|p| extract_binary_exports(p)).unwrap_or_default();

    let src_set: BTreeSet<_> = src_exports.iter().cloned().collect();
    let bin_set: BTreeSet<_> = bin_exports.iter().cloned().collect();
    let src_only: Vec<_> = src_set.difference(&bin_set).cloned().collect();
    let bin_only: Vec<_> = bin_set.difference(&src_set).cloned().collect();

    let mut g_src: BTreeMap<String, usize> = BTreeMap::new();
    for f in &src_exports { *g_src.entry(group_key(f)).or_default() += 1; }
    let mut g_bin: BTreeMap<String, usize> = BTreeMap::new();
    for f in &bin_exports { *g_bin.entry(group_key(f)).or_default() += 1; }

    if json { println!("--json not implemented"); return; }

    println!("== ratatui_ffi exports ==");
    println!("Functions (source): {}", src_exports.len());
    if let Some(p) = &lib { println!("Library: {}", p.to_string_lossy()); }
    if !bin_exports.is_empty() { println!("Functions (binary): {}", bin_exports.len()); }
    if !src_only.is_empty() {
        println!("\nSource-only (not in binary):");
        for f in &src_only { println!("  {}", f); }
    }
    if !bin_only.is_empty() {
        println!("\nBinary-only (not in source):");
        for f in &bin_only { println!("  {}", f); }
    }
    println!("\nGroups (by prefix)");
    println!("  Source:");
    for (k, v) in &g_src { println!("    {:<14} {}", k, v); }
    if !g_bin.is_empty() {
        println!("  Binary:");
        for (k, v) in &g_bin { println!("    {:<14} {}", k, v); }
    }

    // Optional: compare against ratatui docs to spot missing widget families
    let doc_widgets = root.join("target/doc/ratatui/widgets/sidebar-items.js");
    if doc_widgets.exists() {
        println!("\nRatatuí widgets coverage (from docs):");
        if let Ok(s) = fs::read_to_string(&doc_widgets) {
            // very simple extract of struct names: find '"struct":["A","B",...]'
            let mut structs: Vec<String> = Vec::new();
            if let Some(i) = s.find("\"struct\":") {
                if let Some(start) = s[i..].find('[') {
                    let start_idx = i + start + 1;
                    if let Some(end_rel) = s[start_idx..].find(']') {
                        let list = &s[start_idx..start_idx+end_rel];
                        for part in list.split(',') {
                            let part = part.trim();
                            if part.starts_with('\"') && part.ends_with('\"') {
                                let name = part.trim_matches('"').to_string();
                                if !name.is_empty() { structs.push(name); }
                            }
                        }
                    }
                }
            }
            structs.sort();
            structs.dedup();
            // For each widget struct, expect FFI group prefix lowercased name
            for w in structs {
                let key = w.to_lowercase();
                let has = g_src.contains_key(&key);
                println!("  {:<14} {}", w, if has { "✓" } else { "✗" });
            }
        }
    } else {
        println!("\n(ratatui docs not found; run `cargo doc -p ratatui` to enable widget coverage check)");
    }
}
