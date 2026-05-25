mod machine_context;
mod shape;
mod strategy;

pub(super) use crate::source::{
    AliasResolutionContext, SourceAliasResolver, expand_source_type_alias,
};
pub(super) use shape::{
    SupportedWrapper, extract_first_generic_type_ref, extract_generic_type_refs,
    extract_impl_machine_and_state, machine_segment_matching_target, supported_wrapper, type_path,
};
pub(super) use machine_context::missing_transition_machine_context;
#[cfg_attr(not(test), allow(unused_imports))]
pub(super) use strategy::{
    collect_machine_and_states, collect_machine_and_states_in_context,
    collect_machine_and_states_strict, parse_machine_and_state,
    parse_machine_and_state_in_context, parse_primary_machine_and_state,
    parse_primary_machine_and_state_strict,
};

#[cfg(test)]
mod tests {
    use super::{
        AliasResolutionContext, collect_machine_and_states, collect_machine_and_states_in_context,
        extract_impl_machine_and_state, parse_machine_and_state,
        parse_machine_and_state_in_context, parse_primary_machine_and_state,
    };
    use crate::source::module_root_from_file;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use syn::Type;

    fn parse_type(source: &str) -> Type {
        syn::parse_str(source).expect("valid type")
    }

    fn write_temp_rust_file(contents: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let crate_dir = std::env::temp_dir().join(format!("statum_transition_alias_{nanos}"));
        let src_dir = crate_dir.join("src");
        fs::create_dir_all(&src_dir).expect("create temp crate");
        let path = src_dir.join("lib.rs");
        fs::write(&path, contents).expect("write temp file");
        path
    }

    #[test]
    fn primary_parser_preserves_existing_result_behavior() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::core::result::Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            parse_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
    }

    #[test]
    fn target_collector_reads_both_result_branches() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::core::result::Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn primary_parser_reads_first_branch_target() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::statum::Branch<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            parse_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
    }

    #[test]
    fn target_collector_reads_both_branch_targets() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::statum::Branch<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn target_collector_reads_nested_wrappers() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::core::option::Option<::core::result::Result<Machine<Accepted>, ::statum::Branch<Machine<Rejected>, Error>>>",
        );

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![
                ("Machine".to_owned(), "Accepted".to_owned()),
                ("Machine".to_owned(), "Rejected".to_owned()),
            ]
        );
    }

    #[test]
    fn target_collector_ignores_non_machine_payloads_and_dedups() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::core::result::Result<::core::option::Option<Machine<Accepted>>, ::core::result::Result<Machine<Accepted>, Error>>",
        );

        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }

    #[test]
    fn parser_rejects_bare_wrappers() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("Result<Machine<Accepted>, Machine<Rejected>>");

        assert_eq!(parse_machine_and_state(&ty, &target), None);
        assert!(collect_machine_and_states(&ty, &target).is_empty());
    }

    #[test]
    fn parser_rejects_same_leaf_machine_in_other_module() {
        let target = parse_type("FlowMachine<Draft>");
        let ty = parse_type("other::FlowMachine<Done>");

        assert_eq!(parse_machine_and_state(&ty, &target), None);
        assert!(collect_machine_and_states(&ty, &target).is_empty());
    }

    #[test]
    fn parser_accepts_std_wrapper_paths() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type(
            "::std::option::Option<::std::result::Result<Machine<Accepted>, Error>>",
        );

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }

    #[test]
    fn parser_accepts_self_qualified_machine_paths() {
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("::core::option::Option<self::Machine<Accepted>>");

        assert_eq!(
            parse_primary_machine_and_state(&ty, &target),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states(&ty, &target),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }

    #[test]
    fn impl_target_rejects_qualified_state_paths() {
        let ty = parse_type("Machine<crate::Draft>");
        assert!(extract_impl_machine_and_state(&ty).is_none());
    }

    #[test]
    fn parser_resolves_crate_root_aliases_from_submodules() {
        let path = write_temp_rust_file(
            r#"
pub type Result<T> = ::core::result::Result<T, ()>;
pub type Flow<State> = Machine<State>;

mod auth {
    pub fn marker() {}
}
"#,
        );
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("crate::Result<crate::Flow<Accepted>>");
        let context = AliasResolutionContext {
            module_root: module_root_from_file(path.to_str().expect("path")),
            root_module_path: "crate".into(),
            file_path: path.to_string_lossy().into_owned(),
            module_path: "auth".into(),
        };

        assert_eq!(
            parse_machine_and_state_in_context(&ty, &target, Some(&context)),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states_in_context(&ty, &target, Some(&context)),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }

    #[test]
    fn parser_resolves_crate_root_aliases_in_real_fixture_file() {
        let path = format!(
            "{}/tests/ui/valid_transition_crate_aliases.rs",
            env!("CARGO_MANIFEST_DIR")
        );
        let target = parse_type("Machine<Draft>");
        let ty = parse_type("crate::Result<crate::Flow<Accepted>>");
        let context = AliasResolutionContext {
            module_root: module_root_from_file(&path),
            root_module_path: "valid_transition_crate_aliases".into(),
            file_path: path,
            module_path: "valid_transition_crate_aliases::auth".into(),
        };

        assert_eq!(
            parse_machine_and_state_in_context(&ty, &target, Some(&context)),
            Some(("Machine".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(
            collect_machine_and_states_in_context(&ty, &target, Some(&context)),
            vec![("Machine".to_owned(), "Accepted".to_owned())]
        );
    }
}
