macro_rules! translate_err_impl {
    ($lt:lifetime, $from:ty, $for:ty, $kind:path) => {
        impl<$lt> From<$from> for $for {
            fn from(value: $from) -> Self {
                Self { kind: $kind(value) }
            }
        }
    };

    ($from:ty, $for:ty, $kind:path) => {
        impl From<$from> for $for {
            fn from(value: $from) -> Self {
                Self { kind: $kind(value) }
            }
        }
    };
}

pub mod run_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{
        eval_err::EvaluateError, parse_err::ParseError, runtime_err::RuntimeError,
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
        kind: RunErrorKind,
    }

    impl Display for RunError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.kind {
                RunErrorKind::IoError(err) => write!(f, "Io error:\n{err}")?,
                RunErrorKind::ScanErrors(errs) => {
                    write!(f, "Scan errors: ")?;
                    for e in errs {
                        writeln!(f, "\n{e}")?
                    }
                }
                RunErrorKind::ParseErrors(errs) => {
                    write!(f, "Parse errors: ")?;
                    for e in errs {
                        writeln!(f, "\n{e}")?
                    }
                }
                RunErrorKind::EvaluateError(err) => write!(f, "Evaluation error:\n{err}")?,
                RunErrorKind::RuntimeError(err) => write!(f, "Evaluation error:\n{err}")?,
            }
            Ok(())
        }
    }

    impl Error for RunError {}

    translate_err_impl!('a, std::io::Error, RunError, RunErrorKind::IoError);
    translate_err_impl!('a, Vec<ScanError>, RunError, RunErrorKind::ScanErrors);
    translate_err_impl!('a, Vec<ParseError>, RunError, RunErrorKind::ParseErrors);
    translate_err_impl!('a, EvaluateError, RunError, RunErrorKind::EvaluateError);
    translate_err_impl!('a, RuntimeError, RunError, RunErrorKind::RuntimeError);
}

pub mod scan_err {
    use std::{error::Error, fmt::Display};
    #[derive(Debug, Clone, Copy)]
    pub enum ScanErrorKind {
        UnexpectedCharacter(char),
        UnterminatedString,
        #[cfg(feature = "challenges")]
        UnterminatedComment,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct ScanError {
        line: usize,
        column: usize,
        kind: ScanErrorKind,
    }

    impl ScanError {
        pub fn new(line: usize, column: usize, kind: ScanErrorKind) -> Self {
            Self { line, column, kind }
        }
    }

    impl Display for ScanError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.kind {
                ScanErrorKind::UnexpectedCharacter(c) => {
                    write!(
                        f,
                        "[line {}:{}] Unexpected character '{}'",
                        self.line, self.column, c
                    )?;
                }
                ScanErrorKind::UnterminatedString => {
                    write!(
                        f,
                        "[line {}:{}] Unterminated string",
                        self.line, self.column
                    )?;
                }
                #[cfg(feature = "challenges")]
                ScanErrorKind::UnterminatedComment => {
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

    impl Error for ScanError {}
}

pub mod runtime_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::eval_err::EvaluateError;

    #[derive(Debug, Clone)]
    pub struct RuntimeError {
        pub kind: RuntimeErrorKind,
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

    translate_err_impl!('a, EvaluateError, RuntimeError, RuntimeErrorKind::EvaluateError);
}

pub mod parse_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::token::TokenKind;
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
                ParseErrorKind::ExpectedTokenKind(token_kind) => write!(
                    f,
                    "[line {}:{}] Expected token kind: {:?}",
                    self.line, self.column, token_kind
                ),
                ParseErrorKind::UnterminatedGrouping => write!(
                    f,
                    "[line {}:{}] Expected closing \")\" after grouping",
                    self.line, self.column
                ),
                ParseErrorKind::ExpectedSemicolon => write!(
                    f,
                    "[line {}:{}] Expected ; after expression",
                    self.line, self.column
                ),
            }
        }
    }

    impl Error for ParseError {}

    #[derive(Debug, Clone, Copy)]
    pub enum ParseErrorKind {
        UnexpectedTokenKind(TokenKind),
        ExpectedTokenKind(TokenKind),
        UnterminatedGrouping,
        ExpectedSemicolon,
    }
}

pub mod eval_err {
    use std::fmt::Display;

    use crate::tree_walk::{
        lox_var::{LoxVar, LoxVarKinds},
        token::{Token, TokenKind},
    };

    #[derive(Debug, Clone)]
    pub struct EvaluateError {
        kind: EvaluateErrorKind,
        line: Option<usize>,
        column: Option<usize>,
    }

    impl EvaluateError {
        pub fn new(
            kind: EvaluateErrorKind,
            line: Option<usize>,
            column: Option<usize>,
        ) -> Self {
            Self { kind, line, column }
        }
    }

    impl Display for EvaluateError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let position = if self.column.is_some() || self.line.is_some() {
                format!(
                    "[{}{}] ",
                    self.line
                        .and_then(|v| Some(format!("Line {}", v)))
                        .unwrap_or("Column".to_string()),
                    self.column
                        .and_then(|v| Some(format!(":{}", v)))
                        .unwrap_or("".to_string())
                )
            } else {
                "".to_string()
            };
            match &self.kind {
                EvaluateErrorKind::IncompatableUnaryOperator { operator, var_ty } => write!(
                    f,
                    "{}Cannot use unary operator [{operator:?}] on type [{var_ty:?}]",
                    position
                ),
                EvaluateErrorKind::IncompatableBinaryOperator {
                    operator,
                    left_ty,
                    right_ty,
                } => write!(
                    f,
                    "{}Cannot use binary operator [{operator:?}] on types [{left_ty:?}, {right_ty:?}]",
                    position
                ),
                EvaluateErrorKind::UndeclaredVariable { name } => write!(
                    f,
                    "{}Tried to access undeclared variable with name {name}",
                    position
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
            Some(operator.line),
            Some(operator.column),
        )
    }
}
