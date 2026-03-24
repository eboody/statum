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

#[derive(Clone, Debug)]
struct ScannedToken {
    kind: TokenKind,
    line: usize,
}

#[derive(Clone, Debug)]
enum TokenKind {
    Ident(String),
    Punct(char),
}

#[derive(Clone, Debug)]
struct PendingModule {
    open_index: usize,
    name: String,
    start_line: usize,
}

#[derive(Clone, Debug)]
struct ModuleRange {
    path: String,
    start_line: usize,
    end_line: usize,
    depth: usize,
}

#[derive(Clone, Debug)]
enum BlockContext {
    Module {
        close: char,
        path: String,
        start_line: usize,
        depth: usize,
    },
    Opaque {
        close: char,
    },
    Normal {
        close: char,
    },
}

fn build_line_module_paths(content: &str) -> Option<Vec<String>> {
    let line_count = content.lines().count().max(1);
    let tokens = tokenize_source(content);
    let module_ranges = scan_inline_module_ranges(&tokens);
    let mut line_paths = vec![String::new(); line_count];

    for range in module_ranges {
        set_line_range(
            &mut line_paths,
            range.start_line,
            range.end_line,
            &range.path,
        );
    }

    Some(line_paths)
}

fn tokenize_source(content: &str) -> Vec<ScannedToken> {
    let chars: Vec<char> = content.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;

    while index < chars.len() {
        let ch = chars[index];
        match ch {
            '\n' => {
                line += 1;
                index += 1;
            }
            c if c.is_whitespace() => {
                index += 1;
            }
            '/' if chars.get(index + 1) == Some(&'/') => {
                index += 2;
                while let Some(next) = chars.get(index) {
                    if *next == '\n' {
                        break;
                    }
                    index += 1;
                }
            }
            '/' if chars.get(index + 1) == Some(&'*') => {
                let (next_index, next_line) = skip_block_comment(&chars, index, line);
                index = next_index;
                line = next_line;
            }
            '"' => {
                let (next_index, next_line) = skip_quoted_literal(&chars, index, line, '"');
                index = next_index;
                line = next_line;
            }
            '\'' if is_char_literal_start(&chars, index) => {
                let (next_index, next_line) = skip_quoted_literal(&chars, index, line, '\'');
                index = next_index;
                line = next_line;
            }
            'b' => {
                if chars.get(index + 1) == Some(&'"') {
                    let (next_index, next_line) = skip_quoted_literal(&chars, index + 1, line, '"');
                    index = next_index;
                    line = next_line;
                } else if chars.get(index + 1) == Some(&'\'')
                    && is_char_literal_start(&chars, index + 1)
                {
                    let (next_index, next_line) =
                        skip_quoted_literal(&chars, index + 1, line, '\'');
                    index = next_index;
                    line = next_line;
                } else if is_raw_string_start(&chars, index + 1) {
                    let (next_index, next_line) = skip_raw_string_literal(&chars, index + 1, line);
                    index = next_index;
                    line = next_line;
                } else {
                    let (ident, next_index) = read_identifier(&chars, index);
                    tokens.push(ScannedToken {
                        kind: TokenKind::Ident(ident),
                        line,
                    });
                    index = next_index;
                }
            }
            'r' if is_raw_string_start(&chars, index) => {
                let (next_index, next_line) = skip_raw_string_literal(&chars, index, line);
                index = next_index;
                line = next_line;
            }
            'r' if chars.get(index + 1) == Some(&'#')
                && chars
                    .get(index + 2)
                    .is_some_and(|next| is_ident_start(*next)) =>
            {
                let (ident, next_index) = read_raw_identifier(&chars, index);
                tokens.push(ScannedToken {
                    kind: TokenKind::Ident(ident),
                    line,
                });
                index = next_index;
            }
            c if is_ident_start(c) => {
                let (ident, next_index) = read_identifier(&chars, index);
                tokens.push(ScannedToken {
                    kind: TokenKind::Ident(ident),
                    line,
                });
                index = next_index;
            }
            '{' | '}' | '(' | ')' | '[' | ']' | '!' => {
                tokens.push(ScannedToken {
                    kind: TokenKind::Punct(ch),
                    line,
                });
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    tokens
}

fn skip_block_comment(chars: &[char], start: usize, line: usize) -> (usize, usize) {
    let mut index = start + 2;
    let mut depth = 1usize;
    let mut current_line = line;

    while index < chars.len() {
        match (chars[index], chars.get(index + 1).copied()) {
            ('/', Some('*')) => {
                depth += 1;
                index += 2;
            }
            ('*', Some('/')) => {
                depth -= 1;
                index += 2;
                if depth == 0 {
                    break;
                }
            }
            ('\n', _) => {
                current_line += 1;
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }

    (index, current_line)
}

fn skip_quoted_literal(
    chars: &[char],
    start: usize,
    line: usize,
    terminator: char,
) -> (usize, usize) {
    let mut index = start + 1;
    let mut current_line = line;
    let mut escaped = false;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' {
            current_line += 1;
        }

        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        match ch {
            '\\' => {
                escaped = true;
                index += 1;
            }
            c if c == terminator => {
                index += 1;
                break;
            }
            _ => {
                index += 1;
            }
        }
    }

    (index, current_line)
}

fn skip_raw_string_literal(chars: &[char], start: usize, line: usize) -> (usize, usize) {
    let mut index = start;
    let mut current_line = line;

    if chars.get(index) == Some(&'b') {
        index += 1;
    }
    index += 1; // skip `r`

    let mut hashes = 0usize;
    while chars.get(index) == Some(&'#') {
        hashes += 1;
        index += 1;
    }

    if chars.get(index) != Some(&'"') {
        return (index, current_line);
    }
    index += 1;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '\n' {
            current_line += 1;
            index += 1;
            continue;
        }

        if ch == '"' {
            let mut cursor = index + 1;
            let mut matched_hashes = 0usize;
            while matched_hashes < hashes && chars.get(cursor) == Some(&'#') {
                matched_hashes += 1;
                cursor += 1;
            }
            if matched_hashes == hashes {
                index = cursor;
                break;
            }
        }

        index += 1;
    }

    (index, current_line)
}

fn is_char_literal_start(chars: &[char], start: usize) -> bool {
    let mut index = start + 1;
    let mut escaped = false;

    while let Some(ch) = chars.get(index).copied() {
        if ch == '\n' {
            return false;
        }

        if escaped {
            escaped = false;
            index += 1;
            continue;
        }

        match ch {
            '\\' => {
                escaped = true;
                index += 1;
            }
            '\'' => return true,
            c if c.is_whitespace() => return false,
            _ => index += 1,
        }
    }

    false
}

fn is_raw_string_start(chars: &[char], start: usize) -> bool {
    if chars.get(start) != Some(&'r') {
        return false;
    }

    let mut index = start + 1;
    while chars.get(index) == Some(&'#') {
        index += 1;
    }

    chars.get(index) == Some(&'"')
}

fn is_ident_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

fn read_identifier(chars: &[char], start: usize) -> (String, usize) {
    let mut index = start + 1;
    while chars
        .get(index)
        .is_some_and(|next| is_ident_continue(*next))
    {
        index += 1;
    }

    (chars[start..index].iter().collect(), index)
}

fn read_raw_identifier(chars: &[char], start: usize) -> (String, usize) {
    let mut index = start + 2;
    while chars
        .get(index)
        .is_some_and(|next| is_ident_continue(*next))
    {
        index += 1;
    }

    (chars[start..index].iter().collect(), index)
}

fn scan_inline_module_ranges(tokens: &[ScannedToken]) -> Vec<ModuleRange> {
    let mut ranges = Vec::new();
    let mut active_modules = Vec::new();
    let mut block_stack = Vec::new();
    let mut opaque_depth = 0usize;
    let mut pending_module = None::<PendingModule>;
    let mut pending_opaque_open = None::<usize>;

    for (index, token) in tokens.iter().enumerate() {
        match &token.kind {
            TokenKind::Ident(keyword) if opaque_depth == 0 && keyword == "mod" => {
                let Some(ScannedToken {
                    kind: TokenKind::Ident(name),
                    ..
                }) = tokens.get(index + 1)
                else {
                    continue;
                };
                let Some(ScannedToken {
                    kind: TokenKind::Punct('{'),
                    ..
                }) = tokens.get(index + 2)
                else {
                    continue;
                };

                pending_module = Some(PendingModule {
                    open_index: index + 2,
                    name: name.clone(),
                    start_line: token.line,
                });
            }
            TokenKind::Punct('!') if opaque_depth == 0 => {
                if matches!(
                    tokens.get(index + 1),
                    Some(ScannedToken {
                        kind: TokenKind::Punct('{') | TokenKind::Punct('(') | TokenKind::Punct('['),
                        ..
                    })
                ) {
                    pending_opaque_open = Some(index + 1);
                } else if matches!(
                    tokens.get(index + 1),
                    Some(ScannedToken {
                        kind: TokenKind::Ident(_),
                        ..
                    })
                ) && matches!(
                    tokens.get(index + 2),
                    Some(ScannedToken {
                        kind: TokenKind::Punct('{') | TokenKind::Punct('(') | TokenKind::Punct('['),
                        ..
                    })
                ) {
                    pending_opaque_open = Some(index + 2);
                }
            }
            TokenKind::Punct(open @ ('{' | '(' | '[')) => {
                if pending_opaque_open == Some(index) {
                    block_stack.push(BlockContext::Opaque {
                        close: matching_close(*open),
                    });
                    opaque_depth += 1;
                    pending_opaque_open = None;
                    continue;
                }

                if *open == '{'
                    && pending_module
                        .as_ref()
                        .is_some_and(|pending| pending.open_index == index)
                {
                    let pending = pending_module.take().expect("pending module");
                    let path = if active_modules.is_empty() {
                        pending.name.clone()
                    } else {
                        format!("{}::{}", active_modules.join("::"), pending.name)
                    };
                    let depth = active_modules.len() + 1;
                    active_modules.push(pending.name);
                    block_stack.push(BlockContext::Module {
                        close: '}',
                        path,
                        start_line: pending.start_line,
                        depth,
                    });
                    continue;
                }

                block_stack.push(BlockContext::Normal {
                    close: matching_close(*open),
                });
            }
            TokenKind::Punct(close @ ('}' | ')' | ']')) => {
                let Some(context) = block_stack.pop() else {
                    continue;
                };

                match context {
                    BlockContext::Module {
                        close: expected_close,
                        path,
                        start_line,
                        depth,
                    } if expected_close == *close => {
                        let end_line = if token.line > start_line {
                            token.line - 1
                        } else {
                            token.line
                        };

                        ranges.push(ModuleRange {
                            path,
                            start_line,
                            end_line,
                            depth,
                        });
                        active_modules.pop();
                    }
                    BlockContext::Opaque {
                        close: expected_close,
                    } if expected_close == *close => {
                        opaque_depth = opaque_depth.saturating_sub(1);
                    }
                    BlockContext::Normal {
                        close: expected_close,
                    } if expected_close == *close => {}
                    BlockContext::Module { .. }
                    | BlockContext::Opaque { .. }
                    | BlockContext::Normal { .. } => {}
                }
            }
            _ => {}
        }
    }

    ranges.sort_by_key(|range| (range.depth, range.start_line));
    ranges
}

fn matching_close(open: char) -> char {
    match open {
        '{' => '}',
        '(' => ')',
        '[' => ']',
        _ => open,
    }
}

fn set_line_range(line_paths: &mut [String], start_line: usize, end_line: usize, path: &str) {
    if start_line == 0 || end_line < start_line {
        return;
    }

    for line in start_line..=end_line {
        if let Some(slot) = line_paths.get_mut(line - 1) {
            *slot = path.to_string();
        }
    }
}

pub(crate) fn parse_file_modules(
    file_path: &str,
    module_root: &Path,
) -> Option<(String, Vec<String>)> {
    let content = fs::read_to_string(file_path).ok()?;
    let base_module = module_path_from_file_with_root(file_path, module_root);
    let line_modules = build_line_module_paths(&content)?;
    Some((base_module, line_modules))
}
