use core::error::Error;
use core::fmt::Display;

use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};

use crate::{error_codes::ErrorCode, lox_files::FileIdent};

pub trait ToErrorCode {
    fn error_code(&self) -> ErrorCode;
}

pub trait ErrorLevel {
    fn severity(&self) -> Severity;
}

pub trait ToCodespan: ToErrorCode + ErrorLevel + Display {
    fn to_codespan(&self) -> Diagnostic<FileIdent> {
        Diagnostic::new(self.severity())
            .with_message(self.to_string())
            .with_code(self.error_code())
    }
}

pub fn diag_fom_err<T>(from: impl Error) -> Diagnostic<T> {
    Diagnostic::new(Severity::Bug)
        .with_message(from.to_string())
        // Core Error 0001
        .with_code("CE0001")
}

fn to_codespan<T: Display + ErrorLevel + ToErrorCode, I: Copy, K: IntoIterator<Item = Label<I>>>(
    from: &T,
    labels: K,
) -> Diagnostic<I> {
    Diagnostic::new(from.severity())
        .with_message(from.to_string())
        .with_code(from.error_code())
        .with_labels(labels.into_iter().collect())
}

pub mod scan_err {
    use core::fmt::Display;

    use crate::{
        error::{ErrorLevel, ToErrorCode},
        error_codes::ErrorCode,
    };

    #[derive(Debug, Clone)]
    pub enum ScanErrorKind {
        UnexpectedCharacter(char),
        UnterminatedString,
        UnterminatedComment,
    }

    use codespan_reporting::diagnostic::Severity;

    impl ErrorLevel for ScanErrorKind {
        fn severity(&self) -> Severity {
            match self {
                ScanErrorKind::UnexpectedCharacter(_) => Severity::Error,
                ScanErrorKind::UnterminatedString => Severity::Error,
                ScanErrorKind::UnterminatedComment => Severity::Error,
            }
        }
    }

    impl ToErrorCode for ScanErrorKind {
        fn error_code(&self) -> ErrorCode {
            match self {
                ScanErrorKind::UnexpectedCharacter(_) => ErrorCode::UnexpectedCharacter,
                ScanErrorKind::UnterminatedString => ErrorCode::UnterminatedString,
                ScanErrorKind::UnterminatedComment => ErrorCode::UnterminatedComment,
            }
        }
    }

    impl Display for ScanErrorKind {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                ScanErrorKind::UnexpectedCharacter(c) => write!(f, "Unexpected character '{c}'"),
                ScanErrorKind::UnterminatedString => write!(f, "Unterminated string"),
                ScanErrorKind::UnterminatedComment => write!(f, "Unterminated comment"),
            }
        }
    }
}

pub mod parse_err {
    use core::fmt::Display;

    use codespan_reporting::diagnostic::{Diagnostic, Label, LabelStyle, Severity};

    use crate::{
        Span,
        error::{ErrorLevel, ToCodespan, ToErrorCode},
        error_codes::ErrorCode,
        lox_files::FileIdent,
        tree_walk::token::TokenKind,
    };

    #[derive(Debug, Clone)]
    pub enum ParseError {
        UnexpectedTokenKind {
            got: TokenKind,
            expected: Vec<TokenKind>,
            span: Span,
        },
        ExpectedTokenKind {
            got: TokenKind,
            expected: TokenKind,
            span: Span,
        },
        InvalidAssignTarget {
            target: String,
            span: Span,
        },
        TooManyArguments {
            spans: Vec<Span>,
        },
        ExpectedName {
            name: Named,
            after: TokenKind,
            prev_span: Span,
            span: Span,
        },
        ExpectedNumber {
            input: String,
            span: Span,
        },
        InvalidEscape {
            seq: String,
            span: Span,
        },
    }

    impl ToErrorCode for ParseError {
        fn error_code(&self) -> ErrorCode {
            match self {
                ParseError::UnexpectedTokenKind { .. } => ErrorCode::UnexpectedTokenKind,
                ParseError::ExpectedTokenKind { .. } => ErrorCode::UnexpectedTokenKind,
                ParseError::InvalidAssignTarget { .. } => ErrorCode::InvalidAssignTarget,
                ParseError::TooManyArguments { .. } => ErrorCode::Over255Arguments,
                ParseError::ExpectedName { .. } => ErrorCode::ExpectedName,
                ParseError::ExpectedNumber { .. } => ErrorCode::ExpectedNumber,
                ParseError::InvalidEscape { .. } => ErrorCode::InvalidEscape,
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
                ParseError::InvalidEscape { .. } => Severity::Error,
            }
        }
    }

