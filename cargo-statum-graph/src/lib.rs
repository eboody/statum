use std::env;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use cargo_metadata::{Metadata, MetadataCommand, Package, PackageId};
use statum_graph::CodebaseDoc;
use tempfile::TempDir;

mod inspect;

const GRAPH_EXTENSIONS: [&str; 4] = ["mmd", "dot", "puml", "json"];
const GRAPH_PACKAGE_NAME: &str = "statum-graph";
const HELPER_PACKAGE_NAME: &str = "cargo-statum-graph";
const NO_LINKED_MACHINES_MESSAGE: &str = "statum-graph: no linked state machines were found in the target workspace. This can mean the workspace has no Statum machines, or that it depends on incompatible `statum`, `statum-core`, or `statum-graph` versions so linked inventories do not unify. If you expected machines here, ensure those crates use compatible versions.";
const NO_TTY_INSPECT_MESSAGE: &str =
    "statum-graph inspect requires an interactive terminal on stdin and stdout.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    pub input_path: PathBuf,
    pub package: Option<String>,
    pub out_dir: Option<PathBuf>,
    pub stem: String,
    pub patch_statum_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectOptions {
    pub input_path: PathBuf,
    pub package: Option<String>,
    pub patch_statum_root: Option<PathBuf>,
}

#[derive(Debug)]
pub enum Error {
    CurrentDir(io::Error),
    Metadata(cargo_metadata::Error),
    PackageNotFound {
        manifest_path: PathBuf,
        package: String,
    },
    AmbiguousPackage {
        manifest_path: PathBuf,
        candidates: Vec<String>,
    },
    PackageHasNoLibrary {
        manifest_path: PathBuf,
        package: String,
    },
    InvalidStem {
        stem: String,
    },
    NonUtf8Path {
        role: &'static str,
        path: PathBuf,
    },
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    RunnerFailed {
        operation: &'static str,
        manifest_path: PathBuf,
        status: ExitStatus,
        details: Option<String>,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentDir(source) => {
                write!(formatter, "failed to read current directory: {source}")
            }
            Self::Metadata(source) => write!(formatter, "failed to load cargo metadata: {source}"),
            Self::PackageNotFound {
                manifest_path,
                package,
            } => write!(
                formatter,
                "manifest `{}` does not contain package `{package}`",
                manifest_path.display()
            ),
            Self::AmbiguousPackage {
                manifest_path,
                candidates,
            } => {
                if candidates.is_empty() {
                    write!(
                        formatter,
                        "manifest `{}` does not contain a library package",
                        manifest_path.display()
                    )
                } else {
                    write!(
                        formatter,
                        "manifest `{}` does not identify one library package; choose one of: {}",
                        manifest_path.display(),
                        candidates.join(", ")
                    )
                }
            }
            Self::PackageHasNoLibrary {
                manifest_path,
                package,
            } => write!(
                formatter,
                "package `{package}` from manifest `{}` does not expose a library target",
                manifest_path.display()
            ),
            Self::InvalidStem { stem } => write!(
                formatter,
                "invalid output stem `{stem}`: expected a simple file name without path separators"
            ),
            Self::NonUtf8Path { role, path } => write!(
                formatter,
                "cannot generate runner {role} from non-UTF-8 path `{}`",
                path.display()
            ),
            Self::Io {
                action,
                path,
                source,
            } => write!(
                formatter,
                "failed to {action} `{}`: {source}",
                path.display()
            ),
            Self::RunnerFailed {
                operation,
                manifest_path,
                status,
                details,
            } => match details {
                Some(details) => write!(
                    formatter,
                    "{operation} for `{}` failed:\n{details}",
                    manifest_path.display()
                ),
                None => write!(
                    formatter,
                    "{operation} for `{}` failed with status {status}",
                    manifest_path.display()
                ),
            },
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDir(source) => Some(source),
            Self::Metadata(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::PackageNotFound { .. }
            | Self::AmbiguousPackage { .. }
            | Self::PackageHasNoLibrary { .. }
            | Self::InvalidStem { .. }
            | Self::NonUtf8Path { .. }
            | Self::RunnerFailed { .. } => None,
        }
    }
}

