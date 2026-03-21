use std::{collections::HashMap};

use crate::tree_walk::{
    ast::{Expr, Stmt},
    error::{
        ErrorPos,
        anaylysis_err::{ResolverErr, ResolverErrKind},
    },
};

pub struct Resolver {
    scopes: Vec<HashMap<String, bool>>,
    errors: Vec<ResolverErr>,
    func_depth: usize,
}

impl Resolver {
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            errors: Vec::new(),
            func_depth: 0,
        }
    }

    pub fn resolve_stmts(&mut self, stmts: &mut Vec<Stmt>) -> Result<(), Vec<ResolverErr>> {
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

    fn resolve_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Function {
                name,
                parameters,
                body,
                pos
            } => {
                self.decalare(name.clone(), *pos);
                self.define(name.clone());
                {
                    // Resolve function
                    self.begin_scope();
                    for param in parameters {
                        self.decalare(param.clone(), *pos);
                        self.define(param.clone());
                    }
                    let _ = self.resolve_stmts(body);
                    self.end_scope();
                }
            }
            Stmt::Block(stmts) => {
                self.begin_scope();
                let _ = self.resolve_stmts(stmts);
                self.end_scope();
            }
            Stmt::Var { name, initializer, position } => {
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
            Stmt::Return { value, pos } => {if self.func_depth == 0 {self.error(ResolverErrKind::ReturnFromTop, *pos);}; self.resolve_expr(value)},
            Stmt::While { condition, body } => {
                self.resolve_expr(condition);
                self.resolve_stmt(body);
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

                name.clone()
            }
            Expr::Assign { name, value, .. } => {
                self.resolve_expr(value);
                name.clone()
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
        };

        self.resolve_local(expr, &name);
    }

    fn resolve_local(&self, expr: &mut Expr, name: &str) {
        for (i, s) in self.scopes.iter().rev().enumerate() {
            if s.contains_key(name) {
                expr.resolve(i);
                return;
            }
        }
    }
}
