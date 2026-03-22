use quote::format_ident;
use syn::Ident;

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
    file_path: Option<&str>,
    line_number: usize,
) -> Ident {
    let key = format!(
        "{machine_name}::{}::{line_number}",
        file_path.unwrap_or_default()
    );
    format_ident!("__statum_transitions_{:016x}", stable_hash(&key))
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
    use super::{to_shouty_snake_identifier, transition_slice_ident};

    #[test]
    fn shouty_snake_identifier_handles_snake_case_and_raw_prefixes() {
        assert_eq!(to_shouty_snake_identifier("validate"), "VALIDATE");
        assert_eq!(to_shouty_snake_identifier("start_review"), "START_REVIEW");
        assert_eq!(to_shouty_snake_identifier("r#await"), "AWAIT");
    }

    #[test]
    fn transition_slice_ident_tracks_machine_source() {
        let first = transition_slice_ident("ReviewFlow", Some("src/alpha.rs"), 40).to_string();
        let second = transition_slice_ident("ReviewFlow", Some("src/beta.rs"), 40).to_string();
        let third = transition_slice_ident("ReviewFlow", Some("src/alpha.rs"), 91).to_string();

        assert!(first.starts_with("__statum_transitions_"));
        assert!(second.starts_with("__statum_transitions_"));
        assert!(third.starts_with("__statum_transitions_"));
        assert_ne!(first, second);
        assert_ne!(first, third);
    }
}