pub fn run(options: Options) -> Result<Vec<PathBuf>, Error> {
    validate_output_stem(&options.stem)?;
    let prepared = prepare_run(
        &options.input_path,
        options.package.as_deref(),
        options.patch_statum_root.as_deref(),
    )?;
    let out_dir = resolve_out_dir(&prepared.input, options.out_dir.as_deref())?;
    let temp_dir = new_temp_runner_dir()?;
    write_runner_project(
        temp_dir.path(),
        &prepared.selections,
        RunnerMode::Export {
            out_dir: &out_dir,
            stem: &options.stem,
        },
        prepared.patch_root.as_deref(),
    )?;
    run_runner(
        temp_dir.path().join("Cargo.toml"),
        &prepared.input.manifest_path,
        "codebase export",
    )?;

    Ok(bundle_paths(&out_dir, &options.stem))
}

pub fn inspect(options: InspectOptions) -> Result<(), Error> {
    let prepared = prepare_run(
        &options.input_path,
        options.package.as_deref(),
        options.patch_statum_root.as_deref(),
    )?;
    let temp_dir = new_temp_runner_dir()?;
    let workspace_label = prepared.input.manifest_path.display().to_string();
    write_runner_project(
        temp_dir.path(),
        &prepared.selections,
        RunnerMode::Inspect {
            workspace_label: &workspace_label,
        },
        prepared.patch_root.as_deref(),
    )?;
    run_runner(
        temp_dir.path().join("Cargo.toml"),
        &prepared.input.manifest_path,
        "inspect session",
    )
}

pub fn run_inspector(doc: CodebaseDoc, workspace_label: String) -> io::Result<()> {
    inspect::run(doc, workspace_label)
}

fn load_metadata(manifest_path: &Path) -> Result<Metadata, Error> {
    MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()
        .map_err(Error::Metadata)
}

fn select_packages(
    metadata: &Metadata,
    input: &ResolvedInput,
    requested: Option<&str>,
) -> Result<Vec<SelectedPackage>, Error> {
    let manifest_path = input.manifest_path.as_path();
    if let Some(package) = requested {
        let selected = metadata
            .packages
            .iter()
            .find(|candidate| candidate.name.as_ref() == package)
            .ok_or_else(|| Error::PackageNotFound {
                manifest_path: manifest_path.to_path_buf(),
                package: package.to_owned(),
            })?;
        return SelectedPackage::new(selected, manifest_path).map(|package| vec![package]);
    }

    if manifest_path == workspace_root_manifest(metadata) {
        let mut packages = workspace_packages(metadata, &metadata.workspace_members)
            .into_iter()
            .filter(|package| has_library_target(package))
            .collect::<Vec<_>>();
        packages.sort_by(|left, right| {
            left.name
                .as_ref()
                .cmp(right.name.as_ref())
                .then_with(|| left.manifest_path.cmp(&right.manifest_path))
        });

        if packages.is_empty() {
            return Err(Error::AmbiguousPackage {
                manifest_path: manifest_path.to_path_buf(),
                candidates: Vec::new(),
            });
        }

        return packages
            .into_iter()
            .map(|package| SelectedPackage::new(package, manifest_path))
            .collect();
    }

    if let Some(root_package) = metadata.root_package() {
        if has_library_target(root_package) {
            return SelectedPackage::new(root_package, manifest_path).map(|package| vec![package]);
        }
    }

    let default_members = workspace_packages(metadata, &metadata.workspace_default_members);
    let default_library_members = default_members
        .into_iter()
        .filter(|package| has_library_target(package))
        .collect::<Vec<_>>();
    if default_library_members.len() == 1 {
        return SelectedPackage::new(default_library_members[0], manifest_path)
            .map(|package| vec![package]);
    }

    let workspace_members = workspace_packages(metadata, &metadata.workspace_members);
    let library_members = workspace_members
        .into_iter()
        .filter(|package| has_library_target(package))
        .collect::<Vec<_>>();

    match library_members.as_slice() {
        [package] => SelectedPackage::new(package, manifest_path).map(|package| vec![package]),
        [] => Err(Error::AmbiguousPackage {
            manifest_path: manifest_path.to_path_buf(),
            candidates: Vec::new(),
        }),
        _ => Err(Error::AmbiguousPackage {
            manifest_path: manifest_path.to_path_buf(),
            candidates: library_members
                .iter()
                .map(|package| package.name.to_string())
                .collect(),
        }),
    }
}

