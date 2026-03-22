use std::{
    collections::HashMap,
    fmt::Display,
    ops::{Add, Div, Mul, Sub},
    rc::Rc,
};

use crate::tree_walk::{
    error::{
        ErrorPos,
        eval_err::{EvaluateError, EvaluateErrorKind},
        runtime_err::{RuntimeError, RuntimeErrorKind},
    },
    interpreter::env::{Env, EnvironmentImpl, ScopeGuard},
    lox_var::{
        LoxVar,
        class::{InstanceImpl, LoxClass},
        functions::{LoxCallable, LoxFunc},
    },
    token::{Token, TokenKind},
};

#[derive(Debug, Clone)]
pub enum Stmt {
    Expression(Expr),
    Print(Expr),
    Var {
        name: String,
        initializer: Option<Expr>,
        position: ErrorPos,
    },
    Block(Vec<Stmt>),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    Function {
        name: String,
        parameters: Vec<String>,
        body: Vec<Stmt>,
        pos: ErrorPos,
    },
    Return {
        value: Option<Expr>,
        pos: ErrorPos,
    },
    Class {
        name: String,
        super_class: Option<Expr>,
        methods: Vec<Stmt>,
        pos: ErrorPos,
    },
}

#[derive(Debug, Clone)]
pub enum ControlFlow {
    None,
    Return(LoxVar),
}

impl Stmt {
    pub fn run(&self, env: &mut Env) -> Result<ControlFlow, RuntimeError> {
        match self {
            Stmt::Expression(expr) => {
                expr.evaluate(env)?;
                Ok(ControlFlow::None)
            }
            Stmt::Print(expr) => {
                let value = expr.evaluate(env)?;
                println!("{value}");
                Ok(ControlFlow::None)
            }
            Stmt::Var {
                name, initializer, ..
            } => {
                let value = match initializer {
                    Some(value) => value.evaluate(env)?,
                    None => LoxVar::Nil,
                };
                env.borrow_mut().define(name.to_string(), value.clone());
                Ok(ControlFlow::None)
            }
            Stmt::Block(stmts) => {
                let mut scope = ScopeGuard::new(env);

                for stmt in stmts {
                    match stmt.run(&mut scope)? {
                        ControlFlow::None => (),
                        ControlFlow::Return(lox_var) => return Ok(ControlFlow::Return(lox_var)),
                    };
                }

                Ok(ControlFlow::None)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if condition.evaluate(env)?.is_truthy() {
                    then_branch.run(env)
                } else if let Some(branch) = else_branch {
                    branch.run(env)
                } else {
                    Ok(ControlFlow::None)
                }
            }
            Stmt::While { condition, body } => {
                while condition.evaluate(env)?.is_truthy() {
                    match body.run(env)? {
                        ControlFlow::None => (),
                        ControlFlow::Return(lox_var) => return Ok(ControlFlow::Return(lox_var)),
                    };
                }
                Ok(ControlFlow::None)
            }
            Stmt::Function {
                name,
                parameters,
                body,
                pos,
            } => {
                env.borrow_mut().define(
                    name.to_string(),
                    LoxVar::Callable(Rc::new(LoxFunc::new(
                        parameters.len().try_into().map_err(|_| {
                            RuntimeError::new(RuntimeErrorKind::TooManyArguments(*pos))
                        })?,
                        body.clone(),
                        parameters.clone(),
                        name.to_string(),
                        env,
                        false,
                    ))),
                );
                Ok(ControlFlow::None)
            }
            Stmt::Return { value, .. } => Ok(ControlFlow::Return(
                value
                    .as_ref()
                    .unwrap_or(&Expr::Literal(Literal::Nil))
                    .evaluate(env)?
                    .clone(),
            )),
            Stmt::Class {
                name,
                methods,
                super_class,
                pos,
            } => {
                env.borrow_mut().define(name.clone(), LoxVar::Nil);

                let super_class = if let Some(expr) = super_class {
                    let var = expr.evaluate(env)?;

                    if let LoxVar::Class(s_class) = var {
                        Some(s_class)
                    } else {
                        return Err(RuntimeError::new(RuntimeErrorKind::SuperClassIsNotAClass(
                            var,
                            if let Expr::Variable { position, .. } = expr {
                                *position
                            } else {
                                *pos
                            },
                        )));
                    }
                } else {
                    None
                };

                let mut method_env = env.clone();

                if let Some(ref super_class) = super_class {
                    let new_env = Env::new_env(Some(env.clone()));
                    new_env
                        .borrow_mut()
                        .define("super".to_string(), LoxVar::Class(super_class.clone()));

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
                            body.clone(),
                            parameters.clone(),
                            m_name.clone(),
                            &method_env,
                            m_name == "init",
                        );

                        method_map.insert(m_name.clone(), Rc::new(func));
                    }
                }

                let class = LoxClass {
                    name: name.clone(),
                    methods: method_map,
                    super_class,
                };

                env.borrow_mut()
                    .assign(name, &LoxVar::Class(Rc::new(class)), None)
                    .unwrap();

                Ok(ControlFlow::None)
            }
        }
    }
}

