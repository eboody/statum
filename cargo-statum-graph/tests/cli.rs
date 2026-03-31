use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn export_command_accepts_workspace_dir_and_writes_bundle_into_workspace_root() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .status()
        .expect("cargo-statum-graph should run");
    assert!(status.success(), "cargo-statum-graph should succeed");

    let mermaid =
        fs::read_to_string(fixture_dir.path().join("codebase.mmd")).expect("mermaid output");
    let dot = fs::read_to_string(fixture_dir.path().join("codebase.dot")).expect("dot output");
    let plantuml =
        fs::read_to_string(fixture_dir.path().join("codebase.puml")).expect("plantuml output");
    let json = fs::read_to_string(fixture_dir.path().join("codebase.json")).expect("json output");

    assert!(mermaid.contains("graph TD"));
    assert!(mermaid.contains("-.->|state_data|"));
    assert!(mermaid.contains("WorkflowRow::into_machine()"));
    assert!(dot.contains("style=dashed"));
    assert!(plantuml.contains("@startuml"));
    assert!(json.contains("\"links\""));
    assert!(json.contains("\"relations\""));
    assert!(json.contains("\"direct_construction_available\""));
    assert!(json.contains("\"validator_entries\""));
    assert!(json.contains("workflow::Machine"));
    assert!(json.contains("task::Machine"));
}

#[test]
fn export_command_accepts_cargo_style_invocation_from_workspace_root() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .current_dir(fixture_dir.path())
        .arg("statum-graph")
        .arg("export")
        .status()
        .expect("cargo-style invocation should run");
    assert!(status.success(), "cargo-style invocation should succeed");

    assert!(fixture_dir.path().join("codebase.mmd").is_file());
    assert!(fixture_dir.path().join("codebase.dot").is_file());
    assert!(fixture_dir.path().join("codebase.puml").is_file());
    assert!(fixture_dir.path().join("codebase.json").is_file());
}

#[test]
fn legacy_codebase_alias_still_exports_workspace_bundle() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("codebase")
        .arg(fixture_dir.path())
        .status()
        .expect("legacy codebase alias should run");
    assert!(status.success(), "legacy codebase alias should succeed");

    assert!(fixture_dir.path().join("codebase.mmd").is_file());
}

#[test]
fn export_command_reuses_one_cached_runner_home_across_repeated_runs() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let first = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .output()
        .expect("first export run should execute");
    assert!(
        first.status.success(),
        "first export run should succeed: {}",
        String::from_utf8_lossy(&first.stderr)
    );

    let runner_root = runner_root(fixture_dir.path());
    let first_entries = runner_entry_names(&runner_root);
    assert_eq!(first_entries.len(), 1, "expected one cached runner home");

    let second = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .output()
        .expect("second export run should execute");
    assert!(
        second.status.success(),
        "second export run should succeed: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    let second_entries = runner_entry_names(&runner_root);
    assert_eq!(second_entries, first_entries);
    assert!(runner_root
        .join(&second_entries[0])
        .join("Cargo.toml")
        .is_file());
    assert!(runner_root
        .join(&second_entries[0])
        .join("src/main.rs")
        .is_file());
}

#[test]
fn suggest_command_reuses_the_same_cached_runner_home_as_export() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let suggest = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("suggest")
        .arg(fixture_dir.path())
        .output()
        .expect("suggest run should execute");
    assert!(
        suggest.status.success(),
        "suggest run should succeed: {}",
        String::from_utf8_lossy(&suggest.stderr)
    );

    let runner_root = runner_root(fixture_dir.path());
    let suggest_entries = runner_entry_names(&runner_root);
    assert_eq!(suggest_entries.len(), 1, "expected one cached runner home");

    let export = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .output()
        .expect("export run should execute");
    assert!(
        export.status.success(),
        "export run should succeed: {}",
        String::from_utf8_lossy(&export.stderr)
    );

    let export_entries = runner_entry_names(&runner_root);
    assert_eq!(export_entries, suggest_entries);
}

#[test]
fn export_command_uses_distinct_cached_runner_homes_for_different_package_sets() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let all_packages = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .output()
        .expect("workspace-wide export run should execute");
    assert!(
        all_packages.status.success(),
        "workspace-wide export run should succeed: {}",
        String::from_utf8_lossy(&all_packages.stderr)
    );

    let app_only = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .arg("--package")
        .arg("fixture-app")
        .output()
        .expect("package-scoped export run should execute");
    assert!(
        app_only.status.success(),
        "package-scoped export run should succeed: {}",
        String::from_utf8_lossy(&app_only.stderr)
    );

    let entries = runner_entry_names(&runner_root(fixture_dir.path()));
    assert_eq!(
        entries.len(),
        2,
        "expected one cached runner per package set"
    );
}

