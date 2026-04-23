//! Parser for Lox source code.

use std::char;

use codespan_reporting::diagnostic::Diagnostic;

use crate::{
    Span,
    error::{
        ToCodespan as _,
        parse_err::{Named, ParseError},
    },
    lox_files::FileIdent,
    tree_walk::{
        ast::{
            expr::{Expr, FieldAccess},
            stmt::Stmt,
        },
        token::{Token, TokenKind},
    },
};

/// A parser that produces a stream of statements from a sequence of tokens.
pub struct Parser<'source> {
    tokens: Vec<Token<'source>>,
    current: usize,
    errors: Vec<Diagnostic<FileIdent>>,
    file_id: FileIdent,
}

impl<'source> Iterator for Parser<'source> {
    type Item = Result<Stmt, Diagnostic<FileIdent>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.is_at_end() {
            None
        } else {
            match self.declaration() {
                Ok(s) => Some(Ok(s)),
                Err(e) => {
                    self.synchronize();
                    Some(Err(e))
                }
            }
        }
    }
}

impl<'source> Parser<'source> {
    /// Creates a new parser from a sequence of tokens and a file id.
    pub fn new(tokens: Vec<Token<'source>>, file_id: FileIdent) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
            file_id,
        }
    }

    #[must_use]
    fn error(&mut self, kind: ParseError) -> Diagnostic<FileIdent> {
        let err = kind.to_codespan();

        self.errors.push(err.clone());

        err
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn peek(&self) -> &Token<'source> {
        &self.tokens[self.current]
    }

    /// # Panics
    /// Panics if the current token is an Eof
    fn peek_ahead(&self) -> Option<&Token<'source>> {
        if self.is_at_end() {
            None
        } else {
            Some(&self.tokens[self.current + 1])
        }
    }

    fn previous(&self) -> &Token<'source> {
        &self.tokens[self.current - 1]
    }

    fn check(&self, kind: TokenKind) -> bool {
        if self.is_at_end() {
            false
        } else {
            self.peek().kind == kind
        }
    }

    fn advance(&mut self) -> &Token<'source> {
        if !self.is_at_end() {
            self.current += 1
        };
        self.previous()
    }

    fn match_any<T: IntoIterator<Item = TokenKind>>(&mut self, kinds: T) -> bool {
        for k in kinds {
            if self.check(k) {
                self.advance();
                return true;
            }
        }

        false
    }

    /// Parses the tokens into a stream of statements.
    pub fn parse(&mut self) -> crate::Result<Vec<Stmt>> {
        let statements = self
            .filter_map(|res: Result<Stmt, Diagnostic<FileIdent>>| res.ok())
            .collect();

        if self.errors.is_empty() {
            Ok(statements)
        } else {
            Err(self.errors.clone())
        }
    }

    fn declaration(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        if self.match_any([TokenKind::Var]) {
            self.var_declaration()
        } else if self.check(TokenKind::Fun)
            && self
                .peek_ahead()
                .map_or(false, |token| token.kind == TokenKind::Identifier)
            || self
                .peek_ahead()
                .map_or(false, |token| token.kind == TokenKind::UnderScore)
        {
            self.advance();
            self.fun_declaration(Named::Function)
        } else if self.match_any([TokenKind::Class]) {
            self.class_declaration()
        } else {
            self.statement()
        }
    }

    fn class_declaration(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        let (name, name_span) = match self.peek().kind {
            TokenKind::Identifier => {
                let token = self.advance();
                (Some(token.lexeme.to_owned()), token.span.clone())
            }
            TokenKind::UnderScore => {
                let span = self.advance().span.clone();
                (None, span)
            }
            _ => {
                return Err(self.error(ParseError::ExpectedName {
                    name: Named::Class,
                    after: self.previous().kind,
                    span: self.peek().span.clone(),
                    prev_span: self.previous().span.clone(),
                }));
            }
        };

        let super_class = if self.match_any([TokenKind::Less]) {
            let s_name_token = self.consume(
                TokenKind::Identifier,
                ParseError::ExpectedName {
                    name: Named::SuperClass,
                    after: self.previous().kind,
                    prev_span: self.previous().span.clone(),
                    span: self.peek().span.clone(),
                },
            )?;
            Some(Expr::Variable {
                name: s_name_token.lexeme.to_owned(),
                distance: None,
                span: s_name_token.span,
            })
        } else {
            None
        };

        self.consume(
            TokenKind::LeftBrace,
            if super_class.is_some() {
                ParseError::ExpectedTokenKind {
                    got: self.peek().kind,
                    expected: TokenKind::LeftBrace,
                    span: self.peek().span.clone(),
                }
            } else {
                ParseError::UnexpectedTokenKind {
                    got: self.peek().kind,
                    expected: [TokenKind::LeftBrace, TokenKind::Less].to_vec(),
                    span: self.peek().span.clone(),
                }
            },
        )?;

        let mut methods: Vec<Stmt> = Vec::new();

        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            methods.push(self.fun_declaration(Named::Method)?);
        }

        self.expect(TokenKind::RightBrace)?;

        Ok(Stmt::Class {
            name: name,
            methods,
            super_class,
            span: name_span,
        })
    }

    fn fun_declaration(&mut self, kind: Named) -> Result<Stmt, Diagnostic<FileIdent>> {
        let start_pos = self.peek().span.range.start;
        let name = match self.peek().kind {
            TokenKind::Identifier => {
                let token = self.advance();
                Some(token.lexeme.to_owned())
            }
            TokenKind::UnderScore => {
                self.advance();
                None
            }
            _ => {
                return Err(self.error(ParseError::ExpectedName {
                    name: kind,
                    after: self.previous().kind,
                    span: self.peek().span.clone(),
                    prev_span: self.previous().span.clone(),
                }));
            }
        };

        let parameters = self.params()?;

        let end_pos: usize = self.previous().span.range.end;

        self.expect(TokenKind::LeftBrace)?;

        let body = self.block()?;

        Ok(Stmt::Function {
            name: name.to_owned(),
            parameters,
            body,
            param_span: Span::new(start_pos..end_pos, self.file_id),
        })
    }

    fn params(&mut self) -> Result<Vec<Option<String>>, Diagnostic<FileIdent>> {
        self.expect(TokenKind::LeftParen)?;

        let mut parameters: Vec<Option<&'source str>> = Vec::new();

        let mut offending_spans: Vec<Span> = Vec::new();

        if !self.check(TokenKind::RightParen) {
            'collect_arguments: loop {
                let name = match self.peek().kind {
                    TokenKind::Identifier => {
                        let token = self.advance();
                        Some(token.lexeme)
                    }
                    TokenKind::UnderScore => {
                        self.advance();
                        None
                    }
                    _ => {
                        return Err(self.error(ParseError::ExpectedName {
                            name: Named::Parameter,
                            after: self.previous().kind,
                            span: self.peek().span.clone(),
                            prev_span: self.previous().span.clone(),
                        }));
                    }
                };

                parameters.push(name);

                if parameters.len() > u8::MAX as usize {
                    offending_spans.push(self.previous().span.clone());
                }

                if !self.match_any([TokenKind::Comma]) {
                    break 'collect_arguments;
                }
            }
        }

        if !offending_spans.is_empty() {
            let _ = self.error(ParseError::TooManyArguments {
                spans: offending_spans,
            });
        }

        self.expect(TokenKind::RightParen)?;

        Ok(parameters
            .into_iter()
            .map(|param| param.map(|name| name.to_owned()))
            .collect())
    }

    fn var_declaration(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        let (name, span) = match self.peek().kind {
            TokenKind::Identifier => {
                let token = self.advance();
                (Some(token.lexeme.to_owned()), token.span.clone())
            }
            TokenKind::UnderScore => {
                let span = self.advance().span.clone();
                (None, span)
            }
            _ => {
                return Err(self.error(ParseError::UnexpectedTokenKind {
                    got: self.peek().kind,
                    expected: [TokenKind::Identifier, TokenKind::UnderScore].to_vec(),
                    span: self.peek().span.clone(),
                }));
            }
        };

        let initializer = if self.match_any([TokenKind::Equal]) {
            Some(self.expression()?)
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Var {
            name: name,
            initializer,
            span: span,
        })
    }

    fn statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        use TokenKind::*;

        if self.match_any([Print]) {
            self.print_statement()
        } else if self.match_any([If]) {
            self.if_statement()
        } else if self.match_any([LeftBrace]) {
            Ok(Stmt::Block(self.block()?))
        } else if self.match_any([While]) {
            self.while_statement()
        } else if self.match_any([For]) {
            self.for_statement()
        } else if self.match_any([Return]) {
            self.return_statement()
        } else if self.match_any([Break]) {
            self.break_statement()
        } else if self.match_any([Continue]) {
            self.continue_statement()
        } else {
            self.expression_statement()
        }
    }

    fn return_statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        let start_pos: usize = self.previous().span.range.start;

        let value = if !self.check(TokenKind::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };

        let end_pos: usize = self.expect(TokenKind::Semicolon)?.span.range.end;

        Ok(Stmt::Return {
            value,
            span: Span::new(start_pos..end_pos, self.file_id),
        })
    }

    fn break_statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        let span = self.previous().span.clone();
        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Break { span })
    }

    fn continue_statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        let span = self.previous().span.clone();
        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Continue { span })
    }

    fn for_statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        self.expect(TokenKind::LeftParen)?;

        let initializer = if self.match_any([TokenKind::Semicolon]) {
            None
        } else if self.match_any([TokenKind::Var]) {
            Some(self.var_declaration()?)
        } else {
            Some(self.expression_statement()?)
        };

        let condition = if !self.check(TokenKind::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;

        let increment = if !self.check(TokenKind::RightParen) {
            Some(self.expression()?)
        } else {
            None
        };

        self.expect(TokenKind::RightParen)?;

        let mut body = self.statement()?;

        body = Stmt::For {
            condition,
            increment,
            body: Box::new(body),
        };

        if let Some(init) = initializer {
            body = Stmt::Block(vec![init, body])
        }

        Ok(body)
    }

    fn while_statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        self.expect(TokenKind::LeftParen)?;
        let condition = self.expression()?;
        self.expect(TokenKind::RightParen)?;
        Ok(Stmt::While {
            condition,
            body: Box::new(self.statement()?),
        })
    }

    fn if_statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        self.expect(TokenKind::LeftParen)?;
        let condition = self.expression()?;
        self.expect(TokenKind::RightParen)?;

        let then_branch = self.statement()?;
        let else_branch = if self.match_any([TokenKind::Else]) {
            Some(Box::new(self.statement()?))
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            then_branch: Box::new(then_branch),
            else_branch,
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, Diagnostic<FileIdent>> {
        let mut statements = Vec::new();

        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }

        self.expect(TokenKind::RightBrace)?;

        Ok(statements)
    }

    fn print_statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        let value = self.expression()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Print(value))
    }

    fn expression_statement(&mut self) -> Result<Stmt, Diagnostic<FileIdent>> {
        let value = self.expression()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Expression(value))
    }

    fn expression(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        self.comma()
    }

    fn comma(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut exprs = vec![self.assignment()?];

        while self.match_any([TokenKind::Comma]) {
            exprs.push(self.assignment()?);
        }

        if exprs.len() == 1 {
            Ok(exprs.pop().unwrap())
        } else {
            let last = exprs.pop().unwrap();
            Ok(Expr::Comma {
                redundant: exprs,
                result: Box::new(last),
            })
        }
    }

    fn assignment(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let expr = self.ternary()?;

        if self.match_any([TokenKind::Equal]) {
            let equals = self.previous().clone();
            let value = self.assignment()?;

            if let Expr::Variable { name, .. } = expr {
                Ok(Expr::Assign {
                    name,
                    value: Box::new(value),
                    distance: None,
                    span: equals.span,
                })
            } else if let Expr::Get {
                object,
                span,
                get_type,
            } = expr
            {
                Ok(Expr::Set {
                    object,
                    value: Box::new(value),
                    span,
                    set_type: get_type,
                })
            } else {
                let kind = ParseError::InvalidAssignTarget {
                    target: expr.to_string(),
                    span: equals.span,
                };

                let err = kind.to_codespan();

                self.errors.push(err.clone());

                Err(err)
            }
        } else {
            Ok(expr)
        }
    }

    fn ternary(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let expr = self.logic_or()?;

        if self.match_any([TokenKind::Question]) {
            let left = self.expression()?;
            self.expect(TokenKind::Colon)?;
            // Following the spec
            let right = self.ternary()?;

            Ok(Expr::Ternary {
                condition: Box::new(expr),
                left: Box::new(left),
                right: Box::new(right),
            })
        } else {
            Ok(expr)
        }
    }

    fn logic_or(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut expr = self.logic_and()?;

        while self.match_any([TokenKind::Or]) {
            expr = Expr::Logical {
                left: Box::new(expr),
                operator: self.previous().kind,
                right: Box::new(self.logic_and()?),
            };
        }

        Ok(expr)
    }

    fn logic_and(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut expr = self.equality()?;

        while self.match_any([TokenKind::And]) {
            expr = Expr::Logical {
                left: Box::new(expr),
                operator: self.previous().kind,
                right: Box::new(self.equality()?),
            };
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut expr = self.comparison()?;

        use TokenKind::*;
        while self.match_any([BangEqual, EqualEqual]) {
            // Append each series of operators to the expr recursivley
            expr = Expr::binary(expr, self.previous().clone(), self.comparison()?)
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut expr = self.term()?;

        use TokenKind::*;
        while self.match_any([Greater, GreaterEqual, Less, LessEqual]) {
            expr = Expr::binary(expr, self.previous().clone(), self.term()?)
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut expr = self.factor()?;

        use TokenKind::*;
        while self.match_any([Minus, Plus]) {
            expr = Expr::binary(expr, self.previous().clone(), self.factor()?)
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut expr = self.unary()?;

        use TokenKind::*;
        while self.match_any([Slash, Star, Percent]) {
            expr = Expr::binary(expr, self.previous().clone(), self.unary()?)
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        use TokenKind::*;
        if self.match_any([Bang, Minus]) {
            Ok(Expr::unary(self.previous().clone(), self.unary()?))
        } else {
            self.call()
        }
    }

    fn call(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut expr = self.primary()?;

        loop {
            if self.match_any([TokenKind::LeftParen]) {
                expr = self.finish_call(
                    expr,
                    match &self.tokens[self.current - 2] {
                        // No panicking (Just trust me bro)
                        ident if ident.kind == TokenKind::Identifier => ident.span.range.start,
                        _ => self.previous().span.range.start,
                    },
                )?;
            } else if self.match_any([TokenKind::Dot]) {
                let token = self.consume(
                    TokenKind::Identifier,
                    ParseError::ExpectedName {
                        name: Named::Property,
                        after: self.previous().kind,
                        prev_span: self.previous().span.clone(),
                        span: match self.peek_ahead() {
                            Some(t) => t.span.clone(),
                            None => self.peek().span.clone(), // Error originating here for file tests\number\decimal_point_at_eof.lox
                        },
                    },
                )?;
                expr = Expr::Get {
                    object: Box::new(expr),
                    span: token.span,
                    get_type: FieldAccess::Property(token.lexeme.to_owned()),
                };
            } else if self.match_any([TokenKind::LeftBracket]) {
                let start_pos = self.previous().span.range.start;
                let index = self.expression()?;
                let end_pos = self.expect(TokenKind::RightBracket)?.span.range.end;
                expr = Expr::Get {
                    object: Box::new(expr),
                    span: Span::new(start_pos..end_pos, self.file_id),
                    get_type: FieldAccess::Index(Box::new(index)),
                };
            } else {
                break Ok(expr);
            }
        }
    }

    fn finish_call(
        &mut self,
        calee: Expr,
        start_pos: usize,
    ) -> Result<Expr, Diagnostic<FileIdent>> {
        let mut args = Vec::new();

        let mut offending_spans: Vec<Span> = Vec::new();

        if !self.check(TokenKind::RightParen) {
            loop {
                args.push(self.assignment()?);

                if args.len() > u8::MAX as usize {
                    offending_spans.push(self.previous().span.clone());
                }

                if !self.match_any([TokenKind::Comma]) {
                    break;
                }
            }
        }

        let end_pos: usize = self.expect(TokenKind::RightParen)?.span.range.end;

        if !offending_spans.is_empty() {
            let _ = self.error(ParseError::TooManyArguments {
                spans: offending_spans,
            });
        }

        let span = Span::new(start_pos..end_pos, self.file_id);

        Ok(Expr::Call {
            callee: Box::new(calee),
            args,
            span,
        })
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        use crate::tree_walk::ast::Literal;
        use TokenKind::*;
        match self.peek().kind {
            True => {
                self.advance();
                Ok(Expr::Literal(Literal::True))
            }
            False => {
                self.advance();
                Ok(Expr::Literal(Literal::False))
            }
            Nil => {
                self.advance();
                Ok(Expr::Literal(Literal::Nil))
            }
            NAN => {
                self.advance();
                Ok(Expr::Literal(Literal::Number(f64::NAN)))
            }
            INF => {
                self.advance();
                Ok(Expr::Literal(Literal::Number(f64::INFINITY)))
            }
            Number => {
                let token = self.advance().clone();
                Ok(Expr::Literal(Literal::Number(
                    token.lexeme.parse().map_err(|_| {
                        self.error(ParseError::ExpectedNumber {
                            input: token.lexeme.to_owned(),
                            span: token.span.clone(),
                        })
                    })?,
                )))
            }
            String => Ok(Expr::Literal(Literal::String(
                unescape(self.advance().lexeme).map_err(|e_seqs| {
                    for seq in e_seqs {
                        let _ = self.error(ParseError::InvalidEscape {
                            seq,
                            span: self.previous().span.clone(),
                        });
                    }
                    self.errors
                        .last()
                        .expect("Already added errors to it")
                        .clone()
                })?,
            ))),
            LeftBracket => {
                self.advance();
                self.list_expression()
            }
            LeftParen => {
                self.advance();
                let expr = self.expression()?;
                self.expect(RightParen)?;
                Ok(Expr::Grouping(Box::new(expr)))
            }
            Super => {
                let start_pos = self.advance().span.range.start;

                self.expect(TokenKind::Dot)?;

                let ident_token = self.consume(
                    TokenKind::Identifier,
                    ParseError::ExpectedName {
                        name: Named::Method,
                        after: self.previous().kind,
                        prev_span: self.previous().span.clone(),
                        span: self
                            .peek_ahead()
                            .expect("Method name at end of token stream")
                            .span
                            .clone(),
                    },
                )?;

                Ok(Expr::Super {
                    method_name: ident_token.lexeme.to_owned(),
                    distance: None,
                    span: Span::new(start_pos..ident_token.span.range.end, self.file_id),
                })
            }
            This => {
                let span = self.advance().span.clone();
                Ok(Expr::This {
                    distance: None,
                    span,
                })
            }
            Identifier => {
                let token = self.advance();
                Ok(Expr::Variable {
                    name: token.lexeme.to_owned(),
                    distance: None,
                    span: token.span.clone(),
                })
            }
            UnderScore => {
                let start_pos = self.advance().span.range.start;
                self.expect(TokenKind::Equal)?;
                let expr = self.expression()?;
                let end_pos = self.previous().span.range.end;

                Ok(Expr::Throwaway {
                    span: Span::new(start_pos..end_pos, self.file_id),
                    expr: Box::new(expr),
                })
            }
            Try => {
                self.advance();
                Ok(Expr::Try {
                    expr: Box::new(self.expression()?),
                })
            }
            Fun => {
                self.advance();

                let start_pos = self.peek().span.range.start;
                let parameters = self.params()?;
                let end_pos = self.expect(TokenKind::LeftBrace)?.span.range.end;
                let body = self.block()?;

                Ok(Expr::Function {
                    param_span: Span::new(start_pos..end_pos, self.file_id),
                    parameters,
                    body,
                })
            }
            k => Err(self.error(ParseError::UnexpectedTokenKind {
                got: k,
                expected: [
                    True,
                    False,
                    Nil,
                    Number,
                    String,
                    LeftParen,
                    Super,
                    This,
                    Identifier,
                    LeftBracket,
                    UnderScore,
                    Try,
                    Fun,
                ]
                .to_vec(),
                span: self.peek().span.clone(),
            })),
        }
    }

    fn list_expression(&mut self) -> Result<Expr, Diagnostic<FileIdent>> {
        let start_pos: usize = self.previous().span.range.start;

        let mut parts: Vec<Expr> = Vec::new();

        if !self.check(TokenKind::RightBracket) {
            'collect_arguments: loop {
                parts.push(self.assignment()?);

                if !self.match_any([TokenKind::Comma]) {
                    break 'collect_arguments;
                }
            }
        }

        let end_pos: usize = self.expect(TokenKind::RightBracket)?.span.range.end;

        Ok(Expr::List {
            parts,
            span: Span {
                range: start_pos..end_pos,
                file_id: self.file_id,
            },
        })
    }

    /// Move to the function boundary
    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.previous().kind == TokenKind::Semicolon {
                return;
            }

            use TokenKind::*;
            match self.peek().kind {
                Class | Fun | Var | For | If | While | Print | Return => return,
                _ => (),
            }

            self.advance();
        }
    }

    fn consume(
        &mut self,
        kind: TokenKind,
        err: ParseError,
    ) -> Result<Token<'source>, Diagnostic<FileIdent>> {
        if self.check(kind) {
            Ok(self.advance().clone())
        } else {
            Err(self.error(err))
        }
    }

    fn expect(&mut self, expected: TokenKind) -> Result<Token<'source>, Diagnostic<FileIdent>> {
        self.consume(
            expected,
            ParseError::ExpectedTokenKind {
                got: self.peek().kind,
                expected,
                span: self.peek().span.clone(),
            },
        )
    }
}