    impl ToCodespan for ParseError {
        fn to_codespan(&self) -> Diagnostic<FileIdent> {
            match self {
                ParseError::UnexpectedTokenKind { span, expected, .. } => {
                    let file_id = span.file_id;
                    let range = span.range.clone();
                    Diagnostic::new(self.severity())
                        .with_message(self.to_string())
                        .with_code(self.error_code())
                        .with_label(Label::new(LabelStyle::Primary, file_id, range))
                        .with_note(format!(
                            "Expected one of [{}]",
                            expected
                                .iter()
                                .map(|token_kind| token_kind.to_string())
                                .collect::<Vec<String>>()
                                .join(", ")
                        ))
                }
                ParseError::TooManyArguments { spans, .. } => Diagnostic::new(self.severity())
                    .with_message(self.to_string())
                    .with_code(self.error_code())
                    .with_labels_iter(spans.iter().map(|span| {
                        let file_id = span.file_id;
                        let range = span.range.clone();
                        Label::new(LabelStyle::Secondary, file_id, range)
                    })),
                ParseError::ExpectedName {
                    span,
                    prev_span,
                    after,
                    ..
                } => Diagnostic::new(self.severity())
                    .with_message(self.to_string())
                    .with_code(self.error_code())
                    .with_label(Label::new(
                        LabelStyle::Primary,
                        span.file_id,
                        span.range.clone(),
                    ))
                    .with_label(
                        Label::new(
                            LabelStyle::Secondary,
                            prev_span.file_id,
                            prev_span.range.clone(),
                        )
                        .with_message(format!("Expected after `{after}`")),
                    ),
                ParseError::ExpectedTokenKind { span, .. }
                | ParseError::InvalidEscape { span, .. }
                | ParseError::InvalidAssignTarget { span, .. }
                | ParseError::ExpectedNumber { span, .. } => Diagnostic::new(self.severity())
                    .with_message(self.to_string())
                    .with_code(self.error_code())
                    .with_label(Label::new(
                        LabelStyle::Primary,
                        span.file_id,
                        span.range.clone(),
                    )),
            }
        }
    }

    impl Display for ParseError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                ParseError::UnexpectedTokenKind { got, .. } => {
                    write!(f, "Unexpected token kind: `{got}`")
                }
                ParseError::ExpectedTokenKind { got, expected, .. } => {
                    write!(f, "Expected token kind: `{expected}`, got: `{got}`")
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
                ParseError::InvalidEscape { seq, .. } => {
                    write!(f, "Invalid escape sequence `{seq}`")
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

    use codespan_reporting::diagnostic::{Label, LabelStyle, Severity};

    use crate::{
        Span,
        error::{ErrorLevel, ToCodespan, ToErrorCode, to_codespan},
        error_codes::ErrorCode,
        lox_files::FileIdent,
    };

    #[derive(Debug, Clone)]
    pub enum ResolverErr {
        ReadInOwnInitializer(Span),
        DefinedMultipleTimes(String, Vec<Span>),
        ReturnFromTop(Span),
        /// [Return span, Init span]
        ReturnValueFromInit([Span; 2]),
        ThisOutsideClass(Span),
        SelfReferentialInheritance(Span),
        SuperOutsideSubclass(Span),
        BreakFromOutsideLoop(Span),
        ContinueFromOutsideLoop(Span),
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
                ResolverErr::BreakFromOutsideLoop(_) => ErrorCode::BreakFromOutsideLoop,
                ResolverErr::ContinueFromOutsideLoop(_) => ErrorCode::ContinueFromOutsideLoop,
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
                ResolverErr::BreakFromOutsideLoop(_) => Severity::Error,
                ResolverErr::ContinueFromOutsideLoop(_) => Severity::Error,
            }
        }
    }

