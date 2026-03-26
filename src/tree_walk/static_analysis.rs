use std::collections::HashMap;

use crate::tree_walk::{
    ast::{expr::Expr, stmt::Stmt},
    error::{
        ErrorPos,
        anaylysis_err::{ResolverErr, ResolverErrKind},
    },
};

pub struct Resolver {
    scopes: Vec<HashMap<String, bool>>,
    errors: Vec<ResolverErr>,
    func_path: Vec<FunctionKind>,
    class_path: Vec<ClassKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionKind {
    None,
    Method,
    Function,
    Initializer,
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
        }
    }

    pub fn resolve_stmts(&mut self, stmts: &mut [Stmt]) -> Result<(), Vec<ResolverErr>> {
        for s in stmts.iter_mut() {
            self.resolve_stmt(s);
        }

        if !self.errors.is_empty() {
            Err(self.errors.clone())
        } else {
            Ok(())
        }
    }

    fn error(&mut self, kind: ResolverErrKind, pos: ErrorPos) {
        self.errors.push(ResolverErr::new(kind, pos));
    }

    fn begin_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn end_scope(&mut self) {
        self.scopes.pop();
    }

    fn decalare(&mut self, name: String, pos: ErrorPos) {
        let error;
        if let Some(scope) = self.scopes.iter_mut().last() {
            if scope.contains_key(&name) {
                error = Some(ResolverErrKind::DefinedMultipleTimes(name.to_owned()))
            } else {
                error = None
            }
            scope.insert(name, false);
        } else {
            error = None
        }

        if let Some(kind) = error {
            self.error(kind, pos);
        }
    }

    /// # Panics
    /// Panics if define is called before declare
    fn define(&mut self, name: String) {
        if let Some(scope) = self.scopes.iter_mut().last() {
            scope
                .insert(name, true)
                .expect("Tried to define before declaring");
        }
    }

    fn resolve_function(
        &mut self,
        parameters: &Vec<String>,
        pos: ErrorPos,
        body: &mut [Stmt],
        kind: FunctionKind,
    ) {
        self.func_path.push(kind);
        self.begin_scope();

        for param in parameters {
            self.decalare(param.clone(), pos);
            self.define(param.clone());
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
                pos,
            } => {
                self.decalare(name.clone(), *pos);
                self.define(name.clone());
                self.resolve_function(parameters, *pos, body, FunctionKind::Function);
            }
            Stmt::Block(stmts) => {
                self.begin_scope();
                let _ = self.resolve_stmts(stmts);
                self.end_scope();
            }
            Stmt::Var {
                name,
                initializer,
                position,
            } => {
                self.decalare(name.to_owned(), *position);
                if let Some(initializer) = initializer {
                    self.resolve_expr(initializer);
                }
                self.define(name.to_owned());
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
            Stmt::Return { value, pos } => {
                match self.func_path.last().expect(
                    "Only ever descends and ascends in parralel so should never pop the top",
                ) {
                    FunctionKind::None => self.error(ResolverErrKind::ReturnFromTop, *pos),

                    FunctionKind::Initializer => {
                        if value.is_some() {
                            self.error(ResolverErrKind::ReturnValueFromInit, *pos)
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
                pos,
            } => {
                self.decalare(name.to_owned(), *pos);
                self.define(name.to_owned());

                if let Some(s_class) = super_class {
                    if let Expr::Variable {
                        name: s_name,
                        position: s_pos,
                        ..
                    } = s_class
                        && name == s_name
                    {
                        self.error(
                            ResolverErrKind::SelfReferentialInheritance,
                            pos.span_to(*s_pos),
                        );
                    }

                    self.resolve_expr(s_class);

                    self.begin_scope();
                    self.scopes
                        .iter_mut()
                        .last()
                        .expect("Just began a scope")
                        .insert("super".to_owned(), true);
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
                    .insert("this".to_owned(), true);

                for method in methods {
                    if let Stmt::Function {
                        parameters,
                        body,
                        pos,
                        name,
                    } = method
                    {
                        self.resolve_function(
                            parameters,
                            *pos,
                            body,
                            if name == "init" {
                                FunctionKind::Initializer
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
            Expr::Variable { name, position, .. } => {
                if self
                    .scopes
                    .last()
                    .is_some_and(|map| map.get(name).is_some_and(|v| !*v))
                {
                    self.error(ResolverErrKind::ReadInOwnInitializer, *position);
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
            Expr::Get { object, .. } => {
                self.resolve_expr(object);
                return;
            }
            Expr::Set { object, value, .. } => {
                self.resolve_expr(object);
                self.resolve_expr(value);
                return;
            }
            Expr::This { pos, .. } => {
                if *self
                    .class_path
                    .last()
                    .expect("Always has a higher class as they come in pairs")
                    == ClassKind::None
                {
                    self.error(ResolverErrKind::ThisOutsideClass, *pos);
                    return;
                }
                "this"
            }
            Expr::Super { pos, .. } => {
                if *self
                    .class_path
                    .last()
                    .expect("Always has a higher class as they come in pairs")
                    != ClassKind::SubClass
                {
                    self.error(ResolverErrKind::SuperOutsideSubclass, *pos);
                    return;
                }
                "super"
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