impl Display for Stmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stmt::Expression(expr) => write!(f, "{expr}"),
            Stmt::Print(expr) => write!(f, "(print {expr})"),
            Stmt::Var {
                name, initializer, ..
            } => write!(f, "(var {} = {initializer:?})", name),
            Stmt::Block(stmts) => write!(
                f,
                "{{{}}}",
                stmts
                    .iter()
                    .map(|stmt| stmt.to_string())
                    .collect::<String>()
            ),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if let Some(else_branch) = else_branch {
                    write!(f, "(if {condition} then {then_branch} else {else_branch})")
                } else {
                    write!(f, "(if {condition} then {then_branch})")
                }
            }
            Stmt::While { condition, body } => write!(f, "(while {condition} do {body})"),
            Stmt::Function {
                name,
                parameters,
                body,
                ..
            } => {
                write!(f, "(fun {name}(")?;
                if !parameters.is_empty() {
                    write!(f, "{}", parameters[0])?;

                    if parameters.len() >= 2 {
                        for arg in parameters.iter().skip(1) {
                            write!(f, ", {arg}")?;
                        }
                    }
                }
                write!(
                    f,
                    ") {{{}}})",
                    body.iter().map(|stmt| stmt.to_string()).collect::<String>()
                )
            }
            Stmt::Return { value, .. } => {
                write!(f, "(return")?;
                if let Some(val) = value {
                    write!(f, "{val}")?
                };
                write!(f, ")")
            }
            Stmt::Class {
                name,
                methods,
                super_class,
                ..
            } => {
                write!(f, "(class {name}")?;
                if let Some(class) = super_class {
                    write!(f, " < {class}")?;
                }
                if let Some(m) = methods.get(0) {
                    write!(f, " ({m}")?;
                }
                for m in methods.iter().skip(1) {
                    write!(f, "{m}")?;
                }
                write!(f, "))")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),

    Unary {
        operator: TokenKind,
        position: ErrorPos,
        right: Box<Expr>,
    },

    Binary {
        left: Box<Expr>,
        operator: TokenKind,
        right: Box<Expr>,
        position: ErrorPos,
    },

    Grouping(Box<Expr>),

    Variable {
        name: String,
        distance: Option<usize>,
        position: ErrorPos,
    },

    Assign {
        name: String,
        distance: Option<usize>,
        position: ErrorPos,
        value: Box<Expr>,
    },

    Logical {
        left: Box<Expr>,
        operator: TokenKind,
        right: Box<Expr>,
    },

    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        position: ErrorPos,
    },

    Get {
        object: Box<Expr>,
        name: String,
        pos: ErrorPos,
    },

    Set {
        object: Box<Expr>,
        name: String,
        value: Box<Expr>,
        pos: ErrorPos,
    },

    Super {
        method_name: String,
        distance: Option<usize>,
        pos: ErrorPos,
    },

    This {
        distance: Option<usize>,
        pos: ErrorPos,
    },
}

impl Expr {
    pub fn resolve(&mut self, dist: usize) {
        match self {
            Expr::Variable { distance, .. }
            | Expr::Assign { distance, .. }
            | Expr::This { distance, .. }
            | Expr::Super { distance, .. } => *distance = Some(dist),
            _ => {}
        }
    }

    pub fn binary(left: Expr, operator: Token<'_>, right: Expr) -> Self {
        Self::Binary {
            left: Box::new(left),
            operator: operator.kind,
            right: Box::new(right),
            position: operator.into(),
        }
    }

    pub fn unary(operator: Token<'_>, right: Expr) -> Self {
        Self::Unary {
            operator: operator.kind,
            right: Box::new(right),
            position: operator.into(),
        }
    }

