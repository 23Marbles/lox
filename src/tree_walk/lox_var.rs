use std::{
    fmt::{Debug, Display},
    ops::*,
    rc::Rc,
};

use crate::tree_walk::{
    ast::Literal,
    lox_var::{
        class::{InstanceRef, LoxClass},
        functions::LoxCallable,
    },
};

pub mod functions {
    use std::{fmt::Debug, rc::Rc};

    use crate::{
        error::fn_err::{FunctionError, FunctionErrorKind},
        tree_walk::{
            ast::stmt::{ControlFlow, Stmt},
            interpreter::{
                Interpreter,
                env::{Env, EnvironmentImpl as _},
            },
            lox_var::{InstanceRef, LoxVar},
        },
    };

    pub trait LoxCallable: Debug {
        fn call(
            &self,
            interpreter: &mut Interpreter,
            arguments: &[LoxVar],
        ) -> Result<LoxVar, FunctionError>;

        fn signature(&self) -> String;
    }

    pub struct LoxFunc {
        arity: u8,
        name: String,
        body: Vec<Stmt>,
        parameters: Vec<String>,
        closure: Env,
        is_init: bool,
    }

    impl PartialEq for LoxFunc {
        fn eq(&self, other: &Self) -> bool {
            self.arity == other.arity
                && self.name == other.name
                && self.body == other.body
                && self.parameters == other.parameters
                && self.is_init == other.is_init
        }
    }

