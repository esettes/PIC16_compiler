use crate::common::source::{PreprocessedSource, Span};
use crate::diagnostics::DiagnosticBag;

use super::ast::{
    BinaryOp, Expr, ExprKind, FunctionDecl, Item, Stmt, TranslationUnit, UnaryOp, VarDecl,
};
use super::lexer::{Keyword, Symbol, Token, TokenKind};
use super::types::{Qualifiers, ScalarType, StorageClass, Type};

pub struct Parser<'a> {
    tokens: Vec<Token>,
    diagnostics: &'a mut DiagnosticBag,
    index: usize,
    _source: &'a PreprocessedSource,
}

impl<'a> Parser<'a> {
    pub fn new(
        tokens: Vec<Token>,
        source: &'a PreprocessedSource,
        diagnostics: &'a mut DiagnosticBag,
    ) -> Self {
        Self {
            tokens,
            diagnostics,
            index: 0,
            _source: source,
        }
    }

    pub fn parse_translation_unit(&mut self) -> TranslationUnit {
        let mut items = Vec::new();
        while !self.is_eof() {
            items.push(self.parse_item());
        }
        TranslationUnit { items }
    }

    fn parse_item(&mut self) -> Item {
        let start = self.current_span().start;
        let (storage_class, ty) = self.parse_decl_specifiers();
        let (name, name_span) = self.expect_identifier();

        if self.match_symbol(Symbol::LParen) {
            let params = self.parse_params();
            let span = Span::new(start, self.previous_span().end);
            if self.match_symbol(Symbol::LBrace) {
                let body = self.parse_block_after_open(span.start);
                return Item::Function(FunctionDecl {
                    name,
                    return_type: ty,
                    storage_class,
                    params,
                    body: Some(body),
                    span: Span::new(start, self.previous_span().end),
                });
            }
            self.expect_symbol(Symbol::Semicolon);
            Item::Function(FunctionDecl {
                name,
                return_type: ty,
                storage_class,
                params,
                body: None,
                span: Span::new(start, self.previous_span().end),
            })
        } else {
            let initializer = if self.match_symbol(Symbol::Assign) {
                Some(self.parse_expression())
            } else {
                None
            };
            self.expect_symbol(Symbol::Semicolon);
            Item::Global(VarDecl {
                name,
                ty,
                storage_class,
                initializer,
                span: Span::new(start, self.previous_span().end.max(name_span.end)),
            })
        }
    }

    fn parse_params(&mut self) -> Vec<VarDecl> {
        if self.match_symbol(Symbol::RParen) {
            return Vec::new();
        }
        if self.check_keyword(Keyword::Void) && self.peek_symbol(1, Symbol::RParen) {
            self.advance();
            self.expect_symbol(Symbol::RParen);
            return Vec::new();
        }

        let mut params = Vec::new();
        loop {
            let start = self.current_span().start;
            let (storage_class, ty) = self.parse_decl_specifiers();
            let (name, _) = self.expect_identifier();
            params.push(VarDecl {
                name,
                ty,
                storage_class,
                initializer: None,
                span: Span::new(start, self.previous_span().end),
            });
            if self.match_symbol(Symbol::Comma) {
                continue;
            }
            self.expect_symbol(Symbol::RParen);
            break;
        }
        params
    }

    fn parse_block_after_open(&mut self, start: usize) -> Stmt {
        let mut statements = Vec::new();
        while !self.check_symbol(Symbol::RBrace) && !self.is_eof() {
            statements.push(self.parse_statement());
        }
        self.expect_symbol(Symbol::RBrace);
        Stmt::Block(statements, Span::new(start, self.previous_span().end))
    }

