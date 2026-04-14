//! The way you create native functions for lox

use std::{
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
        lox_var::{LoxVar, functions::LoxCallable},
    },
};

/// The default library implemented in the crafting interpreters book.
///
/// # Returns
/// Returns a Vec<(name, function)>.
pub fn default_lib() -> Vec<(&'static str, Rc<dyn LoxCallable>)> {
    vec![
        (
            "clock",
            Rc::new(NativeFunc {
                func: |_, _| {
                    Ok(LoxVar::Number(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Unix epoch was long ago")
                            .as_secs_f64(),
                    ))
                },
                signature: "clock() -> Number",
                arity: 0,
            }),
        ),
        (
            "sleep",
            Rc::new(NativeFunc {
                func: |_, args| match args[0] {
                    LoxVar::Number(secs) => {
                        std::thread::sleep(std::time::Duration::from_secs_f64(secs));
                        Ok(LoxVar::Nil)
                    }
                    _ => Err(RuntimeError::InternalFunctionError {
                        msg: "sleep() expects a Number argument".to_string(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                },
                signature: "sleep(Number) -> Nil",
                arity: 1,
            }),
        ),
        (
            "type",
            Rc::new(NativeFunc {
                func: |_, args| {
                    let ty = match args.get(0).ok_or(RuntimeError::InternalFunctionError {
                        msg: "Expected an argument".to_string(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    })? {
                        LoxVar::Number(_) => "Number",
                        LoxVar::Bool(_) => "Bool",
                        LoxVar::String(_) => "String",
                        LoxVar::Callable(_) => "Callable",
                        LoxVar::Class(_) => "Class",
                        LoxVar::Instance(_) => "Instance",
                        LoxVar::Nil => "Nil",
                        LoxVar::Uninitialized => {
                            return Err(RuntimeError::InternalFunctionError {
                                msg: "Cannot determine type of uninitialized variable".to_string(),
                                code: ErrorCode::IncorrectArgType,
                                sev: Severity::Error,
                            });
                        }
                    };
                    Ok(LoxVar::String(ty.to_string()))
                },
                signature: "type() -> String",
                arity: 1,
            }),
        ),
        (
            "parseNumber",
            Rc::new(NativeFunc {
                func: |_, args| match args.get(0) {
                    Some(LoxVar::String(s)) => {
                        let num = s.parse::<f64>();
                        match num {
                            Ok(n) => Ok(LoxVar::Number(n)),
                            Err(_) => Err(RuntimeError::InternalFunctionError {
                                msg: "Invalid number".to_string(),
                                code: ErrorCode::CouldNotConvert,
                                sev: Severity::Error,
                            }),
                        }
                    }
                    _ => Err(RuntimeError::InternalFunctionError {
                        msg: "Expected a string argument".to_string(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                },
                signature: "parseNumber(String) -> Number",
                arity: 1,
            }),
        ),
        (
            "string",
            Rc::new(NativeFunc {
                func: |_, args| match args.get(0).ok_or(RuntimeError::InternalFunctionError {
                    msg: "Expected an argument".to_string(),
                    code: ErrorCode::IncorrectArgType,
                    sev: Severity::Error,
                })? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_string(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    v => Ok(LoxVar::String(v.to_string())),
                },
                signature: "string(any) -> string",
                arity: 1,
            }),
        ),
        (
            "error",
            Rc::new(NativeFunc {
                func: |_, args| match args.get(0).ok_or(RuntimeError::InternalFunctionError {
                    msg: "Expected an argument".to_string(),
                    code: ErrorCode::IncorrectArgType,
                    sev: Severity::Error,
                })? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_string(),
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
            }),
        ),
        (
            "printErr",
            Rc::new(NativeFunc {
                func: |interp, args| match args.get(0).ok_or(
                    RuntimeError::InternalFunctionError {
                        msg: "Expected an argument".to_string(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    },
                )? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_string(),
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
            }),
        ),
        (
            "exit",
            Rc::new(NativeFunc {
                func: |_, args| match args.get(0).ok_or(RuntimeError::InternalFunctionError {
                    msg: "Expected an argument".to_string(),
                    code: ErrorCode::IncorrectArgType,
                    sev: Severity::Error,
                })? {
                    LoxVar::Uninitialized => Err(RuntimeError::InternalFunctionError {
                        msg: "Cannot read from uninitialized variable".to_string(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                    LoxVar::Number(code) => exit(*code as i32),
                    _ => Err(RuntimeError::InternalFunctionError {
                        msg: "Expected a number".to_string(),
                        code: ErrorCode::IncorrectArgType,
                        sev: Severity::Error,
                    }),
                },
                signature: "exit(number)",
                arity: 1,
            }),
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
    /// The function must handle *ALL* errors including arity checks
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
