//! Token types for Lox source code.

use core::fmt::Display;

use codespan_reporting::diagnostic::{Diagnostic, Label, LabelStyle};

use crate::{
    Span,
    error::{ToErrorCode as _, scan_err::ScanErrorKind},
    lox_files::FileIdent,
};

#[derive(Debug, Clone)]
/// A scanner that tokenizes a source string.
pub struct Scanner<'source> {
    file_id: FileIdent,
    source: &'source str,
    start: usize,
    current: usize,
    eof: bool,
}

fn normalize_char(c: char) -> char {
    match c {
        '；' => ';',
        '（' => '(',
        '）' => ')',
        '｛' => '{',
        '｝' => '}',
        _ => c,
    }
}

impl<'source> Scanner<'source> {
    /// Creates a new scanner with the given source string and file id.
    pub fn new(source: &'source str, file_id: FileIdent) -> Self {
        Self {
            file_id,
            source,
            start: 0,
            current: 0,
            eof: false,
        }
    }

    /// Scans the source string and returns a vector of tokens or a vector of errors.
    pub fn scan_tokens(&mut self) -> crate::Result<Vec<Token<'source>>> {
        let mut errors: Vec<Diagnostic<FileIdent>> = Vec::new();

        let tokens: Vec<Token<'_>> = self
            .filter_map(|v| match v {
                Ok(t) => Some(t),
                Err(e) => {
                    errors.push(e);
                    None
                }
            })
            .collect();

        if errors.is_empty() {
            Ok(tokens)
        } else {
            Err(errors)
        }
    }

    fn lexeme_with_bounds(&self, start: usize, end: usize) -> &'source str {
        &self.source[start..end]
    }

    fn make_token(&self, kind: TokenKind) -> Token<'source> {
        self.make_token_with_slice(kind, self.start, self.current)
    }

    fn make_token_with_slice(&self, kind: TokenKind, start: usize, end: usize) -> Token<'source> {
        Token {
            kind,
            lexeme: self.lexeme_with_bounds(start, end),
            span: Span::new(start..end, self.file_id),
        }
    }

    fn error(&self, kind: ScanErrorKind) -> Diagnostic<FileIdent> {
        Diagnostic::error()
            .with_code(kind.error_code())
            .with_message(kind.to_string())
            .with_label(Label::new(
                LabelStyle::Primary,
                self.file_id,
                self.start..self.current,
            ))
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.source[self.current..].chars().next()?;
        self.current += ch.len_utf8(); // IMPORTANT for UTF-8
        Some(normalize_char(ch))
    }

    fn peek(&self) -> char {
        self.source[self.current..]
            .chars()
            .next()
            .map(normalize_char)
            .unwrap_or('\0')
    }

    fn peek_next(&self) -> char {
        let mut iter = self.source[self.current..].chars();
        iter.next();
        iter.next().map(normalize_char).unwrap_or('\0')
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

    fn make_token_if(
        &mut self,
        expected: char,
        kind: TokenKind,
        other: TokenKind,
    ) -> Token<'source> {
        if self.advance_if_next(expected) {
            self.make_token(kind)
        } else {
            self.make_token(other)
        }
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.source.len()
    }
}

