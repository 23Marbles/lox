//! Static analysis for Lox source code.

use std::collections::HashMap;

use codespan_reporting::diagnostic::Diagnostic;

use crate::{
    Span,
    error::{ToCodespan as _, anaylysis_err::ResolverErr},
    lox_files::FileIdent,
    tree_walk::ast::{
        expr::{Expr, FieldAccess},
        stmt::Stmt,
    },
};

/// A resolver that performs static analysis on a stream of statements.
pub struct Resolver {
    scopes: Vec<HashMap<String, (bool, Span)>>,
    errors: Vec<Diagnostic<FileIdent>>,
    func_path: Vec<FunctionKind>,
    class_path: Vec<ClassKind>,
    /// Each vec *MUST* have at least two elements
    def_mul_times: Vec<HashMap<String, Vec<Span>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FunctionKind {
    None,
    Method,
    Function,
    Initializer(Span),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassKind {
    None,
    Class,
    SubClass,
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver {
    /// Creates a new resolver.
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            errors: Vec::new(),
            func_path: {
                let mut vec = Vec::with_capacity(4);
                vec.push(FunctionKind::None);
                vec
            },
            class_path: {
                let mut vec = Vec::with_capacity(3);
                vec.push(ClassKind::None);
                vec
            },
            def_mul_times: Vec::new(),
        }
    }

    /// Resolves the given stream of statements.
    pub fn resolve_stmts(&mut self, stmts: &mut [Stmt]) -> crate::Result<()> {
        for s in stmts.iter_mut() {
            self.resolve_stmt(s);
        }

        if !self.errors.is_empty() {
            Err(self.errors.clone())
        } else {
            Ok(())
        }
    }

    fn error(&mut self, kind: ResolverErr) {
        self.errors.push(kind.to_codespan());
    }

    fn begin_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.def_mul_times.push(HashMap::new());
    }

    fn end_scope(&mut self) {
        self.scopes.pop();
        if let Some(last) = self.def_mul_times.last().cloned() {
            for (name, redefs) in last.into_iter() {
                self.error(ResolverErr::DefinedMultipleTimes(name, redefs));
            }
        }
        self.def_mul_times.pop();
    }

    fn decalare(&mut self, name: String, span: Span) {
        if let Some(scope) = self.scopes.iter_mut().last() {
            if let Some((_, p_span)) = scope.get(&name) {
                let def_scope = self
                    .def_mul_times
                    .iter_mut()
                    .last()
                    .expect("def_mul_times (len = 0) should always match scopes");
                if let Some(spans) = def_scope.get_mut(&name) {
                    spans.push(span.clone());
                } else {
                    def_scope.insert(name.clone(), vec![p_span.clone(), span.clone()]);
                };
            }
            scope.insert(name, (false, span));
        }
    }

    /// # Panics
    /// Panics if define is called before declare
    fn define(&mut self, name: &String) {
        if let Some(scope) = self.scopes.iter_mut().last() {
            scope
                .get_mut(name)
                .expect("Tried to define before declaring")
                .0 = true;
        }
    }

    fn resolve_function(
        &mut self,
        parameters: &[String],
        span: Span,
        body: &mut [Stmt],
        kind: FunctionKind,
    ) {
        self.func_path.push(kind);
        self.begin_scope();

        for param in parameters {
            self.decalare(param.clone(), span.clone());
            self.define(param);
        }
        let _ = self.resolve_stmts(body);
        self.end_scope();

        self.func_path.pop();
    }

