use std::fmt::Write;

use crate::common::source::{PreprocessedSource, Span};
use crate::diagnostics::DiagnosticBag;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Number(i64),
    Keyword(Keyword),
    Symbol(Symbol),
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Keyword {
    Char,
    Const,
    Continue,
    Do,
    Else,
    Extern,
    For,
    If,
    Int,
    Return,
    Static,
    Unsigned,
    Void,
    Volatile,
    While,
    Break,
    Switch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Symbol {
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Assign,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Ampersand,
    Pipe,
    Caret,
    Bang,
    Tilde,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub struct Lexer<'a> {
    source: &'a PreprocessedSource,
    diagnostics: &'a mut DiagnosticBag,
    index: usize,
}

impl<'a> Lexer<'a> {
    /// Creates a lexer over preprocessed source text and a shared diagnostic bag.
    pub fn new(source: &'a PreprocessedSource, diagnostics: &'a mut DiagnosticBag) -> Self {
        Self {
            source,
            diagnostics,
            index: 0,
        }
    }

    /// Tokenizes the full input stream and appends an explicit EOF token.
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token();
            let is_eof = token.kind == TokenKind::Eof;
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        tokens
    }

    /// Tokenizes the input and renders token kinds for debug artifact output.
    pub fn collect_debug(mut self) -> String {
        let mut output = String::new();
        for token in self.tokenize() {
            let _ = writeln!(output, "{:?}", token.kind);
        }
        output
    }

    /// Scans the next token while updating the current byte index.
    fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();
        if self.index >= self.source.text.len() {
            return Token {
                kind: TokenKind::Eof,
                span: Span::new(self.index, self.index),
            };
        }

        let bytes = self.source.text.as_bytes();
        let start = self.index;
        let ch = bytes[self.index] as char;

        if ch.is_ascii_alphabetic() || ch == '_' {
            self.index += 1;
            while self.index < bytes.len() {
                let current = bytes[self.index] as char;
                if current.is_ascii_alphanumeric() || current == '_' {
                    self.index += 1;
                } else {
                    break;
                }
            }
            let text = &self.source.text[start..self.index];
            return Token {
                kind: keyword_or_ident(text),
                span: Span::new(start, self.index),
            };
        }

        if ch.is_ascii_digit() {
            self.index += 1;
            if ch == '0'
                && self.index < bytes.len()
                && matches!(bytes[self.index] as char, 'x' | 'X')
            {
                self.index += 1;
                while self.index < bytes.len() && (bytes[self.index] as char).is_ascii_hexdigit() {
                    self.index += 1;
                }
            } else {
                while self.index < bytes.len() && (bytes[self.index] as char).is_ascii_digit() {
                    self.index += 1;
                }
            }
            let literal = &self.source.text[start..self.index];
            let value = if literal.starts_with("0x") || literal.starts_with("0X") {
                i64::from_str_radix(&literal[2..], 16).unwrap_or(0)
            } else {
                literal.parse::<i64>().unwrap_or(0)
            };
            return Token {
                kind: TokenKind::Number(value),
                span: Span::new(start, self.index),
            };
        }

        let kind = match self.try_double_symbol() {
            Some(symbol) => TokenKind::Symbol(symbol),
            None => {
                self.index += 1;
                match ch {
                    '(' => TokenKind::Symbol(Symbol::LParen),
                    ')' => TokenKind::Symbol(Symbol::RParen),
                    '{' => TokenKind::Symbol(Symbol::LBrace),
                    '}' => TokenKind::Symbol(Symbol::RBrace),
                    ',' => TokenKind::Symbol(Symbol::Comma),
                    ';' => TokenKind::Symbol(Symbol::Semicolon),
                    '=' => TokenKind::Symbol(Symbol::Assign),
                    '+' => TokenKind::Symbol(Symbol::Plus),
                    '-' => TokenKind::Symbol(Symbol::Minus),
                    '*' => TokenKind::Symbol(Symbol::Star),
                    '/' => TokenKind::Symbol(Symbol::Slash),
                    '%' => TokenKind::Symbol(Symbol::Percent),
                    '&' => TokenKind::Symbol(Symbol::Ampersand),
                    '|' => TokenKind::Symbol(Symbol::Pipe),
                    '^' => TokenKind::Symbol(Symbol::Caret),
                    '!' => TokenKind::Symbol(Symbol::Bang),
                    '~' => TokenKind::Symbol(Symbol::Tilde),
                    '<' => TokenKind::Symbol(Symbol::Less),
                    '>' => TokenKind::Symbol(Symbol::Greater),
                    _ => {
                        self.diagnostics.error(
                            "lexer",
                            Some(Span::new(start, start + 1)),
                            format!("unexpected character `{ch}`"),
                            None,
                        );
                        TokenKind::Eof
                    }
                }
            }
        };

        Token {
            kind,
            span: Span::new(start, self.index),
        }
    }

    /// Recognizes multi-character punctuation before single-character token fallback.
    fn try_double_symbol(&mut self) -> Option<Symbol> {
        let rest = &self.source.text[self.index..];
        let table = [
            ("==", Symbol::EqualEqual),
            ("!=", Symbol::BangEqual),
            ("<=", Symbol::LessEqual),
            (">=", Symbol::GreaterEqual),
            ("&&", Symbol::AndAnd),
            ("||", Symbol::OrOr),
        ];
        for (text, symbol) in table {
            if rest.starts_with(text) {
                self.index += text.len();
                return Some(symbol);
            }
        }
        None
    }

    /// Skips whitespace plus line and block comments before token scanning continues.
    fn skip_whitespace_and_comments(&mut self) {
        let bytes = self.source.text.as_bytes();
        while self.index < bytes.len() {
            let ch = bytes[self.index] as char;
            if ch.is_whitespace() {
                self.index += 1;
                continue;
            }
            if self.source.text[self.index..].starts_with("//") {
                while self.index < bytes.len() && (bytes[self.index] as char) != '\n' {
                    self.index += 1;
                }
                continue;
            }
            if self.source.text[self.index..].starts_with("/*") {
                self.index += 2;
                while self.index + 1 < bytes.len() && &self.source.text[self.index..self.index + 2] != "*/" {
                    self.index += 1;
                }
                if self.index + 1 < bytes.len() {
                    self.index += 2;
                } else {
                    self.diagnostics.error(
                        "lexer",
                        Some(Span::new(self.index.saturating_sub(2), self.index)),
                        "unterminated block comment",
                        None,
                    );
                }
                continue;
            }
            break;
        }
    }
}

/// Reclassifies an identifier as a keyword when it matches the supported C subset.
fn keyword_or_ident(text: &str) -> TokenKind {
    let keyword = match text {
        "char" => Some(Keyword::Char),
        "const" => Some(Keyword::Const),
        "continue" => Some(Keyword::Continue),
        "do" => Some(Keyword::Do),
        "else" => Some(Keyword::Else),
        "extern" => Some(Keyword::Extern),
        "for" => Some(Keyword::For),
        "if" => Some(Keyword::If),
        "int" => Some(Keyword::Int),
        "return" => Some(Keyword::Return),
        "static" => Some(Keyword::Static),
        "unsigned" => Some(Keyword::Unsigned),
        "void" => Some(Keyword::Void),
        "volatile" => Some(Keyword::Volatile),
        "while" => Some(Keyword::While),
        "break" => Some(Keyword::Break),
        "switch" => Some(Keyword::Switch),
        _ => None,
    };
    keyword.map_or_else(|| TokenKind::Identifier(text.to_string()), TokenKind::Keyword)
}
