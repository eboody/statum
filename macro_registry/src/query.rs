use crate::analysis::get_file_analysis;
use crate::callsite::module_path_for_line;

/// Item kinds discoverable through file analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Enum,
    Struct,
}

/// A named item discovered in a source file with its resolved module path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemCandidate {
    pub name: String,
    pub line_number: usize,
    pub module_path: String,
}

/// Returns candidates of `kind` in `module_path`, optionally requiring `required_attr`.
pub fn candidates_in_module(
    file_path: &str,
    module_path: &str,
    kind: ItemKind,
    required_attr: Option<&str>,
) -> Vec<ItemCandidate> {
    let Some(analysis) = get_file_analysis(file_path) else {
        return Vec::new();
    };

    let mut candidates = collect_candidates(&analysis, file_path, kind, None, required_attr)
        .into_iter()
        .filter(|candidate| candidate.module_path == module_path)
        .collect::<Vec<_>>();
    sort_and_dedup(&mut candidates);
    candidates
}

/// Returns same-named candidates in other modules of the same file.
pub fn same_named_candidates_elsewhere(
    file_path: &str,
    module_path: &str,
    kind: ItemKind,
    item_name: &str,
    required_attr: Option<&str>,
) -> Vec<ItemCandidate> {
    let Some(analysis) = get_file_analysis(file_path) else {
        return Vec::new();
    };

    let mut candidates =
        collect_candidates(&analysis, file_path, kind, Some(item_name), required_attr)
            .into_iter()
            .filter(|candidate| candidate.module_path != module_path)
            .collect::<Vec<_>>();
    sort_and_dedup(&mut candidates);
    candidates
}

/// Returns the line number for a plain item in `module_path` that does not carry `excluded_attr`.
pub fn plain_item_line_in_module(
    file_path: &str,
    module_path: &str,
    kind: ItemKind,
    item_name: &str,
    excluded_attr: Option<&str>,
) -> Option<usize> {
    let analysis = get_file_analysis(file_path)?;

    match kind {
        ItemKind::Enum => analysis.enums.iter().find_map(|entry| {
            (entry.item.ident == item_name
                && module_path_for_line(file_path, entry.line_number).as_deref()
                    == Some(module_path)
                && !has_attr(&entry.attrs, excluded_attr))
            .then_some(entry.line_number)
        }),
        ItemKind::Struct => analysis.structs.iter().find_map(|entry| {
            (entry.item.ident == item_name
                && module_path_for_line(file_path, entry.line_number).as_deref()
                    == Some(module_path)
                && !has_attr(&entry.attrs, excluded_attr))
            .then_some(entry.line_number)
        }),
    }
}

/// Formats candidates for user-facing diagnostics.
pub fn format_candidates(candidates: &[ItemCandidate]) -> String {
    candidates
        .iter()
        .map(|candidate| {
            format!(
                "`{}` in `{}` (line {})",
                candidate.name, candidate.module_path, candidate.line_number
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn collect_candidates(
    analysis: &crate::analysis::FileAnalysis,
    file_path: &str,
    kind: ItemKind,
    name_filter: Option<&str>,
    required_attr: Option<&str>,
) -> Vec<ItemCandidate> {
    match kind {
        ItemKind::Enum => analysis
            .enums
            .iter()
            .filter(|entry| name_filter.is_none_or(|name| entry.item.ident == name))
            .filter(|entry| has_attr(&entry.attrs, required_attr))
            .filter_map(|entry| {
                candidate_from_line(file_path, entry.item.ident.to_string(), entry.line_number)
            })
            .collect(),
        ItemKind::Struct => analysis
            .structs
            .iter()
            .filter(|entry| name_filter.is_none_or(|name| entry.item.ident == name))
            .filter(|entry| has_attr(&entry.attrs, required_attr))
            .filter_map(|entry| {
                candidate_from_line(file_path, entry.item.ident.to_string(), entry.line_number)
            })
            .collect(),
    }
}

fn candidate_from_line(file_path: &str, name: String, line_number: usize) -> Option<ItemCandidate> {
    let module_path = module_path_for_line(file_path, line_number)?;
    Some(ItemCandidate {
        name,
        line_number,
        module_path,
    })
}

fn has_attr(attrs: &[String], required_attr: Option<&str>) -> bool {
    required_attr.is_none_or(|attr| attrs.iter().any(|candidate| candidate == attr))
}

fn sort_and_dedup(candidates: &mut Vec<ItemCandidate>) {
    candidates.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.module_path.cmp(&right.module_path))
            .then(left.line_number.cmp(&right.line_number))
    });
    candidates
        .dedup_by(|left, right| left.name == right.name && left.line_number == right.line_number);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_rust_file(contents: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let crate_dir = std::env::temp_dir().join(format!("statum_query_{nanos}"));
        let src_dir = crate_dir.join("src");
        fs::create_dir_all(&src_dir).expect("create temp crate");
        let path = src_dir.join("lib.rs");
        fs::write(&path, contents).expect("write temp file");
        path
    }

    #[test]
    fn candidates_in_module_filters_by_kind_module_and_attr() {
        let path = write_temp_rust_file(
            r#"
mod alpha {
    #[machine]
    pub struct Machine<State> {
        id: u64,
    }
}

mod beta {
    #[machine]
    pub struct Machine<State> {
        id: u64,
    }

    pub struct PlainMachine<State> {
        id: u64,
    }
}
"#,
        );

        let candidates = candidates_in_module(
            path.to_str().expect("path"),
            "beta",
            ItemKind::Struct,
            Some("machine"),
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "Machine");
        assert_eq!(candidates[0].module_path, "beta");

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }

    #[test]
    fn same_named_candidates_elsewhere_returns_other_modules_only() {
        let path = write_temp_rust_file(
            r#"
mod alpha {
    #[machine]
    pub struct Machine<State> {
        id: u64,
    }
}

mod beta {
    #[machine]
    pub struct Machine<State> {
        id: u64,
    }
}
"#,
        );

        let candidates = same_named_candidates_elsewhere(
            path.to_str().expect("path"),
            "beta",
            ItemKind::Struct,
            "Machine",
            Some("machine"),
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].module_path, "alpha");

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }

    #[test]
    fn plain_item_line_in_module_ignores_annotated_items() {
        let path = write_temp_rust_file(
            r#"
mod beta {
    #[state]
    pub enum State {
        Ready,
    }

    pub enum PlainState {
        Ready,
    }
}
"#,
        );

        let plain_line = plain_item_line_in_module(
            path.to_str().expect("path"),
            "beta",
            ItemKind::Enum,
            "PlainState",
            Some("state"),
        );
        let annotated_line = plain_item_line_in_module(
            path.to_str().expect("path"),
            "beta",
            ItemKind::Enum,
            "State",
            Some("state"),
        );

        assert!(plain_line.is_some());
        assert_eq!(annotated_line, None);

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }
}
