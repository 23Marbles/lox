#![allow(clippy::single_call_fn)]

pub mod error_codes;

use std::{
    env,
    io::{Write, stdin, stdout},
    path::Path,
    process::exit,
    rc::Rc,
};

#[cfg(debug_assertions)]
use crate::tree_walk::interpreter::env::EnvironmentImpl;
use crate::{
    Runner,
    tree_walk::{
        error::{ToCodespan, diag_from_io},
        interpreter::Interpreter,
        lox_var::functions::LoxCallable,
        parser::Parser,
        static_analysis::Resolver,
        token::Scanner,
    },
};
pub mod ast;
pub mod error;
pub mod interpreter;
pub mod lox_var;
pub mod parser;
pub mod static_analysis;
pub mod token;

pub fn main(globals: Vec<(&'static str, Rc<dyn LoxCallable>)>) {
    let args: Vec<String> = env::args().collect();

    if args.len() > 2 {
        println!("usage: rlox [script]");
        exit(64)
    } else if args.len() == 2 {
        run_file_with_writer(Path::new(&args[1]), globals, Box::new(stdout()));
    } else {
        run_prompt(globals);
    }
}

pub fn run_file_with_writer<W: Write + Send + 'static>(
    file_path: &Path,
    globals: Vec<(&'static str, Rc<dyn LoxCallable>)>,
    mut writer: W,
) {
    #[cfg(debug_assertions)]
    let _ = writeln!(writer, "script opened at {:?}", file_path);

    let mut runner = Runner::new_with_writer(Box::new(writer)).with_globals(globals);

    if let Err(errs) = runner.run_file(file_path) {
        runner.write_errors(errs);
    }
}

fn run_prompt(globals: Vec<(&'static str, Rc<dyn LoxCallable>)>) {
    let mut line_buf = String::new();

    let mut runner = Runner::new()
        .with_globals(globals)
        .with_print_exprs(cfg!(feature = "repl_expr_print"));

    loop {
        print!("> ");
        if let Err(errs) = stdout().flush().map_err(|e| vec![diag_from_io(e)]) {
            runner.write_errors(errs);
            continue;
        }
        match stdin().read_line(&mut line_buf) {
            Ok(0) => return,
            Err(err) => runner.write_errors([diag_from_io(err)]),
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
            use crate::tree_walk::interpreter::env::EnvironmentImpl;

            runner.interpreter.env.debug_print_env();
            line_buf.clear();
            continue;
        }

        if let Err(errs) = runner.run_snippet(&line_buf) {
            runner.write_errors(errs);
        }

        line_buf.clear();
    }
}

#[expect(dead_code)]
#[deprecated = "Use [`Runner`] instead"]
fn run(source: &str, interpreter: &mut Interpreter, writer: &mut impl Write) -> crate::Result<()> {
    const DEPRECATED_PLACEHOLDER: usize = 0;

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

    let mut resolver = Resolver::new(DEPRECATED_PLACEHOLDER);
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
                for l in out.lines() {
                    let _ = writeln!(writer, "{l}").map_err(|e| vec![diag_from_io::<usize>(e)]);
                }
            }
            Err(err) => {
                #[cfg(debug_assertions)]
                interpreter.env.debug_print_env();
                return Err(vec![err.to_codespan(DEPRECATED_PLACEHOLDER)]);
            }
        }
    }

    Ok(())
}
