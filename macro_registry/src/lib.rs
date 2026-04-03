#![allow(rustdoc::invalid_rust_codeblocks)]
#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]
#![allow(unexpected_cfgs)]

pub mod analysis;
pub mod callsite;
pub mod query;
pub mod registry;

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}
