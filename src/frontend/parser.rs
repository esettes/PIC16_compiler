use crate::common::source::{PreprocessedSource, Span};
use crate::diagnostics::DiagnosticBag;

use super::ast::{
    BinaryOp, Designator, EnumConstant, Expr, ExprKind, FunctionDecl, Initializer,
    InitializerEntry, Item, Stmt, StructDef, StructField, TranslationUnit, UnaryOp, VarDecl,
};
use super::lexer::{Keyword, Symbol, Token, TokenKind};
use super::types::{AddressSpace, MAX_POINTER_DEPTH, Qualifiers, ScalarType, StorageClass, StructId, Type};

use std::collections::{BTreeMap, BTreeSet};

pub struct Parser<'a> {
    tokens: Vec<Token>,
    diagnostics: &'a mut DiagnosticBag,
    index: usize,
    _source: &'a PreprocessedSource,
    typedefs: BTreeMap<String, Type>,
    struct_tags: BTreeMap<String, StructId>,
    struct_defs: Vec<StructDef>,
    enum_tags: BTreeSet<String>,
    enum_constants: Vec<EnumConstant>,
    enum_constant_by_name: BTreeMap<String, i64>,
    global_value_names: BTreeSet<String>,
}

#[derive(Clone, Copy)]
struct DeclSpecifiers {
    storage_class: StorageClass,
    ty: Type,
    is_interrupt: bool,
    is_typedef: bool,
    allows_omitted_declarator: bool,
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
            typedefs: BTreeMap::new(),
            struct_tags: BTreeMap::new(),
            struct_defs: Vec::new(),
            enum_tags: BTreeSet::new(),
            enum_constants: Vec::new(),
            enum_constant_by_name: BTreeMap::new(),
            global_value_names: BTreeSet::new(),
        }
    }

    /// Parses a full translation unit until the EOF token is reached.
    pub fn parse_translation_unit(&mut self) -> TranslationUnit {
        let mut items = Vec::new();
        while !self.is_eof() {
            if let Some(item) = self.parse_item() {
                items.push(item);
            }
        }
        TranslationUnit {
            items,
            struct_defs: self.struct_defs.clone(),
            enum_constants: self.enum_constants.clone(),
        }
    }

    /// Parses one top-level declaration or function definition.
    fn parse_item(&mut self) -> Option<Item> {
        let start = self.current_span().start;
        let decl = self.parse_decl_specifiers();

        if self.match_symbol(Symbol::Semicolon) {
            if decl.is_typedef {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.previous_span().end)),
                    "typedef declaration requires an alias name",
                    None,
                );
                return None;
            }
            if !decl.allows_omitted_declarator {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.previous_span().end)),
                    "declaration requires a declarator",
                    None,
                );
            }
            return None;
        }

        let (name, name_span, ty) = self.parse_declarator(decl.ty);

        if decl.is_typedef {
            if self.match_symbol(Symbol::LParen) {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.previous_span().end)),
                    "typedef of function types is not supported in phase 8",
                    Some("typedef scalar, pointer, array, or struct object types only".to_string()),
                );
                while !self.check_symbol(Symbol::Semicolon) && !self.is_eof() {
                    self.advance();
                }
                self.expect_symbol(Symbol::Semicolon);
                return None;
            }
            self.expect_symbol(Symbol::Semicolon);
            self.register_typedef(name, ty, Span::new(start, self.previous_span().end));
            return None;
        }

        self.register_global_value_name(&name, Span::new(start, name_span.end));

        if self.match_symbol(Symbol::LParen) {
            let params = self.parse_params();
            let span = Span::new(start, self.previous_span().end);
            if self.match_symbol(Symbol::LBrace) {
                let body = self.parse_block_after_open(span.start);
                return Some(Item::Function(FunctionDecl {
                    name,
                    return_type: ty,
                    storage_class: decl.storage_class,
                    is_interrupt: decl.is_interrupt,
                    params,
                    body: Some(body),
                    span: Span::new(start, self.previous_span().end),
                }));
            }
            self.expect_symbol(Symbol::Semicolon);
            Some(Item::Function(FunctionDecl {
                name,
                return_type: ty,
                storage_class: decl.storage_class,
                is_interrupt: decl.is_interrupt,
                params,
                body: None,
                span: Span::new(start, self.previous_span().end),
            }))
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
                Some(self.parse_initializer())
            } else {
                None
            };
            self.expect_symbol(Symbol::Semicolon);
            Some(Item::Global(VarDecl {
                name,
                ty,
                storage_class: decl.storage_class,
                initializer,
                span: Span::new(start, self.previous_span().end.max(name_span.end)),
            }))
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
            if decl.is_typedef {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.previous_span().end)),
                    "`typedef` is not valid in parameter declarations",
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
        if self.match_keyword(Keyword::Case) {
            let value = self.parse_expression();
            self.expect_symbol(Symbol::Colon);
            let body = Box::new(self.parse_statement());
            return Stmt::Case {
                value,
                body,
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_keyword(Keyword::Default) {
            self.expect_symbol(Symbol::Colon);
            let body = Box::new(self.parse_statement());
            return Stmt::Default {
                body,
                span: Span::new(start, self.previous_span().end),
            };
        }
        if self.match_keyword(Keyword::Switch) {
            self.expect_symbol(Symbol::LParen);
            let expr = self.parse_expression();
            self.expect_symbol(Symbol::RParen);
            let body = Box::new(self.parse_statement());
            return Stmt::Switch {
                expr,
                body,
                span: Span::new(start, self.previous_span().end),
            };
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
        if decl.is_typedef {
            self.diagnostics.error(
                "parser",
                Some(Span::new(start, self.previous_span().end)),
                "block-scope typedef declarations are not supported in phase 8",
                Some("declare typedef names at file scope".to_string()),
            );
        }
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
            Some(self.parse_initializer())
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

    /// Parses one initializer, supporting scalar expressions and aggregate initializer lists.
    fn parse_initializer(&mut self) -> Initializer {
        if self.match_symbol(Symbol::LBrace) {
            return self.parse_initializer_list(self.previous_span().start);
        }
        Initializer::Expr(self.parse_expression())
    }

    /// Parses one brace-enclosed initializer list for arrays and structs.
    fn parse_initializer_list(&mut self, start: usize) -> Initializer {
        let mut items = Vec::new();
        if self.match_symbol(Symbol::RBrace) {
            return Initializer::List(items, Span::new(start, self.previous_span().end));
        }

        loop {
            let designator = if self.match_symbol(Symbol::Dot) {
                let start_span = self.previous_span();
                let (field, end_span) = self.expect_identifier();
                self.expect_symbol(Symbol::Assign);
                Some(Designator::Field(
                    field,
                    Span::new(start_span.start, end_span.end),
                ))
            } else if self.match_symbol(Symbol::LBracket) {
                let designator_start = self.previous_span().start;
                let index = self.parse_expression();
                self.expect_symbol(Symbol::RBracket);
                let designator_end = self.previous_span().end;
                self.expect_symbol(Symbol::Assign);
                Some(Designator::Index(
                    index,
                    Span::new(designator_start, designator_end),
                ))
            } else {
                None
            };

            items.push(InitializerEntry {
                designator,
                initializer: self.parse_initializer(),
            });

            if self.match_symbol(Symbol::Comma) {
                if self.check_symbol(Symbol::RBrace) {
                    self.advance();
                    break;
                }
                continue;
            }
            self.expect_symbol(Symbol::RBrace);
            break;
        }

        Initializer::List(items, Span::new(start, self.previous_span().end))
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
        if self.looks_like_cast() {
            self.expect_symbol(Symbol::LParen);
            let ty = self.parse_type_name();
            self.expect_symbol(Symbol::RParen);
            let expr = self.parse_unary();
            return Expr {
                kind: ExprKind::Cast {
                    ty,
                    expr: Box::new(expr),
                },
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

            if self.match_symbol(Symbol::Dot) {
                let start = expr.span.start;
                let (field, _) = self.expect_identifier();
                expr = Expr {
                    kind: ExprKind::Member {
                        base: Box::new(expr),
                        field,
                    },
                    span: Span::new(start, self.previous_span().end),
                };
                continue;
            }

            if self.match_symbol(Symbol::Arrow) {
                let start = expr.span.start;
                let (field, _) = self.expect_identifier();
                expr = Expr {
                    kind: ExprKind::PointerMember {
                        base: Box::new(expr),
                        field,
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
        if let TokenKind::StringLiteral(bytes) = self.current().kind.clone() {
            self.advance();
            return Expr {
                kind: ExprKind::StringLiteral(bytes),
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
        let mut saw_rom = false;
        let mut is_interrupt = false;
        let mut is_typedef = false;
        let mut scalar = None::<ScalarType>;
        let mut explicit_type = None::<Type>;
        let mut allows_omitted_declarator = false;

        loop {
            let current_kind = self.current().kind.clone();
            match current_kind {
                TokenKind::Keyword(Keyword::Static) => {
                    storage = StorageClass::Static;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Extern) => {
                    storage = StorageClass::Extern;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Typedef) => {
                    is_typedef = true;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Const) => {
                    qualifiers.is_const = true;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Rom) => {
                    saw_rom = true;
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
                TokenKind::Keyword(Keyword::Struct) => {
                    if scalar.is_some() || explicit_type.is_some() {
                        self.diagnostics.error(
                            "parser",
                            Some(self.current_span()),
                            "duplicate type specifier",
                            None,
                        );
                    }
                    let (ty, omittable) = self.parse_struct_specifier();
                    explicit_type = Some(ty);
                    allows_omitted_declarator = omittable;
                }
                TokenKind::Keyword(Keyword::Enum) => {
                    if scalar.is_some() || explicit_type.is_some() {
                        self.diagnostics.error(
                            "parser",
                            Some(self.current_span()),
                            "duplicate type specifier",
                            None,
                        );
                    }
                    let (ty, omittable) = self.parse_enum_specifier();
                    explicit_type = Some(ty);
                    allows_omitted_declarator = omittable;
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
                TokenKind::Identifier(name) if self.typedefs.contains_key(&name) => {
                    if scalar.is_some() || explicit_type.is_some() {
                        break;
                    }
                    explicit_type = self.typedefs.get(&name).copied();
                    self.advance();
                }
                _ => break,
            }
        }

        let mut ty = if let Some(ty) = explicit_type {
            if saw_unsigned {
                self.diagnostics.error(
                    "parser",
                    Some(self.current_span()),
                    "`unsigned` cannot be combined with this type specifier",
                    None,
                );
            }
            ty
        } else {
            let scalar = scalar.unwrap_or_else(|| {
                self.diagnostics.error(
                    "parser",
                    Some(self.current_span()),
                    "expected type specifier",
                    Some(
                        "supported types: void, char, unsigned char, int, unsigned int, typedef names, enum, struct"
                            .to_string(),
                    ),
                );
                ScalarType::I16
            });
            Type::new(scalar)
        };
        ty = ty.with_qualifiers(qualifiers);
        if saw_rom {
            ty = ty.with_address_space(AddressSpace::Rom);
        }

        DeclSpecifiers {
            storage_class: storage,
            ty,
            is_interrupt,
            is_typedef,
            allows_omitted_declarator,
        }
    }

    /// Parses one `struct` specifier and records field layout metadata.
    fn parse_struct_specifier(&mut self) -> (Type, bool) {
        let start = self.current_span().start;
        self.expect_keyword(Keyword::Struct);

        let tag = if let TokenKind::Identifier(name) = self.current().kind.clone() {
            self.advance();
            Some(name)
        } else {
            None
        };

        if !self.match_symbol(Symbol::LBrace) {
            if let Some(tag) = tag {
                if let Some(struct_id) = self.struct_tags.get(&tag).copied() {
                    let size = self.struct_defs[struct_id].size;
                    return (Type::struct_type(struct_id, size), false);
                }
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.previous_span().end)),
                    format!("unknown struct tag `{tag}`"),
                    Some("define the struct before using it".to_string()),
                );
                return (Type::new(ScalarType::I16), false);
            }
            self.diagnostics.error(
                "parser",
                Some(Span::new(start, self.previous_span().end)),
                "anonymous struct type requires a field list",
                None,
            );
            return (Type::new(ScalarType::I16), false);
        }

        let placeholder = tag.as_ref().map(|tag_name| {
            if let Some(existing) = self.struct_tags.get(tag_name).copied() {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.current_span().start)),
                    format!("redefinition of struct `{tag_name}`"),
                    None,
                );
                existing
            } else {
                let id = self.struct_defs.len();
                self.struct_defs.push(StructDef {
                    id,
                    name: tag_name.clone(),
                    fields: Vec::new(),
                    size: 0,
                    span: Span::new(start, start),
                });
                self.struct_tags.insert(tag_name.clone(), id);
                id
            }
        });

        let mut fields = Vec::new();
        let mut seen_field_names = BTreeSet::new();
        let mut offset = 0usize;

        while !self.check_symbol(Symbol::RBrace) && !self.is_eof() {
            let field_start = self.current_span().start;
            let decl = self.parse_decl_specifiers();
            if decl.is_interrupt || decl.is_typedef || decl.storage_class != StorageClass::Auto {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(field_start, self.previous_span().end)),
                    "struct fields do not support storage classes, typedef, or interrupt qualifiers",
                    None,
                );
            }
            if decl.allows_omitted_declarator && self.check_symbol(Symbol::Semicolon) {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(field_start, self.current_span().end)),
                    "anonymous nested struct/enum fields are not supported in phase 11",
                    Some("name the field explicitly".to_string()),
                );
                self.advance();
                continue;
            }

            let (name, _, ty) = self.parse_declarator(decl.ty);
            self.expect_symbol(Symbol::Semicolon);

            if !seen_field_names.insert(name.clone()) {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(field_start, self.previous_span().end)),
                    format!("duplicate struct field `{name}`"),
                    None,
                );
                continue;
            }

            fields.push(StructField {
                name,
                ty,
                offset,
                span: Span::new(field_start, self.previous_span().end),
            });
            offset += ty.byte_width();
        }
        self.expect_symbol(Symbol::RBrace);

        let end = self.previous_span().end;
        if let Some(id) = placeholder {
            self.struct_defs[id] = StructDef {
                id,
                name: self.struct_defs[id].name.clone(),
                fields,
                size: offset,
                span: Span::new(start, end),
            };
            (Type::struct_type(id, offset), true)
        } else {
            let id = self.struct_defs.len();
            let struct_name = format!("__anon_struct_{id}");
            self.struct_defs.push(StructDef {
                id,
                name: struct_name,
                fields,
                size: offset,
                span: Span::new(start, end),
            });
            (Type::struct_type(id, offset), true)
        }
    }

    /// Parses one `enum` specifier and captures global enumerator constants.
    fn parse_enum_specifier(&mut self) -> (Type, bool) {
        let start = self.current_span().start;
        self.expect_keyword(Keyword::Enum);

        let tag = if let TokenKind::Identifier(name) = self.current().kind.clone() {
            self.advance();
            Some(name)
        } else {
            None
        };

        let mut defined_here = false;
        if self.match_symbol(Symbol::LBrace) {
            defined_here = true;
            let mut next_value = 0i64;
            while !self.check_symbol(Symbol::RBrace) && !self.is_eof() {
                let (name, span) = self.expect_identifier();
                let value = if self.match_symbol(Symbol::Assign) {
                    let expr = self.parse_expression();
                    if let Some(value) = self.eval_enum_const_expr(&expr) {
                        value
                    } else {
                        self.diagnostics.error(
                            "parser",
                            Some(expr.span),
                            "enum explicit value must be an integer constant expression",
                            None,
                        );
                        next_value
                    }
                } else {
                    next_value
                };

                if self.enum_constant_by_name.contains_key(&name) {
                    self.diagnostics.error(
                        "parser",
                        Some(span),
                        format!("duplicate enumerator `{name}`"),
                        None,
                    );
                } else {
                    if !(i64::from(i16::MIN)..=i64::from(i16::MAX)).contains(&value) {
                        self.diagnostics.error(
                            "parser",
                            Some(span),
                            format!("enumerator `{name}` value {value} is out of range for 16-bit enum representation"),
                            None,
                        );
                    }
                    self.enum_constant_by_name.insert(name.clone(), value);
                    self.enum_constants.push(EnumConstant { name, value, span });
                }
                next_value = value.saturating_add(1);

                if self.match_symbol(Symbol::Comma) {
                    continue;
                } else {
                    break;
                }
            }

            self.expect_symbol(Symbol::RBrace);

            if let Some(tag_name) = tag.as_ref() && !self.enum_tags.insert(tag_name.clone()) {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.previous_span().end)),
                    format!("redefinition of enum `{}`", tag_name),
                    None,
                );
            }
        } else if let Some(tag) = tag {
            if !self.enum_tags.contains(&tag) {
                self.diagnostics.error(
                    "parser",
                    Some(Span::new(start, self.previous_span().end)),
                    format!("unknown enum tag `{tag}`"),
                    Some("define the enum before using it".to_string()),
                );
            }
        } else {
            self.diagnostics.error(
                "parser",
                Some(Span::new(start, self.previous_span().end)),
                "anonymous enum type requires an enumerator list",
                None,
            );
        }

        (Type::new(ScalarType::I16), defined_here)
    }

    /// Evaluates an enum constant expression with integer-only operators.
    fn eval_enum_const_expr(&self, expr: &Expr) -> Option<i64> {
        match &expr.kind {
            ExprKind::IntLiteral(value) => Some(*value),
            ExprKind::StringLiteral(_) => None,
            ExprKind::Name(name) => self.enum_constant_by_name.get(name).copied(),
            ExprKind::Cast { expr, .. } => self.eval_enum_const_expr(expr),
            ExprKind::Unary { op, expr } => {
                let value = self.eval_enum_const_expr(expr)?;
                Some(match op {
                    UnaryOp::Negate => value.wrapping_neg(),
                    UnaryOp::LogicalNot => i64::from(value == 0),
                    UnaryOp::BitwiseNot => !value,
                })
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.eval_enum_const_expr(lhs)?;
                let rhs = self.eval_enum_const_expr(rhs)?;
                Some(match op {
                    BinaryOp::Add => lhs.wrapping_add(rhs),
                    BinaryOp::Sub => lhs.wrapping_sub(rhs),
                    BinaryOp::Multiply => lhs.wrapping_mul(rhs),
                    BinaryOp::Divide => {
                        if rhs == 0 {
                            return None;
                        }
                        lhs.wrapping_div(rhs)
                    }
                    BinaryOp::Modulo => {
                        if rhs == 0 {
                            return None;
                        }
                        lhs.wrapping_rem(rhs)
                    }
                    BinaryOp::ShiftLeft => lhs.wrapping_shl(rhs as u32),
                    BinaryOp::ShiftRight => lhs.wrapping_shr(rhs as u32),
                    BinaryOp::BitAnd => lhs & rhs,
                    BinaryOp::BitOr => lhs | rhs,
                    BinaryOp::BitXor => lhs ^ rhs,
                    BinaryOp::LogicalAnd => i64::from(lhs != 0 && rhs != 0),
                    BinaryOp::LogicalOr => i64::from(lhs != 0 || rhs != 0),
                    BinaryOp::Equal => i64::from(lhs == rhs),
                    BinaryOp::NotEqual => i64::from(lhs != rhs),
                    BinaryOp::Less => i64::from(lhs < rhs),
                    BinaryOp::LessEqual => i64::from(lhs <= rhs),
                    BinaryOp::Greater => i64::from(lhs > rhs),
                    BinaryOp::GreaterEqual => i64::from(lhs >= rhs),
                })
            }
            ExprKind::AddressOf(_)
            | ExprKind::Deref(_)
            | ExprKind::Index { .. }
            | ExprKind::Assign { .. }
            | ExprKind::Call { .. }
            | ExprKind::Member { .. }
            | ExprKind::PointerMember { .. }
            | ExprKind::SizeOfExpr(_)
            | ExprKind::SizeOfType(_) => None,
        }
    }

    /// Records a typedef alias and emits duplicate/conflict diagnostics.
    fn register_typedef(&mut self, name: String, ty: Type, span: Span) {
        if self.typedefs.contains_key(&name) {
            self.diagnostics.error(
                "parser",
                Some(span),
                format!("duplicate typedef `{name}`"),
                None,
            );
            return;
        }
        if self.global_value_names.contains(&name) {
            self.diagnostics.error(
                "parser",
                Some(span),
                format!("typedef `{name}` conflicts with object/function name"),
                Some("use a distinct typedef identifier in this phase".to_string()),
            );
            return;
        }
        self.typedefs.insert(name, ty);
    }

    /// Tracks one global object/function identifier to detect typedef conflicts.
    fn register_global_value_name(&mut self, name: &str, span: Span) {
        if self.typedefs.contains_key(name) {
            self.diagnostics.error(
                "parser",
                Some(span),
                format!("declaration `{name}` conflicts with typedef name"),
                Some("use a distinct object/function name in this phase".to_string()),
            );
        }
        self.global_value_names.insert(name.to_string());
    }

    /// Parses a named declarator with Phase 3 pointer and one-dimensional array suffixes.
    fn parse_declarator(&mut self, mut ty: Type) -> (String, Span, Type) {
        if self.check_symbol(Symbol::LParen) && self.peek_symbol(1, Symbol::Star) {
            let span = self.current_span();
            self.diagnostics.error(
                "parser",
                Some(span),
                "function pointer declarators are not supported in phase 8",
                Some("use direct function calls only for now".to_string()),
            );
            while !self.check_symbol(Symbol::RParen) && !self.is_eof() {
                self.advance();
            }
            self.expect_symbol(Symbol::RParen);
            return ("__unsupported".to_string(), span, ty);
        }
        while self.match_symbol(Symbol::Star) {
            let qualifiers = self.parse_pointer_qualifiers();
            if ty.pointer_depth >= MAX_POINTER_DEPTH as u8 {
                self.diagnostics.error(
                    "parser",
                    Some(self.previous_span()),
                    format!(
                        "pointer depth greater than {MAX_POINTER_DEPTH} is not supported in phase 12"
                    ),
                    Some("reduce the number of nested `*` levels".to_string()),
                );
                continue;
            }
            ty = ty.pointer_to_with_qualifiers(qualifiers);
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
        if decl.is_typedef {
            self.diagnostics.error(
                "parser",
                Some(self.previous_span()),
                "`typedef` storage class is not valid in type names",
                None,
            );
        }
        if decl.storage_class != StorageClass::Auto {
            self.diagnostics.error(
                "parser",
                Some(self.previous_span()),
                "storage-class specifiers are not valid in type names",
                None,
            );
        }
        self.parse_abstract_declarator(decl.ty)
    }

    /// Returns true when the next tokens match a cast-style `(type-name)` prefix.
    fn looks_like_cast(&self) -> bool {
        if !self.check_symbol(Symbol::LParen) {
            return false;
        }
        let mut cursor = self.index + 1;
        let mut saw_type = false;

        loop {
            let Some(token) = self.tokens.get(cursor) else {
                return false;
            };
            match &token.kind {
                TokenKind::Keyword(Keyword::Const)
                | TokenKind::Keyword(Keyword::Rom)
                | TokenKind::Keyword(Keyword::Volatile)
                | TokenKind::Keyword(Keyword::Unsigned) => {
                    cursor += 1;
                }
                TokenKind::Keyword(Keyword::Void)
                | TokenKind::Keyword(Keyword::Char)
                | TokenKind::Keyword(Keyword::Int) => {
                    saw_type = true;
                    cursor += 1;
                }
                TokenKind::Keyword(Keyword::Struct) | TokenKind::Keyword(Keyword::Enum) => {
                    saw_type = true;
                    cursor += 1;
                    if self
                        .tokens
                        .get(cursor)
                        .is_some_and(|next| matches!(next.kind, TokenKind::Identifier(_)))
                    {
                        cursor += 1;
                    }
                    if self
                        .tokens
                        .get(cursor)
                        .is_some_and(|next| matches!(next.kind, TokenKind::Symbol(Symbol::LBrace)))
                    {
                        return false;
                    }
                }
                TokenKind::Identifier(name) if self.typedefs.contains_key(name) => {
                    saw_type = true;
                    cursor += 1;
                }
                _ => break,
            }
        }

        if !saw_type {
            return false;
        }

        while self
            .tokens
            .get(cursor)
            .is_some_and(|token| matches!(token.kind, TokenKind::Symbol(Symbol::Star)))
        {
            cursor += 1;
            while self.tokens.get(cursor).is_some_and(|token| {
                matches!(
                    token.kind,
                    TokenKind::Keyword(Keyword::Const) | TokenKind::Keyword(Keyword::Volatile)
                )
            }) {
                cursor += 1;
            }
        }

        self.tokens
            .get(cursor)
            .is_some_and(|token| matches!(token.kind, TokenKind::Symbol(Symbol::RParen)))
    }

    /// Parses the pointer and array suffix pieces of an abstract declarator.
    fn parse_abstract_declarator(&mut self, mut ty: Type) -> Type {
        while self.match_symbol(Symbol::Star) {
            let qualifiers = self.parse_pointer_qualifiers();
            if ty.pointer_depth >= MAX_POINTER_DEPTH as u8 {
                self.diagnostics.error(
                    "parser",
                    Some(self.previous_span()),
                    format!(
                        "pointer depth greater than {MAX_POINTER_DEPTH} is not supported in phase 12"
                    ),
                    Some("reduce the number of nested `*` levels".to_string()),
                );
                continue;
            }
            ty = ty.pointer_to_with_qualifiers(qualifiers);
        }
        self.parse_type_suffixes(ty)
    }

    /// Parses qualifiers attached to one `*` in a pointer declarator.
    fn parse_pointer_qualifiers(&mut self) -> Qualifiers {
        let mut qualifiers = Qualifiers::default();
        loop {
            match self.current().kind {
                TokenKind::Keyword(Keyword::Const) => {
                    qualifiers.is_const = true;
                    self.advance();
                }
                TokenKind::Keyword(Keyword::Volatile) => {
                    qualifiers.is_volatile = true;
                    self.advance();
                }
                _ => break,
            }
        }
        qualifiers
    }

    /// Parses the supported one-dimensional array suffix for one declarator.
    fn parse_type_suffixes(&mut self, mut ty: Type) -> Type {
        if self.match_symbol(Symbol::LBracket) {
            let len = if self.check_symbol(Symbol::RBracket) {
                0
            } else {
                self.parse_array_len()
            };
            self.expect_symbol(Symbol::RBracket);
            ty = ty.array_of(len);
            while self.match_symbol(Symbol::LBracket) {
                self.diagnostics.error(
                    "parser",
                    Some(self.previous_span()),
                    "multidimensional arrays are not supported in phase 8",
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

    /// Returns true when the current token can start a declaration.
    fn is_decl_start(&self) -> bool {
        matches!(
            self.current().kind,
            TokenKind::Keyword(Keyword::Static)
                | TokenKind::Keyword(Keyword::Extern)
                | TokenKind::Keyword(Keyword::Typedef)
                | TokenKind::Keyword(Keyword::Const)
                | TokenKind::Keyword(Keyword::Rom)
                | TokenKind::Keyword(Keyword::Volatile)
                | TokenKind::Keyword(Keyword::Unsigned)
                | TokenKind::Keyword(Keyword::Void)
                | TokenKind::Keyword(Keyword::Char)
                | TokenKind::Keyword(Keyword::Int)
                | TokenKind::Keyword(Keyword::Enum)
                | TokenKind::Keyword(Keyword::Struct)
                | TokenKind::Keyword(Keyword::Interrupt)
        ) || matches!(&self.current().kind, TokenKind::Identifier(name) if self.typedefs.contains_key(name))
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

#[cfg(test)]
mod tests {
    use super::Parser;
    use crate::common::source::{PreprocessedSource, SourceId, SourcePoint};
    use crate::diagnostics::{DiagnosticBag, WarningProfile};
    use crate::frontend::lexer::Lexer;

    fn parse_source(source: &str) -> (String, DiagnosticBag) {
        let origin = SourcePoint {
            file: SourceId(0),
            line: 1,
            column: 1,
        };
        let mut preprocessed = PreprocessedSource::new();
        preprocessed.push_str(source, origin);

        let mut diagnostics = DiagnosticBag::new(WarningProfile::default());
        let tokens = Lexer::new(&preprocessed, &mut diagnostics).tokenize();
        let ast = Parser::new(tokens, &preprocessed, &mut diagnostics).parse_translation_unit();
        (ast.render(), diagnostics)
    }

    #[test]
    /// Verifies one basic switch statement parses into AST output with case/default labels.
    fn parses_phase9_switch_case_default() {
        let (ast, diagnostics) = parse_source(
            "\
void main(void) {
    switch (mode) {
        case 0:
            PORTB = 0;
            break;
        default:
            break;
    }
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("switch mode"));
        assert!(ast.contains("case 0"));
        assert!(ast.contains("default"));
    }

    #[test]
    /// Verifies nested switch statements parse without parser diagnostics.
    fn parses_phase9_nested_switch() {
        let (ast, diagnostics) = parse_source(
            "\
void main(void) {
    switch (outer) {
        case 1:
            switch (inner) {
                case 2:
                    break;
                default:
                    break;
            }
            break;
        default:
            break;
    }
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.matches("switch ").count() >= 2);
        assert!(ast.contains("case 2"));
    }

    #[test]
    /// Verifies case/default labels parse as ordinary statements inside a switch block.
    fn parses_phase9_case_label_sequence() {
        let (ast, diagnostics) = parse_source(
            "\
void main(void) {
    switch (x) {
        case 1:
        case 2:
            value = 3;
            break;
    }
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("case 1"));
        assert!(ast.contains("case 2"));
    }

    #[test]
    /// Verifies string literals parse as ordinary expressions and keep supported escapes.
    fn parses_phase10_string_literal_expression() {
        let (ast, diagnostics) = parse_source(
            "\
void main(void) {
    PORTB = \"line\\n\";
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("\"line\\n\""));
    }

    #[test]
    /// Verifies one omitted-size array declarator parses for later semantic inference.
    fn parses_phase10_unsized_array_string_initializer() {
        let (ast, diagnostics) = parse_source(
            "\
char msg[] = \"OK\";
void main(void) {
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("global char[] msg"));
    }

    #[test]
    /// Verifies nested structs and array fields parse together for later layout analysis.
    fn parses_phase11_nested_structs_and_array_fields() {
        let (ast, diagnostics) = parse_source(
            "\
struct Point {
    unsigned char x;
    unsigned char y;
};
struct DeviceConfig {
    struct Point led;
    unsigned char name[4];
};
void main(void) {
    struct DeviceConfig config;
    PORTB = config.led.x + config.name[0];
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("struct DeviceConfig"));
        assert!(ast.contains("config.led.x"));
        assert!(ast.contains("config.name[0]"));
    }

    #[test]
    /// Verifies designated initializers parse for both struct fields and array indices.
    fn parses_phase11_designated_initializers() {
        let (ast, diagnostics) = parse_source(
            "\
struct Point {
    unsigned char x;
    unsigned char y;
};
struct Point point = {.x = 1, .y = 2};
unsigned char table[4] = {[0] = 1, [3] = 9};
void main(void) {
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("point"));
        assert!(ast.contains("table"));
    }

    #[test]
    /// Verifies whole-struct assignment syntax parses as an ordinary assignment statement.
    fn parses_phase11_struct_assignment_statement() {
        let (ast, diagnostics) = parse_source(
            "\
struct Pair {
    unsigned char x;
    unsigned char y;
};
void main(void) {
    struct Pair a;
    struct Pair b;
    a = b;
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("expr (a = b)"));
    }

    #[test]
    /// Verifies pointer-to-pointer declarators parse as ordinary nested data pointers.
    fn parses_phase12_pointer_to_pointer_declarator() {
        let (ast, diagnostics) = parse_source(
            "\
unsigned char value;
unsigned char *p;
unsigned char **pp;
void main(void) {
    pp = &p;
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("unsigned char** pp"));
    }

    #[test]
    /// Verifies const-qualified pointer forms preserve qualifier placement around each `*`.
    fn parses_phase12_const_qualified_pointer_forms() {
        let (ast, diagnostics) = parse_source(
            "\
unsigned char x;
const unsigned char *p;
unsigned char * const p2 = &x;
const unsigned char * const p3 = &x;
void main(void) {
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("const unsigned char* p"));
        assert!(ast.contains("unsigned char* const p2"));
        assert!(ast.contains("const unsigned char* const p3"));
    }

    #[test]
    /// Verifies explicit `__rom` byte-array declarations and ROM read intrinsics parse cleanly.
    fn parses_phase13_rom_byte_array_and_read() {
        let (ast, diagnostics) = parse_source(
            "\
const __rom unsigned char table[] = {1, 2, 3};
void main(void) {
    PORTB = __rom_read8(table, 1);
}
",
        );

        assert!(!diagnostics.has_errors(), "{diagnostics}");
        assert!(ast.contains("global const __rom unsigned char[] table"));
        assert!(ast.contains("__rom_read8(table, 1)"));
    }
}