    impl ToCodespan for ResolverErr {
        fn to_codespan(&self) -> codespan_reporting::diagnostic::Diagnostic<FileIdent> {
            match self {
                ResolverErr::DefinedMultipleTimes(_, spans) => {
                    let labels: Vec<_> = core::iter::once(
                        Label::new(
                            LabelStyle::Primary,
                            #[expect(clippy::unwrap_used)]
                            spans.first().unwrap().file_id,
                            #[expect(clippy::unwrap_used)]
                            spans.first().unwrap().range.clone(),
                        )
                        .with_message("First defined here"),
                    )
                    .chain(core::iter::once(
                        Label::new(
                            LabelStyle::Secondary,
                            spans[1].file_id,
                            spans[1].range.clone(),
                        )
                        .with_message("Redefined here"),
                    ))
                    .chain(spans.iter().skip(2).map(|span| {
                        Label::new(LabelStyle::Secondary, span.file_id, span.range.clone())
                            .with_message("And here")
                    }))
                    .collect();

                    to_codespan(self, labels)
                }
                ResolverErr::ReturnValueFromInit([return_span, init_span]) => to_codespan(
                    self,
                    [
                        Label::new(
                            LabelStyle::Primary,
                            return_span.file_id,
                            return_span.range.clone(),
                        )
                        .with_message("Tried to return here"),
                        Label::new(
                            LabelStyle::Secondary,
                            init_span.file_id,
                            init_span.range.clone(),
                        )
                        .with_message("Initializer defined here"),
                    ],
                ),
                ResolverErr::ReadInOwnInitializer(span)
                | ResolverErr::ReturnFromTop(span)
                | ResolverErr::BreakFromOutsideLoop(span)
                | ResolverErr::ContinueFromOutsideLoop(span)
                | ResolverErr::ThisOutsideClass(span)
                | ResolverErr::SelfReferentialInheritance(span)
                | ResolverErr::SuperOutsideSubclass(span) => to_codespan(
                    self,
                    [Label::new(
                        LabelStyle::Primary,
                        span.file_id,
                        span.range.clone(),
                    )],
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
                ResolverErr::BreakFromOutsideLoop(_) => {
                    write!(f, "Cannot use `break` outside of a loop")
                }
                ResolverErr::ContinueFromOutsideLoop(_) => {
                    write!(f, "Cannot use `continue` outside of a loop")
                }
            }
        }
    }
}

pub mod runtime_err {
    use core::fmt::Display;

    use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};

    use crate::{
        Span,
        error::{
            ErrorLevel, ToCodespan, ToErrorCode, access_error::PropertyAccessError,
            eval_err::EvaluateError, fn_err::FunctionError, to_codespan,
        },
        error_codes::ErrorCode,
        lox_files::FileIdent,
        tree_walk::lox_var::LoxVar,
    };

