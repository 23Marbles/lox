use crate::tree_walk::{ast::Stmt, error::runtime_err::RuntimeError, lox_var::Env};

pub struct Interpreter<'a> {
    pub cfg: InterpreterConfig,
    statements: Vec<Stmt>,
    env: &'a mut Env,
}

#[derive(Debug, Clone, Copy)]
pub struct InterpreterConfig {
    print_expr: bool,
}

impl InterpreterConfig {
    pub fn new(print_expr: bool) -> Self {
        Self { print_expr }
    }
}

impl<'a> Interpreter<'a> {
    pub fn new(statements: Vec<Stmt>, env: &'a mut Env, cfg: InterpreterConfig) -> Self {
        Self {
            statements,
            env,
            cfg,
        }
    }

    pub fn update_stmts(&mut self, new_statements: Vec<Stmt>) {
        self.statements = new_statements
    }

    pub fn interpret(&mut self) -> Result<(), RuntimeError> {
        match self.cfg.print_expr {
            true => {
                for s in &self.statements {
                    match s {
                        Stmt::Expression(expr) => println!("{}", expr.evaluate(self.env)?),
                        other_stmt => {
                            other_stmt.run(self.env)?;
                        }
                    }
                }
            }
            false => {
                for s in &self.statements {
                    s.run(self.env)?;
                }
            }
        }

        Ok(())
    }
}
