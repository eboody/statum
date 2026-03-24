use std::collections::VecDeque;
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

#[derive(Default)]
struct DeclarationLines {
    enums: VecDeque<usize>,
    structs: VecDeque<usize>,
    impls: VecDeque<usize>,
}

#[derive(Clone, Debug)]
struct ScannedToken {
    kind: TokenKind,
    line: usize,
}

#[derive(Clone, Debug)]
enum TokenKind {
    Ident(String),
    Punct(char),
}

#[derive(Clone, Debug)]
enum BlockContext {
    Module { close: char },
    Opaque { close: char },
    Normal { close: char },
}

/// Returns parsed analysis for `file_path`.
pub fn get_file_analysis(file_path: &str) -> Option<Rc<FileAnalysis>> {
    Some(Rc::new(build_file_analysis(file_path)?))
}

fn build_file_analysis(file_path: &str) -> Option<FileAnalysis> {
    let content = fs::read_to_string(file_path).ok()?;
    let parsed = syn::parse_file(&content).ok()?;
    let mut lines = scan_declaration_lines(&content);
    let mut analysis = FileAnalysis::default();
    collect_items(parsed.items, &mut analysis, &mut lines)?;
    Some(analysis)
}

fn collect_items(
    items: Vec<syn::Item>,
    analysis: &mut FileAnalysis,
    lines: &mut DeclarationLines,
) -> Option<()> {
    for item in items {
        match item {
            syn::Item::Enum(item_enum) => {
                analysis.enums.push(EnumEntry {
                    attrs: attribute_names(&item_enum.attrs),
                    line_number: lines.enums.pop_front()?,
                    item: item_enum,
                });
            }
            syn::Item::Struct(item_struct) => {
                analysis.structs.push(StructEntry {
                    attrs: attribute_names(&item_struct.attrs),
                    line_number: lines.structs.pop_front()?,
                    item: item_struct,
                });
            }
            syn::Item::Impl(item_impl) => {
                analysis.impls.push(ImplEntry {
                    attrs: attribute_names(&item_impl.attrs),
                    line_number: lines.impls.pop_front()?,
                    item: item_impl,
                });
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, nested_items)) = item_mod.content {
                    collect_items(nested_items, analysis, lines)?;
                }
            }
            _ => {}
        }
    }

    Some(())
}

