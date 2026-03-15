use std::fmt::Display;

use crate::tree_walk::error::scan_err::{ScanError, ScanErrorKind};

pub struct Scanner<'a> {
    source: &'a str,
    start: usize,
    current: usize,
    line: usize,
    eof: bool,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            start: 0,
            current: 0,
            line: 1,
            eof: false,
        }
    }

    fn lexeme_with_bounds(&self, start: usize, end: usize) -> &'a str {
        &self.source[start..end]
    }

    fn make_token(&self, kind: TokenKind) -> Token<'a> {
        self.make_token_with_slice(kind, self.start, self.current)
    }

    fn make_token_with_slice(&self, kind: TokenKind, start: usize, end: usize) -> Token<'a> {
        Token {
            kind,
            lexeme: self.lexeme_with_bounds(start, end),
            line: self.line,
            column: self.column(start),
        }
    }

    fn error(&self, kind: ScanErrorKind) -> ScanError {
        ScanError::new(self.line, self.column(self.start), kind)
    }

    fn advance(&mut self) -> char {
        let c = self.source.as_bytes()[self.current] as char;
        self.current += 1;
        c
    }

    fn peek(&self) -> char {
        if self.is_at_end() {
            '\0'
        } else {
            self.source.as_bytes()[self.current] as char
        }
    }

    fn peek_next(&self) -> char {
        if self.current + 1 >= self.source.len() {
            '\0'
        } else {
            self.source.as_bytes()[self.current + 1] as char
        }
    }

    fn advance_if(&mut self, func: impl FnOnce(/*next_char*/ char) -> bool) -> bool {
        if func(self.peek()) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn advance_if_next(&mut self, expected: char) -> bool {
        self.advance_if(|c| c == expected)
    }

    fn make_token_if(&mut self, expected: char, kind: TokenKind, other: TokenKind) -> Token<'a> {
        if self.advance_if_next(expected) {
            self.make_token(kind)
        } else {
            self.make_token(other)
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }

    fn column(&self, index: usize) -> usize {
        let line_start = self.source[..index].rfind('\n').map(|i| i + 1).unwrap_or(0);

        self.source[line_start..index].chars().count() + 1
    }

    fn new_line(&mut self) {
        self.line += 1;
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Result<Token<'a>, ScanError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.eof {
            return None;
        } else if self.is_at_end() {
            self.eof = true;
            return Some(Ok(self.make_token(TokenKind::Eof)));
        }

        self.start = self.current;

        Some(Ok('scan: {
            let c = self.advance();

            let current_token_type = match c {
                '(' => break 'scan self.make_token(TokenKind::LeftParen),
                ')' => break 'scan self.make_token(TokenKind::RightParen),
                '{' => break 'scan self.make_token(TokenKind::LeftBrace),
                '}' => break 'scan self.make_token(TokenKind::RightBrace),
                ',' => break 'scan self.make_token(TokenKind::Comma),
                '.' => break 'scan self.make_token(TokenKind::Dot),
                '-' => break 'scan self.make_token(TokenKind::Minus),
                '+' => break 'scan self.make_token(TokenKind::Plus),
                ';' => break 'scan self.make_token(TokenKind::Semicolon),
                '*' => break 'scan self.make_token(TokenKind::Star),
                '!' => break 'scan self.make_token(TokenKind::Bang),
                '>' => TokenKind::Greater,
                '<' => TokenKind::Less,
                '=' => TokenKind::Equal,
                '/' => TokenKind::Slash,
                ' ' | '\t' | '\r' => return self.next(), // Ignore whitespace
                '\n' => {
                    self.new_line();
                    return self.next();
                } // Ignore newlines but increment the line counter
                '"' => TokenKind::String,
                x if (x.is_alphabetic() || x  == '_') => {
                    let mut string = String::new();
                    string.push(x);
                    while self.peek().is_alphanumeric() || self.peek() == '_' {
                        string.push(self.advance());
                    }
                    if let Some(key) = TokenKind::search_keyword_str(&string) {
                        break 'scan self.make_token(key);
                    } else {
                        break 'scan self.make_token(TokenKind::Identifier);
                    }
                }
                x if x.is_ascii_digit() => {
                    while self.peek().is_ascii_digit() {
                        self.advance();
                    }

                    if self.peek() == '.' && self.peek_next().is_ascii_digit() {
                        self.advance();

                        while self.peek().is_ascii_digit() {
                            self.advance();
                        }
                    }

                    break 'scan self.make_token(TokenKind::Number);
                }
                x => return Some(Err(self.error(ScanErrorKind::UnexpectedCharacter(x)))),
            };

            match current_token_type {
                TokenKind::String => {
                    while self.peek() != '"' && !self.is_at_end() {
                        if self.peek() == '\n' {
                            self.new_line();
                        }
                        self.advance();
                    }
                    if self.is_at_end() {
                        return Some(Err(self.error(ScanErrorKind::UnterminatedString)));
                    }
                    self.advance();

                    // Do not include "" in the lexme
                    break 'scan self.make_token_with_slice(
                        TokenKind::String,
                        self.start + 1,
                        self.current - 1,
                    );
                }
                TokenKind::Slash => {
                    if self.advance_if_next('/') {
                        while self.peek() != '\n' && !self.is_at_end() {
                            self.advance();
                        }
                        return self.next();
                    } else {
                        {
                            #[cfg(feature = "inline_comments")]
                            {
                                if self.advance_if_next('*') {
                                    loop {
                                        if !self.is_at_end() {
                                            let c = self.advance();
                                            if c == '*' {
                                                if self.advance_if_next('/') {
                                                    return self.next();
                                                }
                                            } else if c == '\n' {
                                                self.new_line();
                                            }
                                        } else {
                                            return Some(Err(
                                                self.error(ScanErrorKind::UnterminatedComment)
                                            ));
                                        }
                                    }
                                } else {
                                    break 'scan self.make_token(current_token_type);
                                }
                            }
                            #[cfg(not(feature = "inline_comments"))]
                            break 'scan self.make_token(current_token_type);
                        }
                    }
                }
                TokenKind::Bang => {
                    break 'scan self.make_token_if('=', TokenKind::BangEqual, current_token_type);
                }
                TokenKind::Equal => {
                    break 'scan self.make_token_if('=', TokenKind::EqualEqual, current_token_type);
                }
                TokenKind::Greater => {
                    break 'scan self.make_token_if(
                        '=',
                        TokenKind::GreaterEqual,
                        current_token_type,
                    );
                }
                TokenKind::Less => {
                    break 'scan self.make_token_if('=', TokenKind::LessEqual, current_token_type);
                }
                _ => unreachable!(),
            }
        }))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Token<'a> {
    pub kind: TokenKind,
    pub lexeme: &'a str,
    pub line: usize,
    pub column: usize,
}