    #[derive(Debug, Clone)]
    pub enum RuntimeError {
        EvaluateError(EvaluateError),
        FunctionError(FunctionError),
        Over255Arguments(usize, Span),
        SuperClassIsNotAClass(LoxVar, Span),
        AccessError(PropertyAccessError, Span),
        StackOverflow,
        InternalFunctionError {
            msg: String,
            code: ErrorCode,
            sev: Severity,
        },
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
                RuntimeError::FunctionError(function_error) => function_error.reason().error_code(),
                RuntimeError::Over255Arguments(..) => ErrorCode::Over255Arguments,
                RuntimeError::SuperClassIsNotAClass(..) => ErrorCode::SuperClassIsNotAClass,
                RuntimeError::StackOverflow => ErrorCode::StackOverflow,
                RuntimeError::InternalFunctionError { code, .. } => *code,
                RuntimeError::AccessError(access_error, ..) => access_error.error_code(),
            }
        }
    }

    impl ErrorLevel for RuntimeError {
        fn severity(&self) -> codespan_reporting::diagnostic::Severity {
            match self {
                RuntimeError::EvaluateError(evaluate_error) => evaluate_error.severity(),
                RuntimeError::FunctionError(function_error) => function_error.reason().severity(),
                RuntimeError::Over255Arguments(..) => Severity::Warning,
                RuntimeError::SuperClassIsNotAClass(..) => Severity::Error,
                RuntimeError::StackOverflow => Severity::Error,
                RuntimeError::InternalFunctionError { sev, .. } => *sev,
                RuntimeError::AccessError(access_error, ..) => access_error.severity(),
            }
        }
    }

    impl ToCodespan for RuntimeError {
        fn to_codespan(&self) -> Diagnostic<FileIdent> {
            match self {
                RuntimeError::EvaluateError(evaluate_error) => evaluate_error.to_codespan(),
                RuntimeError::Over255Arguments(_, span)
                | RuntimeError::SuperClassIsNotAClass(_, span)
                | RuntimeError::AccessError(_, span) => to_codespan(
                    self,
                    [Label::new(
                        codespan_reporting::diagnostic::LabelStyle::Secondary,
                        span.file_id,
                        span.range.clone(),
                    )],
                ),
                RuntimeError::FunctionError(function_error) => to_codespan(
                    &function_error.reason(),
                    function_error
                        .function_base_labels()
                        .iter()
                        .cloned()
                        .chain((|| {
                            let original_error = function_error.reason();
                            if let RuntimeError::InternalFunctionError { .. } = original_error {
                                return [].to_vec();
                            }
                            let span = match &original_error {
                                RuntimeError::EvaluateError(evaluate_error) => {
                                    evaluate_error.span.clone()
                                }
                                RuntimeError::Over255Arguments(_, span)
                                | RuntimeError::SuperClassIsNotAClass(_, span)
                                | RuntimeError::AccessError(_, span) => span.clone(),
                                RuntimeError::InternalFunctionError { .. } => {
                                    *function_error.call_span.clone()
                                }
                                RuntimeError::StackOverflow | RuntimeError::FunctionError(..) => {
                                    unreachable!(
                                        "Stack overflow ignores stack, functions are never the base"
                                    )
                                }
                            };
                            [Label::new(
                                codespan_reporting::diagnostic::LabelStyle::Primary,
                                span.file_id,
                                span.range,
                            )
                            .with_message("Actual error occurred here")]
                            .to_vec()
                        })()),
                )
                .with_notes({
                    let mut stack_trace = vec!["Stack trace (most recent first):".to_owned()];
                    stack_trace.append(&mut function_error.stack_trace());
                    stack_trace
                }),
                RuntimeError::StackOverflow => to_codespan(self, []),
                RuntimeError::InternalFunctionError { .. } => to_codespan(self, []),
            }
        }
    }

    impl Display for RuntimeError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                RuntimeError::EvaluateError(err) => write!(f, "{err}"),
                RuntimeError::FunctionError(err) => write!(f, "{err}"),
                RuntimeError::AccessError(err, ..) => write!(f, "{err}"),
                RuntimeError::Over255Arguments(got, ..) => {
                    write!(
                        f,
                        "Too many arguments for function, expected at most 255, got {got}"
                    )
                }
                RuntimeError::SuperClassIsNotAClass(not_class, ..) => {
                    write!(f, "The super class `{not_class}` must be a class")
                }
                RuntimeError::StackOverflow => write!(f, "Max call depth reached"),
                RuntimeError::InternalFunctionError { msg, .. } => {
                    write!(f, "Internal function error: {msg}")
                }
            }
        }
    }
}

pub mod eval_err {
    use core::{error::Error, fmt::Display};

    use codespan_reporting::diagnostic::{Diagnostic, Label, LabelStyle, Severity};

    use crate::Span;
    use crate::lox_files::FileIdent;
    use crate::tree_walk::lox_var::BinErrReason;
    use crate::{
        error::{ErrorLevel, ToCodespan, ToErrorCode},
        error_codes::ErrorCode,
        tree_walk::{
            lox_var::{LoxVar, LoxVarKind},
            token::TokenKind,
        },
    };

    #[derive(Debug, Clone)]
    pub struct EvaluateError {
        kind: EvaluateErrorKind,
        pub(super) span: Span,
    }

    impl EvaluateError {
        pub fn new(kind: EvaluateErrorKind, span: Span) -> Self {
            Self { kind, span }
        }

