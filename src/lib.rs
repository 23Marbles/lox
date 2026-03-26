//#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::restriction)]
#![allow(clippy::implicit_return)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
/*#![deny(clippy::panic_in_result_fn)]
#![deny(clippy::unnecessary_to_owned)]
#![deny(clippy::redundant_clone)]
#![deny(clippy::unwrap_used)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::cfg_not_test)]
#![warn(clippy::copy_iterator)]
#![warn(clippy::trivially_copy_pass_by_ref)]
#![warn(clippy::missing_trait_methods)]
#![warn(clippy::new_without_default)]
#![warn(clippy::should_implement_trait)]
#![warn(clippy::ptr_arg)]
#![warn(clippy::needless_lifetimes)]
#![warn(clippy::wrong_self_convention)]
*/
//! # Lox
//! This my implementation of Lox in Rust.
//! Currently it only has the treewalk interpreter.
//! The benchmarks and tests are currently all passing.
//!
//! ## Benchmarks

use std::{fmt::Write, fs::File, io::Read, path::Path};

use crate::tree_walk::{
    error::{
        diagnostic::Diagnostic,
        run_err::{RunError},
        scan_err::ScanError,
    },
    interpreter::{
        Interpreter, InterpreterConfig,
        env::{Env, EnvironmentImpl},
    },
    lox_var::{LoxVar},
    parser::Parser,
    static_analysis::Resolver,
    token::{self, Scanner},
};

pub mod std_lib;
#[cfg(feature = "ast")]
pub mod tree_walk;

/// The backend type for the Runner struct
pub enum Backend {
    /// Use the treewalk interpreter
    TreeWalk,
    /// Use the bytecode VM
    ByteCode,
}

/// Used to construct and run a file
pub struct Runner {
    writer: Box<dyn Write>,
    _backend: Backend,
    interpreter: Interpreter,
}

impl Runner {
    pub fn run_file(&mut self, path: &Path) -> Result<(), Vec<Diagnostic>> {
        let mut file = File::open(path).map_err(|e| vec![e.into()])?;
        let mut content = String::new();
        file.read_to_string(&mut content).map_err(|e| vec![e.into()])?;

        self.run_source(&content)
    }

    pub fn run_source(&mut self, source: &str) -> Result<(), Vec<Diagnostic>> {
        let tokens = match Scanner::new(source).scan_tokens() {
            Ok(t) => t,
            Err(errs) => return Err(errs.iter().map(|err| err.into()).collect::<Vec<_>>()),
        };

        let mut parsed = match Parser::new(tokens).parse() {
            Ok(stmts) => stmts,
            Err(errs) => return Err(errs.iter().map(|err| err.into()).collect::<Vec<_>>()),
        };

        match Resolver::new().resolve_stmts(&mut parsed) {
            Ok(_) => (),
            Err(errs) => return Err(errs.iter().map(|err| err.into()).collect::<Vec<_>>()),
        };

        self.interpreter.update_stmts(parsed);

        for res in &mut self.interpreter {
            match res {
                Ok(out) => {
                    for l in out.lines() {
                        writeln!(self.writer, "{l}").map_err(|err| vec![err.into()])?
                    }
                }
                Err(err) => {
                    return Err(err.into());
                }
            }
        }

        Ok(())
    }
}

pub fn run_with_out(source: &str) -> Result<Vec<String>, RunError> {
    let mut interpreter = Interpreter::new(
        Vec::new(),
        Env::new_with_globals(
            std_lib::default_lib()
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
