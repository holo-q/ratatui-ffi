use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anstyle::{AnsiColor, Style};
use std::sync::OnceLock;
// indicatif previously used for spinners; keep clean output now.

fn style_header() -> Style {
    Style::new()
        .bold()
        .fg_color(Some(AnsiColor::Cyan.into()))
}
fn style_ok() -> Style {
    Style::new().fg_color(Some(AnsiColor::Green.into())).bold()
}
fn style_warn() -> Style {
    Style::new().fg_color(Some(AnsiColor::Yellow.into()))
}
fn style_err() -> Style {
    Style::new().fg_color(Some(AnsiColor::Red.into())).bold()
}
fn style_path() -> Style { Style::new().fg_color(Some(AnsiColor::BrightBlack.into())) }
fn style_name() -> Style { Style::new().bold().fg_color(Some(AnsiColor::BrightWhite.into())) }
fn style_group() -> Style { Style::new().fg_color(Some(AnsiColor::Blue.into())) }
fn style_module() -> Style { Style::new().fg_color(Some(AnsiColor::Magenta.into())) }
fn style_type() -> Style { Style::new().fg_color(Some(AnsiColor::Yellow.into())) }
fn style_map() -> Style { Style::new().fg_color(Some(AnsiColor::Green.into())) }
fn render(s: &Style, text: &str) -> String {
    format!("{s}{text}{}", Style::new().render_reset())
}

fn read_file(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}

fn extract_source_exports(src: &str) -> Vec<String> {
    // Simple state machine: when we see #[no_mangle], look ahead for extern "C" fn NAME(
    let mut out = Vec::new();
    let mut lines = src.lines();
    while let Some(line) = lines.next() {
        let l = line.trim();
        if l.starts_with("#[no_mangle]") {
            // scan ahead up to a few lines to find the extern signature (to skip cfg attributes)
            for _ in 0..6 {
                if let Some(next) = lines.next() {
                    let s2 = next.trim();
                    if let Some(idx2) = s2.find("extern \"C\" fn ") {
                        let rest = &s2[idx2 + "extern \"C\" fn ".len()..];
                        let name: String = rest
                            .chars()
                            .take_while(|&c| c == '_' || c.is_ascii_alphanumeric())
                            .collect();
                        if !name.is_empty() {
                            out.push(name);
                        }
                        break;
                    }
                }
            }
            continue;
        }
    }
    // Also accept known macro invocations that expand to extern "C" functions
    // e.g., crate::ratatui_block_title_alignment_fn!(ratatui_paragraph_set_block_title_alignment, FfiParagraph);
    let macro_pat = "ratatui_block_title_alignment_fn!(";
    let mut seek = 0usize;
    while let Some(idx) = src[seek..].find(macro_pat) {
        let start = seek + idx + macro_pat.len();
        let rest = &src[start..];
        // name is the next comma-separated token
        if let Some(end) = rest.find(',') {
            let name = rest[..end].trim();
            if !name.is_empty() {
                // strip possible namespace like crate::, though we expect a bare ident
                let nm = name.trim_start_matches("crate::").trim();
                out.push(nm.to_string());
            }
            seek = start + end;
        } else {
            break;
        }
    }
    // block_adv macro invocations
    let macro_pat2 = "ratatui_block_adv_fn!(";
    let mut seek2 = 0usize;
    while let Some(idx) = src[seek2..].find(macro_pat2) {
        let start = seek2 + idx + macro_pat2.len();
        let rest = &src[start..];
        if let Some(end) = rest.find(',') {
            let name = rest[..end].trim();
            if !name.is_empty() {
                let nm = name.trim_start_matches("crate::").trim();
                out.push(nm.to_string());
            }
            seek2 = start + end;
        } else { break; }
    }
    out.sort();
    out.dedup();
    out
}

