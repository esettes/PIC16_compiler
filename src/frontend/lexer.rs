use std::fmt::Write;

use crate::common::source::{PreprocessedSource, Span};
use crate::diagnostics::DiagnosticBag;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Number(i64),
    StringLiteral(Vec<u8>),
    Keyword(Keyword),
    Symbol(Symbol),
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Keyword {
    Char,
    Const,
    Rom,
    Continue,
    Case,
    Default,
    Do,
    Else,
    Extern,
    For,
    If,
    Int,
    Interrupt,
    Typedef,
    Enum,
    Struct,
    Return,
    Static,
    Unsigned,
    Void,
    Volatile,
    While,
    Break,
    Sizeof,
    Switch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Symbol {
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Semicolon,
    Dot,
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
    LessLess,
    Greater,
    GreaterEqual,
    GreaterGreater,
    AndAnd,
    OrOr,
    Arrow,
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

        if ch == '"' {
            return self.lex_string_literal();
        }

        let kind = match self.try_double_symbol() {
            Some(symbol) => TokenKind::Symbol(symbol),
            None => {
                self.index += 1;
                match ch {
                    '(' => TokenKind::Symbol(Symbol::LParen),
                    ')' => TokenKind::Symbol(Symbol::RParen),
                    '[' => TokenKind::Symbol(Symbol::LBracket),
                    ']' => TokenKind::Symbol(Symbol::RBracket),
                    '{' => TokenKind::Symbol(Symbol::LBrace),
                    '}' => TokenKind::Symbol(Symbol::RBrace),
                    ',' => TokenKind::Symbol(Symbol::Comma),
                    ':' => TokenKind::Symbol(Symbol::Colon),
                    ';' => TokenKind::Symbol(Symbol::Semicolon),
                    '.' => TokenKind::Symbol(Symbol::Dot),
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

    /// Scans one string literal and appends the trailing null terminator byte.
    fn lex_string_literal(&mut self) -> Token {
        let start = self.index;
        self.index += 1;
        let bytes = self.source.text.as_bytes();
        let mut value = Vec::new();
        let mut terminated = false;

        while self.index < bytes.len() {
            match bytes[self.index] {
                b'"' => {
                    self.index += 1;
                    terminated = true;
                    break;
                }
                b'\\' => {
                    self.index += 1;
                    if self.index >= bytes.len() {
                        break;
                    }
                    match bytes[self.index] {
                        b'n' => {
                            value.push(b'\n');
                            self.index += 1;
                        }
                        b'r' => {
                            value.push(b'\r');
                            self.index += 1;
                        }
                        b't' => {
                            value.push(b'\t');
                            self.index += 1;
                        }
                        b'\\' => {
                            value.push(b'\\');
                            self.index += 1;
                        }
                        b'"' => {
                            value.push(b'"');
                            self.index += 1;
                        }
                        b'0' => {
                            value.push(0);
                            self.index += 1;
                        }
                        b'x' | b'X' => {
                            let escape_span =
                                Span::new(self.index.saturating_sub(1), (self.index + 1).min(bytes.len()));
                            self.diagnostics.error(
                                "lexer",
                                Some(escape_span),
                                "hexadecimal string escapes are not supported in phase 10",
                                Some("use \\n, \\r, \\t, \\\\, \\\" or \\0 escapes only".to_string()),
                            );
                            self.index += 1;
                            while self.index < bytes.len() && (bytes[self.index] as char).is_ascii_hexdigit() {
                                self.index += 1;
                            }
                            value.push(0);
                        }
                        other => {
                            let escape_span =
                                Span::new(self.index.saturating_sub(1), (self.index + 1).min(bytes.len()));
                            self.diagnostics.error(
                                "lexer",
                                Some(escape_span),
                                format!("unsupported escape sequence `\\{}`", other as char),
                                Some("use \\n, \\r, \\t, \\\\, \\\" or \\0 escapes only".to_string()),
                            );
                            self.index += 1;
                            value.push(0);
                        }
                    }
                }
                b'\n' | b'\r' => break,
                byte => {
                    value.push(byte);
                    self.index += 1;
                }
            }
        }

        if !terminated {
            self.diagnostics.error(
                "lexer",
                Some(Span::new(start, self.index.min(self.source.text.len()))),
                "unterminated string literal",
                None,
            );
        }

        value.push(0);
        Token {
            kind: TokenKind::StringLiteral(value),
            span: Span::new(start, self.index.min(self.source.text.len())),
        }
    }

    /// Recognizes multi-character punctuation before single-character token fallback.
    fn try_double_symbol(&mut self) -> Option<Symbol> {
        let rest = &self.source.text[self.index..];
        let table = [
            ("==", Symbol::EqualEqual),
            ("!=", Symbol::BangEqual),
            ("<=", Symbol::LessEqual),
            ("<<", Symbol::LessLess),
            (">=", Symbol::GreaterEqual),
            (">>", Symbol::GreaterGreater),
            ("&&", Symbol::AndAnd),
            ("||", Symbol::OrOr),
            ("->", Symbol::Arrow),
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
        "__rom" => Some(Keyword::Rom),
        "continue" => Some(Keyword::Continue),
        "case" => Some(Keyword::Case),
        "default" => Some(Keyword::Default),
        "do" => Some(Keyword::Do),
        "else" => Some(Keyword::Else),
        "extern" => Some(Keyword::Extern),
        "for" => Some(Keyword::For),
        "if" => Some(Keyword::If),
        "int" => Some(Keyword::Int),
        "__interrupt" => Some(Keyword::Interrupt),
        "typedef" => Some(Keyword::Typedef),
        "enum" => Some(Keyword::Enum),
        "struct" => Some(Keyword::Struct),
        "return" => Some(Keyword::Return),
        "sizeof" => Some(Keyword::Sizeof),
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

#[cfg(test)]
mod tests {
    use super::{Lexer, TokenKind};
    use crate::common::source::{PreprocessedSource, SourceId, SourcePoint};
    use crate::diagnostics::{DiagnosticBag, WarningProfile};

    fn lex(source: &str) -> (Vec<TokenKind>, DiagnosticBag) {
        let origin = SourcePoint {
            file: SourceId(0),
            line: 1,
            column: 1,
        };
        let mut preprocessed = PreprocessedSource::new();
        preprocessed.push_str(source, origin);
        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let tokens = Lexer::new(&preprocessed, &mut diagnostics)
            .tokenize()
            .into_iter()
            .map(|token| token.kind)
            .collect();
        (tokens, diagnostics)
    }

    #[test]
    /// Verifies one basic string literal token carries its trailing null byte.
    fn tokenizes_phase10_basic_string_literal() {
        let (tokens, diagnostics) = lex("\"OK\"");
        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert_eq!(
            tokens,
            vec![TokenKind::StringLiteral(vec![b'O', b'K', 0]), TokenKind::Eof]
        );
    }

    #[test]
    /// Verifies the supported common escapes decode into byte payloads.
    fn tokenizes_phase10_escaped_string_literal() {
        let (tokens, diagnostics) = lex("\"A\\n\\t\\\\\\\"\\0\"");
        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert_eq!(
            tokens,
            vec![
                TokenKind::StringLiteral(vec![b'A', b'\n', b'\t', b'\\', b'"', 0, 0]),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    /// Verifies unterminated strings emit a lexer diagnostic.
    fn reports_phase10_unterminated_string_literal() {
        let (_, diagnostics) = lex("\"oops");
        assert!(diagnostics.has_errors());
        assert!(format!("{diagnostics}").contains("unterminated string literal"));
    }

    #[test]
    /// Verifies unsupported string escapes emit a lexer diagnostic.
    fn reports_phase10_unsupported_string_escape() {
        let (_, diagnostics) = lex("\"\\q\"");
        assert!(diagnostics.has_errors());
        assert!(format!("{diagnostics}").contains("unsupported escape sequence"));
    }
}
