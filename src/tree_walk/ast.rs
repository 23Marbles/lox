use std::fmt::{Display, write};

use crate::tree_walk::token::Token;

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
