//! Lox variable types.
//! Also contains the implementation of Lox functions and classes.

use std::{
    cell::RefCell,
    fmt::{Debug, Display},
    ops::*,
    rc::Rc,
};

use crate::{
    error::access_error::AccessError,
    tree_walk::{
        ast::Literal,
        lox_var::{
            class::{InstanceRef, LoxClass},
            functions::LoxCallable,
            list::{LoxList, LoxListRef},
        },
    },
};

/// Represents a type that supports property access via a key.
pub trait PropertyAccess {
    /// The type of the key used for property access.
    type Key;
    /// Gets the value associated with the given key.
    fn get(&self, key: &Self::Key) -> Result<LoxVar, AccessError>;
    /// Sets the value associated with the given key.
    fn set(&mut self, key: Self::Key, value: LoxVar) -> Result<(), AccessError>;
}

/// Contains the implementation of Lox functions.
pub mod functions {
    use std::{fmt::Debug, rc::Rc};

    use crate::{
        error::runtime_err::RuntimeError,
        tree_walk::{
            ast::stmt::{ControlFlow, Stmt},
            interpreter::{
                Interpreter,
                env::{Env, EnvironmentImpl as _},
            },
            lox_var::{InstanceRef, LoxVar},
        },
    };

    /// Represents a callable Lox function.
    pub trait LoxCallable: Debug {
        /// Calls the function with the given arguments.
        fn call(
            &self,
            interpreter: &mut Interpreter,
            arguments: &[LoxVar],
        ) -> Result<LoxVar, RuntimeError>;

        /// Returns the signature of the function, including parameter names and types.
        fn signature(&self) -> String;

        /// Returns the arity of the function, i.e. the number of parameters it takes.
        fn arity(&self) -> u8;
    }

    /// Represents a Lox function defined in the Lox code.
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
        /// Creates a new Lox function with the given arity, body, parameters, name, closure, and initialization flag.
        /// The `closure` is the environment in which the function is defined.
        /// The `is_init` flag indicates whether the function is an initializer for a class.
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

        /// Returns the arity of the function, i.e. the number of arguments it expects.
        pub fn arity(&self) -> u8 {
            self.arity
        }

        /// Binds the function to an instance, creating a new closure with the instance bound to `this`.
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
        ) -> Result<LoxVar, RuntimeError> {
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
                        return Err(e);
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

        fn arity(&self) -> u8 {
            self.parameters.len() as u8
        }
    }
}

/// Represents a Lox class, including its name, methods, and superclass.
pub mod class {
    use std::{cell::RefCell, collections::HashMap, rc::Rc};

    use crate::{
        error::{access_error, runtime_err::RuntimeError},
        tree_walk::{interpreter::Interpreter, lox_var::functions::LoxFunc},
    };

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    /// Represents a Lox class, including its name, methods, and superclass.
    pub struct LoxClass {
        /// The name of the class.
        pub name: String,
        /// The methods defined in this class.
        methods: HashMap<String, Rc<LoxFunc>>,
        /// The superclass of this class, if any.
        super_class: Option<Rc<LoxClass>>,
    }

    impl LoxClass {
        /// Creates a new `LoxClass` instance.
        pub fn new(
            name: String,
            methods: HashMap<String, Rc<LoxFunc>>,
            super_class: Option<Rc<LoxClass>>,
        ) -> Self {
            Self {
                name,
                methods,
                super_class,
            }
        }

        /// Finds a method with the given name in this class or its superclass.
        pub fn find_method(&self, name: &str) -> Result<Rc<LoxFunc>, AccessError> {
            if let Some(m) = self.methods.get(name) {
                Ok(m.clone())
            } else {
                self.super_class
                    .as_ref()
                    .ok_or(AccessError::NonExistentField {
                        name: name.to_owned(),
                    })?
                    .find_method(name)
            }
        }
    }

