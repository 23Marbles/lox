//! AST nodes for the Lox language.

use core::fmt::Display;

use crate::tree_walk::{
    ast::expr::Expr,
    lox_var::LoxVar,
    token::{Token, TokenKind},
};

/// Module for statement AST nodes.
pub mod stmt {
    use crate::Span;

    use super::*;
    #[derive(Debug, Clone, PartialEq)]
    /// Represents a statement in the AST.
    pub enum Stmt {
        /// Represents an [`Expr`] statement.
        Expression(Expr),
        /// Represents a print statement.
        /// This will print the value of the expression.
        Print(Expr),
        /// Represents a variable declaration statement.
        Var {
            /// The span of the variable declaration in the source code.
            span: Span,
            /// The name of the variable being declared.
            name: String,
            /// The initializer expression for the variable, if any.
            initializer: Option<Expr>,
        },
        Throwaway {
            span: Span,
            initializer: Expr,
        },
        /// Represents a block of statements.
        Block(Vec<Stmt>),
        /// Represents an if statement.
        /// This will execute the `then_branch` if the `condition` is true, and the `else_branch` if it is false.
        If {
            /// The condition expression for the if statement.
            condition: Expr,
            /// The branch to execute if the condition is true.
            then_branch: Box<Stmt>,
            /// The branch to execute if the condition is false, if any.
            else_branch: Option<Box<Stmt>>,
        },
        /// Represents a while statement.
        While {
            /// The condition expression for the while statement.
            condition: Expr,
            /// The body of the while statement.
            body: Box<Stmt>,
        },
        /// Represents a function declaration.
        Function {
            /// The span of the function declaration in the source code.
            span: Span,
            /// The name of the function being declared.
            name: String,
            /// The parameters of the function.
            parameters: Vec<String>,
            /// The body of the function.
            body: Vec<Stmt>,
        },
        /// Represents a return statement.
        Return {
            /// The value being returned, if any.
            value: Option<Expr>,
            /// The span of the return statement in the source code.
            span: Span,
        },
        Continue {
            span: Span,
        },
        Break {
            span: Span,
        },
        /// Represents a class declaration.
        Class {
            /// The name of the class being declared.
            name: String,
            /// The superclass of the class, if any.
            super_class: Option<Expr>,
            /// The methods of the class.
            methods: Vec<Stmt>,
            /// The span of the class declaration in the source code.
            span: Span,
        },
    }

    #[derive(Debug, Clone)]
    /// Represents the control flow of a function.
    pub enum ControlFlow {
        /// No control flow, the function continues normally.
        None,
        Continue,
        Break,
        /// A return statement has been encountered, with the given value.
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
                Stmt::Throwaway { initializer, .. } => write!(f, "(throwaway {initializer})"),
                Stmt::Continue { .. } => write!(f, "(continue)"),
                Stmt::Break { .. } => write!(f, "(break)"),
            }
        }
    }
}

/// Module for expressions in the AST.
pub mod expr {
    use crate::Span;

    #[derive(Debug, Clone, PartialEq)]
    /// Represents the type of access, either by property or index.
    pub enum FieldAccess {
        /// Represents a property-based access.
        Property(String),
        /// Represents an index-based access.
        Index(Box<Expr>),
    }

