use std::{
    env,
    error::Error,
    fs::File,
    io::{Read, Write, stdin, stdout},
    path::Path,
    process::exit,
};

use crate::tree_walk::{
    error::{LoxTreeError, RunError},
    token::Scanner,
};
mod error;
mod token;

pub fn main() -> Result<(), RunError> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 2 {
        println!("usage: rlox [script]");
        exit(64)
    } else if args.len() == 2 {
        run_file(Path::new(&args[1]))
    } else {
        run_prompt()
    }
}

fn run_file(file_path: &Path) -> Result<(), RunError> {
    #[cfg(debug_assertions)]
    println!("script opened at {:?}", file_path);

    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    run(&contents)
}

fn run_prompt() -> Result<(), RunError> {
    let mut line_buf = String::new();

    loop {
        print!("> ");
        stdout().flush()?;
        if stdin().read_line(&mut line_buf)? == 0 {
            break Ok(());
        }
        let err = run(&line_buf);
        if let Err(e) = err {
            println!("{e}");
        }
        line_buf.clear();
    }
}

fn run(source: &str) -> Result<(), RunError> {
    let scanner = Scanner::new(source);

    let mut errors = Vec::new();

    for token in scanner {
        match token {
            Ok(t) => println!("{t:?}"),
            Err(e) => errors.push(e),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.into())
    }
}
