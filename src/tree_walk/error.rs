use core::fmt::Display;
use std::{io, ops::Range};

use codespan_reporting::diagnostic::{Diagnostic, Label, LabelStyle, Severity};

use crate::tree_walk::error_codes::ErrorCode;

pub trait ToErrorCode {
    fn error_code(&self) -> ErrorCode;
}

pub trait ErrorLevel {
    fn severity(&self) -> Severity;
}

pub trait ToCodespan: ToErrorCode + ErrorLevel + Display {
    #[allow(unused)]
    fn to_codespan<I: Copy>(&self, file_id: I) -> Diagnostic<I> {
        Diagnostic::new(self.severity())
            .with_message(self.to_string())
            .with_code(self.error_code())
    }
}

pub fn diag_from_io<T>(from: io::Error) -> Diagnostic<T> {
    Diagnostic::new(Severity::Bug)
        .with_message(from.to_string())
        // Core Error 0001
        .with_code("CE0001")
}

fn to_codespan<
    T: Display + ErrorLevel + ToErrorCode,
    K: IntoIterator<Item = (LabelStyle, Range<usize>, Option<impl Display>)>,
    I: Copy,
>(
    from: &T,
    file_id: I,
    labels: K,
) -> Diagnostic<I> {
    Diagnostic::new(from.severity())
        .with_message(from.to_string())
        .with_code(from.error_code())
        .with_labels(
            labels
                .into_iter()
                .map(|(s, r, m)| Label {
                    style: s,
                    file_id,
                    range: r,
                    message: m.map(|d| d.to_string()).unwrap_or(String::new()),
                })
                .collect(),
        )
}

pub mod scan_err {
    use core::fmt::Display;

    use crate::tree_walk::{
        error::{ErrorLevel, ToErrorCode},
        error_codes::ErrorCode,
    };

    #[derive(Debug, Clone, Copy)]
    pub enum ScanErrorKind {
        UnexpectedCharacter(char),
        UnterminatedString,
        #[cfg(feature = "inline_comments")]
        UnterminatedComment,
    }

    use codespan_reporting::diagnostic::Severity;

    impl ErrorLevel for ScanErrorKind {
        fn severity(&self) -> Severity {
            match self {
                ScanErrorKind::UnexpectedCharacter(_) => Severity::Error,
                ScanErrorKind::UnterminatedString => Severity::Error,
                #[cfg(feature = "inline_comments")]
                ScanErrorKind::UnterminatedComment => Severity::Error,
            }
        }
    }

    impl ToErrorCode for ScanErrorKind {
        fn error_code(&self) -> ErrorCode {
            match self {
                ScanErrorKind::UnexpectedCharacter(_) => ErrorCode::UnexpectedCharacter,
                ScanErrorKind::UnterminatedString => ErrorCode::UnterminatedString,
                #[cfg(feature = "inline_comments")]
                ScanErrorKind::UnterminatedComment => ErrorCode::UnterminatedComment,
            }
        }
    }

    impl Display for ScanErrorKind {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
    use core::fmt::Display;
    use std::ops::Range;

    use codespan_reporting::diagnostic::{Label, LabelStyle, Severity};

    use crate::tree_walk::{
        error::{ErrorLevel, ToCodespan, ToErrorCode},
        error_codes::ErrorCode,
        token::TokenKind,
    };

    #[derive(Debug, Clone)]
    pub enum ParseError {
        UnexpectedTokenKind {
            got: TokenKind,
            expected: Vec<TokenKind>,
            span: Range<usize>,
        },
        ExpectedTokenKind {
            got: TokenKind,
            expected: TokenKind,
            span: Range<usize>,
        },
        InvalidAssignTarget {
            target: String,
            span: Range<usize>,
        },
        TooManyArguments {
            got: usize,
            spans: Vec<Range<usize>>,
        },
        ExpectedName {
            name: Named,
            after: TokenKind,
            prev_span: Range<usize>,
            span: Range<usize>,
        },
        ExpectedNumber {
            input: String,
            span: Range<usize>,
        },
    }

