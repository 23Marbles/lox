use std::{error::Error, fmt::Display};

#[derive(Debug, Clone, Copy)]
pub enum LoxErrorKind {
    UnexpectedCharacter(char),
    UnterminatedString,
    InvalidNumber,
}

#[derive(Debug, Clone, Copy)]
pub struct LoxTreeError {
    pub line: usize,
    pub column: usize,
    pub kind: LoxErrorKind,
}

impl LoxTreeError {
    fn new(line: usize, kind: LoxErrorKind) -> Self {
        Self {
            line,
            column: 0,
            kind,
        }
    }
}

impl Display for LoxTreeError {
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
            LoxErrorKind::InvalidNumber => {
                write!(f, "[line {}:{}] Invalid number", self.line, self.column)?;
            }
        }
        Ok(())
    }
}

impl Error for LoxTreeError {}

#[derive(Debug)]
enum RunErrorKind {
    IoError(std::io::Error),
    TreeErrors(Vec<LoxTreeError>),
}

#[derive(Debug)]
pub struct RunError {
    kind: RunErrorKind,
}

impl Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            RunErrorKind::IoError(err) => write!(f, "{err}"),
            RunErrorKind::TreeErrors(errs) => {
                for e in errs {
                    writeln!(f, "{e}")?
                }
                Ok(())
            }
        }
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

impl From<Vec<LoxTreeError>> for RunError {
    fn from(value: Vec<LoxTreeError>) -> Self {
        Self {
            kind: RunErrorKind::TreeErrors(value),
        }
    }
}
