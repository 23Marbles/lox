use lox_runtime::Runner;
use rustyline::{Editor, config::Configurer, history::DefaultHistory};

use crate::highlighter::SyntaxHelper;

mod highlighter;

fn main() {
    let mut rl: Editor<SyntaxHelper, DefaultHistory> = Editor::new().unwrap();

    rl.set_helper(Some(SyntaxHelper));
    rl.set_color_mode(rustyline::ColorMode::Enabled);

    let mut runner = Runner::new().with_default_globals();

    loop {
        let line = rl.readline(">>> ");

        match line {
            Ok(input) => {
                let _ = rl.add_history_entry(input.as_str());

                let res = runner.run_snippet(&input);
                if let Err(errs) = res {
                    runner.write_errors(errs).unwrap();
                }
            }
            Err(_) => break,
        }
    }
}