    impl ToErrorCode for ParseError {
        fn error_code(&self) -> ErrorCode {
            match self {
                ParseError::UnexpectedTokenKind { .. } => ErrorCode::UnexpectedTokenKind,
                ParseError::ExpectedTokenKind { .. } => ErrorCode::ExpectedTokenKind,
                ParseError::InvalidAssignTarget { .. } => ErrorCode::InvalidAssignTarget,
                ParseError::TooManyArguments { .. } => ErrorCode::TooManyArguments,
                ParseError::ExpectedName { .. } => ErrorCode::ExpectedName,
                ParseError::ExpectedNumber { .. } => ErrorCode::ExpectedNumber,
            }
        }
    }

    impl ErrorLevel for ParseError {
        fn severity(&self) -> Severity {
            match self {
                ParseError::UnexpectedTokenKind { .. } => Severity::Error,
                ParseError::ExpectedTokenKind { .. } => Severity::Error,
                ParseError::InvalidAssignTarget { .. } => Severity::Error,
                ParseError::TooManyArguments { .. } => Severity::Warning,
                ParseError::ExpectedName { .. } => Severity::Error,
                ParseError::ExpectedNumber { .. } => Severity::Error,
            }
        }
    }

    impl ToCodespan for ParseError {
        fn to_codespan<I: Copy>(
            &self,
            file_id: I,
        ) -> codespan_reporting::diagnostic::Diagnostic<I> {
            match self {
                ParseError::UnexpectedTokenKind { span, expected, .. } => {
                    codespan_reporting::diagnostic::Diagnostic::new(self.severity())
                        .with_message(self.to_string())
                        .with_code(self.error_code())
                        .with_label(Label::new(LabelStyle::Primary, file_id, span.clone()))
                        .with_note(format!(
                            "Expected one of [{}]",
                            expected
                                .iter()
                                .map(|token_kind| token_kind.to_string())
                                .collect::<Vec<String>>()
                                .join(", ")
                        ))
                }
                ParseError::TooManyArguments { spans, .. } => {
                    codespan_reporting::diagnostic::Diagnostic::new(self.severity())
                        .with_message(self.to_string())
                        .with_code(self.error_code())
                        .with_labels_iter(
                            spans.iter().map(|span| {
                                Label::new(LabelStyle::Secondary, file_id, span.clone())
                            }),
                        )
                }
                ParseError::ExpectedName {
                    span,
                    prev_span,
                    after,
                    ..
                } => codespan_reporting::diagnostic::Diagnostic::new(self.severity())
                    .with_message(self.to_string())
                    .with_code(self.error_code())
                    .with_label(Label::new(LabelStyle::Primary, file_id, span.clone()))
                    .with_label(
                        Label::new(LabelStyle::Secondary, file_id, prev_span.clone())
                            .with_message(format!("Expected after `{after:?}`")),
                    ),
                ParseError::ExpectedTokenKind { span, .. }
                | ParseError::InvalidAssignTarget { span, .. }
                | ParseError::ExpectedNumber { span, .. } => {
                    codespan_reporting::diagnostic::Diagnostic::new(self.severity())
                        .with_message(self.to_string())
                        .with_code(self.error_code())
                        .with_label(Label::new(LabelStyle::Primary, file_id, span.clone()))
                }
            }
        }
    }

