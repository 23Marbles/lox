use std::{error::Error, fmt::Display};

use crate::tree_walk::{error::diagnostic::ErrorTime, token::Token};

pub trait LoxPos {
    fn pos(&self) -> ErrorPos;
}

impl<T: LoxPos> LoxPos for Vec<T> {
    fn pos(&self) -> ErrorPos {
        assert!(!self.is_empty(), "Cannot find the pos of an empty vector");
        self[0].pos().span_to(self.last().unwrap().pos())
    }
}

pub trait DisplayLoxError: LoxPos + Display {
    fn display_err(&self, source: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let pos = self.pos();
        writeln!(f, "\x1b[1;36m{pos} \x1b[39m{self}\x1b[0m")?;
        pos.display_err(source, f)?;
        write!(f, "\n\x1b[1;36m\\\x1b[0m")
    }

    fn lox_msg(&self) -> String {
        self.to_string()
    }
}

pub trait LoxError: Error + DisplayLoxError {
    fn time(&self) -> ErrorTime;
}

pub mod diagnostic {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{DisplayLoxError, ErrorPos, LoxError, LoxPos, runtime_err::RuntimeError};

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum ErrorTime {
        Scanner,
        Parser,
        Resolver,
        Runtime,
        Input,
    }

    pub(super) trait IntoDiagnostic: LoxError {
        fn into_diagnostic(&self) -> Diagnostic {
            Diagnostic {
                pos: self.pos(),
                time: self.time(),
                msg: self.to_string(),
            }
        }
    }

    impl<T: LoxError> IntoDiagnostic for T {}

    #[derive(Debug, Clone, PartialEq)]
    pub struct Diagnostic {
        pos: ErrorPos,
        time: ErrorTime,
        msg: String,
    }

    impl Display for Diagnostic {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.msg)
        }
    }

    impl DisplayLoxError for Diagnostic {}

    impl Error for Diagnostic {}

    impl LoxError for Diagnostic {
        fn time(&self) -> ErrorTime {
            self.time
        }
    }

    impl LoxPos for Diagnostic {
        fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl<T: LoxError> From<&T> for Diagnostic {
        fn from(value: &T) -> Self {
            Self {
                pos: value.pos(),
                msg: value.lox_msg(),
                time: value.time(),
            }
        }
    }

    impl From<std::io::Error> for Diagnostic {
        fn from(value: std::io::Error) -> Self {
            Self {
                pos: ErrorPos::Outside,
                time: ErrorTime::Input,
                msg: value.to_string(),
            }
        }
    }

    impl From<RuntimeError> for Vec<Diagnostic> {
        fn from(value: RuntimeError) -> Self {
            vec![value.into_diagnostic()]
        }
    }

    impl From<std::fmt::Error> for Diagnostic {
        fn from(value: std::fmt::Error) -> Self {
            Self {
                pos: ErrorPos::Outside,
                time: ErrorTime::Input,
                msg: value.to_string(),
            }
        }
    }
}

