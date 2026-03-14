#[cfg(feature = "ast")]
mod tree_walk;

fn main() {
    #[cfg(feature = "ast")]
    if let Err(err) = tree_walk::main() {
        eprintln!("\x1b[1;31mErrored with errors:\x1b[0m\n{err}")
    }
    #[cfg(not(feature = "bytecode"))]
    #[cfg(not(feature = "ast"))]
    println!("Compile with either ast or bytecode enabled")
}