    impl Display for ParseError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                ParseError::UnexpectedTokenKind { got, .. } => {
                    write!(f, "Unexpected token kind: `{:?}`", got)
                }
                ParseError::ExpectedTokenKind { got, expected, .. } => {
                    write!(f, "Expected token kind: `{:?}`, got: `{:?}`", expected, got)
                }
                ParseError::InvalidAssignTarget { target, .. } => {
                    write!(f, "Cannot assign to `{target}`")
                }
                ParseError::TooManyArguments { .. } => {
                    write!(f, "Function can't have over 255 arguments")
                }
                ParseError::ExpectedName { name, after, .. } => {
                    write!(f, "Expected {name} name after token `{after}`")
                }
                ParseError::ExpectedNumber { input, .. } => {
                    write!(f, "Could not parse `{input}` as a number")
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
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
    use core::fmt::Display;
    use std::ops::Range;

    use codespan_reporting::diagnostic::{LabelStyle, Severity};

    use crate::tree_walk::{
        error::{ErrorLevel, ToCodespan, ToErrorCode, to_codespan},
        error_codes::ErrorCode,
    };

    #[derive(Debug, Clone)]
    pub enum ResolverErr {
        ReadInOwnInitializer(Range<usize>),
        DefinedMultipleTimes(String, Vec<Range<usize>>),
        ReturnFromTop(Range<usize>),
        /// [Return pos, Init pos]
        ReturnValueFromInit([Range<usize>; 2]),
        ThisOutsideClass(Range<usize>),
        SelfReferentialInheritance(Range<usize>),
        SuperOutsideSubclass(Range<usize>),
    }

    impl ToErrorCode for ResolverErr {
        fn error_code(&self) -> ErrorCode {
            match self {
                ResolverErr::ReadInOwnInitializer(_) => ErrorCode::ReadInOwnInitilaizer,
                ResolverErr::DefinedMultipleTimes { .. } => ErrorCode::DefinedMultipleTimes,
                ResolverErr::ReturnFromTop(_) => ErrorCode::ReturnFromTop,
                ResolverErr::ReturnValueFromInit(_) => ErrorCode::ReturnValueFromInit,
                ResolverErr::ThisOutsideClass(_) => ErrorCode::ThisOutsideClass,
                ResolverErr::SelfReferentialInheritance(_) => ErrorCode::SelfReferentialInheritance,
                ResolverErr::SuperOutsideSubclass(_) => ErrorCode::SuperOutsideSubclass,
            }
        }
    }

    impl ErrorLevel for ResolverErr {
        fn severity(&self) -> Severity {
            match self {
                ResolverErr::ReadInOwnInitializer(_) => Severity::Error,
                ResolverErr::DefinedMultipleTimes { .. } => Severity::Warning,
                ResolverErr::ReturnFromTop(_) => Severity::Error,
                ResolverErr::ReturnValueFromInit(_) => Severity::Error,
                ResolverErr::ThisOutsideClass(_) => Severity::Error,
                ResolverErr::SelfReferentialInheritance(_) => Severity::Error,
                ResolverErr::SuperOutsideSubclass(_) => Severity::Error,
            }
        }
    }

    impl ToCodespan for ResolverErr {
        fn to_codespan<I: Copy>(
            &self,
            file_id: I,
        ) -> codespan_reporting::diagnostic::Diagnostic<I> {
            match self {
                ResolverErr::DefinedMultipleTimes(_, ranges) => {
                    let labels = std::iter::once((
                        LabelStyle::Primary,
                        ranges.first().unwrap().clone(),
                        Some("First defined here".to_owned()),
                    ))
                    .chain(std::iter::once((
                        LabelStyle::Secondary,
                        ranges.get(1).unwrap().clone(),
                        Some("Redefined here".to_owned()),
                    )))
                    .chain(ranges.iter().skip(2).map(|span| {
                        (
                            LabelStyle::Secondary,
                            span.clone(),
                            Some("And here".to_owned()),
                        )
                    }));

                    to_codespan(self, file_id, labels)
                }
                ResolverErr::ReturnValueFromInit([return_range, init_range]) => to_codespan(
                    self,
                    file_id,
                    [
                        (
                            LabelStyle::Primary,
                            return_range.clone(),
                            Some("Tried to return here".to_owned()),
                        ),
                        (
                            LabelStyle::Secondary,
                            init_range.clone(),
                            Some("Initializer defined here".to_owned()),
                        ),
                    ],
                ),
                ResolverErr::ReadInOwnInitializer(range)
                | ResolverErr::ReturnFromTop(range)
                | ResolverErr::ThisOutsideClass(range)
                | ResolverErr::SelfReferentialInheritance(range)
                | ResolverErr::SuperOutsideSubclass(range) => to_codespan(
                    self,
                    file_id,
                    [(LabelStyle::Primary, range.clone(), None::<String>)],
                ),
            }
        }
    }

    impl Display for ResolverErr {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                ResolverErr::ReadInOwnInitializer(_) => {
                    write!(f, "Can't read variable in its own initializer")
                }
                ResolverErr::DefinedMultipleTimes(name, ..) => {
                    write!(f, "Already a variable with name `{name}` in scope")
                }
                ResolverErr::ReturnFromTop(_) => {
                    write!(f, "Can't return outside of a function")
                }
                ResolverErr::ThisOutsideClass(_) => {
                    write!(f, "Can't use `this` outside a class method")
                }
                ResolverErr::ReturnValueFromInit(_) => {
                    write!(f, "Can't return a value from an initializer")
                }
                ResolverErr::SelfReferentialInheritance(_) => {
                    write!(f, "Classes cannot inherit from themselves")
                }
                ResolverErr::SuperOutsideSubclass(_) => {
                    write!(f, "Can't use super outside a subclass")
                }
            }
        }
    }
}