fn scan_declaration_lines(content: &str) -> DeclarationLines {
    let tokens = tokenize_source(content);
    let mut lines = DeclarationLines::default();
    let mut block_stack = Vec::new();
    let mut opaque_depth = 0usize;
    let mut pending_module_open = None::<usize>;
    let mut pending_opaque_open = None::<usize>;

    for (index, token) in tokens.iter().enumerate() {
        match &token.kind {
            TokenKind::Ident(keyword)
                if opaque_depth == 0 && module_items_allowed(&block_stack) =>
            {
                match keyword.as_str() {
                    "enum" => lines.enums.push_back(token.line),
                    "struct" => lines.structs.push_back(token.line),
                    "impl" => lines.impls.push_back(token.line),
                    "mod" => {
                        if matches!(
                            tokens.get(index + 1),
                            Some(ScannedToken {
                                kind: TokenKind::Ident(_),
                                ..
                            })
                        ) && matches!(
                            tokens.get(index + 2),
                            Some(ScannedToken {
                                kind: TokenKind::Punct('{'),
                                ..
                            })
                        ) {
                            pending_module_open = Some(index + 2);
                        }
                    }
                    _ => {}
                }
            }
            TokenKind::Punct('!') if opaque_depth == 0 => {
                if matches!(
                    tokens.get(index + 1),
                    Some(ScannedToken {
                        kind: TokenKind::Punct('{') | TokenKind::Punct('(') | TokenKind::Punct('['),
                        ..
                    })
                ) {
                    pending_opaque_open = Some(index + 1);
                } else if matches!(
                    tokens.get(index + 1),
                    Some(ScannedToken {
                        kind: TokenKind::Ident(_),
                        ..
                    })
                ) && matches!(
                    tokens.get(index + 2),
                    Some(ScannedToken {
                        kind: TokenKind::Punct('{') | TokenKind::Punct('(') | TokenKind::Punct('['),
                        ..
                    })
                ) {
                    pending_opaque_open = Some(index + 2);
                }
            }
            TokenKind::Punct(open @ ('{' | '(' | '[')) => {
                if pending_opaque_open == Some(index) {
                    block_stack.push(BlockContext::Opaque {
                        close: matching_close(*open),
                    });
                    opaque_depth += 1;
                    pending_opaque_open = None;
                } else if pending_module_open == Some(index) {
                    block_stack.push(BlockContext::Module {
                        close: matching_close(*open),
                    });
                    pending_module_open = None;
                } else {
                    block_stack.push(BlockContext::Normal {
                        close: matching_close(*open),
                    });
                }
            }
            TokenKind::Punct(close @ ('}' | ')' | ']')) => {
                let Some(context) = block_stack.pop() else {
                    continue;
                };

                match context {
                    BlockContext::Module {
                        close: expected_close,
                    } if expected_close == *close => {}
                    BlockContext::Opaque {
                        close: expected_close,
                    } if expected_close == *close => {
                        opaque_depth = opaque_depth.saturating_sub(1);
                    }
                    BlockContext::Normal {
                        close: expected_close,
                    } if expected_close == *close => {}
                    BlockContext::Module { .. }
                    | BlockContext::Opaque { .. }
                    | BlockContext::Normal { .. } => {}
                }
            }
            _ => {}
        }
    }

    lines
}

fn module_items_allowed(block_stack: &[BlockContext]) -> bool {
    block_stack
        .iter()
        .all(|context| matches!(context, BlockContext::Module { .. }))
}

fn tokenize_source(content: &str) -> Vec<ScannedToken> {
    let chars: Vec<char> = content.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    let mut line = 1usize;

    while index < chars.len() {
        let ch = chars[index];
        match ch {
            '\n' => {
                line += 1;
                index += 1;
            }
            c if c.is_whitespace() => index += 1,
            '/' if chars.get(index + 1) == Some(&'/') => {
                index += 2;
                while let Some(next) = chars.get(index) {
                    if *next == '\n' {
                        break;
                    }
                    index += 1;
                }
            }
            '/' if chars.get(index + 1) == Some(&'*') => {
                let (next_index, next_line) = skip_block_comment(&chars, index, line);
                index = next_index;
                line = next_line;
            }
            '"' => {
                let (next_index, next_line) = skip_quoted_literal(&chars, index, line, '"');
                index = next_index;
                line = next_line;
            }
            '\'' if is_char_literal_start(&chars, index) => {
                let (next_index, next_line) = skip_quoted_literal(&chars, index, line, '\'');
                index = next_index;
                line = next_line;
            }
            'b' => {
                if chars.get(index + 1) == Some(&'"') {
                    let (next_index, next_line) = skip_quoted_literal(&chars, index + 1, line, '"');
                    index = next_index;
                    line = next_line;
                } else if chars.get(index + 1) == Some(&'\'')
                    && is_char_literal_start(&chars, index + 1)
                {
                    let (next_index, next_line) =
                        skip_quoted_literal(&chars, index + 1, line, '\'');
                    index = next_index;
                    line = next_line;
                } else if is_raw_string_start(&chars, index + 1) {
                    let (next_index, next_line) = skip_raw_string_literal(&chars, index + 1, line);
                    index = next_index;
                    line = next_line;
                } else {
                    let (ident, next_index) = read_identifier(&chars, index);
                    tokens.push(ScannedToken {
                        kind: TokenKind::Ident(ident),
                        line,
                    });
                    index = next_index;
                }
            }
            'r' if is_raw_string_start(&chars, index) => {
                let (next_index, next_line) = skip_raw_string_literal(&chars, index, line);
                index = next_index;
                line = next_line;
            }
            'r' if chars.get(index + 1) == Some(&'#')
                && chars
                    .get(index + 2)
                    .is_some_and(|next| is_ident_start(*next)) =>
            {
                let (ident, next_index) = read_raw_identifier(&chars, index);
                tokens.push(ScannedToken {
                    kind: TokenKind::Ident(ident),
                    line,
                });
                index = next_index;
            }
            c if is_ident_start(c) => {
                let (ident, next_index) = read_identifier(&chars, index);
                tokens.push(ScannedToken {
                    kind: TokenKind::Ident(ident),
                    line,
                });
                index = next_index;
            }
            '{' | '}' | '(' | ')' | '[' | ']' | '!' => {
                tokens.push(ScannedToken {
                    kind: TokenKind::Punct(ch),
                    line,
                });
                index += 1;
            }
            _ => index += 1,
        }
    }

    tokens
}

