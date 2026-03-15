use std::fmt::Display;

use crate::tree_walk::token::Token;

pub mod run_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{
        ErrorPos, eval_err::EvaluateError, parse_err::ParseError, runtime_err::RuntimeError,
        scan_err::ScanError,
    };

    #[derive(Debug)]
    enum RunErrorKind {
        IoError(std::io::Error),
        ScanErrors(Vec<ScanError>),
        ParseErrors(Vec<ParseError>),
        EvaluateError(EvaluateError),
        RuntimeError(RuntimeError),
    }

    #[derive(Debug)]
    pub struct RunError {
        source: Option<String>,
        kind: RunErrorKind,
    }

    impl RunError {
        pub fn position(&self, index: Option<usize>) -> Option<ErrorPos> {
            match &self.kind {
                RunErrorKind::IoError(_) => None,
                RunErrorKind::ScanErrors(scan_errors) => Some(scan_errors.get(index?)?.pos()),
                RunErrorKind::ParseErrors(parse_errors) => Some(parse_errors.get(index?)?.pos()),
                RunErrorKind::EvaluateError(evaluate_error) => Some(evaluate_error.pos()),
                RunErrorKind::RuntimeError(runtime_error) => Some(runtime_error.pos()),
            }
        }

        pub fn set_source(&mut self, source: String) {
            self.source = Some(source)
        }
    }

    impl Display for RunError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if let Some(source) = &self.source {
                match &self.kind {
                    RunErrorKind::IoError(err) => write!(f, "Io error:\n{err}")?,
                    RunErrorKind::ScanErrors(errs) => {
                        write!(f, "Scan errors: ")?;
                        for (i, e) in errs.iter().enumerate() {
                            write!(f, "\n{e}")?;
                            if let Some(pos) = self.position(Some(i)) {
                                write!(
                                    f,
                                    "\n|   {}",
                                    source
                                        .lines()
                                        .collect::<Vec<&str>>()
                                        .get(pos.line - 1)
                                        .unwrap()
                                )?;
                                write!(
                                    f,
                                    "\n\\  {}{}",
                                    " ".repeat(pos.column),
                                    "^".repeat(pos.len)
                                )?;
                            }
                        }
                    }
                    RunErrorKind::ParseErrors(errs) => {
                        write!(f, "Parse errors: ")?;
                        for (i, e) in errs.iter().enumerate() {
                            write!(f, "\n{e}")?;

                            if let Some(pos) = self.position(Some(i)) {
                                write!(
                                    f,
                                    "\n|   {}",
                                    source
                                        .lines()
                                        .collect::<Vec<&str>>()
                                        .get(pos.line - 1)
                                        .unwrap()
                                )?;
                                write!(
                                    f,
                                    "\n\\  {}{}",
                                    " ".repeat(pos.column),
                                    "^".repeat(pos.len)
                                )?;
                            }
                        }
                    }
                    RunErrorKind::EvaluateError(err) => {
                        write!(f, "Evaluation error:\n{err}")?;
                        if let Some(pos) = self.position(None) {
                            write!(
                                f,
                                "\n|   {}",
                                source
                                    .lines()
                                    .collect::<Vec<&str>>()
                                    .get(pos.line - 1)
                                    .unwrap()
                            )?;
                            write!(f, "\n\\  {}{}", " ".repeat(pos.column), "^".repeat(pos.len))?;
                        }
                    }
                    RunErrorKind::RuntimeError(err) => {
                        write!(f, "Evaluation error:\n{err}")?;
                        if let Some(pos) = self.position(None) {
                            write!(
                                f,
                                "\n|   {}",
                                source
                                    .lines()
                                    .collect::<Vec<&str>>()
                                    .get(pos.line - 1)
                                    .unwrap()
                            )?;
                            write!(f, "\n\\  {}{}", " ".repeat(pos.column), "^".repeat(pos.len))?;
                        }
                    }
                }
            } else {
                match &self.kind {
                    RunErrorKind::IoError(err) => write!(f, "Io error:\n{err}")?,
                    RunErrorKind::ScanErrors(errs) => {
                        write!(f, "Scan errors: ")?;
                        for e in errs {
                            write!(f, "\n{e}")?
                        }
                    }
                    RunErrorKind::ParseErrors(errs) => {
                        write!(f, "Parse errors: ")?;
                        for e in errs {
                            write!(f, "\n{e}")?
                        }
                    }
                    RunErrorKind::EvaluateError(err) => write!(f, "Evaluation error:\n{err}")?,
                    RunErrorKind::RuntimeError(err) => write!(f, "Evaluation error:\n{err}")?,
                }
            }
            Ok(())
        }
    }

    impl Error for RunError {}

    macro_rules! translate_err_impl {
        ($from:ty, $kind:path) => {
            impl From<$from> for RunError {
                fn from(value: $from) -> Self {
                    Self {
                        source: None,
                        kind: $kind(value),
                    }
                }
            }
        };
    }

    translate_err_impl!(std::io::Error, RunErrorKind::IoError);
    translate_err_impl!(Vec<ScanError>, RunErrorKind::ScanErrors);
    translate_err_impl!(Vec<ParseError>, RunErrorKind::ParseErrors);
    translate_err_impl!(EvaluateError, RunErrorKind::EvaluateError);
    translate_err_impl!(RuntimeError, RunErrorKind::RuntimeError);
}

