use std::process::Command;

fn cargo_statum() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cargo-statum"))
}

#[test]
fn graph_mermaid_for_flagship_machine() {
    let output = cargo_statum()
        .args([
            "graph",
            "--machine",
            "axum-sqlite-review",
            "--format",
            "mermaid",
        ])
        .output()
        .expect("cargo-statum should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(stdout.starts_with("stateDiagram-v2\n"), "{stdout}");
    assert!(stdout.contains("Draft"), "{stdout}");
    assert!(stdout.contains("InReview"), "{stdout}");
    assert!(stdout.contains("Published"), "{stdout}");
    assert!(stdout.contains("s0 --> s1: submit"), "{stdout}");
}

#[test]
fn graph_dot_for_flagship_machine_through_cargo_subcommand_argv_shape() {
    let output = cargo_statum()
        .args([
            "statum",
            "graph",
            "--machine",
            "DocumentMachine",
            "--format",
            "dot",
        ])
        .output()
        .expect("cargo-statum should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.starts_with("digraph statum_workflow {\n"),
        "{stdout}"
    );
    assert!(stdout.contains("s0 -> s1 [label=\"submit\"]"), "{stdout}");
    assert!(stdout.contains("s1 -> s2 [label=\"approve\"]"), "{stdout}");
}

#[test]
fn graph_json_for_flagship_machine_declares_metadata_authority() {
    let output = cargo_statum()
        .args([
            "graph",
            "--machine",
            "axum_sqlite_review::DocumentMachine",
            "--format",
            "json",
        ])
        .output()
        .expect("cargo-statum should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is json");
    assert_eq!(json["version"], "v1");
    assert_eq!(json["authority"], "cfg_pruned_macro_input");
    let transitions = json["transitions"]
        .as_array()
        .expect("transitions should be an array");
    assert!(
        transitions
            .iter()
            .any(|transition| transition["method_name"] == "submit"),
        "{json}"
    );
}

#[test]
fn graph_matrix_for_flagship_machine_lists_allowed_and_forbidden_transitions() {
    let output = cargo_statum()
        .args([
            "graph",
            "--machine",
            "axum-sqlite-review",
            "--format",
            "matrix",
        ])
        .output()
        .expect("cargo-statum should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("| from \\ to | Draft | InReview | Published |"),
        "{stdout}"
    );
    assert!(
        stdout.contains("| Draft | forbidden | submit | forbidden |"),
        "{stdout}"
    );
    assert!(
        stdout.contains("| InReview | forbidden | forbidden | approve |"),
        "{stdout}"
    );
    assert!(
        stdout.contains("| Published | forbidden | forbidden | forbidden |"),
        "{stdout}"
    );
}

#[test]
fn graph_lints_for_flagship_machine_reports_authority_and_no_warnings() {
    let output = cargo_statum()
        .args([
            "graph",
            "--machine",
            "axum-sqlite-review",
            "--format",
            "lints",
        ])
        .output()
        .expect("cargo-statum should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.starts_with(
            "Graph invariant lint report for showcases::axum_sqlite_review::DocumentMachine\n"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("authority: cfg_pruned_macro_input"),
        "{stdout}"
    );
    assert!(stdout.contains("false-positive boundary:"), "{stdout}");
    assert!(stdout.contains("No graph invariant warnings."), "{stdout}");
}

#[test]
fn graph_rejects_unknown_machine_without_fallback_claim() {
    let output = cargo_statum()
        .args(["graph", "--machine", "Nope", "--format", "json"])
        .output()
        .expect("cargo-statum should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("unsupported machine `Nope`"), "{stderr}");
    assert!(stderr.contains("axum-sqlite-review"), "{stderr}");
}

#[test]
fn agent_context_for_flagship_machine_is_compact_and_metadata_derived() {
    let output = cargo_statum()
        .args(["agent-context", "--machine", "axum-sqlite-review"])
        .output()
        .expect("cargo-statum should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.starts_with("# statum agent-context v1\n"),
        "{stdout}"
    );
    assert!(
        stdout.contains("machine: showcases::axum_sqlite_review::DocumentMachine"),
        "{stdout}"
    );
    assert!(
        stdout.contains("authority: cfg_pruned_macro_input"),
        "{stdout}"
    );
    assert!(stdout.contains("states:"), "{stdout}");
    assert!(stdout.contains("- Draft data=no"), "{stdout}");
    assert!(stdout.contains("- InReview data=yes"), "{stdout}");
    assert!(stdout.contains("legal_transitions:"), "{stdout}");
    assert!(stdout.contains("- Draft.submit -> InReview"), "{stdout}");
    assert!(stdout.contains("forbidden_calls:"), "{stdout}");
    assert!(stdout.contains("- Draft: approve"), "{stdout}");
    assert!(stdout.contains("- Published: approve, submit"), "{stdout}");
    assert!(stdout.contains("rehydration_rules:"), "{stdout}");
    assert!(
        stdout.contains("- InReview: persisted state data required"),
        "{stdout}"
    );
    assert!(stdout.contains("validators:"), "{stdout}");
    assert!(stdout.contains("not in StableGraphMetadata v1"), "{stdout}");
    assert!(stdout.contains("caveats:"), "{stdout}");
    assert!(stdout.contains("runtime_only_transitions"), "{stdout}");
}

#[test]
fn agent_context_rejects_unknown_machine_without_graph_fallback() {
    let output = cargo_statum()
        .args(["agent-context", "--machine", "Nope"])
        .output()
        .expect("cargo-statum should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("unsupported machine `Nope`"), "{stderr}");
}

#[test]
fn explain_for_flagship_machine_produces_reviewable_metadata_scoped_prose() {
    let first = cargo_statum()
        .args(["explain", "--machine", "axum-sqlite-review"])
        .output()
        .expect("cargo-statum should run");
    let second = cargo_statum()
        .args(["explain", "--machine", "axum-sqlite-review"])
        .output()
        .expect("cargo-statum should run");

    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        first.stdout, second.stdout,
        "explain output must be deterministic"
    );
    let stdout = String::from_utf8(first.stdout).expect("stdout should be utf8");
    assert!(
        stdout.starts_with(
            "# Workflow explanation: showcases::axum_sqlite_review::DocumentMachine\n"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("Observation point: StableGraphMetadata v1 (`cfg_pruned_macro_input`)."),
        "{stdout}"
    );
    assert!(
        stdout.contains("This explanation is derived from metadata only; it does not inspect source code, macro expansion, type checking, runtime policy, validators, storage rows, or side effects."),
        "{stdout}"
    );
    assert!(stdout.contains("## Human workflow"), "{stdout}");
    assert!(
        stdout.contains("1. Draft can move to InReview by calling `submit`."),
        "{stdout}"
    );
    assert!(
        stdout.contains("2. InReview can move to Published by calling `approve`."),
        "{stdout}"
    );
    assert!(
        stdout.contains("Published has no outgoing transitions in this metadata."),
        "{stdout}"
    );
    assert!(stdout.contains("## Review checklist"), "{stdout}");
    assert!(stdout.contains("runtime_only_transitions"), "{stdout}");
    assert!(
        !stdout.contains("source of truth")
            && !stdout.contains("authoritative")
            && !stdout.contains("exhaustive"),
        "explain output must avoid authority claims beyond metadata: {stdout}"
    );
}

#[test]
fn explain_rejects_unknown_machine_without_graph_fallback() {
    let output = cargo_statum()
        .args(["explain", "--machine", "Nope"])
        .output()
        .expect("cargo-statum should run");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "unsupported machine should not render fallback output"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("unsupported machine `Nope`"), "{stderr}");
}

#[test]
fn docs_for_flagship_machine_emit_mermaid_matrix_and_narrative_from_one_metadata_source() {
    let first = cargo_statum()
        .args(["docs", "--machine", "axum-sqlite-review"])
        .output()
        .expect("cargo-statum should run");
    let second = cargo_statum()
        .args(["docs", "--machine", "axum-sqlite-review"])
        .output()
        .expect("cargo-statum should run");

    assert!(
        first.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        first.stdout, second.stdout,
        "generated docs output must be deterministic"
    );
    let stdout = String::from_utf8(first.stdout).expect("stdout should be utf8");
    assert!(
        stdout.starts_with(
            "# Generated protocol docs: showcases::axum_sqlite_review::DocumentMachine\n"
        ),
        "{stdout}"
    );
    assert!(
        stdout.contains("Generated from one `StableGraphMetadata` value."),
        "{stdout}"
    );
    assert!(stdout.contains("## Mermaid state diagram"), "{stdout}");
    assert!(stdout.contains("```mermaid\nstateDiagram-v2"), "{stdout}");
    assert!(stdout.contains("s0 --> s1: submit"), "{stdout}");
    assert!(stdout.contains("## Transition table"), "{stdout}");
    assert!(
        stdout.contains("| Draft | forbidden | submit | forbidden |"),
        "{stdout}"
    );
    assert!(stdout.contains("## Narrative summary"), "{stdout}");
    assert!(stdout.contains("### Human workflow"), "{stdout}");
    assert!(
        stdout.contains("Draft can move to InReview by calling `submit`."),
        "{stdout}"
    );
    assert!(
        !stdout.contains("\n# Workflow explanation:"),
        "generated docs should not nest a standalone H1 inside the narrative section: {stdout}"
    );
    assert!(
        stdout.contains("## Keeping generated artifacts current"),
        "{stdout}"
    );
    assert!(
        stdout.contains("re-run `cargo statum docs --machine axum-sqlite-review`"),
        "{stdout}"
    );
    assert!(
        !stdout.contains("source of truth") && !stdout.contains("authoritative"),
        "docs output must avoid authority claims beyond StableGraphMetadata: {stdout}"
    );
}

#[test]
fn docs_rejects_unknown_machine_without_artifact_fallback() {
    let output = cargo_statum()
        .args(["docs", "--machine", "Nope"])
        .output()
        .expect("cargo-statum should run");

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "unsupported machine should not render fallback docs"
    );
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(stderr.contains("unsupported machine `Nope`"), "{stderr}");
}

#[test]
fn graph_diff_markdown_reports_states_transitions_and_migration_warnings() {
    let temp_dir = std::env::temp_dir().join(format!("statum-graph-diff-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
    let baseline = temp_dir.join("baseline.json");
    let current = temp_dir.join("current.json");

    std::fs::write(&baseline, baseline_snapshot()).expect("baseline should be written");
    std::fs::write(&current, current_snapshot()).expect("current should be written");

    let output = cargo_statum()
        .args([
            "graph",
            "diff",
            "--baseline",
            baseline.to_str().expect("baseline path should be utf8"),
            "--current",
            current.to_str().expect("current path should be utf8"),
            "--format",
            "markdown",
        ])
        .output()
        .expect("cargo-statum should run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    assert!(
        stdout.contains("### Statum graph diff: `example::DocumentMachine`"),
        "{stdout}"
    );
    assert!(stdout.contains("removed state `Archived`"), "{stdout}");
    assert!(stdout.contains("added state `Rejected`"), "{stdout}");
    assert!(
        stdout.contains("removed transition `InReview::archive`"),
        "{stdout}"
    );
    assert!(
        stdout.contains("added transition `InReview::reject`"),
        "{stdout}"
    );
    assert!(
        stdout.contains("persisted rows/events may need migration"),
        "{stdout}"
    );
}

#[test]
fn graph_diff_json_rejects_duplicate_transition_sites() {
    let temp_dir = std::env::temp_dir().join(format!(
        "statum-graph-diff-duplicate-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
    let baseline = temp_dir.join("baseline.json");
    let current = temp_dir.join("current.json");

    std::fs::write(&baseline, baseline_snapshot()).expect("baseline should be written");
    std::fs::write(&current, duplicate_site_snapshot()).expect("current should be written");

    let output = cargo_statum()
        .args([
            "graph",
            "diff",
            "--baseline",
            baseline.to_str().expect("baseline path should be utf8"),
            "--current",
            current.to_str().expect("current path should be utf8"),
            "--format",
            "json",
        ])
        .output()
        .expect("cargo-statum should run");

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf8");
    assert!(
        stderr.contains("duplicate transition site `Draft::submit`"),
        "{stderr}"
    );
}

fn baseline_snapshot() -> &'static str {
    r#"{
  "snapshot_version": "v1",
  "package": "statum-examples",
  "machine_key": "example::DocumentMachine",
  "feature_set": { "cargo_features": [], "target": "x86_64-unknown-linux-gnu" },
  "graph": {
    "version": "v1",
    "authority": "cfg_pruned_macro_input",
    "unsupported_cases": ["runtime_only_transitions"],
    "machine": { "module_path": "example", "rust_type_path": "example::DocumentMachine", "label": null, "description": null, "fields": [] },
    "states": [
      { "rust_name": "Draft", "label": null, "description": null, "has_data": true, "fields": [] },
      { "rust_name": "InReview", "label": null, "description": null, "has_data": true, "fields": [] },
      { "rust_name": "Archived", "label": null, "description": null, "has_data": false, "fields": [] }
    ],
    "transitions": [
      { "method_name": "submit", "label": null, "description": null, "from_state": "Draft", "to_states": ["InReview"] },
      { "method_name": "archive", "label": null, "description": null, "from_state": "InReview", "to_states": ["Archived"] }
    ]
  }
}"#
}

fn current_snapshot() -> &'static str {
    r#"{
  "snapshot_version": "v1",
  "package": "statum-examples",
  "machine_key": "example::DocumentMachine",
  "feature_set": { "cargo_features": [], "target": "x86_64-unknown-linux-gnu" },
  "graph": {
    "version": "v1",
    "authority": "cfg_pruned_macro_input",
    "unsupported_cases": ["runtime_only_transitions"],
    "machine": { "module_path": "example", "rust_type_path": "example::DocumentMachine", "label": null, "description": null, "fields": [] },
    "states": [
      { "rust_name": "Draft", "label": null, "description": null, "has_data": true, "fields": [] },
      { "rust_name": "InReview", "label": null, "description": null, "has_data": true, "fields": [] },
      { "rust_name": "Rejected", "label": null, "description": null, "has_data": false, "fields": [] }
    ],
    "transitions": [
      { "method_name": "submit", "label": null, "description": null, "from_state": "Draft", "to_states": ["InReview"] },
      { "method_name": "reject", "label": null, "description": null, "from_state": "InReview", "to_states": ["Rejected"] }
    ]
  }
}"#
}

fn duplicate_site_snapshot() -> &'static str {
    r#"{
  "snapshot_version": "v1",
  "package": "statum-examples",
  "machine_key": "example::DocumentMachine",
  "feature_set": { "cargo_features": [], "target": "x86_64-unknown-linux-gnu" },
  "graph": {
    "version": "v1",
    "authority": "cfg_pruned_macro_input",
    "unsupported_cases": ["runtime_only_transitions"],
    "machine": { "module_path": "example", "rust_type_path": "example::DocumentMachine", "label": null, "description": null, "fields": [] },
    "states": [
      { "rust_name": "Draft", "label": null, "description": null, "has_data": true, "fields": [] },
      { "rust_name": "InReview", "label": null, "description": null, "has_data": true, "fields": [] },
      { "rust_name": "Rejected", "label": null, "description": null, "has_data": false, "fields": [] }
    ],
    "transitions": [
      { "method_name": "submit", "label": null, "description": null, "from_state": "Draft", "to_states": ["InReview"] },
      { "method_name": "submit", "label": null, "description": null, "from_state": "Draft", "to_states": ["Rejected"] }
    ]
  }
}"#
}
