use std::collections::VecDeque;

#[derive(Clone)]
pub struct EnumEntry {
    pub item: syn::ItemEnum,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

#[derive(Clone)]
pub struct StructEntry {
    pub item: syn::ItemStruct,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

#[derive(Clone)]
pub struct TypeAliasEntry {
    pub item: syn::ItemType,
    pub line_number: usize,
}

#[derive(Clone, Default)]
pub struct FileAnalysis {
    pub enums: Vec<EnumEntry>,
    pub structs: Vec<StructEntry>,
    pub type_aliases: Vec<TypeAliasEntry>,
}

#[derive(Default)]
pub(super) struct DeclarationLines {
    pub(super) enums: VecDeque<usize>,
    pub(super) structs: VecDeque<usize>,
    pub(super) type_aliases: VecDeque<usize>,
}
