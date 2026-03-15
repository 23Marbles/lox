use crate::tree_walk::{
    ast::{Expr, Literal, Stmt},
    error::{
        ErrorPos,
        parse_err::{ParseError, ParseErrorKind},
    },
    token::{Token, TokenKind},
};

pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    current: usize,
    errors: Vec<ParseError>,
}

impl<'a> Iterator for Parser<'a> {
    type Item = Result<Stmt<'a>, ParseError>;

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
            ParseErrorKind::UnexpectedTokenKind(_) | ParseErrorKind::InvalidAssignTarget(_) => (&self.peek()).into(),
            ParseErrorKind::TooManyArguments => (&self.tokens[self.current - 1]).into(),
            ParseErrorKind::ExpectedTokenKind(_) => {
                let prev_pos: ErrorPos = (&self.tokens[self.current - 1]).into();

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

    pub fn parse(&mut self) -> Result<Vec<Stmt<'a>>, Vec<ParseError>> {
        let statements = self.filter_map(|res| res.ok()).collect();

        if self.errors.is_empty() {
            Ok(statements)
        } else {
            Err(self.errors.clone())
        }
    }

    fn declaration(&mut self) -> Result<Stmt<'a>, ParseError> {
        if self.match_any([TokenKind::Var]) {
            self.var_declaration()
        } else {
            self.statement()
        }
    }

    fn var_declaration(&mut self) -> Result<Stmt<'a>, ParseError> {
        let name = self.consume(
            TokenKind::Identifier,
            ParseErrorKind::ExpectedTokenKind(TokenKind::Identifier),
        )?;

        let initializer = if self.match_any([TokenKind::Equal]) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            TokenKind::Semicolon,
            ParseErrorKind::ExpectedTokenKind(TokenKind::Semicolon),
        )?;

        Ok(Stmt::Var { name, initializer })
    }

    fn statement(&mut self) -> Result<Stmt<'a>, ParseError> {
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
        } else {
            self.expression_statement()
        }
    }

    fn for_statement(&mut self) -> Result<Stmt<'a>, ParseError> {
        self.consume(
            TokenKind::LeftParen,
            ParseErrorKind::ExpectedTokenKind(TokenKind::LeftParen),
        )?;

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

        self.consume(
            TokenKind::Semicolon,
            ParseErrorKind::ExpectedTokenKind(TokenKind::Semicolon),
        )?;

        let increment = if !self.check(TokenKind::RightParen) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            TokenKind::RightParen,
            ParseErrorKind::ExpectedTokenKind(TokenKind::RightParen),
        )?;

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

    fn while_statement(&mut self) -> Result<Stmt<'a>, ParseError> {
        self.consume(
            TokenKind::LeftParen,
            ParseErrorKind::ExpectedTokenKind(TokenKind::LeftParen),
        )?;
        let condition = self.expression()?;
        self.consume(
            TokenKind::RightParen,
            ParseErrorKind::ExpectedTokenKind(TokenKind::RightParen),
        )?;
        Ok(Stmt::While {
            condition,
            body: Box::new(self.statement()?),
        })
    }

    fn if_statement(&mut self) -> Result<Stmt<'a>, ParseError> {
        self.consume(
            TokenKind::LeftParen,
            ParseErrorKind::ExpectedTokenKind(TokenKind::LeftParen),
        )?;
        let condition = self.expression()?;
        self.consume(
            TokenKind::RightParen,
            ParseErrorKind::ExpectedTokenKind(TokenKind::RightParen),
        )?;

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

    fn block(&mut self) -> Result<Vec<Stmt<'a>>, ParseError> {
        let mut statements = Vec::new();

        while !self.check(TokenKind::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }

        if self.advance().kind == TokenKind::RightBrace {
            Ok(statements)
        } else {
            Err(self.error(ParseErrorKind::ExpectedTokenKind(TokenKind::RightBrace)))
        }
    }

    fn print_statement(&mut self) -> Result<Stmt<'a>, ParseError> {
        let value = self.expression()?;
        self.consume(
            TokenKind::Semicolon,
            ParseErrorKind::ExpectedTokenKind(TokenKind::Semicolon),
        )?;

        Ok(Stmt::Print(value))
    }

    fn expression_statement(&mut self) -> Result<Stmt<'a>, ParseError> {
        let value = self.expression()?;
        self.consume(
            TokenKind::Semicolon,
            ParseErrorKind::ExpectedTokenKind(TokenKind::Semicolon),
        )?;

        Ok(Stmt::Expression(value))
    }

    fn expression(&mut self) -> Result<Expr<'a>, ParseError> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr<'a>, ParseError> {
        let expr = self.logic_or()?;

        if self.match_any([TokenKind::Equal]) {
            let equals = self.previous();
            let value = self.assignment()?;

            if let Expr::Variable { name, .. } = expr {
                Ok(Expr::Assign {
                    name,
                    position: (&equals).into(),
                    value: Box::new(value),
                })
            } else {
                let err = ParseError::new(
                    ParseErrorKind::InvalidAssignTarget(expr.string()),
                    (&equals).into(),
                );

                self.errors.push(err.clone());

                Err(err)
            }
        } else {
            Ok(expr)
        }
    }

    fn logic_or(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.logic_and()?;

        while self.match_any([TokenKind::Or]) {
            expr = Expr::Logical {
                left: Box::new(expr),
                operator: self.previous(),
                right: Box::new(self.logic_and()?),
            };
        }

        Ok(expr)
    }

    fn logic_and(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.equality()?;

        while self.match_any([TokenKind::And]) {
            expr = Expr::Logical {
                left: Box::new(expr),
                operator: self.previous(),
                right: Box::new(self.equality()?),
            };
        }

        Ok(expr)
    }

    fn equality(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.comparison()?;

        use TokenKind::*;
        while self.match_any([BangEqual, EqualEqual]) {
            // Append each series of operators to the expr recursivley
            expr = Expr::binary(expr, self.previous(), self.comparison()?)
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.term()?;

        use TokenKind::*;
        while self.match_any([Greater, GreaterEqual, Less, LessEqual]) {
            expr = Expr::binary(expr, self.previous(), self.term()?)
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.factor()?;

        use TokenKind::*;
        while self.match_any([Minus, Plus]) {
            expr = Expr::binary(expr, self.previous(), self.factor()?)
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.unary()?;

        use TokenKind::*;
        while self.match_any([Slash, Star]) {
            expr = Expr::binary(expr, self.previous(), self.unary()?)
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr<'a>, ParseError> {
        use TokenKind::*;
        if self.match_any([Bang, Minus]) {
            Ok(Expr::unary(self.previous(), self.unary()?))
        } else {
            self.call()
        }
    }

    fn call(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.primary()?;

        loop {
            if self.match_any([TokenKind::LeftParen]) {
                expr = self.finish_call(expr)?;
            } else {
                break Ok(expr);
            }
        }
    }

    fn finish_call(&mut self, calee: Expr<'a>) -> Result<Expr<'a>, ParseError> {
        let mut args = Vec::new();

        if !self.check(TokenKind::RightParen) {
            loop {
                args.push(self.expression()?);

                if !self.match_any([TokenKind::Comma]) {
                    break;
                }
            }

            if args.len() > u8::MAX as usize {
                let _ = self.error(ParseErrorKind::TooManyArguments);
            }
        }

        let position: ErrorPos = (&self.consume(
            TokenKind::RightParen,
            ParseErrorKind::ExpectedTokenKind(TokenKind::RightParen),
        )?)
            .into();

        Ok(Expr::Call {
            callee: Box::new(calee),
            args,
            position,
        })
    }

    fn primary(&mut self) -> Result<Expr<'a>, ParseError> {
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
                Ok(Expr::Literal(Literal::String(self.previous().lexeme)))
            }
            LeftParen => {
                self.advance();
                let expr = self.expression()?;
                self.consume(
                    RightParen,
                    ParseErrorKind::ExpectedTokenKind(TokenKind::RightParen),
                )?;
                Ok(Expr::Grouping(Box::new(expr)))
            }
            Identifier => {
                let token = self.advance();
                Ok(Expr::Variable {
                    name: token.lexeme,
                    position: (&token).into(),
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

    #[must_use]
    fn consume(&mut self, kind: TokenKind, err: ParseErrorKind) -> Result<Token<'a>, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.error(err))
        }
    }
}
