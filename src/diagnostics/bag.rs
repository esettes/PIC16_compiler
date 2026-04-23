use std::fmt::{Display, Formatter};

use crate::common::source::{SourcePoint, Span};

pub type StageResult<T> = Result<T, DiagnosticBag>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WarningProfile {
    pub wall: bool,
    pub wextra: bool,
    pub werror: bool,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub stage: &'static str,
    pub message: String,
    pub span: Option<Span>,
    pub note: Option<String>,
    pub suggestion: Option<String>,
    pub code: Option<&'static str>,
}

#[derive(Debug)]
pub struct DiagnosticBag {
    pub diagnostics: Vec<Diagnostic>,
    pub warning_profile: WarningProfile,
}

impl DiagnosticBag {
    /// Creates an empty diagnostic bag with the requested warning profile.
    pub fn new(warning_profile: WarningProfile) -> Self {
        Self {
            diagnostics: Vec::new(),
            warning_profile,
        }
    }

    /// Builds a bag containing one diagnostic, typically for top-level failures.
    pub fn single(severity: Severity, stage: &'static str, message: String) -> Self {
        let mut bag = Self::new(WarningProfile::default());
        bag.push(Diagnostic {
            severity,
            stage,
            message,
            span: None,
            note: None,
            suggestion: None,
            code: None,
        });
        bag
    }

    /// Adds one diagnostic and upgrades warnings to errors when `-Werror` is active.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(if diagnostic.severity == Severity::Warning && self.warning_profile.werror {
            Diagnostic {
                severity: Severity::Error,
                ..diagnostic
            }
        } else {
            diagnostic
        });
    }

    /// Records an error diagnostic with optional source span and fix hint.
    pub fn error(
        &mut self,
        stage: &'static str,
        span: Option<Span>,
        message: impl Into<String>,
        suggestion: Option<String>,
    ) {
        self.push(Diagnostic {
            severity: Severity::Error,
            stage,
            message: message.into(),
            span,
            note: None,
            suggestion,
            code: None,
        });
    }

    /// Records a `-Wall` warning when that warning group is enabled.
    pub fn warning(
        &mut self,
        stage: &'static str,
        span: Option<Span>,
        message: impl Into<String>,
        code: &'static str,
    ) {
        if !self.warning_profile.wall {
            return;
        }
        self.push(Diagnostic {
            severity: Severity::Warning,
            stage,
            message: message.into(),
            span,
            note: None,
            suggestion: None,
            code: Some(code),
        });
    }

    /// Records a `-Wextra` warning when that warning group is enabled.
    pub fn extra_warning(
        &mut self,
        stage: &'static str,
        span: Option<Span>,
        message: impl Into<String>,
        code: &'static str,
    ) {
        if !self.warning_profile.wextra {
            return;
        }
        self.push(Diagnostic {
            severity: Severity::Warning,
            stage,
            message: message.into(),
            span,
            note: None,
            suggestion: None,
            code: Some(code),
        });
    }

    /// Adds an informational note diagnostic without changing error state.
    pub fn note(&mut self, stage: &'static str, span: Option<Span>, message: impl Into<String>) {
        self.push(Diagnostic {
            severity: Severity::Note,
            stage,
            message: message.into(),
            span,
            note: None,
            suggestion: None,
            code: None,
        });
    }

    /// Returns true when the bag contains at least one error-level diagnostic.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

impl Default for DiagnosticBag {
    /// Creates a bag with the default warning profile.
    fn default() -> Self {
        Self::new(WarningProfile::default())
    }
}

impl Display for DiagnosticBag {
    /// Formats diagnostics in a compact stage-prefixed form.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        for diagnostic in &self.diagnostics {
            writeln!(formatter, "[{:?}] {}: {}", diagnostic.severity, diagnostic.stage, diagnostic.message)?;
        }
        Ok(())
    }
}

impl Severity {
    /// Returns the lowercase severity label used in human-readable output.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

#[allow(dead_code)]
/// Renders a source point as `line:column` for debug-friendly output.
pub fn render_point(point: SourcePoint) -> String {
    format!("{}:{}", point.line, point.column)
}
