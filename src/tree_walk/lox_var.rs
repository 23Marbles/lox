use std::{cell::RefCell, collections::HashMap, fmt::Display, ops::*, rc::Rc};

use crate::tree_walk::ast::Literal;

pub trait EnvironmentImpl {
    fn new(enclosing: Option<Env>) -> Env;
    fn descend(&mut self);
    #[must_use = "Make sure you are not ascending above global level"]
    fn ascend(&mut self) -> Option<()>;
}

pub type Env = Rc<RefCell<Environment>>;

impl EnvironmentImpl for Env {
    fn new(enclosing: Option<Env>) -> Env {
        Rc::new(RefCell::new(Environment::new(enclosing)))
    }

    fn descend(&mut self) {
        *self = <Env as EnvironmentImpl>::new(Some(self.clone()))
    }

    fn ascend(&mut self) -> Option<()> {
        let parent = self.borrow().enclosing.clone()?;
        *self = Env::new(RefCell::new(Environment {
            enclosing: parent.borrow().enclosing.clone(),
            values: parent.borrow().values.clone(),
        }));
        Some(())
    }
}

#[derive(Debug, Clone)]
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
                .map(|parent| parent.borrow().get(name))
                .flatten()
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

#[derive(Debug, Clone)]
pub enum LoxVar {
    Nil,
    Bool(bool),
    Number(f64),
    String(String),
}

#[derive(Debug, Clone, Copy)]
pub enum LoxVarKinds {
    Nil,
    Bool,
    Number,
    String,
}

impl From<&LoxVar> for LoxVarKinds {
    fn from(value: &LoxVar) -> Self {
        match value {
            LoxVar::Nil => Self::Nil,
            LoxVar::Bool(_) => Self::Bool,
            LoxVar::Number(_) => Self::Number,
            LoxVar::String(_) => Self::String,
        }
    }
}

impl LoxVar {
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

    #[cfg(feature = "challenges")]
    pub fn try_convert(&self, to: LoxVarKinds) -> Option<LoxVar> {
        match to {
            LoxVarKinds::Nil => Some(LoxVar::Nil),
            LoxVarKinds::Bool => Some(LoxVar::Bool(self.is_truthy())),
            LoxVarKinds::Number => match self {
                LoxVar::Nil => None,
                LoxVar::Bool(b) => Some(LoxVar::Number(*b as u8 as f64)),
                LoxVar::Number(n) => Some(LoxVar::Number(*n)),
                LoxVar::String(s) => Some(LoxVar::Number(s.parse().ok()?)),
            },
            LoxVarKinds::String => Some(LoxVar::String(self.to_string())),
        }
    }
}

impl From<&Literal<'_>> for LoxVar {
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
            (LoxVar::String(ls), LoxVar::String(rs)) => Ok(LoxVar::String(ls.to_owned() + &rs)),
            #[cfg(feature = "challenges")]
            (LoxVar::String(s), other) => Ok(LoxVar::String(s.to_owned() + &other.to_string())),
            #[cfg(feature = "challenges")]
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
                #[cfg(feature = "challenges")]
                {
                    _ls.try_convert(LoxVarKinds::Number)?
                        .partial_cmp(&_rs.try_convert(LoxVarKinds::Number)?)
                }
                #[cfg(not(feature = "challenges"))]
                {
                    None
                }
            }
        }
    }
}
