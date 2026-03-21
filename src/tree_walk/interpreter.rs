use crate::tree_walk::{
    ast::Stmt,
    error::runtime_err::RuntimeError,
    interpreter::env::Env,
};

pub struct Interpreter<'a> {
    pub cfg: InterpreterConfig,
    statements: Vec<Stmt>,
    pub env: &'a mut Env,
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

pub mod env {
    use std::{
        cell::RefCell,
        collections::HashMap,
        ops::{Deref, DerefMut},
        rc::Rc,
    };

    use crate::tree_walk::{error::eval_err::EvaluateErrorKind, lox_var::LoxVar};
    pub trait EnvironmentImpl
    where
        Self: Sized,
    {
        fn new_env(enclosing: Option<Self>) -> Self;
        fn new_with_globals(globals: Vec<(&'static str, LoxVar)>) -> Self;
        fn descend(&mut self);
        fn ascend(&mut self);
    }

    pub type Env = Rc<RefCell<Environment>>;

    impl EnvironmentImpl for Env {
        fn new_env(enclosing: Option<Self>) -> Self {
            Rc::new(RefCell::new(Environment::new(enclosing)))
        }

        fn new_with_globals(globals: Vec<(&'static str, LoxVar)>) -> Self {
            Rc::new(RefCell::new(Environment::new_with_globals(globals)))
        }

        fn descend(&mut self) {
            *self = Env::new_env(Some(self.clone()))
        }

        fn ascend(&mut self) {
            let parent = self
                .borrow()
                .enclosing
                .clone()
                .expect("Tried to ascend above global");

            *self = parent;
        }
    }

    pub struct ScopeGuard<'a> {
        env: &'a mut Env,
    }

    impl<'a> ScopeGuard<'a> {
        pub fn new(env: &'a mut Env) -> Self {
            env.descend();
            Self { env }
        }
    }

    impl DerefMut for ScopeGuard<'_> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.env
        }
    }

    impl Deref for ScopeGuard<'_> {
        type Target = Env;

        fn deref(&self) -> &Self::Target {
            self.env
        }
    }

    impl Drop for ScopeGuard<'_> {
        fn drop(&mut self) {
            self.env.ascend();
        }
    }

    #[derive(Clone, Debug)]
    pub struct Environment {
        enclosing: Option<Env>,
        values: HashMap<String, LoxVar>,
    }

    impl Environment {
        fn new(parent: Option<Env>) -> Environment {
            Self {
                enclosing: parent,
                values: HashMap::new(),
            }
        }

        fn new_with_globals(globals: Vec<(&'static str, LoxVar)>) -> Environment {
            Self::new(Some(Rc::new(RefCell::new(Self {
                enclosing: None,
                values: {
                    let mut hash_map = HashMap::with_capacity(4);
                    for (name, value) in globals {
                        if let Some(original) = hash_map.insert(name.to_owned(), value.clone()) {
                            eprintln!("Conflicting globals, overwriting {original} with {value}")
                        }
                    }
                    hash_map
                },
            }))))
        }

        pub fn define(&mut self, name: String, value: LoxVar) {
            self.values.insert(name, value);
        }

        pub fn get(&self, name: &str, dist: Option<usize>) -> Option<LoxVar> {
            if let Some(dist) = dist {
                if dist == 0 {
                    self.values.get(name).cloned()
                } else {
                    self.enclosing.clone()?.borrow().get(name, Some(dist - 1))
                }
            } else if let Some(v) = self.values.get(name) {
                Some(v.clone())
            } else {
                self.enclosing.as_ref()?.borrow().get(name, None)
            }
        }

        pub fn assign(
            &mut self,
            name: &str,
            value: &LoxVar,
            dist: Option<usize>,
        ) -> Result<(), EvaluateErrorKind> {
            if let Some(dist) = dist {
                if dist == 0 {
                    self.values
                        .insert(name.to_owned(), value.clone())
                        .ok_or(EvaluateErrorKind::UndeclaredVariable {
                            name: name.to_owned(),
                        })
                        .map(|_| ())
                } else {
                    self.enclosing
                        .clone()
                        .ok_or(EvaluateErrorKind::UndeclaredVariable {
                            name: name.to_owned(),
                        })?
                        .borrow_mut()
                        .assign(name, value, Some(dist - 1))
                }
            } else if self.values.insert(name.to_owned(), value.clone()).is_some() {
                Ok(())
            } else if let Some(ref parent) = self.enclosing {
                parent.borrow_mut().assign(name, value, None)
            } else {
                Err(EvaluateErrorKind::UndeclaredVariable {
                    name: name.to_owned(),
                })
            }
        }
    }
}
