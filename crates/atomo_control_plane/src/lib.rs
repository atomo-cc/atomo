//! Atomo Multi-Project Control Plane
//!
//! Runs many isolated Atomo projects on shared infrastructure: **silo per project**
//! (one database + one `atomo-server` instance each), **pool per tenant** inside each
//! project (shared schema + `tenant_id` + RLS — see the core crates).
//!
//! This crate is **purely additive**: it sits *in front of* unmodified `atomo-server`
//! instances. It never changes the per-project server.
//!
//! Module map (one area per delivery phase of `docs/guide/advanced/multi-project-design.md`):
//! - [`error`], [`registry`], [`driver`], [`secrets`] — Phase 0 foundations (shared contracts).
//! - [`provisioner`], [`docker`], [`caddy`] — Phase 1 (provisioner + Docker driver + gateway).
//! - [`api`], [`reconciler`] — Phase 2 (control-plane API + reconcile loop).
//! - [`ops`] — Phase 4 (backups, observability, resource limits).
//! - [`extensions`] — Phase 5 (multi-host drivers, shared identity / SSO).
//!
//! Phase 3 (multi-tenant RLS) lives in the core crates (`atomo_schema` / `atomo_server`),
//! not here — it is independent of the control plane.

pub mod error;
pub mod registry;
pub mod driver;
pub mod secrets;

pub mod provisioner;
pub mod docker;
pub mod caddy;

pub mod api;
pub mod reconciler;

pub mod ops;
pub mod extensions;

pub use error::{ControlPlaneError, Result};
pub use registry::{DesiredState, Project, ProjectRegistry, ProjectStatus, SchemaRef};
pub use driver::{Driver, InstanceHandle, InstanceState};
pub use secrets::{EnvSecretStore, SecretStore, SsmSecretStore};
pub use provisioner::Provisioner;
