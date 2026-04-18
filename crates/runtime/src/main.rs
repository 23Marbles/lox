fn main() {
    #[cfg(feature = "break")]
    eprintln!("\x1b[1;31m[Warning!]\x1b[0m Break feature not stabilized");
    #[cfg(feature = "ast")]
    lox_runtime::tree_walk::main(lox_runtime::std_lib::default_lib());

    //#[cfg(not(any(feature = "bytecode", feature = "ast")))]
    //compile_error!("Compile with either ast or bytecode enabled")
}
