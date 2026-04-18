#![warn(missing_docs)]
#![warn(clippy::restriction)]
#![warn(clippy::panic_in_result_fn)]
#![warn(clippy::unnecessary_to_owned)]
#![warn(clippy::redundant_clone)]
#![warn(clippy::unwrap_used)]
#![allow(clippy::question_mark_used)]
#![allow(clippy::min_ident_chars)]
#![allow(clippy::implicit_return)]
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
//!
//! | Name                | Average               | Range              |
//! | ------------------- | --------------------- | ------------------ |
//! | zoo_batch.lox       | 10.070327800 s/iter   | (+/- 0.139661850)  |
//! | method_call.lox     | 15.108997379 s/iter  | (+/- 4.856173500)  |
//! | invocation.lox      | 19.936385232 s/iter  | (+/- 6.740746550)  |
//! | zoo.lox             | 23.176159476 s/iter  | (+/- 8.743484650)  |
//! | instantiation.lox   | 24.841070760 s/iter  | (+/- 34.450921200) |
//! | properties.lox      | 28.623850036 s/iter  | (+/- 9.612263500)  |
//! | fib.lox             | 65.744055751 s/iter  | (+/- 21.469839300) |
//! | string_equality.lox | 79.359264633 s/iter  | (+/- 21.871288400) |
//! | equality.lox        | 88.425643870 s/iter  | (+/- 31.590151000) |
//! | binary_trees.lox    | 99.988585526 s/iter  | (+/- 35.513968700) |
//! | trees.lox           | 212.737865755 s/iter | (+/- 94.397381250) |

use std::{
    fs,
    hint::black_box,
    io,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};

use codespan_reporting::{
    diagnostic::Diagnostic,
    files::Files as _,
    term::{self, Config as ErrWriteConfig},
};

use crate::{
    error::diag_fom_err,
    lox_files::FileIdent,
    tree_walk::{
        ast::stmt::Stmt,
        interpreter::{
            InterpOut, Interpreter, LineType,
            env::{Env, EnvironmentImpl as _},
        },
        lox_var::LoxVar,
        parser::Parser,
        static_analysis::Resolver,
        token::{Scanner, Token},
    },
};

mod error;
pub mod error_codes;
pub mod std_lib;
#[cfg(feature = "ast")]
pub mod tree_walk;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
/// Represents a span of text in a file.
pub struct Span {
    range: core::ops::Range<usize>,
    file_id: FileIdent,
}

impl Span {
    /// Creates a new [`Span`] with the given range and file identifier.
    pub fn new(range: core::ops::Range<usize>, file_id: FileIdent) -> Self {
        Self { range, file_id }
    }
}

/// The backend type for the Runner struct.
pub enum Backend {
    /// Use the bytecode VM.
    ByteCode,
    /// Use the treewalk interpreter.
    TreeWalk,
}

#[derive(Clone)]
/// Configuration for the Runner struct.
pub struct RunnerConfig {
    expr_print: bool,
    err_cfg: ErrWriteConfig,
}

impl RunnerConfig {
    /// Creates a new [`RunnerConfig`] with the given configuration.
    pub fn new(expr_print: bool, err_cfg: ErrWriteConfig) -> Self {
        Self {
            expr_print,
            err_cfg,
        }
    }
}

#[derive(Clone)]
/// Used to construct and run a file.
///
/// # Basic usage
/// ```rust
/// # fn main() {
/// use lox_runtime::Runner;
/// let mut runner = Runner::new().with_default_globals();
///
/// // Prints the name of the script
/// // in this case that would be snippet as as all snippets are called that
/// let output = runner.run_snippet(r#"print "script name: " + argv[0];"#);
///
/// match output {
///     Err(errors) => runner.write_errors(errors).unwrap(),
///     Ok(_) => (),
/// }
/// # }
/// ```
pub struct Runner {
    ok_out: Arc<Mutex<dyn io::Write + Send>>,
    err_out: Arc<Mutex<dyn io::Write + Send>>,
    files: lox_files::LoxFiles,
    current_id: FileIdent,
    interpreter: Interpreter,
    cfg: RunnerConfig,
}

type Result<T> = core::result::Result<T, Vec<Diagnostic<FileIdent>>>;

/// Setting up a [`Runner`].
impl Default for Runner {
    fn default() -> Self {
        Self::new()
    }
}

mod lox_files;
impl Runner {
    /// Creates a new [`Runner`] with the default configuration.
    pub fn new() -> Self {
        Self {
            ok_out: Arc::new(Mutex::new(io::stdout())),
            err_out: Arc::new(Mutex::new(io::stderr())),
            files: lox_files::LoxFiles::new(),
            interpreter: Interpreter::new(FileIdent::default()),
            current_id: FileIdent::default(),
            cfg: RunnerConfig::new(false, ErrWriteConfig::default()),
        }
    }

