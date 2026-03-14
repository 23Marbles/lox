use crate::tree_walk::{
    ast::{Expr, Stmt},
    error::parse_err::{ParseError, ParseErrorKind},
    token::{Token, TokenKind},
};

pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    current: usize,
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
        Self { tokens, current: 0 }
    }

    fn error(&mut self, kind: ParseErrorKind) -> ParseError {
        let errorred_token = self.peek();
        let column = match kind {
            ParseErrorKind::UnexpectedTokenKind(_) | ParseErrorKind::InvalidAssignTarget(_) => {
                errorred_token.column
            }
            ParseErrorKind::ExpectedTokenKind(_) => {
                errorred_token.column + errorred_token.lexeme.len()
            }
        };

        ParseError::new(kind, errorred_token.line, column)
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
        let (statements, errors) =
            self.fold((Vec::new(), Vec::new()), |(mut stmts, mut errs), res| {
                match res {
                    Ok(s) => stmts.push(s),
                    Err(e) => errs.push(e),
                }
                (stmts, errs)
            });

        if errors.is_empty() {
            Ok(statements)
        } else {
            Err(errors)
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
        let name = self
            .consume(TokenKind::Identifier)
            .ok_or(self.error(ParseErrorKind::ExpectedTokenKind(TokenKind::Identifier)))?;

        let initializer;

        if self.match_any([TokenKind::Equal]) {
            initializer = Some(self.expression()?);
        } else {
            initializer = None
        }

        self.consume(TokenKind::Semicolon)
            .ok_or(self.error(ParseErrorKind::ExpectedTokenKind(TokenKind::Semicolon)))?;

        Ok(Stmt::Var { name, initializer })
    }

    fn statement(&mut self) -> Result<Stmt<'a>, ParseError> {
        use TokenKind::*;

        if self.match_any([Print]) {
            self.print_statement()
        } else if self.match_any([LeftBrace]) {
            Ok(Stmt::Block(self.block()?))
        } else {
            self.expression_statement()
        }
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
        if self.consume(TokenKind::Semicolon).is_none() {
            Err(self.error(ParseErrorKind::ExpectedTokenKind(TokenKind::Semicolon)))
        } else {
            Ok(Stmt::Print(value))
        }
    }

    fn expression_statement(&mut self) -> Result<Stmt<'a>, ParseError> {
        let value = self.expression()?;
        if self.consume(TokenKind::Semicolon).is_none() {
            Err(self.error(ParseErrorKind::ExpectedTokenKind(TokenKind::Semicolon)))
        } else {
            Ok(Stmt::Expression(value))
        }
    }

    fn expression(&mut self) -> Result<Expr<'a>, ParseError> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<Expr<'a>, ParseError> {
        let expr = self.equality()?;

        if self.match_any([TokenKind::Equal]) {
            let equals = self.previous();
            let value = self.assignment()?;

            if let Expr::Variable(name) = expr {
                Ok(Expr::Assign {
                    name,
                    value: Box::new(value),
                })
            } else {
                Err(ParseError::new(
                    ParseErrorKind::InvalidAssignTarget(expr.string()),
                    equals.line,
                    equals.column,
                ))
            }
        } else {
            Ok(expr)
        }
    }

    fn equality(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.comparison()?;

        use TokenKind::*;
        while self.match_any([BangEqual, EqualEqual]) {
            let operator: Token<'a> = self.previous();
            let right = self.comparison()?;
            // Append each series of operators to the expr recursivley
            expr = Expr::binary(expr, operator, right)
        }

        Ok(expr)
    }

    fn comparison(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.term()?;

        use TokenKind::*;
        while self.match_any([Greater, GreaterEqual, Less, LessEqual]) {
            let operator = self.previous();
            let right = self.term()?;
            expr = Expr::binary(expr, operator, right)
        }

        Ok(expr)
    }

    fn term(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.factor()?;

        use TokenKind::*;
        while self.match_any([Minus, Plus]) {
            let operator = self.previous();
            let right = self.factor()?;
            expr = Expr::binary(expr, operator, right)
        }

        Ok(expr)
    }

    fn factor(&mut self) -> Result<Expr<'a>, ParseError> {
        let mut expr = self.unary()?;

        use TokenKind::*;
        while self.match_any([Slash, Star]) {
            let operator = self.previous();
            let right = self.unary()?;
            expr = Expr::binary(expr, operator, right)
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr<'a>, ParseError> {
        use TokenKind::*;
        if self.match_any([Bang, Minus]) {
            let operator = self.previous();
            let right = self.unary()?;
            Ok(Expr::unary(operator, right))
        } else {
            self.primary()
        }
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
                match self.consume(RightParen) {
                    Some(_) => Ok(Expr::Grouping(Box::new(expr))),
                    None => {
                        Err(self.error(ParseErrorKind::ExpectedTokenKind(TokenKind::RightParen)))
                    }
                }
            }
            Identifier => Ok(Expr::Variable(self.advance().lexeme)),
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

            if self.peek().kind == TokenKind::Return {
                return;
            }

            self.advance();
        }
    }

    fn consume(&mut self, kind: TokenKind) -> Option<Token<'a>> {
        if self.check(kind) {
            Some(self.advance())
        } else {
            None
        }
    }
}
