use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{OnceLock, RwLock};

use crate::analysis::{get_file_analysis, FileAnalysis};
use crate::cache::tracked_file_matches;
use crate::callsite::{current_source_info, module_path_for_line};

/// Key type for registry lookups.
pub trait RegistryKey: AsRef<str> + Clone + Eq + Hash {
    fn from_module_path(module_path: String) -> Self;

    fn is_unknown(&self) -> bool {
        self.as_ref() == "unknown"
    }
}

/// Value type stored in the registry.
pub trait RegistryValue: Clone {
    fn file_path(&self) -> Option<&str>;
    fn set_file_path(&mut self, file_path: String);
}

/// Domain-specific hooks used by the generic registry loader.
pub trait RegistryDomain {
    type Key: RegistryKey;
    type Value: RegistryValue;
    type Entry;

    fn entries(analysis: &FileAnalysis) -> &[Self::Entry];
    fn entry_line(entry: &Self::Entry) -> usize;
    fn build_value(entry: &Self::Entry, module_path: &Self::Key) -> Option<Self::Value>;

    fn matches_entry(_entry: &Self::Entry) -> bool {
        true
    }

    fn entry_hint(_entry: &Self::Entry) -> Option<String> {
        None
    }
}

/// Domain hook for registries that support disambiguating entries by item name.
pub trait NamedRegistryDomain: RegistryDomain {
    fn entry_name(entry: &Self::Entry) -> String;
    fn value_name(value: &Self::Value) -> String;
}

/// Thread-safe static registry wrapper.
pub struct StaticRegistry<K, V> {
    inner: OnceLock<RwLock<HashMap<K, V>>>,
}

impl<K, V> StaticRegistry<K, V> {
    pub const fn new() -> Self {
        Self {
            inner: OnceLock::new(),
        }
    }

    pub fn map(&self) -> &RwLock<HashMap<K, V>> {
        self.inner.get_or_init(|| RwLock::new(HashMap::new()))
    }
}

impl<K, V> Default for StaticRegistry<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> StaticRegistry<K, V>
where
    K: Eq + Hash,
    V: Clone,
{
    pub fn get_cloned(&self, key: &K) -> Option<V> {
        self.map().read().ok()?.get(key).cloned()
    }

    pub fn insert(&self, key: K, value: V) {
        if let Ok(mut map) = self.map().write() {
            map.insert(key, value);
        }
    }
}

/// Source file and line used for registry resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceContext {
    pub file_path: String,
    pub line_number: usize,
}

impl SourceContext {
    pub fn new(file_path: impl Into<String>, line_number: usize) -> Self {
        Self {
            file_path: file_path.into(),
            line_number,
        }
    }

    pub fn current() -> Option<Self> {
        current_source_info().map(|(file_path, line_number)| Self::new(file_path, line_number))
    }
}

/// Module-matching mode for registry loads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LookupMode<K> {
    Exact(K),
    AnyModule,
}

impl<K> LookupMode<K>
where
    K: RegistryKey,
{
    pub fn from_key(requested_key: &K) -> Self {
        if requested_key.is_unknown() {
            Self::AnyModule
        } else {
            Self::Exact(requested_key.clone())
        }
    }
}

/// One candidate discovered while trying to resolve a registry lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LookupCandidate<K> {
    pub canonical_key: K,
    pub item_hint: Option<String>,
}

/// Result for a successful typed lookup.
#[derive(Clone, Debug, PartialEq)]
pub struct LookupMatch<K, V> {
    pub canonical_key: K,
    pub value: V,
    pub cache_hit: bool,
}

/// Convenience alias for the typed registry load contract.
pub type LookupResult<K, V> = Result<LookupMatch<K, V>, LookupFailure<K>>;

/// Structured failure for typed registry lookups.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LookupFailure<K> {
    SourceUnavailable,
    AnalysisUnavailable {
        file_path: String,
    },
    NotFound {
        requested: LookupMode<K>,
        item_name: Option<String>,
    },
    Ambiguous {
        requested: LookupMode<K>,
        item_name: Option<String>,
        matches: Vec<LookupCandidate<K>>,
    },
}

/// Ensures the requested key is loaded into `registry`, using the current call-site.
pub fn ensure_loaded<D>(
    registry: &StaticRegistry<D::Key, D::Value>,
    requested_key: &D::Key,
) -> Option<D::Value>
where
    D: RegistryDomain,
{
    let requested = LookupMode::from_key(requested_key);
    if let Some(source) = SourceContext::current() {
        return try_ensure_loaded_from_source::<D>(registry, requested, &source)
            .ok()
            .map(|loaded| loaded.value);
    }

    match requested {
        LookupMode::Exact(_) => registry.get_cloned(requested_key),
        LookupMode::AnyModule => None,
    }
}

