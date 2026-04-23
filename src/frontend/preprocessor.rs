use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use crate::backend::pic16::devices::TargetDevice;
use crate::common::source::{PreprocessedSource, SourceId, SourceManager, SourcePoint};
use crate::default_predefined_macros;
use crate::diagnostics::DiagnosticBag;

#[derive(Clone, Debug)]
struct MacroDef {
    value: String,
}

#[derive(Clone, Copy, Debug)]
struct ConditionFrame {
    parent_active: bool,
    branch_taken: bool,
    current_active: bool,
}

pub struct Preprocessor<'a> {
    target: &'a TargetDevice,
    include_dirs: Vec<PathBuf>,
    macros: BTreeMap<String, MacroDef>,
    source_manager: &'a mut SourceManager,
    include_guard: HashSet<PathBuf>,
}

impl<'a> Preprocessor<'a> {
    /// Creates a preprocessor with target macros, user defines, and include search paths.
    pub fn new(
        target: &'a TargetDevice,
        include_dirs: Vec<PathBuf>,
        defines: BTreeMap<String, String>,
        source_manager: &'a mut SourceManager,
    ) -> Self {
        let mut macros = BTreeMap::new();
        for (name, value) in default_predefined_macros(target).into_iter().chain(defines) {
            macros.insert(name, MacroDef { value });
        }
        Self {
            target,
            include_dirs,
            macros,
            source_manager,
            include_guard: HashSet::new(),
        }
    }

    /// Expands one translation unit into preprocessed text with source-origin tracking.
    pub fn process(
        &mut self,
        main_source: SourceId,
        diagnostics: &mut DiagnosticBag,
    ) -> Option<PreprocessedSource> {
        let mut output = PreprocessedSource::new();
        let mut conditions = Vec::new();
        self.process_file(main_source, &mut output, &mut conditions, diagnostics);
        if !conditions.is_empty() {
            diagnostics.error(
                "preprocessor",
                None,
                "unterminated conditional compilation block",
                Some("add matching `#endif`".to_string()),
            );
        }
        if diagnostics.has_errors() {
            None
        } else {
            Some(output)
        }
    }

    /// Processes one source file, recursively handling nested includes and directives.
    fn process_file(
        &mut self,
        source_id: SourceId,
        output: &mut PreprocessedSource,
        conditions: &mut Vec<ConditionFrame>,
        diagnostics: &mut DiagnosticBag,
    ) {
        let path = self.source_manager.path(source_id).to_path_buf();
        if !self.include_guard.insert(path.clone()) {
            return;
        }

        let file = self.source_manager.file(source_id).clone();
        for (line_index, line) in file.text.lines().enumerate() {
            let point = SourcePoint {
                file: source_id,
                line: line_index + 1,
                column: 1,
            };
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                self.handle_directive(
                    &path,
                    source_id,
                    line_index + 1,
                    trimmed,
                    output,
                    conditions,
                    diagnostics,
                );
                continue;
            }

            if is_active(conditions) {
                let expanded = self.expand_line(line, point, diagnostics);
                output.push_str(&expanded, point);
                output.push_char('\n', point);
            }
        }

