//#![warn(missing_docs)]
//#![warn(clippy::pedantic)]
#![warn(clippy::restriction)]
#![warn(clippy::panic_in_result_fn)]
#![warn(clippy::unnecessary_to_owned)]
#![warn(clippy::redundant_clone)]
#![warn(clippy::unwrap_used)]
#![allow(clippy::question_mark_used)]
#![allow(clippy::min_ident_chars)]
#![allow(clippy::implicit_return)]
//#![warn(clippy::pedantic)]
#![warn(clippy::cfg_not_test)]
#![warn(clippy::copy_iterator)]
#![warn(clippy::trivially_copy_pass_by_ref)]
#![warn(clippy::new_without_default)]
#![warn(clippy::should_implement_trait)]
#![warn(clippy::ptr_arg)]
#![warn(clippy::needless_lifetimes)]

//! # Lox
//! This my implementation of Lox in Rust.
//! Currently it only has the treewalk interpreter.
//! The benchmarks and tests are currently all passing.
//!
//! ## Benchmarks

use std::{fmt::Write, fs::File, io::Read as _, path::Path};

use crate::tree_walk::{
    error::{diagnostic::Diagnostic, run_err::RunError, scan_err::ScanError},
    interpreter::{
        Interpreter, InterpreterConfig,
        env::{Env, EnvironmentImpl as _},
    },
    lox_var::LoxVar,
    parser::Parser,
    static_analysis::Resolver,
    token::{self, Scanner},
};

pub mod std_lib;
#[cfg(feature = "ast")]
pub mod tree_walk;

/// The backend type for the Runner struct.
#[non_exhaustive]
pub enum Backend {
    /// Use the bytecode VM.
    ByteCode,
    /// Use the treewalk interpreter.
    TreeWalk,
    /// Use the bytecode VM
    ByteCode,
}

/// Used to construct and run a file.
pub struct Runner {
    out: Box<dyn io::Write + Send>,
    files: SimpleFiles<String, String>,
    interpreter: Interpreter,
}

type Result<T> = std::result::Result<T, Vec<Diagnostic<usize>>>;

impl<'a> Runner {
    pub fn new() -> Self {
        Self {
            out: Box::new(io::stdout()),
            files: SimpleFiles::new(),
            interpreter: Interpreter::default(),
        }
    }

    pub fn with_writer(writer: Box<dyn io::Write + Send>) -> Self {
        Self {
            out: writer,
            files: SimpleFiles::new(),
            interpreter: Interpreter::default(),
        }
    }

    pub fn run_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path).map_err(|e| vec![e.into()])?;

        self.write_source_output(&content)
    }

    pub fn get_tokens(&self, source: &'a str) -> Result<Vec<Token<'a>>> {
        Scanner::new(source).scan_tokens()
    }

    pub fn get_stmts(&self, source: &'a str) -> Result<Vec<Stmt>> {
        Parser::new(self.get_tokens(source)?).parse()
    }

    pub fn get_resolved_stmts(&self, source: &'a str) -> Result<Vec<Stmt>> {
        let mut stmts = self.get_stmts(source)?;
        Resolver::new().resolve_stmts(&mut stmts)?;
        Ok(stmts)
    }

    fn get_output(&mut self, source: &str) -> Result<Vec<InterpOut>> {
        self.interpreter
            .update_stmts(&self.get_resolved_stmts(source)?);
        self.interpreter.run()
    }

    pub fn write_source_output(&mut self, source: &str) -> Result<()> {
        match self.get_output(source) {
            Ok(out) => {
                for line in out.iter().map(|o| o.lines()).flatten() {
                    writeln!(self.out, "{}", line).map_err(|e| vec![e.into()])?
        }

                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}
