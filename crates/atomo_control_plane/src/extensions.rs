//! Optional extensions — multi-host drivers (nomad/k8s) + a shared identity/SSO plane. (Phase 5.)
//!
//! # Stub vs real
//!
//! Everything in this module **compiles and presents a usable interface**, but the action
//! paths are deliberately unimplemented:
//!
//! - **Types & wiring are real:** [`NomadDriver`] and [`K8sDriver`] genuinely `impl
//!   [crate::driver::Driver]`, so they slot into the control plane exactly where the v1
//!   [`crate::docker`] driver does — the orchestrator selects a driver by name and never
//!   knows the difference. [`SsoPlane`] / [`Principal`] are real, documented value types.
//! - **Behavior is deferred:** the start/stop/restart methods return
//!   [`ControlPlaneError::unimplemented`] and [`SsoPlane::verify`] does the same. `state()`
//!   reports [`InstanceState::Unknown`] (truthful: we have not probed anything).
//!
//! # "Build only on need" stance
//!
//! Per the design doc's *Phase 5 — Optional extensions* and *Non-goals*, these seams exist so
//! that the rest of the control plane is shaped correctly (driver-agnostic orchestration,
//! identity kept out of the project-data path), **not** so they can be used today. The single
//! host + Docker driver covers the operator's own portfolio; nomad/k8s are for horizontal
//! scale-out and SSO is for "one human needs one login across projects" — neither is built
//! speculatively. Each `unimplemented` body carries a thorough doc comment describing exactly
//! how a full implementation maps onto the underlying system, so the work is *specified* even
//! though it is not *done*.

use crate::driver::{Driver, InstanceHandle, InstanceState};
use crate::error::{ControlPlaneError, Result};
use crate::registry::Project;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Driver names recognized by the multi-host extension. The orchestrator can use this to
/// validate a configured driver name before attempting to construct one.
///
/// Kept as a stable const so callers (and tests) can reference the canonical names without
/// duplicating string literals.
pub const MULTI_HOST_DRIVERS: &[&str] = &[NomadDriver::NAME, K8sDriver::NAME];

// =====================================================================================
// Multi-host drivers
// =====================================================================================

/// Runs each per-project `atomo-server` as a **Nomad job** for horizontal scale-out.
///
/// This is a Phase-5 scaffold: the trait is implemented for real (so it drops into the same
/// orchestration path as the Docker driver) but the action methods return
/// [`ControlPlaneError::unimplemented`] until a real multi-host need appears.
///
/// # Full-implementation mapping
///
/// A complete `NomadDriver` would talk to the Nomad HTTP API (`http://<addr>:4646/v1/...`,
/// equivalently `nomad job run`/`nomad job stop`) holding a Nomad client + the cluster
/// address and an ACL token:
///
/// - **`start`** — render a Nomad **job spec** from the [`Project`] + resolved `env`:
///   one task group, one `docker`/`exec` task running the `atomo-server` image, the
///   already-injected secrets (`DATABASE_URL`, `JWT_SECRET`, …) as the task's `env` block,
///   plus `ATOMO_PROJECT_ID` for observability. Submit via `PUT /v1/jobs` (or `job run`),
///   poll the resulting **evaluation** until the **allocation** is `running`, then resolve
///   the allocation's published port / service address to build the
///   [`InstanceHandle::upstream`]. The job id becomes [`InstanceHandle::driver_ref`].
///   Service discovery (Nomad services / Consul) supplies the `host:port` the gateway
///   routes to.
/// - **`stop`** — `DELETE /v1/job/<id>` (i.e. `nomad job stop`); idempotent if the job is
///   already absent.
/// - **`restart`** — resubmit the (possibly updated) job spec; Nomad performs a rolling
///   replacement of the allocation, yielding a fresh handle.
/// - **`state`** — read the job's allocation status (`GET /v1/job/<id>/allocations`) and map
///   `running` → [`InstanceState::Running`], `complete`/`stopped` → [`InstanceState::Stopped`],
///   `failed`/`lost` → [`InstanceState::Failed`], anything else → [`InstanceState::Unknown`].
///
/// Per-job CPU/memory `resources` stanzas give the hard isolation called out in the design
/// doc's *Resource isolation & scaling* section.
#[derive(Debug, Clone)]
pub struct NomadDriver {
    /// Base address of the Nomad API (e.g. `http://127.0.0.1:4646`). Stored, not yet used.
    pub address: String,
    /// Optional Nomad ACL token / namespace selector, when the cluster is secured.
    pub token: Option<String>,
}