    impl LoxCallable for LoxClass {
        fn call(
            &self,
            interpreter: &mut Interpreter,
            arguments: &[LoxVar],
        ) -> Result<LoxVar, RuntimeError> {
            let instance = Rc::new(RefCell::new(LoxInstance::new(Rc::new(self.clone()))));

            if let Ok(init) = self.find_method("init") {
                init.bind(instance.clone()).call(interpreter, arguments)?;
            }

            Ok(LoxVar::Instance(instance))
        }

        fn signature(&self) -> String {
            format!("{}()", self.name)
        }

        fn arity(&self) -> u8 {
            self.find_method("init").map(|f| f.arity()).unwrap_or(0)
        }
    }

    #[derive(Debug)]
    /// Represents a Lox instance, including its class and fields.
    pub struct LoxInstance {
        class: Rc<LoxClass>,
        /// The fields of this instance, mapped by their names.
        pub fields: HashMap<String, LoxVar>,
    }

    impl LoxInstance {
        /// Creates a new `LoxInstance` instance.
        pub fn new(class: Rc<LoxClass>) -> Self {
            Self {
                class,
                fields: HashMap::new(),
            }
        }

        /// Returns a reference to the class of this instance.
        pub fn class(&self) -> &LoxClass {
            &self.class
        }
    }

    pub(crate) type InstanceRef = Rc<RefCell<LoxInstance>>;

    impl PropertyAccess for InstanceRef {
        type Key = String;

        fn get(&self, key: &Self::Key) -> Result<LoxVar, AccessError> {
            if let Some(v) = self.borrow().fields.get(key) {
                return Ok(v.clone());
            }

            self.borrow()
                .class
                .find_method(key)
                .map(|m| LoxVar::Callable(m.bind(self.clone())))
        }

        fn set(
            &mut self,
            key: Self::Key,
            value: LoxVar,
        ) -> core::result::Result<(), access_error::AccessError> {
            self.borrow_mut().fields.insert(key, value);
            Ok(())
        }
    }
}

/// Represents a Lox list.
pub mod list {
    use std::{cell::RefCell, fmt::Display, ops::Deref, rc::Rc};

    use codespan_reporting::diagnostic::Severity;

    use crate::{
        error::{
            access_error::{self, AccessError},
            runtime_err::RuntimeError,
        },
        error_codes::ErrorCode,
        tree_walk::{
            interpreter::Interpreter,
            lox_var::{LoxVar, LoxVarKind, PropertyAccess, functions::LoxCallable},
        },
    };

    #[derive(Clone, Debug, PartialEq)]
    /// Represents a Lox list.
    pub struct LoxList {
        values: Vec<LoxVar>,
    }

    impl Deref for LoxList {
        type Target = Vec<LoxVar>;

        fn deref(&self) -> &Self::Target {
            &self.values
        }
    }

    impl LoxList {
        /// Creates a new list from a vector of [`LoxVar`].
        pub fn new(list: Vec<LoxVar>) -> Self {
            Self { values: list }
        }

        /// Gets a reference to the values stored in a [`LoxList`]
        pub fn values(&self) -> &Vec<LoxVar> {
            &self.values
        }
    }

    /// Represents the kind of list access, either by index or property.
    pub enum ListAccessKind {
        /// Represents an index-based access.
        Index(LoxVar),
        /// Represents a property-based access.
        Property(String),
    }

    #[derive(Debug)]
    struct ListCall {
        list: LoxListRef,
        signature: &'static str,
        arity: u8,
        func: fn(&mut Interpreter, LoxListRef, &[LoxVar]) -> Result<LoxVar, RuntimeError>,
    }

    impl LoxCallable for ListCall {
        fn call(
            &self,
            interpreter: &mut Interpreter,
            arguments: &[LoxVar],
        ) -> Result<LoxVar, RuntimeError> {
            (self.func)(interpreter, self.list.clone(), arguments)
        }

        fn signature(&self) -> String {
            self.signature.to_owned()
        }

        fn arity(&self) -> u8 {
            self.arity
        }
    }

    pub(crate) type LoxListRef = Rc<RefCell<LoxList>>;

    impl PropertyAccess for LoxListRef {
        type Key = ListAccessKind;