pub mod run_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{
        DisplayLoxError, ErrorPos, LoxPos,
        diagnostic::{Diagnostic, IntoDiagnostic},
    };

    #[derive(Debug)]
    pub enum RunErrorKind {
        IoError(std::io::Error),
        Diagnostics(Vec<Diagnostic>),
    }

    #[derive(Debug)]
    pub struct RunError {
        pub kind: RunErrorKind,
    }

    impl PartialEq<Vec<Diagnostic>> for RunError {
        fn eq(&self, other: &Vec<Diagnostic>) -> bool {
            if let RunErrorKind::Diagnostics(diagnostics) = &self.kind {
                diagnostics.len() == other.len()
                    && diagnostics.iter().zip(other).all(|(s, o)| s == o)
            } else {
                false
            }
        }
    }

    #[derive(Debug)]
    pub struct RunErrorSourced {
        source: String,
        err: RunError,
    }

    impl Display for RunErrorSourced {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.err.display_err(&self.source, f)
        }
    }

    impl From<std::io::Error> for RunErrorSourced {
        fn from(value: std::io::Error) -> Self {
            Self {
                source: "".to_owned(),
                err: RunError::from(value),
            }
        }
    }

    impl RunError {
        pub fn with_source(self, source: String) -> RunErrorSourced {
            RunErrorSourced { source, err: self }
        }
    }

    impl LoxPos for RunError {
        fn pos(&self) -> ErrorPos {
            match &self.kind {
                RunErrorKind::IoError(_) => ErrorPos::Outside,
                RunErrorKind::Diagnostics(diags) => diags.pos(),
            }
        }
    }

    impl DisplayLoxError for RunError {
        fn display_err(&self, source: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.kind {
                RunErrorKind::IoError(err) => write!(f, "\x1b[1mIo Error: {err}\x1b[0m"),
                RunErrorKind::Diagnostics(diagnostics) => {
                    if let Some(diag) = diagnostics.get(0) {
                        diag.display_err(source, f)?;

                        for d in diagnostics.iter().skip(1) {
                            writeln!(f)?;
                            d.display_err(source, f)?;
                        }

                        Ok(())
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }

    impl Display for RunError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.kind {
                RunErrorKind::IoError(err) => write!(f, "\x1b[1mIo Error: {err}\x1b[0m"),
                RunErrorKind::Diagnostics(diagnostics) => {
                    if let Some(diag) = diagnostics.get(0) {
                        write!(f, "{diag}")?;

                        for d in diagnostics.iter().skip(1) {
                            writeln!(f)?;
                            write!(f, "{d}")?;
                        }

                        Ok(())
                    } else {
                        Ok(())
                    }
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

    impl<T: IntoDiagnostic> From<Vec<T>> for RunError {
        fn from(value: Vec<T>) -> Self {
            Self {
                kind: RunErrorKind::Diagnostics(
                    value.iter().map(|t| t.into_diagnostic()).collect(),
                ),
            }
        }
    }

    impl<T: IntoDiagnostic> From<T> for RunError {
        fn from(value: T) -> Self {
            Self {
                kind: RunErrorKind::Diagnostics(vec![value.into_diagnostic()]),
            }
        }
    }
}

pub mod scan_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{
        DisplayLoxError, ErrorPos, LoxError, LoxPos, diagnostic::ErrorTime,
    };

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

    impl DisplayLoxError for ScanError {}

    impl LoxPos for ScanError {
        fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl LoxError for ScanError {
        fn time(&self) -> ErrorTime {
            ErrorTime::Scanner
        }
    }

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
        error::{DisplayLoxError, ErrorPos, LoxError, LoxPos, diagnostic::ErrorTime},
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

    impl DisplayLoxError for ParseError {}

    impl LoxPos for ParseError {
        fn pos(&self) -> ErrorPos {
            self.pos
        }
    }

    impl LoxError for ParseError {
        fn time(&self) -> ErrorTime {
            ErrorTime::Parser
        }
    }

    impl Error for ParseError {}

    #[derive(Debug, Clone)]
    pub enum ParseErrorKind {
        UnexpectedTokenKind(TokenKind),
        ExpectedTokenKind(TokenKind),
        InvalidAssignTarget(String),
        TooManyArguments,
        ExpectedName { name: Named, after: TokenKind },
    }

    impl Display for ParseErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ParseErrorKind::UnexpectedTokenKind(token_kind) => {
                    write!(f, "Unexpected token kind: `{:?}`", token_kind)
                }
                ParseErrorKind::ExpectedTokenKind(token_kind) => {
                    write!(f, "Expected token kind: `{:?}`", token_kind)
                }
                ParseErrorKind::InvalidAssignTarget(s) => {
                    write!(f, "Invalid assignment target `{s}`")
                }
                ParseErrorKind::TooManyArguments => {
                    write!(f, "Function can't have over 255 arguments")
                }
                ParseErrorKind::ExpectedName { name, after } => {
                    write!(f, "Expected {name} name after token `{after}`")
                }
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    pub enum Named {
        Function,
        Parameter,
        Class,
        SuperClass,
        Method,
        Property,
    }

    impl Display for Named {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Named::Function => write!(f, "function"),
                Named::Parameter => write!(f, "parameter"),
                Named::Class => write!(f, "class"),
                Named::SuperClass => write!(f, "super class"),
                Named::Method => write!(f, "method"),
                Named::Property => write!(f, "property"),
            }
        }
    }
}

pub mod anaylysis_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::error::{
        DisplayLoxError, ErrorPos, LoxError, LoxPos, diagnostic::ErrorTime,
    };

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

    impl DisplayLoxError for ResolverErr {}

    impl Error for ResolverErr {}

    impl LoxError for ResolverErr {
        fn time(&self) -> ErrorTime {
            ErrorTime::Resolver
        }
    }

    #[derive(Debug, Clone)]
    pub enum ResolverErrKind {
        ReadInOwnInitializer,
        DefinedMultipleTimes(String),
        ReturnFromTop,
        ReturnValueFromInit,
        ThisOutsideClass,
        SelfReferentialInheritance,
        SuperOutsideSubclass,
    }

    impl Display for ResolverErrKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ResolverErrKind::ReadInOwnInitializer => {
                    write!(f, "Can't read variable in its own initializer")
                }
                ResolverErrKind::DefinedMultipleTimes(name) => {
                    write!(f, "Already a variablw with this name in scope: {name}")
                }
                ResolverErrKind::ReturnFromTop => {
                    write!(f, "Can't return outside of a function")
                }
                ResolverErrKind::ThisOutsideClass => {
                    write!(f, "Can't use `this` outside a class method")
                }
                ResolverErrKind::ReturnValueFromInit => {
                    write!(f, "Can't return a value from an initializer")
                }
                ResolverErrKind::SelfReferentialInheritance => {
                    write!(f, "Classes cannot inherit from themselves")
                }
                ResolverErrKind::SuperOutsideSubclass => {
                    write!(f, "Can't use super outside a subclass")
                }
            }
        }
    }
}

