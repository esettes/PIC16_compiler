// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use crate::backend::pic16::devices::TargetDevice;
use crate::common::integer::{eval_binary, eval_unary, infer_integer_literal_type, normalize_value, signed_value};
use crate::common::source::Span;
use crate::diagnostics::DiagnosticBag;

use super::ast::{
    BinaryOp, Designator, Expr, ExprKind, FunctionDecl, Initializer, InitializerEntry, Item,
    Stmt, StructDef, TranslationUnit, UnaryOp, UnionDef, VarDecl,
};
use super::types::{
    AddressSpace, CastKind, Qualifiers, ScalarType, StorageClass, StructId, Type, UnionId,
};

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
    pub is_interrupt: bool,
    pub kind: SymbolKind,
    pub span: Span,
    pub fixed_address: Option<u16>,
    pub is_defined: bool,
    pub is_referenced: bool,
    pub parameter_types: Vec<Type>,
    pub enum_const_value: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Function,
    Global,
    StringLiteral,
    Local,
    Param,
    DeviceRegister,
    EnumConstant,
}

#[derive(Clone, Debug)]
pub enum TypedGlobalInitializer {
    Scalar(TypedExpr),
    Bytes(Vec<u8>),
    Address { symbol: SymbolId, offset: usize },
}

#[derive(Clone, Debug)]
pub struct TypedGlobal {
    pub symbol: SymbolId,
    pub initializer: Option<TypedGlobalInitializer>,
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
    Switch {
        expr: TypedExpr,
        body: Box<TypedStmt>,
        span: Span,
    },
    Case {
        value: i64,
        body: Box<TypedStmt>,
        span: Span,
    },
    Default {
        body: Box<TypedStmt>,
        span: Span,
    },
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
    BitField {
        storage: Box<TypedExpr>,
        bit_offset: u8,
        bit_width: u8,
    },
    Assign {
        target: Box<TypedExpr>,
        value: Box<TypedExpr>,
    },
    StructAssign {
        target: Box<TypedExpr>,
        value: Box<TypedExpr>,
        size: usize,
    },
    RomRead8 {
        symbol: SymbolId,
        index: Box<TypedExpr>,
    },
    RomRead16 {
        symbol: SymbolId,
        index: Box<TypedExpr>,
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

#[derive(Clone, Debug)]
struct AggregateAssignment {
    offset: usize,
    ty: Type,
    value: TypedExpr,
    bit_offset: Option<u8>,
    bit_width: Option<u8>,
}

#[derive(Clone, Copy, Debug)]
struct BitfieldLocation {
    offset: usize,
    bit_offset: u8,
    bit_width: u8,
}

#[derive(Clone, Debug)]
struct AggregateInitPlan {
    size: usize,
    assignments: Vec<AggregateAssignment>,
}

#[derive(Clone, Debug)]
enum AnalyzedInitializer {
    Scalar(TypedExpr),
    Aggregate(AggregateInitPlan),
}

struct AggregateInitContext<'a> {
    mode: &'a str,
    assignments: &'a mut Vec<AggregateAssignment>,
    diagnostics: &'a mut DiagnosticBag,
}

#[derive(Clone, Debug)]
struct SwitchContext {
    expr_ty: Type,
    case_values: BTreeMap<i64, Span>,
    default_span: Option<Span>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PointerConversionError {
    Incompatible,
    QualifierDiscard,
    NestedQualifierMismatch,
}

pub struct SemanticAnalyzer<'a> {
    target: &'a TargetDevice,
    symbols: Vec<Symbol>,
    globals: Vec<TypedGlobal>,
    functions: Vec<TypedFunction>,
    struct_defs: Vec<StructDef>,
    union_defs: Vec<UnionDef>,
    globals_by_name: BTreeMap<String, SymbolId>,
    scopes: Vec<BTreeMap<String, SymbolId>>,
    current_function: Option<SymbolId>,
    loop_depth: usize,
    switch_stack: Vec<SwitchContext>,
    switch_label_modes: Vec<bool>,
    string_literal_counter: usize,
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
            struct_defs: Vec::new(),
            union_defs: Vec::new(),
            globals_by_name: BTreeMap::new(),
            scopes: Vec::new(),
            current_function: None,
            loop_depth: 0,
            switch_stack: Vec::new(),
            switch_label_modes: Vec::new(),
            string_literal_counter: 0,
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
        self.struct_defs = unit.struct_defs;
        self.union_defs = unit.union_defs;
        self.validate_aggregate_field_types(diagnostics);
        for constant in unit.enum_constants {
            self.declare_enum_constant(constant.name, constant.value, constant.span, diagnostics);
        }

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
        self.validate_interrupt_handlers(diagnostics);

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
                is_interrupt: false,
                kind: SymbolKind::DeviceRegister,
                span: Span::new(0, 0),
                fixed_address: Some(register.address),
                is_defined: true,
                is_referenced: false,
                parameter_types: Vec::new(),
                enum_const_value: None,
            });
            self.globals_by_name.insert(register.name.to_string(), symbol);
        }
    }

    /// Seeds enum constants as immutable global compile-time symbols.
    fn declare_enum_constant(
        &mut self,
        name: String,
        value: i64,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) {
        if self.globals_by_name.contains_key(&name) {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("redefinition of symbol `{name}`"),
                Some("enum constants must use unique global names".to_string()),
            );
            return;
        }

        let symbol = self.insert_symbol(Symbol {
            id: self.symbols.len(),
            name: name.clone(),
            ty: Type::new(ScalarType::I16),
            storage_class: StorageClass::Extern,
            is_interrupt: false,
            kind: SymbolKind::EnumConstant,
            span,
            fixed_address: None,
            is_defined: true,
            is_referenced: false,
            parameter_types: Vec::new(),
            enum_const_value: Some(value),
        });
        self.globals_by_name.insert(name, symbol);
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

        let ty = self.resolve_object_decl_type(
            global.ty,
            global.initializer.as_ref(),
            global.span,
            &global.name,
            "global",
            diagnostics,
        );
        self.validate_object_type(ty, global.span, &global.name, "global", diagnostics);

        let symbol = self.insert_symbol(Symbol {
            id: self.symbols.len(),
            name: global.name.clone(),
            ty,
            storage_class: global.storage_class,
            is_interrupt: false,
            kind: SymbolKind::Global,
            span: global.span,
            fixed_address: None,
            is_defined: global.initializer.is_none(),
            is_referenced: false,
            parameter_types: Vec::new(),
                enum_const_value: None,
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
            } else if existing_symbol.is_interrupt != function.is_interrupt {
                diagnostics.error(
                    "semantic",
                    Some(function.span),
                    format!(
                        "function `{}` changes interrupt qualifier between declarations",
                        function.name
                    ),
                    Some("declare the ISR consistently on every prototype and definition".to_string()),
                );
            }
            return;
        }

        self.validate_return_type(function.return_type, function.span, &function.name, diagnostics);
        self.validate_interrupt_signature(function, diagnostics);

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
            is_interrupt: function.is_interrupt,
            kind: SymbolKind::Function,
            span: function.span,
            fixed_address: None,
            is_defined: function.body.is_none(),
            is_referenced: function.is_interrupt,
            parameter_types,
            enum_const_value: None,
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
        if self.symbols[symbol].ty.is_rom() && global.initializer.is_none() {
            diagnostics.error(
                "semantic",
                Some(global.span),
                format!(
                    "program-memory object `{}` requires a constant initializer in phase 13",
                    global.name
                ),
                Some("initialize ROM tables and strings at the declaration site".to_string()),
            );
        }
        let initializer = global.initializer.as_ref().and_then(|init| {
            self.analyze_global_initializer(self.symbols[symbol].ty, init, global.span, diagnostics)
        });

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
                is_interrupt: false,
                kind: SymbolKind::Param,
                span: param.span,
                fixed_address: None,
                is_defined: true,
                is_referenced: false,
                parameter_types: Vec::new(),
                enum_const_value: None,
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
            if self.symbols[symbol].is_interrupt {
                self.reject_interrupt_body(symbol, body, diagnostics);
            }
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
                let decl_ty = self.resolve_object_decl_type(
                    decl.ty,
                    decl.initializer.as_ref(),
                    decl.span,
                    &decl.name,
                    "local",
                    diagnostics,
                );
                self.validate_object_type(decl_ty, decl.span, &decl.name, "local", diagnostics);
                let symbol = self.insert_scoped_symbol(
                    decl.name.clone(),
                    decl_ty,
                    decl.storage_class,
                    SymbolKind::Local,
                    decl.span,
                );
                if decl.storage_class == StorageClass::Static {
                    let initializer = decl.initializer.as_ref().and_then(|init| {
                        self.analyze_global_initializer(decl_ty, init, decl.span, diagnostics)
                    });
                    self.globals.push(TypedGlobal {
                        symbol,
                        initializer,
                    });
                    return TypedStmt::VarDecl(symbol, None, decl.span);
                }
                let Some(initializer) = decl.initializer.as_ref() else {
                    return TypedStmt::VarDecl(symbol, None, decl.span);
                };

                if self.current_function_is_interrupt()
                    && (decl_ty.is_array() || decl_ty.is_struct() || decl_ty.is_union())
                {
                    diagnostics.error(
                        "semantic",
                        Some(decl.span),
                        "aggregate initializers are not allowed inside interrupt handlers in phase 15",
                        Some("initialize aggregates outside ISR code and pass scalar values into the handler".to_string()),
                    );
                    return TypedStmt::VarDecl(symbol, None, decl.span);
                }

                match self.analyze_initializer_value(
                    decl_ty,
                    initializer,
                    "local initializer",
                    diagnostics,
                ) {
                    Some(AnalyzedInitializer::Scalar(expr)) => {
                        TypedStmt::VarDecl(symbol, Some(expr), decl.span)
                    }
                    Some(AnalyzedInitializer::Aggregate(plan)) => {
                        let mut statements =
                            Vec::with_capacity(plan.size.saturating_add(plan.assignments.len()) + 1);
                        statements.push(TypedStmt::VarDecl(symbol, None, decl.span));
                        let byte_ty = Type::new(ScalarType::U8);
                        for offset in 0..plan.size {
                            let target =
                                self.build_symbol_offset_lvalue(symbol, decl_ty, offset, byte_ty, decl.span);
                            let expr = TypedExpr {
                                kind: TypedExprKind::Assign {
                                    target: Box::new(target),
                                    value: Box::new(zero_expr(decl.span)),
                                },
                                ty: byte_ty,
                                span: decl.span,
                                value_category: ValueCategory::RValue,
                            };
                            statements.push(TypedStmt::Expr(expr, decl.span));
                        }
                        for assignment in plan.assignments {
                            let target = if let (Some(bit_offset), Some(bit_width)) =
                                (assignment.bit_offset, assignment.bit_width)
                            {
                                let storage = self.build_symbol_offset_lvalue(
                                    symbol,
                                    decl_ty,
                                    assignment.offset,
                                    assignment.ty,
                                    decl.span,
                                );
                                TypedExpr {
                                    kind: TypedExprKind::BitField {
                                        storage: Box::new(storage),
                                        bit_offset,
                                        bit_width,
                                    },
                                    ty: assignment.ty,
                                    span: decl.span,
                                    value_category: ValueCategory::LValue,
                                }
                            } else {
                                self.build_symbol_offset_lvalue(
                                    symbol,
                                    decl_ty,
                                    assignment.offset,
                                    assignment.ty,
                                    decl.span,
                                )
                            };
                            let value = assignment.value;
                            let expr = TypedExpr {
                                kind: TypedExprKind::Assign {
                                    target: Box::new(target),
                                    value: Box::new(value),
                                },
                                ty: assignment.ty,
                                span: decl.span,
                                value_category: ValueCategory::RValue,
                            };
                            statements.push(TypedStmt::Expr(expr, decl.span));
                        }
                        TypedStmt::Block(statements, decl.span)
                    }
                    None => TypedStmt::VarDecl(symbol, None, decl.span),
                }
            }
            Stmt::Expr(expr, span) => TypedStmt::Expr(
                self.analyze_expr(expr, diagnostics)
                    .unwrap_or_else(|| zero_expr(*span)),
                *span,
            ),
            Stmt::Switch { expr, body, span } => {
                let expr = self
                    .analyze_expr(expr, diagnostics)
                    .unwrap_or_else(|| zero_expr(*span));
                let expr = if expr.ty.is_integer() {
                    expr
                } else {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        "switch expression must have integer or enum type",
                        None,
                    );
                    zero_expr(expr.span)
                };

                self.switch_stack.push(SwitchContext {
                    expr_ty: expr.ty,
                    case_values: BTreeMap::new(),
                    default_span: None,
                });
                self.switch_label_modes.push(true);
                let body = Box::new(self.analyze_stmt(body, diagnostics));
                self.switch_label_modes.pop();
                self.switch_stack.pop();

                TypedStmt::Switch {
                    expr,
                    body,
                    span: *span,
                }
            }
            Stmt::Case { value, body, span } => {
                if self.switch_stack.is_empty() {
                    diagnostics.error("semantic", Some(*span), "`case` label outside switch", None);
                    return self.analyze_stmt(body, diagnostics);
                }
                if !self.current_switch_labels_allowed() {
                    diagnostics.error(
                        "semantic",
                        Some(*span),
                        "case labels nested inside control statements are not supported in phase 9",
                        Some("move the case label to the surrounding switch block or nested block".to_string()),
                    );
                    return self.analyze_stmt(body, diagnostics);
                }

                let switch_ty = self.current_switch_type().expect("switch context");
                let typed_value = self.analyze_expr(value, diagnostics);
                let value = self.validate_case_value(typed_value, switch_ty, *span, diagnostics);
                let body = Box::new(self.analyze_stmt(body, diagnostics));
                if let Some(value) = value {
                    TypedStmt::Case {
                        value,
                        body,
                        span: *span,
                    }
                } else {
                    *body
                }
            }
            Stmt::Default { body, span } => {
                if self.switch_stack.is_empty() {
                    diagnostics.error("semantic", Some(*span), "`default` label outside switch", None);
                    return self.analyze_stmt(body, diagnostics);
                }
                if !self.current_switch_labels_allowed() {
                    diagnostics.error(
                        "semantic",
                        Some(*span),
                        "default label nested inside control statements is not supported in phase 9",
                        Some("move the default label to the surrounding switch block or nested block".to_string()),
                    );
                    return self.analyze_stmt(body, diagnostics);
                }
                if let Some(previous) = self.current_switch_default_span() {
                    diagnostics.error(
                        "semantic",
                        Some(*span),
                        "multiple `default` labels in one switch are not allowed",
                        Some(format!("previous default label starts at byte {}", previous.start)),
                    );
                } else if let Some(context) = self.switch_stack.last_mut() {
                    context.default_span = Some(*span);
                }
                TypedStmt::Default {
                    body: Box::new(self.analyze_stmt(body, diagnostics)),
                    span: *span,
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => TypedStmt::If {
                condition: self
                    .analyze_expr(condition, diagnostics)
                    .unwrap_or_else(|| zero_expr(*span)),
                then_branch: Box::new(
                    self.analyze_stmt_with_case_labels_disabled(then_branch, diagnostics),
                ),
                else_branch: else_branch
                    .as_ref()
                    .map(|branch| {
                        Box::new(self.analyze_stmt_with_case_labels_disabled(branch, diagnostics))
                    }),
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
                    body: Box::new(
                        self.analyze_stmt_with_case_labels_disabled(body, diagnostics),
                    ),
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
                    body: Box::new(
                        self.analyze_stmt_with_case_labels_disabled(body, diagnostics),
                    ),
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
                    body: Box::new(
                        self.analyze_stmt_with_case_labels_disabled(body, diagnostics),
                    ),
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
                            if self.symbols[current_function].is_interrupt {
                                "interrupt handler cannot return a value"
                            } else {
                                "void function cannot return a value"
                            },
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
                if self.loop_depth == 0 && self.switch_stack.is_empty() {
                    diagnostics.error(
                        "semantic",
                        Some(*span),
                        "`break` outside loop or switch",
                        None,
                    );
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
            ExprKind::StringLiteral(bytes) => self.analyze_string_literal_expr(bytes, expr.span),
            ExprKind::Name(name) => self.analyze_name(name, expr.span, diagnostics)?,
            ExprKind::Cast { ty, expr: value } => {
                self.analyze_explicit_cast_expr(*ty, value, expr.span, diagnostics)?
            }
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
            ExprKind::Member { base, field } => {
                self.analyze_member_expr(base, field, false, expr.span, diagnostics)?
            }
            ExprKind::PointerMember { base, field } => {
                self.analyze_member_expr(base, field, true, expr.span, diagnostics)?
            }
            ExprKind::SizeOfExpr(value) => self.analyze_sizeof_expr(value, expr.span, diagnostics)?,
            ExprKind::SizeOfType(ty) => self.analyze_sizeof_type(*ty, expr.span, diagnostics)?,
        };

        if decay_arrays && typed.ty.is_array() {
            if typed.ty.is_rom() {
                diagnostics.error(
                    "semantic",
                    Some(expr.span),
                    "program-memory arrays do not decay to data-space pointers in phase 14",
                    Some("read ROM arrays with `table[index]`, `__rom_read8()`, or `__rom_read16()`".to_string()),
                );
                return Some(typed);
            }
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

        if self.symbols[symbol].kind == SymbolKind::EnumConstant {
            self.symbols[symbol].is_referenced = true;
            return Some(TypedExpr {
                kind: TypedExprKind::IntLiteral(self.symbols[symbol].enum_const_value.unwrap_or(0)),
                ty: self.symbols[symbol].ty,
                span,
                value_category: ValueCategory::RValue,
            });
        }

        self.symbols[symbol].is_referenced = true;
        Some(TypedExpr {
            kind: TypedExprKind::Symbol(symbol),
            ty: self.symbols[symbol].ty,
            span,
            value_category: ValueCategory::LValue,
        })
    }

    /// Materializes one string literal as a synthetic static RAM array object.
    fn analyze_string_literal_expr(&mut self, bytes: &[u8], span: Span) -> TypedExpr {
        let symbol = self.intern_string_literal(bytes, span);
        self.symbols[symbol].is_referenced = true;
        TypedExpr {
            kind: TypedExprKind::Symbol(symbol),
            ty: self.symbols[symbol].ty,
            span,
            value_category: ValueCategory::LValue,
        }
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
        if matches!(value.kind, TypedExprKind::RomRead8 { .. } | TypedExprKind::RomRead16 { .. }) {
            diagnostics.error(
                "semantic",
                Some(span),
                "taking the address of a program-memory array element is not supported in phase 14",
                Some("ROM pointers are still unsupported; read the element value directly instead".to_string()),
            );
            return None;
        }
        if matches!(value.kind, TypedExprKind::BitField { .. }) {
            diagnostics.error(
                "semantic",
                Some(span),
                "taking the address of a bitfield is not supported in phase 15",
                Some("copy the bitfield value into an ordinary scalar object first".to_string()),
            );
            return None;
        }
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
        if value.ty.is_rom() {
            diagnostics.error(
                "semantic",
                Some(span),
                "taking the address of a program-memory object is not supported in phase 14",
                Some("read ROM data through direct indexing or `__rom_read8()` / `__rom_read16()`; ROM pointers are not modeled yet".to_string()),
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

    /// Analyzes an explicit C-style cast with Phase 8 scalar and pointer restrictions.
    fn analyze_explicit_cast_expr(
        &mut self,
        target_ty: Type,
        expr: &Expr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        self.validate_const_placement(target_ty, span, "explicit cast target", diagnostics);
        if target_ty.is_pointer() && target_ty.element_type().is_rom() {
            return None;
        }
        if target_ty.is_array() || target_ty.is_struct() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("explicit cast target `{}` is not supported", target_ty),
                Some("cast to scalar values or supported data pointers only".to_string()),
            );
            return None;
        }

        let value = self.analyze_expr(expr, diagnostics)?;
        if value.ty == target_ty {
            return Some(value);
        }

        if value.ty.is_integer() && target_ty.is_integer() {
            return Some(self.coerce_expr(
                value,
                target_ty,
                diagnostics,
                "explicit cast",
                false,
            ));
        }

        if value.ty.is_pointer() && target_ty.is_pointer() {
            return Some(TypedExpr {
                kind: TypedExprKind::Cast {
                    kind: CastKind::Bitcast,
                    expr: Box::new(value),
                },
                ty: target_ty,
                span,
                value_category: ValueCategory::RValue,
            });
        }

        if value.ty.is_integer() && target_ty.is_pointer() {
            if !self.is_integer_zero_constant_expr(&value) {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    "only integer zero may be cast to a pointer in phase 8",
                    Some("use `(T*)0` for null pointer constants".to_string()),
                );
                return None;
            }
            return Some(TypedExpr {
                kind: TypedExprKind::IntLiteral(0),
                ty: target_ty,
                span,
                value_category: ValueCategory::RValue,
            });
        }

        if value.ty.is_pointer() && target_ty.is_integer() {
            if target_ty.bit_width() != 16 {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    "pointer-to-integer casts are only supported for 16-bit integer targets",
                    Some("cast pointers to `int` or `unsigned int` in phase 8".to_string()),
                );
                return None;
            }
            return Some(TypedExpr {
                kind: TypedExprKind::Cast {
                    kind: CastKind::Bitcast,
                    expr: Box::new(value),
                },
                ty: target_ty,
                span,
                value_category: ValueCategory::RValue,
            });
        }

        diagnostics.error(
            "semantic",
            Some(span),
            format!("unsupported explicit cast from `{}` to `{}`", value.ty, target_ty),
            None,
        );
        None
    }

    /// Analyzes `.` and `->` access by lowering to byte-address arithmetic plus dereference.
    fn analyze_member_expr(
        &mut self,
        base: &Expr,
        field: &str,
        through_pointer: bool,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let base = if through_pointer {
            self.analyze_expr(base, diagnostics)?
        } else {
            self.analyze_expr_with_decay(base, diagnostics, false)?
        };

        let aggregate_ty = if through_pointer {
            if !base.ty.is_pointer() {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    "`->` requires a pointer-to-struct or pointer-to-union operand",
                    None,
                );
                return None;
            }
            base.ty.element_type()
        } else {
            if !base.ty.is_aggregate() || base.value_category != ValueCategory::LValue {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    "`.` requires a struct or union lvalue operand",
                    None,
                );
                return None;
            }
            base.ty
        };

        if !aggregate_ty.is_aggregate() {
            diagnostics.error(
                "semantic",
                Some(span),
                "member access requires a struct or union type",
                None,
            );
            return None;
        }

        let Some(field_def) = self.aggregate_field(aggregate_ty, field) else {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("unknown field `{field}`"),
                None,
            );
            return None;
        };

        let field_object_qualifiers = field_def.ty.object_qualifiers();
        let aggregate_object_qualifiers = aggregate_ty.object_qualifiers();
        let field_ty = field_def.ty.with_object_qualifiers(Qualifiers {
            is_const: field_object_qualifiers.is_const || aggregate_object_qualifiers.is_const,
            is_volatile: field_object_qualifiers.is_volatile || aggregate_object_qualifiers.is_volatile,
        });

        if let Some(bit_width) = field_def.bit_width {
            let storage = self.build_member_lvalue(base, through_pointer, field_def.offset, field_ty, span);
            return Some(TypedExpr {
                kind: TypedExprKind::BitField {
                    storage: Box::new(storage),
                    bit_offset: field_def.bit_offset,
                    bit_width,
                },
                ty: field_ty,
                span,
                value_category: ValueCategory::LValue,
            });
        }

        Some(self.build_member_lvalue(base, through_pointer, field_def.offset, field_ty, span))
    }

    /// Builds an lvalue expression that references one scalar field at `base + offset`.
    fn build_member_lvalue(
        &self,
        base: TypedExpr,
        through_pointer: bool,
        offset: usize,
        field_ty: Type,
        span: Span,
    ) -> TypedExpr {
        let byte_ptr_ty = Type::new(ScalarType::U8).pointer_to();
        let mut base_ptr = if through_pointer {
            base
        } else {
            TypedExpr {
                kind: TypedExprKind::AddressOf(Box::new(base.clone())),
                ty: base.ty.pointer_to(),
                span,
                value_category: ValueCategory::RValue,
            }
        };

        if base_ptr.ty != byte_ptr_ty {
            base_ptr = TypedExpr {
                kind: TypedExprKind::Cast {
                    kind: CastKind::Bitcast,
                    expr: Box::new(base_ptr),
                },
                ty: byte_ptr_ty,
                span,
                value_category: ValueCategory::RValue,
            };
        }

        let raw_ptr = if offset == 0 {
            base_ptr
        } else {
            TypedExpr {
                kind: TypedExprKind::Binary {
                    op: BinaryOp::Add,
                    lhs: Box::new(base_ptr),
                    rhs: Box::new(TypedExpr {
                        kind: TypedExprKind::IntLiteral(offset as i64),
                        ty: Type::new(ScalarType::U16),
                        span,
                        value_category: ValueCategory::RValue,
                    }),
                },
                ty: byte_ptr_ty,
                span,
                value_category: ValueCategory::RValue,
            }
        };

        let field_ptr_ty = field_ty.pointer_to();
        let field_ptr = if raw_ptr.ty == field_ptr_ty {
            raw_ptr
        } else {
            TypedExpr {
                kind: TypedExprKind::Cast {
                    kind: CastKind::Bitcast,
                    expr: Box::new(raw_ptr),
                },
                ty: field_ptr_ty,
                span,
                value_category: ValueCategory::RValue,
            }
        };

        TypedExpr {
            kind: TypedExprKind::Deref(Box::new(field_ptr)),
            ty: field_ty,
            span,
            value_category: ValueCategory::LValue,
        }
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
                    return self.analyze_pointer_relational_expr(op, lhs, rhs, span, diagnostics);
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
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                let lhs = self.analyze_expr(lhs, diagnostics)?;
                let rhs = self.analyze_expr(rhs, diagnostics)?;
                self.analyze_shift_expr(op, lhs, rhs, span, diagnostics)
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
                self.diagnose_division_rhs(op, &rhs, span, diagnostics);
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

    /// Analyzes relational comparisons across compatible data-space pointer values.
    fn analyze_pointer_relational_expr(
        &mut self,
        op: BinaryOp,
        lhs: TypedExpr,
        rhs: TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        if !lhs.ty.is_pointer() || !rhs.ty.is_pointer() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("`{op:?}` requires matching integer or pointer operands"),
                None,
            );
            return None;
        }
        if !self.are_compatible_pointer_compare_types(lhs.ty, rhs.ty) {
            diagnostics.error(
                "semantic",
                Some(span),
                format!(
                    "pointer relational comparison requires compatible pointer types, got `{}` and `{}`",
                    lhs.ty, rhs.ty
                ),
                Some("cast explicitly if you really need an address-order comparison across types".to_string()),
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

    /// Analyzes integer arithmetic plus supported data-pointer arithmetic forms.
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
                    if lhs.ty.is_pointer() && rhs.ty.is_pointer() {
                        return self.build_pointer_difference_expr(lhs, rhs, span, diagnostics);
                    }
                }
                _ => {}
            }
            diagnostics.error(
                "semantic",
                Some(span),
                "unsupported pointer arithmetic form in phase 12",
                Some("use pointer +/- integer or compatible pointer subtraction only".to_string()),
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
        let base = self.analyze_expr_with_decay(base, diagnostics, false)?;
        let index = self.analyze_expr(index, diagnostics)?;
        if base.ty.is_array() {
            if base.ty.is_rom() {
                return self.analyze_rom_index_expr(base, index, span, diagnostics);
            }
            let base_span = base.span;
            let base = self.decay_array_expr(base, base_span);
            return self.analyze_data_index_expr(base, index, span, diagnostics);
        }
        self.analyze_data_index_expr(base, index, span, diagnostics)
    }

    /// Lowers one data-space indexing expression into pointer arithmetic followed by dereference.
    fn analyze_data_index_expr(
        &mut self,
        base: TypedExpr,
        index: TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
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

    /// Lowers one direct ROM array read into a dedicated typed ROM-read expression.
    fn analyze_rom_index_expr(
        &mut self,
        base: TypedExpr,
        index: TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        let TypedExprKind::Symbol(symbol) = base.kind else {
            diagnostics.error(
                "semantic",
                Some(base.span),
                "direct ROM indexing requires a named file-scope ROM array object",
                Some("index the declared ROM table name directly".to_string()),
            );
            return None;
        };
        if self.symbols[symbol].kind != SymbolKind::Global {
            diagnostics.error(
                "semantic",
                Some(base.span),
                "direct ROM indexing only accepts file-scope ROM objects in phase 14",
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
        let result_ty = Type::new(element_ty.scalar)
            .with_qualifiers(Qualifiers::default())
            .with_address_space(AddressSpace::Data);
        let index = self.coerce_expr(index, Type::new(ScalarType::U16), diagnostics, "ROM index", true);

        match element_ty.scalar {
            ScalarType::I8 | ScalarType::U8 => Some(TypedExpr {
                kind: TypedExprKind::RomRead8 {
                    symbol,
                    index: Box::new(index),
                },
                ty: result_ty,
                span,
                value_category: ValueCategory::RValue,
            }),
            ScalarType::I16 | ScalarType::U16 => Some(TypedExpr {
                kind: TypedExprKind::RomRead16 {
                    symbol,
                    index: Box::new(index),
                },
                ty: result_ty,
                span,
                value_category: ValueCategory::RValue,
            }),
            ScalarType::Void => {
                diagnostics.error(
                    "semantic",
                    Some(base.span),
                    format!(
                        "direct ROM indexing supports only `char`, `unsigned char`, `int`, or `unsigned int`, got `{}`",
                        base.ty
                    ),
                    None,
                );
                None
            }
        }
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
        if matches!(target.kind, TypedExprKind::RomRead8 { .. } | TypedExprKind::RomRead16 { .. }) {
            diagnostics.error(
                "semantic",
                Some(target.span),
                "writing to a program-memory array element is not allowed in phase 14",
                Some("ROM data is read-only; copy into RAM first if you need writable storage".to_string()),
            );
            return None;
        }
        if target.ty.object_is_const() {
            diagnostics.error(
                "semantic",
                Some(target.span),
                "assignment to const object is not allowed in phase 12",
                Some("initialize the const object at declaration time instead".to_string()),
            );
            return None;
        }
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
        if let TypedExprKind::BitField { bit_width, .. } = target.kind
            && let Some(raw) = signed_or_unsigned_constant_value(&value)
        {
            let mask = if bit_width as usize >= target_ty.bit_width() {
                target_ty.mask()
            } else {
                (1_i64 << bit_width) - 1
            };
            if normalize_value(raw, target_ty) != (normalize_value(raw, target_ty) & mask) {
                diagnostics.warning(
                    "semantic",
                    Some(value.span),
                    format!("assignment value for {}-bit bitfield truncates to fit", bit_width),
                    "W1501",
                );
            }
        }
        if target_ty.is_struct() || target_ty.is_union() {
            if self.current_function_is_interrupt() {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    "whole-aggregate assignment is not supported inside interrupt handlers in phase 15",
                    Some("copy scalar fields explicitly outside the ISR".to_string()),
                );
                return None;
            }
            if value.ty.unqualified() != target_ty.unqualified()
                || (!value.ty.is_struct() && !value.ty.is_union())
            {
                let aggregate_kind = if target_ty.is_union() { "union" } else { "struct" };
                diagnostics.error(
                    "semantic",
                    Some(value.span),
                    format!(
                        "cannot assign incompatible {aggregate_kind} type `{}` to `{}`",
                        value.ty, target_ty
                    ),
                    Some(format!("assign only between the same named {aggregate_kind} type")),
                );
                return None;
            }
            return Some(TypedExpr {
                kind: TypedExprKind::StructAssign {
                    target: Box::new(target),
                    value: Box::new(value),
                    size: target_ty.byte_width(),
                },
                ty: target_ty,
                span,
                value_category: ValueCategory::RValue,
            });
        }
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
        if callee == "__rom_read8" {
            return self.analyze_rom_read8_expr(args, span, diagnostics);
        }
        if callee == "__rom_read16" {
            return self.analyze_rom_read16_expr(args, span, diagnostics);
        }

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

    /// Analyzes the Phase 14 ROM-byte intrinsic over one named `const __rom` byte array object.
    fn analyze_rom_read8_expr(
        &mut self,
        args: &[Expr],
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        self.analyze_rom_read_expr(args, span, diagnostics, 1, "__rom_read8")
    }

    /// Analyzes the Phase 14 ROM-word intrinsic over one named `const __rom` 16-bit array object.
    fn analyze_rom_read16_expr(
        &mut self,
        args: &[Expr],
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        self.analyze_rom_read_expr(args, span, diagnostics, 2, "__rom_read16")
    }

    /// Analyzes one explicit ROM-read builtin over a named file-scope ROM array object.
    fn analyze_rom_read_expr(
        &mut self,
        args: &[Expr],
        span: Span,
        diagnostics: &mut DiagnosticBag,
        element_width: usize,
        builtin: &str,
    ) -> Option<TypedExpr> {
        if args.len() != 2 {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("`{builtin}` expects 2 argument(s), got {}", args.len()),
                Some(format!("use `{builtin}(table, index)`")),
            );
            return None;
        }

        let object = self.analyze_expr_with_decay(&args[0], diagnostics, false)?;
        let TypedExprKind::Symbol(symbol) = object.kind else {
            diagnostics.error(
                "semantic",
                Some(object.span),
                format!("`{builtin}` requires a named ROM array object as its first argument"),
                Some("pass the declared ROM table name directly".to_string()),
            );
            return None;
        };
        if !object.ty.is_array() || !object.ty.is_rom() {
            diagnostics.error(
                "semantic",
                Some(object.span),
                format!("first `{builtin}` argument must be a `const __rom` array object"),
                None,
            );
            return None;
        }
        if self.symbols[symbol].kind != SymbolKind::Global {
            diagnostics.error(
                "semantic",
                Some(object.span),
                format!("`{builtin}` only accepts file-scope ROM objects in phase 14"),
                None,
            );
            return None;
        }
        let element_ty = object.ty.element_type();
        if !matches!(
            (element_width, element_ty.scalar),
            (1, ScalarType::I8 | ScalarType::U8) | (2, ScalarType::I16 | ScalarType::U16)
        )
            || element_ty.pointer_depth != 0
            || element_ty.struct_id.is_some()
        {
            diagnostics.error(
                "semantic",
                Some(object.span),
                format!(
                    "`{builtin}` only supports ROM arrays of {}, got `{}`",
                    if element_width == 1 {
                        "`char` or `unsigned char`"
                    } else {
                        "`int` or `unsigned int`"
                    },
                    object.ty,
                ),
                None,
            );
            return None;
        }

        self.symbols[symbol].is_referenced = true;
        let index = self.analyze_expr(&args[1], diagnostics)?;
        if !index.ty.is_integer() {
            diagnostics.error(
                "semantic",
                Some(index.span),
                format!("`{builtin}` index must be an integer expression"),
                None,
            );
            return None;
        }
        let index = self.coerce_expr(
            index,
            Type::new(ScalarType::U16),
            diagnostics,
            "ROM read index",
            true,
        );

        let result_ty = Type::new(element_ty.scalar)
            .with_qualifiers(Qualifiers::default())
            .with_address_space(AddressSpace::Data);
        Some(match element_width {
            1 => TypedExpr {
                kind: TypedExprKind::RomRead8 {
                    symbol,
                    index: Box::new(index),
                },
                ty: result_ty,
                span,
                value_category: ValueCategory::RValue,
            },
            2 => TypedExpr {
                kind: TypedExprKind::RomRead16 {
                    symbol,
                    index: Box::new(index),
                },
                ty: result_ty,
                span,
                value_category: ValueCategory::RValue,
            },
            _ => unreachable!("unsupported ROM builtin width"),
        })
    }

    /// Analyzes one object initializer and flattens aggregates into byte-offset assignments.
    fn analyze_initializer_value(
        &mut self,
        target_ty: Type,
        initializer: &Initializer,
        context: &str,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<AnalyzedInitializer> {
        if target_ty.is_array() || target_ty.is_struct() || target_ty.is_union() {
            let assignments =
                self.analyze_recursive_aggregate_initializer(target_ty, initializer, context, diagnostics)?;
            return Some(AnalyzedInitializer::Aggregate(AggregateInitPlan {
                size: target_ty.byte_width(),
                assignments,
            }));
        }

        let value = self.analyze_scalar_initializer_expr(initializer, target_ty, context, diagnostics)?;
        Some(AnalyzedInitializer::Scalar(value))
    }

    /// Analyzes one scalar initializer element and applies implicit coercion rules.
    fn analyze_scalar_initializer_expr(
        &mut self,
        initializer: &Initializer,
        target_ty: Type,
        context: &str,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        match initializer {
            Initializer::Expr(expr) => {
                let value = self.analyze_expr(expr, diagnostics)?;
                Some(self.coerce_expr(value, target_ty, diagnostics, context, true))
            }
            Initializer::List(items, span) => {
                if items.len() != 1 {
                    diagnostics.error(
                        "semantic",
                        Some(*span),
                        "scalar initializer lists may contain only one element",
                        Some("use a single scalar value or an aggregate target type".to_string()),
                    );
                    return None;
                }
                if items[0].designator.is_some() {
                    diagnostics.error(
                        "semantic",
                        Some(*span),
                        "designated initializers require an array, struct, or union target",
                        None,
                    );
                    return None;
                }
                self.analyze_scalar_initializer_expr(&items[0].initializer, target_ty, context, diagnostics)
            }
        }
    }

    /// Analyzes one global initializer and materializes scalar or aggregate startup representation.
    fn analyze_global_initializer(
        &mut self,
        target_ty: Type,
        initializer: &Initializer,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedGlobalInitializer> {
        match self.analyze_initializer_value(target_ty, initializer, "global initializer", diagnostics)? {
            AnalyzedInitializer::Scalar(expr) => {
                if target_ty.is_pointer() {
                    if self.is_null_pointer_constant(&expr) {
                        return Some(TypedGlobalInitializer::Scalar(expr));
                    }
                    if let Some((symbol, offset)) = self.extract_constant_address(&expr) {
                        return Some(TypedGlobalInitializer::Address { symbol, offset });
                    }
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        "global pointer initializer must be a null pointer constant or one static data address",
                        Some("use `0`, `&global`, a decayed static array, or a string literal".to_string()),
                    );
                    return None;
                }
                if !is_constant_expression(&expr) {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        "global initializer must be a constant expression",
                        Some("use literals, casts, and integer operators only".to_string()),
                    );
                    return None;
                }
                Some(TypedGlobalInitializer::Scalar(expr))
            }
            AnalyzedInitializer::Aggregate(plan) => {
                let mut bytes = vec![0u8; target_ty.byte_width()];
                let mut valid = true;
                for assignment in plan.assignments {
                    let Some(value) = eval_integer_constant_expr(&assignment.value) else {
                        diagnostics.error(
                            "semantic",
                            Some(assignment.value.span),
                            "global aggregate initializer elements must be constant expressions",
                            None,
                        );
                        valid = false;
                        continue;
                    };
                    if let (Some(bit_offset), Some(bit_width)) = (assignment.bit_offset, assignment.bit_width) {
                        let unit_bytes = assignment.ty.byte_width();
                        if assignment.offset + unit_bytes > bytes.len() {
                            diagnostics.error(
                                "semantic",
                                Some(span),
                                "initializer element exceeded object size during layout",
                                None,
                            );
                            valid = false;
                            continue;
                        }
                        let mut current = 0u64;
                        for byte in 0..unit_bytes {
                            current |= u64::from(bytes[assignment.offset + byte]) << (8 * byte);
                        }
                        let raw_mask = if bit_width as usize >= assignment.ty.bit_width() {
                            assignment.ty.mask() as u64
                        } else {
                            (1u64 << bit_width) - 1
                        };
                        let shifted_mask = raw_mask << bit_offset;
                        let field_value =
                            (normalize_value(value, assignment.ty) as u64 & raw_mask) << bit_offset;
                        current = (current & !shifted_mask) | field_value;
                        for byte in 0..unit_bytes {
                            bytes[assignment.offset + byte] = ((current >> (8 * byte)) & 0xFF) as u8;
                        }
                        continue;
                    }

                    let value = normalize_value(value, assignment.ty) as u64;
                    for byte in 0..assignment.ty.byte_width() {
                        let index = assignment.offset + byte;
                        if index >= bytes.len() {
                            diagnostics.error(
                                "semantic",
                                Some(span),
                                "initializer element exceeded object size during layout",
                                None,
                            );
                            valid = false;
                            break;
                        }
                        bytes[index] = ((value >> (8 * byte)) & 0xFF) as u8;
                    }
                }
                if valid {
                    Some(TypedGlobalInitializer::Bytes(bytes))
                } else {
                    None
                }
            }
        }
    }

    /// Resolves omitted array sizes from supported initializers before symbol layout is fixed.
    fn resolve_object_decl_type(
        &mut self,
        ty: Type,
        initializer: Option<&Initializer>,
        span: Span,
        name: &str,
        context: &str,
        diagnostics: &mut DiagnosticBag,
    ) -> Type {
        let mut ty = ty;
        if ty.is_incomplete_array() {
            let inferred_len = match initializer {
                Some(Initializer::List(items, _)) => {
                    if items.is_empty() {
                        diagnostics.error(
                            "semantic",
                            Some(span),
                            format!("{context} `{name}` cannot infer an array size from an empty initializer"),
                            Some("spell an explicit array length or provide at least one initializer element".to_string()),
                        );
                        1
                    } else {
                        self.infer_array_initializer_len(items, span, diagnostics)
                    }
                }
                Some(Initializer::Expr(Expr {
                    kind: ExprKind::StringLiteral(bytes),
                    ..
                })) => bytes.len(),
                Some(_) => {
                    diagnostics.error(
                        "semantic",
                        Some(span),
                        format!("{context} `{name}` may omit the array size only with a brace initializer list or string literal"),
                        None,
                    );
                    1
                }
                None => {
                    diagnostics.error(
                        "semantic",
                        Some(span),
                        format!("{context} `{name}` uses an incomplete array type without an initializer"),
                        Some("spell an explicit array length or add an initializer".to_string()),
                    );
                    1
                }
            };
            ty = ty.array_of(inferred_len);
        }
        ty
    }

    /// Recursively flattens one array/struct/union initializer into scalar-slot assignments.
    fn analyze_recursive_aggregate_initializer(
        &mut self,
        target_ty: Type,
        initializer: &Initializer,
        context: &str,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<Vec<AggregateAssignment>> {
        let mut assignments = Vec::new();
        let mut init_context = AggregateInitContext {
            mode: context,
            assignments: &mut assignments,
            diagnostics,
        };
        if self.apply_initializer_to_object(target_ty, initializer, 0, &mut init_context) {
            Some(assignments)
        } else {
            None
        }
    }

    /// Applies one initializer recursively to a target object, producing explicit overlay writes.
    fn apply_initializer_to_object(
        &mut self,
        target_ty: Type,
        initializer: &Initializer,
        base_offset: usize,
        init_context: &mut AggregateInitContext<'_>,
    ) -> bool {
        if target_ty.is_array() {
            if let Initializer::Expr(Expr {
                kind: ExprKind::StringLiteral(bytes),
                ..
            }) = initializer
            {
                let Some(string_assignments) = self.analyze_string_array_initializer(
                    target_ty,
                    bytes,
                    initializer_span(initializer),
                    init_context.diagnostics,
                ) else {
                    return false;
                };
                let mut valid = true;
                for assignment in string_assignments {
                    valid &= self.push_scalar_initializer(
                        base_offset + assignment.offset,
                        assignment.ty,
                        assignment.value,
                        init_context.assignments,
                        init_context.diagnostics,
                    );
                }
                return valid;
            }

            let Initializer::List(items, span) = initializer else {
                init_context.diagnostics.error(
                    "semantic",
                    Some(initializer_span(initializer)),
                    format!(
                        "{} for array type requires a brace initializer list or string literal",
                        init_context.mode
                    ),
                    None,
                );
                return false;
            };
            return self.apply_array_initializer(
                target_ty,
                items,
                *span,
                base_offset,
                init_context,
            );
        }

        if target_ty.is_struct() {
            let Initializer::List(items, span) = initializer else {
                init_context.diagnostics.error(
                    "semantic",
                    Some(initializer_span(initializer)),
                    format!("{} for struct type requires a brace initializer list", init_context.mode),
                    None,
                );
                return false;
            };
            return self.apply_struct_initializer(
                target_ty,
                items,
                *span,
                base_offset,
                init_context,
            );
        }

        if target_ty.is_union() {
            let Initializer::List(items, span) = initializer else {
                init_context.diagnostics.error(
                    "semantic",
                    Some(initializer_span(initializer)),
                    format!("{} for union type requires a brace initializer list", init_context.mode),
                    None,
                );
                return false;
            };
            return self.apply_union_initializer(
                target_ty,
                items,
                *span,
                base_offset,
                init_context,
            );
        }

        let Some(value) = self.analyze_scalar_initializer_expr(
            initializer,
            target_ty,
            init_context.mode,
            init_context.diagnostics,
        ) else {
            return false;
        };
        self.push_scalar_initializer(
            base_offset,
            target_ty,
            value,
            init_context.assignments,
            init_context.diagnostics,
        )
    }

    /// Applies a one-dimensional array initializer, including array designators and nested aggregate elements.
    fn apply_array_initializer(
        &mut self,
        target_ty: Type,
        items: &[InitializerEntry],
        span: Span,
        base_offset: usize,
        init_context: &mut AggregateInitContext<'_>,
    ) -> bool {
        let len = target_ty.array_len.unwrap_or(0);
        let element_ty = target_ty.element_type();
        let stride = element_ty.byte_width();
        let mut next_index = 0usize;
        let mut seen = BTreeSet::new();
        let mut valid = true;

        for entry in items {
            let index = match &entry.designator {
                Some(Designator::Index(expr, designator_span)) => {
                    let Some(index) =
                        self.evaluate_array_designator_index(
                            expr,
                            len,
                            *designator_span,
                            init_context.diagnostics,
                        )
                    else {
                        valid = false;
                        continue;
                    };
                    next_index = index.saturating_add(1);
                    index
                }
                Some(Designator::Field(field, designator_span)) => {
                    init_context.diagnostics.error(
                        "semantic",
                        Some(*designator_span),
                        format!("array initializer does not accept field designator `.{field}`"),
                        None,
                    );
                    valid = false;
                    continue;
                }
                None => {
                    if next_index >= len {
                        init_context.diagnostics.error(
                            "semantic",
                            Some(span),
                            "too many initializer elements for array",
                            None,
                        );
                        valid = false;
                        continue;
                    }
                    let index = next_index;
                    next_index += 1;
                    index
                }
            };

            if !seen.insert(index) {
                init_context.diagnostics.error(
                    "semantic",
                    Some(initializer_entry_span(entry)),
                    format!("duplicate array initializer for index [{index}]"),
                    None,
                );
                valid = false;
                continue;
            }

            valid &= self.apply_initializer_to_object(
                element_ty,
                &entry.initializer,
                base_offset + index * stride,
                init_context,
            );
        }

        valid
    }

    /// Applies a struct initializer, including field designators and nested aggregate fields.
    fn apply_struct_initializer(
        &mut self,
        target_ty: Type,
        items: &[InitializerEntry],
        span: Span,
        base_offset: usize,
        init_context: &mut AggregateInitContext<'_>,
    ) -> bool {
        let Some(struct_id) = target_ty.struct_id else {
            init_context.diagnostics.error(
                "semantic",
                Some(span),
                "unknown struct layout during initializer analysis",
                None,
            );
            return false;
        };
        let Some(def) = self.struct_defs.get(struct_id) else {
            init_context.diagnostics.error(
                "semantic",
                Some(span),
                "unknown struct layout during initializer analysis",
                None,
            );
            return false;
        };
        let fields = def.fields.clone();
        let mut next_index = 0usize;
        let mut seen = BTreeSet::new();
        let mut valid = true;

        for entry in items {
            let field_index = match &entry.designator {
                Some(Designator::Field(field_name, designator_span)) => {
                    let Some(index) = fields.iter().position(|field| field.name == *field_name) else {
                        init_context.diagnostics.error(
                            "semantic",
                            Some(*designator_span),
                            format!("unknown designated field `.{field_name}`"),
                            None,
                        );
                        valid = false;
                        continue;
                    };
                    next_index = index.saturating_add(1);
                    index
                }
                Some(Designator::Index(_, designator_span)) => {
                    init_context.diagnostics.error(
                        "semantic",
                        Some(*designator_span),
                        "struct initializer does not accept array index designators",
                        None,
                    );
                    valid = false;
                    continue;
                }
                None => {
                    if next_index >= fields.len() {
                        init_context.diagnostics.error(
                            "semantic",
                            Some(span),
                            "too many initializer elements for struct",
                            None,
                        );
                        valid = false;
                        continue;
                    }
                    let index = next_index;
                    next_index += 1;
                    index
                }
            };

            let field = &fields[field_index];
            if !seen.insert(field_index) {
                init_context.diagnostics.error(
                    "semantic",
                    Some(initializer_entry_span(entry)),
                    format!("duplicate initializer for field `.{}`", field.name),
                    None,
                );
                valid = false;
                continue;
            }

            valid &= if let Some(bit_width) = field.bit_width {
                self.apply_bitfield_initializer(
                    field.ty,
                    field.bit_offset,
                    bit_width,
                    &entry.initializer,
                    base_offset + field.offset,
                    init_context,
                )
            } else {
                self.apply_initializer_to_object(
                    field.ty,
                    &entry.initializer,
                    base_offset + field.offset,
                    init_context,
                )
            };
        }

        valid
    }

    /// Applies a union initializer, selecting one field and overlaying only that field's bytes.
    fn apply_union_initializer(
        &mut self,
        target_ty: Type,
        items: &[InitializerEntry],
        span: Span,
        base_offset: usize,
        init_context: &mut AggregateInitContext<'_>,
    ) -> bool {
        let Some(union_id) = target_ty.union_id else {
            init_context.diagnostics.error(
                "semantic",
                Some(span),
                "unknown union layout during initializer analysis",
                None,
            );
            return false;
        };
        let Some(def) = self.union_defs.get(union_id) else {
            init_context.diagnostics.error(
                "semantic",
                Some(span),
                "unknown union layout during initializer analysis",
                None,
            );
            return false;
        };
        let fields = def.fields.clone();
        if items.is_empty() {
            return true;
        }
        if items.len() > 1 {
            init_context.diagnostics.error(
                "semantic",
                Some(span),
                "too many initializer elements for union",
                None,
            );
            return false;
        }

        let entry = &items[0];
        let Some(field) = (match &entry.designator {
            Some(Designator::Field(field_name, designator_span)) => fields
                .iter()
                .find(|field| field.name == *field_name)
                .cloned()
                .or_else(|| {
                    init_context.diagnostics.error(
                        "semantic",
                        Some(*designator_span),
                        format!("unknown designated union field `.{field_name}`"),
                        None,
                    );
                    None
                }),
            Some(Designator::Index(_, designator_span)) => {
                init_context.diagnostics.error(
                    "semantic",
                    Some(*designator_span),
                    "union initializer does not accept array index designators",
                    None,
                );
                None
            }
            None => fields.first().cloned(),
        }) else {
            return false;
        };

        if let Some(bit_width) = field.bit_width {
            self.apply_bitfield_initializer(
                field.ty,
                field.bit_offset,
                bit_width,
                &entry.initializer,
                base_offset + field.offset,
                init_context,
            )
        } else {
            self.apply_initializer_to_object(
                field.ty,
                &entry.initializer,
                base_offset + field.offset,
                init_context,
            )
        }
    }

    /// Applies one scalar initializer onto one bitfield overlay slot.
    fn apply_bitfield_initializer(
        &mut self,
        storage_ty: Type,
        bit_offset: u8,
        bit_width: u8,
        initializer: &Initializer,
        base_offset: usize,
        init_context: &mut AggregateInitContext<'_>,
    ) -> bool {
        let Some(value) = self.analyze_scalar_initializer_expr(
            initializer,
            storage_ty,
            init_context.mode,
            init_context.diagnostics,
        ) else {
            return false;
        };
        self.push_bitfield_initializer(
            BitfieldLocation {
                offset: base_offset,
                bit_offset,
                bit_width,
            },
            storage_ty,
            value,
            init_context.assignments,
            init_context.diagnostics,
        )
    }

    /// Resolves and checks one array designator index against a concrete element count.
    fn evaluate_array_designator_index(
        &mut self,
        expr: &Expr,
        len: usize,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<usize> {
        let typed = self.analyze_expr(expr, diagnostics)?;
        if !typed.ty.is_integer() {
            diagnostics.error(
                "semantic",
                Some(typed.span),
                "array designator index must be an integer constant expression",
                None,
            );
            return None;
        }
        if !is_constant_expression(&typed) {
            diagnostics.error(
                "semantic",
                Some(typed.span),
                "array designator index must be a constant expression",
                None,
            );
            return None;
        }
        let Some(value) = signed_or_unsigned_constant_value(&typed) else {
            diagnostics.error(
                "semantic",
                Some(typed.span),
                "unsupported array designator expression",
                None,
            );
            return None;
        };
        if value < 0 {
            diagnostics.error(
                "semantic",
                Some(span),
                "array designator index must be non-negative",
                None,
            );
            return None;
        }
        let index = value as usize;
        if index >= len {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("array designator index [{index}] is out of range for {len}-element array"),
                None,
            );
            return None;
        }
        Some(index)
    }

    /// Infers an omitted top-level array length from positional and designated initializer entries.
    fn infer_array_initializer_len(
        &mut self,
        items: &[InitializerEntry],
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> usize {
        let mut next_index = 0usize;
        let mut max_len = 0usize;

        for entry in items {
            let index = match &entry.designator {
                Some(Designator::Index(expr, designator_span)) => {
                    let Some(index) =
                        self.evaluate_array_designator_index(expr, usize::MAX, *designator_span, diagnostics)
                    else {
                        continue;
                    };
                    next_index = index.saturating_add(1);
                    index
                }
                Some(Designator::Field(field, designator_span)) => {
                    diagnostics.error(
                        "semantic",
                        Some(*designator_span),
                        format!("array initializer does not accept field designator `.{field}`"),
                        None,
                    );
                    continue;
                }
                None => {
                    let index = next_index;
                    next_index += 1;
                    index
                }
            };
            max_len = max_len.max(index.saturating_add(1));
        }

        if max_len == 0 {
            diagnostics.error(
                "semantic",
                Some(span),
                "cannot infer an array size from an empty initializer",
                Some("spell an explicit array length or provide at least one initializer element".to_string()),
            );
            1
        } else {
            max_len
        }
    }

    /// Stores one scalar initializer value as one explicit aggregate overlay write.
    fn push_scalar_initializer(
        &mut self,
        offset: usize,
        target_ty: Type,
        value: TypedExpr,
        assignments: &mut Vec<AggregateAssignment>,
        diagnostics: &mut DiagnosticBag,
    ) -> bool {
        let value = self.coerce_expr(value, target_ty, diagnostics, "initializer", true);
        assignments.push(AggregateAssignment {
            offset,
            ty: target_ty,
            value,
            bit_offset: None,
            bit_width: None,
        });
        true
    }

    /// Stores one scalar initializer value as one explicit bitfield overlay write.
    fn push_bitfield_initializer(
        &mut self,
        location: BitfieldLocation,
        target_ty: Type,
        value: TypedExpr,
        assignments: &mut Vec<AggregateAssignment>,
        diagnostics: &mut DiagnosticBag,
    ) -> bool {
        let value = self.coerce_expr(value, target_ty, diagnostics, "initializer", true);
        if let Some(raw) = signed_or_unsigned_constant_value(&value) {
            let mask = if location.bit_width as usize >= target_ty.bit_width() {
                target_ty.mask()
            } else {
                (1_i64 << location.bit_width) - 1
            };
            if normalize_value(raw, target_ty) != (normalize_value(raw, target_ty) & mask) {
                diagnostics.warning(
                    "semantic",
                    Some(value.span),
                    format!(
                        "initializer value for {}-bit bitfield truncates to fit",
                        location.bit_width
                    ),
                    "W1501",
                );
            }
        }
        assignments.push(AggregateAssignment {
            offset: location.offset,
            ty: target_ty,
            value,
            bit_offset: Some(location.bit_offset),
            bit_width: Some(location.bit_width),
        });
        true
    }

    /// Converts one string literal initializer into byte-slot assignments with required null fit.
    fn analyze_string_array_initializer(
        &mut self,
        target_ty: Type,
        bytes: &[u8],
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<Vec<AggregateAssignment>> {
        let len = target_ty.array_len?;
        let element_ty = target_ty.element_type();
        if !matches!(element_ty.scalar, ScalarType::I8 | ScalarType::U8)
            || element_ty.is_pointer()
            || element_ty.is_array()
            || element_ty.is_struct()
            || element_ty.is_union()
        {
            diagnostics.error(
                "semantic",
                Some(span),
                "string literals may initialize only char or unsigned char arrays in phase 10",
                None,
            );
            return None;
        }
        if bytes.len() > len {
            diagnostics.error(
                "semantic",
                Some(span),
                format!(
                    "string initializer is too large for {}-element array (needs {} bytes including trailing null)",
                    len,
                    bytes.len()
                ),
                Some("increase the array size or shorten the string literal".to_string()),
            );
            return None;
        }

        let mut assignments = Vec::with_capacity(len);
        for index in 0..len {
            let value = bytes.get(index).copied().unwrap_or(0);
            assignments.push(AggregateAssignment {
                offset: index * element_ty.byte_width(),
                ty: element_ty,
                value: TypedExpr {
                    kind: TypedExprKind::IntLiteral(i64::from(value)),
                    ty: element_ty,
                    span,
                    value_category: ValueCategory::RValue,
                },
                bit_offset: None,
                bit_width: None,
            });
        }
        Some(assignments)
    }

    /// Creates one anonymous static RAM object that backs a source-level string literal.
    fn intern_string_literal(&mut self, bytes: &[u8], span: Span) -> SymbolId {
        let symbol = self.insert_symbol(Symbol {
            id: self.symbols.len(),
            name: format!("__strlit{}", self.string_literal_counter),
            ty: Type::new(ScalarType::I8).array_of(bytes.len()),
            storage_class: StorageClass::Static,
            is_interrupt: false,
            kind: SymbolKind::StringLiteral,
            span,
            fixed_address: None,
            is_defined: true,
            is_referenced: false,
            parameter_types: Vec::new(),
            enum_const_value: None,
        });
        self.string_literal_counter += 1;
        self.globals.push(TypedGlobal {
            symbol,
            initializer: Some(TypedGlobalInitializer::Bytes(bytes.to_vec())),
        });
        symbol
    }

    /// Returns true when an expression denotes a string literal pointer value.
    fn is_string_literal_pointer_expr(&self, expr: &TypedExpr) -> bool {
        match &expr.kind {
            TypedExprKind::ArrayDecay(value) => {
                matches!(value.kind, TypedExprKind::Symbol(symbol) if self.symbols[symbol].kind == SymbolKind::StringLiteral)
            }
            TypedExprKind::Cast { expr, .. } => self.is_string_literal_pointer_expr(expr),
            _ => false,
        }
    }

    /// Extracts one statically-known RAM symbol address plus byte offset from a pointer expression.
    fn extract_constant_address(&self, expr: &TypedExpr) -> Option<(SymbolId, usize)> {
        match &expr.kind {
            TypedExprKind::ArrayDecay(value) | TypedExprKind::AddressOf(value) => {
                self.extract_constant_lvalue_address(value)
            }
            TypedExprKind::Cast { expr, .. } if expr.ty.is_pointer() => {
                self.extract_constant_address(expr)
            }
            TypedExprKind::Binary { op, lhs, rhs } if expr.ty.is_pointer() => {
                let (symbol, base_offset) = self.extract_constant_address(lhs)?;
                if !rhs.ty.is_integer() || !is_constant_expression(rhs) {
                    return None;
                }
                let delta = signed_or_unsigned_constant_value(rhs)?;
                let adjusted = match op {
                    BinaryOp::Add => (base_offset as i64).checked_add(delta)?,
                    BinaryOp::Sub => (base_offset as i64).checked_sub(delta)?,
                    _ => return None,
                };
                usize::try_from(adjusted).ok().map(|offset| (symbol, offset))
            }
            _ => None,
        }
    }

    /// Extracts one statically-known address from a typed lvalue.
    fn extract_constant_lvalue_address(&self, expr: &TypedExpr) -> Option<(SymbolId, usize)> {
        match &expr.kind {
            TypedExprKind::Symbol(symbol) => Some((*symbol, 0)),
            TypedExprKind::Deref(pointer) => self.extract_constant_address(pointer),
            _ => None,
        }
    }

    /// Returns true when one pointer conversion is implicitly allowed in the current subset.
    fn classify_pointer_conversion(
        &self,
        src_ty: Type,
        target_ty: Type,
    ) -> Result<(), PointerConversionError> {
        let src_ty = src_ty.without_object_qualifiers();
        let target_ty = target_ty.without_object_qualifiers();
        if src_ty == target_ty {
            return Ok(());
        }
        if !src_ty.same_pointer_shape(target_ty) {
            return Err(PointerConversionError::Incompatible);
        }
        if src_ty.pointer_depth == 1 && target_ty.pointer_depth == 1 {
            let src_pointee = src_ty.element_type();
            let target_pointee = target_ty.element_type();
            if qualifiers_include(target_pointee.qualifiers, src_pointee.qualifiers) {
                return Ok(());
            }
            return Err(PointerConversionError::QualifierDiscard);
        }
        Err(PointerConversionError::NestedQualifierMismatch)
    }

    /// Returns true when two pointers may be compared or subtracted as raw data addresses.
    fn are_compatible_pointer_compare_types(&self, lhs_ty: Type, rhs_ty: Type) -> bool {
        lhs_ty.same_pointer_shape(rhs_ty)
    }

    /// Builds an lvalue that refers to one scalar slot inside a declared aggregate object.
    fn build_symbol_offset_lvalue(
        &self,
        symbol: SymbolId,
        object_ty: Type,
        offset: usize,
        field_ty: Type,
        span: Span,
    ) -> TypedExpr {
        let base = TypedExpr {
            kind: TypedExprKind::Symbol(symbol),
            ty: object_ty,
            span,
            value_category: ValueCategory::LValue,
        };
        self.build_member_lvalue(base, false, offset, field_ty, span)
    }

    /// Returns one struct-field descriptor by id/name.
    fn struct_field(&self, struct_id: StructId, field: &str) -> Option<super::ast::StructField> {
        let def = self.struct_defs.get(struct_id)?;
        def.fields.iter().find(|entry| entry.name == field).cloned()
    }

    /// Returns one union-field descriptor by id/name.
    fn union_field(&self, union_id: UnionId, field: &str) -> Option<super::ast::StructField> {
        let def = self.union_defs.get(union_id)?;
        def.fields.iter().find(|entry| entry.name == field).cloned()
    }

    /// Returns one aggregate field descriptor by type/name.
    fn aggregate_field(&self, aggregate_ty: Type, field: &str) -> Option<super::ast::StructField> {
        if let Some(struct_id) = aggregate_ty.struct_id {
            return self.struct_field(struct_id, field);
        }
        if let Some(union_id) = aggregate_ty.union_id {
            return self.union_field(union_id, field);
        }
        None
    }

    /// Returns the switch-expression type for the innermost active switch.
    fn current_switch_type(&self) -> Option<Type> {
        self.switch_stack.last().map(|context| context.expr_ty)
    }

    /// Returns true when the innermost active switch permits direct case/default labels here.
    fn current_switch_labels_allowed(&self) -> bool {
        self.switch_label_modes.last().copied().unwrap_or(false)
    }

    /// Returns the first default-label span already registered for the active switch.
    fn current_switch_default_span(&self) -> Option<Span> {
        self.switch_stack.last().and_then(|context| context.default_span)
    }

    /// Analyzes one nested statement while temporarily forbidding case/default labels here.
    fn analyze_stmt_with_case_labels_disabled(
        &mut self,
        stmt: &Stmt,
        diagnostics: &mut DiagnosticBag,
    ) -> TypedStmt {
        let disable = !self.switch_stack.is_empty() && self.current_switch_labels_allowed();
        if disable {
            self.switch_label_modes.push(false);
        }
        let typed = self.analyze_stmt(stmt, diagnostics);
        if disable {
            self.switch_label_modes.pop();
        }
        typed
    }

    /// Validates one case-label expression and records its normalized value for duplicate checks.
    fn validate_case_value(
        &mut self,
        typed_value: Option<TypedExpr>,
        switch_ty: Type,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<i64> {
        let value = typed_value?;
        if !value.ty.is_integer() {
            diagnostics.error(
                "semantic",
                Some(value.span),
                "case label must use an integer constant expression",
                None,
            );
            return None;
        }
        if !is_constant_expression(&value) {
            diagnostics.error(
                "semantic",
                Some(value.span),
                "case label must be a constant expression",
                None,
            );
            return None;
        }
        if !is_representable_integer_constant(&value, switch_ty) {
            diagnostics.error(
                "semantic",
                Some(value.span),
                format!("case label value is not representable in switch type `{switch_ty}`"),
                None,
            );
            return None;
        }

        let canonical = signed_or_unsigned_constant_value(&value)
            .expect("representable case constant remains evaluable");
        let normalized = normalize_value(canonical, switch_ty);
        if let Some(previous) = self
            .switch_stack
            .last_mut()
            .expect("switch context")
            .case_values
            .insert(normalized, span)
        {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("duplicate case value `{canonical}` in one switch"),
                Some(format!("previous matching case label starts at byte {}", previous.start)),
            );
        }
        Some(normalized)
    }

    /// Returns true when analyzing statements inside the active interrupt handler.
    fn current_function_is_interrupt(&self) -> bool {
        self.current_function
            .is_some_and(|symbol| self.symbols[symbol].is_interrupt)
    }

    /// Returns true when an integer expression is a compile-time zero constant.
    fn is_integer_zero_constant_expr(&self, expr: &TypedExpr) -> bool {
        expr.ty.is_integer()
            && eval_integer_constant_expr(expr)
                .is_some_and(|value| normalize_value(value, expr.ty) == 0)
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
        let supported = ty.is_supported_object_type() || ty.is_supported_value_type() || ty.is_rom();
        if !ty.has_size() || !supported {
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

    /// Analyzes integer shifts with result type tied to the left operand in Phase 5.
    fn analyze_shift_expr(
        &mut self,
        op: BinaryOp,
        lhs: TypedExpr,
        rhs: TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        if !lhs.ty.is_integer() || !rhs.ty.is_integer() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("`{op:?}` requires integer operands"),
                Some("shift operators do not support pointers".to_string()),
            );
            return None;
        }

        let rhs = self.coerce_expr(rhs, lhs.ty, diagnostics, "shift count", false);
        self.diagnose_shift_rhs(op, lhs.ty, &rhs, diagnostics);
        if op == BinaryOp::ShiftRight && lhs.ty.is_signed() {
            diagnostics.extra_warning(
                "semantic",
                Some(span),
                format!("signed `{op:?}` uses arithmetic right shift in phase 5"),
                "W5003",
            );
        }

        Some(TypedExpr {
            kind: TypedExprKind::Binary {
                op,
                lhs: Box::new(lhs.clone()),
                rhs: Box::new(rhs),
            },
            ty: lhs.ty,
            span,
            value_category: ValueCategory::RValue,
        })
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

        let lhs_ty = lhs.ty.unqualified();
        let rhs_ty = rhs.ty.unqualified();

        if lhs_ty == rhs_ty {
            let lhs = self.coerce_expr(lhs, lhs_ty, diagnostics, "binary operand", false);
            let rhs = self.coerce_expr(rhs, rhs_ty, diagnostics, "binary operand", false);
            return (lhs, rhs, lhs_ty);
        }

        if matches!(lhs.kind, TypedExprKind::IntLiteral(_)) {
            let result_ty = rhs_ty;
            let lhs = self.coerce_expr(lhs, result_ty, diagnostics, "integer literal", false);
            let rhs = self.coerce_expr(rhs, result_ty, diagnostics, "binary operand", false);
            return (lhs, rhs, result_ty);
        }
        if matches!(rhs.kind, TypedExprKind::IntLiteral(_)) {
            let result_ty = lhs_ty;
            let lhs = self.coerce_expr(lhs, result_ty, diagnostics, "binary operand", false);
            let rhs = self.coerce_expr(rhs, result_ty, diagnostics, "integer literal", false);
            return (lhs, rhs, result_ty);
        }

        if lhs_ty.bit_width() != rhs_ty.bit_width() {
            let target_ty = if lhs_ty.bit_width() > rhs_ty.bit_width() {
                lhs_ty
            } else {
                rhs_ty
            };
            let lhs = self.coerce_expr(lhs, target_ty, diagnostics, "binary operand", false);
            let rhs = self.coerce_expr(rhs, target_ty, diagnostics, "binary operand", false);
            return (lhs, rhs, target_ty);
        }

        diagnostics.error(
            "semantic",
            Some(span),
            format!(
                "mixed signedness for `{op:?}` with equal-width operands is not supported in phase 5"
            ),
            Some("use matching signedness or add an explicit cast".to_string()),
        );
        let result_ty = lhs_ty;
        let lhs = self.coerce_expr(lhs, result_ty, diagnostics, "binary operand", false);
        let rhs = self.coerce_expr(rhs, result_ty, diagnostics, "binary operand", false);
        (lhs, rhs, result_ty)
    }

    /// Emits constant diagnostics for division or modulo by zero when statically provable.
    fn diagnose_division_rhs(
        &self,
        op: BinaryOp,
        rhs: &TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) {
        if !matches!(op, BinaryOp::Divide | BinaryOp::Modulo) {
            return;
        }
        if let Some(value) = eval_integer_constant_expr(rhs)
            && normalize_value(value, rhs.ty) == 0
        {
            diagnostics.error(
                "semantic",
                Some(span),
                match op {
                    BinaryOp::Divide => "division by constant zero",
                    BinaryOp::Modulo => "modulo by constant zero",
                    _ => unreachable!("division-like operator"),
                },
                Some("guard the divisor or change the constant expression".to_string()),
            );
        }
    }

    /// Emits constant diagnostics for unsupported shift counts in the current Phase 5 model.
    fn diagnose_shift_rhs(
        &self,
        op: BinaryOp,
        lhs_ty: Type,
        rhs: &TypedExpr,
        diagnostics: &mut DiagnosticBag,
    ) {
        let Some(value) = eval_integer_constant_expr(rhs) else {
            return;
        };

        let signed = signed_value(value, rhs.ty);
        if signed < 0 {
            diagnostics.error(
                "semantic",
                Some(rhs.span),
                format!("`{op:?}` shift count must be non-negative"),
                None,
            );
            return;
        }

        let count = normalize_value(value, rhs.ty) as usize;
        if count >= lhs_ty.bit_width() {
            diagnostics.error(
                "semantic",
                Some(rhs.span),
                format!(
                    "`{op:?}` constant shift count {count} exceeds {}-bit value width",
                    lhs_ty.bit_width()
                ),
                Some("use a smaller constant shift count or an explicit cast".to_string()),
            );
        }
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
            if self.are_compatible_pointer_compare_types(lhs.ty, rhs.ty) {
                return Some((lhs, rhs));
            }
            diagnostics.error(
                "semantic",
                Some(span),
                format!(
                    "pointer comparison requires compatible pointer types, got `{}` and `{}`",
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

    /// Builds one compatible pointer subtraction result using 16-bit inline arithmetic only.
    fn build_pointer_difference_expr(
        &mut self,
        lhs: TypedExpr,
        rhs: TypedExpr,
        span: Span,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<TypedExpr> {
        if !lhs.ty.is_pointer() || !rhs.ty.is_pointer() {
            diagnostics.error(
                "semantic",
                Some(span),
                "pointer subtraction requires pointer operands",
                None,
            );
            return None;
        }
        if !self.are_compatible_pointer_compare_types(lhs.ty, rhs.ty) {
            diagnostics.error(
                "semantic",
                Some(span),
                format!(
                    "pointer subtraction requires compatible pointer types, got `{}` and `{}`",
                    lhs.ty, rhs.ty
                ),
                None,
            );
            return None;
        }

        let element_ty = lhs.ty.element_type();
        let element_size = element_ty.byte_width();
        if !matches!(element_size, 1 | 2) {
            diagnostics.error(
                "semantic",
                Some(span),
                format!(
                    "pointer subtraction for element type `{}` is not supported in phase 12",
                    element_ty
                ),
                Some("use element sizes of 1 or 2 bytes only for pointer subtraction".to_string()),
            );
            return None;
        }

        let raw_ty = Type::new(ScalarType::U16);
        let signed_ty = Type::new(ScalarType::I16);
        let lhs = TypedExpr {
            kind: TypedExprKind::Cast {
                kind: CastKind::Bitcast,
                expr: Box::new(lhs),
            },
            ty: raw_ty,
            span,
            value_category: ValueCategory::RValue,
        };
        let rhs = TypedExpr {
            kind: TypedExprKind::Cast {
                kind: CastKind::Bitcast,
                expr: Box::new(rhs),
            },
            ty: raw_ty,
            span,
            value_category: ValueCategory::RValue,
        };

        let mut diff = TypedExpr {
            kind: TypedExprKind::Binary {
                op: BinaryOp::Sub,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty: raw_ty,
            span,
            value_category: ValueCategory::RValue,
        };

        if element_size == 2 {
            diff = TypedExpr {
                kind: TypedExprKind::Binary {
                    op: BinaryOp::ShiftRight,
                    lhs: Box::new(diff),
                    rhs: Box::new(TypedExpr {
                        kind: TypedExprKind::IntLiteral(1),
                        ty: raw_ty,
                        span,
                        value_category: ValueCategory::RValue,
                    }),
                },
                ty: raw_ty,
                span,
                value_category: ValueCategory::RValue,
            };
        }

        Some(TypedExpr {
            kind: TypedExprKind::Cast {
                kind: CastKind::Bitcast,
                expr: Box::new(diff),
            },
            ty: signed_ty,
            span,
            value_category: ValueCategory::RValue,
        })
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
            if expr.ty.is_pointer() {
                let span = expr.span;
                match self.classify_pointer_conversion(expr.ty, target_ty) {
                    Ok(()) => {
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
                    Err(PointerConversionError::QualifierDiscard) => {
                        diagnostics.error(
                            "semantic",
                            Some(span),
                            format!(
                                "discarding qualifiers when converting `{}` to `{}` in {context} is not allowed",
                                expr.ty, target_ty
                            ),
                            None,
                        );
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
                    Err(PointerConversionError::NestedQualifierMismatch) => {
                        diagnostics.error(
                            "semantic",
                            Some(span),
                            format!(
                                "nested pointer qualifiers must match exactly when converting `{}` to `{}` in {context}",
                                expr.ty, target_ty
                            ),
                            Some("add an explicit cast only if you intend a raw address reinterpretation".to_string()),
                        );
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
                    Err(PointerConversionError::Incompatible) => {
                        if self.is_string_literal_pointer_expr(&expr) {
                            diagnostics.error(
                                "semantic",
                                Some(span),
                                format!(
                                    "string literal is incompatible with pointer target type `{}` in {context}",
                                    target_ty
                                ),
                                Some(
                                    "initialize `char*` or `const char*` data pointers with string literals in phase 12"
                                        .to_string(),
                                ),
                            );
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
                    }
                }
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
                Some("use matching data-space pointer types or literal zero".to_string()),
            );
            return expr;
        }
        if expr.ty.is_pointer() && self.is_string_literal_pointer_expr(&expr) {
            diagnostics.error(
                "semantic",
                Some(expr.span),
                format!("string literal is incompatible with target type `{}` in {context}", target_ty),
                Some("initialize a matching data-space pointer or a char array instead".to_string()),
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

        if warn_on_truncate
            && expr.ty.bit_width() > target_ty.bit_width()
            && !is_representable_integer_constant(&expr, target_ty)
        {
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
            && !expr.ty.object_is_const()
            && matches!(
                expr.kind,
                TypedExprKind::Symbol(_) | TypedExprKind::Deref(_) | TypedExprKind::BitField { .. }
            )
    }

    /// Validates one return type against the constrained Phase 3 ABI.
    fn validate_return_type(
        &mut self,
        ty: Type,
        span: Span,
        name: &str,
        diagnostics: &mut DiagnosticBag,
    ) {
        self.validate_const_placement(
            ty,
            span,
            &format!("function `{name}`"),
            diagnostics,
        );
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

    /// Validates the fixed Phase 6 ISR signature for `void __interrupt handler(void)`.
    fn validate_interrupt_signature(
        &mut self,
        function: &FunctionDecl,
        diagnostics: &mut DiagnosticBag,
    ) {
        if !function.is_interrupt {
            return;
        }

        if !function.return_type.is_void() {
            diagnostics.error(
                "semantic",
                Some(function.span),
                format!(
                    "interrupt handler `{}` must return `void` in phase 6",
                    function.name
                ),
                Some("declare it as `void __interrupt isr(void)`".to_string()),
            );
        }

        if !function.params.is_empty() {
            diagnostics.error(
                "semantic",
                Some(function.span),
                format!(
                    "interrupt handler `{}` cannot take parameters in phase 6",
                    function.name
                ),
                Some("remove the parameters and use `void`".to_string()),
            );
        }

        if let Some(existing) = self.interrupt_handler()
            && self.symbols[existing].name != function.name
        {
            diagnostics.error(
                "semantic",
                Some(function.span),
                format!(
                    "phase 6 supports only one interrupt handler; already declared `{}`",
                    self.symbols[existing].name
                ),
                Some("keep a single `__interrupt` function in this program".to_string()),
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
        self.validate_const_placement(ty, span, &format!("parameter `{name}`"), diagnostics);
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
        self.validate_const_placement(ty, span, &format!("{context} `{name}`"), diagnostics);
        if ty.is_rom() {
            if context != "global" {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    format!("{context} `{name}` cannot use `__rom` storage in phase 14"),
                    Some("keep ROM objects at file scope only".to_string()),
                );
                return;
            }
            if !ty.object_is_const() {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    format!("program-memory object `{name}` must be declared `const`"),
                    Some("use `const __rom ...` for ROM tables and strings".to_string()),
                );
                return;
            }
            if !ty.is_array() {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    format!("program-memory object `{name}` uses unsupported type `{ty}`"),
                    Some("phase 14 supports only ROM arrays of 8-bit or 16-bit integers".to_string()),
                );
                return;
            }
            let element_ty = ty.element_type();
            if !matches!(
                element_ty.scalar,
                ScalarType::I8 | ScalarType::U8 | ScalarType::I16 | ScalarType::U16
            )
                || element_ty.pointer_depth != 0
                || element_ty.array_len.is_some()
                || element_ty.struct_id.is_some()
            {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    format!("program-memory object `{name}` uses unsupported ROM element type `{element_ty}`"),
                    Some(
                        "use `const __rom char[]`, `const __rom unsigned char[]`, `const __rom int[]`, or `const __rom unsigned int[]`"
                            .to_string(),
                    ),
                );
            }
            let rom_bytes = ty.byte_width();
            if rom_bytes > 255 {
                diagnostics.error(
                    "semantic",
                    Some(span),
                    format!("program-memory object `{name}` is too large for one phase 14 RETLW table page ({rom_bytes} bytes)"),
                    Some("keep each ROM object at 255 data bytes or fewer".to_string()),
                );
            }
            return;
        }
        if ty.is_array() && ty.element_type().is_array() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("{context} `{name}` uses unsupported multidimensional array type `{ty}`"),
                Some("phase 15 still supports one-dimensional arrays only".to_string()),
            );
            return;
        }
        if ty.is_void() || !ty.is_supported_object_type() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("{context} `{name}` uses unsupported type `{ty}`"),
                Some(
                    "phase 15 supports scalar objects, one-dimensional arrays, and packed struct/union aggregates"
                        .to_string(),
                ),
            );
        }
    }

    /// Accepts Phase 12 const placement now that pointer/object qualifiers are represented directly.
    fn validate_const_placement(
        &self,
        ty: Type,
        span: Span,
        context: &str,
        diagnostics: &mut DiagnosticBag,
    ) {
        if ty.is_pointer() && ty.element_type().is_rom() {
            diagnostics.error(
                "semantic",
                Some(span),
                format!("{context} uses unsupported ROM pointer type `{ty}`"),
                Some("phase 14 supports direct ROM indexing plus `__rom_read8()` / `__rom_read16()`, not ROM pointers".to_string()),
            );
        }
    }

    /// Checks struct and union fields for aggregate, ROM, and bitfield restrictions.
    fn validate_aggregate_field_types(&mut self, diagnostics: &mut DiagnosticBag) {
        for def in &self.struct_defs {
            for field in &def.fields {
                self.validate_one_aggregate_field(
                    "struct",
                    &def.name,
                    Some(def.id),
                    None,
                    field,
                    diagnostics,
                );
            }
        }
        for def in &self.union_defs {
            for field in &def.fields {
                self.validate_one_aggregate_field(
                    "union",
                    &def.name,
                    None,
                    Some(def.id),
                    field,
                    diagnostics,
                );
            }
        }
    }

    /// Checks one aggregate field against the supported Phase 15 object model.
    fn validate_one_aggregate_field(
        &self,
        aggregate_kind: &str,
        aggregate_name: &str,
        self_struct_id: Option<StructId>,
        self_union_id: Option<UnionId>,
        field: &super::ast::StructField,
        diagnostics: &mut DiagnosticBag,
    ) {
        self.validate_const_placement(
            field.ty,
            field.span,
            &format!("{aggregate_kind} `{aggregate_name}` field `{}`", field.name),
            diagnostics,
        );

        let field_ty = field.ty;
        let element_ty = if field_ty.is_array() {
            Some(field_ty.element_type())
        } else {
            None
        };

        if field_ty.is_rom() {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` cannot use program-memory type `{}` in phase 15",
                    field.name, field_ty
                ),
                Some("keep `__rom` objects at file scope only".to_string()),
            );
            return;
        }

        if let Some(bit_width) = field.bit_width {
            if !matches!(field_ty.scalar, ScalarType::U8 | ScalarType::U16)
                || field_ty.is_pointer()
                || field_ty.is_array()
                || field_ty.is_struct()
                || field_ty.is_union()
            {
                diagnostics.error(
                    "semantic",
                    Some(field.span),
                    format!(
                        "{aggregate_kind} `{aggregate_name}` bitfield `{}` must use `unsigned char` or `unsigned int` in phase 15",
                        field.name
                    ),
                    None,
                );
                return;
            }
            if field.bit_offset as usize + bit_width as usize > field_ty.bit_width() {
                diagnostics.error(
                    "semantic",
                    Some(field.span),
                    format!(
                        "{aggregate_kind} `{aggregate_name}` bitfield `{}` exceeds its storage unit",
                        field.name
                    ),
                    None,
                );
                return;
            }
        }

        if field_ty.is_void() {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` uses unsupported type `void`",
                    field.name
                ),
                None,
            );
            return;
        }

        if field_ty.is_incomplete_array() {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` cannot use an incomplete array type",
                    field.name
                ),
                Some(format!("spell an explicit array length for {aggregate_kind} fields")),
            );
            return;
        }

        if field_ty.is_array() && element_ty.is_some_and(Type::is_array) {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` uses unsupported multidimensional array type `{}`",
                    field.name, field_ty
                ),
                Some("phase 15 still supports one-dimensional arrays only".to_string()),
            );
            return;
        }

        if field_ty.is_struct() && field_ty.struct_id == self_struct_id {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` cannot contain itself by value through field `{}`",
                    field.name
                ),
                Some("use a pointer field only if/when incomplete aggregate pointers are supported".to_string()),
            );
            return;
        }

        if field_ty.is_union() && field_ty.union_id == self_union_id {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` cannot contain itself by value through field `{}`",
                    field.name
                ),
                Some("use a pointer field only if/when incomplete aggregate pointers are supported".to_string()),
            );
            return;
        }

        if field_ty.is_struct() && field_ty.struct_size == 0 {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` uses incomplete struct type `{}`",
                    field.name, field_ty
                ),
                Some("define the nested struct before using it by value".to_string()),
            );
            return;
        }

        if field_ty.is_union() && field_ty.union_size == 0 {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` uses incomplete union type `{}`",
                    field.name, field_ty
                ),
                Some("define the nested union before using it by value".to_string()),
            );
            return;
        }

        if let Some(element_ty) = element_ty
            && ((element_ty.is_struct() && element_ty.struct_size == 0)
                || (element_ty.is_union() && element_ty.union_size == 0))
        {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` uses incomplete aggregate element type `{}`",
                    field.name, element_ty
                ),
                Some("define the aggregate before using it in an array field".to_string()),
            );
            return;
        }

        if field_ty.is_pointer()
            && (field_ty.element_type().has_struct_base() || field_ty.element_type().has_union_base())
            && !field_ty.element_type().has_size()
        {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` uses unsupported pointer to incomplete aggregate type `{}`",
                    field.name, field_ty
                ),
                Some("phase 15 does not model incomplete aggregate pointers yet".to_string()),
            );
            return;
        }

        let supported_field = field_ty.is_supported_value_type()
            || field_ty.is_struct()
            || field_ty.is_union()
            || (field_ty.is_array()
                && element_ty.is_some_and(|element| {
                    element.is_supported_value_type() || element.is_struct() || element.is_union()
                }));
        if !supported_field {
            diagnostics.error(
                "semantic",
                Some(field.span),
                format!(
                    "{aggregate_kind} `{aggregate_name}` field `{}` uses unsupported type `{}`",
                    field.name, field_ty
                ),
                None,
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

    /// Ensures the single Phase 6 ISR, when declared, is fully defined before codegen.
    fn validate_interrupt_handlers(&self, diagnostics: &mut DiagnosticBag) {
        if let Some(symbol) = self.interrupt_handler()
            && !self.has_body(symbol)
        {
            diagnostics.error(
                "semantic",
                Some(self.symbols[symbol].span),
                format!(
                    "interrupt handler `{}` must be defined in phase 6",
                    self.symbols[symbol].name
                ),
                None,
            );
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
            TypedStmt::Switch { expr, body, .. } => {
                let _ = self.track_stack_pointer_expr(expr, tainted_locals);
                self.walk_stmt_for_stack_pointer_returns(body, tainted_locals, diagnostics);
            }
            TypedStmt::Case { body, .. } | TypedStmt::Default { body, .. } => {
                self.walk_stmt_for_stack_pointer_returns(body, tainted_locals, diagnostics);
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
            TypedExprKind::BitField { storage, .. } => {
                let _ = self.track_stack_pointer_expr(storage, tainted_locals);
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
            TypedExprKind::StructAssign { target, value, .. } => {
                let _ = self.track_stack_pointer_expr(target, tainted_locals);
                let _ = self.track_stack_pointer_expr(value, tainted_locals);
                false
            }
            TypedExprKind::RomRead8 { index, .. } | TypedExprKind::RomRead16 { index, .. } => {
                let _ = self.track_stack_pointer_expr(index, tainted_locals);
                false
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

    /// Rejects calls and runtime-helper-requiring expressions inside one Phase 6 ISR body.
    fn reject_interrupt_body(
        &self,
        function: SymbolId,
        body: &TypedStmt,
        diagnostics: &mut DiagnosticBag,
    ) {
        self.walk_interrupt_stmt(function, body, diagnostics);
    }

    /// Walks one typed statement tree while enforcing the conservative Phase 6 ISR subset.
    fn walk_interrupt_stmt(
        &self,
        function: SymbolId,
        stmt: &TypedStmt,
        diagnostics: &mut DiagnosticBag,
    ) {
        match stmt {
            TypedStmt::Block(statements, _) => {
                for statement in statements {
                    self.walk_interrupt_stmt(function, statement, diagnostics);
                }
            }
            TypedStmt::Switch { expr, body, .. } => {
                self.walk_interrupt_expr(function, expr, diagnostics);
                self.walk_interrupt_stmt(function, body, diagnostics);
            }
            TypedStmt::Case { body, .. } | TypedStmt::Default { body, .. } => {
                self.walk_interrupt_stmt(function, body, diagnostics);
            }
            TypedStmt::VarDecl(_, initializer, _)
            | TypedStmt::Return(initializer, _) => {
                if let Some(expr) = initializer {
                    self.walk_interrupt_expr(function, expr, diagnostics);
                }
            }
            TypedStmt::Expr(expr, _) => self.walk_interrupt_expr(function, expr, diagnostics),
            TypedStmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.walk_interrupt_expr(function, condition, diagnostics);
                self.walk_interrupt_stmt(function, then_branch, diagnostics);
                if let Some(else_branch) = else_branch {
                    self.walk_interrupt_stmt(function, else_branch, diagnostics);
                }
            }
            TypedStmt::While {
                condition, body, ..
            } => {
                self.walk_interrupt_expr(function, condition, diagnostics);
                self.walk_interrupt_stmt(function, body, diagnostics);
            }
            TypedStmt::DoWhile {
                body,
                condition,
                ..
            } => {
                self.walk_interrupt_stmt(function, body, diagnostics);
                self.walk_interrupt_expr(function, condition, diagnostics);
            }
            TypedStmt::For {
                init,
                condition,
                step,
                body,
                ..
            } => {
                if let Some(init) = init {
                    self.walk_interrupt_stmt(function, init, diagnostics);
                }
                if let Some(condition) = condition {
                    self.walk_interrupt_expr(function, condition, diagnostics);
                }
                if let Some(step) = step {
                    self.walk_interrupt_expr(function, step, diagnostics);
                }
                self.walk_interrupt_stmt(function, body, diagnostics);
            }
            TypedStmt::Break(_) | TypedStmt::Continue(_) | TypedStmt::Empty(_) => {}
        }
    }

    /// Walks one typed expression tree and rejects unsupported Phase 6 ISR operations.
    fn walk_interrupt_expr(
        &self,
        function: SymbolId,
        expr: &TypedExpr,
        diagnostics: &mut DiagnosticBag,
    ) {
        match &expr.kind {
            TypedExprKind::Call { function: callee, args } => {
                diagnostics.error(
                    "semantic",
                    Some(expr.span),
                    format!(
                        "interrupt handler `{}` cannot call `{}` in phase 6",
                        self.symbols[function].name,
                        self.symbols[*callee].name
                    ),
                    Some("keep ISR code inline; call normal functions from non-interrupt code".to_string()),
                );
                for arg in args {
                    self.walk_interrupt_expr(function, arg, diagnostics);
                }
            }
            TypedExprKind::Binary { op, lhs, rhs } => {
                self.walk_interrupt_expr(function, lhs, diagnostics);
                self.walk_interrupt_expr(function, rhs, diagnostics);
                if self.is_interrupt_helper_required(*op, expr.ty, lhs, rhs) {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        format!(
                            "interrupt handler `{}` cannot use `{op:?}` when it would lower through a runtime helper",
                            self.symbols[function].name
                        ),
                        Some("use inline-safe arithmetic only inside the ISR in phase 6".to_string()),
                    );
                }
            }
            TypedExprKind::Unary { expr, .. }
            | TypedExprKind::ArrayDecay(expr)
            | TypedExprKind::AddressOf(expr)
            | TypedExprKind::Deref(expr)
            | TypedExprKind::Cast { expr, .. } => {
                self.walk_interrupt_expr(function, expr, diagnostics);
            }
            TypedExprKind::BitField { storage, .. } => {
                self.walk_interrupt_expr(function, storage, diagnostics);
            }
            TypedExprKind::Assign { target, value } => {
                self.walk_interrupt_expr(function, target, diagnostics);
                self.walk_interrupt_expr(function, value, diagnostics);
            }
            TypedExprKind::StructAssign { target, value, .. } => {
                diagnostics.error(
                    "semantic",
                    Some(expr.span),
                    format!(
                        "interrupt handler `{}` cannot perform whole-aggregate copy in phase 15",
                        self.symbols[function].name
                    ),
                    Some("copy scalar fields explicitly outside the ISR".to_string()),
                );
                self.walk_interrupt_expr(function, target, diagnostics);
                self.walk_interrupt_expr(function, value, diagnostics);
            }
            TypedExprKind::RomRead8 { index, .. } | TypedExprKind::RomRead16 { index, .. } => {
                if !is_constant_expression(index) {
                    diagnostics.error(
                        "semantic",
                        Some(expr.span),
                        format!(
                            "interrupt handler `{}` cannot perform dynamic ROM reads in phase 14",
                            self.symbols[function].name
                        ),
                        Some("use a constant ROM index inside the ISR or prefetch ROM data outside interrupt code".to_string()),
                    );
                }
                self.walk_interrupt_expr(function, index, diagnostics);
            }
            TypedExprKind::IntLiteral(_) | TypedExprKind::Symbol(_) => {}
        }
    }

    /// Returns true when one ISR expression would need a Phase 5 runtime helper call.
    fn is_interrupt_helper_required(
        &self,
        op: BinaryOp,
        ty: Type,
        lhs: &TypedExpr,
        rhs: &TypedExpr,
    ) -> bool {
        let lhs_const = eval_integer_constant_expr(lhs).map(|value| normalize_value(value, ty));
        let rhs_const = eval_integer_constant_expr(rhs).map(|value| normalize_value(value, ty));

        match op {
            BinaryOp::Multiply => {
                !(lhs_const == Some(0)
                    || rhs_const == Some(0)
                    || lhs_const == Some(1)
                    || rhs_const == Some(1)
                    || normalized_power_of_two_shift(lhs_const, ty).is_some()
                    || normalized_power_of_two_shift(rhs_const, ty).is_some())
            }
            BinaryOp::Divide => !(lhs_const == Some(0) || rhs_const == Some(1)),
            BinaryOp::Modulo => !(lhs_const == Some(0) || rhs_const == Some(1)),
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => eval_integer_constant_expr(rhs).is_none(),
            _ => false,
        }
    }

    /// Returns the single interrupt-handler symbol when the program declares one.
    fn interrupt_handler(&self) -> Option<SymbolId> {
        self.symbols
            .iter()
            .find(|symbol| symbol.kind == SymbolKind::Function && symbol.is_interrupt)
            .map(|symbol| symbol.id)
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
                is_interrupt: false,
                kind,
                span,
                fixed_address: None,
                is_defined: true,
                is_referenced: false,
                parameter_types: Vec::new(),
                enum_const_value: None,
            });
        }

        let id = self.insert_symbol(Symbol {
            id: self.symbols.len(),
            name: name.clone(),
            ty,
            storage_class,
            is_interrupt: false,
            kind,
            span,
            fixed_address: None,
            is_defined: true,
            is_referenced: false,
            parameter_types: Vec::new(),
            enum_const_value: None,
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

/// Returns the source span that covers one parsed initializer.
fn initializer_span(initializer: &Initializer) -> Span {
    match initializer {
        Initializer::Expr(expr) => expr.span,
        Initializer::List(_, span) => *span,
    }
}

/// Returns the source span covering one initializer-list entry.
fn initializer_entry_span(entry: &InitializerEntry) -> Span {
    entry
        .designator
        .as_ref()
        .map_or_else(|| initializer_span(&entry.initializer), designator_span)
}

/// Returns the source span covering one parsed designator.
fn designator_span(designator: &Designator) -> Span {
    match designator {
        Designator::Field(_, span) | Designator::Index(_, span) => *span,
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
        | TypedExprKind::BitField { .. }
        | TypedExprKind::StructAssign { .. }
        | TypedExprKind::RomRead8 { .. }
        | TypedExprKind::RomRead16 { .. }
        | TypedExprKind::Call { .. }
        | TypedExprKind::ArrayDecay(_)
        | TypedExprKind::AddressOf(_)
        | TypedExprKind::Deref(_)
        | TypedExprKind::Symbol(_) => false,
    }
}

/// Evaluates one typed integer constant expression under the compiler's fixed-width rules.
fn eval_integer_constant_expr(expr: &TypedExpr) -> Option<i64> {
    if !expr.ty.is_integer() {
        return None;
    }

    let value = match &expr.kind {
        TypedExprKind::IntLiteral(value) => *value,
        TypedExprKind::Unary { op, expr } => eval_unary(*op, eval_integer_constant_expr(expr)?, expr.ty, expr.ty),
        TypedExprKind::Binary { op, lhs, rhs } => {
            let lhs_value = eval_integer_constant_expr(lhs)?;
            let rhs_value = eval_integer_constant_expr(rhs)?;
            eval_binary(*op, lhs_value, rhs_value, lhs.ty, expr.ty)
        }
        TypedExprKind::Cast {
            kind,
            expr: value_expr,
        } => {
            let value = eval_integer_constant_expr(value_expr)?;
            match kind {
                CastKind::ZeroExtend | CastKind::Truncate | CastKind::Bitcast => {
                    normalize_value(value, expr.ty)
                }
                CastKind::SignExtend => normalize_value(signed_value(value, value_expr.ty), expr.ty),
            }
        }
        TypedExprKind::Assign { .. }
        | TypedExprKind::BitField { .. }
        | TypedExprKind::StructAssign { .. }
        | TypedExprKind::RomRead8 { .. }
        | TypedExprKind::RomRead16 { .. }
        | TypedExprKind::Call { .. }
        | TypedExprKind::ArrayDecay(_)
        | TypedExprKind::AddressOf(_)
        | TypedExprKind::Deref(_)
        | TypedExprKind::Symbol(_) => return None,
    };

    Some(normalize_value(value, expr.ty))
}

/// Returns true when an integer constant expression can be represented exactly by `target_ty`.
fn is_representable_integer_constant(expr: &TypedExpr, target_ty: Type) -> bool {
    if !target_ty.is_integer() {
        return false;
    }
    let Some((min, max)) = integer_value_range(target_ty) else {
        return false;
    };
    let Some(value) = eval_integer_constant_expr(expr) else {
        return false;
    };

    let value = if expr.ty.is_signed() {
        signed_value(value, expr.ty)
    } else {
        normalize_value(value, expr.ty)
    };

    (min..=max).contains(&value)
}

/// Returns one constant expression's mathematical integer value before coercing it to a target type.
fn signed_or_unsigned_constant_value(expr: &TypedExpr) -> Option<i64> {
    let value = eval_integer_constant_expr(expr)?;
    Some(if expr.ty.is_signed() {
        signed_value(value, expr.ty)
    } else {
        normalize_value(value, expr.ty)
    })
}

/// Returns the closed signed range that values of this integer type can represent.
fn integer_value_range(ty: Type) -> Option<(i64, i64)> {
    if !ty.is_integer() {
        return None;
    }

    match ty.scalar {
        ScalarType::I8 => Some((i64::from(i8::MIN), i64::from(i8::MAX))),
        ScalarType::U8 => Some((0, i64::from(u8::MAX))),
        ScalarType::I16 => Some((i64::from(i16::MIN), i64::from(i16::MAX))),
        ScalarType::U16 => Some((0, i64::from(u16::MAX))),
        ScalarType::Void => None,
    }
}

/// Returns true when every qualifier present in `required` is also present in `provided`.
fn qualifiers_include(provided: Qualifiers, required: Qualifiers) -> bool {
    (!required.is_const || provided.is_const) && (!required.is_volatile || provided.is_volatile)
}

/// Returns the shift amount when a normalized integer constant is an exact power of two.
fn normalized_power_of_two_shift(value: Option<i64>, ty: Type) -> Option<usize> {
    let value = normalize_value(value?, ty) as u64;
    if value == 0 || !value.is_power_of_two() {
        return None;
    }
    Some(value.trailing_zeros() as usize)
}

/// Collects direct function-call targets that appear anywhere inside one typed statement tree.
fn collect_stmt_calls(stmt: &TypedStmt, callees: &mut BTreeSet<SymbolId>) {
    match stmt {
        TypedStmt::Block(statements, _) => {
            for statement in statements {
                collect_stmt_calls(statement, callees);
            }
        }
        TypedStmt::Switch { expr, body, .. } => {
            collect_expr_calls(expr, callees);
            collect_stmt_calls(body, callees);
        }
        TypedStmt::Case { body, .. } | TypedStmt::Default { body, .. } => {
            collect_stmt_calls(body, callees);
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
        TypedExprKind::BitField { storage, .. } => collect_expr_calls(storage, callees),
        TypedExprKind::Binary { lhs, rhs, .. } => {
            collect_expr_calls(lhs, callees);
            collect_expr_calls(rhs, callees);
        }
        TypedExprKind::Assign { target, value } => {
            collect_expr_calls(target, callees);
            collect_expr_calls(value, callees);
        }
        TypedExprKind::StructAssign { target, value, .. } => {
            collect_expr_calls(target, callees);
            collect_expr_calls(value, callees);
        }
        TypedExprKind::RomRead8 { index, .. } | TypedExprKind::RomRead16 { index, .. } => {
            collect_expr_calls(index, callees)
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
    use super::{eval_integer_constant_expr, is_constant_expression, ValueCategory};
    use crate::common::source::Span;
    use crate::frontend::ast::BinaryOp;
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

    #[test]
    /// Verifies constant-expression evaluation covers the Phase 5 shift operators.
    fn phase_five_constant_expr_evaluates_shifts() {
        let span = Span::new(0, 0);
        let expr = TypedExpr {
            kind: TypedExprKind::Binary {
                op: BinaryOp::ShiftRight,
                lhs: Box::new(TypedExpr {
                    kind: TypedExprKind::IntLiteral(-2),
                    ty: Type::new(ScalarType::I16),
                    span,
                    value_category: ValueCategory::RValue,
                }),
                rhs: Box::new(TypedExpr {
                    kind: TypedExprKind::IntLiteral(1),
                    ty: Type::new(ScalarType::I16),
                    span,
                    value_category: ValueCategory::RValue,
                }),
            },
            ty: Type::new(ScalarType::I16),
            span,
            value_category: ValueCategory::RValue,
        };

        assert_eq!(eval_integer_constant_expr(&expr), Some(0xFFFF));
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
