use std::collections::{BTreeMap, BTreeSet};

use crate::backend::pic16::devices::TargetDevice;
use crate::common::integer::infer_integer_literal_type;
use crate::common::source::Span;
use crate::diagnostics::DiagnosticBag;

use super::ast::{
    BinaryOp, Expr, ExprKind, FunctionDecl, Item, Stmt, TranslationUnit, UnaryOp, VarDecl,
};
use super::types::{CastKind, ScalarType, StorageClass, Type};

pub type SymbolId = usize;

#[derive(Clone, Debug)]
pub struct TypedProgram {
    pub symbols: Vec<Symbol>,
    pub globals: Vec<TypedGlobal>,
    pub functions: Vec<TypedFunction>,
}

#[derive(Clone, Debug)]
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub ty: Type,
    pub storage_class: StorageClass,
    pub kind: SymbolKind,
    pub span: Span,
    pub fixed_address: Option<u16>,
    pub is_defined: bool,
    pub is_referenced: bool,
    pub parameter_types: Vec<Type>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Function,
    Global,
    Local,
    Param,
    DeviceRegister,
}

#[derive(Clone, Debug)]
pub struct TypedGlobal {
    pub symbol: SymbolId,
    pub initializer: Option<TypedExpr>,
}

#[derive(Clone, Debug)]
pub struct TypedFunction {
    pub symbol: SymbolId,
    pub params: Vec<SymbolId>,
    pub locals: Vec<SymbolId>,
    pub body: Option<TypedStmt>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypedStmt {
    Block(Vec<TypedStmt>, Span),
    VarDecl(SymbolId, Option<TypedExpr>, Span),
    Expr(TypedExpr, Span),
    If {
        condition: TypedExpr,
        then_branch: Box<TypedStmt>,
        else_branch: Option<Box<TypedStmt>>,
        span: Span,
    },
    While {
        condition: TypedExpr,
        body: Box<TypedStmt>,
        span: Span,
    },
    DoWhile {
        body: Box<TypedStmt>,
        condition: TypedExpr,
        span: Span,
    },
    For {
        init: Option<Box<TypedStmt>>,
        condition: Option<TypedExpr>,
        step: Option<TypedExpr>,
        body: Box<TypedStmt>,
        span: Span,
    },
    Return(Option<TypedExpr>, Span),
    Break(Span),
    Continue(Span),
    Empty(Span),
}

#[derive(Clone, Debug)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypedExprKind {
    IntLiteral(i64),
    Symbol(SymbolId),
    Unary {
        op: UnaryOp,
        expr: Box<TypedExpr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<TypedExpr>,
        rhs: Box<TypedExpr>,
    },
    Assign {
        target: SymbolId,
        value: Box<TypedExpr>,
    },
    Call {
        function: SymbolId,
        args: Vec<TypedExpr>,
    },
    Cast {
        kind: CastKind,
        expr: Box<TypedExpr>,
    },
}

pub struct SemanticAnalyzer<'a> {
    target: &'a TargetDevice,
    symbols: Vec<Symbol>,
    globals: Vec<TypedGlobal>,
    functions: Vec<TypedFunction>,
    globals_by_name: BTreeMap<String, SymbolId>,
    scopes: Vec<BTreeMap<String, SymbolId>>,
    current_function: Option<SymbolId>,
    loop_depth: usize,
}

impl<'a> SemanticAnalyzer<'a> {
    /// Creates a semantic analyzer primed with target-specific device registers.
    pub fn new(target: &'a TargetDevice) -> Self {
        let mut analyzer = Self {
            target,
            symbols: Vec::new(),
            globals: Vec::new(),
            functions: Vec::new(),
            globals_by_name: BTreeMap::new(),
            scopes: Vec::new(),
            current_function: None,
            loop_depth: 0,
        };
        analyzer.seed_device_registers();
        analyzer
    }

