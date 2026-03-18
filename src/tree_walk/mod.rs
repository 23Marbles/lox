use std::{
    env,
    fs::File,
    io::{Read, Write, stdin, stdout},
    path::Path,
    process::exit,
    rc::Rc,
};

use crate::tree_walk::{
    error::run_err::RunError,
    interpreter::Interpreter,
    lox_var::{Env, EnvironmentImpl, LoxCallable, LoxVar},
    parser::Parser,
    token::Scanner,
};
mod ast;
pub mod error;
mod interpreter;
pub mod lox_var;
mod parser;
mod token;

pub fn main(lib: Vec<(&'static str, Rc<dyn LoxCallable>)>) -> Result<(), RunError> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 2 {
        println!("usage: rlox [script]");
        exit(64)
    } else if args.len() == 2 {
        run_file(Path::new(&args[1]), lib)
    } else {
        run_prompt(lib)
    }
}

fn run_file(
    file_path: &Path,
    lib: Vec<(&'static str, Rc<dyn LoxCallable>)>,
) -> Result<(), RunError> {
    #[cfg(debug_assertions)]
    println!("script opened at {:?}", file_path);

    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let mut env = <Env as EnvironmentImpl>::new(None);

    env.register_globals(
        lib.iter()
            .map(|(name, func)| (name.to_string(), func.clone()))
            .collect(),
    );

    run(&contents, &mut env).map(|_| ()).map_err(|mut err| {
        err.set_source(contents);
        err
    })
}

fn run_prompt(lib: Vec<(&'static str, Rc<dyn LoxCallable>)>) -> Result<(), RunError> {
    let mut line_buf = String::new();
    let mut env = <Env as EnvironmentImpl>::new(None);

    env.register_globals(
        lib.iter()
            .map(|(name, func)| (name.to_string(), func.clone()))
            .collect(),
    );

    loop {
        print!("> ");
        stdout().flush()?;
        if stdin().read_line(&mut line_buf)? == 0 {
            break Ok(());
        }

        #[cfg(feature = "implicit_semicolons")]
        if line_buf.len() > 2
            && line_buf
                .char_indices()
                .nth_back(2)
                .expect("Already checked len is long enough")
                .1
                != ';'
        {
            line_buf.insert(line_buf.len() - 2, ';');
        }

        let out = run(line_buf.trim_end(), &mut env);

        if let Err(mut err) = out {
            err.set_source(line_buf.clone());
            println!("{err}");
        } else if let Some(_var) = out.expect("already checked for err variants") {
            #[cfg(feature = "repl_expr_print")]
            println!("{_var}")
        }

        line_buf.clear();
    }
}

fn run(source: &str, env: &mut Env) -> Result<Option<LoxVar>, RunError> {
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
            for t in tokens.iter() {
                println!("{t:?}")
            }
        }

        let mut parser = Parser::new(tokens);
        let parsed = parser.parse()?;

        #[cfg(debug_assertions)]
        {
            println!("parsed:");
            for t in parsed.iter() {
                println!("{t}")
            }
        }

        let mut interpreter = Interpreter::new(parsed, env);

        Ok(interpreter.interpret()?)
    } else {
        Err(errors.into())
    }
}