fn skip_block_comment(chars: &[char], start: usize, line: usize) -> (usize, usize) {
    let mut index = start + 2;
    let mut depth = 1usize;
    let mut current_line = line;

    while index < chars.len() {
        match (chars[index], chars.get(index + 1).copied()) {
            ('/', Some('*')) => {
                depth += 1;
                index += 2;
            }
            ('*', Some('/')) => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    break;
                }
            }
            ('\n', _) => {
                current_line += 1;
                index += 1;
            }
            _ => index += 1,
        }
    }

    (index, current_line)
}

fn skip_quoted_literal(
    chars: &[char],
    start: usize,
    line: usize,
    terminator: char,
) -> (usize, usize) {
    let mut index = start + 1;
    let mut current_line = line;
    let mut escaped = false;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' {
            current_line += 1;
        }

        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        match ch {
            '\\' => {
                escaped = true;
                index += 1;
            }
            c if c == terminator => {
                index += 1;
                break;
            }
            _ => index += 1,
        }
    }

    (index, current_line)
}

fn skip_raw_string_literal(chars: &[char], start: usize, line: usize) -> (usize, usize) {
    let mut index = start;
    let mut current_line = line;

    if chars.get(index) == Some(&'b') {
        index += 1;
    }
    index += 1; // skip `r`

    let mut hashes = 0usize;
    while chars.get(index) == Some(&'#') {
        hashes += 1;
        index += 1;
    }

    if chars.get(index) != Some(&'"') {
        return (index, current_line);
    }
    index += 1;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' {
            current_line += 1;
            index += 1;
            continue;
        }

        if ch == '"' {
            let mut cursor = index + 1;
            let mut matched_hashes = 0usize;
            while matched_hashes < hashes && chars.get(cursor) == Some(&'#') {
                matched_hashes += 1;
                cursor += 1;
            }
            if matched_hashes == hashes {
                index = cursor;
                break;
            }
        }

        index += 1;
    }

    (index, current_line)
}

fn is_char_literal_start(chars: &[char], start: usize) -> bool {
    let mut index = start + 1;
    let mut escaped = false;

    while let Some(ch) = chars.get(index).copied() {
        if ch == '\n' {
            return false;
        }

        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        match ch {
            '\\' => {
                escaped = true;
                index += 1;
            }
            '\'' => return true,
            c if c.is_whitespace() => return false,
            _ => index += 1,
        }
    }

    false
}