    /// Performs declaration checking, typing, and symbol resolution for one translation unit.
    pub fn analyze(
        mut self,
        unit: TranslationUnit,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedProgram> {
        for item in &unit.items {
            self.declare_item(item, diagnostics);
        }
        if diagnostics.has_errors() {
            return None;
        }

        for item in unit.items {
            match item {
                Item::Function(function) => self.define_function(function, diagnostics),
                Item::Global(global) => self.define_global(global, diagnostics),
            }
        }

        self.emit_warnings(diagnostics);
        if diagnostics.has_errors() {
            None
        } else {
            Some(TypedProgram {
                symbols: self.symbols,
                globals: self.globals,
                functions: self.functions,
            })
        }
    }

    /// Seeds the global symbol table with volatile device-register symbols from the target.
    fn seed_device_registers(&mut self) {
        for register in self.target.sfrs {
            let symbol = self.insert_symbol(Symbol {
                id: self.symbols.len(),
                name: register.name.to_string(),
                ty: Type::new(ScalarType::U8).with_qualifiers(super::types::Qualifiers {
                    is_const: false,
                    is_volatile: true,
                }),
                storage_class: StorageClass::Extern,
                kind: SymbolKind::DeviceRegister,
                span: Span::new(0, 0),
                fixed_address: Some(register.address),
                is_defined: true,
                is_referenced: false,
                parameter_types: Vec::new(),
            });
            self.globals_by_name.insert(register.name.to_string(), symbol);
        }
    }

    /// Registers a top-level declaration before bodies and initializers are analyzed.
    fn declare_item(&mut self, item: &Item, diagnostics: &mut DiagnosticBag) {
        match item {
            Item::Function(function) => self.declare_function_signature(function, diagnostics),
            Item::Global(global) => {
                if self.globals_by_name.contains_key(&global.name) {
                    if let Some(existing) = self.globals_by_name.get(&global.name).copied()
                        && self.symbols[existing].kind == SymbolKind::DeviceRegister
                        && global.storage_class == StorageClass::Extern
                    {
                        return;
                    }
                    diagnostics.error(
                        "semantic",
                        Some(global.span),
                        format!("redefinition of symbol `{}`", global.name),
                        None,
                    );
                    return;
                }
                if !global.ty.is_supported_codegen_scalar() || global.ty.is_void() {
                    diagnostics.error(
                        "semantic",
                        Some(global.span),
                        format!("type `{}` is not lowered in phase 2", global.ty),
                        None,
                    );
                }
                let symbol = self.insert_symbol(Symbol {
                    id: self.symbols.len(),
                    name: global.name.clone(),
                    ty: global.ty,
                    storage_class: global.storage_class,
                    kind: SymbolKind::Global,
                    span: global.span,
                    fixed_address: None,
                    is_defined: global.initializer.is_none(),
                    is_referenced: false,
                    parameter_types: Vec::new(),
                });
                self.globals_by_name.insert(global.name.clone(), symbol);
            }
        }
    }

    /// Adds a function signature to the global symbol table and validates ABI limits.
    fn declare_function_signature(
        &mut self,
        function: &FunctionDecl,
        diagnostics: &mut DiagnosticBag,
    ) {
        if let Some(existing) = self.globals_by_name.get(&function.name).copied() {
            let existing_symbol = &self.symbols[existing];
            if existing_symbol.kind != SymbolKind::Function {
                diagnostics.error(
                    "semantic",
                    Some(function.span),
                    format!("symbol `{}` already declared as non-function", function.name),
                    None,
                );
            }
            return;
        }

        if !function.return_type.is_supported_codegen_scalar() {
            diagnostics.error(
                "semantic",
                Some(function.span),
                format!("return type `{}` is not lowered in phase 2", function.return_type),
                None,
            );
        }
        if function.params.len() > 2 {
            diagnostics.error(
                "semantic",
                Some(function.span),
                "phase 2 ABI supports at most two parameters",
                None,
            );
        }

        let parameter_types = function.params.iter().map(|param| param.ty).collect::<Vec<_>>();
        let symbol = self.insert_symbol(Symbol {
            id: self.symbols.len(),
            name: function.name.clone(),
            ty: function.return_type,
            storage_class: function.storage_class,
            kind: SymbolKind::Function,
            span: function.span,
            fixed_address: None,
            is_defined: function.body.is_none(),
            is_referenced: false,
            parameter_types,
        });
        self.globals_by_name.insert(function.name.clone(), symbol);
    }

    /// Analyzes and records one global variable definition and its optional initializer.
    fn define_global(&mut self, global: VarDecl, diagnostics: &mut DiagnosticBag) {
        let Some(symbol) = self.globals_by_name.get(&global.name).copied() else {
            return;
        };
        if self.symbols[symbol].kind == SymbolKind::DeviceRegister {
            return;
        }

        self.symbols[symbol].is_defined = true;
        let initializer = global.initializer.as_ref().and_then(|expr| {
            let expr = self.analyze_expr(expr, diagnostics)?;
            let coerced = self.coerce_expr(
                expr,
                global.ty,
                diagnostics,
                "global initializer",
                false,
            );
            Some(coerced)
        });
        if let Some(initializer) = initializer.as_ref()
            && !is_constant_expression(initializer)
        {
            diagnostics.error(
                "semantic",
                Some(initializer.span),
                "global initializer must be a constant expression",
                None,
            );
        }

        self.globals.push(TypedGlobal {
            symbol,
            initializer,
        });
    }

    /// Analyzes one function body, parameters, and scoped local symbols.
    fn define_function(&mut self, function: FunctionDecl, diagnostics: &mut DiagnosticBag) {
        let Some(symbol) = self.globals_by_name.get(&function.name).copied() else {
            return;
        };
        if self.symbols[symbol].is_defined && function.body.is_some() && self.has_body(symbol) {
            diagnostics.error(
                "semantic",
                Some(function.span),
                format!("redefinition of function `{}`", function.name),
                None,
            );
            return;
        }

        self.current_function = Some(symbol);
        self.push_scope();

        let mut params = Vec::new();
        for param in function.params {
            if !param.ty.is_supported_codegen_scalar() || param.ty.is_void() {
                diagnostics.error(
                    "semantic",
                    Some(param.span),
                    format!("parameter `{}` uses unsupported type `{}`", param.name, param.ty),
                    None,
                );
            }
            let param_id = self.insert_symbol(Symbol {
                id: self.symbols.len(),
                name: param.name.clone(),
                ty: param.ty,
                storage_class: param.storage_class,
                kind: SymbolKind::Param,
                span: param.span,
                fixed_address: None,
                is_defined: true,
                is_referenced: false,
                parameter_types: Vec::new(),
            });
            self.scopes
                .last_mut()
                .expect("scope exists")
                .insert(param.name, param_id);
            params.push(param_id);
        }

        let body = function
            .body
            .as_ref()
            .map(|stmt| self.analyze_stmt(stmt, diagnostics));

        let locals = self.current_scope_symbols();
        self.pop_scope();
        self.current_function = None;
        self.symbols[symbol].is_defined = function.body.is_some();

        self.functions.push(TypedFunction {
            symbol,
            params,
            locals,
            body,
            return_type: function.return_type,
            span: function.span,
        });
    }

    /// Analyzes one statement and all nested expressions using the current scope state.
    fn analyze_stmt(&mut self, stmt: &Stmt, diagnostics: &mut DiagnosticBag) -> TypedStmt {
        match stmt {
            Stmt::Block(statements, span) => {
                self.push_scope();
                let typed = statements
                    .iter()
                    .map(|statement| self.analyze_stmt(statement, diagnostics))
                    .collect();
                self.pop_scope();
                TypedStmt::Block(typed, *span)
            }
            Stmt::VarDecl(decl) => {
                if !decl.ty.is_supported_codegen_scalar() || decl.ty.is_void() {
                    diagnostics.error(
                        "semantic",
                        Some(decl.span),
                        format!("local `{}` uses unsupported type `{}`", decl.name, decl.ty),
                        None,
                    );
                }
                let symbol = self.insert_scoped_symbol(
                    decl.name.clone(),
                    decl.ty,
                    decl.storage_class,
                    SymbolKind::Local,
                    decl.span,
                );
                let initializer = decl.initializer.as_ref().and_then(|expr| {
                    let expr = self.analyze_expr(expr, diagnostics)?;
                    Some(self.coerce_expr(expr, decl.ty, diagnostics, "local initializer", true))
                });
                TypedStmt::VarDecl(symbol, initializer, decl.span)
            }
            Stmt::Expr(expr, span) => TypedStmt::Expr(
                self.analyze_expr(expr, diagnostics)
                    .unwrap_or_else(|| zero_expr(*span)),
                *span,
            ),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => TypedStmt::If {
                condition: self
                    .analyze_expr(condition, diagnostics)
                    .unwrap_or_else(|| zero_expr(*span)),
                then_branch: Box::new(self.analyze_stmt(then_branch, diagnostics)),
                else_branch: else_branch
                    .as_ref()
                    .map(|branch| Box::new(self.analyze_stmt(branch, diagnostics))),
                span: *span,
            },
            Stmt::While {
                condition,
                body,
                span,
            } => {
                self.loop_depth += 1;
                let typed = TypedStmt::While {
                    condition: self
                        .analyze_expr(condition, diagnostics)
                        .unwrap_or_else(|| zero_expr(*span)),
                    body: Box::new(self.analyze_stmt(body, diagnostics)),
                    span: *span,
                };
                self.loop_depth -= 1;
                typed
            }
            Stmt::DoWhile {
                body,
                condition,
                span,
            } => {
                self.loop_depth += 1;
                let typed = TypedStmt::DoWhile {
                    body: Box::new(self.analyze_stmt(body, diagnostics)),
                    condition: self
                        .analyze_expr(condition, diagnostics)
                        .unwrap_or_else(|| zero_expr(*span)),
                    span: *span,
                };
                self.loop_depth -= 1;
                typed
            }
            Stmt::For {
                init,
                condition,
                step,
                body,
                span,
            } => {
                self.loop_depth += 1;
                let typed = TypedStmt::For {
                    init: init
                        .as_ref()
                        .map(|statement| Box::new(self.analyze_stmt(statement, diagnostics))),
                    condition: condition
                        .as_ref()
                        .and_then(|expr| self.analyze_expr(expr, diagnostics)),
                    step: step
                        .as_ref()
                        .and_then(|expr| self.analyze_expr(expr, diagnostics)),
                    body: Box::new(self.analyze_stmt(body, diagnostics)),
                    span: *span,
                };
                self.loop_depth -= 1;
                typed
            }
            Stmt::Return(expr, span) => {
                let typed = expr.as_ref().and_then(|value| self.analyze_expr(value, diagnostics));
                let typed = if let Some(current_function) = self.current_function {
                    let return_type = self.symbols[current_function].ty;
                    if return_type.is_void() && typed.is_some() {
                        diagnostics.error(
                            "semantic",
                            Some(*span),
                            "void function cannot return a value",
                            None,
                        );
                        None
                    } else if !return_type.is_void() && typed.is_none() {
                        diagnostics.error(
                            "semantic",
                            Some(*span),
                            "non-void function must return a value",
                            None,
                        );
                        None
                    } else {
                        typed.map(|expr| {
                            self.coerce_expr(expr, return_type, diagnostics, "return value", true)
                        })
                    }
                } else {
                    typed
                };
                TypedStmt::Return(typed, *span)
            }
            Stmt::Break(span) => {
                if self.loop_depth == 0 {
                    diagnostics.error("semantic", Some(*span), "`break` outside loop", None);
                }
                TypedStmt::Break(*span)
            }
            Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    diagnostics.error("semantic", Some(*span), "`continue` outside loop", None);
                }
                TypedStmt::Continue(*span)
            }
            Stmt::Empty(span) => TypedStmt::Empty(*span),
        }
    }

    /// Analyzes one expression into a typed form suitable for IR lowering.
    fn analyze_expr(&mut self, expr: &Expr, diagnostics: &mut DiagnosticBag) -> Option<TypedExpr> {
        let typed = match &expr.kind {
            ExprKind::IntLiteral(value) => TypedExpr {
                kind: TypedExprKind::IntLiteral(*value),
                ty: infer_integer_literal_type(*value),
                span: expr.span,
            },
            ExprKind::Name(name) => {
                let symbol = self
                    .resolve_name(name)
                    .or_else(|| self.globals_by_name.get(name).copied());
                let Some(symbol) = symbol else {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        format!("undefined symbol `{name}`"),
                        None,
                    );
                    return None;
                };
                self.symbols[symbol].is_referenced = true;
                TypedExpr {
                    kind: TypedExprKind::Symbol(symbol),
                    ty: self.symbols[symbol].ty,
                    span: expr.span,
                }
            }
            ExprKind::Unary { op, expr: value } => {
                let value = self.analyze_expr(value, diagnostics)?;
                if !value.ty.is_integer() {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        "unary operator requires an integer operand",
                        None,
                    );
                    return None;
                }
                let result_ty = match op {
                    UnaryOp::LogicalNot => Type::new(ScalarType::U8),
                    UnaryOp::Negate | UnaryOp::BitwiseNot => value.ty,
                };
                TypedExpr {
                    kind: TypedExprKind::Unary {
                        op: *op,
                        expr: Box::new(value),
                    },
                    ty: result_ty,
                    span: expr.span,
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.analyze_expr(lhs, diagnostics)?;
                let rhs = self.analyze_expr(rhs, diagnostics)?;
                let (lhs, rhs, operand_ty) =
                    self.balance_binary_operands(*op, lhs, rhs, diagnostics, expr.span);
                let result_ty = match op {
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual
                    | BinaryOp::LogicalAnd
                    | BinaryOp::LogicalOr => Type::new(ScalarType::U8),
                    _ => operand_ty,
                };
                TypedExpr {
                    kind: TypedExprKind::Binary {
                        op: *op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty: result_ty,
                    span: expr.span,
                }
            }
            ExprKind::Assign { target, value } => {
                let target = self.analyze_expr(target, diagnostics)?;
                let TypedExprKind::Symbol(target_symbol) = target.kind else {
                    diagnostics.error(
                        "semantic",
                        Some(target.span),
                        "left side of assignment must be an lvalue name",
                        None,
                    );
                    return None;
                };
                let value = self.analyze_expr(value, diagnostics)?;
                let coerced = self.coerce_expr(
                    value,
                    self.symbols[target_symbol].ty,
                    diagnostics,
                    "assignment",
                    true,
                );
                TypedExpr {
                    kind: TypedExprKind::Assign {
                        target: target_symbol,
                        value: Box::new(coerced),
                    },
                    ty: self.symbols[target_symbol].ty,
                    span: expr.span,
                }
            }
            ExprKind::Call { callee, args } => {
                let Some(function) = self.globals_by_name.get(callee).copied() else {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        format!("undefined function `{callee}`"),
                        None,
                    );
                    return None;
                };
                if self.symbols[function].kind != SymbolKind::Function {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        format!("symbol `{callee}` is not callable"),
                        None,
                    );
                    return None;
                }
                self.symbols[function].is_referenced = true;

                let parameter_types = self.symbols[function].parameter_types.clone();
                if args.len() != parameter_types.len() {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        format!(
                            "function `{callee}` expects {} argument(s), got {}",
                            parameter_types.len(),
                            args.len()
                        ),
                        None,
                    );
                }

                let mut typed_args = Vec::new();
                for (index, argument) in args.iter().enumerate() {
                    let Some(arg) = self.analyze_expr(argument, diagnostics) else {
                        continue;
                    };
                    let Some(param_ty) = parameter_types.get(index).copied() else {
                        typed_args.push(arg);
                        continue;
                    };
                    typed_args.push(self.coerce_expr(arg, param_ty, diagnostics, "function argument", true));
                }
                TypedExpr {
                    kind: TypedExprKind::Call {
                        function,
                        args: typed_args,
                    },
                    ty: self.symbols[function].ty,
                    span: expr.span,
                }
            }
        };
        Some(typed)
    }

    /// Harmonizes binary operand widths and signedness before typed expression construction.
    fn balance_binary_operands(
        &mut self,
        op: BinaryOp,
        lhs: TypedExpr,
        rhs: TypedExpr,
        diagnostics: &mut DiagnosticBag,
        span: Span,
    ) -> (TypedExpr, TypedExpr, Type) {
        if !lhs.ty.is_integer() || !rhs.ty.is_integer() {
            diagnostics.error(
                "semantic",
                Some(span),
                "binary operator requires integer operands",
                None,
            );
            return (lhs, rhs, Type::new(ScalarType::U8));
        }

        if lhs.ty == rhs.ty {
            let result_ty = lhs.ty;
            return (lhs, rhs, result_ty);
        }

        if matches!(lhs.kind, TypedExprKind::IntLiteral(_)) {
            let result_ty = rhs.ty;
            let lhs = self.coerce_expr(lhs, rhs.ty, diagnostics, "integer literal", false);
            return (lhs, rhs, result_ty);
        }
        if matches!(rhs.kind, TypedExprKind::IntLiteral(_)) {
            let result_ty = lhs.ty;
            let rhs = self.coerce_expr(rhs, lhs.ty, diagnostics, "integer literal", false);
            return (lhs, rhs, result_ty);
        }

        if lhs.ty.bit_width() != rhs.ty.bit_width() {
            let target_ty = if lhs.ty.bit_width() > rhs.ty.bit_width() {
                lhs.ty
            } else {
                rhs.ty
            };
            let lhs = self.coerce_expr(lhs, target_ty, diagnostics, "binary operand", false);
            let rhs = self.coerce_expr(rhs, target_ty, diagnostics, "binary operand", false);
            return (lhs, rhs, target_ty);
        }

        diagnostics.error(
            "semantic",
            Some(span),
            format!(
                "mixed signedness for `{op:?}` with equal-width operands is not supported in phase 2"
            ),
            Some("use matching signedness or add an explicit narrowing/widening step".to_string()),
        );
        let result_ty = lhs.ty;
        let rhs = self.coerce_expr(rhs, lhs.ty, diagnostics, "binary operand", false);
        (lhs, rhs, result_ty)
    }

    /// Inserts an explicit semantic cast or literal retagging to reach a target type.
    fn coerce_expr(
        &mut self,
        expr: TypedExpr,
        target_ty: Type,
        diagnostics: &mut DiagnosticBag,
        context: &str,
        warn_on_truncate: bool,
    ) -> TypedExpr {
        if expr.ty == target_ty {
            return expr;
        }
        if !expr.ty.is_integer() || !target_ty.is_integer() {
            diagnostics.error(
                "semantic",
                Some(expr.span),
                format!("cannot coerce `{}` to `{}` in {context}", expr.ty, target_ty),
                None,
            );
            return expr;
        }

        if warn_on_truncate && expr.ty.bit_width() > target_ty.bit_width() {
            diagnostics.warning(
                "semantic",
                Some(expr.span),
                format!("conversion from `{}` to `{}` truncates", expr.ty, target_ty),
                "W1001",
            );
        }

        if matches!(expr.kind, TypedExprKind::IntLiteral(_)) {
            return TypedExpr {
                kind: expr.kind,
                ty: target_ty,
                span: expr.span,
            };
        }

        let kind = if expr.ty.bit_width() < target_ty.bit_width() {
            if expr.ty.is_signed() {
                CastKind::SignExtend
            } else {
                CastKind::ZeroExtend
            }
        } else if expr.ty.bit_width() > target_ty.bit_width() {
            CastKind::Truncate
        } else {
            CastKind::Bitcast
        };

        let span = expr.span;
        TypedExpr {
            kind: TypedExprKind::Cast {
                kind,
                expr: Box::new(expr),
            },
            ty: target_ty,
            span,
        }
    }

    /// Assigns a stable symbol id and stores a symbol in the global table.
    fn insert_symbol(&mut self, mut symbol: Symbol) -> SymbolId {
        let id = self.symbols.len();
        symbol.id = id;
        self.symbols.push(symbol);
        id
    }

    /// Inserts a symbol into the current lexical scope and backing symbol table.
    fn insert_scoped_symbol(
        &mut self,
        name: String,
        ty: Type,
        storage_class: StorageClass,
        kind: SymbolKind,
        span: Span,
    ) -> SymbolId {
        if let Some(scope) = self.scopes.last_mut()
            && scope.contains_key(&name)
        {
            self.symbols.push(Symbol {
                id: self.symbols.len(),
                name: "__shadow_error".to_string(),
                ty,
                storage_class,
                kind,
                span,
                fixed_address: None,
                is_defined: true,
                is_referenced: false,
                parameter_types: Vec::new(),
            });
        }

        let id = self.insert_symbol(Symbol {
            id: self.symbols.len(),
            name: name.clone(),
            ty,
            storage_class,
            kind,
            span,
            fixed_address: None,
            is_defined: true,
            is_referenced: false,
            parameter_types: Vec::new(),
        });
        self.scopes
            .last_mut()
            .expect("scope exists")
            .insert(name, id);
        id
    }

    /// Resolves a name starting from the innermost local scope outward.
    fn resolve_name(&self, name: &str) -> Option<SymbolId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    /// Pushes a new lexical scope for nested statements or function bodies.
    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    /// Pops the current lexical scope after its declarations go out of visibility.
    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Collects the symbol ids currently visible across all active lexical scopes.
    fn current_scope_symbols(&self) -> Vec<SymbolId> {
        let mut locals = BTreeSet::new();
        for scope in &self.scopes {
            locals.extend(scope.values().copied());
        }
        locals.into_iter().collect()
    }

    /// Returns true when the named function already has an analyzed body.
    fn has_body(&self, symbol: SymbolId) -> bool {
        self.functions
            .iter()
            .any(|function| function.symbol == symbol && function.body.is_some())
    }

    /// Emits post-analysis warnings for unused locals, params, and static functions.
    fn emit_warnings(&self, diagnostics: &mut DiagnosticBag) {
        for function in &self.functions {
            let symbol = &self.symbols[function.symbol];
            if symbol.kind == SymbolKind::Function
                && symbol.storage_class == StorageClass::Static
                && !symbol.is_referenced
                && symbol.name != "main"
            {
                diagnostics.warning(
                    "semantic",
                    Some(symbol.span),
                    format!("static function `{}` is never used", symbol.name),
                    "W2001",
                );
            }
        }

        for symbol in &self.symbols {
            if matches!(symbol.kind, SymbolKind::Local | SymbolKind::Param) && !symbol.is_referenced {
                diagnostics.warning(
                    "semantic",
                    Some(symbol.span),
                    format!("variable `{}` is never used", symbol.name),
                    "W2002",
                );
            }
        }
    }
}

/// Returns true when an expression is valid for static initialization in this subset.
fn is_constant_expression(expr: &TypedExpr) -> bool {
    match &expr.kind {
        TypedExprKind::IntLiteral(_) => true,
        TypedExprKind::Unary { expr, .. } => is_constant_expression(expr),
        TypedExprKind::Binary { lhs, rhs, .. } => {
            is_constant_expression(lhs) && is_constant_expression(rhs)
        }
        TypedExprKind::Cast { expr, .. } => is_constant_expression(expr),
        TypedExprKind::Assign { .. }
        | TypedExprKind::Call { .. }
        | TypedExprKind::Symbol(_) => false,
    }
}

/// Synthesizes a typed zero literal for semantic error recovery paths.
fn zero_expr(span: Span) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::IntLiteral(0),
        ty: Type::new(ScalarType::I16),
        span,
    }
}