    /// Creates a new [`Runner`] with the given writer.
    pub fn new_with_writers(
        ok_writer: Box<dyn io::Write + Send>,
        err_writer: Box<dyn io::Write + Send>,
    ) -> Self {
        Self {
            ok_out: Arc::new(Mutex::new(ok_writer)),
            err_out: Arc::new(Mutex::new(err_writer)),
            files: lox_files::LoxFiles::new(),
            interpreter: Interpreter::new(FileIdent::default()),
            current_id: FileIdent::default(),
            cfg: RunnerConfig::new(false, ErrWriteConfig::default()),
        }
    }

    /// Sets whether to print expressions.
    pub fn with_print_exprs(mut self, print_exprs: bool) -> Self {
        self.cfg.expr_print = print_exprs;
        self
    }

    /// Sets the globals for the runner.
    pub fn with_globals(mut self, globals: Vec<(&'static str, LoxVar)>) -> Self {
        self.interpreter = self.interpreter.with_globals(globals);
        self
    }

    /// Sets the globals for the runner.
    pub fn with_default_globals(mut self) -> Self {
        self.interpreter = self.interpreter.with_globals(std_lib::default_lib());
        self
    }

    /// Sets the error writer configuration for the runner.
    pub fn with_err_writer_cfg(mut self, err_writer_cfg: ErrWriteConfig) -> Self {
        self.cfg.err_cfg = err_writer_cfg;
        self
    }
}

/// Running snippets and files and getting output.
impl Runner {
    /// Runs a snippet and writes the output to the configured writer.
    pub fn run_snippet(&mut self, snippet: &str) -> Result<()> {
        let new_id = self.files.add("snippet".to_string(), snippet.into());
        self.update_current_id(new_id);
        self.write_source_output(snippet)
    }

    /// Runs a file and writes the output to the configured writer.
    pub fn run_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path).map_err(|e| vec![diag_fom_err(e)])?;

        let new_id = self.files.add(
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
                .into_owned(),
            content.clone(),
        );
        self.update_current_id(new_id);

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
                for line in out.iter().flat_map(|o| o.marked_lines()) {
                    match line.1 {
                        LineType::Normal => self.write_to_ok_out(&line.0)?,
                        LineType::Error => self.write_to_err_out(&line.0)?,
                    }
                }

                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Returns the time taken to run the given source code, or an error if the source code is invalid.
    pub fn get_run_time(
        &self,
        source: &str,
    ) -> core::result::Result<Duration, Vec<Diagnostic<FileIdent>>> {
        let mut interpreter = self.interpreter.clone();
        interpreter.set_stmts(&self.get_resolved_stmts(source)?);
        let start = std::time::Instant::now();
        black_box(interpreter.run()?);
        Ok(start.elapsed())
    }

    /// Returns the time taken to parse and run the given source code, or an error if the source code is invalid.
    pub fn get_full_time(
        &self,
        source: &str,
    ) -> core::result::Result<Duration, Vec<Diagnostic<FileIdent>>> {
        let ref mut temp = self.clone();
        let start = std::time::Instant::now();
        black_box(temp.run_snippet(source)?);
        Ok(start.elapsed())
    }

    /// Writes errors to the output stream.
    pub fn write_errors(
        &mut self,
        errs: impl IntoIterator<Item = Diagnostic<FileIdent>>,
    ) -> Result<()> {
        for err in errs {
            let mut err_str = String::new();
            term::emit_to_string(&mut err_str, &self.cfg.err_cfg, &self.files, &err)
                .map_err(|e| vec![diag_fom_err(e)])?;
            self.write_to_err_out(&err_str)?;
        }
        Ok(())
    }
}

impl Runner {
    fn update_current_id(&mut self, file_id: FileIdent) {
        self.current_id = file_id;
        self.interpreter.file_id = file_id;
    }

    /// Clears the interpreter state, resetting the environment and call depth.
    pub fn clear_interp(&mut self) {
        self.interpreter.set_env(Env::new_env(None));
    }

    /// Clears the file state, resetting the file map and current file identifier.
    pub fn clear_files(&mut self) {
        self.files.clear();
        self.update_current_id(FileIdent::default());
    }

    /// Writes a line to the output stream.
    fn write_to_ok_out(&mut self, line: &str) -> Result<()> {
        self.ok_out
            .lock()
            .map_err(|e| vec![diag_fom_err(e)])?
            .write(format!("{}\n", line).as_bytes())
            .map_err(|e| vec![diag_fom_err(e)])?;
        Ok(())
    }