    pub fn string(&self) -> String {
        match self {
            Expr::Literal(literal) => literal.to_string(),
            Expr::Unary {
                operator, right, ..
            } => operator.to_string() + &right.string(),
            Expr::Binary {
                left,
                operator,
                right,
                ..
            } => left.string() + " " + &operator.to_string() + " " + &right.string(),
            Expr::Grouping(expr) => "(".to_string() + &expr.string() + ")",
            Expr::Variable { name, .. } => name.to_string(),
            Expr::Assign { name, value, .. } => name.to_string() + " = " + &value.string(),
            Expr::Logical {
                left,
                operator,
                right,
                ..
            } => left.string() + " " + &operator.to_string() + " " + &right.string(),
            Expr::Call { callee, args, .. } => {
                let mut base_string = callee.string() + "(";

                for arg in args {
                    base_string.push_str(&arg.string());
                    base_string.push_str(", ");
                }

                base_string.push(')');

                base_string
            }
            Expr::Get { object, name, .. } => object.string() + "." + name,
            Expr::Set {
                object,
                name,
                value,
                ..
            } => object.string() + "." + name + " = " + &value.to_string(),
            Expr::This { .. } => "this".to_owned(),
            Expr::Super { method_name, .. } => "super.".to_owned() + method_name,
        }
    }

