use std::collections::{BTreeMap, BTreeSet};

use crate::backend::pic16::devices::TargetDevice;
use crate::common::source::Span;
use crate::diagnostics::DiagnosticBag;

use super::ast::{
    BinaryOp, Expr, ExprKind, FunctionDecl, Item, Stmt, TranslationUnit, UnaryOp, VarDecl,
};
use super::types::{ScalarType, StorageClass, Type};

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

    fn seed_device_registers(&mut self) {
        for register in self.target.sfrs {
            let symbol = self.insert_symbol(
                register.name.to_string(),
                Type::new(ScalarType::U8).with_qualifiers(super::types::Qualifiers {
                    is_const: false,
                    is_volatile: true,
                }),
                StorageClass::Extern,
                SymbolKind::DeviceRegister,
                Span::new(0, 0),
                Some(register.address),
                true,
            );
            self.globals_by_name.insert(register.name.to_string(), symbol);
        }
    }

    fn declare_item(&mut self, item: &Item, diagnostics: &mut DiagnosticBag) {
        match item {
            Item::Function(function) => {
                self.declare_function_signature(function, diagnostics);
            }
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
                if !global.ty.is_supported_codegen_scalar() {
                    diagnostics.error(
                        "semantic",
                        Some(global.span),
                        format!("type `{}` not yet lowered in v0.1", global.ty),
                        Some("use `char`, `unsigned char`, or `void` in v0.1".to_string()),
                    );
                }
                let symbol = self.insert_symbol(
                    global.name.clone(),
                    global.ty,
                    global.storage_class,
                    SymbolKind::Global,
                    global.span,
                    None,
                    global.initializer.is_none(),
                );
                self.globals_by_name.insert(global.name.clone(), symbol);
            }
        }
    }

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
                return;
            }
            return;
        }

        if !function.return_type.is_supported_codegen_scalar() {
            diagnostics.error(
                "semantic",
                Some(function.span),
                format!("return type `{}` not yet lowered in v0.1", function.return_type),
                Some("use `void`, `char`, or `unsigned char`".to_string()),
            );
        }
        if function.params.len() > 2 {
            diagnostics.error(
                "semantic",
                Some(function.span),
                "v0.1 supports at most two 8-bit parameters per function",
                None,
            );
        }

        let symbol = self.insert_symbol(
            function.name.clone(),
            function.return_type,
            function.storage_class,
            SymbolKind::Function,
            function.span,
            None,
            function.body.is_none(),
        );
        self.globals_by_name.insert(function.name.clone(), symbol);
    }

    fn define_global(&mut self, global: VarDecl, diagnostics: &mut DiagnosticBag) {
        let Some(symbol) = self.globals_by_name.get(&global.name).copied() else {
            return;
        };
        if self.symbols[symbol].kind == SymbolKind::DeviceRegister {
            return;
        }
        self.symbols[symbol].is_defined = true;
        let initializer = global
            .initializer
            .as_ref()
            .and_then(|expr| self.analyze_expr(expr, diagnostics));
        if let Some(initializer) = initializer.as_ref()
            && !is_constant_expression(initializer)
        {
            diagnostics.error(
                "semantic",
                Some(initializer.span),
                "global initializer must be a constant expression in v0.1",
                None,
            );
        }
        self.globals.push(TypedGlobal {
            symbol,
            initializer,
        });
    }

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
            let param_id = self.insert_scoped_symbol(
                param.name.clone(),
                param.ty,
                param.storage_class,
                SymbolKind::Param,
                param.span,
            );
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
                let initializer = decl
                    .initializer
                    .as_ref()
                    .and_then(|expr| self.analyze_expr(expr, diagnostics));
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
                let typed = expr
                    .as_ref()
                    .and_then(|value| self.analyze_expr(value, diagnostics));
                if let Some(current_function) = self.current_function {
                    let return_type = self.symbols[current_function].ty;
                    if return_type.is_void() && typed.is_some() {
                        diagnostics.error(
                            "semantic",
                            Some(*span),
                            "void function cannot return a value",
                            None,
                        );
                    }
                    if !return_type.is_void() && typed.is_none() {
                        diagnostics.error(
                            "semantic",
                            Some(*span),
                            "non-void function must return a value",
                            None,
                        );
                    }
                }
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

    fn analyze_expr(&mut self, expr: &Expr, diagnostics: &mut DiagnosticBag) -> Option<TypedExpr> {
        let typed = match &expr.kind {
            ExprKind::IntLiteral(value) => TypedExpr {
                kind: TypedExprKind::IntLiteral(*value),
                ty: Type::new(ScalarType::U8),
                span: expr.span,
            },
            ExprKind::Name(name) => {
                let symbol = self.resolve_name(name).or_else(|| self.globals_by_name.get(name).copied());
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
                TypedExpr {
                    kind: TypedExprKind::Unary {
                        op: *op,
                        expr: Box::new(value.clone()),
                    },
                    ty: value.ty,
                    span: expr.span,
                }
            }
            ExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.analyze_expr(lhs, diagnostics)?;
                let rhs = self.analyze_expr(rhs, diagnostics)?;
                if lhs.ty.bit_width() != rhs.ty.bit_width() {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        "operands must have matching widths in v0.1",
                        None,
                    );
                }
                let result_type = match op {
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual
                    | BinaryOp::LogicalAnd
                    | BinaryOp::LogicalOr => Type::new(ScalarType::U8),
                    _ => lhs.ty,
                };
                TypedExpr {
                    kind: TypedExprKind::Binary {
                        op: *op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty: result_type,
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
                if self.symbols[target_symbol].ty.bit_width() != value.ty.bit_width() {
                    diagnostics.warning(
                        "semantic",
                        Some(expr.span),
                        "assignment truncates or widens value in v0.1",
                        "W1001",
                    );
                }
                TypedExpr {
                    kind: TypedExprKind::Assign {
                        target: target_symbol,
                        value: Box::new(value),
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
                let typed_args = args
                    .iter()
                    .filter_map(|argument| self.analyze_expr(argument, diagnostics))
                    .collect::<Vec<_>>();
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

    #[allow(clippy::too_many_arguments)]
    fn insert_symbol(
        &mut self,
        name: String,
        ty: Type,
        storage_class: StorageClass,
        kind: SymbolKind,
        span: Span,
        fixed_address: Option<u16>,
        is_defined: bool,
    ) -> SymbolId {
        let id = self.symbols.len();
        self.symbols.push(Symbol {
            id,
            name,
            ty,
            storage_class,
            kind,
            span,
            fixed_address,
            is_defined,
            is_referenced: false,
        });
        id
    }

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
            });
        }
        let id = self.insert_symbol(name.clone(), ty, storage_class, kind, span, None, true);
        self.scopes
            .last_mut()
            .expect("scope exists")
            .insert(name, id);
        id
    }

    fn resolve_name(&self, name: &str) -> Option<SymbolId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn current_scope_symbols(&self) -> Vec<SymbolId> {
        let mut locals = BTreeSet::new();
        for scope in &self.scopes {
            locals.extend(scope.values().copied());
        }
        locals.into_iter().collect()
    }

    fn has_body(&self, symbol: SymbolId) -> bool {
        self.functions
            .iter()
            .any(|function| function.symbol == symbol && function.body.is_some())
    }

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

fn is_constant_expression(expr: &TypedExpr) -> bool {
    match &expr.kind {
        TypedExprKind::IntLiteral(_) => true,
        TypedExprKind::Unary { expr, .. } => is_constant_expression(expr),
        TypedExprKind::Binary { lhs, rhs, .. } => {
            is_constant_expression(lhs) && is_constant_expression(rhs)
        }
        _ => false,
    }
}

fn zero_expr(span: Span) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::IntLiteral(0),
        ty: Type::new(ScalarType::U8),
        span,
    }
}
