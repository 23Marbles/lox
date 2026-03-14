use crate::tree_walk::{
    ast::Stmt,
    error::runtime_err::RuntimeError,
    lox_var::{Environment, LoxVar},
};

pub struct Interpreter<'a> {
    statements: Vec<Stmt<'a>>,
    env: Environment,
}

impl<'a> Interpreter<'a> {
    pub fn new(statements: Vec<Stmt<'a>>) -> Self {
        Self {
            statements,
            env: Environment::new(),
        }
    }

    pub fn interpret(&mut self) -> Result<Option<LoxVar>, RuntimeError> {
        let mut last = None;

        for s in &self.statements {
            last = s.run(&mut self.env)?;

            #[cfg(debug_assertions)]
            if let Some(v) = &last {
                println!("Statemnt: {s} evaluates to {v}");
            }
        }

        Ok(last)
    }
}
