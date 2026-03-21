use std::fmt::Display;

use crate::tree_walk::token::Token;

pub trait LoxPos {
    fn pos(&self) -> ErrorPos;
}

pub trait LoxError: LoxPos + Display {
    fn display_err(&self, source: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pos = self.pos();
        writeln!(f, "\x1b[1;36m{pos} \x1b[39m{self}\x1b[0m")?;
        pos.display_err(source, f)?;
        write!(f, "\n\x1b[1;36m\\\x1b[0m")
    }
}

pub mod run_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{
        ErrorPos, LoxError, LoxPos, anaylysis_err::ResolverErr, eval_err::EvaluateError, parse_err::ParseError, runtime_err::RuntimeError, scan_err::ScanError
    };

    #[derive(Debug)]
    enum RunErrorKind {
        IoError(std::io::Error),
        ScanErrors(Vec<ScanError>),
        ParseErrors(Vec<ParseError>),
        ResolverErrors(Vec<ResolverErr>),
        EvaluateError(EvaluateError),
        RuntimeError(RuntimeError),
    }

    #[derive(Debug)]
    pub struct RunError {
        source: Option<String>,
        kind: RunErrorKind,
    }

    impl RunError {
        pub fn set_source(&mut self, source: String) {
            self.source = Some(source)
        }
    }

    impl LoxPos for RunError {
        fn pos(&self) -> ErrorPos {
            match &self.kind {
                RunErrorKind::IoError(_) => panic!("IoError does not have a position"),
                RunErrorKind::ScanErrors(scan_errors) => {
                    assert!(
                        !scan_errors.is_empty(),
                        "Does not have any position due to scan errors being empty"
                    );

                    let from = scan_errors[0].pos();
                    let to = scan_errors.last().unwrap().pos(); // Already checked len

                    from.span_to(to)
                }
                RunErrorKind::ParseErrors(parse_errors) => {
                    assert!(
                        !parse_errors.is_empty(),
                        "Does not have any position due to parse errors being empty"
                    );

                    let from = parse_errors[0].pos();
                    let to = parse_errors.last().unwrap().pos(); // Already checked len

                    from.span_to(to)
                }
                RunErrorKind::ResolverErrors(resolver_errs) => {
                    assert!(
                        !resolver_errs.is_empty(),
                        "Does not have any position due to resolver errors being empty"
                    );

                    let from = resolver_errs[0].pos();
                    let to = resolver_errs.last().unwrap().pos(); // Already checked len

                    from.span_to(to)}
                RunErrorKind::EvaluateError(evaluate_error) => evaluate_error.pos(),
                RunErrorKind::RuntimeError(runtime_error) => runtime_error.pos(),
            }
        }
    }

    impl Display for RunError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            if let Some(source) = &self.source {
                match &self.kind {
                    RunErrorKind::IoError(err) => write!(f, "\x1b[1mIo error:\n{err}\x1b[0m")?,
                    RunErrorKind::ScanErrors(errs) => {
                        for e in errs {
                            writeln!(f)?;
                            e.display_err(source, f)?;
                        }
                    }
                    RunErrorKind::ParseErrors(errs) => {
                        for e in errs {
                            writeln!(f)?;
                            e.display_err(source, f)?;
                        }
                    }
                    RunErrorKind::ResolverErrors(errs) => {
                        for e in errs {
                            writeln!(f)?;
                            e.display_err(source, f)?;
                        }
                    },
                    RunErrorKind::EvaluateError(err) => {
                        err.display_err(source, f)?;
                    }
                    RunErrorKind::RuntimeError(err) => {
                        err.display_err(source, f)?;
                    }
                }
            } else {
                match &self.kind {
                    RunErrorKind::IoError(err) => write!(f, "Io error:\n{err}")?,
                    RunErrorKind::ScanErrors(errs) => {
                        write!(f, "Scan errors:")?;
                        for e in errs {
                            writeln!(f)?;
                            write!(f, "\n{} {e}", e.pos())?
                        }
                    }
                    RunErrorKind::ParseErrors(errs) => {
                        write!(f, "Parse errors:")?;
                        for e in errs {
                            writeln!(f)?;
                            write!(f, "\n{e}")?
                        }
                    }
                    RunErrorKind::ResolverErrors(errs) => {
                        write!(f, "Reolver errors:")?;
                        for e in errs {
                            writeln!(f)?;
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
    translate_err_impl!(Vec<ResolverErr>, RunErrorKind::ResolverErrors);
    translate_err_impl!(EvaluateError, RunErrorKind::EvaluateError);
    translate_err_impl!(RuntimeError, RunErrorKind::RuntimeError);
}

pub mod scan_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{ErrorPos, LoxError, LoxPos};

    #[derive(Debug, Clone, Copy)]
    pub struct ScanError {
        kind: ScanErrorKind,
        pos: ErrorPos,
    }

    impl ScanError {
        pub fn new(kind: ScanErrorKind, pos: ErrorPos) -> Self {
            Self { kind, pos }
        }
    }

    impl Display for ScanError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.kind)
        }
    }

    impl LoxPos for ScanError {
        fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl LoxError for ScanError {}

    impl Error for ScanError {}

    #[derive(Debug, Clone, Copy)]
    pub enum ScanErrorKind {
        UnexpectedCharacter(char),
        UnterminatedString,
        #[cfg(feature = "inline_comments")]
        UnterminatedComment,
    }

    impl Display for ScanErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ScanErrorKind::UnexpectedCharacter(c) => write!(f, "Unexpected character '{c}'"),
                ScanErrorKind::UnterminatedString => write!(f, "Unterminated string"),
                #[cfg(feature = "inline_comments")]
                ScanErrorKind::UnterminatedComment => write!(f, "Unterminated comment"),
            }
        }
    }
}

