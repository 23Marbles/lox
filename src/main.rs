mod std_lib;
#[cfg(feature = "ast")]
mod tree_walk;

fn main() {
    #[cfg(feature = "break")]
    eprintln!("\x1b[1;31m[Warning!]\x1b[0m Break feature not stabilized");
    #[cfg(feature = "ast")]
    if let Err(err) = tree_walk::main(std_lib::lib()) {
        eprintln!("\x1b[1;31mErrored with errors:\x1b[0m\n{err}")
    }
    #[cfg(not(feature = "bytecode"))]
    #[cfg(not(feature = "ast"))]
    eprintln!("Compile with either ast or bytecode enabled")
}
