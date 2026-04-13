use super::analysis::{FileAnalysis, get_file_analysis};
use super::callsite::module_path_for_line;

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

/// A type alias discovered in source with its resolved module path.
#[derive(Clone)]
pub struct TypeAliasCandidate {
    pub item: syn::ItemType,
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

/// Returns type aliases of `alias_name` declared in `module_path`.
pub fn type_aliases_in_module(
    file_path: &str,
    module_path: &str,
    alias_name: &str,
) -> Vec<TypeAliasCandidate> {
    let Some(analysis) = get_file_analysis(file_path) else {
        return Vec::new();
    };

    let mut candidates = analysis
        .type_aliases
        .iter()
        .filter(|entry| entry.item.ident == alias_name)
        .filter_map(|entry| {
            let resolved_module = module_path_for_line(file_path, entry.line_number)?;
            (resolved_module == module_path).then(|| TypeAliasCandidate {
                item: entry.item.clone(),
                line_number: entry.line_number,
                module_path: resolved_module,
            })
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.item
            .ident
            .to_string()
            .cmp(&right.item.ident.to_string())
            .then(left.module_path.cmp(&right.module_path))
            .then(left.line_number.cmp(&right.line_number))
    });
    candidates.dedup_by(|left, right| {
        left.item.ident == right.item.ident
            && left.module_path == right.module_path
            && left.line_number == right.line_number
    });
    candidates
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
    analysis: &FileAnalysis,
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
    candidates.dedup_by(|left, right| {
        left.name == right.name
            && left.module_path == right.module_path
            && left.line_number == right.line_number
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
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

    fn write_temp_crate(files: &[(&str, &str)]) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let crate_dir = std::env::temp_dir().join(format!("statum_query_layout_{nanos}"));

        for (relative_path, contents) in files {
            let path = crate_dir.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create temp crate parent");
            }
            fs::write(path, contents).expect("write temp crate file");
        }

        crate_dir
    }

    fn remove_temp_crate(crate_dir: &Path) {
        let _ = fs::remove_dir_all(crate_dir);
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
    fn sort_and_dedup_keeps_same_line_candidates_in_distinct_modules() {
        let mut candidates = vec![
            ItemCandidate {
                name: "Machine".into(),
                line_number: 1,
                module_path: "beta".into(),
            },
            ItemCandidate {
                name: "Machine".into(),
                line_number: 1,
                module_path: "alpha".into(),
            },
            ItemCandidate {
                name: "Machine".into(),
                line_number: 1,
                module_path: "beta".into(),
            },
        ];

        sort_and_dedup(&mut candidates);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].module_path, "alpha");
        assert_eq!(candidates[1].module_path, "beta");
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

    #[test]
    fn candidates_in_module_ignores_comment_only_declarations() {
        let path = write_temp_rust_file(
            r#"
mod comment_only {
    /*
    #[machine]
    struct Machine<State> {
        id: u64,
    }
    */
}

mod workflow {
    #[machine]
    pub struct Machine<State> {
        id: u64,
    }
}
"#,
        );

        let candidates = candidates_in_module(
            path.to_str().expect("path"),
            "workflow",
            ItemKind::Struct,
            Some("machine"),
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "Machine");
        assert_eq!(candidates[0].module_path, "workflow");

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }

    #[test]
    fn candidates_in_module_handles_split_declaration_lines() {
        let path = write_temp_rust_file(
            r#"
mod workflow {
    #[machine]
    pub
    struct
    Machine<State> {
        id: u64,
    }
}
"#,
        );

        let candidates = candidates_in_module(
            path.to_str().expect("path"),
            "workflow",
            ItemKind::Struct,
            Some("machine"),
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "Machine");
        assert_eq!(candidates[0].line_number, 5);

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }

    #[test]
    fn candidates_in_module_ignores_local_same_named_items_in_other_modules() {
        let path = write_temp_rust_file(
            r#"
mod alpha {
    fn helper() {
        struct Machine<State> {
            _marker: core::marker::PhantomData<State>,
        }
    }
}

mod beta {
    #[machine]
    pub struct Machine<State> {
        _marker: core::marker::PhantomData<State>,
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
        assert_eq!(candidates[0].line_number, 12);

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }

    #[test]
    fn candidates_in_module_reads_external_module_rs_files() {
        let crate_dir = write_temp_crate(&[
            ("src/lib.rs", "mod flows;\n"),
            (
                "src/flows.rs",
                "#[machine]\npub struct WorkflowMachine<State> {\n    id: u64,\n}\n",
            ),
        ]);
        let flows = crate_dir.join("src").join("flows.rs");

        let candidates = candidates_in_module(
            flows.to_str().expect("path"),
            "flows",
            ItemKind::Struct,
            Some("machine"),
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].name, "WorkflowMachine");
        assert_eq!(candidates[0].module_path, "flows");
        assert_eq!(candidates[0].line_number, 2);

        remove_temp_crate(&crate_dir);
    }

    #[test]
    fn type_aliases_in_module_reads_external_mod_rs_files() {
        let crate_dir = write_temp_crate(&[
            ("src/lib.rs", "mod flows;\n"),
            (
                "src/flows/mod.rs",
                "pub type Next = crate::Flow<crate::Accepted>;\n",
            ),
        ]);
        let flows = crate_dir.join("src").join("flows").join("mod.rs");

        let aliases = type_aliases_in_module(flows.to_str().expect("path"), "flows", "Next");

        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].item.ident, "Next");
        assert_eq!(aliases[0].module_path, "flows");
        assert_eq!(aliases[0].line_number, 1);

        remove_temp_crate(&crate_dir);
    }
}
