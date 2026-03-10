use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{OnceLock, RwLock};

use crate::analysis::{get_file_analysis, FileAnalysis};
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

/// Ensures the requested key is loaded into `registry`, using the current call-site.
pub fn ensure_loaded<D>(
    registry: &StaticRegistry<D::Key, D::Value>,
    requested_key: &D::Key,
) -> Option<D::Value>
where
    D: RegistryDomain,
{
    let source_info = current_source_info();
    if source_info.is_none() {
        return registry.get_cloned(requested_key);
    }

    let file_path = source_info?.0;
    if let Some(cached) = registry.get_cloned(requested_key) {
        if cache_matches_file(&cached, &file_path) {
            return Some(cached);
        }
    }

    let analysis = get_file_analysis(&file_path)?;
    let mut found: Option<(D::Key, D::Value)> = None;
    for entry in D::entries(&analysis) {
        if !D::matches_entry(entry) {
            continue;
        }

        let Some(resolved_module_path) = module_path_for_line(&file_path, D::entry_line(entry))
        else {
            continue;
        };
        if !module_matches(requested_key, &resolved_module_path) {
            continue;
        }

        let canonical_key = D::Key::from_module_path(resolved_module_path);
        let Some(mut value) = D::build_value(entry, &canonical_key) else {
            continue;
        };
        value.set_file_path(file_path.clone());
        found = Some((canonical_key, value));
        break;
    }

    if let Some((canonical_key, value)) = found {
        store_with_alias(registry, requested_key, &canonical_key, &value);
        return Some(value);
    }

    None
}

fn module_matches<K: RegistryKey>(requested_key: &K, resolved_module_path: &str) -> bool {
    requested_key.is_unknown() || requested_key.as_ref() == resolved_module_path
}

fn cache_matches_file<V: RegistryValue>(value: &V, file_path: &str) -> bool {
    value.file_path() == Some(file_path)
}

fn store_with_alias<K, V>(
    registry: &StaticRegistry<K, V>,
    requested_key: &K,
    canonical_key: &K,
    value: &V,
) where
    K: AsRef<str> + Clone + Eq + Hash,
    V: Clone,
{
    registry.insert(canonical_key.clone(), value.clone());
    if canonical_key.as_ref() != requested_key.as_ref() {
        registry.insert(requested_key.clone(), value.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[derive(Clone, Debug)]
    struct TestValue {
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

    #[test]
    fn module_matching_supports_unknown_and_exact() {
        assert!(module_matches(&TestKey("unknown".into()), "crate::foo"));
        assert!(module_matches(&TestKey("crate::foo".into()), "crate::foo"));
        assert!(!module_matches(&TestKey("crate::foo".into()), "crate::bar"));
    }

    #[test]
    fn store_with_alias_adds_both_keys_when_different() {
        let registry: StaticRegistry<TestKey, TestValue> = StaticRegistry::new();
        let requested = TestKey("unknown".into());
        let canonical = TestKey("crate::mod_a".into());
        let value = TestValue {
            file_path: Some("/tmp/a.rs".into()),
        };

        store_with_alias(&registry, &requested, &canonical, &value);

        let map = registry.map().read().expect("lock");
        assert!(map.get(&requested).is_some());
        assert!(map.get(&canonical).is_some());
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn cache_match_checks_current_file() {
        let value = TestValue {
            file_path: Some("/tmp/a.rs".into()),
        };
        assert!(cache_matches_file(&value, "/tmp/a.rs"));
        assert!(!cache_matches_file(&value, "/tmp/b.rs"));
    }

    #[test]
    fn cache_match_requires_tracked_file_path() {
        let value = TestValue { file_path: None };
        assert!(!cache_matches_file(&value, "/tmp/a.rs"));
    }
}
