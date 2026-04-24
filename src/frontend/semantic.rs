use std::collections::{BTreeMap, BTreeSet};

use crate::backend::pic16::devices::TargetDevice;
use crate::common::integer::infer_integer_literal_type;
use crate::common::source::Span;
use crate::diagnostics::DiagnosticBag;

use super::ast::{
    BinaryOp, Expr, ExprKind, FunctionDecl, Item, Stmt, TranslationUnit, UnaryOp, VarDecl,
};
use super::types::{CastKind, Qualifiers, ScalarType, StorageClass, Type};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueCategory {
    LValue,
    RValue,
}

#[derive(Clone, Debug)]
pub struct TypedExpr {
    pub kind: TypedExprKind,
    pub ty: Type,
    pub span: Span,
    pub value_category: ValueCategory,
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
    ArrayDecay(Box<TypedExpr>),
    AddressOf(Box<TypedExpr>),
    Deref(Box<TypedExpr>),
    Assign {
        target: Box<TypedExpr>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Active,
    Done,
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

        self.reject_recursive_calls(diagnostics);

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
                ty: Type::new(ScalarType::U8).with_qualifiers(Qualifiers {
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

    /// Registers one top-level declaration before bodies and initializers are analyzed.
    fn declare_item(&mut self, item: &Item, diagnostics: &mut DiagnosticBag) {
        match item {
            Item::Function(function) => self.declare_function_signature(function, diagnostics),
            Item::Global(global) => self.declare_global(global, diagnostics),
        }
    }

    /// Registers one global variable declaration and validates its Phase 3 type shape.
    fn declare_global(&mut self, global: &VarDecl, diagnostics: &mut DiagnosticBag) {
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

        self.validate_object_type(global.ty, global.span, &global.name, "global", diagnostics);
        if global.ty.is_array() && global.initializer.is_some() {
            diagnostics.error(
                "semantic",
                Some(global.span),
                "array initializers are not implemented in phase 3",
                Some("declare the array without an initializer and fill it in code".to_string()),
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

    /// Adds a function signature to the global symbol table and validates the fixed ABI.
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

        self.validate_return_type(function.return_type, function.span, &function.name, diagnostics);

        let parameter_types = function
            .params
            .iter()
            .map(|param| self.normalize_param_type(param.ty))
            .collect::<Vec<_>>();
        for (param, ty) in function.params.iter().zip(parameter_types.iter().copied()) {
            self.validate_param_type(ty, param.span, &param.name, diagnostics);
        }

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
        let initializer = if self.symbols[symbol].ty.is_array() {
            None
        } else {
            global.initializer.as_ref().and_then(|expr| {
                let expr = self.analyze_expr(expr, diagnostics)?;
                let coerced = self.coerce_expr(
                    expr,
                    global.ty,
                    diagnostics,
                    "global initializer",
                    false,
                );
                Some(coerced)
            })
        };
        if let Some(initializer) = initializer.as_ref()
            && !is_constant_expression(initializer)
        {
            diagnostics.error(
                "semantic",
                Some(initializer.span),
                "global initializer must be a constant expression in phase 3",
                Some("use literals, casts, and integer operators only".to_string()),
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
        let function_symbol_start = self.symbols.len();

        let mut params = Vec::new();
        for param in function.params {
            let param_ty = self.normalize_param_type(param.ty);
            let param_id = self.insert_symbol(Symbol {
                id: self.symbols.len(),
                name: param.name.clone(),
                ty: param_ty,
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
        if let Some(body) = body.as_ref() {
            self.reject_stack_local_pointer_returns(body, diagnostics);
        }

        let locals = self.function_symbols_since(function_symbol_start);
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
                self.validate_object_type(decl.ty, decl.span, &decl.name, "local", diagnostics);
                if decl.ty.is_array() && decl.initializer.is_some() {
                    diagnostics.error(
                        "semantic",
                        Some(decl.span),
                        "array initializers are not implemented in phase 3",
                        Some("declare the array without an initializer and fill it in code".to_string()),
                    );
                }
                let symbol = self.insert_scoped_symbol(
                    decl.name.clone(),
                    decl.ty,
                    decl.storage_class,
                    SymbolKind::Local,
                    decl.span,
                );
                let initializer = if decl.ty.is_array() {
                    None
                } else {
                    decl.initializer.as_ref().and_then(|expr| {
                        let expr = self.analyze_expr(expr, diagnostics)?;
                        Some(self.coerce_expr(expr, decl.ty, diagnostics, "local initializer", true))
                    })
                };
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
                            let expr =
                                self.coerce_expr(expr, return_type, diagnostics, "return value", true);
                            if self.returns_stack_local_address(&expr) {
                                diagnostics.error(
                                    "semantic",
                                    Some(expr.span),
                                    "returning the address of a stack local is not supported",
                                    Some(
                                        "return a global/static object address or write through an output parameter"
                                            .to_string(),
                                    ),
                                );
                            }
                            expr
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
        self.analyze_expr_with_decay(expr, diagnostics, true)
    }

    /// Analyzes one expression while controlling whether array lvalues decay automatically.
    fn analyze_expr_with_decay(
        &mut self,
        expr: &Expr,
        diagnostics: &mut DiagnosticBag,
        decay_arrays: bool,
    ) -> Option<TypedExpr> {
        let typed = match &expr.kind {
            ExprKind::IntLiteral(value) => TypedExpr {
                kind: TypedExprKind::IntLiteral(*value),
                ty: infer_integer_literal_type(*value),
                span: expr.span,
                value_category: ValueCategory::RValue,
            },
            ExprKind::Name(name) => self.analyze_name(name, expr.span, diagnostics)?,
            ExprKind::Unary { op, expr: value } => {
                self.analyze_unary_expr(*op, value, expr.span, diagnostics)?
            }
            ExprKind::AddressOf(value) => self.analyze_address_of(value, expr.span, diagnostics)?,
            ExprKind::Deref(value) => self.analyze_deref(value, expr.span, diagnostics)?,
            ExprKind::Binary { op, lhs, rhs } => {
                self.analyze_binary_expr(*op, lhs, rhs, expr.span, diagnostics)?
            }
            ExprKind::Index { base, index } => {
                self.analyze_index_expr(base, index, expr.span, diagnostics)?
            }
            ExprKind::Assign { target, value } => {
                self.analyze_assign_expr(target, value, expr.span, diagnostics)?
            }
            ExprKind::Call { callee, args } => {
                self.analyze_call_expr(callee, args, expr.span, diagnostics)?
            }
            ExprKind::SizeOfExpr(value) => self.analyze_sizeof_expr(value, expr.span, diagnostics)?,
            ExprKind::SizeOfType(ty) => self.analyze_sizeof_type(*ty, expr.span, diagnostics)?,
        };

        if decay_arrays && typed.ty.is_array() {
            Some(self.decay_array_expr(typed, expr.span))
        } else {
            Some(typed)
        }
    }

    /// Resolves one source-level name into a typed symbol reference.
    fn analyze_name(
        &mut self,
        name: &str,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let symbol = self
            .resolve_name(name)
            .or_else(|| self.globals_by_name.get(name).copied());
        let Some(symbol) = symbol else {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("undefined symbol `{name}`"),
                None,
            );
            return None;
        };
        if self.symbols[symbol].kind == SymbolKind::Function {
            diagnostics.error(
                "semantic",
                Some(span),
                "function values and function pointers are not supported in phase 3",
                Some("call the function directly instead".to_string()),
            );
            return None;
        }
        self.symbols[symbol].is_referenced = true;
        Some(TypedExpr {
            kind: TypedExprKind::Symbol(symbol),
            ty: self.symbols[symbol].ty,
            span,
            value_category: ValueCategory::LValue,
        })
    }

    /// Analyzes one unary expression in the supported Phase 3 scalar model.
    fn analyze_unary_expr(
        &mut self,
        op: UnaryOp,
        expr: &Expr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let value = self.analyze_expr(expr, diagnostics)?;
        match op {
            UnaryOp::LogicalNot => {
                if !value.ty.is_scalar_value() {
                    diagnostics.error(
                        "semantic",
                        Some(span),
                        "logical not requires an integer or pointer operand",
                        None,
                    );
                    return None;
                }
                Some(TypedExpr {
                    kind: TypedExprKind::Unary {
                        op,
                        expr: Box::new(value),
                    },
                    ty: Type::new(ScalarType::U8),
                    span,
                    value_category: ValueCategory::RValue,
                })
            }
            UnaryOp::Negate | UnaryOp::BitwiseNot => {
                if !value.ty.is_integer() {
                    diagnostics.error(
                        "semantic",
                        Some(span),
                        "unary operator requires an integer operand",
                        None,
                    );
                    return None;
                }
                Some(TypedExpr {
                    kind: TypedExprKind::Unary {
                        op,
                        expr: Box::new(value.clone()),
                    },
                    ty: value.ty,
                    span,
                    value_category: ValueCategory::RValue,
                })
            }
        }
    }

    /// Analyzes one address-of expression over a supported assignable object.
    fn analyze_address_of(
        &mut self,
        expr: &Expr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let value = self.analyze_expr_with_decay(expr, diagnostics, false)?;
        if value.value_category != ValueCategory::LValue {
            diagnostics.error(
                "semantic",
                Some(span),
                "address-of requires an lvalue operand",
                None,
            );
            return None;
        }
        if value.ty.is_array() {
            diagnostics.error(
                "semantic",
                Some(span),
                "taking the address of a whole array is not supported in phase 3",
                Some("use the array name for decay or take `&array[index]`".to_string()),
            );
            return None;
        }
        if !value.ty.is_supported_pointer_target() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("cannot take the address of unsupported type `{}`", value.ty),
                None,
            );
            return None;
        }
        let target_ty = value.ty;
        Some(TypedExpr {
            kind: TypedExprKind::AddressOf(Box::new(value)),
            ty: target_ty.pointer_to(),
            span,
            value_category: ValueCategory::RValue,
        })
    }

    /// Analyzes one dereference expression over a constrained Phase 3 data pointer.
    fn analyze_deref(
        &mut self,
        expr: &Expr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let pointer = self.analyze_expr(expr, diagnostics)?;
        if !pointer.ty.is_pointer() {
            diagnostics.error(
                "semantic",
                Some(span),
                "dereference requires a supported data pointer",
                None,
            );
            return None;
        }
        let element_ty = pointer.ty.element_type();
        if !element_ty.is_supported_pointer_target() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("pointer target `{}` is not supported in phase 3", element_ty),
                None,
            );
            return None;
        }
        Some(TypedExpr {
            kind: TypedExprKind::Deref(Box::new(pointer)),
            ty: element_ty,
            span,
            value_category: ValueCategory::LValue,
        })
    }

    /// Analyzes one binary expression, including pointer arithmetic and comparisons.
    fn analyze_binary_expr(
        &mut self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        match op {
            BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                let lhs = self.analyze_expr(lhs, diagnostics)?;
                let rhs = self.analyze_expr(rhs, diagnostics)?;
                if !lhs.ty.is_scalar_value() || !rhs.ty.is_scalar_value() {
                    diagnostics.error(
                        "semantic",
                        Some(span),
                        "logical operators require integer or pointer operands",
                        None,
                    );
                    return None;
                }
                Some(TypedExpr {
                    kind: TypedExprKind::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty: Type::new(ScalarType::U8),
                    span,
                    value_category: ValueCategory::RValue,
                })
            }
            BinaryOp::Equal | BinaryOp::NotEqual => {
                let lhs = self.analyze_expr(lhs, diagnostics)?;
                let rhs = self.analyze_expr(rhs, diagnostics)?;
                self.analyze_equality_expr(op, lhs, rhs, span, diagnostics)
            }
            BinaryOp::Less | BinaryOp::LessEqual | BinaryOp::Greater | BinaryOp::GreaterEqual => {
                let lhs = self.analyze_expr(lhs, diagnostics)?;
                let rhs = self.analyze_expr(rhs, diagnostics)?;
                if lhs.ty.is_pointer() || rhs.ty.is_pointer() {
                    diagnostics.error(
                        "semantic",
                        Some(span),
                        "relational comparisons on pointers are not supported in phase 3",
                        Some("use `==` or `!=` for supported pointer comparisons".to_string()),
                    );
                    return None;
                }
                let (lhs, rhs, _) =
                    self.balance_integer_operands(op, lhs, rhs, diagnostics, span);
                Some(TypedExpr {
                    kind: TypedExprKind::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty: Type::new(ScalarType::U8),
                    span,
                    value_category: ValueCategory::RValue,
                })
            }
            BinaryOp::Add | BinaryOp::Sub => {
                let lhs = self.analyze_expr(lhs, diagnostics)?;
                let rhs = self.analyze_expr(rhs, diagnostics)?;
                self.analyze_add_sub_expr(op, lhs, rhs, span, diagnostics)
            }
            BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Modulo => {
                let lhs = self.analyze_expr(lhs, diagnostics)?;
                let rhs = self.analyze_expr(rhs, diagnostics)?;
                let (lhs, rhs, result_ty) =
                    self.balance_integer_operands(op, lhs, rhs, diagnostics, span);
                Some(TypedExpr {
                    kind: TypedExprKind::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                    ty: result_ty,
                    span,
                    value_category: ValueCategory::RValue,
                })
            }
        }
    }

    /// Analyzes equality and inequality across integers, matching pointers, and null.
    fn analyze_equality_expr(
        &mut self,
        op: BinaryOp,
        lhs: TypedExpr,
        rhs: TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        if lhs.ty.is_pointer() || rhs.ty.is_pointer() {
            let (lhs, rhs) = self.balance_pointer_operands(lhs, rhs, diagnostics, span)?;
            return Some(TypedExpr {
                kind: TypedExprKind::Binary {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                ty: Type::new(ScalarType::U8),
                span,
                value_category: ValueCategory::RValue,
            });
        }

        let (lhs, rhs, _) = self.balance_integer_operands(op, lhs, rhs, diagnostics, span);
        Some(TypedExpr {
            kind: TypedExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: Type::new(ScalarType::U8),
            span,
            value_category: ValueCategory::RValue,
        })
    }

    /// Analyzes integer arithmetic plus constrained pointer-plus-or-minus-integer forms.
    fn analyze_add_sub_expr(
        &mut self,
        op: BinaryOp,
        lhs: TypedExpr,
        rhs: TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        if lhs.ty.is_pointer() || rhs.ty.is_pointer() {
            match op {
                BinaryOp::Add => {
                    if lhs.ty.is_pointer() && rhs.ty.is_integer() {
                        return Some(self.build_pointer_offset_expr(op, lhs, rhs, span, diagnostics));
                    }
                    if lhs.ty.is_integer() && rhs.ty.is_pointer() {
                        return Some(self.build_pointer_offset_expr(op, rhs, lhs, span, diagnostics));
                    }
                }
                BinaryOp::Sub => {
                    if lhs.ty.is_pointer() && rhs.ty.is_integer() {
                        return Some(self.build_pointer_offset_expr(op, lhs, rhs, span, diagnostics));
                    }
                }
                _ => {}
            }
            diagnostics.error(
                "semantic",
                Some(span),
                "unsupported pointer arithmetic form in phase 3",
                Some("use pointer +/- integer only".to_string()),
            );
            return None;
        }

        let (lhs, rhs, result_ty) = self.balance_integer_operands(op, lhs, rhs, diagnostics, span);
        Some(TypedExpr {
            kind: TypedExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: result_ty,
            span,
            value_category: ValueCategory::RValue,
        })
    }

    /// Lowers one indexing expression into pointer arithmetic followed by dereference.
    fn analyze_index_expr(
        &mut self,
        base: &Expr,
        index: &Expr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let base = self.analyze_expr(base, diagnostics)?;
        let index = self.analyze_expr(index, diagnostics)?;
        if !base.ty.is_pointer() {
            diagnostics.error(
                "semantic",
                Some(span),
                "indexing requires an array or supported data pointer",
                None,
            );
            return None;
        }
        if !index.ty.is_integer() {
            diagnostics.error(
                "semantic",
                Some(span),
                "array and pointer indices must be integers",
                None,
            );
            return None;
        }

        let element_ty = base.ty.element_type();
        if !element_ty.is_supported_pointer_target() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("indexed element type `{}` is not supported in phase 3", element_ty),
                None,
            );
            return None;
        }

        let scaled = self.scale_index_expr(index, element_ty, diagnostics);
        let address = TypedExpr {
            kind: TypedExprKind::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(base.clone()),
                rhs: Box::new(scaled),
            },
            ty: base.ty,
            span,
            value_category: ValueCategory::RValue,
        };
        Some(TypedExpr {
            kind: TypedExprKind::Deref(Box::new(address)),
            ty: element_ty,
            span,
            value_category: ValueCategory::LValue,
        })
    }

    /// Analyzes one assignment and preserves the target place for later lowering.
    fn analyze_assign_expr(
        &mut self,
        target: &Expr,
        value: &Expr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let target = self.analyze_expr_with_decay(target, diagnostics, false)?;
        if !self.is_assignable_lvalue(&target) {
            diagnostics.error(
                "semantic",
                Some(target.span),
                "left side of assignment must be an assignable lvalue",
                None,
            );
            return None;
        }
        let value = self.analyze_expr(value, diagnostics)?;
        let target_ty = target.ty;
        let value = self.coerce_expr(value, target_ty, diagnostics, "assignment", true);
        Some(TypedExpr {
            kind: TypedExprKind::Assign {
                target: Box::new(target),
                value: Box::new(value),
            },
            ty: target_ty,
            span,
            value_category: ValueCategory::RValue,
        })
    }

    /// Analyzes one direct function call within the Phase 4 call ABI.
    fn analyze_call_expr(
        &mut self,
        callee: &str,
        args: &[Expr],
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let Some(function) = self.globals_by_name.get(callee).copied() else {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("undefined function `{callee}`"),
                None,
            );
            return None;
        };
        if self.symbols[function].kind != SymbolKind::Function {
            diagnostics.error(
                "semantic",
                Some(span),
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
                Some(span),
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
        Some(TypedExpr {
            kind: TypedExprKind::Call {
                function,
                args: typed_args,
            },
            ty: self.symbols[function].ty,
            span,
            value_category: ValueCategory::RValue,
        })
    }

    /// Analyzes `sizeof(expr)` without triggering array decay on the operand.
    fn analyze_sizeof_expr(
        &mut self,
        expr: &Expr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let value = self.analyze_expr_with_decay(expr, diagnostics, false)?;
        self.build_sizeof_value(value.ty, span, diagnostics)
    }

    /// Analyzes `sizeof(type)` over the constrained Phase 3 type model.
    fn analyze_sizeof_type(
        &mut self,
        ty: Type,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        self.build_sizeof_value(ty, span, diagnostics)
    }

    /// Materializes a compile-time `sizeof` result as an unsigned 16-bit literal.
    fn build_sizeof_value(
        &mut self,
        ty: Type,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        if !ty.has_size() || !ty.is_supported_object_type() && !ty.is_supported_value_type() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("`sizeof` does not support incomplete or unsupported type `{}`", ty),
                None,
            );
            return None;
        }
        Some(TypedExpr {
            kind: TypedExprKind::IntLiteral(ty.byte_width() as i64),
            ty: Type::new(ScalarType::U16),
            span,
            value_category: ValueCategory::RValue,
        })
    }

    /// Applies C-style array decay from an lvalue array object to a data pointer value.
    fn decay_array_expr(&mut self, expr: TypedExpr, span: Span) -> TypedExpr {
        TypedExpr {
            ty: expr.ty.decay(),
            span,
            kind: TypedExprKind::ArrayDecay(Box::new(expr)),
            value_category: ValueCategory::RValue,
        }
    }

    /// Harmonizes integer operand widths and signedness before arithmetic or compare lowering.
    fn balance_integer_operands(
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
                format!("`{op:?}` requires integer operands"),
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
                "mixed signedness for `{op:?}` with equal-width operands is not supported in phase 3"
            ),
            Some("use matching signedness or add an explicit cast".to_string()),
        );
        let result_ty = lhs.ty;
        let rhs = self.coerce_expr(rhs, lhs.ty, diagnostics, "binary operand", false);
        (lhs, rhs, result_ty)
    }

    /// Harmonizes supported pointer comparisons, including explicit null pointer constants.
    fn balance_pointer_operands(
        &mut self,
        lhs: TypedExpr,
        rhs: TypedExpr,
        diagnostics: &mut DiagnosticBag,
        span: Span,
    ) -> Option<(TypedExpr, TypedExpr)> {
        if lhs.ty.is_pointer() && rhs.ty.is_pointer() {
            if lhs.ty.same_pointer_target(rhs.ty) {
                return Some((lhs, rhs));
            }
            diagnostics.error(
                "semantic",
                Some(span),
                format!(
                    "pointer comparison requires matching pointee types, got `{}` and `{}`",
                    lhs.ty, rhs.ty
                ),
                None,
            );
            return None;
        }

        if lhs.ty.is_pointer() && self.is_null_pointer_constant(&rhs) {
            let rhs = self.coerce_expr(rhs, lhs.ty, diagnostics, "pointer comparison", false);
            return Some((lhs, rhs));
        }
        if rhs.ty.is_pointer() && self.is_null_pointer_constant(&lhs) {
            let lhs = self.coerce_expr(lhs, rhs.ty, diagnostics, "pointer comparison", false);
            return Some((lhs, rhs));
        }

        diagnostics.error(
            "semantic",
            Some(span),
            "unsupported pointer comparison operands",
            Some("compare matching pointer types or compare against literal zero".to_string()),
        );
        None
    }

    /// Builds one pointer-plus-or-minus-integer expression with element-size-aware scaling.
    fn build_pointer_offset_expr(
        &mut self,
        op: BinaryOp,
        pointer: TypedExpr,
        offset: TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> TypedExpr {
        let scaled = self.scale_index_expr(offset, pointer.ty.element_type(), diagnostics);
        TypedExpr {
            kind: TypedExprKind::Binary {
                op,
                lhs: Box::new(pointer.clone()),
                rhs: Box::new(scaled),
            },
            ty: pointer.ty,
            span,
            value_category: ValueCategory::RValue,
        }
    }

    /// Scales one integer index by the pointee size used by array and pointer indexing.
    fn scale_index_expr(
        &mut self,
        expr: TypedExpr,
        element_ty: Type,
        diagnostics: &mut DiagnosticBag,
    ) -> TypedExpr {
        let span = expr.span;
        let expr = self.coerce_expr(
            expr,
            Type::new(ScalarType::U16),
            diagnostics,
            "index expression",
            true,
        );
        if element_ty.byte_width() == 1 {
            return expr;
        }
        if let TypedExprKind::IntLiteral(value) = expr.kind {
            return TypedExpr {
                kind: TypedExprKind::IntLiteral(value * element_ty.byte_width() as i64),
                ty: Type::new(ScalarType::U16),
                span,
                value_category: ValueCategory::RValue,
            };
        }
        TypedExpr {
            kind: TypedExprKind::Binary {
                op: BinaryOp::Add,
                lhs: Box::new(expr.clone()),
                rhs: Box::new(expr),
            },
            ty: Type::new(ScalarType::U16),
            span,
            value_category: ValueCategory::RValue,
        }
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
        if target_ty.is_pointer() {
            if expr.ty.is_pointer() && expr.ty.same_pointer_target(target_ty) {
                let span = expr.span;
                return TypedExpr {
                    kind: TypedExprKind::Cast {
                        kind: CastKind::Bitcast,
                        expr: Box::new(expr),
                    },
                    ty: target_ty,
                    span,
                    value_category: ValueCategory::RValue,
                };
            }
            if self.is_null_pointer_constant(&expr) {
                return TypedExpr {
                    kind: TypedExprKind::IntLiteral(0),
                    ty: target_ty,
                    span: expr.span,
                    value_category: ValueCategory::RValue,
                };
            }
            diagnostics.error(
                "semantic",
                Some(expr.span),
                format!("cannot coerce `{}` to `{}` in {context}", expr.ty, target_ty),
                Some("only matching pointer types and literal zero are supported".to_string()),
            );
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
                value_category: ValueCategory::RValue,
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
            value_category: ValueCategory::RValue,
        }
    }

    /// Returns true when one typed expression is the literal zero null-pointer constant.
    fn is_null_pointer_constant(&self, expr: &TypedExpr) -> bool {
        matches!(expr.kind, TypedExprKind::IntLiteral(0))
    }

    /// Returns true when one expression can appear on the left side of assignment.
    fn is_assignable_lvalue(&self, expr: &TypedExpr) -> bool {
        expr.value_category == ValueCategory::LValue
            && !expr.ty.is_array()
            && !expr.ty.qualifiers.is_const
            && matches!(expr.kind, TypedExprKind::Symbol(_) | TypedExprKind::Deref(_))
    }

    /// Validates one return type against the constrained Phase 3 ABI.
    fn validate_return_type(
        &mut self,
        ty: Type,
        span: Span,
        name: &str,
        diagnostics: &mut DiagnosticBag,
    ) {
        if ty.is_void() {
            return;
        }
        if !ty.is_supported_value_type() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("function `{name}` uses unsupported return type `{ty}`"),
                None,
            );
        }
    }

    /// Validates one parameter type against the fixed Phase 3 call ABI.
    fn validate_param_type(
        &mut self,
        ty: Type,
        span: Span,
        name: &str,
        diagnostics: &mut DiagnosticBag,
    ) {
        if ty.is_void() || !ty.is_supported_value_type() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("parameter `{name}` uses unsupported type `{ty}`"),
                None,
            );
        }
    }

    /// Validates one declared object type against the supported array and pointer model.
    fn validate_object_type(
        &mut self,
        ty: Type,
        span: Span,
        name: &str,
        context: &str,
        diagnostics: &mut DiagnosticBag,
    ) {
        if ty.is_void() || !ty.is_supported_object_type() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("{context} `{name}` uses unsupported type `{ty}`"),
                Some(
                    "phase 3 supports char, unsigned char, int, unsigned int, pointers to them, and one-dimensional arrays of them"
                        .to_string(),
                ),
            );
        }
    }

    /// Applies the C parameter-array decay rule to one declared parameter type.
    fn normalize_param_type(&self, ty: Type) -> Type {
        if ty.is_array() {
            ty.decay()
        } else {
            ty
        }
    }

    /// Rejects direct or mutual recursion because Phase 4 stack sizing is static and non-cyclic.
    fn reject_recursive_calls(&self, diagnostics: &mut DiagnosticBag) {
        let mut state = BTreeMap::<SymbolId, VisitState>::new();
        for function in &self.functions {
            self.visit_call_graph(function.symbol, &mut state, &mut Vec::new(), diagnostics);
        }
    }

    /// Walks one function in DFS order and emits a diagnostic when a call cycle is found.
    fn visit_call_graph(
        &self,
        symbol: SymbolId,
        state: &mut BTreeMap<SymbolId, VisitState>,
        stack: &mut Vec<SymbolId>,
        diagnostics: &mut DiagnosticBag,
    ) {
        match state.get(&symbol).copied() {
            Some(VisitState::Done) => return,
            Some(VisitState::Active) => {
                let name = self.symbols[symbol].name.clone();
                diagnostics.error(
                    "semantic",
                    Some(self.symbols[symbol].span),
                    format!("recursive call cycle involving `{name}` is not supported in phase 4"),
                    Some("phase 4 computes software-stack usage statically; keep the call graph acyclic".to_string()),
                );
                return;
            }
            None => {}
        }

        state.insert(symbol, VisitState::Active);
        stack.push(symbol);
        if let Some(function) = self.functions.iter().find(|function| function.symbol == symbol)
            && let Some(body) = &function.body
        {
            let mut callees = BTreeSet::new();
            collect_stmt_calls(body, &mut callees);
            for callee in callees {
                if matches!(state.get(&callee).copied(), Some(VisitState::Active)) {
                    let cycle = stack
                        .iter()
                        .map(|id| self.symbols[*id].name.as_str())
                        .chain(std::iter::once(self.symbols[callee].name.as_str()))
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    diagnostics.error(
                        "semantic",
                        Some(self.symbols[callee].span),
                        format!("recursive call cycle `{cycle}` is not supported in phase 4"),
                        Some("phase 4 computes software-stack usage statically; keep the call graph acyclic".to_string()),
                    );
                    continue;
                }
                self.visit_call_graph(callee, state, stack, diagnostics);
            }
        }
        stack.pop();
        state.insert(symbol, VisitState::Done);
    }

    /// Returns true when the expression directly produces a pointer into a non-static local object.
    fn returns_stack_local_address(&self, expr: &TypedExpr) -> bool {
        if !expr.ty.is_pointer() {
            return false;
        }
        match &expr.kind {
            TypedExprKind::ArrayDecay(value) | TypedExprKind::AddressOf(value) => {
                self.is_stack_lvalue(value)
            }
            TypedExprKind::Binary { lhs, rhs, .. } => {
                self.returns_stack_local_address(lhs) || self.returns_stack_local_address(rhs)
            }
            TypedExprKind::Cast { expr, .. } => self.returns_stack_local_address(expr),
            _ => false,
        }
    }

    /// Rejects obvious local-pointer alias chains that can return stack storage indirectly.
    fn reject_stack_local_pointer_returns(&self, body: &TypedStmt, diagnostics: &mut DiagnosticBag) {
        let mut tainted_locals = BTreeSet::new();
        self.walk_stmt_for_stack_pointer_returns(body, &mut tainted_locals, diagnostics);
    }

    /// Walks statements in source order and propagates conservative stack-pointer taint.
    fn walk_stmt_for_stack_pointer_returns(
        &self,
        stmt: &TypedStmt,
        tainted_locals: &mut BTreeSet<SymbolId>,
        diagnostics: &mut DiagnosticBag,
    ) {
        match stmt {
            TypedStmt::Block(statements, _) => {
                for statement in statements {
                    self.walk_stmt_for_stack_pointer_returns(statement, tainted_locals, diagnostics);
                }
            }
            TypedStmt::VarDecl(symbol, initializer, _) => {
                if let Some(initializer) = initializer {
                    let may_point_to_stack =
                        self.track_stack_pointer_expr(initializer, tainted_locals);
                    if may_point_to_stack && self.is_local_pointer_symbol(*symbol) {
                        tainted_locals.insert(*symbol);
                    }
                }
            }
            TypedStmt::Expr(expr, _) => {
                let _ = self.track_stack_pointer_expr(expr, tainted_locals);
            }
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let mut branch_seed = tainted_locals.clone();
                let _ = self.track_stack_pointer_expr(condition, &mut branch_seed);

                let mut then_taint = branch_seed.clone();
                self.walk_stmt_for_stack_pointer_returns(then_branch, &mut then_taint, diagnostics);

                let mut merged = then_taint;
                if let Some(else_branch) = else_branch {
                    let mut else_taint = branch_seed;
                    self.walk_stmt_for_stack_pointer_returns(else_branch, &mut else_taint, diagnostics);
                    merged.extend(else_taint);
                } else {
                    merged.extend(branch_seed);
                }
                *tainted_locals = merged;
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                let _ = self.track_stack_pointer_expr(condition, tainted_locals);
                let mut loop_taint = tainted_locals.clone();
                self.walk_stmt_for_stack_pointer_returns(body, &mut loop_taint, diagnostics);
                let _ = self.track_stack_pointer_expr(condition, &mut loop_taint);
                tainted_locals.extend(loop_taint);
            }
            TypedStmt::DoWhile {
                body,
                condition,
                ..
            } => {
                let mut loop_taint = tainted_locals.clone();
                self.walk_stmt_for_stack_pointer_returns(body, &mut loop_taint, diagnostics);
                let _ = self.track_stack_pointer_expr(condition, &mut loop_taint);
                tainted_locals.extend(loop_taint);
            }
            TypedStmt::For {
                init,
                condition,
                step,
                body,
                ..
            } => {
                if let Some(init) = init {
                    self.walk_stmt_for_stack_pointer_returns(init, tainted_locals, diagnostics);
                }
                let mut loop_taint = tainted_locals.clone();
                if let Some(condition) = condition {
                    let _ = self.track_stack_pointer_expr(condition, &mut loop_taint);
                }
                self.walk_stmt_for_stack_pointer_returns(body, &mut loop_taint, diagnostics);
                if let Some(step) = step {
                    let _ = self.track_stack_pointer_expr(step, &mut loop_taint);
                }
                tainted_locals.extend(loop_taint);
            }
            TypedStmt::Return(expr, _) => {
                if let Some(expr) = expr {
                    let may_point_to_stack = self.track_stack_pointer_expr(expr, tainted_locals);
                    if expr.ty.is_pointer()
                        && may_point_to_stack
                        && !self.returns_stack_local_address(expr)
                    {
                        diagnostics.error(
                            "semantic",
                            Some(expr.span),
                            "returning a pointer that may refer to a stack local is not supported",
                            Some(
                                "return a global/static object address or write through an output parameter"
                                    .to_string(),
                            ),
                        );
                    }
                }
            }
            TypedStmt::Break(_) | TypedStmt::Continue(_) | TypedStmt::Empty(_) => {}
        }
    }

    /// Tracks whether one expression may evaluate to a pointer into stack local storage.
    fn track_stack_pointer_expr(
        &self,
        expr: &TypedExpr,
        tainted_locals: &mut BTreeSet<SymbolId>,
    ) -> bool {
        match &expr.kind {
            TypedExprKind::IntLiteral(_) => false,
            TypedExprKind::Symbol(symbol) => {
                expr.ty.is_pointer() && self.is_local_pointer_symbol(*symbol) && tainted_locals.contains(symbol)
            }
            TypedExprKind::Unary { expr, .. } => self.track_stack_pointer_expr(expr, tainted_locals),
            TypedExprKind::Binary { lhs, rhs, .. } => {
                let lhs_tainted = self.track_stack_pointer_expr(lhs, tainted_locals);
                let rhs_tainted = self.track_stack_pointer_expr(rhs, tainted_locals);
                expr.ty.is_pointer() && (lhs_tainted || rhs_tainted)
            }
            TypedExprKind::ArrayDecay(value) | TypedExprKind::AddressOf(value) => self.is_stack_lvalue(value),
            TypedExprKind::Deref(value) => {
                let _ = self.track_stack_pointer_expr(value, tainted_locals);
                false
            }
            TypedExprKind::Assign { target, value } => {
                let value_tainted = self.track_stack_pointer_expr(value, tainted_locals);
                if let TypedExprKind::Symbol(symbol) = target.kind
                    && self.is_local_pointer_symbol(symbol)
                    && value_tainted
                {
                    tainted_locals.insert(symbol);
                } else {
                    let _ = self.track_stack_pointer_expr(target, tainted_locals);
                }
                expr.ty.is_pointer() && value_tainted
            }
            TypedExprKind::Call { args, .. } => {
                let arg_tainted = args
                    .iter()
                    .fold(false, |acc, arg| self.track_stack_pointer_expr(arg, tainted_locals) || acc);
                expr.ty.is_pointer() && arg_tainted
            }
            TypedExprKind::Cast { expr, .. } => self.track_stack_pointer_expr(expr, tainted_locals),
        }
    }

    /// Returns true when the lvalue names a non-static local object allocated in the call frame.
    fn is_stack_lvalue(&self, expr: &TypedExpr) -> bool {
        match &expr.kind {
            TypedExprKind::Symbol(symbol) => {
                let symbol = &self.symbols[*symbol];
                symbol.kind == SymbolKind::Local && symbol.storage_class != StorageClass::Static
            }
            TypedExprKind::Deref(_) => false,
            _ => false,
        }
    }

    /// Returns true when one symbol is a non-static local pointer variable.
    fn is_local_pointer_symbol(&self, symbol: SymbolId) -> bool {
        let symbol = &self.symbols[symbol];
        symbol.kind == SymbolKind::Local
            && symbol.storage_class != StorageClass::Static
            && symbol.ty.is_pointer()
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

    /// Collects all symbols created while analyzing the current function body.
    fn function_symbols_since(&self, start: usize) -> Vec<SymbolId> {
        self.symbols[start..]
            .iter()
            .filter(|symbol| matches!(symbol.kind, SymbolKind::Local | SymbolKind::Param))
            .map(|symbol| symbol.id)
            .collect()
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
        | TypedExprKind::ArrayDecay(_)
        | TypedExprKind::AddressOf(_)
        | TypedExprKind::Deref(_)
        | TypedExprKind::Symbol(_) => false,
    }
}