fn workspace_root_manifest(metadata: &Metadata) -> PathBuf {
    normalize_absolute_path(&metadata.workspace_root.as_std_path().join("Cargo.toml"))
}

fn workspace_packages<'a>(metadata: &'a Metadata, ids: &[PackageId]) -> Vec<&'a Package> {
    ids.iter()
        .filter_map(|id| metadata.packages.iter().find(|package| package.id == *id))
        .collect()
}

fn has_library_target(package: &Package) -> bool {
    package.targets.iter().any(|target| {
        target.kind.iter().any(|kind| {
            matches!(
                kind,
                cargo_metadata::TargetKind::Lib
                    | cargo_metadata::TargetKind::RLib
                    | cargo_metadata::TargetKind::DyLib
            )
        })
    })
}

fn resolve_out_dir(input: &ResolvedInput, out_dir: Option<&Path>) -> Result<PathBuf, Error> {
    match out_dir {
        Some(path) => absolutize(path).map_err(Error::CurrentDir),
        None => Ok(input.default_output_dir.clone()),
    }
}

fn prepare_run(
    input_path: &Path,
    requested_package: Option<&str>,
    patch_statum_root: Option<&Path>,
) -> Result<PreparedRun, Error> {
    let input_path = absolutize(input_path).map_err(Error::CurrentDir)?;
    let input = resolve_input(&input_path);
    let metadata = load_metadata(&input.manifest_path)?;
    let selections = select_packages(&metadata, &input, requested_package)?;
    let patch_root = match patch_statum_root {
        Some(path) => Some(absolutize(path).map_err(Error::CurrentDir)?),
        None => detect_patch_root(),
    };

    Ok(PreparedRun {
        input,
        selections,
        patch_root,
    })
}

fn detect_patch_root() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir.parent()?;
    if looks_like_statum_workspace(candidate) {
        Some(candidate.to_path_buf())
    } else {
        None
    }
}

fn looks_like_statum_workspace(path: &Path) -> bool {
    [
        "Cargo.toml",
        "statum/Cargo.toml",
        "statum-core/Cargo.toml",
        "statum-graph/Cargo.toml",
        "statum-macros/Cargo.toml",
    ]
    .into_iter()
    .all(|relative| path.join(relative).is_file())
}

fn resolve_input(path: &Path) -> ResolvedInput {
    if path.is_dir() {
        ResolvedInput {
            manifest_path: path.join("Cargo.toml"),
            default_output_dir: path.to_path_buf(),
        }
    } else {
        ResolvedInput {
            manifest_path: path.to_path_buf(),
            default_output_dir: path
                .parent()
                .expect("absolute file path should have a parent")
                .to_path_buf(),
        }
    }
}

fn write_runner_project(
    runner_dir: &Path,
    selections: &[SelectedPackage],
    mode: RunnerMode<'_>,
    patch_root: Option<&Path>,
) -> Result<(), Error> {
    let src_dir = runner_dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|source| Error::Io {
        action: "create runner source directory",
        path: src_dir.clone(),
        source,
    })?;

    let manifest_path = runner_dir.join("Cargo.toml");
    let manifest = build_runner_manifest(selections, patch_root)?;
    fs::write(&manifest_path, manifest).map_err(|source| Error::Io {
        action: "write generated runner manifest",
        path: manifest_path.clone(),
        source,
    })?;

    let main_path = src_dir.join("main.rs");
    let main = build_runner_main(selections, mode)?;
    fs::write(&main_path, main).map_err(|source| Error::Io {
        action: "write generated runner source",
        path: main_path.clone(),
        source,
    })?;

    Ok(())
}