#[test]
fn export_command_fails_closed_for_duplicate_machine_paths_across_workspace_members() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_duplicate_machine_path_fixture(fixture_dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .output()
        .expect("cargo-statum-graph should run");
    assert!(
        !output.status.success(),
        "duplicate machine paths should fail closed"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("duplicate machine path `flow::Machine`"),
        "stderr should report duplicate machine path, got: {stderr}"
    );
    assert!(
        stderr.contains("--package") && stderr.contains("distinct module path"),
        "stderr should report duplicate machine path, got: {stderr}"
    );
}

#[test]
fn export_command_rejects_invalid_output_stem_before_runner_build() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .arg("--stem")
        .arg("../escape")
        .output()
        .expect("cargo-statum-graph should run");
    assert!(!output.status.success(), "invalid stem should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid output stem `../escape`"),
        "stderr should report the invalid stem, got: {stderr}"
    );
    assert!(!fixture_dir.path().join("..").join("escape.mmd").exists());
}

#[test]
fn export_command_fails_closed_when_no_linked_machines_are_found() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_no_machine_fixture(fixture_dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .output()
        .expect("cargo-statum-graph should run");
    assert!(
        !output.status.success(),
        "missing linked machines should fail closed"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no linked state machines were found in the target workspace"),
        "stderr should explain the empty linked inventory, got: {stderr}"
    );
    assert!(
        stderr.contains("compatible versions"),
        "stderr should explain the likely version-mismatch fix, got: {stderr}"
    );
    assert!(!fixture_dir.path().join("codebase.mmd").exists());
    assert!(!fixture_dir.path().join("codebase.dot").exists());
    assert!(!fixture_dir.path().join("codebase.puml").exists());
    assert!(!fixture_dir.path().join("codebase.json").exists());
}

#[test]
fn export_command_does_not_leak_heuristic_only_relations_into_exact_bundle() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_heuristic_only_fixture(fixture_dir.path());

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(fixture_dir.path())
        .status()
        .expect("cargo-statum-graph should run");
    assert!(status.success(), "cargo-statum-graph should succeed");

    let mermaid =
        fs::read_to_string(fixture_dir.path().join("codebase.mmd")).expect("mermaid output");
    let json = fs::read_to_string(fixture_dir.path().join("codebase.json")).expect("json output");

    assert!(
        !mermaid.contains("exact refs:"),
        "heuristic-only couplings must not appear as exact summary edges, got: {mermaid}"
    );
    assert!(
        json.contains("\"relations\": []"),
        "heuristic-only couplings must not appear in the exact JSON relations, got: {json}"
    );
    assert!(
        json.contains("\"links\": []"),
        "heuristic-only couplings must not appear in the exact JSON links, got: {json}"
    );
}

#[test]
fn inspect_command_fails_closed_without_an_interactive_terminal() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("inspect")
        .arg(fixture_dir.path())
        .output()
        .expect("cargo-statum-graph should run");
    assert!(
        !output.status.success(),
        "inspect should fail without a terminal"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("inspect session"),
        "stderr should not be double-wrapped by the parent CLI, got: {stderr}"
    );
    assert!(
        stderr.contains("requires an interactive terminal on stdin and stdout"),
        "stderr should explain the interactive terminal requirement, got: {stderr}"
    );
}

#[test]
fn suggest_command_reports_exact_typed_orchestration_warnings() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("suggest")
        .arg(fixture_dir.path())
        .output()
        .expect("cargo-statum-graph should run");
    assert!(output.status.success(), "suggest should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("composition diagnostics:"),
        "unexpected suggest stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("warning:"),
        "unexpected suggest stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("workflow::Machine"),
        "unexpected suggest stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("task::Machine"),
        "unexpected suggest stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("missing composition role"),
        "unexpected suggest stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("consider `#[machine(role = composition)]`"),
        "unexpected suggest stdout:\n{stdout}"
    );
}

