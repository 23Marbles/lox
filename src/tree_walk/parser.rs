use crate::tree_walk::{
    ast::{Expr, Literal, Stmt},
    error::{
        ErrorPos,
        parse_err::{Named, ParseError, ParseErrorKind},
    },
    token::{Token, TokenKind},
};

pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    current: usize,
    errors: Vec<ParseError>,
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<Stmt, ParseError>;

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

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
        }
    }

    #[must_use]
    fn error(&mut self, kind: ParseErrorKind) -> ParseError {
        let errored_pos: ErrorPos = match kind {
            ParseErrorKind::UnexpectedTokenKind(_) | ParseErrorKind::InvalidAssignTarget(_) => {
                // The current tokens full position
                self.peek().into()
            }
            ParseErrorKind::TooManyArguments => self.tokens[self.current - 1].into(),
            ParseErrorKind::ExpectedTokenKind(_) | ParseErrorKind::ExpectedName(_) => {
                // The first charcter after the current tokens position
                let prev_pos: ErrorPos = self.tokens[self.current - 1].into();

                prev_pos.end()
            }
        };

        let err = ParseError::new(kind, errored_pos);

        self.errors.push(err.clone());

        err
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn peek(&self) -> Token<'a> {
        self.tokens[self.current]
    }

    fn previous(&self) -> Token<'a> {
        self.tokens[self.current - 1]
    }

    fn check(&self, kind: TokenKind) -> bool {
        if self.is_at_end() {
            false
        } else {
            self.peek().kind == kind
        }
    }

    fn advance(&mut self) -> Token<'a> {
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

    pub fn parse(&mut self) -> Result<Vec<Stmt>, Vec<ParseError>> {
        let statements = self.filter_map(|res| res.ok()).collect();

        if self.errors.is_empty() {
            Ok(statements)
        } else {
            Err(self.errors.clone())
        }
    }

    fn declaration(&mut self) -> Result<Stmt, ParseError> {
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

    fn class_declaration(&mut self) -> Result<Stmt, ParseError> {
        let token = self.consume(
            TokenKind::Identifier,
            ParseErrorKind::ExpectedName(Named::Class),
        )?;
        self.expect(TokenKind::LeftBrace)?;

        let mut methods: Vec<Stmt> = Vec::new();

        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            methods.push(self.fun_declaration(Named::Method)?);
        }

        let end_pos: ErrorPos = self.expect(TokenKind::RightBrace)?.into();

        Ok(Stmt::Class {
            name: token.lexeme.to_owned(),
            methods,
            pos: ErrorPos::from(token).span_to(end_pos),
        })
    }

    fn fun_declaration(&mut self, kind: Named) -> Result<Stmt, ParseError> {
        let name = self
            .consume(TokenKind::Identifier, ParseErrorKind::ExpectedName(kind))?
            .lexeme;

        let start_pos: ErrorPos = self.expect(TokenKind::LeftParen)?.into();

        let mut parameters: Vec<&'a str> = Vec::new();

        if !self.check(TokenKind::RightParen) {
            'collect_arguments: loop {
                if parameters.len() >= u8::MAX as usize {
                    let _ = self.error(ParseErrorKind::TooManyArguments);
                }

                parameters.push(
                    self.consume(
                        TokenKind::Identifier,
                        ParseErrorKind::ExpectedName(Named::Parameter),
                    )?
                    .lexeme,
                );

                if !self.match_any([TokenKind::Comma]) {
                    break 'collect_arguments;
                }
            }
        }

        let end_pos: ErrorPos = self.expect(TokenKind::RightParen)?.into();
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
            pos: start_pos.span_to(end_pos),
        })
    }

    fn var_declaration(&mut self) -> Result<Stmt, ParseError> {
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
            position: token.into(),
        })
    }

    fn statement(&mut self) -> Result<Stmt, ParseError> {
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

    fn return_statement(&mut self) -> Result<Stmt, ParseError> {
        let start_pos: ErrorPos = self.previous().into();

        let value = if !self.check(TokenKind::Semicolon) {
            Some(self.expression()?)
        } else {
            None
        };

        let end_pos: ErrorPos = self
            .expect(TokenKind::Semicolon)?.into();

        Ok(Stmt::Return {
            value,
            pos: start_pos.span_to(end_pos),
        })
    }

    fn for_statement(&mut self) -> Result<Stmt, ParseError> {
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

    fn while_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LeftParen)?;
        let condition = self.expression()?;
        self.expect(TokenKind::RightParen)?;
        Ok(Stmt::While {
            condition,
            body: Box::new(self.statement()?),
        })
    }

    fn if_statement(&mut self) -> Result<Stmt, ParseError> {
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

    fn block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();

        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }

        if self.advance().kind == TokenKind::RightBrace {
            Ok(statements)
        } else {
            Err(self.error(
                ParseErrorKind::ExpectedTokenKind(TokenKind::RightBrace)
            ))
        }
    }

    fn print_statement(&mut self) -> Result<Stmt, ParseError> {
        let value = self.expression()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Print(value))
    }

    fn expression_statement(&mut self) -> Result<Stmt, ParseError> {
        let value = self.expression()?;
        self.expect(TokenKind::Semicolon)?;

        Ok(Stmt::Expression(value))
    }

    fn expression(&mut self) -> Result<Expr, ParseError> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr, ParseError> {
        let expr = self.logic_or()?;

        if self.match_any([TokenKind::Equal]) {
            let equals = self.previous();
            let value = self.assignment()?;

            if let Expr::Variable { name, .. } = expr {
                Ok(Expr::Assign {
                    name,
                    position: equals.into(),
                    value: Box::new(value),
                    distance: None,
                })
            } else if let Expr::Get { object, name, pos } = expr {
                Ok(Expr::Set {
                    object,
                    name,
                    value: Box::new(value),
                    pos,
                })
            } else {
                let err = ParseError::new(
                    ParseErrorKind::InvalidAssignTarget(expr.string()),
                    equals.into(),
                );

                self.errors.push(err.clone());

                Err(err)
            }
        } else {
            Ok(expr)
        }
    }

    fn logic_or(&mut self) -> Result<Expr, ParseError> {
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

    fn logic_and(&mut self) -> Result<Expr, ParseError> {
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

    fn equality(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.comparison()?;

        use TokenKind::*;
        while self.match_any([BangEqual, EqualEqual]) {
            // Append each series of operators to the expr recursivley
            expr = Expr::binary(expr, self.previous(), self.comparison()?)
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.term()?;

        use TokenKind::*;
        while self.match_any([Greater, GreaterEqual, Less, LessEqual]) {
            expr = Expr::binary(expr, self.previous(), self.term()?)
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.factor()?;

        use TokenKind::*;
        while self.match_any([Minus, Plus]) {
            expr = Expr::binary(expr, self.previous(), self.factor()?)
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.unary()?;

        use TokenKind::*;
        while self.match_any([Slash, Star]) {
            expr = Expr::binary(expr, self.previous(), self.unary()?)
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        use TokenKind::*;
        if self.match_any([Bang, Minus]) {
            Ok(Expr::unary(self.previous(), self.unary()?))
        } else {
            self.call()
        }
    }

    fn call(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.primary()?;

        loop {
            if self.match_any([TokenKind::LeftParen]) {
                expr = self.finish_call(
                    expr,
                    match self.tokens[self.current - 2] {
                        // No panicking (Just trust me bro)
                        ident if ident.kind == TokenKind::Identifier => ident.into(),
                        _ => self.previous().into(),
                    },
                )?;
            } else if self.match_any([TokenKind::Dot]) {
                let token = self.consume(
                    TokenKind::Identifier,
                    ParseErrorKind::ExpectedName(Named::Property),
                )?;
                expr = Expr::Get {
                    object: Box::new(expr),
                    name: token.lexeme.to_owned(),
                    pos: token.into(),
                };
            } else {
                break Ok(expr);
            }
        }
    }

    fn finish_call(&mut self, calee: Expr, start_pos: ErrorPos) -> Result<Expr, ParseError> {
        let mut args = Vec::new();

        if !self.check(TokenKind::RightParen) {
            loop {
                if args.len() > u8::MAX as usize {
                    let _ = self.error(ParseErrorKind::TooManyArguments);
                }
                args.push(self.expression()?);

                if !self.match_any([TokenKind::Comma]) {
                    break;
                }
            }
        }

        let position = start_pos.span_to((&self.expect(TokenKind::RightParen)?).into());

        Ok(Expr::Call {
            callee: Box::new(calee),
            args,
            position,
        })
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
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
                self.advance();
                Ok(Expr::Literal(Literal::Number(
                    self.previous().lexeme.parse().unwrap(),
                )))
            }
            String => {
                self.advance();
                Ok(Expr::Literal(Literal::String(
                    self.previous().lexeme.to_owned(),
                )))
            }
            LeftParen => {
                self.advance();
                let expr = self.expression()?;
                self.expect(RightParen)?;
                Ok(Expr::Grouping(Box::new(expr)))
            }
            This => {
                let pos = self.advance().into();
                Ok(Expr::This {
                    distance: None,
                    pos,
                })
            }
            Identifier => {
                let token = self.advance();
                Ok(Expr::Variable {
                    name: token.lexeme.to_owned(),
                    position: token.into(),
                    distance: None,
                })
            }
            k => Err(self.error(ParseErrorKind::UnexpectedTokenKind(k))),
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

    fn consume(&mut self, kind: TokenKind, err: ParseErrorKind) -> Result<Token<'a>, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.error(err))
        }
    }

    fn expect(&mut self, expected: TokenKind) -> Result<Token<'a>, ParseError> {
        self.consume(expected, ParseErrorKind::ExpectedTokenKind(expected))
    }
}