pub mod parse_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::{
        error::{ErrorPos, LoxError, LoxPos},
        token::TokenKind,
    };
    #[derive(Debug, Clone)]
    pub struct ParseError {
        pub kind: ParseErrorKind,
        pos: ErrorPos,
    }

    impl ParseError {
        pub fn new(kind: ParseErrorKind, pos: ErrorPos) -> Self {
            Self { kind, pos }
        }
    }

    impl Display for ParseError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.kind)
        }
    }

    impl LoxPos for ParseError {
        fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl LoxError for ParseError {}

    impl Error for ParseError {}

    #[derive(Debug, Clone)]
    pub enum ParseErrorKind {
        UnexpectedTokenKind(TokenKind),
        ExpectedTokenKind(TokenKind),
        InvalidAssignTarget(String),
        TooManyArguments,
        ExpectedName(Named),
        ReturnOutsideFunction,
    }

    impl Display for ParseErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ParseErrorKind::UnexpectedTokenKind(token_kind) => {
                    write!(f, "Unexpected token kind: [{:?}]", token_kind)
                }
                ParseErrorKind::ExpectedTokenKind(token_kind) => {
                    write!(f, "Expected token kind: [{:?}]", token_kind)
                }
                ParseErrorKind::InvalidAssignTarget(s) => {
                    write!(f, "Invalid assignment target [{s}]")
                }
                ParseErrorKind::TooManyArguments => {
                    write!(f, "Function can't have over 255 arguments")
                }
                ParseErrorKind::ExpectedName(named) => write!(f, "Expected {named} name"),
                ParseErrorKind::ReturnOutsideFunction => {
                    write!(f, "Cannot return outside a function body")
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub enum Named {
        Function,
        Parameter,
    }

    impl Display for Named {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Named::Function => write!(f, "function"),
                Named::Parameter => write!(f, "parameter"),
            }
        }
    }
}

pub mod anaylysis_err {
    use std::fmt::Display;

    use crate::tree_walk::error::{ErrorPos, LoxError, LoxPos};

    #[derive(Debug, Clone)]
    pub struct ResolverErr {
        pos: ErrorPos,
        kind: ResolverErrKind,
    }

    impl ResolverErr {
        pub fn new(kind: ResolverErrKind, pos: ErrorPos) -> Self {
            Self { pos, kind }
        }
    }

