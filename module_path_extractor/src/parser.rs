use std::fs;
use std::path::Path;

use crate::pathing::module_path_from_file_with_root;

fn compose_module_path(base: &str, nested: &str) -> String {
    if base == "crate" {
        nested.to_string()
    } else {
        format!("{base}::{nested}")
    }
}

pub(crate) fn resolve_module_path_from_lines(
    base_module: &str,
    line_modules: &[String],
    line_number: usize,
) -> Option<String> {
    if line_number == 0 {
        return Some(base_module.to_string());
    }

    match line_modules.get(line_number - 1) {
        Some(path) if !path.is_empty() => Some(compose_module_path(base_module, path)),
        _ => Some(base_module.to_string()),
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

fn raw_string_prefix_len(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if bytes.get(start) != Some(&b'r') {
        return None;
    }
    let mut idx = start + 1;
    let mut hashes = 0usize;
    while bytes.get(idx) == Some(&b'#') {
        hashes += 1;
        idx += 1;
    }
    if bytes.get(idx) != Some(&b'"') {
        return None;
    }
    Some((hashes, idx - start + 1))
}

fn raw_identifier_len(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'r') || bytes.get(start + 1) != Some(&b'#') {
        return None;
    }

    let mut idx = start + 2;
    if !is_ident_start(*bytes.get(idx)?) {
        return None;
    }

    idx += 1;
    while idx < bytes.len() && is_ident_continue(bytes[idx]) {
        idx += 1;
    }

    Some(idx - start)
}

fn handle_identifier_token(
    token: &str,
    expect_mod_ident: &mut bool,
    pending_mod_name: &mut Option<String>,
    expect_mod_open: &mut bool,
) {
    if *expect_mod_ident {
        *pending_mod_name = Some(token.to_string());
        *expect_mod_ident = false;
        *expect_mod_open = true;
        return;
    }

    if token == "mod" {
        *expect_mod_ident = true;
        *pending_mod_name = None;
        *expect_mod_open = false;
    } else if *expect_mod_open {
        *pending_mod_name = None;
        *expect_mod_open = false;
    }
}

fn build_line_module_paths(content: &str) -> Vec<String> {
    #[derive(Clone, Copy)]
    enum Mode {
        Normal,
        LineComment,
        BlockComment { depth: usize },
        String { escaped: bool },
        Char { escaped: bool },
        RawString { hashes: usize },
    }

    let bytes = content.as_bytes();
    let mut line_paths = vec![String::new()];
    let mut line = 1usize;
    let mut i = 0usize;
    let mut mode = Mode::Normal;
    let mut brace_stack: Vec<Option<String>> = Vec::new();
    let mut module_stack: Vec<String> = Vec::new();
    let mut expect_mod_ident = false;
    let mut pending_mod_name: Option<String> = None;
    let mut expect_mod_open = false;

    let current_module_path = |stack: &[String]| -> String {
        if stack.is_empty() {
            String::new()
        } else {
            stack.join("::")
        }
    };

    while i < bytes.len() {
        let byte = bytes[i];

        if byte == b'\n' {
            line += 1;
            if line_paths.len() < line {
                line_paths.push(current_module_path(&module_stack));
            } else if let Some(existing) = line_paths.get_mut(line - 1) {
                *existing = current_module_path(&module_stack);
            }
        }

        match mode {
            Mode::LineComment => {
                if byte == b'\n' {
                    mode = Mode::Normal;
                }
                i += 1;
                continue;
            }
            Mode::BlockComment { depth } => {
                if byte == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    mode = Mode::BlockComment { depth: depth + 1 };
                    i += 2;
                    continue;
                }
                if byte == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    if depth == 1 {
                        mode = Mode::Normal;
                    } else {
                        mode = Mode::BlockComment { depth: depth - 1 };
                    }
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            Mode::String { escaped } => {
                if byte == b'\\' && !escaped {
                    mode = Mode::String { escaped: true };
                } else if byte == b'"' && !escaped {
                    mode = Mode::Normal;
                } else {
                    mode = Mode::String { escaped: false };
                }
                i += 1;
                continue;
            }
            Mode::Char { escaped } => {
                if byte == b'\\' && !escaped {
                    mode = Mode::Char { escaped: true };
                } else if byte == b'\'' && !escaped {
                    mode = Mode::Normal;
                } else {
                    mode = Mode::Char { escaped: false };
                }
                i += 1;
                continue;
            }
            Mode::RawString { hashes } => {
                if byte == b'"' {
                    let mut matched = true;
                    for offset in 0..hashes {
                        if bytes.get(i + 1 + offset) != Some(&b'#') {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        mode = Mode::Normal;
                        i += 1 + hashes;
                        continue;
                    }
                }
                i += 1;
                continue;
            }
            Mode::Normal => {}
        }

        if byte == b'/' && bytes.get(i + 1) == Some(&b'/') {
            mode = Mode::LineComment;
            i += 2;
            continue;
        }
        if byte == b'/' && bytes.get(i + 1) == Some(&b'*') {
            mode = Mode::BlockComment { depth: 1 };
            i += 2;
            continue;
        }
        if byte == b'"' {
            mode = Mode::String { escaped: false };
            i += 1;
            continue;
        }
        if byte == b'\'' {
            mode = Mode::Char { escaped: false };
            i += 1;
            continue;
        }
        if let Some((hashes, consumed)) = raw_string_prefix_len(bytes, i) {
            mode = Mode::RawString { hashes };
            i += consumed;
            continue;
        }

        if let Some(consumed) = raw_identifier_len(bytes, i) {
            let token = &content[i..i + consumed];
            i += consumed;
            handle_identifier_token(
                token,
                &mut expect_mod_ident,
                &mut pending_mod_name,
                &mut expect_mod_open,
            );
            continue;
        }

        if is_ident_start(byte) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let token = &content[start..i];
            handle_identifier_token(
                token,
                &mut expect_mod_ident,
                &mut pending_mod_name,
                &mut expect_mod_open,
            );
            continue;
        }

        if byte == b'{' {
            if let Some(module_name) = pending_mod_name.take() {
                // Only inline `mod name { ... }` blocks affect the active module stack.
                // `mod name;` declarations leave the current file's line mapping unchanged.
                module_stack.push(module_name.clone());
                brace_stack.push(Some(module_name));
                if let Some(current) = line_paths.get_mut(line - 1) {
                    *current = current_module_path(&module_stack);
                }
            } else {
                brace_stack.push(None);
            }
            expect_mod_ident = false;
            expect_mod_open = false;
            i += 1;
            continue;
        }

        if byte == b'}' {
            if let Some(marker) = brace_stack.pop() {
                if marker.is_some() {
                    let _ = module_stack.pop();
                    if let Some(current) = line_paths.get_mut(line - 1) {
                        *current = current_module_path(&module_stack);
                    }
                }
            }
            expect_mod_ident = false;
            expect_mod_open = false;
            i += 1;
            continue;
        }

        if byte == b';' {
            pending_mod_name = None;
            expect_mod_ident = false;
            expect_mod_open = false;
            i += 1;
            continue;
        }

        i += 1;
    }

    line_paths
}

pub(crate) fn parse_file_modules(
    file_path: &str,
    module_root: &Path,
) -> Option<(String, Vec<String>)> {
    let content = fs::read_to_string(file_path).ok()?;
    let base_module = module_path_from_file_with_root(file_path, module_root);
    let line_modules = build_line_module_paths(&content);
    Some((base_module, line_modules))
}
