mod tree_walk;

fn main() {
    match tree_walk::main() {
        Err(err) => {
            println!("{err}")
        }
        Ok(_) => {}
    }
}
