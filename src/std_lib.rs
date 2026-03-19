use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::tree_walk::{
    error::fn_err::{FunctionError, FunctionErrorKind},
    lox_var::{Env, LoxCallable, LoxVar},
};

pub fn lib() -> Vec<(&'static str, Rc<dyn LoxCallable>)> {
    vec![(
        "clock",
        Rc::new(NativeFunc {
            func: |_, args| {
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
    pub func: fn(&mut Env, Vec<LoxVar>) -> Result<LoxVar, FunctionError>,
    pub signature: &'static str,
}

impl LoxCallable for NativeFunc {
    fn call(&self, env: &mut Env, args: Vec<LoxVar>) -> Result<LoxVar, FunctionError> {
        (self.func)(env, args)
    }

    fn signature(&self) -> String {
        self.signature.to_string()
    }
}
