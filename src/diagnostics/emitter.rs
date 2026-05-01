// SPDX-License-Identifier: GPL-3.0-or-later

use crate::common::source::{PreprocessedSource, SourceManager};

use super::bag::DiagnosticBag;

pub struct DiagnosticEmitter<'a> {
    sources: &'a SourceManager,
    preprocessed: &'a PreprocessedSource,
}

impl<'a> DiagnosticEmitter<'a> {
    /// Builds an emitter that can map diagnostic spans back to source lines.
    pub fn new(sources: &'a SourceManager, preprocessed: &'a PreprocessedSource) -> Self {
        Self {
            sources,
            preprocessed,
        }
    }

    /// Prints diagnostics with source context when span information is available.
    pub fn print(&self, bag: &DiagnosticBag) {
        for diagnostic in &bag.diagnostics {
            if let Some(span) = diagnostic.span
                && let Some(point) = self.preprocessed.point(span.start)
            {
                let path = self.sources.path(point.file);
                eprintln!(
                    "{}:{}:{}: {}: {}",
                    path.display(),
                    point.line,
                    point.column,
                    diagnostic.severity.as_str(),
                    diagnostic.message
                );
                let line = self.sources.line_text(point);
                eprintln!("  {line}");
                eprintln!("  {}^", " ".repeat(point.column.saturating_sub(1)));
                if let Some(suggestion) = &diagnostic.suggestion {
                    eprintln!("  help: {suggestion}");
                }
                continue;
            }

            eprintln!(
                "{}: {}: {}",
                diagnostic.stage,
                diagnostic.severity.as_str(),
                diagnostic.message
            );
            if let Some(suggestion) = &diagnostic.suggestion {
                eprintln!("  help: {suggestion}");
            }
        }
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
