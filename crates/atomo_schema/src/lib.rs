//! Atomo Schema - TypeScript to Rust Code Generation
//!
//! This crate implements Hasura v2 style GraphQL code generation:
//! parsing TypeScript interface definitions and generating corresponding
//! Rust structs and GraphQL resolvers for Hasura v2 compatibility.
#![allow(dead_code)]
// Pre-existing lints in code-generation modules; cleaned up incrementally.
#![allow(
    clippy::needless_range_loop,
    clippy::useless_format,
    clippy::new_without_default,
    clippy::regex_creation_in_loops,
    clippy::for_kv_map,
    clippy::manual_strip,
    clippy::if_same_then_else,
    clippy::match_single_binding
)]

pub mod dsl_parser;
pub mod hasura_v2_resolver_generator;
pub mod hasura_v2_type_generator;
pub mod hook_access_generator;
pub mod operation_definitions;
pub mod parser;
pub mod types;
pub mod typescript_parser;

#[cfg(test)]
pub mod integration_tests;

pub use dsl_parser::*;
pub use hasura_v2_resolver_generator::*;
pub use hasura_v2_type_generator::*;
pub use hook_access_generator::*;
pub use operation_definitions::*;
pub use parser::*;
pub use types::*;
pub use typescript_parser::*;