pub mod runtime_err {
    use core::fmt::Display;
    use std::ops::Range;

    use codespan_reporting::diagnostic::{Diagnostic, Label, LabelStyle, Severity};

    use crate::tree_walk::{
        error::{
            ErrorLevel, ToCodespan, ToErrorCode, eval_err::EvaluateError, fn_err::FunctionError,
        },
        error_codes::ErrorCode,
        lox_var::LoxVar,
    };

    #[derive(Debug, Clone)]
    pub enum RuntimeError {
        EvaluateError(EvaluateError),
        FunctionError(FunctionError),
        TooManyArguments(usize, Range<usize>),
        SuperClassIsNotAClass(LoxVar, Range<usize>),
        StackOverflow,
    }

    impl From<EvaluateError> for RuntimeError {
        fn from(value: EvaluateError) -> Self {
            RuntimeError::EvaluateError(value)
        }
    }

    impl From<FunctionError> for RuntimeError {
        fn from(value: FunctionError) -> Self {
            RuntimeError::FunctionError(value)
        }
    }

    impl ToErrorCode for RuntimeError {
        fn error_code(&self) -> ErrorCode {
            match self {
                RuntimeError::EvaluateError(evaluate_error) => evaluate_error.error_code(),
                RuntimeError::FunctionError(function_error) => function_error.error_code(),
                RuntimeError::TooManyArguments(..) => ErrorCode::TooManyArguments,
                RuntimeError::SuperClassIsNotAClass(..) => ErrorCode::SuperClassIsNotAClass,
                RuntimeError::StackOverflow => ErrorCode::StackOverflow,
            }
        }
    }

    impl ErrorLevel for RuntimeError {
        fn severity(&self) -> codespan_reporting::diagnostic::Severity {
            match self {
                RuntimeError::EvaluateError(evaluate_error) => evaluate_error.severity(),
                RuntimeError::FunctionError(function_error) => function_error.severity(),
                RuntimeError::TooManyArguments(..) => Severity::Warning,
                RuntimeError::SuperClassIsNotAClass(..) => Severity::Error,
                RuntimeError::StackOverflow => Severity::Error,
            }
        }
    }

    impl ToCodespan for RuntimeError {
        fn to_codespan<I: Copy>(
            &self,
            file_id: I,
        ) -> codespan_reporting::diagnostic::Diagnostic<I> {
            match self {
                RuntimeError::EvaluateError(evaluate_error) => evaluate_error.to_codespan(file_id),
                RuntimeError::FunctionError(function_error) => function_error.to_codespan(file_id),
                RuntimeError::TooManyArguments(_, range)
                | RuntimeError::SuperClassIsNotAClass(_, range) => Diagnostic::new(self.severity())
                    .with_code(self.error_code())
                    .with_message(self.to_string())
                    .with_label(Label::new(LabelStyle::Primary, file_id, range.clone())),
                RuntimeError::StackOverflow => Diagnostic::new(self.severity())
                    .with_code(self.error_code())
                    .with_message(self.to_string()),
            }
        }
    }