/// Ensures a named entry is loaded into `registry`, using the current call-site.
pub fn ensure_loaded_by_name<D>(
    registry: &StaticRegistry<D::Key, D::Value>,
    requested_key: &D::Key,
    item_name: &str,
) -> Option<D::Value>
where
    D: NamedRegistryDomain,
{
    let requested = LookupMode::from_key(requested_key);
    if let Some(source) = SourceContext::current() {
        return try_ensure_loaded_by_name_from_source::<D>(registry, requested, item_name, &source)
            .ok()
            .map(|loaded| loaded.value);
    }

    match requested {
        LookupMode::Exact(_) => registry
            .get_cloned(requested_key)
            .filter(|value| D::value_name(value) == item_name),
        LookupMode::AnyModule => None,
    }
}

/// Typed lookup using the current call-site as the source context.
pub fn try_ensure_loaded<D>(
    registry: &StaticRegistry<D::Key, D::Value>,
    requested: LookupMode<D::Key>,
) -> LookupResult<D::Key, D::Value>
where
    D: RegistryDomain,
{
    let Some(source) = SourceContext::current() else {
        return Err(LookupFailure::SourceUnavailable);
    };
    try_ensure_loaded_from_source::<D>(registry, requested, &source)
}

/// Typed lookup using an explicit source context.
pub fn try_ensure_loaded_from_source<D>(
    registry: &StaticRegistry<D::Key, D::Value>,
    requested: LookupMode<D::Key>,
    source: &SourceContext,
) -> LookupResult<D::Key, D::Value>
where
    D: RegistryDomain,
{
    resolve_lookup::<D, _, _>(registry, requested, None, source, |_, _| true, |_, _| true)
}

/// Typed named lookup using the current call-site as the source context.
pub fn try_ensure_loaded_by_name<D>(
    registry: &StaticRegistry<D::Key, D::Value>,
    requested: LookupMode<D::Key>,
    item_name: &str,
) -> LookupResult<D::Key, D::Value>
where
    D: NamedRegistryDomain,
{
    let Some(source) = SourceContext::current() else {
        return Err(LookupFailure::SourceUnavailable);
    };
    try_ensure_loaded_by_name_from_source::<D>(registry, requested, item_name, &source)
}

/// Typed named lookup using an explicit source context.
pub fn try_ensure_loaded_by_name_from_source<D>(
    registry: &StaticRegistry<D::Key, D::Value>,
    requested: LookupMode<D::Key>,
    item_name: &str,
    source: &SourceContext,
) -> LookupResult<D::Key, D::Value>
where
    D: NamedRegistryDomain,
{
    resolve_lookup::<D, _, _>(
        registry,
        requested,
        Some(item_name),
        source,
        |entry, expected| D::entry_name(entry) == expected,
        |value, expected| D::value_name(value) == expected,
    )
}

fn resolve_lookup<D, EntryNameMatch, ValueNameMatch>(
    registry: &StaticRegistry<D::Key, D::Value>,
    requested: LookupMode<D::Key>,
    item_name: Option<&str>,
    source: &SourceContext,
    entry_name_matches: EntryNameMatch,
    value_name_matches: ValueNameMatch,
) -> LookupResult<D::Key, D::Value>
where
    D: RegistryDomain,
    EntryNameMatch: Fn(&D::Entry, &str) -> bool,
    ValueNameMatch: Fn(&D::Value, &str) -> bool,
{
    if let LookupMode::Exact(requested_key) = &requested {
        if let Some(cached) = registry.get_cloned(requested_key) {
            let name_matches = item_name.is_none_or(|name| value_name_matches(&cached, name));
            if tracked_file_matches(cached.file_path(), &source.file_path) && name_matches {
                return Ok(LookupMatch {
                    canonical_key: requested_key.clone(),
                    value: cached,
                    cache_hit: true,
                });
            }
        }
    }

    let Some(analysis) = get_file_analysis(&source.file_path) else {
        return Err(LookupFailure::AnalysisUnavailable {
            file_path: source.file_path.clone(),
        });
    };

    let mut matches = Vec::new();
    let mut line_module_cache: HashMap<usize, Option<String>> = HashMap::new();
    for entry in D::entries(&analysis) {
        if !D::matches_entry(entry) || !item_name.is_none_or(|name| entry_name_matches(entry, name))
        {
            continue;
        }

        let line_number = D::entry_line(entry);
        let resolved_module_path = if let Some(cached) = line_module_cache.get(&line_number) {
            cached.clone()
        } else {
            let resolved = module_path_for_line(&source.file_path, line_number);
            line_module_cache.insert(line_number, resolved.clone());
            resolved
        };

        let Some(resolved_module_path) = resolved_module_path else {
            continue;
        };
        if !matches_lookup_mode(&requested, &resolved_module_path) {
            continue;
        }

        let canonical_key = D::Key::from_module_path(resolved_module_path);
        let Some(mut value) = D::build_value(entry, &canonical_key) else {
            continue;
        };
        value.set_file_path(source.file_path.clone());
        matches.push((
            LookupCandidate {
                canonical_key,
                item_hint: D::entry_hint(entry),
            },
            value,
        ));
    }

    dedup_matches(&mut matches);

    if matches.is_empty() {
        return Err(LookupFailure::NotFound {
            requested,
            item_name: item_name.map(str::to_string),
        });
    }

    if matches.len() > 1 {
        return Err(LookupFailure::Ambiguous {
            requested,
            item_name: item_name.map(str::to_string),
            matches: matches
                .into_iter()
                .map(|(candidate, _)| candidate)
                .collect(),
        });
    }

    let (candidate, value) = matches.pop().expect("single match");
    store_lookup_result(registry, &requested, &candidate.canonical_key, &value);
    Ok(LookupMatch {
        canonical_key: candidate.canonical_key,
        value,
        cache_hit: false,
    })
}

