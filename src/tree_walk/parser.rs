use crate::tree_walk::{
    ast::Expr,
    error::{ParseError, ParseErrorKind},
    token::{Token, TokenKind},
};

pub struct Parser<'a> {
    tokens: Vec<Token<'a>>,
    current: usize,
    errors: Vec<ParseError>,
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self {
            tokens,
            current: 0,
            errors: Vec::new(),
        }
    }

    fn error(&mut self, kind: ParseErrorKind) -> ParseError {
        let errorred_token = self.peek();
        let err = ParseError {
            kind,
            line: errorred_token.line,
            column: errorred_token.column,
        };
        self.errors.push(ParseError {
            kind,
            line: errorred_token.line,
            column: errorred_token.column,
        });
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

    pub fn parse(&mut self) -> Result<Expr<'a>, Vec<ParseError>> {
        match self.expression() {
            Ok(expr) => Ok(expr),
            Err(err) => {
                /*self.synchronize(); */
                Err(vec![err])
            }
        }
    }

    fn expression(&mut self) -> Result<Expr<'a>, ParseError> {
        self.equality()
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
                    None => Err(self.error(ParseErrorKind::UnterminatedGrouping)),
                }
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

            match self.peek().kind {
                TokenKind::Return => return,
                _ => (),
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