    impl Display for ResolverErr {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.kind)
        }
    }

    impl LoxPos for ResolverErr {
        fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl LoxError for ResolverErr {}

    #[derive(Debug, Clone)]
    pub enum ResolverErrKind {
        ReadInOwnInitializer,
        DefinedMultipleTimes(String),
        ReturnFromTop,
    }

    impl Display for ResolverErrKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ResolverErrKind::ReadInOwnInitializer => {
                    write!(f, "Tried to read variable in its own initializer")
                },
                ResolverErrKind::DefinedMultipleTimes(name) => write!(f, "Already a variablw with this name in scope: {name}"),
                ResolverErrKind::ReturnFromTop => write!(f, "Tried to return outside of a function"),
            }
        }
    }
}

pub mod runtime_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{
        ErrorPos, LoxError, LoxPos, eval_err::EvaluateError, fn_err::FunctionError,
    };

    #[derive(Debug, Clone)]
    pub struct RuntimeError {
        pub kind: RuntimeErrorKind,
    }

    impl RuntimeError {
        pub fn new(kind: RuntimeErrorKind) -> Self {
            Self { kind }
        }
    }

    impl Display for RuntimeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.kind)
        }
    }

    impl LoxPos for RuntimeError {
        fn pos(&self) -> ErrorPos {
            match &self.kind {
                RuntimeErrorKind::EvaluateError(evaluate_error) => evaluate_error.pos(),
                RuntimeErrorKind::FunctionError(function_error) => function_error.pos(),
                RuntimeErrorKind::TooManyArguments(error_pos) => *error_pos,
            }
        }
    }

    impl LoxError for RuntimeError {
        fn display_err(&self, source: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.kind {
                RuntimeErrorKind::EvaluateError(evaluate_error) => {
                    evaluate_error.display_err(source, f)
                }
                RuntimeErrorKind::FunctionError(function_error) => {
                    function_error.display_err(source, f)
                }
                RuntimeErrorKind::TooManyArguments(error_pos) => {
                    writeln!(f, "\x1b[1;36m{error_pos} \x1b[39m{self}\x1b[0m")?;
                    error_pos.display_err(source, f)?;
                    write!(f, "\n\x1b[1;36m\\\x1b[0m")
                }
            }
        }
    }

    impl Error for RuntimeError {}

    #[derive(Debug, Clone)]
    pub enum RuntimeErrorKind {
        EvaluateError(EvaluateError),
        FunctionError(FunctionError),
        TooManyArguments(ErrorPos),
    }

    impl Display for RuntimeErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                RuntimeErrorKind::EvaluateError(err) => write!(f, "{err}"),
                RuntimeErrorKind::FunctionError(err) => write!(f, "{err}"),
                RuntimeErrorKind::TooManyArguments(..) => {
                    write!(f, "Too many arguments for function, expected at most 255")
                }
            }
        }
    }

    impl From<EvaluateError> for RuntimeError {
        fn from(value: EvaluateError) -> Self {
            Self {
                kind: RuntimeErrorKind::EvaluateError(value),
            }
        }
    }

    impl From<FunctionError> for RuntimeError {
        fn from(value: FunctionError) -> Self {
            Self {
                kind: RuntimeErrorKind::FunctionError(value),
            }
        }
    }
}

pub mod eval_err {
    use std::fmt::Display;

    use crate::tree_walk::{
        error::{ErrorPos, LoxError, LoxPos},
        lox_var::{LoxVar, LoxVarKind},
        token::TokenKind,
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

        pub fn bin_eval_error(
            pos: ErrorPos,
            kind: TokenKind,
            left_var: &LoxVar,
            right_var: &LoxVar,
        ) -> Self {
            Self {
                kind: EvaluateErrorKind::IncompatableBinaryOperator {
                    operator: kind,
                    left_ty: left_var.into(),
                    right_ty: right_var.into(),
                },
                pos,
            }
        }
    }

    impl LoxPos for EvaluateError {
        fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl LoxError for EvaluateError {}

