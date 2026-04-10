use std::{collections::HashMap, rc::Rc};

use crate::tree_walk::{
    ast::{
        Literal,
        expr::Expr,
        stmt::{ControlFlow, Stmt},
    },
    error::{
        ToCodespan,
        eval_err::{EvaluateError, EvaluateErrorKind},
        runtime_err::RuntimeError,
    },
    interpreter::env::{Env, EnvironmentImpl as _},
    lox_var::{
        LoxVar,
        class::{InstanceImpl as _, LoxClass},
        functions::{LoxCallable, LoxFunc},
    },
};

const MAX_CALL_DEPTH: usize = 1024;

#[derive(Debug, Clone)]
pub struct Interpreter {
    statements: Vec<Stmt>,
    ip: usize,
    out_buf: InterpOut,
    pub env: Env,
    call_depth: usize,
    file_id: usize,
}

#[derive(Debug, Clone, Default)]
pub struct InterpOut {
    lines: Vec<String>,
}

impl InterpOut {
    pub fn lines(&self) -> Vec<String> {
        self.lines.clone()
    }

    fn println(&mut self, value: impl ToString) {
        for s in value.to_string().lines() {
            self.lines.push(s.to_owned());
        }
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

impl Iterator for Interpreter {
    type Item = Result<InterpOut, RuntimeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.out_buf.clear();
        let ip = self.ip;
        self.ip += 1;
        Some(
            self.run_stmt(self.statements.get(ip)?.clone())
                .map(|_| self.out_buf.clone()),
        )
    }
}

impl Interpreter {
    pub fn new(file_id: usize) -> Self {
        Self {
            statements: Vec::new(),
            env: Env::new_env(None),
            ip: 0,
            out_buf: InterpOut::default(),
            call_depth: 0,
            file_id,
        }
    }

    pub fn with_stmts(mut self, statements: &[Stmt]) -> Self {
        self.statements = statements.to_vec();
        self
    }

    pub fn set_stmts(&mut self, statements: &[Stmt]) {
        self.statements = statements.to_vec();
    }

    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    pub fn set_env(&mut self, env: Env) {
        self.env = env;
    }