impl NomadDriver {
    /// Canonical driver name (matches the design doc and [`MULTI_HOST_DRIVERS`]).
    pub const NAME: &'static str = "nomad";

    /// Construct a driver bound to a Nomad cluster address. Does not connect (lazy).
    pub fn new(address: impl Into<String>) -> Self {
        Self {
            address: address.into(),
            token: None,
        }
    }

    /// Attach an ACL token / namespace credential.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }
}

#[async_trait]
impl Driver for NomadDriver {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn start(
        &self,
        _project: &Project,
        _env: &HashMap<String, String>,
    ) -> Result<InstanceHandle> {
        Err(ControlPlaneError::unimplemented(
            "NomadDriver::start (Phase 5 multi-host scale-out)",
        ))
    }

    async fn stop(&self, _project: &Project) -> Result<()> {
        Err(ControlPlaneError::unimplemented(
            "NomadDriver::stop (Phase 5 multi-host scale-out)",
        ))
    }

    async fn restart(
        &self,
        _project: &Project,
        _env: &HashMap<String, String>,
    ) -> Result<InstanceHandle> {
        Err(ControlPlaneError::unimplemented(
            "NomadDriver::restart (Phase 5 multi-host scale-out)",
        ))
    }

    async fn state(&self, _project: &Project) -> Result<InstanceState> {
        // Truthful: nothing was provisioned, so we cannot claim Running/Stopped.
        Ok(InstanceState::Unknown)
    }
}

/// Runs each per-project `atomo-server` as a **Kubernetes Deployment + Service** for
/// horizontal scale-out across a cluster.
///
/// Phase-5 scaffold: real trait impl, deferred behavior (see the module docs and
/// [`NomadDriver`] for the shared rationale).
///
/// # Full-implementation mapping
///
/// A complete `K8sDriver` would hold a Kubernetes API client scoped to a namespace
/// (equivalently shelling out to `kubectl`), and per project manage a small set of objects:
///
/// - **`start`** — apply a **Deployment** (one replica of the `atomo-server` image; the
///   resolved `env` secrets surfaced via a per-project **Secret** + `envFrom`, plus
///   `ATOMO_PROJECT_ID`) and a **Service** (ClusterIP) fronting it. Wait for the Deployment's
///   `availableReplicas >= 1`, then set [`InstanceHandle::upstream`] to the in-cluster
///   service DNS (`<svc>.<ns>.svc.cluster.local:<port>`). [`InstanceHandle::driver_ref`]
///   becomes the Deployment name (e.g. `atomo-<project-id>`).
/// - **`stop`** — delete the Deployment (and optionally scale to zero instead, to preserve
///   the Service); idempotent on already-absent objects.
/// - **`restart`** — patch the Deployment's pod template (e.g. an annotation bump or updated
///   env), triggering a rolling restart; return a fresh handle once the new pod is ready.
/// - **`state`** — read Deployment status and map `availableReplicas >= 1` →
///   [`InstanceState::Running`], `replicas == 0` → [`InstanceState::Stopped`], a
///   `ProgressDeadlineExceeded`/crash-looping condition → [`InstanceState::Failed`],
///   otherwise [`InstanceState::Unknown`].
///
/// Resource `requests`/`limits` on the pod give CPU/memory isolation; an Ingress (or the
/// existing Atomo gateway pointed at the Service) handles external routing. The per-project
/// Secret keeps the design doc's invariant that the driver receives already-injected env and
/// never reaches into the control-plane secret store itself.
#[derive(Debug, Clone)]
pub struct K8sDriver {
    /// Kubernetes namespace projects are deployed into (e.g. `atomo`). Stored, not yet used.
    pub namespace: String,
    /// Container image to run for each project (e.g. `ghcr.io/atomo/atomo-server:<tag>`).
    pub image: String,
}

impl K8sDriver {
    /// Canonical driver name (matches the design doc and [`MULTI_HOST_DRIVERS`]).
    pub const NAME: &'static str = "k8s";

    /// Construct a driver bound to a namespace + server image. Does not connect (lazy).
    pub fn new(namespace: impl Into<String>, image: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            image: image.into(),
        }
    }
}

#[async_trait]
impl Driver for K8sDriver {
    fn name(&self) -> &str {
        Self::NAME
    }

    async fn start(
        &self,
        _project: &Project,
        _env: &HashMap<String, String>,
    ) -> Result<InstanceHandle> {
        Err(ControlPlaneError::unimplemented(
            "K8sDriver::start (Phase 5 multi-host scale-out)",
        ))
    }