impl<'a> Display for Token<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            TokenKind::String => write!(f, "\"{}\"", self.lexeme),
            TokenKind::Eof => write!(f, "eof"),
            _ => write!(f, "{}", self.lexeme),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenKind {
    // Single-character tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,

    // One or two character tokens.
    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    // Literals.
    Identifier,
    String,
    Number,

    // Keywords.
    And,
    Class,
    Else,
    False,
    Fun,
    For,
    If,
    Nil,
    Or,
    Print,
    Return,
    Super,
    This,
    True,
    Var,
    While,
    #[cfg(feature = "break")]
    Break,

    Eof,
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TokenKind::*;
        match self {
            And => write!(f, "and"),
            Class => write!(f, "class"),
            Else => write!(f, "else"),
            False => write!(f, "false"),
            Fun => write!(f, "fun"),
            For => write!(f, "for"),
            If => write!(f, "if"),
            Nil => write!(f, "nil"),
            Or => write!(f, "or"),
            Print => write!(f, "print"),
            Return => write!(f, "return"),
            Super => write!(f, "super"),
            This => write!(f, "this"),
            True => write!(f, "true"),
            Var => write!(f, "var"),
            While => write!(f, "while"),
            LeftParen => write!(f, "("),
            RightParen => write!(f, ")"),
            LeftBrace => write!(f, "{{"),
            RightBrace => write!(f, "}}"),
            Comma => write!(f, ","),
            Dot => write!(f, "."),
            Minus => write!(f, "-"),
            Plus => write!(f, "+"),
            Semicolon => write!(f, ";"),
            Slash => write!(f, "/"),
            Star => write!(f, "*"),
            Bang => write!(f, "!"),
            BangEqual => write!(f, "!="),
            Equal => write!(f, "="),
            EqualEqual => write!(f, "=="),
            Greater => write!(f, ">"),
            GreaterEqual => write!(f, ">="),
            Less => write!(f, "<"),
            LessEqual => write!(f, "<="),
            Identifier => write!(f, "ident"),
            String => write!(f, "\"string\""),
            Number => write!(f, "num"),
            Eof => write!(f, "\0"),
            #[cfg(feature = "break")]
            Break => write!(f, "break"),
        }
    }
}

impl TokenKind {
    fn search_keyword_str(string: &str) -> Option<Self> {
        use TokenKind::*;
        match string {
            "and" => Some(And),
            "class" => Some(Class),
            "else" => Some(Else),
            "false" => Some(False),
            "fun" => Some(Fun),
            "for" => Some(For),
            "if" => Some(If),
            "nil" => Some(Nil),
            "or" => Some(Or),
            "print" => Some(Print),
            "return" => Some(Return),
            "super" => Some(Super),
            "this" => Some(This),
            "true" => Some(True),
            "var" => Some(Var),
            "while" => Some(While),
            #[cfg(feature = "break")]
            "break" => Some(Break),
            _ => None,
        }
    }
}
