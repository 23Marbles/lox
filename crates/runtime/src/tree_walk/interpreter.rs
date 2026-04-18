//! Interpreter for the Lox language.

use std::{
    cell::RefCell,
    collections::HashMap,
    ops::{Neg as _, Rem as _},
    rc::Rc,
};

use crate::{
    error::{
        ToCodespan as _,
        eval_err::{EvaluateError, EvaluateErrorKind},
        fn_err::FunctionError,
        runtime_err::RuntimeError,
    },
    lox_files::FileIdent,
    tree_walk::{
        ast::{
            Literal,
            expr::{Expr, FieldAccess},
            stmt::{ControlFlow, Stmt},
        },
        interpreter::env::{Env, EnvironmentImpl as _},
        lox_var::{
            BinErrReason, LoxVar, LoxVarKind, PropertyAccess as _,
            class::LoxClass,
            functions::{LoxCallable as _, LoxFunc},
            list::{ListAccessKind, LoxList},
        },
    },
};

const MAX_CALL_DEPTH: usize = 1024;

#[derive(Debug, Clone)]
/// Represents an interpreter that executes a sequence of statements.
pub struct Interpreter {
    statements: Vec<Stmt>,
    ip: usize,
    out_buf: InterpOut,
    /// The environment in which the interpreter executes.
    pub env: Env,
    call_depth: usize,
    pub(crate) file_id: FileIdent,
}

#[derive(Debug, Clone)]
/// Represents the type of a line of output, either normal or an error.
pub enum LineType {
    /// A normal line of output.
    Normal,
    /// An error line of output.
    Error,
}

#[derive(Debug, Clone, Default)]
/// Represents the output of an interpreter, consisting of a list of lines.
pub struct InterpOut {
    lines: Vec<(String, LineType)>,
}

impl InterpOut {
    /// Returns the lines of output as a vector of strings.
    pub fn fall_lines(&self) -> Vec<String> {
        self.lines.clone().into_iter().map(|(s, _)| s).collect()
    }

    /// Returns the lines of output as a vector of (string, line type) pairs.
    pub fn marked_lines(&self) -> Vec<(String, LineType)> {
        self.lines.clone()
    }

    /// Pushes a string to the output buffer, marking it as a normal line.
    fn println(&mut self, value: impl ToString) {
        for s in value.to_string().lines() {
            self.lines.push((s.to_owned(), LineType::Normal));
        }
    }

