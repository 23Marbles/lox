use std::{error::Error, fmt::Display};

use crate::tree_walk::token::TokenKind;

#[derive(Debug, Clone, Copy)]
pub enum LoxErrorKind {
    UnexpectedCharacter(char),
    UnterminatedString,
    #[cfg(feature = "challenges")]
    UnterminatedComment,
}

#[derive(Debug, Clone, Copy)]
pub struct LoxScanError {
    pub line: usize,
    pub column: usize,
    pub kind: LoxErrorKind,
}

impl LoxScanError {
    fn new(line: usize, kind: LoxErrorKind) -> Self {
        Self {
            line,
            column: 0,
            kind,
        }
    }
}

impl Display for LoxScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            LoxErrorKind::UnexpectedCharacter(c) => {
                write!(
                    f,
                    "[line {}:{}] Unexpected character '{}'",
                    self.line, self.column, c
                )?;
            }
            LoxErrorKind::UnterminatedString => {
                write!(
                    f,
                    "[line {}:{}] Unterminated string",
                    self.line, self.column
                )?;
            }
            #[cfg(feature = "challenges")]
            LoxErrorKind::UnterminatedComment => {
                write!(
                    f,
                    "[line {}:{}] Unterminated comment",
                    self.line, self.column
                )?;
            }
        }
        Ok(())
    }
}

impl Error for LoxScanError {}

#[derive(Debug)]
enum RunErrorKind {
    IoError(std::io::Error),
    ScanErrors(Vec<LoxScanError>),
    ParseErrors(Vec<ParseError>)
}

#[derive(Debug)]
pub struct RunError {
    kind: RunErrorKind,
}

impl Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            RunErrorKind::IoError(err) => write!(f, "Io error:\n{err}")?,
            RunErrorKind::ScanErrors(errs) => {
                writeln!(f, "Scan errors: ")?;
                for e in errs {
                    writeln!(f, "{e}")?
                }
            }
            RunErrorKind::ParseErrors(errs) => {
                writeln!(f, "Parse errors: ")?;
                for e in errs {
                    writeln!(f, "{e}")?
                }
            }
        }
        Ok(())
    }
}

impl Error for RunError {}

impl From<std::io::Error> for RunError {
    fn from(value: std::io::Error) -> Self {
        Self {
            kind: RunErrorKind::IoError(value),
        }
    }
}

impl From<Vec<LoxScanError>> for RunError {
    fn from(value: Vec<LoxScanError>) -> Self {
        Self {
            kind: RunErrorKind::ScanErrors(value),
        }
    }
}

impl From<Vec<ParseError>> for RunError {
    fn from(value: Vec<ParseError>) -> Self {
        Self { kind: RunErrorKind::ParseErrors(value) }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub line: usize,
    pub column: usize,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ParseErrorKind::UnexpectedTokenKind(token_kind) => write!(
                f,
                "[line {}:{}] Unexpected token kind: {:?}",
                self.line, self.column, token_kind
            ),
            ParseErrorKind::UnterminatedGrouping => write!(
                f,
                "[line {}:{}] Expected closing \")\" after expression",
                self.line, self.column
            ),
        }
    }
}

impl Error for ParseError {}

#[derive(Debug, Clone, Copy)]
pub enum ParseErrorKind {
    UnexpectedTokenKind(TokenKind),
    UnterminatedGrouping,
}
