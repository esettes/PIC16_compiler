// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceId(pub usize);

#[derive(Clone, Debug)]
pub struct SourceFile {
    pub id: SourceId,
    pub path: PathBuf,
    pub text: String,
    pub line_starts: Vec<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourcePoint {
    pub file: SourceId,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug)]
pub struct PreprocessedSource {
    pub text: String,
    pub origins: Vec<SourcePoint>,
}

impl PreprocessedSource {
    /// Creates an empty preprocessed buffer with no origin mapping.
    pub fn new() -> Self {
        Self {
            text: String::new(),
            origins: Vec::new(),
        }
    }

    /// Appends one character while preserving its source origin.
    pub fn push_char(&mut self, ch: char, origin: SourcePoint) {
        self.text.push(ch);
        self.origins.push(origin);
    }

    /// Appends a string and assigns the same origin to each emitted character.
    pub fn push_str(&mut self, text: &str, origin: SourcePoint) {
        for ch in text.chars() {
            self.push_char(ch, origin);
        }
    }

    /// Returns the number of origin-tracked characters in the buffer.
    pub fn len(&self) -> usize {
        self.origins.len()
    }

    /// Returns true when the preprocessed buffer has no tracked characters.
    pub fn is_empty(&self) -> bool {
        self.origins.is_empty()
    }

    /// Resolves a preprocessed character index back to its original source point.
    pub fn point(&self, index: usize) -> Option<SourcePoint> {
        self.origins
            .get(index.min(self.origins.len().saturating_sub(1)))
            .copied()
    }
}

impl Default for PreprocessedSource {
    /// Creates an empty preprocessed buffer.
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    /// Builds a half-open span over preprocessed character indexes.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Default)]
pub struct SourceManager {
    files: Vec<SourceFile>,
}

impl SourceManager {
    /// Creates a source manager with no loaded files.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a file once and returns a stable source identifier for later lookups.
    pub fn load(&mut self, path: &Path) -> Result<SourceId, std::io::Error> {
        let canonical = fs::canonicalize(path)?;
        if let Some(existing) = self.files.iter().find(|file| file.path == canonical) {
            return Ok(existing.id);
        }
        let text = fs::read_to_string(&canonical)?;
        let id = SourceId(self.files.len());
        let file = SourceFile {
            id,
            line_starts: compute_line_starts(&text),
            path: canonical,
            text,
        };
        self.files.push(file);
        Ok(id)
    }

    /// Returns the loaded source file for a previously issued source id.
    pub fn file(&self, id: SourceId) -> &SourceFile {
        &self.files[id.0]
    }

    /// Returns the canonical path associated with a source id.
    pub fn path(&self, id: SourceId) -> &Path {
        &self.file(id).path
    }

    /// Returns the full source line that contains the supplied source point.
    pub fn line_text(&self, point: SourcePoint) -> String {
        let file = self.file(point.file);
        let start = file.line_starts[point.line.saturating_sub(1)];
        let end = file
            .line_starts
            .get(point.line)
            .copied()
            .unwrap_or(file.text.len());
        file.text[start..end].trim_end_matches('\n').to_string()
    }
}

/// Precomputes the byte offsets where each source line begins.
fn compute_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, ch) in text.char_indices() {
        if ch == '\n' {
            starts.push(index + 1);
        }
    }
    starts
}

impl Display for SourcePoint {
    /// Formats a source point as `line:column`.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.line, self.column)
    }
}
// SPDX-License-Identifier: GPL-3.0-or-later
