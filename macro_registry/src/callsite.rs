use module_path_extractor::{
    find_module_path, get_pseudo_module_path, get_source_info, module_path_from_file,
};
use proc_macro2::Span;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceContextQuality {
    Complete,
    Partial,
    Missing,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceContext {
    pub file_path: Option<String>,
    pub line_number: usize,
    pub module_path: Option<String>,
}

impl SourceContext {
    pub fn quality(&self) -> SourceContextQuality {
        if self.file_path.is_some() && self.line_number > 0 && self.module_path.is_some() {
            SourceContextQuality::Complete
        } else if self.file_path.is_some() || self.module_path.is_some() {
            SourceContextQuality::Partial
        } else {
            SourceContextQuality::Missing
        }
    }
}

fn force_missing_source_info() -> bool {
    std::env::var("STATUM_TEST_FORCE_MISSING_SOURCE_INFO").is_ok()
}

fn force_missing_module_path() -> bool {
    std::env::var("STATUM_TEST_FORCE_MISSING_MODULE_PATH").is_ok()
}

fn normalize_file_path(file_path: &str) -> String {
    let path = Path::new(file_path);
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }

    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path).to_string_lossy().into_owned(),
        Err(_) => file_path.to_string(),
    }
}

fn source_file_exists(file_path: &str) -> bool {
    Path::new(file_path).is_file()
}

/// Returns `(file_path, line_number)` for the current macro call-site when available.
pub fn current_source_info() -> Option<(String, usize)> {
    if force_missing_source_info() {
        return None;
    }

    get_source_info()
        .map(|(file_path, line_number)| (normalize_file_path(&file_path), line_number))
        .filter(|(file_path, _)| source_file_exists(file_path))
}

/// Returns `(file_path, line_number)` for an explicit span when available.
#[allow(unexpected_cfgs)]
#[cfg(rust_analyzer)]
pub fn source_info_for_span(_span: Span) -> Option<(String, usize)> {
    None
}

/// Returns `(file_path, line_number)` for an explicit span when available.
#[allow(unexpected_cfgs)]
#[cfg(not(rust_analyzer))]
pub fn source_info_for_span(span: Span) -> Option<(String, usize)> {
    if force_missing_source_info() {
        return None;
    }

    let file_path = span
        .local_file()
        .map(|path| path.to_string_lossy().into_owned())
        .or_else(|| {
            let file_path = span.file();
            (!file_path.is_empty()).then_some(file_path)
        })?;

    let file_path = normalize_file_path(&file_path);
    source_file_exists(&file_path).then_some((file_path, span.start().line))
}

/// Returns `(file_path, line_number)` for an explicit span, falling back to the
/// current macro call-site when the span does not carry usable source info.
pub fn source_info_for_span_or_callsite(span: Span) -> Option<(String, usize)> {
    match source_info_for_span(span) {
        Some((file_path, line_number)) if line_number > 0 => Some((file_path, line_number)),
        Some((file_path, _)) => current_source_info().or(Some((file_path, 0))),
        None => current_source_info(),
    }
}

/// Returns the best-effort source file for the current macro call-site.
pub fn current_source_file() -> Option<String> {
    current_source_info().map(|(file_path, _)| file_path)
}

/// Returns the best-effort module path for the current macro call-site.
pub fn current_module_path_opt() -> Option<String> {
    let (file_path, line_number) = current_source_info()?;
    module_path_for_line(&file_path, line_number)
}

/// Returns the best-effort module path for the current macro call-site file at `line_number`.
pub fn current_module_path_at_line(line_number: usize) -> Option<String> {
    let file_path = current_source_file()?;
    module_path_for_line(&file_path, line_number)
}

/// Returns the best-effort module path for an explicit span or, if needed, the
/// current macro call-site.
pub fn module_path_for_span(span: Span) -> Option<String> {
    if force_missing_module_path() {
        return None;
    }

    let (file_path, line_number) = source_info_for_span_or_callsite(span)?;
    if line_number == 0 {
        return None;
    }
    module_path_for_line(&file_path, line_number)
}

/// Returns the best-effort module path for the current macro call-site.
pub fn current_module_path() -> String {
    current_module_path_opt().unwrap_or_else(get_pseudo_module_path)
}

/// Resolves the module path for a specific source file and line number.
pub fn module_path_for_line(file_path: &str, line_number: usize) -> Option<String> {
    find_module_path(file_path, line_number)
}

pub fn source_context_for_span_or_callsite(span: Span) -> SourceContext {
    let source_info = source_info_for_span_or_callsite(span);
    let module_path = if force_missing_module_path() {
        None
    } else {
        source_info
            .as_ref()
            .and_then(|(file_path, line_number)| {
                (*line_number > 0).then(|| module_path_for_line(file_path, *line_number))
            })
            .flatten()
    };

    SourceContext {
        file_path: source_info.as_ref().map(|(file_path, _)| file_path.clone()),
        line_number: source_info
            .map(|(_, line_number)| line_number)
            .unwrap_or_default(),
        module_path,
    }
}

pub fn best_effort_source_context_for_span_or_callsite(span: Span) -> SourceContext {
    let strict = source_context_for_span_or_callsite(span);
    if strict.quality() == SourceContextQuality::Complete {
        return strict;
    }

    let current_source = current_source_info();
    let file_path = strict.file_path.clone().or_else(|| {
        current_source
            .as_ref()
            .map(|(file_path, _)| file_path.clone())
    });
    let line_number = if strict.line_number > 0 {
        strict.line_number
    } else {
        current_source
            .as_ref()
            .map(|(_, line_number)| *line_number)
            .unwrap_or_default()
    };
    let module_path = if force_missing_module_path() {
        None
    } else {
        strict
            .module_path
            .clone()
            .or_else(|| {
                file_path.as_deref().and_then(|file_path| {
                    if line_number > 0 {
                        module_path_for_line(file_path, line_number)
                    } else {
                        None
                    }
                })
            })
            .or_else(|| file_path.as_deref().map(module_path_from_file))
            .or_else(current_module_path_opt)
            .or_else(|| Some(get_pseudo_module_path()))
    };

    SourceContext {
        file_path,
        line_number,
        module_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("statum_callsite_{label}_{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    #[test]
    fn module_path_for_line_resolves_raw_identifier_modules() {
        let crate_dir = unique_temp_dir("raw_ident_mods");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");

        write_file(
            &lib,
            "pub(crate) mod r#async {\n    pub mod r#type {\n        pub fn marker() {}\n    }\n}\n",
        );

        let found = module_path_for_line(&lib.to_string_lossy(), 3);
        assert_eq!(found.as_deref(), Some("r#async::r#type"));

        let _ = fs::remove_dir_all(crate_dir);
    }
}