/// Collects direct function-call targets that appear anywhere inside one typed statement tree.
fn collect_stmt_calls(stmt: &TypedStmt, callees: &mut BTreeSet<SymbolId>) {
    match stmt {
        TypedStmt::Block(statements, _) => {
            for statement in statements {
                collect_stmt_calls(statement, callees);
            }
        }
        TypedStmt::VarDecl(_, initializer, _) => {
            if let Some(initializer) = initializer {
                collect_expr_calls(initializer, callees);
            }
        }
        TypedStmt::Expr(expr, _) => collect_expr_calls(expr, callees),
        TypedStmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr_calls(condition, callees);
            collect_stmt_calls(then_branch, callees);
            if let Some(else_branch) = else_branch {
                collect_stmt_calls(else_branch, callees);
            }
        }
        TypedStmt::While {
            condition, body, ..
        } => {
            collect_expr_calls(condition, callees);
            collect_stmt_calls(body, callees);
        }
        TypedStmt::DoWhile {
            body, condition, ..
        } => {
            collect_stmt_calls(body, callees);
            collect_expr_calls(condition, callees);
        }
        TypedStmt::For {
            init,
            condition,
            step,
            body,
            ..
        } => {
            if let Some(init) = init {
                collect_stmt_calls(init, callees);
            }
            if let Some(condition) = condition {
                collect_expr_calls(condition, callees);
            }
            if let Some(step) = step {
                collect_expr_calls(step, callees);
            }
            collect_stmt_calls(body, callees);
        }
        TypedStmt::Return(expr, _) => {
            if let Some(expr) = expr {
                collect_expr_calls(expr, callees);
            }
        }
        TypedStmt::Break(_) | TypedStmt::Continue(_) | TypedStmt::Empty(_) => {}
    }
}