        fn get(&self, key: &Self::Key) -> Result<LoxVar, access_error::AccessError> {
            match key {
                ListAccessKind::Index(index_var) => {
                    if let LoxVar::Number(idx) = *index_var {
                        let index = if idx.is_finite() && idx.fract() == 0.0 {
                            if idx >= 0.0 && idx <= usize::MAX as f64 {
                                idx as usize
                            } else {
                                return Err(AccessError::OutOfBounds {
                                    tried: idx,
                                    len: self.borrow().values.len(),
                                });
                            }
                        } else {
                            return Err(AccessError::NonIntegerIndex { tried: idx });
                        };
                        self.borrow()
                            .values
                            .get(index)
                            .ok_or(access_error::AccessError::OutOfBounds {
                                tried: index as f64,
                                len: self.borrow().values.len(),
                            })
                            .cloned()
                    } else {
                        Err(AccessError::InccorrectIndexKind {
                            tried_kind: index_var.kind(),
                        })
                    }
                }
                ListAccessKind::Property(name) => match name.as_str() {
                    "length" => Ok(LoxVar::Number(self.borrow().values.len() as f64)),
                    "push" => Ok(LoxVar::Callable(Rc::new(ListCall {
                        list: self.clone(),
                        signature: "push(Any) -> nil",
                        arity: 1,
                        func: |_, list, args| {
                            let val = args.first().ok_or(RuntimeError::InternalFunctionError {
                                msg: "Expected an argument".to_owned(),
                                code: ErrorCode::IncorrectArity,
                                sev: Severity::Error,
                            })?;
                            list.borrow_mut().values.push(val.clone());
                            Ok(LoxVar::Nil)
                        },
                    }))),
                    "pop" => Ok(LoxVar::Callable(Rc::new(ListCall {
                        list: self.clone(),
                        signature: "pop() -> Any",
                        arity: 0,
                        func: |_, list, _| {
                            let val = list.borrow_mut().values.pop();
                            if let Some(v) = val {
                                Ok(v)
                            } else {
                                Err(RuntimeError::InternalFunctionError {
                                    msg: "Cannot pop from empty list".to_owned(),
                                    code: ErrorCode::PopEmptyList,
                                    sev: Severity::Warning,
                                })
                            }
                        },
                    }))),
                    "join" => Ok(LoxVar::Callable(Rc::new(ListCall {
                        list: self.clone(),
                        signature: "join(String) -> String",
                        arity: 1,
                        func: |_, list, args| {
                            let join_val =
                                args.first().ok_or(RuntimeError::InternalFunctionError {
                                    msg: "Expected an argument".to_owned(),
                                    code: ErrorCode::IncorrectArity,
                                    sev: Severity::Error,
                                })?;
                            if let LoxVar::String(join_string) = join_val {
                                Ok(LoxVar::String(
                                    list.borrow()
                                        .values()
                                        .iter()
                                        .map(|v| v.to_string())
                                        .collect::<Vec<_>>()
                                        .join(join_string),
                                ))
                            } else {
                                Err(RuntimeError::InternalFunctionError {
                                    msg: "Expected a String argument".to_owned(),
                                    code: ErrorCode::IncorrectArgType,
                                    sev: Severity::Error,
                                })
                            }
                        },
                    }))),
                    _ => Err(AccessError::NonExistentField { name: name.clone() }),
                },
            }
        }

        fn set(&mut self, key: Self::Key, value: LoxVar) -> Result<(), access_error::AccessError> {
            match key {
                ListAccessKind::Index(index_var) => {
                    if let LoxVar::Number(idx) = index_var {
                        let index = if idx.is_finite() && idx.fract() == 0.0 {
                            if idx >= 0.0 && idx <= usize::MAX as f64 {
                                idx as usize
                            } else {
                                return Err(AccessError::OutOfBounds {
                                    tried: idx,
                                    len: self.borrow().values.len(),
                                });
                            }
                        } else {
                            return Err(AccessError::NonIntegerIndex { tried: idx });
                        };
                        let len = self.borrow().values.len();
                        *self.borrow_mut().values.get_mut(index).ok_or(
                            access_error::AccessError::OutOfBounds {
                                tried: index as f64,
                                len,
                            },
                        )? = value;
                        Ok(())
                    } else {
                        Err(AccessError::InccorrectIndexKind {
                            tried_kind: index_var.kind(),
                        })
                    }
                }
                ListAccessKind::Property(_) => Err(AccessError::CannotSetProperties {
                    kind: LoxVarKind::List,
                }),
            }
        }
    }