pub mod scan_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::ErrorPos;
    #[derive(Debug, Clone, Copy)]
    pub enum ScanErrorKind {
        UnexpectedCharacter(char),
        UnterminatedString,
        #[cfg(feature = "inline_comments")]
        UnterminatedComment,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct ScanError {
        kind: ScanErrorKind,
        pos: ErrorPos,
    }

    impl ScanError {
        pub fn new(kind: ScanErrorKind, pos: ErrorPos) -> Self {
            Self { kind, pos }
        }

        pub fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl Display for ScanError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.kind {
                ScanErrorKind::UnexpectedCharacter(c) => {
                    write!(f, "{} Unexpected character '{}'", self.pos, c)?;
                }
                ScanErrorKind::UnterminatedString => {
                    write!(f, "{} Unterminated string", self.pos)?;
                }
                #[cfg(feature = "inline_comments")]
                ScanErrorKind::UnterminatedComment => {
                    write!(f, "{} Unterminated comment", self.pos)?;
                }
            }
            Ok(())
        }
    }

    impl Error for ScanError {}
}

pub mod runtime_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{ErrorPos, eval_err::EvaluateError};

    #[derive(Debug, Clone)]
    pub struct RuntimeError {
        pub kind: RuntimeErrorKind,
    }

    impl RuntimeError {
        pub fn pos(&self) -> ErrorPos {
            match &self.kind {
                RuntimeErrorKind::EvaluateError(evaluate_error) => evaluate_error.pos(),
            }
        }
    }

    impl Display for RuntimeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.kind {
                RuntimeErrorKind::EvaluateError(err) => write!(f, "{err}"),
            }
        }
    }

    impl Error for RuntimeError {}

    #[derive(Debug, Clone)]
    pub enum RuntimeErrorKind {
        EvaluateError(EvaluateError),
    }

    impl From<EvaluateError> for RuntimeError {
        fn from(value: EvaluateError) -> Self {
            Self {
                kind: RuntimeErrorKind::EvaluateError(value),
            }
        }
    }
}