    pub fn with_globals(mut self, globals: Vec<(&'static str, Rc<dyn LoxCallable>)>) -> Self {
        self.env = Env::new_env(Some(Env::new_with_globals(
            globals
                .iter()
                .cloned()
                .map(|(name, func)| (name, LoxVar::Callable(func)))
                .collect(),
        )));
        self
    }

    pub fn run(&mut self) -> crate::Result<Vec<InterpOut>> {
        let id = self.file_id;
        dbg!(&self.statements);
        self.into_iter()
            .map(|r| r.map_err(|e| vec![e.to_codespan(id)]))
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn run_stmt(&mut self, stmt: Stmt) -> Result<ControlFlow, RuntimeError> {
        match stmt {
            Stmt::Expression(expr) => {
                self.evaluate_expr(expr)?;
                Ok(ControlFlow::None)
            }
            Stmt::Print(expr) => {
                let value = self.evaluate_expr(expr)?;
                dbg!(&value);
                self.out_buf.println(value);
                Ok(ControlFlow::None)
            }
            Stmt::Var {
                name, initializer, ..
            } => {
                let value = match initializer {
                    Some(value) => self.evaluate_expr(value)?,
                    None => LoxVar::Nil,
                };
                self.env.borrow_mut().define(name, value);
                Ok(ControlFlow::None)
            }
            Stmt::Block(stmts) => {
                let previous = self.env.clone();
                self.env = Env::new_env(Some(previous.clone()));

                for stmt in stmts {
                    match {
                        match self.run_stmt(stmt) {
                            Ok(c) => c,
                            Err(e) => {
                                self.env = previous;
                                return Err(e);
                            }
                        }
                    } {
                        ControlFlow::None => {}
                        ControlFlow::Return(v) => {
                            self.env = previous;
                            return Ok(ControlFlow::Return(v));
                        }
                    }
                }

                self.env = previous;
                Ok(ControlFlow::None)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.evaluate_expr(condition)?.is_truthy() {
                    self.run_stmt(*then_branch)
                } else if let Some(branch) = else_branch {
                    self.run_stmt(*branch)
                } else {
                    Ok(ControlFlow::None)
                }
            }
            Stmt::While { condition, body } => {
                while self.evaluate_expr(condition.clone())?.is_truthy() {
                    match self.run_stmt(*body.clone())? {
                        ControlFlow::None => (),
                        ControlFlow::Return(lox_var) => {
                            return Ok(ControlFlow::Return(lox_var));
                        }
                    };
                }
                Ok(ControlFlow::None)
            }
            Stmt::Function {
                name,
                parameters,
                body,
                span,
            } => {
                self.env.borrow_mut().define(
                    name.to_owned(),
                    LoxVar::Callable(Rc::new(LoxFunc::new(
                        parameters
                            .len()
                            .try_into()
                            .map_err(|_| RuntimeError::TooManyArguments(parameters.len(), span))?,
                        body,
                        parameters,
                        name,
                        self.env.clone(),
                        false,
                    ))),
                );
                Ok(ControlFlow::None)
            }
            Stmt::Return { value, .. } => Ok(ControlFlow::Return(
                self.evaluate_expr(value.unwrap_or(Expr::Literal(Literal::Nil)))?,
            )),
            Stmt::Class {
                name,
                methods,
                super_class,
                span,
            } => {
                let super_class = if let Some(expr) = super_class {
                    let var = self.evaluate_expr(expr.clone())?;

                    if let LoxVar::Class(s_class) = var {
                        Some(s_class)
                    } else {
                        return Err(RuntimeError::SuperClassIsNotAClass(var, span));
                    }
                } else {
                    None
                };

                let mut method_env = self.env.clone();

                if let Some(ref super_class) = super_class {
                    let new_env = Env::new_env(Some(self.env.clone()));
                    new_env
                        .borrow_mut()
                        .define("super".to_owned(), LoxVar::Class(super_class.clone()));

                    method_env = new_env;
                }

                let mut method_map = HashMap::new();

                for method in methods {
                    if let Stmt::Function {
                        name: m_name,
                        parameters,
                        body,
                        ..
                    } = method
                    {
                        let func = LoxFunc::new(
                            parameters.len() as u8,
                            body,
                            parameters,
                            m_name.clone(),
                            method_env.clone(),
                            m_name == "init",
                        );

                        method_map.insert(m_name, Rc::new(func));
                    }
                }

                let class = LoxClass {
                    name: name.clone(),
                    methods: method_map,
                    super_class,
                };

                self.env
                    .borrow_mut()
                    .define(name, LoxVar::Class(Rc::new(class)));

                Ok(ControlFlow::None)
            }
        }
    }

    pub fn evaluate_expr(&mut self, expr: Expr) -> Result<LoxVar, RuntimeError> {
        use crate::tree_walk::token::TokenKind;
        use core::ops::{Add as _, Div as _, Mul as _, Sub as _};
        match expr {
            Expr::Literal(literal) => Ok(literal.into()),
            Expr::Unary {
                operator,
                right,
                span,
            } => {
                let right_var = self.evaluate_expr(*right)?;
                match operator {
                    TokenKind::Minus => {
                        if let LoxVar::Number(n) = right_var {
                            Ok(LoxVar::Number(-n))
                        } else {
                            Err(EvaluateError::new(
                                EvaluateErrorKind::IncompatableUnaryOperator {
                                    operator: TokenKind::Minus,
                                    var_ty: right_var.into(),
                                },
                                span,
                            )
                            .into())
                        }
                    }
                    TokenKind::Bang => Ok(LoxVar::Bool(!right_var.is_truthy())),
                    _ => {
                        unreachable!("only tokens with kind minus and bang can be unary operators")
                    }
                }
            }
            Expr::Binary {
                left,
                operator,
                right,
                span,
            } => {
                let left_var = self.evaluate_expr(*left)?;
                let right_var = self.evaluate_expr(*right)?;

                use TokenKind::*;
                use core::cmp::Ordering;

                match operator {
                    Greater => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Greater))
                        } else {
                            Err(
                                EvaluateError::bin_eval_error(span, Greater, &left_var, &right_var)
                                    .into(),
                            )
                        }
                    }
                    GreaterEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Less))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                span,
                                GreaterEqual,
                                &left_var,
                                &right_var,
                            )
                            .into())
                        }
                    }
                    Less => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Less))
                        } else {
                            Err(
                                EvaluateError::bin_eval_error(span, Less, &left_var, &right_var)
                                    .into(),
                            )
                        }
                    }
                    LessEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Greater))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                span, LessEqual, &left_var, &right_var,
                            )
                            .into())
                        }
                    }
                    EqualEqual => Ok(LoxVar::Bool(left_var.eq(&right_var))),
                    BangEqual => Ok(LoxVar::Bool(left_var.ne(&right_var))),
                    Minus => left_var
                        .sub(&right_var)
                        .map_err(|(l, r)| EvaluateError::bin_eval_error(span, Minus, l, r).into()),
                    Slash => {
                        if right_var == LoxVar::Number(0.) {
                            Err(EvaluateError::new(EvaluateErrorKind::DivisionByZero, span).into())
                        } else {
                            left_var.div(&right_var).map_err(|(l, r)| {
                                EvaluateError::bin_eval_error(span, Slash, l, r).into()
                            })
                        }
                    }
                    Star => left_var
                        .mul(&right_var)
                        .map_err(|(l, r)| EvaluateError::bin_eval_error(span, Star, l, r).into()),
                    Plus => left_var
                        .add(&right_var)
                        .map_err(|(l, r)| EvaluateError::bin_eval_error(span, Plus, l, r).into()),
                    _ => unreachable!(
                        "only tokens with kind gt, ge, lt, le, ne, eq, minus, plus, slash and star can be binary operators"
                    ),
                }
            }
            Expr::Grouping(expr) => self.evaluate_expr(*expr),
            Expr::Variable {
                name,
                span,
                distance,
            } => self.env.borrow().get(&name, distance).ok_or(
                EvaluateError::new(
                    EvaluateErrorKind::UndeclaredVariable {
                        name: name.to_owned(),
                    },
                    span,
                )
                .into(),
            ),
            Expr::Assign {
                name,
                span,
                value,
                distance,
            } => {
                let value = self.evaluate_expr(*value)?;
                self.env
                    .borrow_mut()
                    .assign(&name, &value, distance)
                    .map_err(|err| EvaluateError::new(err, span))?;
                Ok(value)
            }
            Expr::Logical {
                left,
                operator,
                right,
                ..
            } => {
                let left = self.evaluate_expr(*left)?;
                match operator {
                    TokenKind::And => {
                        if !left.is_truthy() {
                            Ok(left)
                        } else {
                            self.evaluate_expr(*right)
                        }
                    }
                    TokenKind::Or => {
                        if left.is_truthy() {
                            Ok(left)
                        } else {
                            self.evaluate_expr(*right)
                        }
                    }
                    _ => unreachable!("Logical expr can only have operators Or or And"),
                }
            }
            Expr::Call { callee, args, span } => {
                self.call_depth += 1;

                let arguments: Vec<LoxVar> = args
                    .iter()
                    .map(|expr| self.evaluate_expr(expr.clone()))
                    .collect::<Result<_, _>>()?;
                let res = match self.evaluate_expr(*callee)? {
                    LoxVar::Callable(func) => {
                        let ret = func.call(self, &arguments).map_err(|mut err| {
                            err.call_span = Some(Box::new(span));
                            err.input_args = arguments;

                            err
                        })?;

                        if self.call_depth > MAX_CALL_DEPTH {
                            return Err(RuntimeError::StackOverflow);
                        }

                        Ok(ret)
                    }
                    LoxVar::Class(class) => {
                        let ret = class.call(self, &arguments).map_err(|mut err| {
                            err.call_span = Some(Box::new(span));
                            err.input_args = arguments;

                            err
                        })?;

                        if self.call_depth > MAX_CALL_DEPTH {
                            return Err(RuntimeError::StackOverflow);
                        }

                        Ok(ret)
                    }
                    other => Err(EvaluateError::new(
                        EvaluateErrorKind::NonCallable {
                            name: other.to_string(),
                        },
                        span,
                    )
                    .into()),
                };

                self.call_depth -= 1;

                return res;
            }
            Expr::Get { object, name, span } => {
                let obj = self.evaluate_expr(*object)?;

                if let LoxVar::Instance(inst) = obj {
                    if let Some(property) = inst.get(&name) {
                        Ok(property)
                    } else if let Some(method) = inst.borrow().class.find_method(&name) {
                        Ok(LoxVar::Callable(method))
                    } else {
                        Err(
                            EvaluateError::new(EvaluateErrorKind::UndefinedProperty(name), span)
                                .into(),
                        )
                    }
                } else {
                    Err(EvaluateError::new(EvaluateErrorKind::NotInstance(obj.into()), span).into())
                }
            }
            Expr::Set {
                object,
                name,
                value,
                span,
            } => {
                let obj = self.evaluate_expr(*object)?;

                if let LoxVar::Instance(mut inst) = obj {
                    let val = self.evaluate_expr(*value)?;

                    inst.set(name, val.clone());

                    Ok(val)
                } else {
                    Err(EvaluateError::new(EvaluateErrorKind::NotInstance(obj.into()), span).into())
                }
            }
            Expr::This { distance, .. } => Ok(self
                .env
                .borrow()
                .get("this", distance)
                .expect("Cannot have a reference to this outside of method")),
            Expr::Super {
                distance,
                method_name,
                span,
            } => {
                let dist = distance.expect("super must be resolved");

                let superclass = match self.env.borrow().get("super", Some(dist)) {
                    Some(LoxVar::Class(cls)) => cls,
                    _ => unreachable!("super must be a class"),
                };

                let instance = match self.env.borrow().get("this", Some(dist - 1)) {
                    Some(LoxVar::Instance(inst)) => inst,
                    _ => unreachable!("this must be an instance"),
                };

                let method = superclass
                    .find_method(&method_name)
                    .ok_or(EvaluateError::new(
                        EvaluateErrorKind::UndefinedProperty(method_name.to_owned()),
                        span,
                    ))?;

                Ok(LoxVar::Callable(method.bind(instance)))
            }
        }
    }
}

