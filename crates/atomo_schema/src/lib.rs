//! Atomo Schema - TypeScript to Rust Code Generation
//! 
//! This crate implements the core "Dual-Mode Schema" functionality:
//! parsing TypeScript interface definitions and generating corresponding 
//! Rust structs for the CRM domain models.

pub mod parser;
pub mod generator;
pub mod types;
pub mod typescript_parser;
pub mod graphql_generator;
pub mod resolver_generator;
pub mod hasura_v2_resolver_generator;
pub mod hasura_v2_type_generator;
pub mod modern_type_generator;
pub mod modern_resolver_generator;

pub use parser::*;
pub use generator::*;
pub use types::*;
pub use typescript_parser::*;
pub use graphql_generator::*;
pub use resolver_generator::*;
pub use hasura_v2_resolver_generator::*;
pub use hasura_v2_type_generator::*;
pub use modern_type_generator::*;
pub use modern_resolver_generator::*;
