#![allow(dead_code)]
#![allow(clippy::wrong_self_convention)]

pub mod showcases;
pub mod toy_demos;

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}