fn build_runner_manifest(
    selections: &[SelectedPackage],
    patch_root: Option<&Path>,
) -> Result<String, Error> {
    let mut manifest = String::from(
        "[package]\nname = \"statum-graph-runner\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\n",
    );
    for (index, selection) in selections.iter().enumerate() {
        manifest.push_str(&format!(
            "{} = {{ package = {}, path = {} }}\n",
            selection.dependency_alias(index),
            toml_str(&selection.package_name),
            toml_path(&selection.manifest_dir, "dependency package path")?,
        ));
    }

    match patch_root {
        Some(root) => {
            if !selections
                .iter()
                .any(|selection| selection.package_name == GRAPH_PACKAGE_NAME)
            {
                manifest.push_str(&format!(
                    "statum-graph = {{ path = {} }}\n",
                    toml_path(root.join(GRAPH_PACKAGE_NAME), "patched statum-graph path")?
                ));
            }
            if !selections
                .iter()
                .any(|selection| selection.package_name == HELPER_PACKAGE_NAME)
            {
                manifest.push_str(&format!(
                    "cargo-statum-graph = {{ path = {} }}\n",
                    toml_path(
                        root.join(HELPER_PACKAGE_NAME),
                        "patched cargo-statum-graph path",
                    )?
                ));
            }
            push_patch_tables(&mut manifest, root)?;
        }
        None => {
            if !selections
                .iter()
                .any(|selection| selection.package_name == GRAPH_PACKAGE_NAME)
            {
                manifest.push_str(&format!(
                    "statum-graph = {{ version = {} }}\n",
                    toml_str(&format!("={}", env!("CARGO_PKG_VERSION")))
                ));
            }
            if !selections
                .iter()
                .any(|selection| selection.package_name == HELPER_PACKAGE_NAME)
            {
                manifest.push_str(&format!(
                    "cargo-statum-graph = {{ version = {} }}\n",
                    toml_str(&format!("={}", env!("CARGO_PKG_VERSION")))
                ));
            }
        }
    }

    Ok(manifest)
}

fn push_patch_tables(manifest: &mut String, root: &Path) -> Result<(), Error> {
    for source in ["crates-io", "https://github.com/eboody/statum"] {
        if source == "crates-io" {
            manifest.push_str("\n[patch.crates-io]\n");
        } else {
            manifest.push_str(&format!("\n[patch.{}]\n", toml_str(source)));
        }
        for package in [
            "macro_registry",
            "module_path_extractor",
            "statum",
            "statum-core",
            "statum-graph",
            "statum-macros",
        ] {
            manifest.push_str(&format!(
                "{package} = {{ path = {} }}\n",
                toml_path(root.join(package), "patched workspace package path")?
            ));
        }
    }

    Ok(())
}

fn build_runner_main(
    selections: &[SelectedPackage],
    mode: RunnerMode<'_>,
) -> Result<String, Error> {
    let mut source = String::from("#[allow(unused_imports)]\n");
    source.push_str("use std::io::IsTerminal as _;\n");
    for (index, selection) in selections.iter().enumerate() {
        source.push_str(&format!(
            "use {} as _;\n",
            selection.dependency_alias(index)
        ));
    }
    source.push_str("\nfn main() -> std::process::ExitCode {\n");
    source.push_str("    match run() {\n");
    source.push_str("        Ok(()) => std::process::ExitCode::SUCCESS,\n");
    source.push_str("        Err(error) => {\n");
    source.push_str("            eprintln!(\"{}\", error);\n");
    source.push_str("            std::process::ExitCode::FAILURE\n");
    source.push_str("        }\n");
    source.push_str("    }\n");
    source.push_str("}\n\n");
    source.push_str("fn run() -> Result<(), Box<dyn std::error::Error>> {\n");
    source.push_str("    let doc = statum_graph::CodebaseDoc::linked()?;\n");
    source.push_str("    if doc.machines().is_empty() {\n");
    source.push_str("        return Err(std::io::Error::other(");
    source.push_str(&rust_str(NO_LINKED_MACHINES_MESSAGE));
    source.push_str(").into());\n");
    source.push_str("    }\n");
    match mode {
        RunnerMode::Export { out_dir, stem } => {
            source.push_str("    statum_graph::codebase::render::write_all_to_dir(\n");
            source.push_str("        &doc,\n");
            source.push_str(&format!(
                "        {},\n",
                rust_path(out_dir, "output directory")?
            ));
            source.push_str(&format!("        {},\n", rust_str(stem)));
            source.push_str("    )?;\n");
        }
        RunnerMode::Inspect { workspace_label } => {
            source.push_str(
                "    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {\n",
            );
            source.push_str("        return Err(std::io::Error::other(");
            source.push_str(&rust_str(NO_TTY_INSPECT_MESSAGE));
            source.push_str(").into());\n");
            source.push_str("    }\n");
            source.push_str("    cargo_statum_graph::run_inspector(\n");
            source.push_str("        doc,\n");
            source.push_str(&format!(
                "        {}.to_owned(),\n",
                rust_str(workspace_label)
            ));
            source.push_str("    )?;\n");
        }
    }
    source.push_str("    Ok(())\n");
    source.push_str("}\n");
    Ok(source)
}

