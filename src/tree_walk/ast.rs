use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Sub},
};

use crate::tree_walk::{
    error::eval_err::{EvaluateError, EvaluateErrorKind, make_eval_error},
    lox_var::LoxVar,
    token::{Token, TokenKind},
};

#[derive(Debug)]
pub enum Expr<'a> {
    Literal(Literal<'a>),

    Unary {
        operator: Token<'a>,
        right: Box<Expr<'a>>,
    },

    Binary {
        left: Box<Expr<'a>>,
        operator: Token<'a>,
        right: Box<Expr<'a>>,
    },

    Grouping(Box<Expr<'a>>),
}

impl<'a> Expr<'a> {
    pub fn binary(left: Expr<'a>, operator: Token<'a>, right: Expr<'a>) -> Self {
        Self::Binary {
            left: Box::new(left),
            operator,
            right: Box::new(right),
        }
    }

    pub fn unary(operator: Token<'a>, right: Expr<'a>) -> Self {
        Self::Unary {
            operator,
            right: Box::new(right),
        }
    }

    pub fn evaluate(&self) -> Result<LoxVar, EvaluateError> {
        match self {
            Expr::Literal(literal) => Ok(literal.into()),
            Expr::Unary { operator, right } => {
                let right_var = right.evaluate()?;
                match operator.kind {
                    TokenKind::Minus => {
                        if let LoxVar::Number(n) = right_var {
                            Ok(LoxVar::Number(-n))
                        } else {
                            Err(EvaluateError::new(
                                EvaluateErrorKind::IncompatableUnaryOperator {
                                    operator: TokenKind::Minus,
                                    var_ty: right_var.into(),
                                },
                                operator.line,
                                operator.column,
                            ))
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
            } => {
                let left_var = left.evaluate()?;
                let right_var = right.evaluate()?;

                use TokenKind::*;
                use std::cmp::Ordering;

                match operator.kind {
                    Greater => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Greater))
                        } else {
                            Err(make_eval_error(operator, Greater, left_var, right_var))
                        }
                    }
                    GreaterEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Less))
                        } else {
                            Err(make_eval_error(operator, GreaterEqual, left_var, right_var))
                        }
                    }
                    Less => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Less))
                        } else {
                            Err(make_eval_error(operator, Less, left_var, right_var))
                        }
                    }
                    LessEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Greater))
                        } else {
                            Err(make_eval_error(operator, LessEqual, left_var, right_var))
                        }
                    }
                    EqualEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord == Ordering::Equal))
                        } else {
                            Err(make_eval_error(operator, EqualEqual, left_var, right_var))
                        }
                    }
                    BangEqual => {
                        if let Some(ord) = left_var.partial_cmp(&right_var) {
                            Ok(LoxVar::Bool(ord != Ordering::Equal))
                        } else {
                            Err(make_eval_error(operator, BangEqual, left_var, right_var))
                        }
                    }
                    Minus => left_var
                        .sub(right_var)
                        .map_err(|(l, r)| make_eval_error(operator, Minus, l, r)),
                    Slash => left_var
                        .div(right_var)
                        .map_err(|(l, r)| make_eval_error(operator, Slash, l, r)),
                    Star => left_var
                        .mul(right_var)
                        .map_err(|(l, r)| make_eval_error(operator, Star, l, r)),
                    Plus => left_var
                        .add(right_var)
                        .map_err(|(l, r)| make_eval_error(operator, Plus, l, r)),
                    _ => unreachable!(
                        "only tokens with kind gt, ge, lt, le, ne, eq, minus, plus, slash and star can be binary operators"
                    ),
                }
            }
            Expr::Grouping(expr) => {
                todo!()
            }
        }
    }
}

impl Display for Expr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Literal(literal) => write!(f, "{literal}"),
            Expr::Unary { operator, right } => write!(f, "({operator} {right})"),
            Expr::Binary {
                left,
                operator,
                right,
            } => write!(f, "({operator} {left} {right})"),
            Expr::Grouping(expr) => write!(f, "(group {expr})"),
        }
    }
}

#[derive(Debug)]
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
            Literal::String(s) => write!(f, "\n{s}\n"),
            Literal::True => write!(f, "true"),
            Literal::False => write!(f, "false"),
            Literal::Nil => write!(f, "nil"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn expr_print() {
        let expression = Expr::Binary {
            left: Box::new(Expr::Literal(Literal::False)),
            operator: Token {
                kind: crate::tree_walk::token::TokenKind::EqualEqual,
                lexeme: "==",
                line: 2,
                column: 1,
            },
            right: Box::new(Expr::Unary {
                operator: Token {
                    kind: crate::tree_walk::token::TokenKind::Bang,
                    lexeme: "!",
                    line: 4,
                    column: 7,
                },
                right: Box::new(Expr::Literal(Literal::True)),
            }),
        };
        assert_eq!(expression.to_string(), "(== false (! true))")
    }
}
