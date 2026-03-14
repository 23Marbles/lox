use std::{
    env,
    fs::File,
    io::{Read, Write, stdin, stdout},
    path::Path,
    process::exit,
};

use crate::tree_walk::{
    error::run_err::RunError, interpreter::Interpreter, parser::Parser, token::Scanner,
};
mod ast;
mod error;
mod interpreter;
mod lox_var;
mod parser;
mod token;

pub fn main<'a>() -> Result<(), RunError<'a>> {
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

fn run_prompt<'a>() -> Result<(), RunError<'a>> {
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

fn run<'a>(source: &'a str) -> Result<(), RunError<'a>> {
    let scanner = Scanner::new(source);

    let mut errors: Vec<error::scan_err::ScanError> = Vec::new();

    let tokens: Vec<token::Token<'_>> = scanner
        .filter_map(|v| match v {
            Ok(t) => Some(t),
            Err(e) => {
                errors.push(e);
                None
            }
        })
        .collect();

    if errors.is_empty() {
        #[cfg(debug_assertions)]
        {
            println!("tokens:");
            for t in tokens.iter().cloned() {
                println!("{t:?}")
            }
        }

        let mut parser = Parser::new(tokens);
        let parsed = parser.parse()?;

        #[cfg(debug_assertions)]
        {
            println!("parsed:");
            for t in parsed.iter().cloned() {
                println!("{t}")
            }
        }

        let mut interpreter: Interpreter<'a> = Interpreter::new(parsed);

        if let Some(out) = interpreter.interpret()? {
            println!("{out}")
        }

        Ok(())
    } else {
        Err(errors.into())
    }
}
