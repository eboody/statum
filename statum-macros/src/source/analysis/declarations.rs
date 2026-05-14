use super::model::DeclarationLines;
use crate::source::scan::{ScannedToken, TokenKind, matching_close, tokenize_source};

#[derive(Clone, Debug)]
enum BlockContext {
    Module { close: char },
    Opaque { close: char },
    Normal { close: char },
}

pub(super) fn scan_declaration_lines(content: &str) -> DeclarationLines {
    let tokens = tokenize_source(content);
    let mut lines = DeclarationLines::default();
    let mut block_stack = Vec::new();
    let mut opaque_depth = 0usize;
    let mut pending_module_open = None::<usize>;
    let mut pending_opaque_open = None::<usize>;

    for (index, token) in tokens.iter().enumerate() {
        match &token.kind {
            TokenKind::Ident(keyword)
                if opaque_depth == 0 && module_items_allowed(&block_stack) =>
            {
                match keyword.as_str() {
                    "enum" => lines.enums.push_back(token.line),
                    "struct" => lines.structs.push_back(token.line),
                    "type" => lines.type_aliases.push_back(token.line),
                    "mod"
                        if matches!(
                            tokens.get(index + 1),
                            Some(ScannedToken {
                                kind: TokenKind::Ident(_),
                                ..
                            })
                        ) && matches!(
                            tokens.get(index + 2),
                            Some(ScannedToken {
                                kind: TokenKind::Punct('{'),
                                ..
                            })
                        ) =>
                    {
                        pending_module_open = Some(index + 2);
                    }
                    _ => {}
                }
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
                } else if pending_module_open == Some(index) {
                    block_stack.push(BlockContext::Module {
                        close: matching_close(*open),
                    });
                    pending_module_open = None;
                } else {
                    block_stack.push(BlockContext::Normal {
                        close: matching_close(*open),
                    });
                }
            }
            TokenKind::Punct(close @ ('}' | ')' | ']')) => {
                let Some(context) = block_stack.pop() else {
                    continue;
                };

                match context {
                    BlockContext::Module {
                        close: expected_close,
                    } if expected_close == *close => {}
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

    lines
}

fn module_items_allowed(block_stack: &[BlockContext]) -> bool {
    block_stack
        .iter()
        .all(|context| matches!(context, BlockContext::Module { .. }))
}
