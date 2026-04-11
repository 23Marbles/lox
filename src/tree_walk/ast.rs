use core::fmt::Display;

use crate::tree_walk::{
    ast::expr::Expr,
    lox_var::LoxVar,
    token::{Token, TokenKind},
};

pub mod stmt {
    use core::ops::Range;

    use super::*;
    #[derive(Debug, Clone, PartialEq)]
    pub enum Stmt {
        Expression(Expr),
        Print(Expr),
        Var {
            name: String,
            initializer: Option<Expr>,
            span: Range<usize>,
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
            span: Range<usize>,
        },
        Return {
            value: Option<Expr>,
            span: Range<usize>,
        },
        Class {
            name: String,
            super_class: Option<Expr>,
            methods: Vec<Stmt>,
            span: Range<usize>,
        },
    }

    #[derive(Debug, Clone)]
    pub enum ControlFlow {
        None,
        Return(LoxVar),
    }

    impl Display for Stmt {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                Stmt::Expression(expr) => write!(f, "{expr}"),
                Stmt::Print(expr) => write!(f, "(print {expr})"),
                Stmt::Var {
                    name, initializer, ..
                } => write!(
                    f,
                    "(var {}{})",
                    name,
                    initializer
                        .as_ref()
                        .map(|value| " = ".to_owned() + &value.to_string())
                        .unwrap_or("".to_owned())
                ),
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
                    if let Some(m) = methods.first() {
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
}

pub mod expr {
    use core::ops::Range;

    use super::*;
    #[derive(Debug, Clone, PartialEq)]
    pub enum Expr {
        Literal(Literal),

        Unary {
            operator: TokenKind,
            span: Range<usize>,
            right: Box<Expr>,
        },

        Binary {
            left: Box<Expr>,
            operator: TokenKind,
            right: Box<Expr>,
            span: Range<usize>,
        },

        Grouping(Box<Expr>),

        Variable {
            name: String,
            distance: Option<usize>,
            span: Range<usize>,
        },

        Assign {
            name: String,
            distance: Option<usize>,
            span: Range<usize>,
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
            span: Range<usize>,
        },

        Get {
            object: Box<Expr>,
            name: String,
            span: Range<usize>,
        },

        Set {
            object: Box<Expr>,
            name: String,
            value: Box<Expr>,
            span: Range<usize>,
        },

        Super {
            method_name: String,
            distance: Option<usize>,
            span: Range<usize>,
        },

        This {
            distance: Option<usize>,
            span: Range<usize>,
        },
    }

    impl Expr {
        pub fn resolve(&mut self, dist: usize) {
            match self {
                Expr::Variable { distance, .. }
                | Expr::Assign { distance, .. }
                | Expr::This { distance, .. }
                | Expr::Super { distance, .. } => *distance = Some(dist),
                _ => panic!(
                    "Tried to resolve a non resolvable expression ({self:?}). This is a bug."
                ),
            }
        }

        pub fn binary(left: Expr, operator: Token<'_>, right: Expr) -> Self {
            Self::Binary {
                left: Box::new(left),
                operator: operator.kind,
                right: Box::new(right),
                span: operator.span,
            }
        }

        pub fn unary(operator: Token<'_>, right: Expr) -> Self {
            Self::Unary {
                operator: operator.kind,
                right: Box::new(right),
                span: operator.span,
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
                Expr::Grouping(expr) => "(".to_owned() + &expr.string() + ")",
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
    }

    impl Display for Expr {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Number(f64),
    String(String),
    True,
    False,
    Nil,
}

impl Display for Literal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Literal::Number(n) => write!(f, "{n}"),
            Literal::String(s) => write!(f, "\"{s}\""),
            Literal::True => write!(f, "true"),
            Literal::False => write!(f, "false"),
            Literal::Nil => write!(f, "nil"),
        }
    }
}
