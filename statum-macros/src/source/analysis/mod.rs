use std::fs;
use std::rc::Rc;

mod collect;
mod declarations;
mod model;

pub(crate) use model::FileAnalysis;

use collect::collect_items;
use declarations::scan_declaration_lines;

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
        assert_eq!(analysis.enums[0].line_number, 3);
        assert_eq!(analysis.structs[0].line_number, 8);

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
        assert_eq!(analysis.enums[0].item.ident, "TaskState");
        assert_eq!(analysis.structs[0].item.ident, "TaskMachine");

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
    }
}