/// Collects direct function-call targets that appear anywhere inside one typed expression tree.
fn collect_expr_calls(expr: &TypedExpr, callees: &mut BTreeSet<SymbolId>) {
    match &expr.kind {
        TypedExprKind::Unary { expr, .. }
        | TypedExprKind::ArrayDecay(expr)
        | TypedExprKind::AddressOf(expr)
        | TypedExprKind::Deref(expr)
        | TypedExprKind::Cast { expr, .. } => collect_expr_calls(expr, callees),
        TypedExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_calls(lhs, callees);
            collect_expr_calls(rhs, callees);
        }
        TypedExprKind::Assign { target, value } => {
            collect_expr_calls(target, callees);
            collect_expr_calls(value, callees);
        }
        TypedExprKind::Call { function, args } => {
            callees.insert(*function);
            for arg in args {
                collect_expr_calls(arg, callees);
            }
        }
        TypedExprKind::IntLiteral(_) | TypedExprKind::Symbol(_) => {}
    }
}

/// Synthesizes a typed zero literal for semantic error recovery paths.
fn zero_expr(span: Span) -> TypedExpr {
    TypedExpr {
        kind: TypedExprKind::IntLiteral(0),
        ty: Type::new(ScalarType::I16),
        span,
        value_category: ValueCategory::RValue,
    }
}

