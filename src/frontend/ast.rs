use std::fmt::Write;

use crate::common::source::Span;

use super::types::{StorageClass, Type};

#[derive(Clone, Debug)]
pub struct TranslationUnit {
    pub items: Vec<Item>,
}

#[derive(Clone, Debug)]
pub enum Item {
    Function(FunctionDecl),
    Global(VarDecl),
}

#[derive(Clone, Debug)]
pub struct FunctionDecl {
    pub name: String,
    pub return_type: Type,
    pub storage_class: StorageClass,
    pub params: Vec<VarDecl>,
    pub body: Option<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct VarDecl {
    pub name: String,
    pub ty: Type,
    pub storage_class: StorageClass,
    pub initializer: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Block(Vec<Stmt>, Span),
    VarDecl(VarDecl),
    Expr(Expr, Span),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Box<Stmt>,
        span: Span,
    },
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
        span: Span,
    },
    For {
        init: Option<Box<Stmt>>,
        condition: Option<Expr>,
        step: Option<Expr>,
        body: Box<Stmt>,
        span: Span,
    },
    Return(Option<Expr>, Span),
    Break(Span),
    Continue(Span),
    Empty(Span),
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum ExprKind {
    IntLiteral(i64),
    Name(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    AddressOf(Box<Expr>),
    Deref(Box<Expr>),
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    SizeOfExpr(Box<Expr>),
    SizeOfType(Type),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Negate,
    LogicalNot,
    BitwiseNot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Multiply,
    Divide,
    Modulo,
    BitAnd,
    BitOr,
    BitXor,
    LogicalAnd,
    LogicalOr,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

impl TranslationUnit {
    /// Renders a compact textual view of the parsed AST for debugging artifacts.
    pub fn render(&self) -> String {
        let mut output = String::new();
        for item in &self.items {
            match item {
                Item::Function(function) => {
                    let _ = writeln!(
                        output,
                        "fn {}({}) -> {}",
                        function.name,
                        function
                            .params
                            .iter()
                            .map(|param| format!("{} {}", param.ty, param.name))
                            .collect::<Vec<_>>()
                            .join(", "),
                        function.return_type
                    );
                    if let Some(body) = &function.body {
                        render_stmt(body, 1, &mut output);
                    }
                }
                Item::Global(global) => {
                    let _ = writeln!(output, "global {} {}", global.ty, global.name);
                }
            }
        }
        output
    }
}

/// Renders one statement subtree with indentation that matches AST depth.
fn render_stmt(stmt: &Stmt, indent: usize, output: &mut String) {
    let prefix = "  ".repeat(indent);
    match stmt {
        Stmt::Block(statements, _) => {
            let _ = writeln!(output, "{prefix}block");
            for statement in statements {
                render_stmt(statement, indent + 1, output);
            }
        }
        Stmt::VarDecl(decl) => {
            let _ = writeln!(output, "{prefix}let {} {}", decl.ty, decl.name);
        }
        Stmt::Expr(expr, _) => {
            let _ = writeln!(output, "{prefix}expr {}", render_expr(expr));
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            let _ = writeln!(output, "{prefix}if {}", render_expr(condition));
            render_stmt(then_branch, indent + 1, output);
            if let Some(else_branch) = else_branch {
                let _ = writeln!(output, "{prefix}else");
                render_stmt(else_branch, indent + 1, output);
            }
        }
        Stmt::While { condition, body, .. } => {
            let _ = writeln!(output, "{prefix}while {}", render_expr(condition));
            render_stmt(body, indent + 1, output);
        }
        Stmt::DoWhile { body, condition, .. } => {
            let _ = writeln!(output, "{prefix}do");
            render_stmt(body, indent + 1, output);
            let _ = writeln!(output, "{prefix}while {}", render_expr(condition));
        }
        Stmt::For {
            init,
            condition,
            step,
            body,
            ..
        } => {
            let _ = writeln!(
                output,
                "{prefix}for init={} cond={} step={}",
                init.as_ref().map_or_else(|| "-".to_string(), |_| "stmt".to_string()),
                condition
                    .as_ref()
                    .map_or_else(|| "-".to_string(), render_expr),
                step.as_ref().map_or_else(|| "-".to_string(), render_expr)
            );
            render_stmt(body, indent + 1, output);
        }
        Stmt::Return(expr, _) => {
            let _ = writeln!(
                output,
                "{prefix}return {}",
                expr.as_ref().map_or_else(|| "void".to_string(), render_expr)
            );
        }
        Stmt::Break(_) => {
            let _ = writeln!(output, "{prefix}break");
        }
        Stmt::Continue(_) => {
            let _ = writeln!(output, "{prefix}continue");
        }
        Stmt::Empty(_) => {
            let _ = writeln!(output, "{prefix}empty");
        }
    }
}

/// Renders one expression subtree into a compact single-line form.
fn render_expr(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::IntLiteral(value) => value.to_string(),
        ExprKind::Name(name) => name.clone(),
        ExprKind::Unary { op, expr } => format!("{op:?}({})", render_expr(expr)),
        ExprKind::AddressOf(expr) => format!("&({})", render_expr(expr)),
        ExprKind::Deref(expr) => format!("*({})", render_expr(expr)),
        ExprKind::Binary { op, lhs, rhs } => {
            format!("({} {op:?} {})", render_expr(lhs), render_expr(rhs))
        }
        ExprKind::Index { base, index } => {
            format!("{}[{}]", render_expr(base), render_expr(index))
        }
        ExprKind::Assign { target, value } => {
            format!("({} = {})", render_expr(target), render_expr(value))
        }
        ExprKind::Call { callee, args } => format!(
            "{}({})",
            callee,
            args.iter().map(render_expr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::SizeOfExpr(expr) => format!("sizeof({})", render_expr(expr)),
        ExprKind::SizeOfType(ty) => format!("sizeof({ty})"),
    }
}
