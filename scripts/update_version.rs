// cargo-deps: regex="1", toml="0.8.8"

extern crate toml;
extern crate regex;

use std::env;
use std::fs;
use regex::Regex;
use toml::Value;

const PUBLISHED_CRATES: [&str; 3] = [
    "statum-core",
    "statum-macros",
    "statum",
];
const VERSIONED_SNIPPET_FILES: [&str; 4] = [
    "README.md",
    "statum/README.md",
    "statum-core/README.md",
    "statum-macros/README.md",
];

fn parse_semver_triplet(input: &str, field: &str) -> Result<[u32; 3], Box<dyn std::error::Error>> {
    let parts: Vec<u32> = input
        .split('.')
        .map(|s| s.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 3 {
        return Err(format!("{field} must be in format x.y.z").into());
    }
    Ok([parts[0], parts[1], parts[2]])
}

fn apply_internal_dep_versions(table_value: &mut Value, new_version: &str) {
    let Some(table) = table_value.as_table_mut() else {
        return;
    };

    for dep_name in PUBLISHED_CRATES {
        let Some(dep_value) = table.get_mut(dep_name) else {
            continue;
        };

        match dep_value {
            Value::String(_) => {
                *dep_value = Value::String(new_version.to_string());
            }
            Value::Table(dep_table) => {
                if dep_table.contains_key("path") || dep_table.contains_key("version") {
                    dep_table.insert("version".to_string(), Value::String(new_version.to_string()));
                }
            }
            _ => {}
        }
    }
}

fn apply_workspace_versions(doc: &mut Value, new_version: &str) {
    let Some(workspace) = doc.get_mut("workspace") else {
        return;
    };
    let Some(workspace_table) = workspace.as_table_mut() else {
        return;
    };

    if let Some(package) = workspace_table.get_mut("package") {
        if let Some(package_table) = package.as_table_mut() {
            package_table.insert("version".to_string(), Value::String(new_version.to_string()));
        }
    }

    if let Some(dependencies) = workspace_table.get_mut("dependencies") {
        apply_internal_dep_versions(dependencies, new_version);
    }
}

fn update_install_snippets(file_path: &str, new_version: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut content = fs::read_to_string(file_path)?;
    let replacements = [
        (
            Regex::new(r#"statum = "\d+\.\d+\.\d+""#)?,
            format!("statum = \"{new_version}\""),
        ),
        (
            Regex::new(r#"statum-core = "\d+\.\d+\.\d+""#)?,
            format!("statum-core = \"{new_version}\""),
        ),
        (
            Regex::new(r#"statum-macros = "\d+\.\d+\.\d+""#)?,
            format!("statum-macros = \"{new_version}\""),
        ),
        (
            Regex::new(r#"version = "\d+\.\d+\.\d+""#)?,
            format!("version = \"{new_version}\""),
        ),
    ];

    for (regex, replacement) in replacements {
        content = regex.replace_all(&content, replacement.as_str()).into_owned();
    }

    fs::write(file_path, content)?;
    println!("Updated install snippet in {file_path}");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <target_version> (e.g. 1.0.0)", args[0]);
        std::process::exit(1);
    }

    let new_version = args[1].trim().to_string();
    parse_semver_triplet(&new_version, "Target version")?;
    let root_cargo_path = "Cargo.toml";
    let cargo_content = fs::read_to_string(root_cargo_path)?;
    let cargo_toml: Value = toml::from_str(&cargo_content)?;
    let current_version = cargo_toml["workspace"]["package"]["version"]
        .as_str()
        .ok_or("Version not found")?;

    if current_version == new_version {
        return Err(format!("Target version matches current version: {current_version}").into());
    }
    println!("Updating versions from {} to {}", current_version, new_version);

    let mut root_doc: Value = toml::from_str(&cargo_content)?;
    apply_workspace_versions(&mut root_doc, &new_version);
    fs::write(root_cargo_path, toml::to_string_pretty(&root_doc)?)?;
    println!("Updated version metadata in {}", root_cargo_path);

    for file_path in VERSIONED_SNIPPET_FILES {
        update_install_snippets(file_path, &new_version)?;
    }

    Ok(())
}
