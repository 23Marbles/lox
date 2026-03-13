use crate::tree_walk::{ast::Expr, error::eval_err::EvaluateError, lox_var::LoxVar};

pub struct Interpreter<'a> {
    expr: Expr<'a>,
}

impl<'a> Interpreter<'a> {
    pub fn new(expr: Expr<'a>) -> Self {
        Self { expr }
    }
    pub fn interpret(&self) -> Result<LoxVar, EvaluateError> {
        self.expr.evaluate()
    }
}
