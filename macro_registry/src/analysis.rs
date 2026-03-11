use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

use crate::cache::{file_fingerprint, fresh_cached_value, CachedValue};

/// Cached enum entry extracted from a parsed source file.
#[derive(Clone)]
pub struct EnumEntry {
    pub item: syn::ItemEnum,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

/// Cached struct entry extracted from a parsed source file.
#[derive(Clone)]
pub struct StructEntry {
    pub item: syn::ItemStruct,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

/// Parsed/cached representation of enums and structs in one source file.
#[derive(Clone, Default)]
pub struct FileAnalysis {
    pub enums: Vec<EnumEntry>,
    pub structs: Vec<StructEntry>,
}

type CachedFileAnalysis = CachedValue<Rc<FileAnalysis>>;

thread_local! {
    static FILE_ANALYSIS_CACHE: RefCell<HashMap<String, CachedFileAnalysis>> = RefCell::new(HashMap::new());
}

/// Returns cached analysis for `file_path`, parsing and caching on first access.
pub fn get_file_analysis(file_path: &str) -> Option<Rc<FileAnalysis>> {
    let fingerprint = file_fingerprint(file_path)?;

    if let Some(cached) = fresh_cached_value(
        FILE_ANALYSIS_CACHE.with(|cache| cache.borrow().get(file_path).cloned()),
        fingerprint,
    ) {
        return Some(cached);
    }

    let analysis = Rc::new(build_file_analysis(file_path)?);
    FILE_ANALYSIS_CACHE.with(|cache| {
        cache.borrow_mut().insert(
            file_path.to_string(),
            CachedFileAnalysis::new(fingerprint, analysis.clone()),
        );
    });
    Some(analysis)
}

fn build_file_analysis(file_path: &str) -> Option<FileAnalysis> {
    let contents = fs::read_to_string(file_path).ok()?;
    let parsed = syn::parse_file(&contents).ok()?;
    let mut analysis = FileAnalysis::default();

    for item in parsed.items {
        match item {
            syn::Item::Enum(item_enum) => {
                let name = item_enum.ident.to_string();
                let span_line = item_enum.ident.span().start().line;
                let line_number = if span_line > 0 {
                    span_line
                } else {
                    find_item_line(&contents, "enum", &name)?
                };
                analysis.enums.push(EnumEntry {
                    attrs: attribute_names(&item_enum.attrs),
                    item: item_enum,
                    line_number,
                });
            }
            syn::Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let span_line = item_struct.ident.span().start().line;
                let line_number = if span_line > 0 {
                    span_line
                } else {
                    find_item_line(&contents, "struct", &name)?
                };
                analysis.structs.push(StructEntry {
                    attrs: attribute_names(&item_struct.attrs),
                    item: item_struct,
                    line_number,
                });
            }
            _ => {}
        }
    }

    Some(analysis)
}

fn attribute_names(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut names = Vec::new();

    for attr in attrs {
        let Some(ident) = attr.path().get_ident() else {
            continue;
        };
        let name = ident.to_string();
        if !names.iter().any(|existing| existing == &name) {
            names.push(name);
        }
    }

    names
}

fn find_item_line(contents: &str, kind: &str, item_name: &str) -> Option<usize> {
    for (idx, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if line_starts_item_decl(trimmed, kind, item_name) {
            return Some(idx + 1);
        }
    }

    None
}

fn line_starts_item_decl(line: &str, kind: &str, item_name: &str) -> bool {
    let mut rest = line.trim_start();

    if let Some(after_pub) = consume_keyword(rest, "pub") {
        rest = after_pub.trim_start();
        if rest.starts_with('(') {
            let Some(after_vis) = consume_parenthesized(rest) else {
                return false;
            };
            rest = after_vis.trim_start();
        }
    }

    if let Some(after_unsafe) = consume_keyword(rest, "unsafe") {
        rest = after_unsafe.trim_start();
    }

    let Some(after_kind) = consume_keyword(rest, kind) else {
        return false;
    };
    let rest = after_kind.trim_start();
    let Some(after_name) = rest.strip_prefix(item_name) else {
        return false;
    };

    after_name
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || matches!(ch, '<' | '{' | '(' | ';' | ':'))
}

