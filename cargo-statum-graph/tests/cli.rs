use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn codebase_command_accepts_workspace_dir_and_writes_bundle_into_workspace_root() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_fixture(fixture_dir.path());

    let status = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("codebase")
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
    assert!(json.contains("\"validator_entries\""));
    assert!(json.contains("workflow::Machine"));
    assert!(json.contains("task::Machine"));
}

#[test]
fn codebase_command_fails_closed_for_duplicate_machine_paths_across_workspace_members() {
    let fixture_dir = tempdir().expect("fixture tempdir");
    write_duplicate_machine_path_fixture(fixture_dir.path());

    let output = Command::new(env!("CARGO_BIN_EXE_cargo-statum-graph"))
        .arg("codebase")
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