fn is_raw_string_start(chars: &[char], start: usize) -> bool {
    if chars.get(start) != Some(&'r') {
        return false;
    }

    let mut index = start + 1;
    while chars.get(index) == Some(&'#') {
        index += 1;
    }

    chars.get(index) == Some(&'"')
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn read_identifier(chars: &[char], start: usize) -> (String, usize) {
    let mut index = start + 1;
    while chars
        .get(index)
        .is_some_and(|next| is_ident_continue(*next))
    {
        index += 1;
    }

    (chars[start..index].iter().collect(), index)
}

fn read_raw_identifier(chars: &[char], start: usize) -> (String, usize) {
    let mut index = start + 2;
    while chars
        .get(index)
        .is_some_and(|next| is_ident_continue(*next))
    {
        index += 1;
    }

    (chars[start..index].iter().collect(), index)
}

fn matching_close(open: char) -> char {
    match open {
        '{' => '}',
        '(' => ')',
        '[' => ']',
        _ => open,
    }
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
        assert_eq!(analysis.enums[0].line_number, 3);
        assert_eq!(analysis.structs[0].line_number, 8);
        assert_eq!(analysis.impls[0].line_number, 12);

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
        assert_eq!(analysis.impls[0].line_number, 4);

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

        assert_eq!(payload_lines, vec![3, 9]);

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
    fn line_numbers_ignore_comments_and_use_real_item_lines() {
        let path = write_temp_rust_file(
            r#"
mod comment_only {
    /*
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

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        assert_eq!(analysis.structs.len(), 1);
        assert_eq!(analysis.structs[0].item.ident, "Machine");
        assert_eq!(analysis.structs[0].line_number, 12);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn line_numbers_handle_split_item_declarations() {
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

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        assert_eq!(analysis.structs.len(), 1);
        assert_eq!(analysis.structs[0].item.ident, "Machine");
        assert_eq!(analysis.structs[0].line_number, 5);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn line_numbers_ignore_items_inside_macro_bodies() {
        let path = write_temp_rust_file(
            r#"
generated! {
    struct Fake<State> {
        id: u64,
    }
}

#[machine]
struct Real<State> {
    id: u64,
}
"#,
        );

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        assert_eq!(analysis.structs.len(), 1);
        assert_eq!(analysis.structs[0].item.ident, "Real");
        assert_eq!(analysis.structs[0].line_number, 9);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn line_numbers_ignore_enum_struct_and_impl_inside_macro_invocations_for_all_delimiters() {
        for (label, open, close) in [
            ("brace", "{", "}"),
            ("paren", "(", ")"),
            ("bracket", "[", "]"),
        ] {
            let lines = scan_declaration_lines(&format!(
                "generated!{open}\n    enum FakeState {{\n        Hidden,\n    }}\n\n    struct FakeMachine<FakeState> {{}}\n\n    impl FakeMachine<FakeState> {{\n        fn run(self) -> Self {{ self }}\n    }}\n{close};\n\nenum RealState {{\n    Ready,\n}}\n\nstruct RealMachine<RealState> {{}}\n\nimpl RealMachine<RealState> {{\n    fn run(self) -> Self {{ self }}\n}}\n"
            ));
            assert_eq!(
                lines.enums.into_iter().collect::<Vec<_>>(),
                vec![13],
                "enum lines for {label} delimiter"
            );
            assert_eq!(
                lines.structs.into_iter().collect::<Vec<_>>(),
                vec![17],
                "struct lines for {label} delimiter"
            );
            assert_eq!(
                lines.impls.into_iter().collect::<Vec<_>>(),
                vec![19],
                "impl lines for {label} delimiter"
            );
        }
    }

    #[test]
    fn line_numbers_ignore_local_items_inside_function_bodies() {
        let lines = scan_declaration_lines(
            r#"
mod alpha {
    fn helper() {
        enum LocalState {
            Hidden,
        }

        struct LocalMachine<LocalState> {}

        impl LocalMachine<LocalState> {
            fn run(self) -> Self { self }
        }
    }
}

mod beta {
    enum RealState {
        Ready,
    }

    struct RealMachine<RealState> {}

    impl RealMachine<RealState> {
        fn run(self) -> Self { self }
    }
}
"#,
        );

        assert_eq!(lines.enums.into_iter().collect::<Vec<_>>(), vec![17]);
        assert_eq!(lines.structs.into_iter().collect::<Vec<_>>(), vec![21]);
        assert_eq!(lines.impls.into_iter().collect::<Vec<_>>(), vec![23]);
    }
}
