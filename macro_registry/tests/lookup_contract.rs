use macro_registry::analysis::{EnumEntry, FileAnalysis};
use macro_registry::registry::{
    try_ensure_loaded, try_ensure_loaded_by_name_from_source, try_ensure_loaded_from_source,
    LookupFailure, LookupMode, NamedRegistryDomain, RegistryDomain, RegistryKey, RegistryValue,
    SourceContext, StaticRegistry,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TestKey(String);

impl AsRef<str> for TestKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl RegistryKey for TestKey {
    fn from_module_path(module_path: String) -> Self {
        Self(module_path)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TestValue {
    name: String,
    file_path: Option<String>,
}

impl RegistryValue for TestValue {
    fn file_path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    fn set_file_path(&mut self, file_path: String) {
        self.file_path = Some(file_path);
    }
}

struct TestDomain;

impl RegistryDomain for TestDomain {
    type Key = TestKey;
    type Value = TestValue;
    type Entry = EnumEntry;

    fn entries(analysis: &FileAnalysis) -> &[Self::Entry] {
        &analysis.enums
    }

    fn entry_line(entry: &Self::Entry) -> usize {
        entry.line_number
    }

    fn build_value(entry: &Self::Entry, _module_path: &Self::Key) -> Option<Self::Value> {
        Some(TestValue {
            name: entry.item.ident.to_string(),
            file_path: None,
        })
    }

    fn matches_entry(entry: &Self::Entry) -> bool {
        entry.attrs.iter().any(|attr| attr == "state")
    }

    fn entry_hint(entry: &Self::Entry) -> Option<String> {
        Some(entry.item.ident.to_string())
    }
}

impl NamedRegistryDomain for TestDomain {
    fn entry_name(entry: &Self::Entry) -> String {
        entry.item.ident.to_string()
    }

    fn value_name(value: &Self::Value) -> String {
        value.name.clone()
    }
}

fn write_temp_rust_file(contents: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let crate_dir = std::env::temp_dir().join(format!("statum_registry_integration_{nanos}"));
    let src_dir = crate_dir.join("src");
    fs::create_dir_all(&src_dir).expect("create temp crate");
    let path = src_dir.join("lib.rs");
    fs::write(&path, contents).expect("write temp file");
    path
}

#[test]
fn public_lookup_api_reports_ambiguous_module_without_name() {
    let path = write_temp_rust_file(
        r#"
mod workflow {
    #[state]
    enum TaskState {
        Draft,
    }

    #[state]
    enum ReviewState {
        Review,
    }
}
"#,
    );
    let registry: StaticRegistry<TestKey, TestValue> = StaticRegistry::new();
    let source = SourceContext::new(path.to_string_lossy(), 2);

    let result = try_ensure_loaded_from_source::<TestDomain>(
        &registry,
        LookupMode::Exact(TestKey("workflow".into())),
        &source,
    );

    match result {
        Err(LookupFailure::Ambiguous { matches, .. }) => {
            assert_eq!(matches.len(), 2);
            assert_eq!(matches[0].item_hint.as_deref(), Some("TaskState"));
            assert_eq!(matches[1].item_hint.as_deref(), Some("ReviewState"));
        }
        other => panic!("expected ambiguity, got {other:?}"),
    }

    let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
}

#[test]
fn public_lookup_api_can_resolve_named_entries_and_report_missing_source() {
    let path = write_temp_rust_file(
        r#"
mod workflow {
    #[state]
    enum TaskState {
        Draft,
    }

    #[state]
    enum ReviewState {
        Review,
    }
}
"#,
    );
    let registry: StaticRegistry<TestKey, TestValue> = StaticRegistry::new();
    let source = SourceContext::new(path.to_string_lossy(), 2);

    let named = try_ensure_loaded_by_name_from_source::<TestDomain>(
        &registry,
        LookupMode::Exact(TestKey("workflow".into())),
        "ReviewState",
        &source,
    )
    .expect("named lookup");
    assert_eq!(named.value.name, "ReviewState");

    let missing_source =
        try_ensure_loaded::<TestDomain>(&registry, LookupMode::Exact(TestKey("workflow".into())));
    assert_eq!(missing_source, Err(LookupFailure::SourceUnavailable));

    let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
}