fn consume_keyword<'a>(input: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = input.strip_prefix(keyword)?;
    if rest
        .chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return None;
    }
    Some(rest)
}

fn consume_parenthesized(input: &str) -> Option<&str> {
    let mut chars = input.char_indices();
    let Some((_, '(')) = chars.next() else {
        return None;
    };

    let mut depth = 1usize;
    for (idx, ch) in chars {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&input[idx + 1..]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_rust_file(contents: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("statum_analysis_{nanos}.rs"));
        fs::write(&path, contents).expect("write temp file");
        path
    }

    #[test]
    fn parses_line_numbers_for_struct_and_enum() {
        let path = write_temp_rust_file(
            r#"
#[state]
pub(crate) enum MyState {
    A,
}

#[machine]
pub struct MyMachine<MyState> {
    id: u64,
}
"#,
        );

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        assert_eq!(analysis.enums.len(), 1);
        assert_eq!(analysis.structs.len(), 1);
        assert!(analysis.enums[0].line_number > 0);
        assert!(analysis.structs[0].line_number > 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_analysis_cache_is_scoped_per_file_path() {
        let path_a = write_temp_rust_file(
            r#"
#[state]
enum StateA { A }
"#,
        );
        let path_b = write_temp_rust_file(
            r#"
#[state]
enum StateB { B }
"#,
        );

        let a_first = get_file_analysis(path_a.to_str().expect("a path")).expect("analysis a1");
        let a_second = get_file_analysis(path_a.to_str().expect("a path")).expect("analysis a2");
        let b_first = get_file_analysis(path_b.to_str().expect("b path")).expect("analysis b1");

        assert!(Rc::ptr_eq(&a_first, &a_second));
        assert!(!Rc::ptr_eq(&a_first, &b_first));

        let _ = fs::remove_file(path_a);
        let _ = fs::remove_file(path_b);
    }

    #[test]
    fn analysis_cache_reuses_when_file_unchanged() {
        let path = write_temp_rust_file(
            r#"
#[state]
enum ReuseState { A }
"#,
        );
        let path_str = path.to_str().expect("path").to_string();

        let first = get_file_analysis(&path_str).expect("analysis first");
        let second = get_file_analysis(&path_str).expect("analysis second");

        assert!(Rc::ptr_eq(&first, &second));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn analysis_cache_invalidates_when_file_changes() {
        let path = write_temp_rust_file(
            r#"
#[state]
enum BeforeState { A }
"#,
        );
        let path_str = path.to_str().expect("path").to_string();

        let first = get_file_analysis(&path_str).expect("analysis first");
        assert_eq!(first.enums.len(), 1);
        assert_eq!(first.enums[0].item.ident.to_string(), "BeforeState");

        // Give coarse filesystems time to advance mtime.
        thread::sleep(Duration::from_millis(2));
        fs::write(
            &path,
            r#"
#[state]
enum ChangedState { A, B }
"#,
        )
        .expect("rewrite file");

        let second = get_file_analysis(&path_str).expect("analysis second");
        assert!(!Rc::ptr_eq(&first, &second));
        assert_eq!(second.enums.len(), 1);
        assert_eq!(second.enums[0].item.ident.to_string(), "ChangedState");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn fallback_line_match_handles_visibility_modifiers() {
        assert!(line_starts_item_decl(
            "pub(crate) struct Machine<State> {",
            "struct",
            "Machine",
        ));
        assert!(line_starts_item_decl(
            "pub(in crate::x) enum WorkflowState {",
            "enum",
            "WorkflowState",
        ));
        assert!(!line_starts_item_decl(
            "pub(crate) struct NotMachine<State> {",
            "struct",
            "Machine",
        ));
    }
}