    impl Display for RuntimeError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                RuntimeError::EvaluateError(err) => write!(f, "{err}"),
                RuntimeError::FunctionError(err) => write!(f, "{err}"),
                RuntimeError::TooManyArguments(got, ..) => {
                    write!(
                        f,
                        "Too many arguments for function, expected at most 255, got {got}"
                    )
                }
                RuntimeError::SuperClassIsNotAClass(not_class, ..) => {
                    write!(f, "The super class `{not_class}` must be a class")
                }
                RuntimeError::StackOverflow => write!(f, "Max call depth reached"),
            }
        }
    }
}

pub mod eval_err {
    use core::{error::Error, fmt::Display};
    use std::ops::Range;

    use codespan_reporting::diagnostic::{Diagnostic, Label, LabelStyle, Severity};

    use crate::tree_walk::{
        error::{ErrorLevel, ToCodespan, ToErrorCode},
        error_codes::ErrorCode,
        lox_var::{LoxVar, LoxVarKind},
        token::TokenKind,
    };

    #[derive(Debug, Clone)]
    pub struct EvaluateError {
        kind: EvaluateErrorKind,
        pub(super) span: Range<usize>,
    }

    impl EvaluateError {
        pub fn new(kind: EvaluateErrorKind, pos: Range<usize>) -> Self {
            Self { kind, span: pos }
        }

        pub fn bin_eval_error(
            pos: Range<usize>,
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
                span: pos,
            }
        }
    }

    impl Error for EvaluateError {}

    impl Display for EvaluateError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

    impl ToErrorCode for EvaluateError {
        fn error_code(&self) -> ErrorCode {
            match self.kind {
                EvaluateErrorKind::IncompatableUnaryOperator { .. } => {
                    ErrorCode::IncompatableUnaryOperator
                }
                EvaluateErrorKind::IncompatableBinaryOperator { .. } => {
                    ErrorCode::IncompatableBinaryOperator
                }
                EvaluateErrorKind::UndeclaredVariable { .. } => ErrorCode::UndeclaredVariable,
                EvaluateErrorKind::NonCallable { .. } => ErrorCode::NonCallable,
                EvaluateErrorKind::IncorrectArity { .. } => ErrorCode::IncorrectArity,
                EvaluateErrorKind::IncompatableConversion { .. } => {
                    ErrorCode::IncompatibleConversion
                }
                EvaluateErrorKind::DivisionByZero => ErrorCode::DivisionByZero,
                EvaluateErrorKind::UndefinedProperty(_) => ErrorCode::UndefinedProperty,
                EvaluateErrorKind::NotInstance(_) => ErrorCode::NotInstance,
            }
        }
    }

    impl ErrorLevel for EvaluateError {
        fn severity(&self) -> codespan_reporting::diagnostic::Severity {
            match self.kind {
                EvaluateErrorKind::IncompatableUnaryOperator { .. } => Severity::Error,
                EvaluateErrorKind::IncompatableBinaryOperator { .. } => Severity::Error,
                EvaluateErrorKind::UndeclaredVariable { .. } => Severity::Error,
                EvaluateErrorKind::NonCallable { .. } => Severity::Error,
                EvaluateErrorKind::IncorrectArity { .. } => Severity::Error,
                EvaluateErrorKind::IncompatableConversion { .. } => Severity::Error,
                EvaluateErrorKind::DivisionByZero => Severity::Warning,
                EvaluateErrorKind::UndefinedProperty(_) => Severity::Error,
                EvaluateErrorKind::NotInstance(_) => Severity::Error,
            }
        }
    }

    impl ToCodespan for EvaluateError {
        fn to_codespan<I: Copy>(&self, file_id: I) -> Diagnostic<I> {
            Diagnostic::new(self.severity())
                .with_message(self.to_string())
                .with_code(self.error_code())
                .with_label(Label::new(LabelStyle::Primary, file_id, self.span.clone()))
        }
    }

    impl Display for EvaluateErrorKind {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
                    write!(f, "Cannot coerce `{from:?}` to `{to:?}`")
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
}

