use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::tree_walk::{
    error::fn_err::{FunctionError, FunctionErrorKind},
    lox_var::{LoxVar, functions::LoxCallable},
};

pub fn lib() -> Vec<(&'static str, Rc<dyn LoxCallable>)> {
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

#[derive(Debug, Clone)]
pub struct NativeFunc {
    pub func: fn(&[LoxVar]) -> Result<LoxVar, FunctionError>,
    pub signature: &'static str,
}

impl LoxCallable for NativeFunc {
    fn call(&self, args: &[LoxVar]) -> Result<LoxVar, FunctionError> {
        (self.func)(args)
    }

    fn signature(&self) -> String {
        self.signature.to_owned()
    }
}