fn run_runner(
    runner_manifest_path: PathBuf,
    target_manifest_path: &Path,
    operation: &'static str,
) -> Result<(), Error> {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&runner_manifest_path)
        .output()
        .map_err(|source| Error::Io {
            action: "run generated cargo runner",
            path: runner_manifest_path.clone(),
            source,
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(Error::RunnerFailed {
            operation,
            manifest_path: target_manifest_path.to_path_buf(),
            status: output.status,
            details: normalize_runner_failure_details(&output.stderr, &output.stdout),
        })
    }
}

fn normalize_runner_failure_details(stderr: &[u8], stdout: &[u8]) -> Option<String> {
    let text = if stderr.is_empty() {
        String::from_utf8_lossy(stdout).into_owned()
    } else {
        String::from_utf8_lossy(stderr).into_owned()
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(
            trimmed
                .strip_prefix("Error: ")
                .unwrap_or(trimmed)
                .to_owned(),
        )
    }
}

fn bundle_paths(out_dir: &Path, stem: &str) -> Vec<PathBuf> {
    GRAPH_EXTENSIONS
        .into_iter()
        .map(|extension| out_dir.join(format!("{stem}.{extension}")))
        .collect()
}

fn absolutize(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    Ok(normalize_absolute_path(&absolute))
}

fn rust_path(value: &Path, role: &'static str) -> Result<String, Error> {
    Ok(rust_str(path_utf8(value, role)?))
}

fn toml_path(value: impl AsRef<Path>, role: &'static str) -> Result<String, Error> {
    Ok(toml_str(path_utf8(value.as_ref(), role)?))
}

fn rust_str(value: &str) -> String {
    let escaped: String = value.chars().flat_map(char::escape_default).collect();
    format!("\"{escaped}\"")
}

fn toml_str(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\u{08}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{0C}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            control if control.is_control() => {
                let code = control as u32;
                if code <= 0xFFFF {
                    write!(&mut escaped, "\\u{code:04X}")
                        .expect("writing to a String should not fail");
                } else {
                    write!(&mut escaped, "\\U{code:08X}")
                        .expect("writing to a String should not fail");
                }
            }
            other => escaped.push(other),
        }
    }

    format!("\"{escaped}\"")
}

fn validate_output_stem(stem: &str) -> Result<(), Error> {
    let mut components = Path::new(stem).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(Error::InvalidStem {
            stem: stem.to_owned(),
        }),
    }
}

fn path_utf8<'a>(path: &'a Path, role: &'static str) -> Result<&'a str, Error> {
    path.to_str().ok_or_else(|| Error::NonUtf8Path {
        role,
        path: path.to_path_buf(),
    })
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    debug_assert!(
        path.is_absolute(),
        "path should be absolute before normalization"
    );

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                normalized.push(std::path::MAIN_SEPARATOR.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some() {
                    normalized.pop();
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }

    normalized
}

struct SelectedPackage {
    package_name: String,
    manifest_dir: PathBuf,
}

impl SelectedPackage {
    fn new(package: &Package, manifest_path: &Path) -> Result<Self, Error> {
        if !has_library_target(package) {
            return Err(Error::PackageHasNoLibrary {
                manifest_path: manifest_path.to_path_buf(),
                package: package.name.to_string(),
            });
        }

        Ok(Self {
            package_name: package.name.to_string(),
            manifest_dir: package
                .manifest_path
                .as_std_path()
                .parent()
                .expect("package manifest should have a parent")
                .to_path_buf(),
        })
    }

    fn dependency_alias(&self, index: usize) -> String {
        if self.package_name == GRAPH_PACKAGE_NAME {
            Self::graph_dependency_alias().to_owned()
        } else if self.package_name == HELPER_PACKAGE_NAME {
            Self::helper_dependency_alias().to_owned()
        } else {
            format!("graph_target_{index}")
        }
    }

    fn graph_dependency_alias() -> &'static str {
        "statum_graph"
    }

    fn helper_dependency_alias() -> &'static str {
        "cargo_statum_graph"
    }
}