pub mod fn_err {
    use core::{error::Error, fmt::Display};
    use std::ops::Range;

    use codespan_reporting::diagnostic::{Label, LabelStyle, Severity};

    use crate::tree_walk::{
        error::{ErrorLevel, ToCodespan, ToErrorCode, runtime_err::RuntimeError},
        error_codes::ErrorCode,
        lox_var::LoxVar,
    };

    #[derive(Debug, Clone)]
    pub struct FunctionError {
        pub call_span: Option<Box<Range<usize>>>,
        pub input_args: Vec<LoxVar>,
        kind: FunctionErrorKind,
    }

    impl FunctionError {
        pub fn new(kind: FunctionErrorKind) -> Self {
            Self {
                kind,
                call_span: None,
                input_args: Vec::new(),
            }
        }
    }

    impl Error for FunctionError {}

    impl Display for FunctionError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{}", self.kind)
        }
    }

    impl ErrorLevel for FunctionError {
        fn severity(&self) -> codespan_reporting::diagnostic::Severity {
            match &self.kind {
                FunctionErrorKind::IncorrectArity { .. } => Severity::Error,
                FunctionErrorKind::SubError(sub_error) => sub_error.severity(),
            }
        }
    }

    impl ToErrorCode for FunctionError {
        fn error_code(&self) -> ErrorCode {
            match &self.kind {
                FunctionErrorKind::IncorrectArity { .. } => ErrorCode::IncorrectArity,
                FunctionErrorKind::SubError(runtime_error) => runtime_error.error_code(),
            }
        }
    }

    impl ToCodespan for FunctionError {
        fn to_codespan<I: Copy>(
            &self,
            file_id: I,
        ) -> codespan_reporting::diagnostic::Diagnostic<I> {
            match &self.kind {
                FunctionErrorKind::IncorrectArity { .. } => {
                    let err = codespan_reporting::diagnostic::Diagnostic::new(self.severity())
                        .with_message(self.to_string())
                        .with_code(self.error_code());

                    if let Some(span) = &self.call_span {
                        err.with_label(Label::new(LabelStyle::Primary, file_id, *span.clone()))
                    } else {
                        err
                    }
                }
                FunctionErrorKind::SubError(runtime_error) => {
                    let err = runtime_error.to_codespan(file_id);

                    if let Some(span) = &self.call_span {
                        err.with_label(Label::new(LabelStyle::Secondary, file_id, *span.clone()))
                    } else {
                        err
                    }
                }
            }
            .with_note(format!(
                "Called with args: [{}]",
                self.input_args
                    .iter()
                    .map(|arg| arg.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ))
        }
    }

    impl From<RuntimeError> for FunctionError {
        fn from(runtime_error: RuntimeError) -> Self {
            Self {
                call_span: match &runtime_error {
                    RuntimeError::EvaluateError(evaluate_error) => {
                        Some(Box::new(evaluate_error.span.clone()))
                    }
                    RuntimeError::FunctionError(function_error) => function_error.call_span.clone(),
                    RuntimeError::TooManyArguments(_, span) => Some(Box::new(span.clone())),
                    RuntimeError::SuperClassIsNotAClass(_, span) => Some(Box::new(span.clone())),
                    RuntimeError::StackOverflow => None,
                },
                input_args: Vec::new(),
                kind: FunctionErrorKind::SubError(Box::new(runtime_error)),
            }
        }
    }

    #[derive(Debug, Clone)]
    pub enum FunctionErrorKind {
        IncorrectArity { expected: u8, got: usize },
        SubError(Box<RuntimeError>),
    }

    impl Display for FunctionErrorKind {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                FunctionErrorKind::IncorrectArity { expected, got } => write!(
                    f,
                    "Icorrect number of arguments for function call, expected {expected} got {got}"
                ),
                FunctionErrorKind::SubError(runtime_error) => write!(f, "{}", runtime_error),
            }
        }
    }
}
