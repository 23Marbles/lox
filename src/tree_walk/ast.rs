use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Sub},
    rc::Rc,
};

use crate::tree_walk::{
    error::{
        ErrorPos,
        eval_err::{EvaluateError, EvaluateErrorKind, make_bin_eval_error},
        runtime_err::{RuntimeError, RuntimeErrorKind},
    },
    lox_var::{Env, EnvironmentImpl, LoxFunc, LoxVar, LoxVarKind},
    token::{Token, TokenKind},
};

#[derive(Debug, Clone)]
pub enum Stmt<'a> {
    Expression(Expr<'a>),
    Print(Expr<'a>),
    Var {
        name: &'a str,
        initializer: Option<Expr<'a>>,
    },
    Block(Vec<Stmt<'a>>),
    If {
        condition: Expr<'a>,
        then_branch: Box<Stmt<'a>>,
        else_branch: Option<Box<Stmt<'a>>>,
    },
    While {
        condition: Expr<'a>,
        body: Box<Stmt<'a>>,
    },
    Function {
        name: &'a str,
        parameters: Vec<&'a str>,
        body: Vec<Stmt<'a>>,
    },
    Return {
        value: Expr<'a>,
    },
}

impl<'a> Stmt<'a> {
    pub fn run(self, env: &mut Env) -> Result<Option<LoxVar>, RuntimeError> {
        match self {
            Stmt::Expression(expr) => Ok(Some(expr.evaluate(env)?)),
            Stmt::Print(expr) => {
                let value = expr.evaluate(env)?;
                println!("{value}");
                Ok(None)
            }
            Stmt::Var { name, initializer } => {
                let value = match initializer {
                    Some(value) => value.evaluate(env)?,
                    None => LoxVar::Nil,
                };
                env.borrow_mut().define(name.to_string(), value.clone());
                Ok(None)
            }
            Stmt::Block(stmts) => {
                env.descend();
                for stmt in stmts {
                    stmt.run(env)?;
                }
                env.ascend().expect("Has descended");
                Ok(None)
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if condition.evaluate(env)?.is_truthy() {
                    then_branch.run(env)?;
                } else if let Some(branch) = else_branch {
                    branch.run(env)?;
                }
                Ok(None)
            }
            Stmt::While { condition, body } => {
                while condition.evaluate(env)?.is_truthy() {
                    body.clone().run(env)?;
                }
                Ok(None)
            }
            Stmt::Function {
                name,
                parameters,
                body,
            } => {
                env.borrow_mut().define(name.to_string(), LoxVar::Callable(
                    Rc::new(
                        LoxFunc::new(parameters.len(), body, parameters)
                    )
                ));
                Ok(None)
            }
            Stmt::Return { value } => Ok(Some(value.evaluate(env)?)),
        }
    }
}

impl Display for Stmt<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stmt::Expression(expr) => write!(f, "{expr}"),
            Stmt::Print(expr) => write!(f, "(print {expr})"),
            Stmt::Var { name, initializer } => write!(f, "(var {} = {initializer:?})", name),
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
            Stmt::Return { value } => write!(f, "(return {value})"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Expr<'a> {
    Literal(Literal<'a>),

    Unary {
        operator: TokenKind,
        position: ErrorPos,
        right: Box<Expr<'a>>,
    },

    Binary {
        left: Box<Expr<'a>>,
        operator: TokenKind,
        right: Box<Expr<'a>>,
        position: ErrorPos,
    },

    Grouping(Box<Expr<'a>>),

    Variable {
        name: &'a str,
        position: ErrorPos,
    },

    Assign {
        name: &'a str,
        position: ErrorPos,
        value: Box<Expr<'a>>,
    },

    Logical {
        left: Box<Expr<'a>>,
        operator: TokenKind,
        right: Box<Expr<'a>>,
        _position: ErrorPos,
    },

    Call {
        callee: Box<Expr<'a>>,
        args: Vec<Expr<'a>>,
        position: ErrorPos,
    },
}

impl<'a> Expr<'a> {
    pub fn binary(left: Expr<'a>, operator: Token<'a>, right: Expr<'a>) -> Self {
        Self::Binary {
            left: Box::new(left),
            operator: operator.kind,
            right: Box::new(right),
            position: (&operator).into(),
        }
    }

    pub fn unary(operator: Token<'a>, right: Expr<'a>) -> Self {
        Self::Unary {
            operator: operator.kind,
            right: Box::new(right),
            position: (&operator).into(),
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
                                    var_ty: (&right_var).into(),
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
                            Err(
                                make_bin_eval_error(*position, Greater, &left_var, &right_var)
                                    .into(),
                            )
                        }
                    }
                    GreaterEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Less))
                        } else {
                            Err(
                                make_bin_eval_error(*position, GreaterEqual, &left_var, &right_var)
                                    .into(),
                            )
                        }
                    }
                    Less => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Less))
                        } else {
                            Err(make_bin_eval_error(*position, Less, &left_var, &right_var).into())
                        }
                    }
                    LessEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Greater))
                        } else {
                            Err(
                                make_bin_eval_error(*position, LessEqual, &left_var, &right_var)
                                    .into(),
                            )
                        }
                    }
                    EqualEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Equal))
                        } else {
                            Err(
                                make_bin_eval_error(*position, EqualEqual, &left_var, &right_var)
                                    .into(),
                            )
                        }
                    }
                    BangEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Equal))
                        } else {
                            Err(
                                make_bin_eval_error(*position, BangEqual, &left_var, &right_var)
                                    .into(),
                            )
                        }
                    }
                    Minus => left_var
                        .sub(&right_var)
                        .map_err(|(l, r)| make_bin_eval_error(*position, Minus, l, r).into()),
                    Slash => left_var
                        .div(&right_var)
                        .map_err(|(l, r)| make_bin_eval_error(*position, Slash, l, r).into()),
                    Star => left_var
                        .mul(&right_var)
                        .map_err(|(l, r)| make_bin_eval_error(*position, Star, l, r).into()),
                    Plus => left_var
                        .add(&right_var)
                        .map_err(|(l, r)| make_bin_eval_error(*position, Plus, l, r).into()),
                    _ => unreachable!(
                        "only tokens with kind gt, ge, lt, le, ne, eq, minus, plus, slash and star can be binary operators"
                    ),
                }
            }
            Expr::Grouping(expr) => expr.evaluate(env),
            Expr::Variable { name, position } => env.borrow().get(name.to_string()).ok_or(
                EvaluateError::new(
                    EvaluateErrorKind::UndeclaredVariable {
                        name: name.to_string(),
                    },
                    *position,
                )
                .into(),
            ),
            Expr::Assign {
                name,
                position,
                value,
            } => {
                let value = value.evaluate(env)?;
                if !env.borrow_mut().assign(name, value.clone()) {
                    Err(EvaluateError::new(
                        EvaluateErrorKind::UndeclaredVariable {
                            name: name.to_string(),
                        },
                        *position,
                    )
                    .into())
                } else {
                    Ok(value)
                }
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
            } => match callee.evaluate(env)? {
                LoxVar::Callable(func) => {
                    let arguments: Vec<LoxVar> =
                        args.iter().map(|expr| expr.evaluate(env)).collect()?;

                    Ok(func.call(env, arguments).map_err(|mut err| {
                        err.call_pos = Some(*position);
                        err.input_args = arguments;

                        err
                    })?)
                }
                other => Err(EvaluateError::new(
                    EvaluateErrorKind::NonCallable {
                        name: other.to_string(),
                    },
                    *position,
                )
                .into()),
            },
        }
    }
}

impl Display for Expr<'_> {
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
        }
    }
}

#[derive(Debug, Clone)]
pub enum Literal<'a> {
    Number(f64),
    String(&'a str),
    True,
    False,
    Nil,
}

impl Display for Literal<'_> {
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