pub mod env {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use crate::tree_walk::{error::eval_err::EvaluateErrorKind, lox_var::LoxVar};
    pub trait EnvironmentImpl
    where
        Self: Sized,
    {
        fn debug_print(&self);
        fn new_env(enclosing: Option<Self>) -> Self;
        fn new_with_globals(globals: Vec<(&'static str, LoxVar)>) -> Self;
    }

    pub type Env = Rc<RefCell<Environment>>;

    impl EnvironmentImpl for Env {
        fn new_env(enclosing: Option<Self>) -> Self {
            Rc::new(RefCell::new(Environment::new(enclosing)))
        }

        fn new_with_globals(globals: Vec<(&'static str, LoxVar)>) -> Self {
            Rc::new(RefCell::new(Environment::new_with_globals(globals)))
        }

        fn debug_print(&self) {
            println!("=== ENVIRONMENT DUMP ===");

            let mut current = Some(self.clone());
            let mut depth = 0;

            while let Some(env_rc) = current {
                let env_ref = env_rc.borrow();

                println!(
                    "\nScope {} {}:",
                    depth,
                    if depth == 0 {
                        "(current)"
                    } else if env_ref.enclosing.is_none() {
                        "(global)"
                    } else {
                        ""
                    }
                );

                for (k, v) in &env_ref.values {
                    println!("  {} = {:?}", k, v);
                }

                current = env_ref.enclosing.clone();
                depth += 1;
            }

            println!("\n========================");
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
            Self {
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
            }
        }

        pub fn define(&mut self, name: String, value: LoxVar) {
            self.values.insert(name, value);
        }

        pub fn get_at(&self, dist: usize, name: &str) -> Option<LoxVar> {
            if dist == 0 {
                return self.values.get(name).cloned();
            }

            self.enclosing.as_ref()?.borrow().get_at(dist - 1, name)
        }

        pub fn get_global(&self, name: &str) -> Option<LoxVar> {
            if let Some(ref parent) = self.enclosing {
                return parent.borrow().get_global(name);
            }

            if let Some(v) = self.values.get(name) {
                return Some(v.clone());
            }

            None
        }

        pub fn get(&self, name: &str, distance: Option<usize>) -> Option<LoxVar> {
            #[cfg(feature = "debug_print")]
            println!("LOOKUP: {} @ {:?}", name, distance);
            if let Some(dist) = distance {
                self.get_at(dist, name)
            } else {
                self.get_global(name)
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
            } else if let Some(ref parent) = self.enclosing {
                parent.borrow_mut().assign(name, value, None)
            } else if self
                .values
                .insert(name.to_owned(), value.to_owned())
                .is_some()
            {
                Ok(())
            } else {
                Err(EvaluateErrorKind::UndeclaredVariable {
                    name: name.to_owned(),
                })
            }
        }
    }
}
