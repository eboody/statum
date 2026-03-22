use std::fs;
use std::rc::Rc;

/// Enum entry extracted from a parsed source file.
#[derive(Clone)]
pub struct EnumEntry {
    pub item: syn::ItemEnum,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

/// Struct entry extracted from a parsed source file.
#[derive(Clone)]
pub struct StructEntry {
    pub item: syn::ItemStruct,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

/// Impl entry extracted from a parsed source file.
#[derive(Clone)]
pub struct ImplEntry {
    pub item: syn::ItemImpl,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

/// Parsed representation of enums, structs, and impls in one source file.
#[derive(Clone, Default)]
pub struct FileAnalysis {
    pub enums: Vec<EnumEntry>,
    pub structs: Vec<StructEntry>,
    pub impls: Vec<ImplEntry>,
}

/// Returns parsed analysis for `file_path`.
pub fn get_file_analysis(file_path: &str) -> Option<Rc<FileAnalysis>> {
    Some(Rc::new(build_file_analysis(file_path)?))
}

fn build_file_analysis(file_path: &str) -> Option<FileAnalysis> {
    let contents = fs::read_to_string(file_path).ok()?;
    let parsed = syn::parse_file(&contents).ok()?;
    let mut analysis = FileAnalysis::default();
    let mut next_search_line = 1usize;

    collect_items(
        parsed.items,
        &contents,
        &mut analysis,
        &mut next_search_line,
    )?;

    Some(analysis)
}

fn collect_items(
    items: Vec<syn::Item>,
    contents: &str,
    analysis: &mut FileAnalysis,
    next_search_line: &mut usize,
) -> Option<()> {
    for item in items {
        match item {
            syn::Item::Enum(item_enum) => {
                let name = item_enum.ident.to_string();
                let line_number = find_item_line_from(contents, "enum", &name, *next_search_line)?;
                *next_search_line = line_number.saturating_add(1);
                analysis.enums.push(EnumEntry {
                    attrs: attribute_names(&item_enum.attrs),
                    item: item_enum,
                    line_number,
                });
            }
            syn::Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let line_number =
                    find_item_line_from(contents, "struct", &name, *next_search_line)?;
                *next_search_line = line_number.saturating_add(1);
                analysis.structs.push(StructEntry {
                    attrs: attribute_names(&item_struct.attrs),
                    item: item_struct,
                    line_number,
                });
            }
            syn::Item::Impl(item_impl) => {
                let line_number = find_impl_line_from(contents, *next_search_line)?;
                *next_search_line = line_number.saturating_add(1);
                analysis.impls.push(ImplEntry {
                    attrs: attribute_names(&item_impl.attrs),
                    item: item_impl,
                    line_number,
                });
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, nested_items)) = item_mod.content {
                    collect_items(nested_items, contents, analysis, next_search_line)?;
                }
            }
            _ => {}
        }
    }

    Some(())
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

fn find_item_line_from(contents: &str, kind: &str, item_name: &str, start_line: usize) -> Option<usize> {
    for (idx, line) in contents
        .lines()
        .enumerate()
        .skip(start_line.saturating_sub(1))
    {
        let trimmed = line.trim_start();
        if line_starts_item_decl(trimmed, kind, item_name) {
            return Some(idx + 1);
        }
    }

    None
}

fn find_impl_line_from(contents: &str, start_line: usize) -> Option<usize> {
    for (idx, line) in contents
        .lines()
        .enumerate()
        .skip(start_line.saturating_sub(1))
    {
        let trimmed = line.trim_start();
        if line_starts_impl_decl(trimmed) {
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

fn line_starts_impl_decl(line: &str) -> bool {
    let mut rest = line.trim_start();

    if let Some(after_default) = consume_keyword(rest, "default") {
        rest = after_default.trim_start();
    }

    if let Some(after_unsafe) = consume_keyword(rest, "unsafe") {
        rest = after_unsafe.trim_start();
    }

    let Some(after_impl) = consume_keyword(rest, "impl") else {
        return false;
    };

    after_impl
        .chars()
        .next()
        .is_none_or(|ch| ch.is_whitespace() || matches!(ch, '<'))
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

impl MyMachine<MyState> {
    fn run(self) -> Self {
        self
    }
}
"#,
        );

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        assert_eq!(analysis.enums.len(), 1);
        assert_eq!(analysis.structs.len(), 1);
        assert_eq!(analysis.impls.len(), 1);
        assert!(analysis.enums[0].line_number > 0);
        assert!(analysis.structs[0].line_number > 0);
        assert!(analysis.impls[0].line_number > 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn get_file_analysis_reparses_each_request() {
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

        assert!(!Rc::ptr_eq(&a_first, &a_second));
        assert!(!Rc::ptr_eq(&a_first, &b_first));

        let _ = fs::remove_file(path_a);
        let _ = fs::remove_file(path_b);
    }

    #[test]
    fn collects_items_from_inline_modules() {
        let path = write_temp_rust_file(
            r#"
mod workflow {
    #[state]
    enum TaskState {
        Draft,
    }

    #[machine]
    struct TaskMachine<TaskState> {
        id: u64,
    }
}
"#,
        );

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        assert_eq!(analysis.enums.len(), 1);
        assert_eq!(analysis.structs.len(), 1);
        assert_eq!(analysis.impls.len(), 0);
        assert_eq!(analysis.enums[0].item.ident, "TaskState");
        assert_eq!(analysis.structs[0].item.ident, "TaskMachine");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn collects_impl_blocks_from_inline_modules() {
        let path = write_temp_rust_file(
            r#"
mod workflow {
    #[transition]
    impl Machine<State> {
        fn run(self) -> Self {
            self
        }
    }
}
"#,
        );

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        assert_eq!(analysis.impls.len(), 1);
        assert!(analysis.impls[0]
            .attrs
            .iter()
            .any(|attr| attr == "transition"));
        assert!(analysis.impls[0].line_number > 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn collects_distinct_line_numbers_for_same_named_structs_in_sibling_modules() {
        let path = write_temp_rust_file(
            r#"
mod shared {
    pub struct Payload {
        id: u64,
    }
}

mod workflow {
    pub struct Payload {
        id: u64,
    }
}
"#,
        );

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        let payload_lines = analysis
            .structs
            .iter()
            .filter(|entry| entry.item.ident == "Payload")
            .map(|entry| entry.line_number)
            .collect::<Vec<_>>();

        assert_eq!(payload_lines.len(), 2);
        assert_ne!(payload_lines[0], payload_lines[1]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn get_file_analysis_reflects_file_changes() {
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

        assert!(line_starts_impl_decl("impl Machine<State> {"));
        assert!(line_starts_impl_decl(
            "unsafe impl Trait for Machine<State> {"
        ));
        assert!(line_starts_impl_decl(
            "default impl Trait for Machine<State> {"
        ));
        assert!(!line_starts_impl_decl("simple_machine_impl();"));
    }
}