    impl Display for EvaluateError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.kind)
        }
    }

    #[derive(Debug, Clone)]
    pub enum EvaluateErrorKind {
        IncompatableUnaryOperator {
            operator: TokenKind,
            var_ty: LoxVarKind,
        },
        IncompatableBinaryOperator {
            operator: TokenKind,
            left_ty: LoxVarKind,
            right_ty: LoxVarKind,
        },
        UndeclaredVariable {
            name: String,
        },
        NonCallable {
            name: String,
        },
        IncorrectArity {
            expected: u8,
            got: usize,
        },
        IncompatableConversion {
            from: LoxVarKind,
            to: LoxVarKind,
        },
        DivisionByZero,
    }

    impl Display for EvaluateErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                EvaluateErrorKind::IncompatableUnaryOperator { operator, var_ty } => write!(
                    f,
                    "Cannot use unary operator [{operator:?}] on type [{var_ty:?}]",
                ),
                EvaluateErrorKind::IncompatableBinaryOperator {
                    operator,
                    left_ty,
                    right_ty,
                } => write!(
                    f,
                    "Cannot use binary operator [{operator:?}] on types [{left_ty:?}, {right_ty:?}]",
                ),
                EvaluateErrorKind::UndeclaredVariable { name } => {
                    write!(f, "Tried to access undeclared variable with name {name}")
                }
                EvaluateErrorKind::NonCallable { name } => {
                    write!(f, "Could not call {}", name)
                }
                EvaluateErrorKind::IncorrectArity { expected, got } => {
                    write!(
                        f,
                        "Expected {expected} args for function call but recieved {got}",
                    )
                }
                EvaluateErrorKind::IncompatableConversion { from, to } => {
                    write!(f, "Could not convert from {from:?} to {to:?}")
                }
                EvaluateErrorKind::DivisionByZero => write!(f, "Tried to divide by zero"),
            }
        }
    }

    impl From<EvaluateError> for EvaluateErrorKind {
        fn from(value: EvaluateError) -> Self {
            value.kind
        }
    }
}

pub mod fn_err {
    use std::fmt::Display;

    use crate::tree_walk::{
        error::{
            ErrorPos, LoxError, LoxPos,
            runtime_err::{RuntimeError, RuntimeErrorKind},
        },
        lox_var::LoxVar,
    };

    #[derive(Debug, Clone)]
    pub struct FunctionError {
        pub call_pos: Option<Box<ErrorPos>>,
        pub sub_pos: Option<Box<ErrorPos>>,
        pub input_args: Vec<LoxVar>,
        kind: FunctionErrorKind,
    }

    impl FunctionError {
        pub fn new(kind: FunctionErrorKind) -> Self {
            Self {
                kind,
                call_pos: None,
                sub_pos: None,
                input_args: Vec::new(),
            }
        }
    }

    impl LoxPos for FunctionError {
        fn pos(&self) -> ErrorPos {
            *self
                .call_pos
                .clone()
                .expect("Call pos needs to be set for function error")
        }
    }

    impl LoxError for FunctionError {
        fn display_err(&self, source: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let call_pos = *self
                .call_pos
                .clone()
                .expect("Call pos needs to be inserted to produce errors");

            writeln!(
                f,
                "\x1b[1;36m{call_pos} \x1b[39mFunction errored with error: {self}\x1b[0m"
            )?;
            writeln!(
                f,
                "\x1b[1;36m|\x1b[0m \x1b[1mFunction called here with arguments ({})\x1b[0m",
                self.input_args
                    .iter()
                    .enumerate()
                    .map(|(i, v)| if i == 0 {
                        v.to_string()
                    } else {
                        ", ".to_owned() + &v.to_string()
                    })
                    .collect::<String>()
            )?;
            call_pos.display_err(source, f)?;

            if let Some(sub_pos) = self.sub_pos.as_ref() {
                write!(
                    f,
                    "\n\x1b[1;36m{sub_pos}\x1b[0m \x1b[1mActual error occured here\x1b[0m\n"
                )?;
                sub_pos.display_err(source, f)?;
            }

            write!(f, "\n\x1b[1;36m\\\x1b[0m")
        }
    }

