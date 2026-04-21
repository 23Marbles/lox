use std::env;

use lox_runtime::{Runner, tree_walk::token::Token};
use rustyline::{Editor, config::Configurer, history::DefaultHistory};

use crate::highlighter::SyntaxHelper;

mod highlighter;

#[derive(Debug, Default, Clone, Copy)]
enum CliMode {
    Token,
    Ast,
    #[default]
    Runner,
}

fn main() {
    let mut rl: Editor<SyntaxHelper, DefaultHistory> = Editor::new().unwrap();

    rl.set_helper(Some(SyntaxHelper));
    rl.set_color_mode(rustyline::ColorMode::Enabled);

    let mut runner = Runner::new().with_default_globals();

    let mode = env::args()
        .nth(1)
        .map(|name| match name.as_str() {
            "token" => Some(CliMode::Token),
            "ast" => Some(CliMode::Ast),
            "runner" => Some(CliMode::Runner),
            x => {
                eprintln!("Unknown arg {x}");
                None
            }
        })
        .flatten()
        .unwrap_or_default();

    println!("The mode is {mode:?}");

    loop {
        let line = rl.readline(">>> ");

        match line {
            Ok(input) => {
                let _ = rl.add_history_entry(input.as_str());

                let res = match mode {
                    CliMode::Token => runner.get_tokens(&input).map(|tokens| {
                        for t in tokens {
                            println!("{t}")
                        }
                    }),
                    CliMode::Ast => runner.get_resolved_stmts(&input).map(|stmts| {
                        for s in stmts {
                            println!("{s}")
                        }
                    }),
                    CliMode::Runner => runner.run_snippet(&input),
                };
                if let Err(errs) = res {
                    runner.write_errors(errs).unwrap();
                }
            }
            Err(_) => break,
        }
    }
}