    fn resolve_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Function {
                name,
                parameters,
                body,
                span,
            } => {
                self.decalare(name.clone(), span.clone());
                self.define(name);
                self.resolve_function(parameters, span.clone(), body, FunctionKind::Function);
            }
            Stmt::Block(stmts) => {
                self.begin_scope();
                let _ = self.resolve_stmts(stmts);
                self.end_scope();
            }
            Stmt::Var {
                name,
                initializer,
                span,
            } => {
                self.decalare(name.clone(), span.clone());
                if let Some(initializer) = initializer {
                    self.resolve_expr(initializer);
                }
                self.define(name);
            }
            Stmt::Expression(expr) => self.resolve_expr(expr),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(condition);
                self.resolve_stmt(then_branch);
                if let Some(branch) = else_branch {
                    self.resolve_stmt(branch);
                }
            }
            Stmt::Print(expr) => self.resolve_expr(expr),
            Stmt::Return { value, span } => {
                match self.func_path.last().expect(
                    "Only ever descends and ascends in parralel so should never pop the top",
                ) {
                    FunctionKind::None => self.error(ResolverErr::ReturnFromTop(span.clone())),

                    FunctionKind::Initializer(init_span) => {
                        if value.is_some() {
                            self.error(ResolverErr::ReturnValueFromInit([
                                span.clone(),
                                init_span.clone(),
                            ]))
                        }
                    }
                    _ => (),
                };

                if let Some(val) = value {
                    self.resolve_expr(val)
                }
            }
            Stmt::While { condition, body } => {
                self.resolve_expr(condition);
                self.resolve_stmt(body);
            }
            Stmt::Class {
                name,
                super_class,
                methods,
                span,
            } => {
                self.decalare(name.clone(), span.clone());
                self.define(name);

                if let Some(Expr::Variable {
                    name: s_name,
                    span: s_span,
                    ..
                }) = super_class.clone()
                {
                    if *name == s_name {
                        assert_eq!(
                            s_span.file_id, span.file_id,
                            "Superclass must be defined in the same file as the if there is a self-referential inheritance"
                        );
                        self.error(ResolverErr::SelfReferentialInheritance(Span::new(
                            span.range.start..s_span.range.end,
                            span.file_id,
                        )));
                    }

                    #[expect(clippy::unwrap_used)]
                    // Already know this is a Some(Variant); candidate for unwrap_unchecked() if the performace boost is worth it
                    self.resolve_expr(super_class.as_mut().unwrap());

                    self.begin_scope();

                    self.scopes
                        .iter_mut()
                        .last()
                        .expect("Just began a scope")
                        .insert("super".to_owned(), (true, s_span));
                }

                self.begin_scope();
                self.class_path.push(if super_class.is_some() {
                    ClassKind::SubClass
                } else {
                    ClassKind::Class
                });

                self.scopes
                    .iter_mut()
                    .last()
                    .expect("Just began a new scope")
                    .insert("this".to_owned(), (true, span.clone()));

                for method in methods {
                    if let Stmt::Function {
                        parameters,
                        body,
                        span,
                        name,
                    } = method
                    {
                        self.resolve_function(
                            parameters,
                            span.clone(),
                            body,
                            if name == "init" {
                                FunctionKind::Initializer(span.clone())
                            } else {
                                FunctionKind::Method
                            },
                        );
                    }
                }

                self.end_scope();

                if super_class.is_some() {
                    self.end_scope();
                }

                self.class_path.pop();
            }
        }
    }

    fn resolve_expr(&mut self, expr: &mut Expr) {
        let name = match expr {
            Expr::Variable { name, span, .. } => {
                if self
                    .scopes
                    .last()
                    .is_some_and(|map| map.get(name).is_some_and(|v| !v.0))
                {
                    self.error(ResolverErr::ReadInOwnInitializer(span.clone()));
                }

                &name.clone()
            }
            Expr::Assign { name, value, .. } => {
                self.resolve_expr(value);
                &name.clone()
            }
            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
                return;
            }
            Expr::Call { callee, args, .. } => {
                self.resolve_expr(callee);
                for arg in args {
                    self.resolve_expr(arg);
                }
                return;
            }
            Expr::Grouping(expr) => {
                self.resolve_expr(expr);
                return;
            }
            Expr::Literal(..) => {
                return;
            }
            Expr::Logical { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
                return;
            }
            Expr::Unary { right, .. } => {
                self.resolve_expr(right);
                return;
            }
            Expr::Get {
                object, get_type, ..
            } => {
                self.resolve_expr(object);
                match get_type {
                    FieldAccess::Property(_) => (),
                    FieldAccess::Index(expr) => self.resolve_expr(expr),
                }
                return;
            }
            Expr::Set {
                object,
                value,
                set_type,
                ..
            } => {
                self.resolve_expr(object);
                self.resolve_expr(value);
                match set_type {
                    FieldAccess::Property(_) => (),
                    FieldAccess::Index(expr) => self.resolve_expr(expr),
                }
                return;
            }
            Expr::This { span, .. } => {
                if *self
                    .class_path
                    .last()
                    .expect("Always has a higher class as they come in pairs")
                    == ClassKind::None
                {
                    self.error(ResolverErr::ThisOutsideClass(span.clone()));
                    return;
                }
                "this"
            }
            Expr::Super { span, .. } => {
                if *self
                    .class_path
                    .last()
                    .expect("Always has a higher class as they come in pairs")
                    != ClassKind::SubClass
                {
                    self.error(ResolverErr::SuperOutsideSubclass(span.clone()));
                    return;
                }
                "super"
            }
            Expr::List { parts, .. } => {
                for part in parts {
                    self.resolve_expr(part);
                }
                return;
            }
            Expr::Ternary {
                condition,
                left,
                right,
            } => {
                self.resolve_expr(condition);
                self.resolve_expr(left);
                self.resolve_expr(right);
                return;
            }
            Expr::Comma { redundant, result } => {
                for expr in redundant {
                    self.resolve_expr(expr);
                }
                self.resolve_expr(result.as_mut());
                return;
            }
        };

        self.resolve_local(expr, name);
    }

    fn resolve_local(&self, expr: &mut Expr, name: &str) {
        //println!("RESOLVE {} -> {:?}", name, self.scopes);
        for (i, s) in self.scopes.iter().rev().enumerate() {
            if s.contains_key(name) {
                expr.resolve(i);
                return;
            }
        }
    }
}