struct PreparedRun {
    input: ResolvedInput,
    selections: Vec<SelectedPackage>,
    patch_root: Option<PathBuf>,
}

#[derive(Clone, Copy)]
enum RunnerMode<'a> {
    Export { out_dir: &'a Path, stem: &'a str },
    Inspect { workspace_label: &'a str },
}

struct ResolvedInput {
    manifest_path: PathBuf,
    default_output_dir: PathBuf,
}

fn new_temp_runner_dir() -> Result<TempDir, Error> {
    TempDir::new().map_err(|source| Error::Io {
        action: "create temporary runner directory",
        path: env::temp_dir(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_str_escapes_control_characters() {
        assert_eq!(
            rust_str("line 1\n\"quoted\"\t\\tail"),
            "\"line 1\\n\\\"quoted\\\"\\t\\\\tail\""
        );
    }

    #[test]
    fn toml_str_escapes_control_characters() {
        assert_eq!(
            toml_str("line 1\n\"quoted\"\t\\tail\u{1F}"),
            "\"line 1\\n\\\"quoted\\\"\\t\\\\tail\\u001F\""
        );
    }

    #[cfg(unix)]
    #[test]
    fn rust_path_rejects_non_utf8_path() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![0x66, 0x80, 0x6F]));

        let error = rust_path(&path, "output directory").expect_err("non-UTF-8 path should fail");

        assert!(matches!(
            error,
            Error::NonUtf8Path {
                role: "output directory",
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn toml_path_rejects_non_utf8_path() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![0x66, 0x80, 0x6F]));

        let error =
            toml_path(&path, "dependency package path").expect_err("non-UTF-8 path should fail");

        assert!(matches!(
            error,
            Error::NonUtf8Path {
                role: "dependency package path",
                ..
            }
        ));
    }

    #[test]
    fn absolutize_normalizes_cur_dir_components() {
        let current_dir = env::current_dir().expect("current dir");
        let normalized = absolutize(Path::new(".").join("Cargo.toml").as_path()).expect("path");

        assert_eq!(normalized, current_dir.join("Cargo.toml"));
    }

    #[test]
    fn build_runner_main_supports_inspect_mode() {
        let selections = vec![SelectedPackage {
            package_name: "app".to_owned(),
            manifest_dir: PathBuf::from("/tmp/app"),
        }];

        let source = build_runner_main(
            &selections,
            RunnerMode::Inspect {
                workspace_label: "/tmp/workspace/Cargo.toml",
            },
        )
        .expect("runner source");

        assert!(source.contains("cargo_statum_graph::run_inspector"));
        assert!(source.contains("is_terminal()"));
        assert!(source.contains("/tmp/workspace/Cargo.toml"));
        assert!(!source.contains("write_all_to_dir("));
    }

    #[test]
    fn build_runner_manifest_reuses_selected_helper_dependency() {
        let selections = vec![
            SelectedPackage {
                package_name: GRAPH_PACKAGE_NAME.to_owned(),
                manifest_dir: PathBuf::from("/tmp/graph"),
            },
            SelectedPackage {
                package_name: HELPER_PACKAGE_NAME.to_owned(),
                manifest_dir: PathBuf::from("/tmp/helper"),
            },
        ];

        let manifest = build_runner_manifest(&selections, None).expect("runner manifest");

        assert_eq!(manifest.matches("package = \"statum-graph\"").count(), 1);
        assert_eq!(
            manifest.matches("package = \"cargo-statum-graph\"").count(),
            1
        );
        assert!(manifest.contains("statum_graph = { package = \"statum-graph\""));
        assert!(manifest.contains("cargo_statum_graph = { package = \"cargo-statum-graph\""));
    }
}