    fn parse_statement(&mut self) -> Stmt {
        let start = self.current_span().start;
        if self.match_symbol(Symbol::LBrace) {
            return self.parse_block_after_open(start);
        }
        if self.is_decl_start() {
            return Stmt::VarDecl(self.parse_local_decl());
        }
        if self.match_keyword(Keyword::Return) {
            let expr = if self.check_symbol(Symbol::Semicolon) {
                None
            } else {
                Some(self.parse_expression())
            };
            self.expect_symbol(Symbol::Semicolon);
            return Stmt::Return(expr, Span::new(start, self.previous_span().end));
        }
        if self.match_keyword(Keyword::If) {
            self.expect_symbol(Symbol::LParen);
            let condition = self.parse_expression();
            self.expect_symbol(Symbol::RParen);
            let then_branch = Box::new(self.parse_statement());
            let else_branch = if self.match_keyword(Keyword::Else) {
                Some(Box::new(self.parse_statement()))
            } else {
                None
            };
            return Stmt::If {
                condition,
                then_branch,
                else_branch,
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_keyword(Keyword::While) {
            self.expect_symbol(Symbol::LParen);
            let condition = self.parse_expression();
            self.expect_symbol(Symbol::RParen);
            let body = Box::new(self.parse_statement());
            return Stmt::While {
                condition,
                body,
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_keyword(Keyword::Do) {
            let body = Box::new(self.parse_statement());
            self.expect_keyword(Keyword::While);
            self.expect_symbol(Symbol::LParen);
            let condition = self.parse_expression();
            self.expect_symbol(Symbol::RParen);
            self.expect_symbol(Symbol::Semicolon);
            return Stmt::DoWhile {
                body,
                condition,
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_keyword(Keyword::For) {
            self.expect_symbol(Symbol::LParen);
            let init = if self.check_symbol(Symbol::Semicolon) {
                self.advance();
                None
            } else if self.is_decl_start() {
                Some(Box::new(Stmt::VarDecl(self.parse_local_decl())))
            } else {
                let expr = self.parse_expression();
                self.expect_symbol(Symbol::Semicolon);
                Some(Box::new(Stmt::Expr(
                    expr,
                    Span::new(start, self.previous_span().end),
                )))
            };

            let condition = if self.check_symbol(Symbol::Semicolon) {
                self.advance();
                None
            } else {
                let expr = self.parse_expression();
                self.expect_symbol(Symbol::Semicolon);
                Some(expr)
            };

            let step = if self.check_symbol(Symbol::RParen) {
                None
            } else {
                Some(self.parse_expression())
            };
            self.expect_symbol(Symbol::RParen);
            let body = Box::new(self.parse_statement());
            return Stmt::For {
                init,
                condition,
                step,
                body,
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_keyword(Keyword::Break) {
            self.expect_symbol(Symbol::Semicolon);
            return Stmt::Break(Span::new(start, self.previous_span().end));
        }
        if self.match_keyword(Keyword::Continue) {
            self.expect_symbol(Symbol::Semicolon);
            return Stmt::Continue(Span::new(start, self.previous_span().end));
        }
        if self.match_keyword(Keyword::Switch) {
            self.diagnostics.error(
                "parser",
                Some(self.previous_span()),
                "`switch` is planned but not implemented in v0.1",
                Some("rewrite the construct using `if`/`else if` for now".to_string()),
            );
            self.synchronize_statement();
            return Stmt::Empty(Span::new(start, self.previous_span().end));
        }
        if self.match_symbol(Symbol::Semicolon) {
            return Stmt::Empty(Span::new(start, self.previous_span().end));
        }

        let expr = self.parse_expression();
        self.expect_symbol(Symbol::Semicolon);
        Stmt::Expr(expr, Span::new(start, self.previous_span().end))
    }

    fn parse_local_decl(&mut self) -> VarDecl {
        let start = self.current_span().start;
        let (storage_class, ty) = self.parse_decl_specifiers();
        let (name, _) = self.expect_identifier();
        let initializer = if self.match_symbol(Symbol::Assign) {
            Some(self.parse_expression())
        } else {
            None
        };
        self.expect_symbol(Symbol::Semicolon);
        VarDecl {
            name,
            ty,
            storage_class,
            initializer,
            span: Span::new(start, self.previous_span().end),
        }
    }

    fn parse_expression(&mut self) -> Expr {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Expr {
        let start = self.current_span().start;
        let lhs = self.parse_logical_or();
        if self.match_symbol(Symbol::Assign) {
            let rhs = self.parse_assignment();
            return Expr {
                kind: ExprKind::Assign {
                    target: Box::new(lhs),
                    value: Box::new(rhs),
                },
                span: Span::new(start, self.previous_span().end),
            };
        }
        lhs
    }

    fn parse_logical_or(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_logical_and, &[(Symbol::OrOr, BinaryOp::LogicalOr)])
    }

    fn parse_logical_and(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_bit_or, &[(Symbol::AndAnd, BinaryOp::LogicalAnd)])
    }

    fn parse_bit_or(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_bit_xor, &[(Symbol::Pipe, BinaryOp::BitOr)])
    }

    fn parse_bit_xor(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_bit_and, &[(Symbol::Caret, BinaryOp::BitXor)])
    }

    fn parse_bit_and(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_equality, &[(Symbol::Ampersand, BinaryOp::BitAnd)])
    }

    fn parse_equality(&mut self) -> Expr {
        self.parse_left_assoc(
            Self::parse_relational,
            &[
                (Symbol::EqualEqual, BinaryOp::Equal),
                (Symbol::BangEqual, BinaryOp::NotEqual),
            ],
        )
    }

    fn parse_relational(&mut self) -> Expr {
        self.parse_left_assoc(
            Self::parse_additive,
            &[
                (Symbol::Less, BinaryOp::Less),
                (Symbol::LessEqual, BinaryOp::LessEqual),
                (Symbol::Greater, BinaryOp::Greater),
                (Symbol::GreaterEqual, BinaryOp::GreaterEqual),
            ],
        )
    }

    fn parse_additive(&mut self) -> Expr {
        self.parse_left_assoc(
            Self::parse_multiplicative,
            &[(Symbol::Plus, BinaryOp::Add), (Symbol::Minus, BinaryOp::Sub)],
        )
    }

    fn parse_multiplicative(&mut self) -> Expr {
        self.parse_left_assoc(
            Self::parse_unary,
            &[
                (Symbol::Star, BinaryOp::Multiply),
                (Symbol::Slash, BinaryOp::Divide),
                (Symbol::Percent, BinaryOp::Modulo),
            ],
        )
    }

    fn parse_left_assoc(
        &mut self,
        next: fn(&mut Self) -> Expr,
        ops: &[(Symbol, BinaryOp)],
    ) -> Expr {
        let mut expr = next(self);
        loop {
            let mut matched = None;
            for (symbol, op) in ops {
                if self.match_symbol(*symbol) {
                    matched = Some(*op);
                    break;
                }
            }
            let Some(op) = matched else {
                break;
            };
            let start = expr.span.start;
            let rhs = next(self);
            expr = Expr {
                kind: ExprKind::Binary {
                    op,
                    lhs: Box::new(expr),
                    rhs: Box::new(rhs),
                },
                span: Span::new(start, self.previous_span().end),
            };
        }
        expr
    }

    fn parse_unary(&mut self) -> Expr {
        let start = self.current_span().start;
        if self.match_symbol(Symbol::Minus) {
            let expr = self.parse_unary();
            return Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::Negate,
                    expr: Box::new(expr),
                },
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_symbol(Symbol::Bang) {
            let expr = self.parse_unary();
            return Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::LogicalNot,
                    expr: Box::new(expr),
                },
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_symbol(Symbol::Tilde) {
            let expr = self.parse_unary();
            return Expr {
                kind: ExprKind::Unary {
                    op: UnaryOp::BitwiseNot,
                    expr: Box::new(expr),
                },
                span: Span::new(start, self.previous_span().end),
            };
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Expr {
        let start = self.current_span().start;
        if self.match_symbol(Symbol::LParen) {
            let expr = self.parse_expression();
            self.expect_symbol(Symbol::RParen);
            return expr;
        }
        if let TokenKind::Number(value) = self.current().kind.clone() {
            self.advance();
            return Expr {
                kind: ExprKind::IntLiteral(value),
                span: Span::new(start, self.previous_span().end),
            };
        }
        if let TokenKind::Identifier(name) = self.current().kind.clone() {
            self.advance();
            if self.match_symbol(Symbol::LParen) {
                let mut args = Vec::new();
                if !self.check_symbol(Symbol::RParen) {
                    loop {
                        args.push(self.parse_expression());
                        if self.match_symbol(Symbol::Comma) {
                            continue;
                        }
                        break;
                    }
                }
                self.expect_symbol(Symbol::RParen);
                return Expr {
                    kind: ExprKind::Call { callee: name, args },
                    span: Span::new(start, self.previous_span().end),
                };
            }
            return Expr {
                kind: ExprKind::Name(name),
                span: Span::new(start, self.previous_span().end),
            };
        }

        self.diagnostics.error(
            "parser",
            Some(self.current_span()),
            "expected expression",
            None,
        );
        self.advance();
        Expr {
            kind: ExprKind::IntLiteral(0),
            span: Span::new(start, self.previous_span().end),
        }
    }

    fn parse_decl_specifiers(&mut self) -> (StorageClass, Type) {
        let mut storage = StorageClass::Auto;
        let mut qualifiers = Qualifiers::default();
        let mut saw_unsigned = false;
        let mut scalar = None::<ScalarType>;

        loop {
            match &self.current().kind {
                TokenKind::Keyword(Keyword::Static) => {
                    storage = StorageClass::Static;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Extern) => {
                    storage = StorageClass::Extern;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Const) => {
                    qualifiers.is_const = true;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Volatile) => {
                    qualifiers.is_volatile = true;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Unsigned) => {
                    saw_unsigned = true;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Void) => {
                    scalar = Some(ScalarType::Void);
                    self.advance();
                    break;
                }
                TokenKind::Keyword(Keyword::Char) => {
                    scalar = Some(if saw_unsigned { ScalarType::U8 } else { ScalarType::I8 });
                    self.advance();
                    break;
                }
                TokenKind::Keyword(Keyword::Int) => {
                    scalar = Some(if saw_unsigned { ScalarType::U16 } else { ScalarType::I16 });
                    self.advance();
                    break;
                }
                _ => break,
            }
        }

        let scalar = scalar.unwrap_or_else(|| {
            self.diagnostics.error(
                "parser",
                Some(self.current_span()),
                "expected type specifier",
                Some("supported types: void, char, unsigned char, int, unsigned int".to_string()),
            );
            ScalarType::I16
        });
        (storage, Type::new(scalar).with_qualifiers(qualifiers))
    }

    fn synchronize_statement(&mut self) {
        while !self.is_eof() {
            if self.match_symbol(Symbol::Semicolon) || self.match_symbol(Symbol::RBrace) {
                break;
            }
            self.advance();
        }
    }

    fn is_decl_start(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Keyword(Keyword::Static)
                | TokenKind::Keyword(Keyword::Extern)
                | TokenKind::Keyword(Keyword::Const)
                | TokenKind::Keyword(Keyword::Volatile)
                | TokenKind::Keyword(Keyword::Unsigned)
                | TokenKind::Keyword(Keyword::Void)
                | TokenKind::Keyword(Keyword::Char)
                | TokenKind::Keyword(Keyword::Int)
        )
    }

    fn expect_identifier(&mut self) -> (String, Span) {
        let span = self.current_span();
        if let TokenKind::Identifier(name) = self.current().kind.clone() {
            self.advance();
            return (name, span);
        }
        self.diagnostics.error("parser", Some(span), "expected identifier", None);
        ("__error".to_string(), span)
    }

    fn expect_symbol(&mut self, symbol: Symbol) {
        if !self.match_symbol(symbol) {
            self.diagnostics.error(
                "parser",
                Some(self.current_span()),
                format!("expected symbol `{:?}`", symbol),
                None,
            );
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword) {
        if !self.match_keyword(keyword) {
            self.diagnostics.error(
                "parser",
                Some(self.current_span()),
                format!("expected keyword `{:?}`", keyword),
                None,
            );
        }
    }

    fn match_symbol(&mut self, symbol: Symbol) -> bool {
        if self.check_symbol(symbol) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_keyword(&mut self, keyword: Keyword) -> bool {
        if self.check_keyword(keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check_symbol(&self, symbol: Symbol) -> bool {
        matches!(self.current().kind, TokenKind::Symbol(current) if current == symbol)
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(current) if current == keyword)
    }

    fn peek_symbol(&self, offset: usize, symbol: Symbol) -> bool {
        self.tokens
            .get(self.index + offset)
            .is_some_and(|token| matches!(token.kind, TokenKind::Symbol(current) if current == symbol))
    }

    fn current(&self) -> &Token {
        &self.tokens[self.index.min(self.tokens.len().saturating_sub(1))]
    }

    fn current_span(&self) -> Span {
        self.current().span
    }

    fn previous_span(&self) -> Span {
        if self.index == 0 {
            Span::new(0, 0)
        } else {
            self.tokens[self.index - 1].span
        }
    }

    fn advance(&mut self) {
        if !self.is_eof() {
            self.index += 1;
        }
    }

    fn is_eof(&self) -> bool {
        self.current().kind == TokenKind::Eof
    }
}