    impl Display for LoxList {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            write!(
                f,
                "[{}]",
                self.values
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

#[derive(Clone, Debug)]
/// Represents a Lox variable, which can be a number, boolean, string, callable, class, instance, or nil.
pub enum LoxVar {
    /// Represents a Lox number.
    Number(f64),
    /// Represents a Lox boolean.
    Bool(bool),
    /// Represents a Lox string.
    String(String),
    /// Represents a Lox callable function.
    Callable(Rc<dyn LoxCallable>),
    /// Represents a Lox class.
    Class(Rc<LoxClass>),
    /// Represents a Lox instance.
    Instance(InstanceRef),
    /// Represents a Lox list.
    List(LoxListRef),
    /// Represents a nil value.
    Nil,
    /// Represents an uninitialized variable.
    Uninitialized,
}

#[derive(Debug, Clone, Copy)]
/// Represents the kind of a Lox variable, which can be a number, boolean, string, callable, class, instance, or nil.
pub enum LoxVarKind {
    /// Represents a Lox number.
    Number,
    /// Represents a Lox boolean.
    Bool,
    /// Represents a Lox string.
    String,
    /// Represents a Lox callable function.
    Callable,
    /// Represents a Lox class.
    Class,
    /// Represents a Lox instance.
    Instance,
    /// Represents a nil value.
    Nil,
    /// Represents an uninitialized variable.
    Uninitialized,
    /// Represents a Lox list.
    List,
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
            LoxVar::Uninitialized => Self::Uninitialized,
            LoxVar::List(..) => Self::List,
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
            LoxVar::Uninitialized => Self::Uninitialized,
            LoxVar::List(..) => Self::List,
        }
    }
}

impl LoxVar {
    /// Returns the kind of this variable, which can be a number, boolean, string, callable, class, instance, or nil.
    pub fn kind(&self) -> LoxVarKind {
        self.into()
    }

    /// Returns `true` if this variable is truthy, `false` otherwise.
    pub fn is_truthy(&self) -> bool {
        match self {
            LoxVar::Nil => false,
            LoxVar::Bool(b) => *b,
            _ => true,
        }
    }

    /// Returns `true` if this variable is uninitialized, `false` otherwise.
    pub fn is_uninitialized(&self) -> bool {
        matches!(self, LoxVar::Uninitialized)
    }
}

fn unescape(input: &str) -> String {
    let mut result = String::new();
    let mut chars = input.chars();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => result.push('\n'),
                Some('t') => result.push('\t'),
                Some('r') => result.push('\r'),
                Some('\\') => result.push('\\'),
                Some('"') => result.push('"'),
                Some(other) => {
                    // unknown escape, keep as-is or handle error
                    result.push('\\');
                    result.push(other);
                }
                None => result.push('\\'),
            }
        } else {
            result.push(c);
        }
    }

    result
}

