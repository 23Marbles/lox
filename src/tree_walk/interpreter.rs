use crate::tree_walk::{
    ast::Stmt,
    error::runtime_err::RuntimeError,
    lox_var::{Env, LoxVar},
};

pub struct Interpreter<'a, 'b> {
    statements: Vec<Stmt<'a>>,
    env: &'b mut Env,
}

impl<'a, 'b> Interpreter<'a, 'b> {
    pub fn new(statements: Vec<Stmt<'a>>, env: &'b mut Env) -> Self {
        Self { statements, env }
    }

    pub fn interpret(&mut self) -> Result<Option<LoxVar>, RuntimeError> {
        let mut last = None;

        for s in &self.statements {
            last = s.run(self.env)?;
        }

        Ok(last)
    }
}
