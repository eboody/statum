use crate::source::scan::{ScannedToken, TokenKind, matching_close};

#[derive(Clone, Debug)]
struct PendingModule {
    open_index: usize,
    name: String,
    start_line: usize,
}

#[derive(Clone, Debug)]
pub(super) struct ModuleRange {
    pub(super) path: String,
    pub(super) start_line: usize,
    pub(super) end_line: usize,
    pub(super) close_line: usize,
    pub(super) depth: usize,
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

pub(super) fn scan_inline_module_ranges(tokens: &[ScannedToken]) -> Vec<ModuleRange> {
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
                            close_line: token.line,
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
