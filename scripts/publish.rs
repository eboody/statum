// cargo-deps: toml="0.8.8"

extern crate toml;

use std::fs;
use std::io;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use toml::Value;

const PUBLISH_ORDER: [&str; 5] = [
    "module_path_extractor",
    "macro_registry",
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
    matches!(crate_name, "module_path_extractor" | "statum-core")
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
    let cargo_content = fs::read_to_string(format!("{crate_name}/Cargo.toml"))?;
    let cargo_toml: Value = toml::from_str(&cargo_content)?;
    cargo_toml["package"]["version"]
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Version not found".into())
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

fn read_line_trimmed() -> Result<String, Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ensure_clean_worktree()?;

    println!("Enter target version (e.g. 1.0.0):");
    let target_version = read_line_trimmed()?;
    if target_version.is_empty() {
        return Err("Target version cannot be empty".into());
    }

    println!("\nUpdating versions...");
    run(
        {
            let mut cmd = Command::new("cargo");
            cmd.args(["script", "scripts/update_version.rs", "--", &target_version]);
            cmd
        },
        "Version update",
    )?;

    let version = verify_versions_match()?;
    println!("✓ All publishable crates are aligned at version {version}");

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
            cmd.args(["test", "--workspace"]);
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

    println!(
        "\nPublish readiness checks passed for version {version}. Type 'publish' to continue with actual publish:"
    );
    let confirm = read_line_trimmed()?;
    if confirm != "publish" {
        return Err("Publish aborted by user".into());
    }

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
