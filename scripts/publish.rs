use std::env;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

const PUBLISH_ORDER: [&str; 3] = [
    "statum-core",
    "statum-macros",
    "statum",
];
const VERSION_EXISTS_WARNING: &str = "already exists on crates.io index";

fn has_publish_conflict(context: &str, stdout: &str, stderr: &str) -> bool {
    context.contains("cargo publish")
        && (stdout.contains(VERSION_EXISTS_WARNING) || stderr.contains(VERSION_EXISTS_WARNING))
}

fn can_publish_dry_run(crate_name: &str) -> bool {
    matches!(crate_name, "statum-core")
}

fn crates_io_version_exists(
    crate_name: &str,
    version: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let url = format!("https://crates.io/api/v1/crates/{crate_name}/{version}");
    let output = Command::new("curl").args(["-fsSI", &url]).output()?;

    if output.status.success() {
        return Ok(true);
    }

    if output.status.code() == Some(22) {
        return Ok(false);
    }

    if !output.stdout.is_empty() {
        println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprintln!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    }
    Err(format!("Failed to query crates.io for {crate_name} {version}").into())
}

fn run(mut cmd: Command, context: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = cmd.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if output.status.success() && !has_publish_conflict(context, &stdout, &stderr) {
        return Ok(());
    }

    if !output.stdout.is_empty() {
        println!("stdout:\n{stdout}");
    }
    if !output.stderr.is_empty() {
        eprintln!("stderr:\n{stderr}");
    }
    Err(format!("{context} failed").into())
}

fn ensure_clean_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err("Failed to inspect git status".into());
    }
    if !output.stdout.is_empty() {
        return Err(
            "Working tree is not clean. Commit or stash changes before running publish script."
                .into(),
        );
    }
    Ok(())
}

fn crate_version(crate_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new("cargo")
        .args(["pkgid", "-p", crate_name])
        .output()?;
    if !output.status.success() {
        return Err(format!("Failed to resolve cargo pkgid for {crate_name}").into());
    }

    let pkgid = String::from_utf8(output.stdout)?;
    pkgid.trim()
        .rsplit_once('#')
        .map(|(_, version)| version.to_owned())
        .ok_or_else(|| format!("Version not found in cargo pkgid output for {crate_name}").into())
}

fn verify_versions_match() -> Result<String, Box<dyn std::error::Error>> {
    let first = crate_version(PUBLISH_ORDER[0])?;
    for crate_name in PUBLISH_ORDER.iter().skip(1) {
        let version = crate_version(crate_name)?;
        if version != first {
            return Err(format!(
                "Version mismatch: {} has {}, expected {}",
                crate_name, version, first
            )
            .into());
        }
    }
    Ok(first)
}

fn ensure_versions_are_unpublished(version: &str) -> Result<(), Box<dyn std::error::Error>> {
    for crate_name in PUBLISH_ORDER {
        if crates_io_version_exists(crate_name, version)? {
            return Err(format!(
                "{crate_name} is already published at version {version}; bump versions before release"
            )
            .into());
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() > 2 {
        return Err(format!(
            "Usage: {} [expected_version]",
            args.first().map(String::as_str).unwrap_or("publish.rs")
        )
        .into());
    }

    ensure_clean_worktree()?;

    let version = verify_versions_match()?;
    if let Some(expected_version) = args.get(1) {
        if expected_version != &version {
            return Err(format!(
                "Workspace is aligned at version {version}, expected {expected_version}"
            )
            .into());
        }
    }

    println!("Publishing aligned workspace version {version}");
    ensure_versions_are_unpublished(&version)?;
    println!("✓ No publishable crate is already on crates.io at version {version}");

    println!("\nRunning pre-publish checks...");
    run(
        {
            let mut cmd = Command::new("cargo");
            cmd.args(["fmt", "--all", "--check"]);
            cmd
        },
        "cargo fmt --check",
    )?;
    run(
        {
            let mut cmd = Command::new("cargo");
            cmd.args([
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ]);
            cmd
        },
        "cargo clippy",
    )?;
    run(
        {
            let mut cmd = Command::new("cargo");
            cmd.args(["test", "--workspace", "--all-features"]);
            cmd
        },
        "cargo test",
    )?;

    println!("\nRunning publish readiness checks in dependency order...");
    for crate_name in PUBLISH_ORDER {
        if can_publish_dry_run(crate_name) {
            println!("Dry-run publishing {crate_name}...");
            run(
                {
                    let mut cmd = Command::new("cargo");
                    cmd.args(["publish", "-p", crate_name, "--dry-run", "--allow-dirty"]);
                    cmd
                },
                &format!("cargo publish --dry-run for {crate_name}"),
            )?;
        } else {
            println!("Inspecting package contents for {crate_name}...");
            run(
                {
                    let mut cmd = Command::new("cargo");
                    cmd.args(["package", "-p", crate_name, "--allow-dirty", "--list"]);
                    cmd
                },
                &format!("cargo package --list for {crate_name}"),
            )?;
        }
    }

    println!("\nPublish readiness checks passed for version {version}.");

    for (idx, crate_name) in PUBLISH_ORDER.iter().enumerate() {
        println!("\nPublishing {crate_name}...");
        run(
            {
                let mut cmd = Command::new("cargo");
                cmd.args(["publish", "-p", crate_name]);
                cmd
            },
            &format!("cargo publish for {crate_name}"),
        )?;

        if idx + 1 != PUBLISH_ORDER.len() {
            println!("Waiting 30 seconds before publishing next crate...");
            sleep(Duration::from_secs(30));
        }
    }

    println!("\n✓ All crates published successfully.");
    Ok(())
}