    /// Pushes a string to the output buffer, marking it as an error line.
    fn eprintln(&mut self, value: impl ToString) {
        for s in value.to_string().lines() {
            self.lines.push((s.to_owned(), LineType::Error));
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
    /// Creates a new interpreter with the given file ID.
    pub fn new(file_id: FileIdent) -> Self {
        Self {
            statements: Vec::new(),
            env: Env::new_env(None),
            ip: 0,
            out_buf: InterpOut::default(),
            call_depth: 0,
            file_id,
        }
    }

    /// Returns a new interpreter with the given statements.
    pub fn with_stmts(mut self, statements: &[Stmt]) -> Self {
        self.ip = 0;
        self.statements = statements.to_vec();
        self
    }

    /// Sets the statements for the interpreter.
    pub fn set_stmts(&mut self, statements: &[Stmt]) {
        self.ip = 0;
        self.statements = statements.to_vec();
    }

    /// Returns a new interpreter with the given environment.
    pub fn with_env(mut self, env: Env) -> Self {
        self.env = env;
        self
    }

    /// Sets the environment for the interpreter.
    pub fn set_env(&mut self, env: Env) {
        self.env = env;
    }

    /// Returns a new interpreter with the given globals.
    pub fn with_globals(mut self, globals: Vec<(&'static str, LoxVar)>) -> Self {
        self.env = Env::new_with_globals(
            globals.to_vec(),
        );
        self
    }

    /// Prints an error to the output buffer.
    pub fn eprintln(&mut self, err: impl ToString) {
        self.out_buf.eprintln(err);
    }

    /// Prints a value to the output buffer.
    pub fn println(&mut self, err: impl ToString) {
        self.out_buf.println(err);
    }

    /// Runs the interpreter and returns the output.
    pub fn run(&mut self) -> crate::Result<Vec<InterpOut>> {
        self.into_iter()
            .map(|r| r.map_err(|e| vec![e.to_codespan()]))
            .collect::<Result<Vec<_>, _>>()
    }

    /// Runs a single statement and returns the control flow.
    pub fn run_stmt(&mut self, stmt: Stmt) -> Result<ControlFlow, RuntimeError> {
        match stmt {
            Stmt::Expression(expr) => {
                self.evaluate_expr(expr)?;
                Ok(ControlFlow::None)
            }
            Stmt::Print(expr) => {
                let value = self.evaluate_expr(expr)?;
                self.out_buf.println(value);
                Ok(ControlFlow::None)
            }
            Stmt::Var {
                name, initializer, ..
            } => {
                let value = match initializer {
                    Some(value) => self.evaluate_expr(value)?,
                    None => LoxVar::Uninitialized,
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
                            .map_err(|_| RuntimeError::Over255Arguments(parameters.len(), span))?,
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
                    let var = self.evaluate_expr(expr)?;

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

                let class = LoxClass::new(name.clone(), method_map, super_class);

                self.env
                    .borrow_mut()
                    .define(name, LoxVar::Class(Rc::new(class)));

                Ok(ControlFlow::None)
            }
        }
    }

    /// Evaluates an expression and returns the result.
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
                    TokenKind::Minus => right_var.neg().map_err(|e| {
                        EvaluateError::new(
                            EvaluateErrorKind::IncompatableUnaryOperator {
                                operator: TokenKind::Minus,
                                var_ty: e.into(),
                            },
                            span,
                        )
                        .into()
                    }),
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
                            Err(EvaluateError::bin_eval_error(
                                span,
                                Greater,
                                &left_var,
                                &right_var,
                                BinErrReason::Incompatible,
                            )
                            .into())
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
                                BinErrReason::Incompatible,
                            )
                            .into())
                        }
                    }
                    Less => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Less))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                span,
                                Less,
                                &left_var,
                                &right_var,
                                BinErrReason::Incompatible,
                            )
                            .into())
                        }
                    }
                    LessEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Greater))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                span,
                                LessEqual,
                                &left_var,
                                &right_var,
                                BinErrReason::Incompatible,
                            )
                            .into())
                        }
                    }
                    EqualEqual => Ok(LoxVar::Bool(left_var.eq(&right_var))),
                    BangEqual => Ok(LoxVar::Bool(left_var.ne(&right_var))),
                    Minus => left_var.sub(&right_var).map_err(|(l, r, reason)| {
                        EvaluateError::bin_eval_error(span, Minus, l, r, reason).into()
                    }),
                    Slash => {
                        #[cfg(feature = "div_zero_error")]
                        if right_var == LoxVar::Number(0.) {
                            return Err(EvaluateError::new(
                                EvaluateErrorKind::DivisionByZero,
                                span,
                            )
                            .into());
                        }
                        left_var.div(&right_var).map_err(|(l, r, reason)| {
                            EvaluateError::bin_eval_error(span, Slash, l, r, reason).into()
                        })
                    }
                    Star => left_var.mul(&right_var).map_err(|(l, r, reason)| {
                        EvaluateError::bin_eval_error(span, Star, l, r, reason).into()
                    }),
                    Percent => left_var.rem(&right_var).map_err(|(l, r, reason)| {
                        EvaluateError::bin_eval_error(span, Percent, l, r, reason).into()
                    }),
                    Plus => left_var.add(&right_var).map_err(|(l, r, reason)| {
                        EvaluateError::bin_eval_error(span, Plus, l, r, reason).into()
                    }),
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
            } => self
                .env
                .borrow()
                .get(&name, distance)
                .map_err(|k| EvaluateError::new(k, span).into()),
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
                        if arguments.len() != func.arity() as usize {
                            return Err(EvaluateError::new(
                                EvaluateErrorKind::IncorrectArity {
                                    expected: func.arity(),
                                    got: arguments.len(),
                                },
                                span,
                            )
                            .into());
                        }

                        let ret = func.call(self, &arguments).map_err(|err| {
                            FunctionError::from_runtime_error(
                                err,
                                func.signature(),
                                arguments.clone(),
                                span.clone(),
                            )
                        })?;

                        if self.call_depth > MAX_CALL_DEPTH {
                            return Err(RuntimeError::StackOverflow);
                        }

                        Ok(ret)
                    }
                    LoxVar::Class(class) => {
                        if arguments.len() != class.arity() as usize {
                            return Err(EvaluateError::new(
                                EvaluateErrorKind::IncorrectArity {
                                    expected: class.arity(),
                                    got: arguments.len(),
                                },
                                span,
                            )
                            .into());
                        }

                        let ret = class.call(self, &arguments).map_err(|err| {
                            FunctionError::from_runtime_error(
                                err,
                                class.name.clone(),
                                arguments.clone(),
                                span.clone(),
                            )
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

                res
            }
            Expr::Get {
                object,
                span,
                get_type,
            } => {
                let obj = self.evaluate_expr(*object)?;

                match obj {
                    LoxVar::Instance(inst) => {
                        let name = match get_type {
                            FieldAccess::Property(name) => name,
                            FieldAccess::Index(_) => {
                                return Err(RuntimeError::EvaluateError(EvaluateError::new(
                                    EvaluateErrorKind::CannotIndexInto(LoxVarKind::Instance),
                                    span,
                                )));
                            }
                        };
                        inst.get(&name)
                            .or_else(|_| {
                                Ok(LoxVar::Callable(inst.borrow().class().find_method(&name)?))
                            })
                            .map_err(|e| RuntimeError::AccessError(e, span))
                    }
                    LoxVar::List(list) => list
                        .get(&match get_type {
                            FieldAccess::Index(expr) => {
                                ListAccessKind::Index(self.evaluate_expr(*expr)?)
                            }
                            FieldAccess::Property(name) => ListAccessKind::Property(name),
                        })
                        .map_err(|e| RuntimeError::AccessError(e, span)),
                    v => Err(EvaluateError::new(
                        match get_type {
                            FieldAccess::Property(_) => {
                                EvaluateErrorKind::CannotGetProperties(v.kind())
                            }
                            FieldAccess::Index(_) => EvaluateErrorKind::CannotIndexInto(v.kind()),
                        },
                        span,
                    )
                    .into()),
                }
            }
            Expr::Set {
                object,
                value,
                span,
                set_type,
            } => {
                let obj = self.evaluate_expr(*object)?;

                match obj {
                    LoxVar::Instance(mut inst) => {
                        let val = self.evaluate_expr(*value)?;

                        inst.set(
                            if let FieldAccess::Property(name) = set_type {
                                name
                            } else {
                                return Err(RuntimeError::EvaluateError(EvaluateError::new(
                                    EvaluateErrorKind::CannotIndexInto(LoxVarKind::Instance),
                                    span,
                                )));
                            },
                            val.clone(),
                        )
                        .map_err(|e| RuntimeError::AccessError(e, span))?;

                        Ok(val)
                    }
                    LoxVar::List(mut list) => {
                        let val = self.evaluate_expr(*value)?;

                        list.set(
                            match set_type {
                                FieldAccess::Index(expr) => {
                                    ListAccessKind::Index(self.evaluate_expr(*expr)?)
                                }
                                FieldAccess::Property(name) => ListAccessKind::Property(name),
                            },
                            val.clone(),
                        )
                        .map_err(|e| RuntimeError::AccessError(e, span))?;

                        Ok(val)
                    }
                    v => Err(EvaluateError::new(
                        match set_type {
                            FieldAccess::Property(_) => {
                                EvaluateErrorKind::CannotGetProperties(v.kind())
                            }
                            FieldAccess::Index(_) => EvaluateErrorKind::CannotIndexInto(v.kind()),
                        },
                        span,
                    )
                    .into()),
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

                let superclass = match self
                    .env
                    .borrow()
                    .get("super", Some(dist))
                    .map_err(|k| EvaluateError::new(k, span.clone()))?
                {
                    LoxVar::Class(cls) => cls,
                    _ => unreachable!("super must be a class"),
                };

                let instance = match self
                    .env
                    .borrow()
                    .get("this", Some(dist - 1))
                    .map_err(|k| EvaluateError::new(k, span.clone()))?
                {
                    LoxVar::Instance(inst) => inst,
                    _ => unreachable!("this must be an instance"),
                };

                let method = superclass
                    .find_method(&method_name)
                    .map_err(|k| RuntimeError::AccessError(k, span))?;

                Ok(LoxVar::Callable(method.bind(instance)))
            }
            Expr::List { parts, .. } => Ok(LoxVar::List(Rc::new(RefCell::new(LoxList::new(
                parts
                    .into_iter()
                    .map(|ex| self.evaluate_expr(ex))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .collect::<Result<Vec<_>, _>>()?,
            ))))),
        }
    }
}

