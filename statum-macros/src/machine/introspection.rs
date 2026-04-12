use quote::format_ident;
use std::sync::atomic::{AtomicU64, Ordering};
use syn::Ident;

static UNIQUE_SLICE_ID_FALLBACK: AtomicU64 = AtomicU64::new(1);

pub(crate) fn to_shouty_snake_identifier(value: &str) -> String {
    let mut result = String::new();

    for (idx, segment) in value.trim_start_matches("r#").split('_').enumerate() {
        if segment.is_empty() {
            continue;
        }

        if idx > 0 {
            result.push('_');
        }

        for ch in segment.chars() {
            for upper in ch.to_uppercase() {
                result.push(upper);
            }
        }
    }

    result
}

pub(crate) fn transition_slice_ident(
    machine_name: &str,
    module_path: &str,
    file_path: Option<&str>,
    line_number: usize,
) -> Ident {
    let key = slice_scope_key(machine_name, module_path, file_path, line_number, "transitions");
    format_ident!("__STATUM_TRANSITIONS_{:016X}", stable_hash(&key))
}

pub(crate) fn transition_presentation_slice_ident(
    machine_name: &str,
    module_path: &str,
    file_path: Option<&str>,
    line_number: usize,
) -> Ident {
    let key = slice_scope_key(
        machine_name,
        module_path,
        file_path,
        line_number,
        "transition_presentation",
    );
    format_ident!(
        "__STATUM_TRANSITION_PRESENTATION_{:016X}",
        stable_hash(&key)
    )
}

pub(crate) fn linked_transition_slice_ident(
    machine_name: &str,
    module_path: &str,
    file_path: Option<&str>,
    line_number: usize,
) -> Ident {
    let key = slice_scope_key(
        machine_name,
        module_path,
        file_path,
        line_number,
        "linked_transitions",
    );
    format_ident!("__STATUM_LINKED_TRANSITIONS_{:016X}", stable_hash(&key))
}

fn slice_scope_key(
    machine_name: &str,
    module_path: &str,
    file_path: Option<&str>,
    line_number: usize,
    kind: &str,
) -> String {
    if let Some(file_path) = file_path.filter(|_| line_number > 0) {
        return format!("{kind}::{module_path}::{machine_name}::{file_path}::{line_number}");
    }

    if module_path != "unknown" {
        return format!("{kind}::{module_path}::{machine_name}");
    }

    let fallback = UNIQUE_SLICE_ID_FALLBACK.fetch_add(1, Ordering::Relaxed);
    format!("{kind}::{machine_name}::__fallback__::{fallback}")
}

fn stable_hash(input: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{
        linked_transition_slice_ident, to_shouty_snake_identifier,
        transition_presentation_slice_ident, transition_slice_ident,
    };

    #[test]
    fn shouty_snake_identifier_handles_snake_case_and_raw_prefixes() {
        assert_eq!(to_shouty_snake_identifier("validate"), "VALIDATE");
        assert_eq!(to_shouty_snake_identifier("start_review"), "START_REVIEW");
        assert_eq!(to_shouty_snake_identifier("r#await"), "AWAIT");
    }

    #[test]
    fn transition_slice_ident_tracks_machine_source() {
        let first =
            transition_slice_ident("ReviewFlow", "crate::alpha", Some("src/alpha.rs"), 40)
                .to_string();
        let second =
            transition_slice_ident("ReviewFlow", "crate::alpha", Some("src/beta.rs"), 40)
                .to_string();
        let third =
            transition_slice_ident("ReviewFlow", "crate::alpha", Some("src/alpha.rs"), 91)
                .to_string();

        assert!(first.starts_with("__STATUM_TRANSITIONS_"));
        assert!(second.starts_with("__STATUM_TRANSITIONS_"));
        assert!(third.starts_with("__STATUM_TRANSITIONS_"));
        assert_ne!(first, second);
        assert_ne!(first, third);
    }

    #[test]
    fn transition_presentation_slice_ident_tracks_machine_source() {
        let first = transition_presentation_slice_ident(
            "ReviewFlow",
            "crate::alpha",
            Some("src/alpha.rs"),
            40,
        )
        .to_string();
        let second = transition_presentation_slice_ident(
            "ReviewFlow",
            "crate::alpha",
            Some("src/beta.rs"),
            40,
        )
        .to_string();
        let third = transition_presentation_slice_ident(
            "ReviewFlow",
            "crate::alpha",
            Some("src/alpha.rs"),
            91,
        )
        .to_string();

        assert!(first.starts_with("__STATUM_TRANSITION_PRESENTATION_"));
        assert!(second.starts_with("__STATUM_TRANSITION_PRESENTATION_"));
        assert!(third.starts_with("__STATUM_TRANSITION_PRESENTATION_"));
        assert_ne!(first, second);
        assert_ne!(first, third);
    }

    #[test]
    fn linked_transition_slice_ident_tracks_machine_source() {
        let first = linked_transition_slice_ident(
            "ReviewFlow",
            "crate::alpha",
            Some("src/alpha.rs"),
            40,
        )
        .to_string();
        let second = linked_transition_slice_ident(
            "ReviewFlow",
            "crate::alpha",
            Some("src/beta.rs"),
            40,
        )
        .to_string();
        let third = linked_transition_slice_ident(
            "ReviewFlow",
            "crate::alpha",
            Some("src/alpha.rs"),
            91,
        )
        .to_string();

        assert!(first.starts_with("__STATUM_LINKED_TRANSITIONS_"));
        assert!(second.starts_with("__STATUM_LINKED_TRANSITIONS_"));
        assert!(third.starts_with("__STATUM_LINKED_TRANSITIONS_"));
        assert_ne!(first, second);
        assert_ne!(first, third);
    }

    #[test]
    fn transition_slice_ident_uses_module_path_when_source_identity_is_missing() {
        let first = transition_slice_ident("Flow", "crate::alpha", None, 0).to_string();
        let second = transition_slice_ident("Flow", "crate::beta", None, 0).to_string();

        assert_ne!(first, second);
    }

    #[test]
    fn transition_slice_ident_fallback_counter_avoids_unknown_source_collisions() {
        let first = transition_slice_ident("Flow", "unknown", None, 0).to_string();
        let second = transition_slice_ident("Flow", "unknown", None, 0).to_string();

        assert_ne!(first, second);
    }
}