        self.include_guard.remove(&path);
    }

    #[allow(clippy::too_many_arguments)]
    /// Handles one preprocessor directive line in the context of the current condition stack.
    fn handle_directive(
        &mut self,
        current_path: &Path,
        source_id: SourceId,
        line_number: usize,
        directive_line: &str,
        output: &mut PreprocessedSource,
        conditions: &mut Vec<ConditionFrame>,
        diagnostics: &mut DiagnosticBag,
    ) {
        let origin = SourcePoint {
            file: source_id,
            line: line_number,
            column: 1,
        };
        let content = directive_line.trim_start_matches('#').trim();
        let (directive, rest) = split_directive(content);

        match directive {
            "include" => {
                if !is_active(conditions) {
                    return;
                }
                let Some(path) = parse_include(rest) else {
                    diagnostics.error(
                        "preprocessor",
                        None,
                        "malformed #include",
                        Some("use `#include \"path.h\"` or `#include <path.h>`".to_string()),
                    );
                    return;
                };
                let Some(resolved) = self.resolve_include(current_path, &path) else {
                    diagnostics.error(
                        "preprocessor",
                        None,
                        format!("include `{path}` not found"),
                        None,
                    );
                    return;
                };
                match self.source_manager.load(&resolved) {
                    Ok(source) => self.process_file(source, output, conditions, diagnostics),
                    Err(error) => diagnostics.error(
                        "preprocessor",
                        None,
                        format!("failed to read include `{}`: {error}", resolved.display()),
                        None,
                    ),
                }
            }
            "define" => {
                if !is_active(conditions) {
                    return;
                }
                let mut parts = rest.splitn(2, char::is_whitespace);
                let Some(name) = parts.next() else {
                    diagnostics.error(
                        "preprocessor",
                        None,
                        "malformed #define",
                        Some("expected identifier after `#define`".to_string()),
                    );
                    return;
                };
                if name.contains('(') {
                    diagnostics.error(
                        "preprocessor",
                        None,
                        format!("function-like macro `{name}` is not implemented"),
                        None,
                    );
                    return;
                }
                let value = parts.next().unwrap_or("").trim().to_string();
                self.macros.insert(name.to_string(), MacroDef { value });
            }
            "undef" => {
                if is_active(conditions) {
                    self.macros.remove(rest.trim());
                }
            }
            "ifdef" => {
                let active = self.macros.contains_key(rest.trim());
                push_condition(conditions, active);
            }
            "ifndef" => {
                let active = !self.macros.contains_key(rest.trim());
                push_condition(conditions, active);
            }
            "if" => {
                let active = self.evaluate_if_expression(rest);
                push_condition(conditions, active);
            }
            "else" => {
                let Some(frame) = conditions.last_mut() else {
                    diagnostics.error(
                        "preprocessor",
                        None,
                        "`#else` without matching `#if`",
                        None,
                    );
                    return;
                };
                frame.current_active = frame.parent_active && !frame.branch_taken;
                frame.branch_taken = true;
            }
            "endif" => {
                if conditions.pop().is_none() {
                    diagnostics.error(
                        "preprocessor",
                        None,
                        "`#endif` without matching `#if`",
                        None,
                    );
                }
            }
            "" => {}
            _ => {
                diagnostics.error(
                    "preprocessor",
                    None,
                    format!("unsupported preprocessor directive `#{directive}`"),
                    Some(
                        "supported directives: #include, #define, #undef, #if, #ifdef, #ifndef, #else, #endif"
                            .to_string(),
                    ),
                );
            }
        }

        if is_active(conditions) {
            output.push_char('\n', origin);
        }
    }

    /// Resolves an include path against the current file, user paths, and builtin include dir.
    fn resolve_include(&self, current_path: &Path, include: &str) -> Option<PathBuf> {
        let current_dir = current_path.parent().unwrap_or_else(|| Path::new("."));
        let local_candidate = current_dir.join(include);
        if local_candidate.exists() {
            return Some(local_candidate);
        }
        for include_dir in &self.include_dirs {
            let candidate = include_dir.join(include);
            if candidate.exists() {
                return Some(candidate);
            }
        }
        let builtin = PathBuf::from("include").join(include);
        if builtin.exists() {
            return Some(builtin);
        }
        None
    }

    /// Expands object-like macros in a line and guards against runaway recursion.
    fn expand_line(
        &self,
        line: &str,
        origin: SourcePoint,
        diagnostics: &mut DiagnosticBag,
    ) -> String {
        let mut current = line.to_string();
        for _ in 0..16 {
            let next = expand_identifiers(&current, &self.macros);
            if next == current {
                return next;
            }
            current = next;
        }
        diagnostics.error(
            "preprocessor",
            None,
            format!("macro expansion recursion limit hit in {}", self.target.name),
            Some("simplify mutually recursive macros".to_string()),
        );
        let _ = origin;
        current
    }

    /// Evaluates the limited `#if` expression syntax supported by the current preprocessor.
    fn evaluate_if_expression(&self, expression: &str) -> bool {
        let trimmed = expression.trim();
        if trimmed.starts_with("defined(") && trimmed.ends_with(')') {
            let name = trimmed
                .trim_start_matches("defined(")
                .trim_end_matches(')')
                .trim();
            return self.macros.contains_key(name);
        }
        if let Some(value) = self.macros.get(trimmed) {
            return value.value != "0";
        }
        trimmed != "0"
    }
}

/// Splits a directive line into its directive keyword and trailing payload.
fn split_directive(content: &str) -> (&str, &str) {
    if let Some(index) = content.find(char::is_whitespace) {
        (&content[..index], content[index..].trim())
    } else {
        (content, "")
    }
}

/// Parses `#include` syntax and returns the raw path between quotes or angle brackets.
fn parse_include(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') {
        return Some(trimmed.trim_matches('"').to_string());
    }
    if trimmed.starts_with('<') && trimmed.ends_with('>') {
        return Some(trimmed.trim_matches(&['<', '>'][..]).to_string());
    }
    None
}

/// Pushes a new conditional-compilation frame derived from the current parent state.
fn push_condition(conditions: &mut Vec<ConditionFrame>, active_branch: bool) {
    let parent_active = is_active(conditions);
    conditions.push(ConditionFrame {
        parent_active,
        branch_taken: active_branch,
        current_active: parent_active && active_branch,
    });
}

/// Returns true when all active preprocessor condition frames allow emission.
fn is_active(conditions: &[ConditionFrame]) -> bool {
    conditions.iter().all(|frame| frame.current_active)
}

/// Expands identifiers using object-like macro definitions while preserving strings.
fn expand_identifiers(line: &str, macros: &BTreeMap<String, MacroDef>) -> String {
    let mut output = String::new();
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '"' {
            output.push(ch);
            index += 1;
            while index < chars.len() {
                let current = chars[index];
                output.push(current);
                index += 1;
                if current == '"' {
                    break;
                }
                if current == '\\' && index < chars.len() {
                    output.push(chars[index]);
                    index += 1;
                }
            }
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = index;
            index += 1;
            while index < chars.len() && (chars[index].is_ascii_alphanumeric() || chars[index] == '_') {
                index += 1;
            }
            let name: String = chars[start..index].iter().collect();
            if let Some(definition) = macros.get(&name) {
                output.push_str(&definition.value);
            } else {
                output.push_str(&name);
            }
            continue;
        }
        output.push(ch);
        index += 1;
    }
    output
}