fn unescape(input: &str) -> Result<String, Vec<String>> {
    let mut ok = String::new();
    let mut err = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => ok.push('\n'),
                Some('t') => ok.push('\t'),
                Some('r') => ok.push('\r'),
                Some('\\') => ok.push('\\'),
                Some('"') => ok.push('"'),

                Some('x') => {
                    let mut erred = false;
                    let hex: String = chars.by_ref().take(2).collect();
                    if hex.len() != 2 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                        erred = true;
                    }

                    let value = u8::from_str_radix(&hex, 16);

                    if let Ok(val) = value
                        && !erred
                    {
                        ok.push(val as char);
                    } else {
                        err.push(format!("\\x{hex}"));
                    }
                }

                Some('u') => {
                    let mut erred = false;
                    let hex: String = chars.by_ref().take(4).collect();
                    if hex.len() != 4 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                        erred = true;
                    }
                    let character = u32::from_str_radix(&hex, 16)
                        .ok()
                        .and_then(|val| char::from_u32(val));
                    if let Some(c) = character
                        && !erred
                    {
                        ok.push(c);
                    } else {
                        err.push(format!("\\u{hex}"));
                    }
                }

                Some(other) => {
                    // preserve unknown escapes literally
                    ok.push('\\');
                    ok.push(other);
                }

                None => {}
            }
        } else {
            ok.push(c);
        }
    }

    if err.is_empty() { Ok(ok) } else { Err(err) }
}
