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

pub mod run_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{
        eval_err::EvaluateError, parse_err::ParseError, scan_err::ScanError,
    };
    #[derive(Debug)]
    enum RunErrorKind {
        IoError(std::io::Error),
        ScanErrors(Vec<ScanError>),
        ParseErrors(Vec<ParseError>),
        EvaluateError(EvaluateError),
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
                RunErrorKind::EvaluateError(err) => writeln!(f, "Evaluation error:\n{err}")?,
            }
            Ok(())
        }
    }

    impl Error for RunError {}

    macro_rules! run_from_err {
        ($from:ty, $kind:path) => {
            impl From<$from> for RunError {
                fn from(value: $from) -> Self {
                    Self { kind: $kind(value) }
                }
            }
        };
    }

    run_from_err!(std::io::Error, RunErrorKind::IoError);
    run_from_err!(Vec<ScanError>, RunErrorKind::ScanErrors);
    run_from_err!(Vec<ParseError>, RunErrorKind::ParseErrors);
    run_from_err!(EvaluateError, RunErrorKind::EvaluateError);
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
}

pub mod eval_err {
    use std::fmt::Display;

    use crate::tree_walk::{
        lox_var::{LoxVar, LoxVarKinds},
        token::{Token, TokenKind},
    };

    #[derive(Debug, Clone, Copy)]
    pub struct EvaluateError {
        kind: EvaluateErrorKind,
        line: usize,
        column: usize,
    }

    impl EvaluateError {
        pub fn new(kind: EvaluateErrorKind, line: usize, column: usize) -> Self {
            Self { kind, line, column }
        }
    }

    impl Display for EvaluateError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self.kind {
                EvaluateErrorKind::IncompatableUnaryOperator { operator, var_ty } => write!(
                    f,
                    "[Line {}:{}] Cannot use unary operator [{operator:?}] on type [{var_ty:?}]",
                    self.line, self.column
                ),
                EvaluateErrorKind::IncompatableBinaryOperator {
                    operator,
                    left_ty,
                    right_ty,
                } => write!(
                    f,
                    "[Line {}:{}] Cannot use binary operator [{operator:?}] on types [{left_ty:?}, {right_ty:?}]",
                    self.line, self.column
                ),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
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
    }

    pub fn make_eval_error(
        operator: &Token,
        kind: TokenKind,
        left_var: LoxVar,
        right_var: LoxVar,
    ) -> EvaluateError {
        EvaluateError::new(
            EvaluateErrorKind::IncompatableBinaryOperator {
                operator: kind,
                left_ty: left_var.into(),
                right_ty: right_var.into(),
            },
            operator.line,
            operator.column,
        )
    }
}