#[test]
fn suggest_command_reports_heuristic_only_composition_candidates() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_heuristic_only_fixture(fixture_dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("suggest")
        .arg(fixture_dir.path())
        .output()
        .expect("cargo-statum-graph should run");
    assert!(output.status.success(), "suggest should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("composition diagnostics: 0 warning, 1 suggestion"));
    assert!(stdout.contains("suggestion: workflow::Machine -> task::Machine"));
    assert!(stdout.contains("heuristic composition candidate"));
    assert!(stdout.contains("heuristic lane"));
}

#[test]
fn export_command_exports_composition_workflow_for_statum_examples() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate should live under workspace root");
    let examples_dir = workspace_root.join("statum-examples");
    let out_dir = tempdir().expect("output tempdir");

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("export")
        .arg(&examples_dir)
        .arg("--out-dir")
        .arg(out_dir.path())
        .output()
        .expect("cargo-statum-graph should run against statum-examples");
    assert!(
        output.status.success(),
        "statum-examples export should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mermaid = fs::read_to_string(out_dir.path().join("codebase.mmd")).expect("mermaid output");
    let json = fs::read_to_string(out_dir.path().join("codebase.json")).expect("json output");

    assert!(
        mermaid.contains("DocumentFlow [composition]"),
        "expected composition machine in mermaid export, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains("composition refs: payload x2, param x2"),
        "expected direct composition summary edge in mermaid export, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains("composition + exact refs: param, param [via]"),
        "expected detached-handoff composition summary edge in mermaid export, got:\n{mermaid}"
    );
    assert!(
        json.contains("example_18_composition_machine::DocumentFlow"),
        "expected composition machine in json export, got:\n{json}"
    );
    assert!(
        json.contains("\"semantic\": \"composition_direct_child\""),
        "expected composition semantic in json export, got:\n{json}"
    );
    assert!(
        json.contains("\"route_name\": \"Publish\""),
        "expected detached publication provenance in json export, got:\n{json}"
    );
}

fn write_fixture(dir: &Path) {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate should live under workspace root");
    let root_manifest = format!(
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/domain\", \"crates/app\"]\n\n[workspace.dependencies]\nstatum = {{ path = {:?} }}\n",
        workspace_root.join("statum")
    );
    let domain_manifest =
        "[package]\nname = \"fixture-domain\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nstatum = { workspace = true }\n";
    let domain_lib = "pub mod task {\n    use statum::{machine, state, transition};\n\n    #[state]\n    pub enum State {\n        Idle,\n        Running,\n    }\n\n    #[machine]\n    pub struct Machine<State> {}\n\n    #[transition]\n    impl Machine<Idle> {\n        pub fn start(self) -> Machine<Running> {\n            self.transition()\n        }\n    }\n}\n";
    let app_manifest = "[package]\nname = \"fixture-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nfixture-domain = { path = \"../domain\" }\nstatum = { workspace = true }\n";
    let app_lib = "pub mod workflow {\n    use fixture_domain::task;\n    use statum::{Error, machine, state, transition, validators};\n\n    #[state]\n    pub enum State {\n        Draft,\n        InProgress(task::Machine<task::Running>),\n        Done,\n    }\n\n    #[machine]\n    pub struct Machine<State> {}\n\n    #[transition]\n    impl Machine<Draft> {\n        pub fn start(self, task: task::Machine<task::Running>) -> Machine<InProgress> {\n            self.transition_with(task)\n        }\n    }\n\n    #[transition]\n    impl Machine<InProgress> {\n        pub fn finish(self) -> Machine<Done> {\n            self.transition()\n        }\n    }\n\n    pub struct WorkflowRow {\n        pub status: &'static str,\n    }\n\n    #[validators(Machine)]\n    impl WorkflowRow {\n        fn is_draft(&self) -> statum::Result<()> {\n            if self.status == \"draft\" {\n                Ok(())\n            } else {\n                Err(Error::InvalidState)\n            }\n        }\n\n        fn is_in_progress(&self) -> statum::Result<task::Machine<task::Running>> {\n            if self.status == \"in_progress\" {\n                Ok(task::Machine::<task::Running>::builder().build())\n            } else {\n                Err(Error::InvalidState)\n            }\n        }\n\n        fn is_done(&self) -> statum::Result<()> {\n            if self.status == \"done\" {\n                Ok(())\n            } else {\n                Err(Error::InvalidState)\n            }\n        }\n    }\n}\n";

    fs::create_dir_all(dir.join("crates/domain/src")).expect("fixture domain src dir");
    fs::create_dir_all(dir.join("crates/app/src")).expect("fixture app src dir");
    fs::write(dir.join("Cargo.toml"), root_manifest).expect("fixture root cargo manifest");
    fs::write(dir.join("crates/domain/Cargo.toml"), domain_manifest)
        .expect("fixture domain cargo manifest");
    fs::write(dir.join("crates/domain/src/lib.rs"), domain_lib).expect("fixture domain lib");
    fs::write(dir.join("crates/app/Cargo.toml"), app_manifest).expect("fixture app cargo manifest");
    fs::write(dir.join("crates/app/src/lib.rs"), app_lib).expect("fixture app lib");
}

fn runner_root(workspace_dir: &Path) -> std::path::PathBuf {
    workspace_dir.join("target/statum-graph/runner")
}

fn runner_entry_names(runner_root: &Path) -> Vec<String> {
    let mut entries = fs::read_dir(runner_root)
        .expect("cached runner root should exist")
        .map(|entry| {
            entry
                .expect("runner entry")
                .file_name()
                .into_string()
                .expect("runner dir name should be UTF-8")
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn write_duplicate_machine_path_fixture(dir: &Path) {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate should live under workspace root");
    let root_manifest = format!(
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/a\", \"crates/b\"]\n\n[workspace.dependencies]\nstatum = {{ path = {:?} }}\n",
        workspace_root.join("statum")
    );
    let crate_manifest_suffix =
        "version = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nstatum = { workspace = true }\n";
    let lib_rs = "pub mod flow {\n    use statum::{machine, state};\n\n    #[state]\n    pub enum State {\n        Draft,\n    }\n\n    #[machine]\n    pub struct Machine<State> {}\n}\n";

    fs::create_dir_all(dir.join("crates/a/src")).expect("fixture a src dir");
    fs::create_dir_all(dir.join("crates/b/src")).expect("fixture b src dir");
    fs::write(dir.join("Cargo.toml"), root_manifest).expect("fixture root cargo manifest");
    fs::write(
        dir.join("crates/a/Cargo.toml"),
        format!("[package]\nname = \"fixture-a\"\n{crate_manifest_suffix}"),
    )
    .expect("fixture a cargo manifest");
    fs::write(
        dir.join("crates/b/Cargo.toml"),
        format!("[package]\nname = \"fixture-b\"\n{crate_manifest_suffix}"),
    )
    .expect("fixture b cargo manifest");
    fs::write(dir.join("crates/a/src/lib.rs"), lib_rs).expect("fixture a lib");
    fs::write(dir.join("crates/b/src/lib.rs"), lib_rs).expect("fixture b lib");
}

fn write_no_machine_fixture(dir: &Path) {
    let root_manifest = "[workspace]\nresolver = \"2\"\nmembers = [\"crates/app\"]\n".to_owned();
    let app_manifest =
        "[package]\nname = \"fixture-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n";
    let app_lib =
        "pub struct PlainData {\n    pub id: u32,\n}\n\npub fn answer() -> u32 {\n    42\n}\n";

    fs::create_dir_all(dir.join("crates/app/src")).expect("fixture app src dir");
    fs::write(dir.join("Cargo.toml"), root_manifest).expect("fixture root cargo manifest");
    fs::write(dir.join("crates/app/Cargo.toml"), app_manifest).expect("fixture app cargo manifest");
    fs::write(dir.join("crates/app/src/lib.rs"), app_lib).expect("fixture app lib");
}

fn write_heuristic_only_fixture(dir: &Path) {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate should live under workspace root");
    let root_manifest = format!(
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/domain\", \"crates/app\"]\n\n[workspace.dependencies]\nstatum = {{ path = {:?} }}\n",
        workspace_root.join("statum")
    );
    let domain_manifest =
        "[package]\nname = \"fixture-domain\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nstatum = { workspace = true }\n";
    let domain_lib = "pub mod task {\n    use statum::{machine, state, transition};\n\n    #[state]\n    pub enum State {\n        Idle,\n        Running,\n    }\n\n    #[machine]\n    pub struct Machine<State> {}\n\n    #[transition]\n    impl Machine<Idle> {\n        pub fn start(self) -> Machine<Running> {\n            self.transition()\n        }\n    }\n}\n";
    let app_manifest = "[package]\nname = \"fixture-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nfixture-domain = { path = \"../domain\" }\nstatum = { workspace = true }\n";
    let app_lib = "pub mod workflow {\n    use fixture_domain::task;\n    use statum::{machine, state, transition};\n\n    #[state]\n    pub enum State {\n        Draft,\n        Done,\n    }\n\n    #[machine]\n    pub struct Machine<State> {}\n\n    #[transition]\n    impl Machine<Draft> {\n        pub fn start(self) -> Machine<Done> {\n            let _builder = task::Machine::<task::Running>::builder();\n            self.transition()\n        }\n    }\n}\n";

    fs::create_dir_all(dir.join("crates/domain/src")).expect("fixture domain src dir");
    fs::create_dir_all(dir.join("crates/app/src")).expect("fixture app src dir");
    fs::write(dir.join("Cargo.toml"), root_manifest).expect("fixture root cargo manifest");
    fs::write(dir.join("crates/domain/Cargo.toml"), domain_manifest)
        .expect("fixture domain cargo manifest");
    fs::write(dir.join("crates/domain/src/lib.rs"), domain_lib).expect("fixture domain lib");
    fs::write(dir.join("crates/app/Cargo.toml"), app_manifest).expect("fixture app cargo manifest");
    fs::write(dir.join("crates/app/src/lib.rs"), app_lib).expect("fixture app lib");
}
