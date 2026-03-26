use std::{path::Path, rc::Rc};

use crate::tree_walk::{
    error::{
        run_err::{RunError, RunErrorSourced},
        scan_err::ScanError,
    },
    interpreter::{
        Interpreter, InterpreterConfig,
        env::{Env, EnvironmentImpl},
    },
    lox_var::{LoxVar, functions::LoxCallable},
    parser::Parser,
    run_file,
    static_analysis::Resolver,
    token::{self, Scanner},
};

mod std_lib;
pub mod tree_walk;

pub fn run_file_with_globals(path: &Path) -> Result<(), RunErrorSourced> {
    run_file(&path, std_lib::lib())
}

pub fn run_with_out(source: &str) -> Result<Vec<String>, RunError> {
    let mut interpreter = Interpreter::new(
        Vec::new(),
        Env::new_with_globals(
            std_lib::lib()
                .iter()
                .cloned()
                .map(|(name, func)| (name, LoxVar::Callable(func)))
                .collect(),
        ),
        InterpreterConfig::default(),
    );

    let scanner = Scanner::new(source);

    let mut errors: Vec<ScanError> = Vec::new();

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
        let mut parser = Parser::new(tokens);
        let mut parsed = parser.parse()?;

        let mut resolver = Resolver::new();
        resolver.resolve_stmts(&mut parsed)?;

        interpreter.update_stmts(parsed);

        let mut out = Vec::new();

        for res in interpreter {
            match res {
                Ok(i_out) => {
                    for l in i_out.lines() {
                        out.push(l);
                    }
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        Ok(out)
    } else {
        Err(errors.into())
    }
}

pub fn lib() -> Vec<(&'static str, Rc<dyn LoxCallable + 'static>)> {
    std_lib::lib()
}