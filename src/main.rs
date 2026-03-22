#[cfg(feature = "ast")]
use std::env;
use std::path::PathBuf;

mod std_lib;
#[cfg(feature = "ast")]
mod tree_walk;

fn main() {
    #[cfg(feature = "break")]
    eprintln!("\x1b[1;31m[Warning!]\x1b[0m Break feature not stabilized");
    #[cfg(feature = "ast")]
    if env::args().collect::<Vec<_>>().get(1) == Some(&"test".to_owned()) {
        test(env::args().collect::<Vec<_>>().get(2).unwrap_or(&"test".to_owned()));
    } else if let Err(err) = tree_walk::main(std_lib::lib()) {
        eprintln!("\x1b[1;31mErrored with errors:\x1b[0m\n{err}")
    }

    #[cfg(not(any(feature = "bytecode", feature = "ast")))]
    compile_error!("Compile with either ast or bytecode enabled")
}

fn run_test(script_path: PathBuf) {
    match tree_walk::run_file(&script_path, std_lib::lib()) {
        Ok(_) => println!("OK"),
        Err(e) => println!("{}: {e}", script_path.to_string_lossy()), // format e to include line/column
    }
}

fn test(dir: &String) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        run_test(path);
    }
}

