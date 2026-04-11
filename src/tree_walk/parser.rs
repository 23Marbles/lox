use std::ops::Range;

use codespan_reporting::diagnostic::Diagnostic;

use crate::tree_walk::{
    ast::{Literal, expr::Expr, stmt::Stmt},
    error::{
        ToCodespan,
        parse_err::{Named, ParseError},
    },
    token::{Token, TokenKind},
};

pub struct Parser<'source> {
    tokens: Vec<Token<'source>>,
    current: usize,
    errors: Vec<Diagnostic<usize>>,
    file_id: usize,
}

impl<'source> Iterator for Parser<'source> {
    type Item = Result<Stmt, Diagnostic<usize>>;

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
    pub fn new(tokens: Vec<Token<'source>>, file_id: usize) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
            file_id,
        }
    }

    #[must_use]
    fn error(&mut self, kind: ParseError) -> Diagnostic<usize> {
        let err = kind.to_codespan(self.file_id);

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

    pub fn parse(&mut self) -> crate::Result<Vec<Stmt>> {
        let statements = self
            .filter_map(|res: Result<Stmt, Diagnostic<usize>>| res.ok())
            .collect();

        if self.errors.is_empty() {
            Ok(statements)
        } else {
            Err(self.errors.clone())
        }
    }

    fn declaration(&mut self) -> Result<Stmt, Diagnostic<usize>> {
        if self.match_any([TokenKind::Var]) {
            self.var_declaration()
        } else if self.match_any([TokenKind::Fun]) {
            self.fun_declaration(Named::Function)
        } else if self.match_any([TokenKind::Class]) {
            self.class_declaration()
        } else {
            self.statement()
        }
    }

    fn class_declaration(&mut self) -> Result<Stmt, Diagnostic<usize>> {
        let name_token = self.consume(
            TokenKind::Identifier,
            ParseError::ExpectedName {
                name: Named::Class,
                after: self.previous().kind,
                span: match self.peek_ahead() {
                    Some(token) => token.span.clone(),
                    None => self.peek().span.clone(), // Fallback to current token span
                },
                prev_span: self.previous().span.clone(),
            },
        )?;

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

        self.expect(TokenKind::LeftBrace)?;

        let mut methods: Vec<Stmt> = Vec::new();

        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            methods.push(self.fun_declaration(Named::Method)?);
        }

        self.expect(TokenKind::RightBrace)?;

        Ok(Stmt::Class {
            name: name_token.lexeme.to_owned(),
            methods,
            super_class,
            span: name_token.span,
        })
    }

    fn fun_declaration(&mut self, kind: Named) -> Result<Stmt, Diagnostic<usize>> {
        let name = self
            .consume(
                TokenKind::Identifier,
                ParseError::ExpectedName {
                    name: kind,
                    after: self.previous().kind,
                    prev_span: self.previous().span.clone(),
                    span: self
                        .peek_ahead()
                        .expect("Function decl at end of token stream")
                        .span
                        .clone(),
                },
            )?
            .lexeme;

        let start_pos: usize = self.expect(TokenKind::LeftParen)?.span.start;

        let mut parameters: Vec<&'source str> = Vec::new();

        let mut offending_spans: Vec<Range<usize>> = Vec::new();

        if !self.check(TokenKind::RightParen) {
            'collect_arguments: loop {
                parameters.push(
                    self.consume(
                        TokenKind::Identifier,
                        ParseError::ExpectedName {
                            name: Named::Parameter,
                            after: self.previous().kind,
                            prev_span: self.previous().span.clone(),
                            span: self
                                .peek_ahead()
                                .expect("Parameter at end of token stream")
                                .span
                                .clone(),
                        },
                    )?
                    .lexeme,
                );

                if parameters.len() > u8::MAX as usize {
                    offending_spans.push(self.previous().span.clone());
                }

                if !self.match_any([TokenKind::Comma]) {
                    break 'collect_arguments;
                }
            }
        }

        let end_pos: usize = self.expect(TokenKind::RightParen)?.span.end;

        if !offending_spans.is_empty() {
            let _ = self.error(ParseError::TooManyArguments {
                got: u8::MAX as usize + offending_spans.len(),
                spans: offending_spans,
            });
        }

        self.expect(TokenKind::LeftBrace)?;

        let body = self.block()?;

        Ok(Stmt::Function {
            name: name.to_owned(),
            parameters: parameters
                .iter()
                .cloned()
                .map(|param| param.to_owned())
                .collect(),
            body,
            span: Range {
                start: start_pos,
                end: end_pos,
            },
        })
    }

    fn var_declaration(&mut self) -> Result<Stmt, Diagnostic<usize>> {
        let token = self.expect(TokenKind::Identifier)?;

        let initializer = if self.match_any([TokenKind::Equal]) {
            Some(self.expression()?)
        } else {
            None
        };

        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Var {
            name: token.lexeme.to_owned(),
            initializer,
            span: token.span,
        })
    }

    fn statement(&mut self) -> Result<Stmt, Diagnostic<usize>> {
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
        } else {
            self.expression_statement()
        }
    }

    fn return_statement(&mut self) -> Result<Stmt, Diagnostic<usize>> {
        let start_pos: usize = self.previous().span.start;

        let value = if !self.check(TokenKind::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };

        let end_pos: usize = self.expect(TokenKind::Semicolon)?.span.end;

        Ok(Stmt::Return {
            value,
            span: Range {
                start: start_pos,
                end: end_pos,
            },
        })
    }

    fn for_statement(&mut self) -> Result<Stmt, Diagnostic<usize>> {
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

        if let Some(inc) = increment {
            body = Stmt::Block(vec![body, Stmt::Expression(inc)])
        }

        body = Stmt::While {
            condition: condition.unwrap_or(Expr::Literal(Literal::True)),
            body: Box::new(body),
        };

        if let Some(init) = initializer {
            body = Stmt::Block(vec![init, body])
        }

        Ok(body)
    }

    fn while_statement(&mut self) -> Result<Stmt, Diagnostic<usize>> {
        self.expect(TokenKind::LeftParen)?;
        let condition = self.expression()?;
        self.expect(TokenKind::RightParen)?;
        Ok(Stmt::While {
            condition,
            body: Box::new(self.statement()?),
        })
    }

    fn if_statement(&mut self) -> Result<Stmt, Diagnostic<usize>> {
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

    fn block(&mut self) -> Result<Vec<Stmt>, Diagnostic<usize>> {
        let mut statements = Vec::new();

        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }

        self.expect(TokenKind::RightBrace)?;

        Ok(statements)
    }

    fn print_statement(&mut self) -> Result<Stmt, Diagnostic<usize>> {
        let value = self.expression()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Print(value))
    }

    fn expression_statement(&mut self) -> Result<Stmt, Diagnostic<usize>> {
        let value = self.expression()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Expression(value))
    }

    fn expression(&mut self) -> Result<Expr, Diagnostic<usize>> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr, Diagnostic<usize>> {
        let expr = self.logic_or()?;

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
            } else if let Expr::Get { object, name, span } = expr {
                Ok(Expr::Set {
                    object,
                    name,
                    value: Box::new(value),
                    span,
                })
            } else {
                let kind = ParseError::InvalidAssignTarget {
                    target: expr.to_string(),
                    span: equals.span,
                };

                let err = kind.to_codespan(self.file_id);

                self.errors.push(err.clone());

                Err(err)
            }
        } else {
            Ok(expr)
        }
    }

    fn logic_or(&mut self) -> Result<Expr, Diagnostic<usize>> {
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

    fn logic_and(&mut self) -> Result<Expr, Diagnostic<usize>> {
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

    fn equality(&mut self) -> Result<Expr, Diagnostic<usize>> {
        let mut expr = self.comparison()?;

        use TokenKind::*;
        while self.match_any([BangEqual, EqualEqual]) {
            // Append each series of operators to the expr recursivley
            expr = Expr::binary(expr, self.previous().clone(), self.comparison()?)
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, Diagnostic<usize>> {
        let mut expr = self.term()?;

        use TokenKind::*;
        while self.match_any([Greater, GreaterEqual, Less, LessEqual]) {
            expr = Expr::binary(expr, self.previous().clone(), self.term()?)
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, Diagnostic<usize>> {
        let mut expr = self.factor()?;

        use TokenKind::*;
        while self.match_any([Minus, Plus]) {
            expr = Expr::binary(expr, self.previous().clone(), self.factor()?)
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, Diagnostic<usize>> {
        let mut expr = self.unary()?;

        use TokenKind::*;
        while self.match_any([Slash, Star]) {
            expr = Expr::binary(expr, self.previous().clone(), self.unary()?)
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, Diagnostic<usize>> {
        use TokenKind::*;
        if self.match_any([Bang, Minus]) {
            Ok(Expr::unary(self.previous().clone(), self.unary()?))
        } else {
            self.call()
        }
    }

    fn call(&mut self) -> Result<Expr, Diagnostic<usize>> {
        let mut expr = self.primary()?;

        loop {
            if self.match_any([TokenKind::LeftParen]) {
                expr = self.finish_call(
                    expr,
                    match &self.tokens[self.current - 2] {
                        // No panicking (Just trust me bro)
                        ident if ident.kind == TokenKind::Identifier => ident.span.start,
                        _ => self.previous().span.start,
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
                    name: token.lexeme.to_owned(),
                    span: token.span,
                };
            } else {
                break Ok(expr);
            }
        }
    }

    fn finish_call(&mut self, calee: Expr, start_pos: usize) -> Result<Expr, Diagnostic<usize>> {
        let mut args = Vec::new();

        let mut offending_spans: Vec<Range<usize>> = Vec::new();

        if !self.check(TokenKind::RightParen) {
            loop {
                args.push(self.expression()?);

                if args.len() > u8::MAX as usize {
                    offending_spans.push(self.previous().span.clone());
                }

                if !self.match_any([TokenKind::Comma]) {
                    break;
                }
            }
        }

        let end_pos: usize = self.expect(TokenKind::RightParen)?.span.end;

        if !offending_spans.is_empty() {
            let _ = self.error(ParseError::TooManyArguments {
                got: u8::MAX as usize + offending_spans.len(),
                spans: offending_spans,
            });
        }

        let span = Range {
            start: start_pos,
            end: end_pos,
        };

        Ok(Expr::Call {
            callee: Box::new(calee),
            args,
            span,
        })
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic<usize>> {
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
                self.advance().lexeme.to_owned(),
            ))),
            LeftParen => {
                self.advance();
                let expr = self.expression()?;
                self.expect(RightParen)?;
                Ok(Expr::Grouping(Box::new(expr)))
            }
            Super => {
                let start_pos = self.advance().span.start;

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
                    span: Range {
                        start: start_pos,
                        end: ident_token.span.end,
                    },
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
            k => Err(self.error(ParseError::UnexpectedTokenKind {
                got: k,
                expected: [
                    True, False, Nil, Number, String, LeftParen, Super, This, Identifier,
                ]
                .to_vec(),
                span: self.peek().span.clone(),
            })),
        }
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
    ) -> Result<Token<'source>, Diagnostic<usize>> {
        if self.check(kind) {
            Ok(self.advance().clone())
        } else {
            Err(self.error(err))
        }
    }

    fn expect(&mut self, expected: TokenKind) -> Result<Token<'source>, Diagnostic<usize>> {
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
