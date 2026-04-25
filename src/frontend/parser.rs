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

#[derive(Clone, Copy)]
struct DeclSpecifiers {
    storage_class: StorageClass,
    ty: Type,
    is_interrupt: bool,
}

impl<'a> Parser<'a> {
    /// Creates a parser over tokenized preprocessed source and a shared diagnostic sink.
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

    /// Parses a full translation unit until the EOF token is reached.
    pub fn parse_translation_unit(&mut self) -> TranslationUnit {
        let mut items = Vec::new();
        while !self.is_eof() {
            items.push(self.parse_item());
        }
        TranslationUnit { items }
    }

    /// Parses one top-level declaration or function definition.
    fn parse_item(&mut self) -> Item {
        let start = self.current_span().start;
        let decl = self.parse_decl_specifiers();
        let (name, name_span, ty) = self.parse_declarator(decl.ty);

        if self.match_symbol(Symbol::LParen) {
            let params = self.parse_params();
            let span = Span::new(start, self.previous_span().end);
            if self.match_symbol(Symbol::LBrace) {
                let body = self.parse_block_after_open(span.start);
                return Item::Function(FunctionDecl {
                    name,
                    return_type: ty,
                    storage_class: decl.storage_class,
                    is_interrupt: decl.is_interrupt,
                    params,
                    body: Some(body),
                    span: Span::new(start, self.previous_span().end),
                });
            }
            self.expect_symbol(Symbol::Semicolon);
            Item::Function(FunctionDecl {
                name,
                return_type: ty,
                storage_class: decl.storage_class,
                is_interrupt: decl.is_interrupt,
                params,
                body: None,
                span: Span::new(start, self.previous_span().end),
            })
        } else {
            if decl.is_interrupt {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, name_span.end)),
                    "`__interrupt` is only valid on function declarations",
                    Some("declare the interrupt handler as `void __interrupt isr(void)`".to_string()),
                );
            }
            let initializer = if self.match_symbol(Symbol::Assign) {
                Some(self.parse_expression())
            } else {
                None
            };
            self.expect_symbol(Symbol::Semicolon);
            Item::Global(VarDecl {
                name,
                ty,
                storage_class: decl.storage_class,
                initializer,
                span: Span::new(start, self.previous_span().end.max(name_span.end)),
            })
        }
    }

    /// Parses a function parameter list, including the special `void)` case.
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
            let decl = self.parse_decl_specifiers();
            if decl.is_interrupt {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.previous_span().end)),
                    "`__interrupt` is not valid on parameters",
                    None,
                );
            }
            let (name, _, ty) = self.parse_declarator(decl.ty);
            params.push(VarDecl {
                name,
                ty,
                storage_class: decl.storage_class,
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

    /// Parses a block body after the opening brace has already been consumed.
    fn parse_block_after_open(&mut self, start: usize) -> Stmt {
        let mut statements = Vec::new();
        while !self.check_symbol(Symbol::RBrace) && !self.is_eof() {
            statements.push(self.parse_statement());
        }
        self.expect_symbol(Symbol::RBrace);
        Stmt::Block(statements, Span::new(start, self.previous_span().end))
    }

    /// Parses one statement in the supported C subset.
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
                "`switch` is not implemented yet",
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

    /// Parses a local variable declaration terminated by a semicolon.
    fn parse_local_decl(&mut self) -> VarDecl {
        let start = self.current_span().start;
        let decl = self.parse_decl_specifiers();
        let (name, _, ty) = self.parse_declarator(decl.ty);
        if decl.is_interrupt {
            self.diagnostics.error(
                "parser",
                Some(Span::new(start, self.previous_span().end)),
                "`__interrupt` is only valid on top-level function declarations",
                None,
            );
        }
        let initializer = if self.match_symbol(Symbol::Assign) {
            Some(self.parse_expression())
        } else {
            None
        };
        self.expect_symbol(Symbol::Semicolon);
        VarDecl {
            name,
            ty,
            storage_class: decl.storage_class,
            initializer,
            span: Span::new(start, self.previous_span().end),
        }
    }

    /// Parses the highest-level expression grammar entrypoint.
    fn parse_expression(&mut self) -> Expr {
        self.parse_assignment()
    }

    /// Parses right-associative assignment expressions.
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

    /// Parses `||` expressions with left associativity.
    fn parse_logical_or(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_logical_and, &[(Symbol::OrOr, BinaryOp::LogicalOr)])
    }

    /// Parses `&&` expressions with left associativity.
    fn parse_logical_and(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_bit_or, &[(Symbol::AndAnd, BinaryOp::LogicalAnd)])
    }

    /// Parses bitwise OR expressions with left associativity.
    fn parse_bit_or(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_bit_xor, &[(Symbol::Pipe, BinaryOp::BitOr)])
    }

    /// Parses bitwise XOR expressions with left associativity.
    fn parse_bit_xor(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_bit_and, &[(Symbol::Caret, BinaryOp::BitXor)])
    }

    /// Parses bitwise AND expressions with left associativity.
    fn parse_bit_and(&mut self) -> Expr {
        self.parse_left_assoc(Self::parse_equality, &[(Symbol::Ampersand, BinaryOp::BitAnd)])
    }

    /// Parses equality and inequality comparisons.
    fn parse_equality(&mut self) -> Expr {
        self.parse_left_assoc(
            Self::parse_relational,
            &[
                (Symbol::EqualEqual, BinaryOp::Equal),
                (Symbol::BangEqual, BinaryOp::NotEqual),
            ],
        )
    }

    /// Parses relational comparisons such as `<` and `>=`.
    fn parse_relational(&mut self) -> Expr {
        self.parse_left_assoc(
            Self::parse_shift,
            &[
                (Symbol::Less, BinaryOp::Less),
                (Symbol::LessEqual, BinaryOp::LessEqual),
                (Symbol::Greater, BinaryOp::Greater),
                (Symbol::GreaterEqual, BinaryOp::GreaterEqual),
            ],
        )
    }

    /// Parses shift expressions over `<<` and `>>`.
    fn parse_shift(&mut self) -> Expr {
        self.parse_left_assoc(
            Self::parse_additive,
            &[
                (Symbol::LessLess, BinaryOp::ShiftLeft),
                (Symbol::GreaterGreater, BinaryOp::ShiftRight),
            ],
        )
    }

    /// Parses additive expressions over `+` and `-`.
    fn parse_additive(&mut self) -> Expr {
        self.parse_left_assoc(
            Self::parse_multiplicative,
            &[(Symbol::Plus, BinaryOp::Add), (Symbol::Minus, BinaryOp::Sub)],
        )
    }

    /// Parses multiplicative expressions over `*`, `/`, and `%`.
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

    /// Parses a left-associative operator chain for one precedence level.
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

    /// Parses prefix unary operators before delegating to postfix expressions.
    fn parse_unary(&mut self) -> Expr {
        let start = self.current_span().start;
        if self.match_keyword(Keyword::Sizeof) {
            if self.match_symbol(Symbol::LParen) {
                if self.is_decl_start() {
                    let ty = self.parse_type_name();
                    self.expect_symbol(Symbol::RParen);
                    return Expr {
                        kind: ExprKind::SizeOfType(ty),
                        span: Span::new(start, self.previous_span().end),
                    };
                }
                let expr = self.parse_expression();
                self.expect_symbol(Symbol::RParen);
                return Expr {
                    kind: ExprKind::SizeOfExpr(Box::new(expr)),
                    span: Span::new(start, self.previous_span().end),
                };
            }
            let expr = self.parse_unary();
            return Expr {
                kind: ExprKind::SizeOfExpr(Box::new(expr)),
                span: Span::new(start, self.previous_span().end),
            };
        }
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
        if self.match_symbol(Symbol::Ampersand) {
            let expr = self.parse_unary();
            return Expr {
                kind: ExprKind::AddressOf(Box::new(expr)),
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_symbol(Symbol::Star) {
            let expr = self.parse_unary();
            return Expr {
                kind: ExprKind::Deref(Box::new(expr)),
                span: Span::new(start, self.previous_span().end),
            };
        }
        self.parse_postfix()
    }

    /// Parses postfix calls and indexing after a primary expression.
    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_primary();
        loop {
            if self.match_symbol(Symbol::LParen) {
                let start = expr.span.start;
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

                let callee = match expr.kind {
                    ExprKind::Name(name) => name,
                    _ => {
                        self.diagnostics.error(
                            "parser",
                            Some(expr.span),
                            "only direct function calls are supported",
                            Some("call a named function directly in this phase".to_string()),
                        );
                        "__error".to_string()
                    }
                };

                expr = Expr {
                    kind: ExprKind::Call { callee, args },
                    span: Span::new(start, self.previous_span().end),
                };
                continue;
            }

            if self.match_symbol(Symbol::LBracket) {
                let start = expr.span.start;
                let index = self.parse_expression();
                self.expect_symbol(Symbol::RBracket);
                expr = Expr {
                    kind: ExprKind::Index {
                        base: Box::new(expr),
                        index: Box::new(index),
                    },
                    span: Span::new(start, self.previous_span().end),
                };
                continue;
            }

            break;
        }
        expr
    }

    /// Parses literals, names, and parenthesized expressions.
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

    /// Parses storage, qualifier, and scalar type specifiers for a declaration.
    fn parse_decl_specifiers(&mut self) -> DeclSpecifiers {
        let mut storage = StorageClass::Auto;
        let mut qualifiers = Qualifiers::default();
        let mut saw_unsigned = false;
        let mut is_interrupt = false;
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
                TokenKind::Keyword(Keyword::Interrupt) => {
                    is_interrupt = true;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Void) => {
                    if scalar.is_some() {
                        self.diagnostics.error(
                            "parser",
                            Some(self.current_span()),
                            "duplicate type specifier",
                            None,
                        );
                    }
                    scalar = Some(ScalarType::Void);
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Char) => {
                    if scalar.is_some() {
                        self.diagnostics.error(
                            "parser",
                            Some(self.current_span()),
                            "duplicate type specifier",
                            None,
                        );
                    }
                    scalar = Some(if saw_unsigned { ScalarType::U8 } else { ScalarType::I8 });
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Int) => {
                    if scalar.is_some() {
                        self.diagnostics.error(
                            "parser",
                            Some(self.current_span()),
                            "duplicate type specifier",
                            None,
                        );
                    }
                    scalar = Some(if saw_unsigned { ScalarType::U16 } else { ScalarType::I16 });
                    self.advance();
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
        DeclSpecifiers {
            storage_class: storage,
            ty: Type::new(scalar).with_qualifiers(qualifiers),
            is_interrupt,
        }
    }

    /// Parses a named declarator with Phase 3 pointer and one-dimensional array suffixes.
    fn parse_declarator(&mut self, mut ty: Type) -> (String, Span, Type) {
        if self.check_symbol(Symbol::LParen) && self.peek_symbol(1, Symbol::Star) {
            let span = self.current_span();
            self.diagnostics.error(
                "parser",
                Some(span),
                "function pointer declarators are not supported in phase 3",
                Some("use direct function calls only for now".to_string()),
            );
            while !self.check_symbol(Symbol::RParen) && !self.is_eof() {
                self.advance();
            }
            self.expect_symbol(Symbol::RParen);
            return ("__unsupported".to_string(), span, ty);
        }
        while self.match_symbol(Symbol::Star) {
            ty = ty.pointer_to();
        }
        let (name, span) = self.expect_identifier();
        (name, span, self.parse_type_suffixes(ty))
    }

    /// Parses a standalone type name used by `sizeof(type)`.
    fn parse_type_name(&mut self) -> Type {
        let decl = self.parse_decl_specifiers();
        if decl.is_interrupt {
            self.diagnostics.error(
                "parser",
                Some(self.previous_span()),
                "`__interrupt` is not valid in type names",
                None,
            );
        }
        self.parse_abstract_declarator(decl.ty)
    }

    /// Parses the pointer and array suffix pieces of an abstract declarator.
    fn parse_abstract_declarator(&mut self, mut ty: Type) -> Type {
        while self.match_symbol(Symbol::Star) {
            ty = ty.pointer_to();
        }
        self.parse_type_suffixes(ty)
    }

    /// Parses the supported one-dimensional array suffix for one declarator.
    fn parse_type_suffixes(&mut self, mut ty: Type) -> Type {
        if self.match_symbol(Symbol::LBracket) {
            let len = self.parse_array_len();
            self.expect_symbol(Symbol::RBracket);
            ty = ty.array_of(len);
            while self.match_symbol(Symbol::LBracket) {
                self.diagnostics.error(
                    "parser",
                    Some(self.previous_span()),
                    "multidimensional arrays are not supported in phase 3",
                    Some("use one-dimensional arrays only for now".to_string()),
                );
                let _ = self.parse_array_len();
                self.expect_symbol(Symbol::RBracket);
            }
        }
        ty
    }

    /// Parses one fixed array length expression restricted to an integer literal.
    fn parse_array_len(&mut self) -> usize {
        let span = self.current_span();
        let TokenKind::Number(value) = self.current().kind.clone() else {
            self.diagnostics.error(
                "parser",
                Some(span),
                "array length must be an integer literal",
                None,
            );
            return 1;
        };
        self.advance();
        if value <= 0 {
            self.diagnostics.error(
                "parser",
                Some(span),
                "array length must be positive",
                None,
            );
            return 1;
        }
        value as usize
    }

    /// Skips tokens until a likely statement boundary after a parse error.
    fn synchronize_statement(&mut self) {
        while !self.is_eof() {
            if self.match_symbol(Symbol::Semicolon) || self.match_symbol(Symbol::RBrace) {
                break;
            }
            self.advance();
        }
    }

    /// Returns true when the current token can start a declaration.
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
                | TokenKind::Keyword(Keyword::Interrupt)
        )
    }

    /// Consumes an identifier token or reports a parser error and synthesizes one.
    fn expect_identifier(&mut self) -> (String, Span) {
        let span = self.current_span();
        if let TokenKind::Identifier(name) = self.current().kind.clone() {
            self.advance();
            return (name, span);
        }
        self.diagnostics.error("parser", Some(span), "expected identifier", None);
        ("__error".to_string(), span)
    }

    /// Consumes the expected symbol or records an error at the current token.
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

    /// Consumes the expected keyword or records an error at the current token.
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

    /// Consumes a symbol token when it matches the requested variant.
    fn match_symbol(&mut self, symbol: Symbol) -> bool {
        if self.check_symbol(symbol) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Consumes a keyword token when it matches the requested variant.
    fn match_keyword(&mut self, keyword: Keyword) -> bool {
        if self.check_keyword(keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Checks whether the current token is the requested symbol.
    fn check_symbol(&self, symbol: Symbol) -> bool {
        matches!(self.current().kind, TokenKind::Symbol(current) if current == symbol)
    }

    /// Checks whether the current token is the requested keyword.
    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current().kind, TokenKind::Keyword(current) if current == keyword)
    }

    /// Peeks ahead for a symbol without consuming any tokens.
    fn peek_symbol(&self, offset: usize, symbol: Symbol) -> bool {
        self.tokens
            .get(self.index + offset)
            .is_some_and(|token| matches!(token.kind, TokenKind::Symbol(current) if current == symbol))
    }

    /// Returns the current token, clamping safely at EOF.
    fn current(&self) -> &Token {
        &self.tokens[self.index.min(self.tokens.len().saturating_sub(1))]
    }

    /// Returns the span of the current token.
    fn current_span(&self) -> Span {
        self.current().span
    }

    /// Returns the span of the previously consumed token, or an empty span at start.
    fn previous_span(&self) -> Span {
        if self.index == 0 {
            Span::new(0, 0)
        } else {
            self.tokens[self.index - 1].span
        }
    }

    /// Advances to the next token unless the parser is already at EOF.
    fn advance(&mut self) {
        if !self.is_eof() {
            self.index += 1;
        }
    }

    /// Returns true when the parser is positioned on the EOF token.
    fn is_eof(&self) -> bool {
        self.current().kind == TokenKind::Eof
    }
}