    impl Display for FunctionError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.kind)
        }
    }

    #[derive(Debug, Clone)]
    pub enum FunctionErrorKind {
        IncorrectArity { expected: u8, got: usize },
        SubError(Box<RuntimeErrorKind>),
    }

    impl Display for FunctionErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                FunctionErrorKind::IncorrectArity { expected, got } => write!(
                    f,
                    "Icorrect number of arguments for function call, expected {expected} got {got}"
                ),
                FunctionErrorKind::SubError(runtime_error) => write!(f, "{}", runtime_error),
            }
        }
    }

    impl From<RuntimeError> for FunctionError {
        fn from(value: RuntimeError) -> Self {
            Self {
                call_pos: None,
                sub_pos: Some(Box::new(value.pos())),
                input_args: Vec::new(),
                kind: FunctionErrorKind::SubError(Box::new(value.kind)),
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ErrorPos {
    line: usize,
    column: usize,
    columns: usize,
    lines: usize,
}

impl ErrorPos {
    pub fn new(line: usize, column: usize, columns: usize, lines: usize) -> Self {
        Self {
            line,
            column,
            columns,
            lines,
        }
    }

    pub fn end(&self) -> Self {
        Self {
            line: self.line + (self.lines - 1),
            column: self.column + self.columns,
            columns: 1,
            lines: 1,
        }
    }

    /// # Panics
    /// Panics if to is less then from.
    pub fn span_to(&self, to: Self) -> Self {
        assert!(
            to.line >= self.line,
            "The line of the second is before the first"
        );
        if self.line == to.line {
            assert!(
                to.column >= self.column,
                "The column of the second is before the first"
            );
        }

        Self {
            line: self.line,
            column: self.column,
            columns: if self.line == to.line {
                (to.column + to.columns) - self.column
            } else {
                to.column + to.columns
            },
            lines: self.lines + ((to.line + to.lines - 1) - self.line),
        }
    }
}

impl Display for ErrorPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[Line {}:{}]", self.line, self.column)
    }
}

impl LoxPos for ErrorPos {
    fn pos(&self) -> ErrorPos {
        *self
    }
}

impl LoxError for ErrorPos {
    fn display_err(&self, source: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let lines = source.lines().collect::<Vec<&str>>()
            [self.line - 1..(self.line + self.lines) - 1]
            .to_vec();

        match self.lines {
            0 => Ok(()),
            1 => {
                write!(f, "\x1b[1;36m|\x1b[0m  {}", lines[0])?;
                write!(
                    f,
                    "\n\x1b[1;36m|\x1b[0m {}\x1b[31m{}\x1b[0m",
                    " ".repeat(self.column),
                    "^".repeat(self.columns)
                )
            }
            2 => {
                write!(f, "\x1b[1;36m|\x1b[0m  {}", lines[0])?;
                write!(
                    f,
                    "\n\x1b[1;36m|\x1b[0m{}\x1b[31m{}\x1b[0m",
                    " ".repeat(self.column),
                    "^".repeat(lines[0].len() - self.column)
                )?;

                write!(f, "\n\x1b[1;36m|\x1b[0m  {}", lines[1])?;
                write!(
                    f,
                    "\n\x1b[1;36m|\x1b[0m  \x1b[31m{}\x1b[0m",
                    "^".repeat(self.columns)
                )
            }
            _ => {
                // > 2
                write!(f, "\x1b[1;36m|\x1b[0m  {}", lines[0])?;
                write!(
                    f,
                    "\n\x1b[1;36m|\x1b[0m{}\x1b[31m{}\x1b[0m",
                    " ".repeat(self.column),
                    "^".repeat(lines[0].len() - self.column)
                )?;

                for l in &lines[1..lines.len() - 2] {
                    write!(f, "\n\x1b[1;36m|\x1b[0m  {}", l)?;
                    write!(
                        f,
                        "\n\x1b[1;36m|\x1b[0m  \x1b[31m{}\x1b[0m",
                        "^".repeat(l.len())
                    )?;
                }

                write!(
                    f,
                    "\n\x1b[1;36m|\x1b[0m  {}",
                    lines
                        .last()
                        .expect("This iterator must have at leat 3 elements")
                )?;
                write!(
                    f,
                    "\n\x1b[1;36m|\x1b[0m  \x1b[31m{}\x1b[0m",
                    "^".repeat(self.columns)
                )
            }
        }
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
            columns: match value.kind {
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
            lines: 1,
        }
    }
}

impl From<Token<'_>> for ErrorPos {
    fn from(value: Token<'_>) -> Self {
        use super::token::TokenKind::*;
        Self {
            line: value.line,
            column: if value.kind == String {
                value.column - 1
            } else {
                value.column
            },
            columns: match value.kind {
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
            lines: 1,
        }
    }
}
