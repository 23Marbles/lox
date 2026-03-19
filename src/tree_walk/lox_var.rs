use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{Debug, Display},
    ops::*,
    rc::Rc,
};

use crate::tree_walk::{
    ast::{Literal, Stmt},
    error::fn_err::{FunctionError, FunctionErrorKind},
};

pub trait EnvironmentImpl
where
    Self: Sized,
{
    fn register_globals(&mut self, lib: Vec<(String, Rc<dyn LoxCallable>)>);
    fn new(enclosing: Option<Self>) -> Self;
    fn descend(&mut self);
    fn ascend(&mut self);
}

pub type Env = Rc<RefCell<Environment>>;

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

impl EnvironmentImpl for Env {
    fn new(enclosing: Option<Env>) -> Env {
        Rc::new(RefCell::new(Environment::new(enclosing)))
    }

    fn descend(&mut self) {
        *self = <Env as EnvironmentImpl>::new(Some(self.clone()))
    }

    /// # Panics
    /// Panics if the parent is None (Tried to ascend above the global environment)
    fn ascend(&mut self) {
        let parent = self
            .borrow()
            .enclosing
            .clone()
            .expect("Tried to ascend above global");

        *self = parent;
    }

    fn register_globals(&mut self, lib: Vec<(String, Rc<dyn LoxCallable>)>) {
        let mut self_mut = self.borrow_mut();
        for (name, func) in lib {
            self_mut.define(name, LoxVar::Callable(func));
        }
    }
}

#[derive(Clone, Debug)]
pub struct Environment {
    enclosing: Option<Env>,
    values: HashMap<String, LoxVar>,
}

impl Environment {
    pub fn new(enclosing: Option<Env>) -> Self {
        Self {
            values: HashMap::new(),
            enclosing,
        }
    }

    pub fn define(&mut self, name: String, value: LoxVar) {
        self.values.insert(name, value);
    }

    pub fn get(&self, name: String) -> Option<LoxVar> {
        self.values.get(&name).cloned().or_else(|| {
            self.enclosing
                .as_ref()
                .and_then(|parent| parent.borrow().get(name))
        })
    }

    /// # Panics
    /// Panics if the enclosing variable is assigned and
    pub fn assign(&mut self, name: &str, value: LoxVar) -> bool {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
            return true;
        } else if let Some(ref parent) = self.enclosing {
            return parent.borrow_mut().assign(name, value);
        }

        false
    }
}

pub trait LoxCallable: Debug {
    fn call(&self, arguments: Vec<LoxVar>) -> Result<LoxVar, FunctionError>;

    fn signature(&self) -> String;
}

#[derive(Debug)]
pub struct LoxFunc {
    arity: u8,
    name: String,
    body: Vec<Stmt>,
    parameters: Vec<String>,
    closure: Env,
}

impl LoxFunc {
    pub fn new(arity: u8, body: Vec<Stmt>, parameters: Vec<String>, name: String, closure: &Env) -> Self {
        Self {
            arity,
            body,
            parameters,
            name,
            closure: closure.clone(),
        }
    }
}

impl LoxCallable for LoxFunc {
    fn call(&self, arguments: Vec<LoxVar>) -> Result<LoxVar, FunctionError> {
        if arguments.len() != self.arity as usize {
            Err(FunctionError::new(FunctionErrorKind::IncorrectArity {
                expected: self.arity,
                got: arguments.len(),
            }))
        } else {
            let mut env = self.closure.clone();
            let mut scope = ScopeGuard::new(&mut env);
            for (name, arg) in self.parameters.iter().zip(arguments) {
                scope.borrow_mut().define(name.to_string(), arg);
            }

            for stmt in &self.body {
                match stmt.run(&mut scope)? {
                    super::ast::ControlFlow::None => (),
                    super::ast::ControlFlow::Return(lox_var) => return Ok(lox_var),
                }
            }

            Ok(LoxVar::Nil) // Default to this if nothing is returned
        }
    }

    fn signature(&self) -> String {
        let mut sig = self.name.clone();

        sig += "(";

        if !self.parameters.is_empty() {
            sig += &self.parameters[0]
        }

        for p in self.parameters.iter().skip(1) {
            sig += ", ";
            sig += p;
        }

        sig += ")";

        sig
    }
}

#[derive(Clone, Debug)]
pub enum LoxVar {
    Nil,
    Bool(bool),
    Number(f64),
    String(String),
    Callable(Rc<dyn LoxCallable>),
}

#[derive(Debug, Clone, Copy)]
pub enum LoxVarKind {
    Nil,
    Bool,
    Number,
    String,
    Callable,
}