/// Module for the environment, which holds the current scope's variables and allows looking up values by name.
pub mod env {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use super::*;
    pub(crate) trait EnvironmentImpl
    where
        Self: Sized,
    {
        fn debug_print_env(&self);
        fn new_env(enclosing: Option<Self>) -> Self;
        fn new_with_globals(globals: Vec<(&'static str, LoxVar)>) -> Self;
    }

    pub(crate) type Env = Rc<RefCell<Environment>>;

    impl EnvironmentImpl for Env {
        fn new_env(enclosing: Option<Self>) -> Self {
            Rc::new(RefCell::new(Environment::new(enclosing)))
        }

        fn new_with_globals(globals: Vec<(&'static str, LoxVar)>) -> Self {
            Rc::new(RefCell::new(Environment::new_with_globals(globals)))
        }

        fn debug_print_env(&self) {
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
    /// Represents an environment, holding variables and allowing lookups by name.
    pub struct Environment {
        enclosing: Option<Env>,
        values: HashMap<String, LoxVar>,
    }

    impl Environment {
        /// Creates a new environment with the given parent.
        pub fn new(parent: Option<Env>) -> Environment {
            Self {
                enclosing: parent,
                values: HashMap::new(),
            }
        }

        /// Creates a new environment with the given globals.
        pub fn new_with_globals(globals: Vec<(&'static str, LoxVar)>) -> Environment {
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

        /// Defines a variable in this environment.
        pub fn define(&mut self, name: String, value: LoxVar) {
            self.values.insert(name, value);
        }

        /// Gets a variable from this environment, searching up the enclosing chain if necessary.
        fn get_at(&self, dist: usize, name: &str) -> Result<LoxVar, EvaluateErrorKind> {
            if dist == 0 {
                if let Some(v) = self.values.get(name) {
                    if v.is_uninitialized() {
                        return Err(EvaluateErrorKind::UninitializedVariable {
                            name: name.to_owned(),
                        });
                    }
                    return Ok(v.clone());
                } else {
                    return Err(EvaluateErrorKind::UndeclaredVariable {
                        name: name.to_owned(),
                    });
                }
            }

            self.enclosing
                .as_ref()
                .ok_or(EvaluateErrorKind::UndeclaredVariable {
                    name: name.to_owned(),
                })?
                .borrow()
                .get_at(dist - 1, name)
        }

        /// Gets a variable from the global scope.
        fn get_global(&self, name: &str) -> Result<LoxVar, EvaluateErrorKind> {
            if let Some(ref parent) = self.enclosing {
                return parent.borrow().get_global(name);
            }

            if let Some(v) = self.values.get(name) {
                if v.is_uninitialized() {
                    return Err(EvaluateErrorKind::UninitializedVariable {
                        name: name.to_owned(),
                    });
                }
                return Ok(v.clone());
            }

            Err(EvaluateErrorKind::UndeclaredVariable {
                name: name.to_owned(),
            })
        }

        /// Gets a variable from this environment, searching up the enclosing chain if necessary.
        /// If `distance` is `Some`, the variable is looked up at the given distance,
        /// if `distance` is `None`, the variable is looked up in the global scope.
        pub fn get(
            &self,
            name: &str,
            distance: Option<usize>,
        ) -> Result<LoxVar, EvaluateErrorKind> {
            //#[cfg(debug_assertions)]
            //println!("LOOKUP: {} @ {:?}", name, distance);
            if let Some(dist) = distance {
                self.get_at(dist, name)
            } else {
                self.get_global(name)
            }
        }

        /// Assigns a value to a variable in this environment, searching up the enclosing chain if necessary.
        /// If `dist` is `Some`, the variable is assigned at the given distance,
        /// if `dist` is `None`, the variable is assigned in the global scope.
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
