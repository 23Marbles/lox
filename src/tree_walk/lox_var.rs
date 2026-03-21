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
    use std::fmt::Debug;

    use crate::tree_walk::{
        ast::{ControlFlow, Stmt},
        error::fn_err::{FunctionError, FunctionErrorKind},
        interpreter::env::{Env, EnvironmentImpl, ScopeGuard},
        lox_var::{InstanceRef, LoxVar},
    };

    pub trait LoxCallable: Debug {
        fn call(&self, arguments: &Vec<LoxVar>) -> Result<LoxVar, FunctionError>;

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
        pub fn new(
            arity: u8,
            body: Vec<Stmt>,
            parameters: Vec<String>,
            name: String,
            closure: &Env,
        ) -> Self {
            Self {
                arity,
                body,
                parameters,
                name,
                closure: closure.clone(),
            }
        }

        pub fn bind(&self, instance: InstanceRef) -> LoxFunc {
            let env = Env::new_env(Some(self.closure.clone()));

            env.borrow_mut()
                .define("this".to_owned(), LoxVar::Instance(instance));

            LoxFunc {
                arity: self.arity,
                name: self.name.clone(),
                body: self.body.clone(),
                parameters: self.parameters.clone(),
                closure: env,
            }
        }
    }

    impl LoxCallable for LoxFunc {
        fn call(&self, arguments: &Vec<LoxVar>) -> Result<LoxVar, FunctionError> {
            if arguments.len() != self.arity as usize {
                Err(FunctionError::new(FunctionErrorKind::IncorrectArity {
                    expected: self.arity,
                    got: arguments.len(),
                }))
            } else {
                let mut env = self.closure.clone();
                let mut scope = ScopeGuard::new(&mut env);
                for (name, arg) in self.parameters.iter().zip(arguments) {
                    scope.borrow_mut().define(name.to_owned(), arg.to_owned());
                }

                for stmt in &self.body {
                    match stmt.run(&mut scope)? {
                        ControlFlow::None => (),
                        ControlFlow::Return(lox_var) => return Ok(lox_var),
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
}

pub mod class {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use crate::tree_walk::{
        error::fn_err::{FunctionError, FunctionErrorKind},
        lox_var::{
            LoxVar,
            functions::{LoxCallable, LoxFunc},
        },
    };

    #[derive(Debug, Clone)]
    pub struct LoxClass {
        pub name: String,
        pub methods: HashMap<String, Rc<LoxFunc>>,
    }

    impl LoxClass {
        pub fn find_method(&self, name: &str) -> Option<Rc<LoxFunc>> {
            self.methods.get(name).cloned()
        }
    }

    impl LoxCallable for LoxClass {
        fn call(&self, arguments: &Vec<LoxVar>) -> Result<LoxVar, FunctionError> {
            if !arguments.is_empty() {
                return Err(FunctionError::new(FunctionErrorKind::IncorrectArity {
                    expected: 0,
                    got: arguments.len(),
                }));
            }

            let instance = Rc::new(RefCell::new(LoxInstance::new(Rc::new(self.clone()))));

            if let Some(init) = self.find_method("init") {
                init.bind(instance.clone()).call(arguments)?;
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
                return Some(LoxVar::Callable(Rc::new(method.bind(self.clone()))));
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
            LoxVar::Callable(c) => write!(f, "<{}>", c.signature()),
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