    pub fn evaluate(&self, env: &mut Env) -> Result<LoxVar, RuntimeError> {
        match self {
            Expr::Literal(literal) => Ok(literal.into()),
            Expr::Unary {
                operator,
                right,
                position,
            } => {
                let right_var = right.evaluate(env)?;
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
                                *position,
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
                position,
            } => {
                let left_var = left.evaluate(env)?;
                let right_var = right.evaluate(env)?;

                use TokenKind::*;
                use std::cmp::Ordering;

                match operator {
                    Greater => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Greater))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                *position, Greater, &left_var, &right_var,
                            )
                            .into())
                        }
                    }
                    GreaterEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Less))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                *position,
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
                            Err(EvaluateError::bin_eval_error(
                                *position, Less, &left_var, &right_var,
                            )
                            .into())
                        }
                    }
                    LessEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Greater))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                *position, LessEqual, &left_var, &right_var,
                            )
                            .into())
                        }
                    }
                    EqualEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Equal))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                *position, EqualEqual, &left_var, &right_var,
                            )
                            .into())
                        }
                    }
                    BangEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Equal))
                        } else {
                            Err(EvaluateError::bin_eval_error(
                                *position, BangEqual, &left_var, &right_var,
                            )
                            .into())
                        }
                    }
                    Minus => left_var.sub(&right_var).map_err(|(l, r)| {
                        EvaluateError::bin_eval_error(*position, Minus, l, r).into()
                    }),
                    Slash => {
                        if right_var == LoxVar::Number(0.) {
                            Err(
                                EvaluateError::new(EvaluateErrorKind::DivisionByZero, *position)
                                    .into(),
                            )
                        } else {
                            left_var.div(&right_var).map_err(|(l, r)| {
                                EvaluateError::bin_eval_error(*position, Slash, l, r).into()
                            })
                        }
                    }
                    Star => left_var.mul(&right_var).map_err(|(l, r)| {
                        EvaluateError::bin_eval_error(*position, Star, l, r).into()
                    }),
                    Plus => left_var.add(&right_var).map_err(|(l, r)| {
                        EvaluateError::bin_eval_error(*position, Plus, l, r).into()
                    }),
                    _ => unreachable!(
                        "only tokens with kind gt, ge, lt, le, ne, eq, minus, plus, slash and star can be binary operators"
                    ),
                }
            }
            Expr::Grouping(expr) => expr.evaluate(env),
            Expr::Variable {
                name,
                position,
                distance,
            } => env.borrow().get(name, *distance).ok_or(
                EvaluateError::new(
                    EvaluateErrorKind::UndeclaredVariable {
                        name: name.to_owned(),
                    },
                    *position,
                )
                .into(),
            ),
            Expr::Assign {
                name,
                position,
                value,
                distance,
            } => {
                let value = value.evaluate(env)?;
                env.borrow_mut()
                    .assign(name, &value, *distance)
                    .map_err(|err| EvaluateError::new(err, *position))?;
                Ok(value)
            }
            Expr::Logical {
                left,
                operator,
                right,
                ..
            } => {
                let left = left.evaluate(env)?;
                match operator {
                    TokenKind::And => {
                        if !left.is_truthy() {
                            Ok(left)
                        } else {
                            right.evaluate(env)
                        }
                    }
                    TokenKind::Or => {
                        if left.is_truthy() {
                            Ok(left)
                        } else {
                            right.evaluate(env)
                        }
                    }
                    _ => unreachable!("Logical expr can only have operators Or or And"),
                }
            }
            Expr::Call {
                callee,
                args,
                position,
            } => {
                let arguments: Vec<LoxVar> = args
                    .iter()
                    .map(|expr| expr.evaluate(env))
                    .collect::<Result<_, _>>()?;
                match callee.evaluate(env)? {
                    LoxVar::Callable(func) => Ok(func.call(&arguments).map_err(|mut err| {
                        err.call_pos = Some(Box::new(*position));
                        err.input_args = arguments;

                        err
                    })?),
                    LoxVar::Class(class) => Ok(class.call(&arguments).map_err(|mut err| {
                        err.call_pos = Some(Box::new(*position));
                        err.input_args = arguments;

                        err
                    })?),
                    other => Err(EvaluateError::new(
                        EvaluateErrorKind::NonCallable {
                            name: other.to_string(),
                        },
                        *position,
                    )
                    .into()),
                }
            }
            Expr::Get { object, name, pos } => {
                let obj = object.evaluate(env)?;

                if let LoxVar::Instance(inst) = obj {
                    if let Some(property) = inst.get(name) {
                        Ok(property)
                    } else if let Some(method) = inst.borrow().class.find_method(name) {
                        Ok(LoxVar::Callable(method))
                    } else {
                        Err(EvaluateError::new(
                            EvaluateErrorKind::UndefinedProperty(name.to_owned()),
                            *pos,
                        )
                        .into())
                    }
                } else {
                    Err(EvaluateError::new(EvaluateErrorKind::NotInstance(obj.into()), *pos).into())
                }
            }
            Expr::Set {
                object,
                name,
                value,
                pos,
            } => {
                let obj = object.evaluate(env)?;

                if let LoxVar::Instance(mut inst) = obj {
                    let val = value.evaluate(env)?;

                    inst.set(name.clone(), val.clone());

                    Ok(val)
                } else {
                    Err(EvaluateError::new(EvaluateErrorKind::NotInstance(obj.into()), *pos).into())
                }
            }
            Expr::This { distance, .. } => Ok(env
                .borrow()
                .get("this", *distance)
                .expect("Cannot have a reference to this outside of method")),
            Expr::Super {
                distance,
                method_name,
                pos,
            } => {
                let dist = distance.expect("super must be resolved");

                let superclass = match env.borrow().get("super", Some(dist)) {
                    Some(LoxVar::Class(cls)) => cls,
                    _ => unreachable!("super must be a class"),
                };

                let instance = match env.borrow().get("this", Some(dist - 1)) {
                    Some(LoxVar::Instance(inst)) => inst,
                    _ => unreachable!("this must be an instance"),
                };

                let method = superclass
                    .find_method(method_name)
                    .ok_or(EvaluateError::new(
                        EvaluateErrorKind::UndefinedProperty(method_name.to_owned()),
                        *pos,
                    ))?;

                Ok(LoxVar::Callable(method.bind(instance)))
            }
        }
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Literal(literal) => write!(f, "{literal}"),
            Expr::Unary {
                operator, right, ..
            } => write!(f, "({operator} {right})"),
            Expr::Binary {
                left,
                operator,
                right,
                ..
            } => write!(f, "({operator} {left} {right})"),
            Expr::Grouping(expr) => write!(f, "(group {expr})"),
            Expr::Variable { name, .. } => write!(f, "(variable {name})"),
            Expr::Assign { name, value, .. } => write!(f, "(assign {name} {value})"),
            Expr::Logical {
                left,
                operator,
                right,
                ..
            } => write!(f, "({operator} {left} {right})"),
            Expr::Call { callee, args, .. } => {
                write!(f, "(fun {callee} (")?;
                if !args.is_empty() {
                    write!(f, "{}", args[0])?;
                    for arg in args.iter().skip(1) {
                        write!(f, ", {arg}")?;
                    }
                }

                write!(f, "))")?;
                Ok(())
            }
            Expr::Get { object, name, .. } => write!(f, "(get ({object} {name}))"),
            Expr::Set {
                object,
                name,
                value,
                ..
            } => write!(f, "(set ({object} {name}) {value})"),
            Expr::This { .. } => write!(f, "(this)"),
            Expr::Super { .. } => write!(f, "super"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Literal {
    Number(f64),
    String(String),
    True,
    False,
    Nil,
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Literal::Number(n) => write!(f, "{n}"),
            Literal::String(s) => write!(f, "\"{s}\""),
            Literal::True => write!(f, "true"),
            Literal::False => write!(f, "false"),
            Literal::Nil => write!(f, "nil"),
        }
    }
}