impl From<Literal> for LoxVar {
    fn from(value: Literal) -> Self {
        match value {
            Literal::Number(n) => Self::Number(n),
            Literal::String(s) => Self::String(unescape(&s)),
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
                write!(f, "<{} instance>", i.borrow().class().name)
            }
            LoxVar::Uninitialized => write!(f, "<uninitialized>"),
            LoxVar::List(lox_list) => write!(f, "{}", lox_list.borrow()),
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

            (Self::List(l), Self::List(r)) => l.borrow().values() == r.borrow().values(),

            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// The reason a binary operation error has occurred
pub enum BinErrReason {
    /// The operands were incompatible
    Incompatible,
    /// A non-negative integer was expected but not recieved
    NotPositiveInteger,
}

type BinOut<'err> = Result<LoxVar, (&'err LoxVar, &'err LoxVar, BinErrReason)>;

macro_rules! bin_num_op_impl {
    ($trait:ident, $fn:ident) => {
        impl<'input> $trait for &'input LoxVar {
            type Output = BinOut<'input>;

            fn $fn(self, rhs: Self) -> Self::Output {
                match (self, rhs) {
                    (LoxVar::Number(ln), LoxVar::Number(rn)) => Ok(LoxVar::Number(ln.$fn(rn))),
                    (l, r) => Err((l, r, BinErrReason::Incompatible)),
                }
            }
        }
    };
}

bin_num_op_impl!(Rem, rem);
bin_num_op_impl!(Sub, sub);
bin_num_op_impl!(Div, div);

impl<'input> Add for &'input LoxVar {
    type Output = BinOut<'input>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (LoxVar::Number(ln), LoxVar::Number(rn)) => Ok(LoxVar::Number(ln + rn)),
            (LoxVar::String(ls), LoxVar::String(rs)) => Ok(LoxVar::String(ls.to_owned() + rs)),
            (LoxVar::List(ll), LoxVar::List(rl)) => {
                Ok(LoxVar::List(Rc::new(RefCell::new(LoxList::new(
                    ll.borrow()
                        .values()
                        .iter()
                        .chain(rl.borrow().values())
                        .cloned()
                        .collect::<Vec<_>>(),
                )))))
            }
            (l, r) => Err((l, r, BinErrReason::Incompatible)),
        }
    }
}
impl<'input> Mul for &'input LoxVar {
    type Output = BinOut<'input>;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (LoxVar::Number(ln), LoxVar::Number(rn)) => Ok(LoxVar::Number(ln * rn)),
            (LoxVar::String(s), LoxVar::Number(n)) => Ok(LoxVar::String(s.to_owned().repeat(
                if n.is_sign_positive() && n.is_finite() && n.fract() == 0.0 {
                    *n as usize
                } else {
                    return Err((self, rhs, BinErrReason::NotPositiveInteger));
                },
            ))),
            (LoxVar::Number(n), LoxVar::String(s)) => Ok(LoxVar::String(s.to_owned().repeat(
                if n.is_sign_positive() && n.is_finite() && n.fract() == 0.0 {
                    *n as usize
                } else {
                    return Err((self, rhs, BinErrReason::NotPositiveInteger));
                },
            ))),
            (LoxVar::List(l), LoxVar::Number(n)) => {
                Ok(LoxVar::List(Rc::new(RefCell::new(LoxList::new(
                    match if n.is_sign_positive() && n.is_finite() && n.fract() == 0.0 {
                        *n as usize
                    } else {
                        return Err((self, rhs, BinErrReason::NotPositiveInteger));
                    } {
                        0 => Vec::<LoxVar>::new(),
                        r => {
                            let list = l.borrow().values().clone();

                            let mut resultant_list = Vec::new();

                            for _ in 0..r {
                                resultant_list.push(list.clone());
                            }

                            resultant_list
                                .into_iter()
                                .flatten()
                                .collect::<Vec<LoxVar>>()
                        }
                    },
                )))))
            }
            (l, r) => Err((l, r, BinErrReason::Incompatible)),
        }
    }
}

impl Neg for &LoxVar {
    type Output = Result<LoxVar, Self>;

    fn neg(self) -> Self::Output {
        match self {
            LoxVar::Number(n) => Ok(LoxVar::Number(-n)),
            l => Err(l),
        }
    }
}

impl PartialOrd for LoxVar {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        match (self, other) {
            (LoxVar::Number(l), LoxVar::Number(r)) => l.partial_cmp(r),
            (LoxVar::String(l), LoxVar::String(r)) => l.partial_cmp(r),
            (_ls, _rs) => None,
        }
    }
}
