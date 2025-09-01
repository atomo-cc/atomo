//! Atomo Schema - TypeScript to Rust Code Generation
//! 
//! This crate implements Hasura v2 style GraphQL code generation:
//! parsing TypeScript interface definitions and generating corresponding 
//! Rust structs and GraphQL resolvers for Hasura v2 compatibility.

pub mod parser;
pub mod types;
pub mod typescript_parser;
pub mod hasura_v2_resolver_generator;
pub mod hasura_v2_type_generator;
pub mod operation_definitions;
pub mod dsl_parser;
pub mod hook_access_generator;

#[cfg(test)]
pub mod integration_tests;

pub use parser::*;
pub use types::*;
pub use typescript_parser::*;
pub use hasura_v2_resolver_generator::*;
pub use hasura_v2_type_generator::*;
pub use operation_definitions::*;
pub use dsl_parser::*;
pub use hook_access_generator::*;