    impl Debug for LoxFunc {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            f.debug_struct("LoxFunc")
                .field("arity", &self.arity)
                .field("name", &self.name)
                .field("body", &self.body)
                .field("parameters", &self.parameters)
                //.field("closure", &self.closure)
                .field("is_init", &self.is_init)
                .finish()
        }
    }

    impl LoxFunc {
        pub fn new(
            arity: u8,
            body: Vec<Stmt>,
            parameters: Vec<String>,
            name: String,
            closure: Env,
            is_init: bool,
        ) -> Self {
            Self {
                arity,
                body,
                parameters,
                name,
                closure,
                is_init,
            }
        }

        pub fn arity(&self) -> u8 {
            self.arity
        }

        pub fn bind(&self, instance: InstanceRef) -> Rc<LoxFunc> {
            let env = Env::new_env(Some(self.closure.clone()));

            env.borrow_mut()
                .define("this".to_owned(), LoxVar::Instance(instance));

            Rc::new(LoxFunc {
                arity: self.arity,
                name: self.name.clone(),
                body: self.body.clone(),
                parameters: self.parameters.clone(),
                closure: env,
                is_init: self.is_init,
            })
        }
    }

    impl LoxCallable for LoxFunc {
        fn call(
            &self,
            interpreter: &mut Interpreter,
            arguments: &[LoxVar],
        ) -> Result<LoxVar, FunctionError> {
            if arguments.len() != self.arity as usize {
                return Err(FunctionError::new(FunctionErrorKind::IncorrectArity {
                    expected: self.arity,
                    got: arguments.len(),
                }));
            }

            let new_env = Env::new_env(Some(self.closure.clone()));

            for (name, arg) in self.parameters.iter().zip(arguments) {
                new_env.borrow_mut().define(name.to_owned(), arg.to_owned());
            }

            let previous = interpreter.env.clone();

            interpreter.env = new_env.clone();

            let mut result: Option<LoxVar> = None;

            for stmt in &self.body {
                match interpreter.run_stmt(stmt.clone()) {
                    Ok(ControlFlow::None) => {}
                    Ok(ControlFlow::Return(val)) => {
                        result = Some(val);
                        break;
                    }
                    Err(e) => {
                        interpreter.env = previous;
                        return Err(e.into());
                    }
                }
            }

            interpreter.env = previous;

            if self.is_init {
                return Ok(new_env
                    .borrow()
                    .get("this", Some(1))
                    .expect("initializer must have this"));
            }

            Ok(result.unwrap_or(LoxVar::Nil))
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
}

pub mod class {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use crate::{
        error::fn_err::{FunctionError, FunctionErrorKind},
        tree_walk::{interpreter::Interpreter, lox_var::functions::LoxFunc},
    };

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    pub struct LoxClass {
        pub name: String,
        pub methods: HashMap<String, Rc<LoxFunc>>,
        pub super_class: Option<Rc<LoxClass>>,
    }

    impl LoxClass {
        pub fn find_method(&self, name: &str) -> Option<Rc<LoxFunc>> {
            if let Some(m) = self.methods.get(name) {
                Some(m.clone())
            } else {
                self.super_class.as_ref()?.find_method(name)
            }
        }

        fn arity(&self) -> Option<u8> {
            Some(self.find_method("init")?.arity())
        }
    }

    impl LoxCallable for LoxClass {
        fn call(
            &self,
            interpreter: &mut Interpreter,
            arguments: &[LoxVar],
        ) -> Result<LoxVar, FunctionError> {
            if arguments.len() != self.arity().unwrap_or(0) as usize {
                return Err(FunctionError::new(FunctionErrorKind::IncorrectArity {
                    expected: self.arity().unwrap_or(0),
                    got: arguments.len(),
                }));
            }

            let instance = Rc::new(RefCell::new(LoxInstance::new(Rc::new(self.clone()))));

            if let Some(init) = self.find_method("init") {
                init.bind(instance.clone()).call(interpreter, arguments)?;
            }

            Ok(LoxVar::Instance(instance))
        }

        fn signature(&self) -> String {
            format!("{}()", self.name)
        }
    }

    #[derive(Debug)]
    pub struct LoxInstance {
        pub class: Rc<LoxClass>,
        pub fields: HashMap<String, LoxVar>,
    }

    impl LoxInstance {
        pub fn new(class: Rc<LoxClass>) -> Self {
            Self {
                class,
                fields: HashMap::new(),
            }
        }
    }

    pub trait InstanceImpl {
        fn get(&self, name: &str) -> Option<LoxVar>;
        fn set(&mut self, name: String, value: LoxVar);
    }

    pub type InstanceRef = Rc<RefCell<LoxInstance>>;

    impl InstanceImpl for InstanceRef {
        fn get(&self, name: &str) -> Option<LoxVar> {
            if let Some(v) = self.borrow().fields.get(name) {
                return Some(v.clone());
            }

            if let Some(method) = self.borrow().class.find_method(name) {
                return Some(LoxVar::Callable(method.bind(self.clone())));
            }

            None
        }

        fn set(&mut self, name: String, value: LoxVar) {
            self.borrow_mut().fields.insert(name, value);
        }
    }
}

#[derive(Clone, Debug)]
pub enum LoxVar {
    Number(f64),
    Bool(bool),
    String(String),
    Callable(Rc<dyn LoxCallable>),
    Class(Rc<LoxClass>),
    Instance(InstanceRef),
    Nil,
}

#[derive(Debug, Clone, Copy)]
pub enum LoxVarKind {
    Number,
    Bool,
    String,
    Callable,
    Class,
    Instance,
    Nil,
}

impl From<LoxVar> for LoxVarKind {
    fn from(value: LoxVar) -> Self {
        match value {
            LoxVar::Nil => Self::Nil,
            LoxVar::Bool(..) => Self::Bool,
            LoxVar::Number(..) => Self::Number,
            LoxVar::String(..) => Self::String,
            LoxVar::Callable(..) => Self::Callable,
            LoxVar::Class(..) => Self::Class,
            LoxVar::Instance(..) => Self::Instance,
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
            LoxVar::Class(..) => Self::Class,
            LoxVar::Instance(..) => Self::Instance,
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
            LoxVarKind::String => Some(LoxVar::String(self.to_owned())),
        }
    }
}

impl From<Literal> for LoxVar {
    fn from(value: Literal) -> Self {
        match value {
            Literal::Number(n) => Self::Number(n),
            Literal::String(s) => Self::String(s),
            Literal::True => Self::Bool(true),
            Literal::False => Self::Bool(false),
            Literal::Nil => Self::Nil,
        }
    }
}

impl Display for LoxVar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LoxVar::Nil => write!(f, "nil"),
            LoxVar::Bool(b) => write!(f, "{b}"),
            LoxVar::Number(n) => write!(f, "{n}"),
            LoxVar::String(s) => write!(f, "{s}"),
            LoxVar::Callable(c) => write!(f, "<fn {}>", c.signature()),
            LoxVar::Class(c) => write!(f, "{}", c.name),
            LoxVar::Instance(i) => {
                write!(f, "{} instance", i.borrow().class.name)
            }
        }
    }
}

impl PartialEq for LoxVar {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(l), Self::Number(r)) => l == r,
            (Self::Bool(l), Self::Bool(r)) => l == r,
            (Self::String(l), Self::String(r)) => l == r,

            // identity comparison
            (Self::Callable(l), Self::Callable(r)) => Rc::ptr_eq(l, r),
            (Self::Class(l), Self::Class(r)) => Rc::ptr_eq(l, r),
            (Self::Instance(l), Self::Instance(r)) => Rc::ptr_eq(l, r),

            // nil == nil
            (Self::Nil, Self::Nil) => true,

            _ => false,
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
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
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