        pub fn bin_eval_error(
            span: Span,
            kind: TokenKind,
            left_var: &LoxVar,
            right_var: &LoxVar,
            reason: BinErrReason,
        ) -> Self {
            Self {
                kind: match reason {
                    BinErrReason::Incompatible => EvaluateErrorKind::IncompatableBinaryOperator {
                        operator: kind,
                        left_ty: left_var.into(),
                        right_ty: right_var.into(),
                    },
                    BinErrReason::NotPositiveInteger => {
                        EvaluateErrorKind::NotPositiveIntegerOperand {
                            operator: kind,
                            left_ty: left_var.into(),
                            right_ty: right_var.into(),
                        }
                    }
                },
                span,
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
        NotPositiveIntegerOperand {
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
        CannotGetProperties(LoxVarKind),
        CannotSetProperties(LoxVarKind),
        CannotIndexInto(LoxVarKind),
        UninitializedVariable {
            name: String,
        },
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
                EvaluateErrorKind::CannotGetProperties(_) => ErrorCode::CannotGetProperties,
                EvaluateErrorKind::UninitializedVariable { .. } => ErrorCode::UninitializedVariable,
                EvaluateErrorKind::CannotIndexInto(_) => ErrorCode::CannotIndexInto,
                EvaluateErrorKind::NotPositiveIntegerOperand { .. } => {
                    ErrorCode::NonPositiveIntegerOperand
                }
                EvaluateErrorKind::CannotSetProperties(_) => ErrorCode::CannotSetProperties,
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
                EvaluateErrorKind::CannotGetProperties(_) => Severity::Error,
                EvaluateErrorKind::UninitializedVariable { .. } => Severity::Warning,
                EvaluateErrorKind::CannotIndexInto(_) => Severity::Error,
                EvaluateErrorKind::NotPositiveIntegerOperand { .. } => Severity::Error,
                EvaluateErrorKind::CannotSetProperties(_) => Severity::Error,
            }
        }
    }

    impl ToCodespan for EvaluateError {
        fn to_codespan(&self) -> Diagnostic<FileIdent> {
            Diagnostic::new(self.severity())
                .with_message(self.to_string())
                .with_code(self.error_code())
                .with_label(Label::new(
                    LabelStyle::Primary,
                    self.span.file_id,
                    self.span.range.clone(),
                ))
        }
    }

    impl Display for EvaluateErrorKind {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match &self {
                EvaluateErrorKind::IncompatableUnaryOperator { operator, var_ty } => write!(
                    f,
                    "Cannot use unary operator `{operator}` on type `{var_ty}`",
                ),
                EvaluateErrorKind::IncompatableBinaryOperator {
                    operator,
                    left_ty,
                    right_ty,
                } => write!(
                    f,
                    "Cannot use binary operator `{operator}` on types `{left_ty}`, `{right_ty}`",
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
                    write!(f, "Cannot coerce `{from}` to `{to}`")
                }
                EvaluateErrorKind::DivisionByZero => write!(f, "Cannot to divide by zero"),
                EvaluateErrorKind::CannotGetProperties(kind) => {
                    write!(f, "Cannot get properties on variable of kind {kind}")
                }
                EvaluateErrorKind::UninitializedVariable { name } => {
                    write!(
                        f,
                        "Tried to access an uninitialized variable with name {name}",
                    )
                }
                EvaluateErrorKind::CannotIndexInto(kind) => {
                    write!(f, "Cannot index into variable of kind {kind}")
                }
                EvaluateErrorKind::NotPositiveIntegerOperand {
                    operator,
                    left_ty,
                    right_ty,
                } => write!(
                    f,
                    "Binary operation of type `{operator}` on types `{left_ty}`, `{right_ty}` requires a positive, non negative integer"
                ),
                EvaluateErrorKind::CannotSetProperties(kind) => {
                    write!(f, "Cannot set properties on variable of kind {kind}")
                }
            }
        }
    }
}

pub mod fn_err {
    use core::{error::Error, fmt::Display};

    use codespan_reporting::diagnostic::Label;

    use crate::Span;
    use crate::lox_files::FileIdent;
    use crate::{error::runtime_err::RuntimeError, tree_walk::lox_var::LoxVar};

    #[derive(Debug, Clone)]
    pub struct FunctionError {
        pub(super) call_span: Box<Span>,
        name: String,
        pub input_args: Vec<LoxVar>,
        kind: Box<RuntimeError>,
    }

    impl FunctionError {
        pub fn from_runtime_error(
            err: RuntimeError,
            signature: String,
            input_args: Vec<LoxVar>,
            call_span: Span,
        ) -> Self {
            Self {
                call_span: Box::new(call_span),
                name: signature,
                input_args,
                kind: Box::new(err),
            }
        }

        pub fn with_call_span(mut self, call_span: Span) -> Self {
            self.call_span = Box::new(call_span);
            self
        }

        pub fn reason(&self) -> RuntimeError {
            match *self.kind.clone() {
                RuntimeError::FunctionError(function_error) => function_error.reason(),
                o => o,
            }
        }

