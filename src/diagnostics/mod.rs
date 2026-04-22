pub mod bag;
pub mod emitter;

pub use bag::{Diagnostic, DiagnosticBag, Severity, StageResult, WarningProfile};
pub use emitter::DiagnosticEmitter;

