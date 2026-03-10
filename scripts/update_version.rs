// cargo-deps: toml="0.8.8"

extern crate toml;

use std::env;
use std::fs;
use toml::Value;

const PUBLISHED_CRATES: [&str; 5] = [
    "module_path_extractor",
    "macro_registry",
    "statum-core",
    "statum-macros",
    "statum",
];
const ALL_CRATES: [&str; 6] = [
    "module_path_extractor",
    "macro_registry",
    "statum-core",
    "statum-macros",
    "statum",
    "statum-examples",
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

fn apply_internal_versions(doc: &mut Value, new_version: &str) {
    if let Some(package) = doc.get_mut("package") {
        if let Some(package_table) = package.as_table_mut() {
            package_table.insert("version".to_string(), Value::String(new_version.to_string()));
        }
    }

    for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(section_value) = doc.get_mut(section) {
            apply_internal_dep_versions(section_value, new_version);
        }
    }

    if let Some(targets) = doc.get_mut("target") {
        if let Some(targets_table) = targets.as_table_mut() {
            for (_, target_cfg) in targets_table.iter_mut() {
                let Some(target_table) = target_cfg.as_table_mut() else {
                    continue;
                };
                for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(section_value) = target_table.get_mut(section) {
                        apply_internal_dep_versions(section_value, new_version);
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <target_version> (e.g. 1.0.0)", args[0]);
        std::process::exit(1);
    }

    let new_version = args[1].trim().to_string();
    parse_semver_triplet(&new_version, "Target version")?;

    let root_cargo_path = "statum/Cargo.toml";
    let cargo_content = fs::read_to_string(root_cargo_path)?;
    let cargo_toml: Value = toml::from_str(&cargo_content)?;
    let current_version = cargo_toml["package"]["version"]
        .as_str()
        .ok_or("Version not found")?;

    if current_version == new_version {
        return Err(format!("Target version matches current version: {current_version}").into());
    }
    println!("Updating versions from {} to {}", current_version, new_version);

    for crate_path in &ALL_CRATES {
        let cargo_path = format!("{}/Cargo.toml", crate_path);
        let content = fs::read_to_string(&cargo_path)?;
        let mut doc: Value = toml::from_str(&content)?;

        apply_internal_versions(&mut doc, &new_version);

        fs::write(&cargo_path, toml::to_string_pretty(&doc)?)?;
        println!("Updated version metadata in {}", cargo_path);
    }

    Ok(())
}
