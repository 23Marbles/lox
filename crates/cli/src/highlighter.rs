use std::borrow::Cow;

use lox_runtime::{
    lox_files::FileIdent,
    tree_walk::token::{Scanner, TokenKind},
};
use rustyline::{
    Helper,
    completion::Completer,
    highlight::Highlighter,
    hint::Hinter,
    validate::{ValidationContext, ValidationResult, Validator},
};

pub struct SyntaxHelper;

impl Helper for SyntaxHelper {}

impl Completer for SyntaxHelper {
    type Candidate = String;
}

impl Hinter for SyntaxHelper {
    type Hint = String;
}

impl Validator for SyntaxHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        let input = ctx.input();

        match depth(input) {
            Ok(0) => Ok(ValidationResult::Valid(None)),
            Ok(_) => Ok(ValidationResult::Incomplete),
            Err(_) => Ok(ValidationResult::Invalid(None)),
        }
    }
}

fn depth(source: &str) -> Result<usize, ()> {
    let mut depth: isize = 0;
    let mut in_string = false;
    let mut escaping = false;

    for c in source.chars() {
        match c {
            '\\' if in_string && !escaping => {
                escaping = true;
                continue;
            }
            '"' if !escaping => {
                in_string = !in_string;
            }
            '{' | '[' | '(' if !in_string => depth += 1,
            '}' | ']' | ')' if !in_string => depth -= 1,
            _ => {}
        }

        escaping = false;
    }

    Ok(depth as usize)
}

impl Highlighter for SyntaxHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        static mut I: usize = 0;
        unsafe { I += 1 }
        eprintln!("Refreshing {}", unsafe { I });
        let scanner = Scanner::new(line, FileIdent::new(0));

        let mut out = String::new();
        let mut last_end = 0;

        for result in scanner {
            let Ok(token) = result else { continue };

            // preserve gaps between tokens
            let start = token.span.range.start;
            let end = token.span.range.end;

            out.push_str(&line[last_end..start]);

            push_coloured(
                &mut out,
                &line[start..end],
                match token.kind {
                    t if t.is_keyword() => 34,
                    TokenKind::String => 32,
                    t if t.is_literal() => 33,
                    TokenKind::Identifier => 37,
                    _ => 37,
                },
            );

            last_end = end;
        }

        out.push_str(&line[last_end..]);

        Cow::Owned(out)
    }
}

fn push_coloured(out: &mut String, text: &str, color: u8) {
    use std::fmt::Write;
    let _ = write!(out, "\x1b[{}m{}\x1b[0m", color, text);
}