    /// Writes a line to the error stream.
    fn write_to_err_out(&mut self, line: &str) -> Result<()> {
        self.err_out
            .lock()
            .map_err(|e| vec![diag_fom_err(e)])?
            .write(format!("{}\n", line).as_bytes())
            .map_err(|e| vec![diag_fom_err(e)])?;
        Ok(())
    }

    /// Gets the source code for the given file ID.
    pub fn get_source(&self, file_id: FileIdent) -> Option<&str> {
        self.files.source(file_id).ok()
    }

    /// Gets the tokens for the given source.
    pub fn get_tokens<'source>(&self, source: &'source str) -> Result<Vec<Token<'source>>> {
        Scanner::new(source, self.current_id).scan_tokens()
    }

    /// Gets the statements for the given source.
    pub fn get_stmts(&self, source: &str) -> Result<Vec<Stmt>> {
        Parser::new(self.get_tokens(source)?, self.current_id).parse()
    }

    /// Gets the resolved statements for the given source.
    pub fn get_resolved_stmts(&self, source: &str) -> Result<Vec<Stmt>> {
        let mut stmts = self.get_stmts(source)?;
        Resolver::new().resolve_stmts(&mut stmts)?;
        Ok(stmts)
    }
}

#[derive(Debug, Default, Clone)]
pub(in crate) struct TestWriter {
    str_buf: String,
}

impl io::Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.str_buf
            .push_str(core::str::from_utf8(buf).map_err(io::Error::other)?);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Never fails
        Ok(())
    }
}

#[derive(Default)]
/// A shared writer that can be used to write output from the runner.
pub struct SharedWriter(Arc<Mutex<TestWriter>>);

impl SharedWriter {
    /// Reads the contents of the writer.
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
        let mut guard = self.0.lock().map_err(|e| io::Error::other(e.to_string()))?;

        guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = self.0.lock().map_err(|e| io::Error::other(e.to_string()))?;

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

            let mut runner =
                Runner::new_with_writers(Box::new(writer.clone()), Box::new(writer.clone()));

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

    use std::{iter::zip, path::PathBuf};

    use super::*;

    #[test]
    /// Tests that the REPL for a bug where the instructions are not executed correctly and the ip is not reset.
    fn test_repl_ip_override() {
        let snippets = ["4+5; 6+7;", "print 4+5;"];
        let expected_output = [std::result::Result::Ok(""), std::result::Result::Ok("9\n")];

        Runner::test_repl(&snippets, &expected_output);
    }

    #[test]
    /// Tests that the current id is updated correctly after each snippet is executed in the REPL.
    fn test_current_id_repl_updates() {
        let snippets = ["1+1;", "2+2;"];

        let mut runner = Runner::new();

        let mut seen = Vec::new();

        for snippet in snippets.iter() {
            runner.run_snippet(snippet).unwrap();
            assert!(
                !seen.iter().any(|id| id == &runner.current_id),
                "Current_id is not updating"
            );
            assert!(
                !seen.iter().any(|id| id == &runner.interpreter.file_id),
                "Interpreter file_id is not updating"
            );
            seen.push(runner.current_id);
        }
    }

    #[test]
    /// Tests that the repl removes previous snippets with errors.
    fn test_repl_removes_errs() {
        let snippets = ["true * false;", "2+2;"];

        let mut runner = Runner::new();

        let mut seen = Vec::new();

        for snippet in snippets.iter() {
            let _ = runner.run_snippet(snippet);
            assert!(
                !seen.iter().any(|id| id == &runner.current_id),
                "Current_id is not updating"
            );
            assert!(
                !seen.iter().any(|id| id == &runner.interpreter.file_id),
                "Interpreter file_id is not updating"
            );
            seen.push(runner.current_id);
        }
    }

    #[test]
    /// Tests that the file runner removes previous snippets with errors.
    fn test_file_removes_errs() {
        let files = [
            PathBuf::from("../../tests/error.lox"),
            PathBuf::from("../../tests/hello_world.lox"),
        ];

        let mut runner = Runner::new();

        let mut seen = Vec::new();

        for path in files.iter() {
            let _ = runner.run_file(path);
            assert!(
                !seen.iter().any(|id| id == &runner.current_id),
                "Current_id is not updating"
            );
            assert!(
                !seen.iter().any(|id| id == &runner.interpreter.file_id),
                "Interpreter file_id is not updating"
            );
            seen.push(runner.current_id);
        }
    }
}