pub mod parse_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::{error::ErrorPos, token::TokenKind};
    #[derive(Debug, Clone)]
    pub struct ParseError {
        pub kind: ParseErrorKind,
        pos: ErrorPos,
    }

    impl ParseError {
        pub fn new(kind: ParseErrorKind, pos: ErrorPos) -> Self {
            Self { kind, pos }
        }

        pub fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl Display for ParseError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.kind {
                ParseErrorKind::UnexpectedTokenKind(token_kind) => {
                    write!(f, "{} Unexpected token kind: [{:?}]", self.pos, token_kind)
                }
                ParseErrorKind::ExpectedTokenKind(token_kind) => {
                    write!(f, "{} Expected token kind: [{:?}]", self.pos, token_kind)
                }
                ParseErrorKind::InvalidAssignTarget(s) => {
                    write!(f, "{} Invalid assignment target [{s}]", self.pos)
                }
                ParseErrorKind::TooManyArguments => {
                    write!(f, "{} Can't have over 255 arguments", self.pos)
                }
            }
        }
    }

    impl Error for ParseError {}

    #[derive(Debug, Clone)]
    pub enum ParseErrorKind {
        UnexpectedTokenKind(TokenKind),
        ExpectedTokenKind(TokenKind),
        InvalidAssignTarget(String),
        TooManyArguments,
    }
}

pub mod eval_err {
    use std::fmt::Display;

    use crate::tree_walk::{
        error::ErrorPos,
        lox_var::{LoxVar, LoxVarKinds},
        token::{Token, TokenKind},
    };

    #[derive(Debug, Clone)]
    pub struct EvaluateError {
        kind: EvaluateErrorKind,
        pos: ErrorPos,
    }

    impl EvaluateError {
        pub fn new(kind: EvaluateErrorKind, pos: ErrorPos) -> Self {
            Self { kind, pos }
        }

        pub fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl Display for EvaluateError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.kind {
                EvaluateErrorKind::IncompatableUnaryOperator { operator, var_ty } => write!(
                    f,
                    "{} Cannot use unary operator [{operator:?}] on type [{var_ty:?}]",
                    self.pos
                ),
                EvaluateErrorKind::IncompatableBinaryOperator {
                    operator,
                    left_ty,
                    right_ty,
                } => write!(
                    f,
                    "{} Cannot use binary operator [{operator:?}] on types [{left_ty:?}, {right_ty:?}]",
                    self.pos
                ),
                EvaluateErrorKind::UndeclaredVariable { name } => write!(
                    f,
                    "{} Tried to access undeclared variable with name {name}",
                    self.pos
                ),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum EvaluateErrorKind {
        IncompatableUnaryOperator {
            operator: TokenKind,
            var_ty: LoxVarKinds,
        },
        IncompatableBinaryOperator {
            operator: TokenKind,
            left_ty: LoxVarKinds,
            right_ty: LoxVarKinds,
        },
        UndeclaredVariable {
            name: String,
        },
    }

    pub fn make_bin_eval_error(
        operator: &Token,
        kind: TokenKind,
        left_var: &LoxVar,
        right_var: &LoxVar,
    ) -> EvaluateError {
        EvaluateError::new(
            EvaluateErrorKind::IncompatableBinaryOperator {
                operator: kind,
                left_ty: left_var.into(),
                right_ty: right_var.into(),
            },
            operator.into(),
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ErrorPos {
    line: usize,
    column: usize,
    len: usize,
}

impl ErrorPos {
    pub fn new(line: usize, column: usize, len: usize) -> Self {
        Self { line, column, len }
    }

    pub fn end(&self) -> Self {
        Self {
            line: self.line,
            column: self.column + self.len,
            len: 1,
        }
    }
}

impl Display for ErrorPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Line {}:{}]", self.line, self.column)
    }
}

impl From<&Token<'_>> for ErrorPos {
    fn from(value: &Token<'_>) -> Self {
        use super::token::TokenKind::*;
        Self {
            line: value.line,
            column: if value.kind == String {
                value.column - 1
            } else {
                value.column
            },
            len: match value.kind {
                BangEqual | EqualEqual | GreaterEqual | LessEqual => 2,
                Or | If => 2,
                And | Var | Fun | For | Nil => 3,
                Else | This | True => 4,
                Class | False | Print | Super | While => 5,
                Return => 6,
                Identifier => value.lexeme.len(),
                String => value.lexeme.len() + 2,
                Number => value.lexeme.len(),
                _ => 1,
            },
        }
    }
}
