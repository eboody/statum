#[derive(Clone, Debug)]
pub(crate) struct ScannedToken {
    pub(crate) kind: TokenKind,
    pub(crate) line: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum TokenKind {
    Ident(String),
    Punct(char),
}

pub(crate) fn tokenize_source(content: &str) -> Vec<ScannedToken> {
    let chars: Vec<char> = content.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    let mut line = 1usize;

    while index < chars.len() {
        let ch = chars[index];
        match ch {
            '\n' => {
                line += 1;
                index += 1;
            }
            c if c.is_whitespace() => index += 1,
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
            _ => index += 1,
        }
    }

    tokens
}

pub(crate) fn matching_close(open: char) -> char {
    match open {
        '{' => '}',
        '(' => ')',
        '[' => ']',
        _ => open,
    }
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
            _ => index += 1,
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
            _ => index += 1,
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
    index += 1;

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
