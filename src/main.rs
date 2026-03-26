fn main() {
    #[cfg(feature = "break")]
    eprintln!("\x1b[1;31m[Warning!]\x1b[0m Break feature not stabilized");
    #[cfg(feature = "ast")]
    if let Err(err) = lox::tree_walk::main(lox::std_lib::default_lib()) {
        eprintln!("\x1b[1;31mErrored with errors:\x1b[0m\n{err}")
    }
    #[cfg(not(feature = "bytecode"))]
    #[cfg(not(feature = "ast"))]
    eprintln!("Compile with either ast or bytecode enabled")
}