    async fn stop(&self, _project: &Project) -> Result<()> {
        Err(ControlPlaneError::unimplemented(
            "K8sDriver::stop (Phase 5 multi-host scale-out)",
        ))
    }

    async fn restart(
        &self,
        _project: &Project,
        _env: &HashMap<String, String>,
    ) -> Result<InstanceHandle> {
        Err(ControlPlaneError::unimplemented(
            "K8sDriver::restart (Phase 5 multi-host scale-out)",
        ))
    }

    async fn state(&self, _project: &Project) -> Result<InstanceState> {
        Ok(InstanceState::Unknown)
    }
}

// =====================================================================================
// Shared identity / SSO plane
// =====================================================================================

/// An authenticated subject of the **control-plane-issued** SSO token, mapped down to a
/// per-project identity.
///
/// This is intentionally *not* a project's own user record. The design doc's *Identity & auth*
/// section fixes the v1 model: **per-project identity is the default and is free** — each
/// `atomo-server` instance owns its `users`/`sessions`/`JWT_SECRET`, and a user in project A is
/// unrelated to project B. A shared identity plane is a future extension built only when one
/// human genuinely needs a single login across projects; when that day comes, a verified
/// [`Principal`] is *mapped* to the per-project identity rather than replacing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    /// Stable, control-plane-wide subject id (the SSO "who").
    pub subject: String,
    /// Human-readable display name, if the token carried one.
    pub display_name: Option<String>,
    /// Projects this principal is entitled to, and the role it maps to **within** each.
    /// Empty until a real cross-project entitlement model is built. The map keys are
    /// [`Project::id`]s; values are opaque project-local role names.
    pub project_roles: HashMap<String, String>,
}

impl Principal {
    /// True if this principal is entitled to act on `project_id` at all.
    pub fn can_access(&self, project_id: &str) -> bool {
        self.project_roles.contains_key(project_id)
    }

    /// The project-local role this principal maps to within `project_id`, if any.
    pub fn role_in(&self, project_id: &str) -> Option<&str> {
        self.project_roles.get(project_id).map(String::as_str)
    }
}

/// The shared identity / SSO plane — verifies a **control-plane-issued** token centrally and
/// resolves it to a [`Principal`].
///
/// # Model
///
/// The control plane (not any single project) issues an SSO token to a human operator. That
/// token is verified *here*, centrally, and mapped to a [`Principal`] carrying the projects +
/// roles the human may assume. This keeps the design doc's two hard invariants intact:
///
/// 1. **Control-plane auth is separate from project auth** — verifying an SSO token never, by
///    itself, grants access to project *data*; it only resolves *which* per-project identity a
///    human may step into.
/// 2. **Per-project identity remains the source of truth** — SSO is a mapping layer on top, not
///    a replacement for each instance's own `users`/`sessions`.
///
/// # Deferred
///
/// Per the confirmed v1 decision (per-project identity), [`SsoPlane::verify`] returns
/// [`ControlPlaneError::unimplemented`]. A real implementation would validate the token's
/// signature against the control plane's signing key (e.g. a JWT verified with the
/// control-plane public key / JWKS), check expiry + audience, and load the principal's
/// entitlements from the registry. Built only on demonstrated need.
#[derive(Debug, Clone, Default)]
pub struct SsoPlane {
    /// Expected token audience (the control plane's own identifier). Stored, not yet used.
    pub audience: Option<String>,
    /// Reference to the verification key material (e.g. an SSM ref / JWKS URL), resolved at
    /// verify time. Stored, not yet used.
    pub signing_key_ref: Option<String>,
}

impl SsoPlane {
    /// Construct an unconfigured SSO plane. Verification is deferred (see type docs).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the expected token audience.
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(audience.into());
        self
    }

    /// Set the reference to the verification key material.
    pub fn with_signing_key_ref(mut self, signing_key_ref: impl Into<String>) -> Self {
        self.signing_key_ref = Some(signing_key_ref.into());
        self
    }

    /// Verify a control-plane-issued SSO `token` and resolve it to a [`Principal`].
    ///
    /// Deferred per the v1 per-project-identity decision; returns
    /// [`ControlPlaneError::unimplemented`] for now. See the type-level docs for the full model.
    pub async fn verify(&self, _token: &str) -> Result<Principal> {
        Err(ControlPlaneError::unimplemented(
            "SsoPlane::verify (Phase 5 shared identity / SSO plane)",
        ))
    }
}