        pub fn function_base_labels(&self) -> [Label<FileIdent>; 2] {
            match *self.kind.clone() {
                RuntimeError::FunctionError(function_error) => {
                    function_error.function_base_labels()
                }
                _ => [
                    Label::new(
                        codespan_reporting::diagnostic::LabelStyle::Primary,
                        self.call_span.file_id,
                        self.call_span.range.clone(),
                    )
                    .with_message("Error happened in this function"),
                    Label::new(
                        codespan_reporting::diagnostic::LabelStyle::Primary,
                        self.call_span.file_id,
                        self.call_span.range.clone(),
                    )
                    .with_message(format!(
                        "Function called with arguments: {:?}",
                        self.input_args
                    )),
                ],
            }
        }

        pub fn stack_trace(&self) -> Vec<String> {
            match *self.kind.clone() {
                RuntimeError::FunctionError(function_error) => {
                    let mut stack_trace = function_error.stack_trace();
                    stack_trace.append(&mut vec![self.name.to_string()]);
                    stack_trace
                }
                _ => vec![self.name.to_string()],
            }
        }
    }

    impl Error for FunctionError {}

    impl Display for FunctionError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(f, "{}", self.kind)
        }
    }
}

pub mod access_error {
    use crate::tree_walk::lox_var::LoxVarKind;

    use super::*;

    #[derive(Debug, Clone)]
    pub enum PropertyAccessError {
        OutOfBounds {
            tried: f64,
            len: usize,
        },
        NonIntegerIndex {
            tried: f64,
        },
        InccorrectIndexKind {
            tried_kind: LoxVarKind,
        },
        NonExistentField {
            name: String,
        },
        IncorrectType {
            expected: LoxVarKind,
            got: LoxVarKind,
        },
        CannotSetProperties {
            kind: LoxVarKind,
        },
    }

    impl ErrorLevel for PropertyAccessError {
        fn severity(&self) -> Severity {
            match self {
                PropertyAccessError::OutOfBounds { .. } => Severity::Error,
                PropertyAccessError::NonIntegerIndex { .. } => Severity::Error,
                PropertyAccessError::NonExistentField { .. } => Severity::Error,
                PropertyAccessError::IncorrectType { .. } => Severity::Error,
                PropertyAccessError::InccorrectIndexKind { .. } => Severity::Error,
                PropertyAccessError::CannotSetProperties { .. } => Severity::Error,
            }
        }
    }

    impl ToErrorCode for PropertyAccessError {
        fn error_code(&self) -> ErrorCode {
            match self {
                PropertyAccessError::OutOfBounds { .. } => ErrorCode::OutOfBounds,
                PropertyAccessError::NonIntegerIndex { .. } => ErrorCode::NonIntegerIndex,
                PropertyAccessError::NonExistentField { .. } => ErrorCode::NonExistentField,
                PropertyAccessError::IncorrectType { .. } => ErrorCode::IncorrectType,
                PropertyAccessError::InccorrectIndexKind { .. } => ErrorCode::IncorrectIndexKind,
                PropertyAccessError::CannotSetProperties { .. } => ErrorCode::CannotSetProperties,
            }
        }
    }

    impl Display for PropertyAccessError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                PropertyAccessError::OutOfBounds { tried, len } => {
                    write!(
                        f,
                        "{}",
                        match (tried.is_sign_positive(), len) {
                            (false, _) => format!("Index `{}` must not be negative", tried),
                            (true, 0) => "Cannot index into an empty list".to_owned(),
                            (true, l) =>
                                format!("Index `{}` out of bounds. Max index: `{}`", tried, l - 1),
                        }
                    )
                }
                PropertyAccessError::NonIntegerIndex { tried } => {
                    write!(f, "Index `{}` must be a non-negative integer", tried)
                }
                PropertyAccessError::NonExistentField { name } => {
                    write!(f, "Field `{}` does not exist", name)
                }
                PropertyAccessError::IncorrectType { expected, got } => {
                    write!(
                        f,
                        "Incorrect type for property: expected `{expected}`, got `{got}`",
                    )
                }
                PropertyAccessError::InccorrectIndexKind { tried_kind } => write!(
                    f,
                    "Cannot index into this object with a value of kind `{tried_kind}`"
                ),
                PropertyAccessError::CannotSetProperties { kind } => {
                    write!(f, "Cannot set properties on a vale of kind `{kind}`")
                }
            }
        }
    }
}
