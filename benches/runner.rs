use std::{
    fs,
    path::{Path, PathBuf},
};

use lox::{Runner, std_lib};

/// Recursively collects all .lox files in the given directory and its subdirectories.
fn recurse_build_check(dir: PathBuf) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Recursively collect from subdirectories
                paths.extend(recurse_build_check(path));
            } else if path.is_file()
                && let Some(ext) = path.extension()
            {
                match ext.to_str() {
                    Some("lox") => paths.push(path),
                    Some("ignore") => paths.push(path),
                    _ => {}
                }
            }
        }
    }

    paths
}

fn main() {
    for path in recurse_build_check(PathBuf::from("benches/scripts")) {
        bench(&path);
    }
}

fn bench(path: &Path) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap();
    println!("==== STARTING BENCH ====\n{}", name);
    Runner::new()
        .with_globals(std_lib::default_lib())
        .run_file(path)
        .unwrap();
}