impl<'source> Iterator for Scanner<'source> {
    type Item = Result<Token<'source>, Diagnostic<FileIdent>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.eof {
            return None;
        } else if self.is_at_end() {
            self.eof = true;
            return Some(Ok(Token {
                kind: TokenKind::Eof,
                lexeme: "\0",
                span: Span::new(self.current..self.current, self.file_id),
            }));
        }

        self.start = self.current;

        Some(Ok('scan: {
            let c = self.advance();

            let current_token_type = match c? {
                '(' => break 'scan self.make_token(TokenKind::LeftParen),
                ')' => break 'scan self.make_token(TokenKind::RightParen),
                '{' => break 'scan self.make_token(TokenKind::LeftBrace),
                '}' => break 'scan self.make_token(TokenKind::RightBrace),
                '[' => break 'scan self.make_token(TokenKind::LeftBracket),
                ']' => break 'scan self.make_token(TokenKind::RightBracket),
                ',' => break 'scan self.make_token(TokenKind::Comma),
                '.' => break 'scan self.make_token(TokenKind::Dot),
                '-' => break 'scan self.make_token(TokenKind::Minus),
                '+' => break 'scan self.make_token(TokenKind::Plus),
                ';' => break 'scan self.make_token(TokenKind::Semicolon),
                '*' => break 'scan self.make_token(TokenKind::Star),
                '%' => break 'scan self.make_token(TokenKind::Percent),
                '?' => break 'scan self.make_token(TokenKind::Question),
                ':' => break 'scan self.make_token(TokenKind::Colon),
                '!' => TokenKind::Bang,
                '>' => TokenKind::Greater,
                '<' => TokenKind::Less,
                '=' => TokenKind::Equal,
                '/' => TokenKind::Slash,
                ' ' | '\t' | '\r' | '\n' => return self.next(), // Ignore whitespace and newlines
                '"' => TokenKind::String,
                x if (x.is_alphabetic() || x == '_') => {
                    let mut string = String::new();
                    string.push(x);
                    while self.peek().is_alphanumeric() || self.peek() == '_' {
                        string.push(
                            self.advance()
                                .ok_or(self.error(ScanErrorKind::UnterminatedString))
                                .ok()?,
                        );
                    }
                    if let Some(keyword) = TokenKind::search_keyword_str(&string) {
                        break 'scan self.make_token(keyword);
                    } else if &string == "_" {
                        break 'scan self.make_token(TokenKind::UnderScore);
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
                    loop {
                        match self.advance() {
                            Some('"') => break,
                            Some('\\') => {
                                if self.advance().is_none() {
                                    return Some(
                                        Err(self.error(ScanErrorKind::UnterminatedString)),
                                    );
                                }
                            }
                            None => {
                                return Some(Err(self.error(ScanErrorKind::UnterminatedString)));
                            }
                            _ => {}
                        }
                    }

                    // Do not include the quotes in the lexme, but include the quotes in the span
                    let mut token = self.make_token_with_slice(
                        TokenKind::String,
                        self.start + 1,
                        self.current - 1,
                    );
                    token.span.range.end += 1;
                    token.span.range.start -= 1;
                    break 'scan token;
                }
                TokenKind::Slash => {
                    if self.advance_if_next('/') {
                        while self.peek() != '\n' && !self.is_at_end() {
                            self.advance();
                        }
                        return self.next();
                    } else {
                        {
                            {
                                if self.advance_if_next('*') {
                                    loop {
                                        if !self.is_at_end() {
                                            let c = self.advance();
                                            if c == Some('*') {
                                                if self.advance_if_next('/') {
                                                    return self.next();
                                                }
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

#[derive(Debug, Clone, PartialEq)]
/// A token with a kind, lexeme, and span in the source code.
pub struct Token<'a> {
    /// The kind of the token.
    pub kind: TokenKind,
    /// The lexeme of the token tied to the source code.
    pub lexeme: &'a str,
    /// The span of the token in the source code.
    pub span: Span,
}

impl<'a> Display for Token<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.kind {
            TokenKind::String => write!(f, "\"{}\"", self.lexeme),
            TokenKind::Eof => write!(f, "eof"),
            _ => write!(f, "{}", self.lexeme),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// The kind of a token.
#[expect(missing_docs)]
pub enum TokenKind {
    // Single-character tokens.
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Question,
    Colon,
    Comma,
    Dot,
    Minus,
    Plus,
    Semicolon,
    Slash,
    Star,
    Percent,
    UnderScore,

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
    NaN,
    INF,
    Break,

    Eof,
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
            String => write!(f, "\""),
            Number => write!(f, "num"),
            Eof => write!(f, "\0"),
            Break => write!(f, "break"),
            NaN => write!(f, "NaN"),
            INF => write!(f, "INF"),
            Percent => write!(f, "%"),
            LeftBracket => write!(f, "["),
            RightBracket => write!(f, "]"),
            Question => write!(f, "?"),
            Colon => write!(f, ":"),
            UnderScore => write!(f, "_"),
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
            "break" => Some(Break),
            "NaN" => Some(NaN),
            "INF" => Some(INF),
            _ => None,
        }
    }

    /// Checks if the tokenkind is a keyword
    pub fn is_keyword(&self) -> bool {
        match self {
            TokenKind::And
            | TokenKind::Class
            | TokenKind::Else
            | TokenKind::Fun
            | TokenKind::For
            | TokenKind::If
            | TokenKind::Or
            | TokenKind::Print
            | TokenKind::Return
            | TokenKind::Super
            | TokenKind::This
            | TokenKind::Var
            | TokenKind::While => true,
            _ => false,
        }
    }

    /// Checks if the tokenkind is a literal
    pub fn is_literal(&self) -> bool {
        match self {
            TokenKind::String
            | TokenKind::Number
            | TokenKind::False
            | TokenKind::Nil
            | TokenKind::True => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_FILE_ID: FileIdent = FileIdent::new(0);

    macro_rules! test_scanner {
        ($source:literal) => {
            Scanner {
                file_id: TEST_FILE_ID,
                source: $source,
                start: 0,
                current: 0,
                eof: false,
            }
        };
    }

    macro_rules! test_token {
        ($kind:expr, $lexme:literal, $start:literal..$end:literal) => {
            Token {
                kind: $kind,
                lexeme: $lexme,
                span: Span::new($start..$end, TEST_FILE_ID),
            }
        };
    }

    macro_rules! scan_test {
        ($source:literal, [$($expected:expr),* $(,)?]) => {{
            let mut scanner = test_scanner!($source);
            let tokens = scanner.scan_tokens().unwrap();
            let expected = vec![$($expected),*];

            assert_eq!(
                tokens.len(),
                expected.len(),
                "token count mismatch\nactual: {:#?}\nexpected: {:#?}",
                tokens,
                expected
            );

            for (i, (actual, expected)) in tokens.iter().zip(expected.iter()).enumerate() {
                assert_eq!(
                    actual,
                    expected,
                    "mismatch at token index {}\nactual: {:#?}\nexpected: {:#?}",
                    i,
                    actual,
                    expected
                );
            }
        }};
    }

    #[test]
    fn test_identifiers() {
        scan_test!(
            "_ __ __ident ident_ ident_ident _6 _7ident _ident6 7ident__ ident_6 ident_7_ident 6 7_ 6__7",
            [
                test_token!(TokenKind::UnderScore, "_", 0..1),
                test_token!(TokenKind::Identifier, "__", 2..4),
                test_token!(TokenKind::Identifier, "__ident", 5..12),
                test_token!(TokenKind::Identifier, "ident_", 13..19),
                test_token!(TokenKind::Identifier, "ident_ident", 20..31),
                test_token!(TokenKind::Identifier, "_6", 32..34),
                test_token!(TokenKind::Identifier, "_7ident", 35..42),
                test_token!(TokenKind::Identifier, "_ident6", 43..50),
                test_token!(TokenKind::Number, "7", 51..52),
                test_token!(TokenKind::Identifier, "ident__", 52..59),
                test_token!(TokenKind::Identifier, "ident_6", 60..67),
                test_token!(TokenKind::Identifier, "ident_7_ident", 68..81),
                test_token!(TokenKind::Number, "6", 82..83),
                test_token!(TokenKind::Number, "7", 84..85),
                test_token!(TokenKind::UnderScore, "_", 85..86),
                test_token!(TokenKind::Number, "6", 87..88),
                test_token!(TokenKind::Identifier, "__7", 88..91),
                test_token!(TokenKind::Eof, "\0", 91..91),
            ]
        );
    }

    #[test]
    fn test_eof_span() {
        let mut scanner = test_scanner!("hello");
        assert_eq!(
            scanner.scan_tokens().unwrap().last().unwrap(),
            &Token {
                kind: TokenKind::Eof,
                lexeme: "\0",
                span: Span::new(5..5, TEST_FILE_ID)
            }
        );
    }

    #[test]
    fn test_eof_span_newline() {
        let mut scanner = test_scanner!("hello\n");
        assert_eq!(
            scanner.scan_tokens().unwrap().last().unwrap(),
            &Token {
                kind: TokenKind::Eof,
                lexeme: "\0",
                span: Span::new(6..6, TEST_FILE_ID)
            }
        );
    }

    #[test]
    fn test_mandarin_identifier() {
        let mut scanner = test_scanner!("你好啊");
        assert_eq!(
            scanner.scan_tokens(),
            Ok(vec![
                Token {
                    kind: TokenKind::Identifier,
                    lexeme: "你好啊",
                    // mandarin characters span multiple bytes
                    span: Span::new(0..9, TEST_FILE_ID)
                },
                Token {
                    kind: TokenKind::Eof,
                    lexeme: "\0",
                    span: Span::new(9..9, TEST_FILE_ID)
                }
            ]),
            "The identifier token should have a span that includes the entire lexeme plus the quotes for strings"
        );
    }

    #[test]
    fn test_mandarin_string() {
        let mut scanner = test_scanner!("\"你好啊\"");
        assert_eq!(
            scanner.scan_tokens(),
            Ok(vec![
                Token {
                    kind: TokenKind::String,
                    // Strings dont include the quotes in the lexeme
                    lexeme: "你好啊",
                    // mandarin characters span multiple bytes, include the quotes in the span
                    span: Span::new(0..11, TEST_FILE_ID)
                },
                Token {
                    kind: TokenKind::Eof,
                    lexeme: "\0",
                    span: Span::new(11..11, TEST_FILE_ID)
                }
            ])
        );
    }

    #[test]
    fn test_token_kind_search_keyword_str() {
        assert_eq!(TokenKind::search_keyword_str("and"), Some(TokenKind::And));
        assert_eq!(
            TokenKind::search_keyword_str("class"),
            Some(TokenKind::Class)
        );
        assert_eq!(TokenKind::search_keyword_str("else"), Some(TokenKind::Else));
        assert_eq!(
            TokenKind::search_keyword_str("false"),
            Some(TokenKind::False)
        );
        assert_eq!(TokenKind::search_keyword_str("fun"), Some(TokenKind::Fun));
        assert_eq!(TokenKind::search_keyword_str("for"), Some(TokenKind::For));
        assert_eq!(TokenKind::search_keyword_str("if"), Some(TokenKind::If));
        assert_eq!(TokenKind::search_keyword_str("nil"), Some(TokenKind::Nil));
        assert_eq!(TokenKind::search_keyword_str("or"), Some(TokenKind::Or));
        assert_eq!(
            TokenKind::search_keyword_str("print"),
            Some(TokenKind::Print)
        );
        assert_eq!(
            TokenKind::search_keyword_str("return"),
            Some(TokenKind::Return)
        );
        assert_eq!(
            TokenKind::search_keyword_str("super"),
            Some(TokenKind::Super)
        );
        assert_eq!(TokenKind::search_keyword_str("this"), Some(TokenKind::This));
        assert_eq!(TokenKind::search_keyword_str("true"), Some(TokenKind::True));
        assert_eq!(TokenKind::search_keyword_str("var"), Some(TokenKind::Var));
        assert_eq!(
            TokenKind::search_keyword_str("while"),
            Some(TokenKind::While)
        );
        assert_eq!(TokenKind::search_keyword_str(""), None);
    }
}
