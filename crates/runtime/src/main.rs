fn main() {
    #[cfg(feature = "ast")]
    lox_runtime::tree_walk::main(lox_runtime::std_lib::default_lib());

    //#[cfg(not(any(feature = "bytecode", feature = "ast")))]
    //compile_error!("Compile with either ast or bytecode enabled")
}
