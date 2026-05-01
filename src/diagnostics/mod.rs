// SPDX-License-Identifier: GPL-3.0-or-later

pub mod bag;
pub mod emitter;

pub use bag::{Diagnostic, DiagnosticBag, Severity, StageResult, WarningProfile};
pub use emitter::DiagnosticEmitter;
// SPDX-License-Identifier: GPL-3.0-or-later

