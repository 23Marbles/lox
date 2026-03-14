use std::{
    env,
    fs::File,
    io::{Read, Write, stdin, stdout},
    path::Path,
    process::exit,
};

use crate::tree_walk::{
    error::run_err::RunError, interpreter::Interpreter, lox_var::Environment, parser::Parser, token::Scanner
};
mod ast;
mod error;
mod interpreter;
mod lox_var;
mod parser;
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

    let mut env = Environment::new();
    run(&contents, &mut env)
}

fn run_prompt() -> Result<(), RunError> {
    let mut line_buf = String::new();
    let mut env = Environment::new();

    loop {
        print!("> ");
        stdout().flush()?;
        if stdin().read_line(&mut line_buf)? == 0 {
            break Ok(());
        }

        let source = line_buf.clone();

        let err = run(&source, &mut env);
        
        if let Err(e) = err {
            println!("{e}");
        }
        line_buf.clear();
    }
}

fn run<'a>(source: &'a str, env: &mut Environment) -> Result<(), RunError> {
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

        let mut interpreter = Interpreter::new(parsed, env);

        if let Some(out) = interpreter.interpret()? {
            println!("{out}")
        }

        Ok(())
    } else {
        Err(errors.into())
    }
}
