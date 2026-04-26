//! The way you create native functions for lox

use std::{
    cell::RefCell,
    process::exit,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use codespan_reporting::diagnostic::Severity;

use crate::{
    error::runtime_err::RuntimeError,
    error_codes::ErrorCode,
    tree_walk::{
        interpreter::Interpreter,
        lox_var::{LoxVar, functions::LoxCallable, list::LoxList},
    },
};

/// # Returns
/// Returns a Vec<(name, loxvar)>.
pub fn default_lib() -> Vec<(&'static str, LoxVar)> {
    vec![
        (
            "clock",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, _| {
                    Ok(LoxVar::Number(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Unix epoch was long ago")
                            .as_secs_f64(),
                    ))
                },
                signature: "clock() -> number",
                arity: 0,
            })),
        ),
        (
            "sleep",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, args| match args[0] {
                    LoxVar::Number(secs) => {
                        std::thread::sleep(match core::time::Duration::try_from_secs_f64(secs) {
                            Ok(d) => d,
                            Err(e) => {
                                return Err(RuntimeError::InternalFunctionError {
                                    msg: e.to_string(),
                                    code: ErrorCode::InternalRustError,
                                    sev: Severity::Error,
                                });
                            }
                        });
                        Ok(LoxVar::Nil)
                    }
                    _ => Err(RuntimeError::InternalFunctionError {
                        msg: "Expected a number argument".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                },
                signature: "sleep(number) -> nil",
                arity: 1,
            })),
        ),
        (
            "type",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, args| {
                    let ty = match args.first().ok_or(RuntimeError::InternalFunctionError {
                        msg: "Expected an argument".to_owned(),
                        code: ErrorCode::IncorrectArity,
                        sev: Severity::Error,
                    })? {
                        LoxVar::Instance(inst) => inst.borrow().class().name.clone(),
                        other => other.kind().to_string(),
                    };
                    Ok(LoxVar::String(ty))
                },
                signature: "type() -> string",
                arity: 1,
            })),
        ),
        (
            "parseNumber",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, args| match args.first() {
                    Some(LoxVar::String(s)) => {
                        let num = s.trim().parse::<f64>();
                        match num {
                            Ok(n) => Ok(LoxVar::Number(n)),
                            Err(_) => Err(RuntimeError::InternalFunctionError {
                                msg: "Invalid number".to_owned(),
                                code: ErrorCode::CouldNotConvert,
                                sev: Severity::Error,
                            }),
                        }
                    }
                    _ => Err(RuntimeError::InternalFunctionError {
                        msg: "Expected a string argument".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                },
                signature: "parseNumber(string) -> number",
                arity: 1,
            })),
        ),
        (
            "string",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, args| match args.first().ok_or(RuntimeError::InternalFunctionError {
                    msg: "Expected an argument".to_owned(),
                    code: ErrorCode::IncorrectArgType,
                    sev: Severity::Error,
                })? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    v => Ok(LoxVar::String(v.to_string())),
                },
                signature: "string(any) -> string",
                arity: 1,
            })),
        ),
        (
            "error",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, args| match args.first().ok_or(RuntimeError::InternalFunctionError {
                    msg: "Expected an argument".to_owned(),
                    code: ErrorCode::IncorrectArgType,
                    sev: Severity::Error,
                })? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    v => Err(RuntimeError::InternalFunctionError {
                        msg: v.to_string(),
                        code: ErrorCode::UserForcedError,
                        sev: Severity::Error,
                    }),
                },
                signature: "error(any)",
                arity: 1,
            })),
        ),
        (
            "printErr",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |interp, args| match args.first().ok_or(
                    RuntimeError::InternalFunctionError {
                        msg: "Expected an argument".to_owned(),
                        code: ErrorCode::IncorrectArity,
                        sev: Severity::Error,
                    },
                )? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    v => {
                        interp.eprintln(v);
                        Ok(LoxVar::Nil)
                    }
                },
                signature: "printErr(any) -> nil",
                arity: 1,
            })),
        ),
        (
            "exit",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, args| match args.first().ok_or(RuntimeError::InternalFunctionError {
                    msg: "Expected an argument".to_owned(),
                    code: ErrorCode::IncorrectArgType,
                    sev: Severity::Error,
                })? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    LoxVar::Number(code) => exit(*code as i32),
                    _ => Err(RuntimeError::InternalFunctionError {
                        msg: "Expected a number".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                },
                signature: "exit(number)",
                arity: 1,
            })),
        ),
        (
            "assert",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, args| match args.first().ok_or(RuntimeError::InternalFunctionError {
                    msg: "Expected an argument".to_owned(),
                    code: ErrorCode::IncorrectArgType,
                    sev: Severity::Error,
                })? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    LoxVar::Bool(b) => {
                        if *b {
                            Ok(LoxVar::Nil)
                        } else {
                            Err(RuntimeError::InternalFunctionError {
                                msg: "Assertion failed".to_owned(),
                                code: ErrorCode::AssertionFailed,
                                sev: Severity::Error,
                            })
                        }
                    }
                    _ => Err(RuntimeError::InternalFunctionError {
                        msg: "Expected a bool".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                },
                signature: "assert(bool) -> nil",
                arity: 1,
            })),
        ),
        (
            "assertEq",
            LoxVar::Callable(Rc::new(NativeFunc {
                func: |_, args| match (
                    args.first().ok_or(RuntimeError::InternalFunctionError {
                        msg: "Expected an argument".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    })?,
                    args.get(1).ok_or(RuntimeError::InternalFunctionError {
                        msg: "Expected an argument".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    })?,
                ) {
                    (LoxVar::Uninitialized, _) => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    (_, LoxVar::Uninitialized) => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_owned(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    (l, r) => {
                        if l == r {
                            Ok(LoxVar::Nil)
                        } else {
                            Err(RuntimeError::InternalFunctionError {
                                msg: format!("Assert Eq failed. left: `{l}` right: `{r}`"),
                                code: ErrorCode::AssertionFailed,
                                sev: Severity::Error,
                            })
                        }
                    }
                },
                signature: "assertEq(any, any) -> nil",
                arity: 2,
            })),
        ),
        (
            "argv",
            LoxVar::List(Rc::new(RefCell::new(LoxList::new(
                std::env::args()
                    .skip(1)
                    .map(LoxVar::String)
                    .collect::<Vec<_>>(),
            )))),
        ),
    ]
}

/// Functions that can be implemented in rust and called in lox
#[derive(Debug, Clone)]
pub struct NativeFunc {
    func: fn(&mut Interpreter, &[LoxVar]) -> Result<LoxVar, RuntimeError>,
    signature: &'static str,
    arity: u8,
}

impl NativeFunc {
    /// Creates a new native function that can then be passed as a global
    pub fn new(
        function: fn(&mut Interpreter, &[LoxVar]) -> Result<LoxVar, RuntimeError>,
        signature: &'static str,
        arity: u8,
    ) -> Self {
        Self {
            func: function,
            signature,
            arity,
        }
    }
}

impl LoxCallable for NativeFunc {
    fn call(&self, interp: &mut Interpreter, args: &[LoxVar]) -> Result<LoxVar, RuntimeError> {
        (self.func)(interp, args)
    }

    fn signature(&self) -> String {
        self.signature.to_owned()
    }

    fn arity(&self) -> u8 {
        self.arity
    }
}