#[cfg(test)]
mod tests {
    use super::{is_constant_expression, ValueCategory};
    use crate::common::source::Span;
    use crate::frontend::semantic::{TypedExpr, TypedExprKind};
    use crate::frontend::types::{ScalarType, Type};

    #[test]
    /// Verifies `sizeof`-compatible array and pointer types report the expected byte widths.
    fn phase_three_type_sizes_cover_arrays_and_pointers() {
        assert_eq!(Type::new(ScalarType::U8).byte_width(), 1);
        assert_eq!(Type::new(ScalarType::U16).pointer_to().byte_width(), 2);
        assert_eq!(Type::new(ScalarType::I16).array_of(4).byte_width(), 8);
    }

    #[test]
    /// Verifies scalar and array expressions keep distinct value categories.
    fn phase_three_value_categories_distinguish_places_from_values() {
        let span = Span::new(0, 0);
        let lvalue = TypedExpr {
            kind: TypedExprKind::Symbol(1),
            ty: Type::new(ScalarType::U8),
            span,
            value_category: ValueCategory::LValue,
        };
        let rvalue = TypedExpr {
            kind: TypedExprKind::IntLiteral(3),
            ty: Type::new(ScalarType::U16),
            span,
            value_category: ValueCategory::RValue,
        };

        assert_eq!(lvalue.value_category, ValueCategory::LValue);
        assert_eq!(rvalue.value_category, ValueCategory::RValue);
    }

    #[test]
    /// Verifies address and decay forms stay out of constant-expression classification.
    fn phase_three_constant_expr_rejects_memory_address_forms() {
        let span = Span::new(0, 0);
        let symbol = TypedExpr {
            kind: TypedExprKind::Symbol(0),
            ty: Type::new(ScalarType::U8),
            span,
            value_category: ValueCategory::LValue,
        };
        let decay = TypedExpr {
            kind: TypedExprKind::ArrayDecay(Box::new(symbol)),
            ty: Type::new(ScalarType::U8).pointer_to(),
            span,
            value_category: ValueCategory::RValue,
        };

        assert!(!is_constant_expression(&decay));
    }
}
