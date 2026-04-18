#![allow(clippy::single_call_fn)]
//! The tree-walk interpreter for the Lox language.

use std::{
    env,
    io::{Write, stdin, stdout},
    path::Path,
};

#[cfg(debug_assertions)]
use crate::tree_walk::interpreter::env::EnvironmentImpl as _;
use crate::{
    Runner, RunnerOutput,
    error::{ToCodespan as _, diag_fom_err},
    lox_files::FileIdent,
    tree_walk::{
        interpreter::Interpreter, lox_var::LoxVar, parser::Parser, static_analysis::Resolver,
        token::Scanner,
    },
};
pub mod ast;
pub mod interpreter;
pub mod lox_var;
pub mod parser;
pub mod static_analysis;
pub mod token;

/// The entry point for the tree-walk interpreter.
pub fn main(globals: Vec<(&'static str, LoxVar)>) {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        run_file_with_writer(Path::new(&args[1]), globals, Box::new(stdout()));
    } else {
        run_prompt(globals);
    }
}

/// Runs a Lox script file with the given globals and writes output to the provided writer.
pub fn run_file_with_writer<W: Write + Send + 'static>(
    file_path: &Path,
    globals: Vec<(&'static str, LoxVar)>,
    mut writer: W,
) {
    #[cfg(debug_assertions)]
    let _ = writeln!(writer, "script opened at {:?}", file_path);

    let mut runner =
        Runner::new_with_writers(RunnerOutput::plain(writer), RunnerOutput::stderr_terminal())
            .with_globals(globals);

    if let Err(errs) = runner.run_file(file_path) {
        let _ = runner.write_errors(errs);
    }
}

/// Runs a Lox prompt with the given globals.
pub fn run_prompt(globals: Vec<(&'static str, LoxVar)>) {
    let mut line_buf = String::new();

    let mut runner = Runner::new()
        .with_globals(globals)
        .with_print_exprs(cfg!(feature = "repl_expr_print"));

    loop {
        print!("> ");
        if let Err(errs) = stdout().flush().map_err(|e| vec![diag_fom_err(e)]) {
            let _ = runner.write_errors(errs);
            continue;
        }
        match stdin().read_line(&mut line_buf) {
            Ok(0) => return,
            Err(err) => {
                let _ = runner.write_errors([diag_fom_err(err)]);
                continue;
            }
            _ => {}
        }

        #[cfg(feature = "implicit_semicolons")]
        {
            todo!("feature = "implicit_semicolons": needs more work");
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
        }
        #[cfg(debug_assertions)]
        if line_buf.trim() == "vars" {
            use crate::tree_walk::interpreter::env::EnvironmentImpl as _;

            runner.interpreter.env.debug_print_env();
            line_buf.clear();
            continue;
        }

        if let Err(errs) = runner.run_snippet(line_buf.trim_end()) {
            let _ = runner.write_errors(errs);
        }

        line_buf.clear();
    }
}

#[expect(dead_code)]
#[deprecated = "Use [`Runner`] instead"]
fn run(
    source: &str,
    interpreter: &mut Interpreter,
    writer: &mut impl Write,
    e_writer: &mut impl Write,
) -> crate::Result<()> {
    const DEPRECATED_PLACEHOLDER: FileIdent = FileIdent::new(0);

    let mut scanner = Scanner::new(source, DEPRECATED_PLACEHOLDER);

    let tokens = scanner.scan_tokens()?;

    #[cfg(debug_assertions)]
    {
        let _ = writeln!(writer, "tokens:");
        for t in tokens.iter() {
            let _ = writeln!(writer, "{t:?}");
        }
    }

    let mut parser = Parser::new(tokens, DEPRECATED_PLACEHOLDER);
    let mut parsed = parser.parse()?;

    #[cfg(debug_assertions)]
    {
        let _ = writeln!(writer, "parsed:");
        for t in parsed.iter() {
            let _ = writeln!(writer, "{t}");
        }
    }

    let mut resolver = Resolver::new();
    resolver.resolve_stmts(&mut parsed)?;

    #[cfg(debug_assertions)]
    {
        let _ = writeln!(writer, "resolved:");
        for t in parsed.iter() {
            let _ = writeln!(writer, "{t:?}");
        }
    }

    interpreter.set_stmts(&parsed);

    for res in &mut *interpreter {
        match res {
            Ok(out) => {
                for l in out.marked_lines() {
                    let _ = match l.1 {
                        interpreter::LineType::Normal => {
                            writeln!(writer, "{}", l.0).map_err(|e| vec![diag_fom_err::<usize>(e)])
                        }
                        interpreter::LineType::Error => writeln!(e_writer, "{}", l.0)
                            .map_err(|e| vec![diag_fom_err::<usize>(e)]),
                    };
                }
            }
            Err(err) => {
                #[cfg(debug_assertions)]
                interpreter.env.debug_print_env();
                return Err(vec![err.to_codespan()]);
            }
        }
    }

    Ok(())
}
