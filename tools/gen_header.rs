use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Generate include/ratatui_ffi.h using cbindgen CLI to avoid adding build deps.
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = repo_root.join("cbindgen.toml");
    let out_dir = repo_root.join("include");
    let out = out_dir.join("ratatui_ffi.h");

    std::fs::create_dir_all(&out_dir).expect("create include/ dir");

    let mut cmd = Command::new("cbindgen");
    cmd.current_dir(&repo_root)
        .arg("--config")
        .arg(cfg)
        .arg("--crate")
        .arg("ratatui_ffi")
        .arg("--output")
        .arg(&out);

    let status = cmd
        .status()
        .expect("failed to spawn cbindgen (is it installed?)");
    if !status.success() {
        eprintln!("cbindgen exited with status: {}", status);
        std::process::exit(status.code().unwrap_or(1));
    }
    println!("Wrote {}", out.display());
}
