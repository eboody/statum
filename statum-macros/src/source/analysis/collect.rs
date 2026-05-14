use super::model::{DeclarationLines, EnumEntry, FileAnalysis, StructEntry, TypeAliasEntry};

pub(super) fn collect_items(
    items: Vec<syn::Item>,
    analysis: &mut FileAnalysis,
    lines: &mut DeclarationLines,
) -> Option<()> {
    for item in items {
        match item {
            syn::Item::Enum(item_enum) => {
                analysis.enums.push(EnumEntry {
                    attrs: attribute_names(&item_enum.attrs),
                    line_number: lines.enums.pop_front()?,
                    item: item_enum,
                });
            }
            syn::Item::Struct(item_struct) => {
                analysis.structs.push(StructEntry {
                    attrs: attribute_names(&item_struct.attrs),
                    line_number: lines.structs.pop_front()?,
                    item: item_struct,
                });
            }
            syn::Item::Type(item_type) => {
                analysis.type_aliases.push(TypeAliasEntry {
                    line_number: lines.type_aliases.pop_front()?,
                    item: item_type,
                });
            }
            syn::Item::Mod(item_mod) => {
                if let Some((_, nested_items)) = item_mod.content {
                    collect_items(nested_items, analysis, lines)?;
                }
            }
            _ => {}
        }
    }

    Some(())
}

fn attribute_names(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut names = Vec::new();

    for attr in attrs {
        let Some(ident) = attr.path().get_ident() else {
            continue;
        };
        let name = ident.to_string();
        if !names.iter().any(|existing| existing == &name) {
            names.push(name);
        }
    }

    names
}