impl From<LoxVar> for LoxVarKind {
    fn from(value: LoxVar) -> Self {
        match value {
            LoxVar::Nil => Self::Nil,
            LoxVar::Bool(..) => Self::Bool,
            LoxVar::Number(..) => Self::Number,
            LoxVar::String(..) => Self::String,
            LoxVar::Callable(..) => Self::Callable,
        }
    }
}

impl From<&LoxVar> for LoxVarKind {
    fn from(value: &LoxVar) -> Self {
        match value {
            LoxVar::Nil => Self::Nil,
            LoxVar::Bool(..) => Self::Bool,
            LoxVar::Number(..) => Self::Number,
            LoxVar::String(..) => Self::String,
            LoxVar::Callable(..) => Self::Callable,
        }
    }
}

impl LoxVar {
    pub fn kind(&self) -> LoxVarKind {
        self.into()
    }

    fn expect_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            LoxVar::Nil => false,
            LoxVar::Bool(b) => *b,
            _ => true,
        }
    }

    #[cfg(feature = "implicit_converions")]
    pub fn try_convert(&self, to: LoxVarKind) -> Option<LoxVar> {
        match to {
            LoxVarKind::Nil => Some(LoxVar::Nil),
            LoxVarKind::Bool => Some(LoxVar::Bool(self.is_truthy())),
            LoxVarKind::Number => match self {
                LoxVar::Nil => None,
                LoxVar::Bool(b) => Some(LoxVar::Number(*b as u8 as f64)),
                LoxVar::Number(n) => Some(LoxVar::Number(*n)),
                LoxVar::String(s) => Some(LoxVar::Number(s.parse().ok()?)),
            },
            LoxVarKind::String => Some(LoxVar::String(self.to_string())),
        }
    }
}

impl From<&Literal> for LoxVar {
    fn from(value: &Literal) -> Self {
        match value {
            Literal::Number(n) => Self::Number(*n),
            Literal::String(s) => Self::String(s.to_string()),
            Literal::True => Self::Bool(true),
            Literal::False => Self::Bool(false),
            Literal::Nil => Self::Nil,
        }
    }
}

impl Display for LoxVar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoxVar::Nil => write!(f, "nil"),
            LoxVar::Bool(b) => write!(f, "{b}"),
            LoxVar::Number(n) => write!(f, "{n}"),
            LoxVar::String(s) => write!(f, "{s}"),
            LoxVar::Callable(c) => write!(f, "{}", c.signature()),
        }
    }
}

impl PartialEq for LoxVar {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bool(l0), Self::Bool(r0)) => l0 == r0,
            (Self::Number(l0), Self::Number(r0)) => l0 == r0,
            (Self::String(l0), Self::String(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

macro_rules! bin_num_op_impl {
    ($trait:ident, $fn:ident) => {
        impl $trait for &LoxVar {
            type Output = Result<LoxVar, (Self, Self)>;

            fn $fn(self, rhs: Self) -> Self::Output {
                let ln = self.expect_number().ok_or((self, rhs))?;
                let rn = rhs.expect_number().ok_or((self, rhs))?;
                Ok(LoxVar::Number(ln.$fn(rn)))
            }
        }
    };
}

impl Add for &LoxVar {
    type Output = Result<LoxVar, (Self, Self)>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (LoxVar::Number(ln), LoxVar::Number(rn)) => Ok(LoxVar::Number(ln + rn)),
            (LoxVar::String(ls), LoxVar::String(rs)) => Ok(LoxVar::String(ls.to_owned() + rs)),
            #[cfg(feature = "implicit_converions")]
            (LoxVar::String(s), other) => Ok(LoxVar::String(s.to_owned() + &other.to_string())),
            #[cfg(feature = "implicit_converions")]
            (other, LoxVar::String(s)) => Ok(LoxVar::String(other.to_string() + &s)),
            (l, r) => Err((l, r)),
        }
    }
}
bin_num_op_impl!(Sub, sub);
bin_num_op_impl!(Div, div);
bin_num_op_impl!(Mul, mul);

impl PartialOrd for LoxVar {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (LoxVar::Number(l), LoxVar::Number(r)) => l.partial_cmp(r),
            (_ls, _rs) => {
                #[cfg(feature = "implicit_converions")]
                {
                    _ls.try_convert(LoxVarKind::Number)?
                        .partial_cmp(&_rs.try_convert(LoxVarKind::Number)?)
                }
                #[cfg(not(feature = "implicit_converions"))]
                {
                    None
                }
            }
        }
    }
}