fn matches_lookup_mode<K: RegistryKey>(
    requested: &LookupMode<K>,
    resolved_module_path: &str,
) -> bool {
    match requested {
        LookupMode::Exact(requested_key) => requested_key.as_ref() == resolved_module_path,
        LookupMode::AnyModule => true,
    }
}

fn dedup_matches<K, V>(matches: &mut Vec<(LookupCandidate<K>, V)>)
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    let mut seen = HashSet::new();
    matches.retain(|(candidate, _)| {
        seen.insert((candidate.canonical_key.clone(), candidate.item_hint.clone()))
    });
}

fn store_lookup_result<K, V>(
    registry: &StaticRegistry<K, V>,
    requested: &LookupMode<K>,
    canonical_key: &K,
    value: &V,
) where
    K: AsRef<str> + Clone + Eq + Hash,
    V: Clone,
{
    registry.insert(canonical_key.clone(), value.clone());
    if let LookupMode::Exact(requested_key) = requested {
        if canonical_key.as_ref() != requested_key.as_ref() {
            registry.insert(requested_key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::EnumEntry;
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
        let crate_dir = std::env::temp_dir().join(format!("statum_registry_{nanos}"));
        let src_dir = crate_dir.join("src");
        fs::create_dir_all(&src_dir).expect("create temp crate");
        let path = src_dir.join("lib.rs");
        fs::write(&path, contents).expect("write temp file");
        path
    }

    #[test]
    fn lookup_mode_from_key_uses_any_module_for_unknown() {
        assert_eq!(
            LookupMode::from_key(&TestKey("unknown".into())),
            LookupMode::AnyModule
        );
        assert_eq!(
            LookupMode::from_key(&TestKey("crate::foo".into())),
            LookupMode::Exact(TestKey("crate::foo".into()))
        );
    }

    #[test]
    fn store_lookup_result_only_aliases_exact_requests() {
        let registry: StaticRegistry<TestKey, TestValue> = StaticRegistry::new();
        let value = TestValue {
            name: "State".into(),
            file_path: Some("/tmp/a.rs".into()),
        };

        store_lookup_result(
            &registry,
            &LookupMode::Exact(TestKey("crate::workflow".into())),
            &TestKey("crate::workflow".into()),
            &value,
        );
        store_lookup_result(
            &registry,
            &LookupMode::AnyModule,
            &TestKey("crate::other".into()),
            &value,
        );

        let map = registry.map().read().expect("lock");
        assert!(map.get(&TestKey("crate::workflow".into())).is_some());
        assert!(map.get(&TestKey("crate::other".into())).is_some());
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn try_ensure_loaded_from_source_reports_ambiguous_module() {
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
    fn try_ensure_loaded_by_name_from_source_finds_named_match() {
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

        let loaded = try_ensure_loaded_by_name_from_source::<TestDomain>(
            &registry,
            LookupMode::Exact(TestKey("workflow".into())),
            "ReviewState",
            &source,
        )
        .expect("named lookup");

        assert_eq!(loaded.canonical_key, TestKey("workflow".into()));
        assert_eq!(loaded.value.name, "ReviewState");
        assert!(!loaded.cache_hit);

        let _ = fs::remove_dir_all(path.parent().expect("src").parent().expect("crate"));
    }

    #[test]
    fn try_ensure_loaded_without_source_reports_unavailable() {
        let registry: StaticRegistry<TestKey, TestValue> = StaticRegistry::new();
        let result = try_ensure_loaded::<TestDomain>(
            &registry,
            LookupMode::Exact(TestKey("workflow".into())),
        );

        assert_eq!(result, Err(LookupFailure::SourceUnavailable));
    }

    #[test]
    fn cache_match_checks_current_file() {
        let value = TestValue {
            name: "State".into(),
            file_path: Some("/tmp/a.rs".into()),
        };
        assert!(tracked_file_matches(value.file_path(), "/tmp/a.rs"));
        assert!(!tracked_file_matches(value.file_path(), "/tmp/b.rs"));
    }

    #[test]
    fn cache_match_requires_tracked_file_path() {
        let value = TestValue {
            name: "State".into(),
            file_path: None,
        };
        assert!(!tracked_file_matches(value.file_path(), "/tmp/a.rs"));
    }
}
