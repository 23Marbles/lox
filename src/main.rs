mod tree_walk;

fn main() {
    if let Err(err) = tree_walk::main() {
        eprintln!("\x1b[1;31mErrored with errors:\x1b[0m\n{err}")
    }
}