    impl Display for FieldAccess {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                FieldAccess::Property(name) => write!(f, "(prop {name})"),
                FieldAccess::Index(idx) => write!(f, "(prop {idx})"),
            }
        }
    }

    impl FieldAccess {
        fn string(&self) -> String {
            match self {
                FieldAccess::Property(name) => format!(".{name}"),
                FieldAccess::Index(idx) => format!("[{idx}]"),
            }
        }
    }

    use super::*;
    #[derive(Debug, Clone, PartialEq)]
    /// Represents an expression in the AST.
    pub enum Expr {
        /// Represents a literal value in the AST. Contains a [`Literal`] value.
        Literal(Literal),
        /// Represents a list literal in the AST. Contains a list of [`Expr`] expressions.
        List {
            /// The list of expressions in the list literal.
            parts: Vec<Expr>,
            /// The span of the list literal in the source code.
            span: Span,
        },
        /// Represents a unary expression in the AST. Contains an [`operator`][`TokenKind`] and a [`right`][`Expr`] expression.
        Unary {
            /// The operator of the unary expression.
            operator: TokenKind,
            /// The span of the unary expression in the source code.
            span: Span,
            /// The right-hand side expression of the unary expression.
            right: Box<Expr>,
        },

        /// Represents a binary expression in the AST. Contains a [`left`][`Expr`] expression, an [`operator`][`TokenKind`], and a [`right`][`Expr`] expression.
        Binary {
            /// The left-hand side expression of the binary expression.
            left: Box<Expr>,
            /// The operator of the binary expression.
            operator: TokenKind,
            /// The right-hand side expression of the binary expression.
            right: Box<Expr>,
            /// The span of the binary expression in the source code.
            span: Span,
        },

        /// Represents a grouping expression in the AST. Contains an [`Expr`] expression.
        Grouping(Box<Expr>),

        Comma {
            redundant: Vec<Expr>,
            result: Box<Expr>,
        },

        /// Represents a variable expression in the AST. Contains a `name` and an optional `distance` for lookup.
        /// This is set in the resolver and used during evaluation.
        Variable {
            /// The span of the variable expression in the source code.
            span: Span,
            /// The name of the variable.
            name: String,
            /// The distance from the current scope for lookup. If it is `None`, the variable is a global variable.
            distance: Option<usize>,
        },

        /// Represents an assignment expression in the AST. Contains a `name`, `distance`, [`value`][`Expr`] and an optional `distance` for lookup.
        /// This is set in the resolver and used during evaluation.
        Assign {
            /// The name of the variable being assigned.
            name: String,
            /// The distance from the current scope for lookup. If it is `None`, the variable is a global variable.
            distance: Option<usize>,
            /// The span of the assignment expression in the source code.
            span: Span,
            /// The value being assigned to the variable.
            value: Box<Expr>,
        },

        /// Represents a logical expression in the AST. Contains a [`left`][`Expr`] expression, an `operator`, and a [`right`][`Expr`] expression.
        Logical {
            /// The left-hand side expression of the logical expression.
            left: Box<Expr>,
            /// The operator of the logical expression.
            operator: TokenKind,
            /// The right-hand side expression of the logical expression.
            right: Box<Expr>,
        },

        Ternary {
            condition: Box<Expr>,
            left: Box<Expr>,
            right: Box<Expr>,
        },

        /// Represents a function call expression in the AST. Contains a [`callee`][`Expr`] expression and a list of [`args`][`Expr`] expressions.
        Call {
            /// The expression being called.
            callee: Box<Expr>,
            /// The list of arguments to the function call.
            args: Vec<Expr>,
            /// The span of the function call expression in the source code.
            span: Span,
        },

        /// Represents a get expression in the AST. Contains an [`object`][`Expr`] expression and a [`get_type`][`PropertyAccess`].
        Get {
            /// The object expression being accessed.
            object: Box<Expr>,
            /// The type of get operation (property or index).
            get_type: FieldAccess,
            /// The span of the get expression in the source code.
            span: Span,
        },

        /// Represents a set expression in the AST. Contains an [`object`][`Expr`] expression, a [`set_type`][`PropertyAccess`], and a [`value`][`Expr`] expression.
        Set {
            /// The object expression being set.
            object: Box<Expr>,
            /// The type of set operation (property or index).
            set_type: FieldAccess,
            /// The value being set.
            value: Box<Expr>,
            /// The span of the set expression in the source code.
            span: Span,
        },

        /// Represents a super expression in the AST. Contains a [`method_name`] string and an optional [`distance`] usize.
        Super {
            /// The name of the method being called.
            method_name: String,
            /// The distance from the current scope to the superclass scope.
            /// If `None`, distance is to the global scope.
            distance: Option<usize>,
            /// The span of the super expression in the source code.
            span: Span,
        },

        /// Represents a `this` expression in the AST. Contains an optional [`distance`] usize.
        This {
            /// The distance from the current scope to the superclass scope.
            /// If `None`, distance is to the global scope.
            distance: Option<usize>,
            /// The span of the `this` expression in the source code.
            span: Span,
        },
    }

    impl Expr {
        /// Resolves the [`distance`] field of a [`Variable`], [`Assign`], [`This`], or [`Super`] expression to the given [`dist`] value.
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

        /// Creates a binary expression with the given left and right operands and operator.
        pub fn binary(left: Expr, operator: Token<'_>, right: Expr) -> Self {
            Self::Binary {
                left: Box::new(left),
                operator: operator.kind,
                right: Box::new(right),
                span: operator.span,
            }
        }

        /// Creates a unary expression with the given operator and right operand.
        pub fn unary(operator: Token<'_>, right: Expr) -> Self {
            Self::Unary {
                operator: operator.kind,
                right: Box::new(right),
                span: operator.span,
            }
        }

        /// Returns a string representation of the expression.
        /// This is designed to be human-readable, not machine-parsable.
        /// It more closely resembles the source code than a machine-readable format.
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
                Expr::Get {
                    object, get_type, ..
                } => object.string() + &get_type.string(),
                Expr::Set {
                    object,
                    value,
                    set_type,
                    ..
                } => object.string() + &set_type.string() + " = " + &value.to_string(),
                Expr::This { .. } => "this".to_owned(),
                Expr::Super { method_name, .. } => "super.".to_owned() + method_name,
                Expr::List { parts, .. } => format!(
                    "[{}]",
                    parts
                        .iter()
                        .map(|ex| ex.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                Expr::Ternary {
                    condition,
                    left,
                    right,
                } => condition.string() + " ? " + &left.string() + " : " + &right.string(),
                Expr::Comma { redundant, result } => {
                    redundant
                        .iter()
                        .map(|ex| ex.string())
                        .collect::<Vec<_>>()
                        .join(", ")
                        + ", "
                        + &result.string()
                }
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
                Expr::Get {
                    object, get_type, ..
                } => write!(f, "(get ({object} {get_type}))"),
                Expr::Set {
                    object,
                    set_type,
                    value,
                    ..
                } => write!(f, "(set ({object} {set_type}) {value})"),
                Expr::This { .. } => write!(f, "(this)"),
                Expr::Super { .. } => write!(f, "super"),
                Expr::List { parts, .. } => write!(
                    f,
                    "(list{})",
                    if !parts.is_empty() {
                        " ".to_string()
                            + &parts
                                .iter()
                                .map(|ex| ex.to_string())
                                .collect::<Vec<_>>()
                                .join(" ")
                    } else {
                        "".to_string()
                    }
                ),
                Expr::Ternary {
                    condition,
                    left,
                    right,
                } => write!(f, "(ternary ({condition} {left} {right}))"),
                Expr::Comma { redundant, result } => write!(
                    f,
                    "(comma {}{})",
                    if !redundant.is_empty() {
                        redundant
                            .iter()
                            .map(|ex| ex.to_string())
                            .collect::<Vec<_>>()
                            .join(" ")
                            + ", "
                    } else {
                        "".to_string()
                    },
                    result
                ),
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Represents a literal value in the AST.
pub enum Literal {
    /// Represents a numeric literal value.
    Number(f64),
    /// Represents a string literal value.
    String(String),
    /// Represents the boolean literal value `true`.
    True,
    /// Represents the boolean literal value `false`.
    False,
    /// Represents the `nil` literal value.
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