pub mod runtime_err {
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::{
        error::{
            DisplayLoxError, ErrorPos, LoxError, LoxPos, diagnostic::ErrorTime,
            eval_err::EvaluateError, fn_err::FunctionError,
        },
        lox_var::LoxVar,
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
                RuntimeErrorKind::TooManyArguments(error_pos)
                | RuntimeErrorKind::SuperClassIsNotAClass(.., error_pos)
                | RuntimeErrorKind::StackOverflow(error_pos) => *error_pos,
            }
        }
    }

    impl DisplayLoxError for RuntimeError {
        fn display_err(&self, source: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self.kind {
                RuntimeErrorKind::EvaluateError(evaluate_error) => {
                    evaluate_error.display_err(source, f)
                }
                RuntimeErrorKind::FunctionError(function_error) => {
                    function_error.display_err(source, f)
                }
                RuntimeErrorKind::TooManyArguments(error_pos)
                | RuntimeErrorKind::SuperClassIsNotAClass(.., error_pos)
                | RuntimeErrorKind::StackOverflow(error_pos) => {
                    writeln!(f, "\x1b[1;36m{error_pos} \x1b[39m{self}\x1b[0m")?;
                    error_pos.display_err(source, f)?;
                    write!(f, "\n\x1b[1;36m\\\x1b[0m")
                }
            }
        }
    }

    impl LoxError for RuntimeError {
        fn time(&self) -> ErrorTime {
            ErrorTime::Runtime
        }
    }

    impl Error for RuntimeError {}

    #[derive(Debug, Clone)]
    pub enum RuntimeErrorKind {
        EvaluateError(EvaluateError),
        FunctionError(FunctionError),
        TooManyArguments(ErrorPos),
        SuperClassIsNotAClass(LoxVar, ErrorPos),
        StackOverflow(ErrorPos),
    }

    impl Display for RuntimeErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                RuntimeErrorKind::EvaluateError(err) => write!(f, "{err}"),
                RuntimeErrorKind::FunctionError(err) => write!(f, "{err}"),
                RuntimeErrorKind::TooManyArguments(..) => {
                    write!(f, "Too many arguments for function, expected at most 255")
                }
                RuntimeErrorKind::SuperClassIsNotAClass(not_class, ..) => {
                    write!(f, "The super class `{not_class}` must be a class")
                }
                RuntimeErrorKind::StackOverflow(..) => write!(f, "Max call depth reached"),
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
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::{
        error::{DisplayLoxError, ErrorPos, LoxError, LoxPos, diagnostic::ErrorTime},
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

    impl Error for EvaluateError {}

    impl DisplayLoxError for EvaluateError {}

    impl LoxError for EvaluateError {
        fn time(&self) -> ErrorTime {
            ErrorTime::Runtime
        }
    }

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
        UndefinedProperty(String),
        NotInstance(LoxVarKind),
    }

    impl Display for EvaluateErrorKind {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match &self {
                EvaluateErrorKind::IncompatableUnaryOperator { operator, var_ty } => write!(
                    f,
                    "Cannot use unary operator `{operator:?}` on type `{var_ty:?}`",
                ),
                EvaluateErrorKind::IncompatableBinaryOperator {
                    operator,
                    left_ty,
                    right_ty,
                } => write!(
                    f,
                    "Cannot use binary operator `{operator:?}` on types `{left_ty:?}`, `{right_ty:?}`",
                ),
                EvaluateErrorKind::UndeclaredVariable { name } => {
                    write!(f, "Tried to access undeclared variable with name {name}")
                }
                EvaluateErrorKind::NonCallable { name } => {
                    write!(f, "Could not call `{name}`")
                }
                EvaluateErrorKind::IncorrectArity { expected, got } => {
                    write!(
                        f,
                        "Expected {expected} args for function call but recieved {got}",
                    )
                }
                EvaluateErrorKind::IncompatableConversion { from, to } => {
                    write!(f, "Could not convert from `{from:?}` to `{to:?}`")
                }
                EvaluateErrorKind::DivisionByZero => write!(f, "Tried to divide by zero"),
                EvaluateErrorKind::UndefinedProperty(name) => {
                    write!(f, "Property `{name}` is not defined")
                }
                EvaluateErrorKind::NotInstance(kind) => {
                    write!(
                        f,
                        "Tried to access a property of a non-instance, is of type `{kind:?}` instead"
                    )
                }
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
    use std::{error::Error, fmt::Display};

    use crate::tree_walk::{
        error::{
            DisplayLoxError, ErrorPos, LoxError, LoxPos,
            diagnostic::ErrorTime,
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

    impl Error for FunctionError {}

    impl DisplayLoxError for FunctionError {
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

    impl LoxError for FunctionError {
        fn time(&self) -> ErrorTime {
            ErrorTime::Runtime
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorPos {
    InFile {
        line: usize,
        column: usize,
        columns: usize,
        lines: usize,
    },
    Outside,
}

impl ErrorPos {
    pub fn new(line: usize, column: usize, columns: usize, lines: usize) -> Self {
        Self::InFile {
            line,
            column,
            columns,
            lines,
        }
    }

    pub fn end(&self) -> Self {
        if let ErrorPos::InFile {
            line,
            column,
            columns,
            lines,
        } = self
        {
            ErrorPos::InFile {
                line: line + lines - 1,
                column: column + columns,
                columns: 1,
                lines: 1,
            }
        } else {
            ErrorPos::Outside
        }
    }

    /// # Panics
    /// Panics if to is less then from.
    pub fn span_to(&self, to: Self) -> Self {
        match (self, to) {
            (
                ErrorPos::InFile {
                    line,
                    column,
                    lines,
                    ..
                },
                ErrorPos::InFile {
                    line: to_line,
                    column: to_column,
                    columns: to_columns,
                    lines: to_lines,
                },
            ) => {
                assert!(
                    *line < to_line,
                    "The line of the second is before the first"
                );
                assert!(
                    to_line > *line || (to_line == *line && to_column >= *column),
                    "End position before first position"
                );

                Self::InFile {
                    line: *line,
                    column: *column,
                    columns: if *line == to_line {
                        (to_column + to_columns) - column
                    } else {
                        to_column + to_columns
                    },
                    lines: *lines + ((to_line + to_lines - 1) - line),
                }
            }
            _ => ErrorPos::Outside,
        }
    }
}

impl Display for ErrorPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let ErrorPos::InFile { line, column, .. } = self {
            write!(f, "[Line {}:{}]", line, column)
        } else {
            Ok(())
        }
    }
}

impl LoxPos for ErrorPos {
    fn pos(&self) -> ErrorPos {
        *self
    }
}

impl DisplayLoxError for ErrorPos {
    fn display_err(&self, source: &str, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let ErrorPos::InFile {
            line,
            column,
            columns,
            lines: lines_num,
        } = self
        {
            let lines =
                source.lines().collect::<Vec<&str>>()[line - 1..(line + lines_num) - 1].to_vec();

            match lines_num {
                0 => Ok(()),
                1 => {
                    write!(f, "\x1b[1;36m|\x1b[0m  {}", lines[0])?;
                    write!(
                        f,
                        "\n\x1b[1;36m|\x1b[0m {}\x1b[31m{}\x1b[0m",
                        " ".repeat(*column),
                        "^".repeat(*columns)
                    )
                }
                2 => {
                    write!(f, "\x1b[1;36m|\x1b[0m  {}", lines[0])?;
                    write!(
                        f,
                        "\n\x1b[1;36m|\x1b[0m{}\x1b[31m{}\x1b[0m",
                        " ".repeat(*column),
                        "^".repeat(lines[0].len() - column)
                    )?;

                    write!(f, "\n\x1b[1;36m|\x1b[0m  {}", lines[1])?;
                    write!(
                        f,
                        "\n\x1b[1;36m|\x1b[0m  \x1b[31m{}\x1b[0m",
                        "^".repeat(*columns)
                    )
                }
                _ => {
                    // > 2
                    write!(f, "\x1b[1;36m|\x1b[0m  {}", lines[0])?;
                    write!(
                        f,
                        "\n\x1b[1;36m|\x1b[0m{}\x1b[31m{}\x1b[0m",
                        " ".repeat(*column),
                        "^".repeat(lines[0].len() - column)
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
                        "^".repeat(*columns)
                    )
                }
            }
        } else {
            Ok(())
        }
    }
}

impl LoxError for ErrorPos {
    /// # Not to be used
    fn time(&self) -> ErrorTime {
        unimplemented!("Time cannot be called on ErrorPos")
    }
}

impl From<&Token<'_>> for ErrorPos {
    fn from(value: &Token<'_>) -> Self {
        use super::token::TokenKind::*;
        ErrorPos::InFile {
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
        ErrorPos::InFile {
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

impl Error for ErrorPos {}
