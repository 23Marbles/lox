//! The way you create native functions for lox

use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    error::fn_err::{FunctionError, FunctionErrorKind},
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
    vec![(
        "clock",
        Rc::new(NativeFunc {
            func: |args| {
                if !args.is_empty() {
                    Err(FunctionError::new(FunctionErrorKind::IncorrectArity {
                        expected: 0,
                        got: args.len(),
                    }))
                } else {
                    Ok(LoxVar::Number(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .expect("Unix epoch was long ago")
                            .as_secs_f64(),
                    ))
                }
            },
            signature: "clock() -> Number",
        }),
    )]
}

/// Functions that can be implemented in rust and called in lox
#[derive(Debug, Clone)]
pub struct NativeFunc {
    pub(crate) func: fn(&[LoxVar]) -> Result<LoxVar, FunctionError>,
    pub(crate) signature: &'static str,
}

impl NativeFunc {
    /// Creates a new native function that can then be passed as a global
    /// The function must handle *ALL* errors including arity checks
    pub fn new(
        function: fn(&[LoxVar]) -> Result<LoxVar, FunctionError>,
        signature: &'static str,
    ) -> Self {
        Self {
            func: function,
            signature,
        }
    }
}

impl LoxCallable for NativeFunc {
    fn call(&self, _: &mut Interpreter, args: &[LoxVar]) -> Result<LoxVar, FunctionError> {
        (self.func)(args)
    }

    fn signature(&self) -> String {
        self.signature.to_owned()
    }
}
