use std::{
    env,
    fs::File,
    io::{Read, Write, stdin, stdout},
    path::Path,
    process::exit,
    rc::Rc,
};

use crate::tree_walk::{
    error::run_err::{RunError, RunErrorSourced},
    interpreter::{
        Interpreter, InterpreterConfig,
        env::{Env, EnvironmentImpl},
    },
    lox_var::{LoxVar, functions::LoxCallable},
    parser::Parser,
    static_analysis::Resolver,
    token::Scanner,
};
pub mod ast;
pub mod error;
pub mod interpreter;
pub mod lox_var;
pub mod parser;
pub mod static_analysis;
pub mod token;

pub fn main(globals: Vec<(&'static str, Rc<dyn LoxCallable>)>) -> Result<(), RunErrorSourced> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 2 {
        println!("usage: rlox [script]");
        exit(64)
    } else if args.len() == 2 {
        run_file(Path::new(&args[1]), globals)
    } else {
        run_prompt(globals)
    }
}

pub fn run_file_with_writer(
    file_path: &Path,
    globals: Vec<(&'static str, Rc<dyn LoxCallable>)>,
    writer: &mut impl Write
) -> Result<(), RunErrorSourced> {
    #[cfg(feature = "debug_print")]
    writeln!(writer, "script opened at {:?}", file_path);

    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let env = Env::new_with_globals(
        globals
            .iter()
            .cloned()
            .map(|(name, func)| (name, LoxVar::Callable(func)))
            .collect(),
    );

    let mut interpreter = Interpreter::new(Vec::new(), env, InterpreterConfig::new(false));

    run(&contents, &mut interpreter, writer).map_err(|err| err.with_source(contents))
}

pub fn run_file(
    file_path: &Path,
    globals: Vec<(&'static str, Rc<dyn LoxCallable>)>,
) -> Result<(), RunErrorSourced> {
    #[cfg(feature = "debug_print")]
    println!("script opened at {:?}", file_path);

    let mut file = File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let env = Env::new_with_globals(
        globals
            .iter()
            .cloned()
            .map(|(name, func)| (name, LoxVar::Callable(func)))
            .collect(),
    );

    let mut interpreter = Interpreter::new(Vec::new(), env, InterpreterConfig::new(false));

    run(&contents, &mut interpreter, &mut stdout()).map_err(|err| err.with_source(contents))
}

fn run_prompt(globals: Vec<(&'static str, Rc<dyn LoxCallable>)>) -> Result<(), RunErrorSourced> {
    let mut line_buf = String::new();

    let env = Env::new_with_globals(
        globals
            .iter()
            .cloned()
            .map(|(name, func)| (name, LoxVar::Callable(func)))
            .collect(),
    );
    let mut interpreter = Interpreter::new(
        Vec::new(),
        env,
        InterpreterConfig::new(cfg!(feature = "repl_expr_print")),
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

        let out = run(line_buf.trim_end(), &mut interpreter, &mut stdout());

        if let Err(err) = out {
            println!("{}", err.with_source(line_buf.clone()));
        }

        line_buf.clear();
    }
}

fn run(source: &str, interpreter: &mut Interpreter, writer: &mut impl Write) -> Result<(), RunError> {
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
        #[cfg(feature = "debug_print")]
        {
            writeln!(writer, "tokens:");
            for t in tokens.iter() {
                writeln!(writer, "{t:?}")
            }
        }

        let mut parser = Parser::new(tokens);
        let mut parsed = parser.parse()?;

        #[cfg(feature = "debug_print")]
        {
            writeln!(writer, "parsed:");
            for t in parsed.iter() {
                writeln!(writer, "{t}")
            }
        }

        let mut resolver = Resolver::new();
        resolver.resolve_stmts(&mut parsed)?;

        #[cfg(feature = "debug_print")]
        {
            writeln!(writer, "reolved:");
            for t in parsed.iter() {
                writeln!(writer, "{t:?}")
            }
        }

        interpreter.update_stmts(parsed);

        for res in &mut *interpreter {
            match res {
                Ok(out) => {
                    for l in out.lines() {
                        writeln!(writer, "{l}")?
                    }
                }
                Err(err) => {
                    #[cfg(feature = "debug_print")]
                    debug_print(&interpreter.env);
                    return Err(err.into());
                }
            }
        }

        Ok(())
    } else {
        Err(errors.into())
    }
}
