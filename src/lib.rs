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

use std::{
    fs, io,
    path::Path,
    rc::Rc,
    sync::{Arc, Mutex},
};

use codespan_reporting::{
    diagnostic::Diagnostic,
    files::SimpleFiles,
    term::{self, Config as ErrWriteConfig},
};

use crate::tree_walk::{
    ast::stmt::Stmt,
    error::diag_from_io,
    interpreter::{
        InterpOut, Interpreter,
        env::{Env, EnvironmentImpl as _},
    },
    lox_var::functions::LoxCallable,
    parser::Parser,
    static_analysis::Resolver,
    token::{Scanner, Token},
};

pub mod std_lib;
#[cfg(feature = "ast")]
pub mod tree_walk;

/// The backend type for the Runner struct.
pub enum Backend {
    /// Use the bytecode VM.
    ByteCode,
    /// Use the treewalk interpreter.
    TreeWalk,
}

pub struct RunnerConfig {
    expr_print: bool,
    err_cfg: ErrWriteConfig,
}

impl RunnerConfig {
    pub fn new(expr_print: bool, err_cfg: ErrWriteConfig) -> Self {
        Self {
            expr_print,
            err_cfg,
        }
    }
}

/// Used to construct and run a file.
pub struct Runner {
    out: Box<dyn io::Write + Send>,
    files: SimpleFiles<String, String>,
    current_id: usize,
    interpreter: Interpreter,
    cfg: RunnerConfig,
}

type Result<T> = core::result::Result<T, Vec<Diagnostic<usize>>>;

/// Setting up a [`Runner`].
impl Default for Runner {
    fn default() -> Self {
        Self::new()
    }
}

impl Runner {
    pub fn new() -> Self {
        Self {
            out: Box::new(io::stdout()),
            files: SimpleFiles::new(),
            interpreter: Interpreter::new(0),
            current_id: 0,
            cfg: RunnerConfig::new(false, ErrWriteConfig::default()),
        }
    }

    pub fn new_with_writer(writer: Box<dyn io::Write + Send>) -> Self {
        Self {
            out: writer,
            files: SimpleFiles::new(),
            interpreter: Interpreter::new(0),
            current_id: 0,
            cfg: RunnerConfig::new(false, ErrWriteConfig::default()),
        }
    }

    pub fn with_print_exprs(mut self, print_exprs: bool) -> Self {
        self.cfg.expr_print = print_exprs;
        self
    }

    pub fn with_globals(mut self, globals: Vec<(&'static str, Rc<dyn LoxCallable>)>) -> Self {
        self.interpreter = self.interpreter.with_globals(globals);
        self
    }

    pub fn with_err_writer_cfg(mut self, err_writer_cfg: ErrWriteConfig) -> Self {
        self.cfg.err_cfg = err_writer_cfg;
        self
    }
}

/// Running snippets and files and getting output.
impl Runner {
    pub fn run_snippet(&mut self, snippet: &str) -> Result<()> {
        self.current_id = self.files.add("snippet".into(), snippet.into());
        self.write_source_output(snippet)
    }

    pub fn run_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path).map_err(|e| vec![diag_from_io(e)])?;

        self.current_id = self.files.add(
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
                .into_owned(),
            content.clone(),
        );

        self.write_source_output(&content)
    }

    fn get_output(&mut self, source: &str) -> Result<Vec<InterpOut>> {
        self.interpreter
            .set_stmts(&self.get_resolved_stmts(source)?);
        self.interpreter.run()
    }

    fn write_source_output(&mut self, source: &str) -> Result<()> {
        match self.get_output(source) {
            Ok(out) => {
                for line in out.iter().flat_map(|o| o.lines()) {
                    writeln!(self.out, "{}", line).map_err(|e| vec![diag_from_io(e)])?
                }

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Writes errors to the output stream.
    pub fn write_errors(&mut self, errs: impl IntoIterator<Item = Diagnostic<usize>>) {
        for err in errs {
            // Pretend missing files dont exist
            let _ = term::emit_to_io_write(&mut self.out, &self.cfg.err_cfg, &self.files, &err);
        }
    }
}

impl Runner {
    pub fn clear_interp(&mut self) {
        self.interpreter.set_env(Env::new_env(None));
        self.current_id = 0;
    }

    pub fn get_source(&self, file_id: usize) -> Option<&str> {
        Some(self.files.get(file_id).ok()?.source())
    }

    pub fn get_tokens<'source>(&self, source: &'source str) -> Result<Vec<Token<'source>>> {
        Scanner::new(source, self.current_id).scan_tokens()
    }

    pub fn get_stmts(&self, source: &str) -> Result<Vec<Stmt>> {
        Parser::new(self.get_tokens(source)?, self.current_id).parse()
    }

    pub fn get_resolved_stmts(&self, source: &str) -> Result<Vec<Stmt>> {
        let mut stmts = self.get_stmts(source)?;
        Resolver::new(self.current_id).resolve_stmts(&mut stmts)?;
        Ok(stmts)
    }
}

#[derive(Debug, Default, Clone)]
pub(in crate) struct TestWriter {
    str_buf: String,
}

impl io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.str_buf.push_str(
            core::str::from_utf8(buf).map_err(io::Error::other)?,
        );
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Never fails
        Ok(())
    }
}

#[derive(Default)]
pub struct SharedWriter(Arc<Mutex<TestWriter>>);

impl SharedWriter {
    pub fn read(&self) -> String {
        let guard = self
            .0
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))
            .unwrap();
        guard.str_buf.clone()
    }
}

impl Clone for SharedWriter {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))?;

        guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = self
            .0
            .lock()
            .map_err(|e| io::Error::other(e.to_string()))?;

        guard.flush()
    }
}

#[cfg(test)]
mod tests {
    /// Tests functions for the Runner.
    impl Runner {
        /// Runs a series of snippets and asserts that the output matches the expected output.
        /// Errors are compared as strings and passed in as codes.
        pub fn test_repl(
            snippets: &[&str],
            expected_output: &[std::result::Result<&str, Vec<&str>>],
        ) {
            let writer = SharedWriter::default();

            let mut runner = Runner::new_with_writer(Box::new(writer.clone()));

            for (snippet, exp_out) in zip(snippets, expected_output) {
                match runner.run_snippet(snippet) {
                    Ok(_) => {
                        let output = writer.read();

                        let expected = exp_out.clone().unwrap().to_string();

                        assert_eq!(output, expected);

                        writer.0.lock().unwrap().str_buf.clear();
                    }
                    Err(errs) => {
                        let actual = errs
                            .iter()
                            .map(|e| e.code.as_ref().unwrap().to_string())
                            .collect::<Vec<String>>();

                        let expected = exp_out
                            .as_ref()
                            .unwrap_err()
                            .iter()
                            .map(|c| c.to_string())
                            .collect::<Vec<String>>();

                        assert_eq!(actual, expected);
                    }
                }
            }
        }
    }

    use std::iter::zip;

    use super::*;

    #[test]
    /// Tests that the REPL for a bug where the instructions are not executed correctly and the ip is not reset.
    fn test_repl_ip_override() {
        let snippets = ["4+5; 6+7;", "print 4+5;"];
        let expected_output = [std::result::Result::Ok(""), std::result::Result::Ok("9\n")];

        Runner::test_repl(&snippets, &expected_output);
    }
}