fn strip_comments(s: &str) -> String {
    // Remove // line comments and /* */ block comments (naive but adequate for enums)
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0usize;
    let mut block_depth = 0usize;
    let mut in_line = false;
    while i < bytes.len() {
        let c = bytes[i] as char;
        let next = if i + 1 < bytes.len() { Some(bytes[i + 1] as char) } else { None };
        if block_depth > 0 {
            if c == '*' && next == Some('/') {
                block_depth -= 1;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_line {
            if c == '\n' { in_line = false; out.push(c); }
            i += 1;
            continue;
        }
        if c == '/' && next == Some('/') {
            in_line = true;
            i += 2;
            continue;
        }
        if c == '/' && next == Some('*') {
            block_depth += 1;
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn extract_enum_variants_from_source(src: &str, enum_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let needle = format!("pub enum {}", enum_name);
    if let Some(idx) = src.find(&needle) {
        let rest = &src[idx + needle.len()..];
        if let Some(start) = rest.find('{') {
            let mut depth = 1;
            let mut body = String::new();
            for ch in rest[start + 1..].chars() {
                match ch {
                    '{' => {
                        depth += 1;
                        body.push(ch);
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                        body.push(ch);
                    }
                    _ => body.push(ch),
                }
            }
            let body = strip_comments(&body);
            // Split by commas at top-level (not inside parentheses)
            let mut parts: Vec<String> = Vec::new();
            let mut cur = String::new();
            let mut depth_paren = 0i32;
            for ch in body.chars() {
                match ch {
                    '(' => { depth_paren += 1; cur.push(ch); }
                    ')' => { depth_paren -= 1; cur.push(ch); }
                    ',' if depth_paren == 0 => {
                        parts.push(cur.trim().to_string());
                        cur.clear();
                    }
                    _ => cur.push(ch),
                }
            }
            if !cur.trim().is_empty() { parts.push(cur.trim().to_string()); }
            for part in parts {
                let part = part.trim();
                if part.is_empty() || part.starts_with('#') {
                    continue;
                }
                let mut iter = part.split(|c: char| c == ' ' || c == '=' || c == '(');
                if let Some(name) = iter.next() {
                    if !name.is_empty() {
                        out.push(name.to_string());
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn collect_public_enums_with_variants(repo_src: &Path) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut stack = vec![repo_src.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if let Ok(read) = fs::read_dir(&path) {
                for entry in read.flatten() {
                    stack.push(entry.path());
                }
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            let mut off = 0usize;
            let bytes = text.as_bytes();
            while let Some(rel) = text[off..].find("pub enum ") {
                let start = off + rel + "pub enum ".len();
                // skip whitespace
                let mut i = start;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
                // capture name
                let mut j = i;
                while j < bytes.len() {
                    let ch = bytes[j] as char;
                    if ch.is_alphanumeric() || ch == '_' { j += 1; } else { break; }
                }
                if j > i {
                    if let Ok(name) = std::str::from_utf8(&bytes[i..j]) {
                        let variants = extract_enum_variants_from_source(&text, name);
                        if !variants.is_empty() {
                            out.entry(name.to_string()).or_default().extend(variants);
                        }
                    }
                }
                off = start;
            }
        }
    }
    for v in out.values_mut() { v.sort(); v.dedup(); }
    out
}

fn collect_ffi_enums_with_variants(ffi_src: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut off = 0usize;
    let bytes = ffi_src.as_bytes();
    while let Some(rel) = ffi_src[off..].find("pub enum Ffi") {
        let start = off + rel + "pub enum ".len();
        let mut i = start;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
        let mut j = i;
        while j < bytes.len() {
            let ch = bytes[j] as char;
            if ch.is_alphanumeric() || ch == '_' { j += 1; } else { break; }
        }
        if j > i {
            if let Ok(name) = std::str::from_utf8(&bytes[i..j]) {
                let variants = extract_enum_variants_from_source(ffi_src, name);
                out.insert(name.to_string(), variants);
            }
        }
        off = start;
    }
    out
}

fn ratatui_version_from_lock(lock_path: &Path) -> Option<String> {
    let data = fs::read_to_string(lock_path).ok()?;
    let mut in_package = false;
    let mut saw_name = false;
    for line in data.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            in_package = true;
            saw_name = false;
            continue;
        }
        if !in_package {
            continue;
        }
        if line.is_empty() || line.starts_with('[') {
            in_package = false;
            continue;
        }
        if line.starts_with("name = \"ratatui\"") {
            saw_name = true;
            continue;
        }
        if saw_name && line.starts_with("version = ") {
            let v = line.trim_start_matches("version = ").trim();
            return Some(v.trim_matches('"').to_string());
        }
    }
    None
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let base = env::temp_dir();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    base.join(format!("{}-{}-{}", prefix, std::process::id(), ts))
}

fn clone_ratatui(tag: &str) -> Option<PathBuf> {
    let dest = unique_temp_dir("ratatui-src");
    if fs::create_dir(&dest).is_err() {
        return None;
    }
    let status = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg("--branch")
        .arg(tag)
        .arg("https://github.com/ratatui-org/ratatui.git")
        .arg(&dest)
        .status()
        .ok()?;
    if status.success() {
        Some(dest)
    } else {
        let _ = fs::remove_dir_all(&dest);
        None
    }
}

struct TargetRepo {
    path: PathBuf,
    cleanup: bool,
}

fn clone_git_into(url: &str, rev: &str, dest: &Path) -> bool {
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let status = Command::new("git")
        .arg("clone")
        .arg("--depth").arg("1")
        .arg("--branch").arg(rev)
        .arg(url)
        .arg(dest)
        .status();
    match status {
        Ok(s) if s.success() => true,
        _ => { let _ = fs::remove_dir_all(dest); false }
    }
}

fn ensure_target_repo(root: &Path, cli_src: Option<PathBuf>, cli_git: Option<(String, String)>) -> Option<TargetRepo> {
    if let Some(p) = cli_src {
        if p.exists() { return Some(TargetRepo { path: p, cleanup: false }); }
    }
    if let Ok(override_path) = env::var("FFI_INTROSPECT_SRC_PATH").or_else(|_| env::var("RATATUI_SRC_PATH")) {
        let pb = PathBuf::from(override_path);
        if pb.exists() {
            return Some(TargetRepo {
                path: pb,
                cleanup: false,
            });
        }
    }
    if let Some((url, tag)) = cli_git {
        let repo_name = url.split('/').last().and_then(|s| s.strip_suffix(".git").or(Some(s))).unwrap_or("repo");
        let cache_path = root.join("target/src-cache").join(repo_name).join(&tag);
        if cache_path.join(".git").exists() {
            return Some(TargetRepo { path: cache_path, cleanup: false });
        }
        if cache_path.exists() { let _ = fs::remove_dir_all(&cache_path); }
        if clone_git_into(&url, &tag, &cache_path) {
            return Some(TargetRepo { path: cache_path, cleanup: false });
        }
        return None;
    }

    let lock_path = root.join("Cargo.lock");
    let version = ratatui_version_from_lock(&lock_path)?;
    let tag = format!("v{}", version);
    let cache_base = root.join("target/ratatui-src");
    let cache_path = cache_base.join(&tag);
    if cache_path.join(".git").exists() {
        return Some(TargetRepo {
            path: cache_path,
            cleanup: false,
        });
    }
    if cache_path.exists() {
        let _ = fs::remove_dir_all(&cache_path);
    }
    if clone_git_into("https://github.com/ratatui-org/ratatui.git", &tag, &cache_path) {
        return Some(TargetRepo {
            path: cache_path,
            cleanup: false,
        });
    }

    clone_ratatui(&tag).map(|p| TargetRepo {
        path: p,
        cleanup: true,
    })
}

fn find_ratatui_item_source(repo: &Path, kind: &str, name: &str) -> Option<(PathBuf, String)> {
    let needle = format!("pub {} {}", kind, name);
    let mut stack = vec![repo.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(&path) {
                for entry in entries.flatten() {
                    stack.push(entry.path());
                }
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            if text.contains(&needle) {
                return Some((path, text));
            }
        }
    }
    None
}

fn collect_pub_items(repo: &Path, subdir: &str, kind: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let base = repo.join(subdir);
    let pattern = format!("pub {} ", kind);
    let mut stack = vec![base];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if let Ok(read) = fs::read_dir(&path) {
                for entry in read.flatten() {
                    stack.push(entry.path());
                }
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            let mut offset = 0;
            let bytes = text.as_bytes();
            while let Some(rel) = text[offset..].find(&pattern) {
                let start = offset + rel + pattern.len();
                let mut idx = start;
                while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
                    idx += 1;
                }
                let mut end = idx;
                while end < bytes.len() {
                    let ch = bytes[end] as char;
                    if ch.is_alphanumeric() || ch == '_' {
                        end += 1;
                    } else {
                        break;
                    }
                }
                if end > idx {
                    if let Ok(name) = std::str::from_utf8(&bytes[idx..end]) {
                        out.insert(name.to_string());
                    }
                }
                offset = start;
            }
        }
    }
    out
}

fn ratatui_widget_structs(repo: &Path) -> BTreeSet<String> {
    collect_pub_items(repo, "src/widgets", "struct")
}

fn ratatui_widget_enums(repo: &Path) -> BTreeSet<String> {
    collect_pub_items(repo, "src/widgets", "enum")
}

fn collect_pub_items_detailed(repo: &Path, subdir: &str, kind: &str) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let base = repo.join(subdir);
    let pattern = format!("pub {} ", kind);
    // Collect files sorted for deterministic scope/file order
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![base.clone()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if let Ok(read) = fs::read_dir(&path) {
                for entry in read.flatten() { stack.push(entry.path()); }
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files.sort();
    for path in files {
        if let Ok(text) = fs::read_to_string(&path) {
            let mut offset = 0usize;
            let bytes = text.as_bytes();
            while let Some(rel) = text[offset..].find(&pattern) {
                let start = offset + rel + pattern.len();
                let mut i = start;
                while i < bytes.len() && bytes[i].is_ascii_whitespace() { i += 1; }
                let mut j = i;
                while j < bytes.len() {
                    let ch = bytes[j] as char;
                    if ch.is_alphanumeric() || ch == '_' { j += 1; } else { break; }
                }
                if j > i {
                    if let Ok(name) = std::str::from_utf8(&bytes[i..j]) {
                        out.push((name.to_string(), path.clone()));
                    }
                }
                offset = start;
            }
        }
    }
    out
}

fn collect_public_enums_with_variants_detailed(src_dir: &Path) -> Vec<(String, Vec<String>, PathBuf)> {
    let mut out: Vec<(String, Vec<String>, PathBuf)> = Vec::new();
    // Gather files sorted for deterministic order
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![src_dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if let Ok(read) = fs::read_dir(&path) {
                for entry in read.flatten() { stack.push(entry.path()); }
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files.sort();
    for path in files {
        if let Ok(text) = fs::read_to_string(&path) {
            let mut off = 0usize;
            while let Some(rel) = text[off..].find("pub enum ") {
                let start = off + rel + "pub enum ".len();
                let mut i = start;
                while i < text.len() && text.as_bytes()[i].is_ascii_whitespace() { i += 1; }
                let mut j = i;
                while j < text.len() {
                    let ch = text.as_bytes()[j] as char;
                    if ch.is_alphanumeric() || ch == '_' { j += 1; } else { break; }
                }
                if j > i {
                    let name = text[i..j].to_string();
                    let variants = extract_enum_variants_from_source(&text, &name);
                    out.push((name, variants, path.clone()));
                }
                off = start;
            }
        }
    }
    out
}

fn scope_from(base_src: &Path, file: &Path) -> String {
    let rel = file.strip_prefix(base_src).unwrap_or(file);
    let scope = rel.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    if scope.is_empty() { "src".to_string() } else { scope }
}

fn ratatui_public_structs(repo: &Path) -> BTreeSet<String> {
    collect_pub_items(repo, "src", "struct")
}

fn ratatui_public_traits(repo: &Path) -> BTreeSet<String> {
    collect_pub_items(repo, "src", "trait")
}

fn ratatui_public_functions(repo: &Path) -> BTreeSet<String> {
    collect_pub_items(repo, "src", "fn")
}

fn ratatui_public_types(repo: &Path) -> BTreeSet<String> {
    collect_pub_items(repo, "src", "type")
}

#[derive(Debug, Clone)]
struct PubConst {
    name: String,
    type_sig: Option<String>,
    value_snippet: Option<String>,
    file: PathBuf,
    module_key: String,
}

fn collect_public_consts_detailed(repo: &Path) -> Vec<PubConst> {
    let mut out: Vec<PubConst> = Vec::new();
    let base = repo.join("src");
    // Collect and sort files lexicographically for deterministic order
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![base.clone()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            if let Ok(read) = fs::read_dir(&path) {
                for entry in read.flatten() { stack.push(entry.path()); }
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files.sort();
    for path in files {
        let Ok(text) = fs::read_to_string(&path) else { continue; };
        let bytes = text.as_bytes();
        let mut off = 0usize;
        while let Some(rel) = text[off..].find("pub const ") {
            let start = off + rel + "pub const ".len();
            let mut i = start;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            // Skip const fn definitions
            if i + 2 <= bytes.len() {
                let head = &text[i..bytes.len().min(i + 8)];
                if head.starts_with("fn") && head[2..].chars().next().map(|c| !c.is_alphanumeric() && c != '_').unwrap_or(true) {
                    off = start;
                    continue;
                }
            }
            let mut j = i;
            while j < bytes.len() {
                let ch = bytes[j] as char;
                if ch.is_alphanumeric() || ch == '_' { j += 1; } else { break; }
            }
            if j <= i { off = start; continue; }
            let name = std::str::from_utf8(&bytes[i..j]).unwrap_or("").to_string();

            // Parse declaration until the next semicolon to avoid spanning into code blocks
            let rest = &text[j..];
            let semi_idx = match rest.find(';') { Some(v) => v, None => { off = start; continue; } };
            let decl = &rest[..semi_idx];
            let decl_nc = strip_comments(decl);
            // Collect optional type between ':' and '=' or end
            let mut type_sig: Option<String> = None;
            if let Some(colon_pos) = decl_nc.find(':') {
                let after_colon = &decl_nc[colon_pos + 1..];
                let end_pos = after_colon.find('=').unwrap_or(after_colon.len());
                let t = after_colon[..end_pos].trim();
                if !t.is_empty() { type_sig = Some(t.to_string()); }
            }
            // Optionally capture a small value snippet after '=' up to semicolon
            let mut value_snippet: Option<String> = None;
            if let Some(eq_pos) = decl_nc.find('=') {
                let v = decl_nc[eq_pos + 1..].trim();
                if !v.is_empty() {
                    let snippet = if v.len() > 80 { format!("{} …", &v[..80]) } else { v.to_string() };
                    value_snippet = Some(snippet);
                }
            }

            let rel = path.strip_prefix(&base).unwrap_or(&path);
            let module_key = rel.parent()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or_else(|| rel.file_stem().and_then(|s| s.to_str()).unwrap_or(""))
                .to_string();

            out.push(PubConst { name, type_sig, value_snippet, file: path.clone(), module_key });
            off = start;
        }
    }
    out
}

fn to_snake_case(name: &str) -> String {
    if name.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(name.len() * 2);
    let mut prev_is_lower = false;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            if prev_is_lower {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_is_lower = false;
        } else {
            prev_is_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
            out.push(ch);
        }
    }
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
    let Some(nm) = nm_cmd() else {
        return out;
    };
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

static GROUP_TRIM_PREFIX: OnceLock<String> = OnceLock::new();
fn group_key(name: &str) -> String {
    // Try to trim a known crate prefix like "ratatui_" to show logical groups
    let trim = GROUP_TRIM_PREFIX.get().map(|s| s.as_str()).unwrap_or("");
    let n = if !trim.is_empty() && name.starts_with(trim) {
        &name[trim.len()..]
    } else {
        name
    };
    n.split('_').next().unwrap_or("").to_string()
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
    // Parse flags: --json, --src PATH, --git URL --tag TAG
    let mut i = 1usize;
    let mut json = false;
    let mut cli_src: Option<PathBuf> = None;
    let mut cli_git: Option<(String, String)> = None;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => { json = true; i += 1; }
            "--src" if i + 1 < args.len() => { cli_src = Some(PathBuf::from(&args[i + 1])); i += 2; }
            "--git" if i + 3 < args.len() && args[i + 2].as_str() == "--tag" => {
                cli_git = Some((args[i + 1].clone(), args[i + 3].clone()));
                i += 4;
            }
            _ => { i += 1; }
        }
    }
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()));
    // Configure grouping and naming conventions
    let trim_prefix = env::var("FFI_INTROSPECT_TRIM_PREFIX").unwrap_or_else(|_| "ratatui_".to_string());
    let _ = GROUP_TRIM_PREFIX.set(trim_prefix);
    let src_path = root.join("src/lib.rs");
    let src = read_file(&src_path);
    let src_exports = extract_source_exports(&src);
    let lib = find_library(&root);
    let bin_exports = lib
        .as_ref()
        .map(|p| extract_binary_exports(p))
        .unwrap_or_default();

    // Prepare target sources (clone or reuse cache)
    let rat_repo = ensure_target_repo(&root, cli_src, cli_git);

    let _src_set: BTreeSet<_> = src_exports.iter().cloned().collect();
    let bin_set: BTreeSet<_> = bin_exports.iter().cloned().collect();

    let mut g_src: BTreeMap<String, usize> = BTreeMap::new();
    for f in &src_exports {
        *g_src.entry(group_key(f)).or_default() += 1;
    }
    let mut g_bin: BTreeMap<String, usize> = BTreeMap::new();
    for f in &bin_exports {
        *g_bin.entry(group_key(f)).or_default() += 1;
    }

    if json {
        // No separate modes; keep output human-friendly, single run prints everything
        // but JSON request gets a short notice for now.
        println!("JSON output not implemented; use plain output.");
        return;
    }

    // Pretty, concise, grouped by type then scope
    println!("{}", render(&style_header(), "== FFI Coverage =="));
    if let Some(repo) = rat_repo.as_ref() {
        println!("Target: {}", render(&style_path(), &repo.path.display().to_string()));
    }
    println!(
        "FFI functions: {} (binary: {})",
        render(&style_name(), &src_exports.len().to_string()),
        render(&style_name(), &bin_exports.len().to_string())
    );

    // FFI functions (green if present in binary, red otherwise)
    println!("\n{}", render(&style_header(), "FFI Functions"));
    // Group by function group prefix
    let mut by_group: BTreeMap<String, Vec<&String>> = BTreeMap::new();
    for f in &src_exports { by_group.entry(group_key(f)).or_default().push(f); }
    for (grp, mut items) in by_group {
        items.sort();
        println!("  [{}]", render(&style_group(), &grp));
        for f in items {
            let ok = bin_set.contains(f);
            let mark = if ok { render(&style_ok(), "✔") } else { render(&style_err(), "✘") };
            println!("    {} {}", mark, render(&style_name(), f));
        }
    }

    if let Some(repo) = rat_repo.as_ref() {
        // Structs grouped by scope (directory under src)
        println!("\n{}", render(&style_header(), "Structs"));
        let structs = collect_pub_items_detailed(&repo.path, "src", "struct");
        let base_src = repo.path.join("src");
        let mut by_scope: BTreeMap<String, Vec<(String, PathBuf)>> = BTreeMap::new();
        for (name, file) in structs { by_scope.entry(scope_from(&base_src, &file)).or_default().push((name, file)); }
        for (scope, items) in by_scope {
            println!("  [{}]", render(&style_module(), &scope));
            for (sname, _file) in items {
                println!("    {}", render(&style_name(), &sname));
            }
        }

        // Enums
        println!("\n{}", render(&style_header(), "Enums"));
        let rat_enums_d = collect_public_enums_with_variants_detailed(&repo.path.join("src"));
        let ffi_enums = collect_ffi_enums_with_variants(&src);
        let base_src = repo.path.join("src");
        let mut by_scope: BTreeMap<String, Vec<(String, Vec<String>, PathBuf)>> = BTreeMap::new();
        for (name, vars, file) in rat_enums_d { by_scope.entry(scope_from(&base_src, &file)).or_default().push((name, vars, file)); }
        for (scope, items) in by_scope {
            println!("  [{}]", render(&style_module(), &scope));
            for (rat_name, rat_variants, _file) in items {
                let ffi_name = format!("Ffi{}", rat_name);
                if let Some(ffi_variants) = ffi_enums.get(&ffi_name) {
                    let rset: BTreeSet<_> = rat_variants.iter().cloned().collect();
                    let fset: BTreeSet<_> = ffi_variants.iter().cloned().collect();
                    let missing: Vec<_> = rset.difference(&fset).cloned().collect();
                    let ok = missing.is_empty();
                    let mark = if ok { render(&style_ok(), "✔") } else { render(&style_err(), "✘") };
                    if ok {
                        println!("    {} {}  [mapped {}]", mark, render(&style_name(), &rat_name), render(&style_map(), &ffi_name));
                    } else {
                        let miss = missing.iter().map(|v| render(&style_err(), v)).collect::<Vec<_>>().join(", ");
                        println!("    {} {}  [mapped {}] missing: {}", mark, render(&style_name(), &rat_name), render(&style_map(), &ffi_name), miss);
                    }
                } else {
                    println!("    {} {}  [no FFI enum]", render(&style_err(), "✘"), render(&style_name(), &rat_name));
                }
            }
        }

        // Consts (generic): show origin and mapping suggestion
        println!("\n{}", render(&style_header(), "Consts"));
        let base_src = repo.path.join("src");
        let mut by_scope: BTreeMap<String, BTreeMap<PathBuf, Vec<PubConst>>> = BTreeMap::new();
        for c in collect_public_consts_detailed(&repo.path) {
            let scope = scope_from(&base_src, &c.file);
            by_scope.entry(scope).or_default().entry(c.file.clone()).or_default().push(c);
        }
        for (scope, files) in by_scope {
            println!("  [{}]", render(&style_module(), &scope));
            for (_file, items) in files {
                for c in items {
                    let rel = c.file.strip_prefix(&repo.path).unwrap_or(&c.file);
                    let mut decl = render(&style_name(), &c.name);
                    if let Some(t) = &c.type_sig { decl.push_str(&format!(": {}", render(&style_type(), t))); }
                    let def_prefix = env::var("FFI_INTROSPECT_DEFINE_PREFIX").unwrap_or_else(|_| "RATATUI".into());
                    let get_prefix = env::var("FFI_INTROSPECT_GETTER_PREFIX").unwrap_or_else(|_| "ratatui".into());
                    let define = format!("{}_{}_{}", def_prefix, c.module_key.to_uppercase(), c.name.to_uppercase());
                    let getter = format!("{}_{}_get_{}", get_prefix, to_snake_case(&c.module_key), to_snake_case(&c.name));
                    println!("    {}  ({}) -> define {}, getter {}()", decl, render(&style_path(), &rel.display().to_string()), render(&style_map(), &define), render(&style_map(), &getter));
                }
            }
        }
    }

    if let Some(repo) = rat_repo {
        if repo.cleanup {
            let _ = fs::remove_dir_all(repo.path);
        }
    }
}
