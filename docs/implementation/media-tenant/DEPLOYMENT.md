# Candidate deployment review

Candidate source: HEAD 73b375017fe7a9428b73a2f57835a602abef8bf4 plus focused media upload/metadata patch. Existing unrelated storage lifecycle release TODO is not part of this change.

Candidate image: atomo-media-tenant-candidate:20260908, SHA256 be7d116cb0b96afe83e5f2ac19a34c8e457a0a680335d6d667088c5252556e50. Built release server with cargo-chef, then overlaid the binary on the previous runtime image; embedded admin assets are deliberately unchanged. Version metadata identifies a local media-tenant candidate, not a published release.

The previous local runtime had dev/unknown/unknown version provenance. Its commit cannot be inferred safely from image creation date. Current binary includes v0.7.0 typed-null, projection reconciliation and storage policy changes; this is a binary upgrade and requires a restored-database startup compatibility check, core API and conditional-update/retention smoke before deploying any shared instance.

No shared service is deployed by the verification procedure. An operator must retain the old image and verified backup, drain writers, compare all application base-row fingerprints before/after startup, and deploy the exact verified image using the service's compose override. Do not run an unpinned latest build. Do not modify old NULL-owned media or globally bind an administrator to one tenant. On rollback retain revision SQL retention/head guards; reverting the binary does not authorize deleting history or removing guards. The old binary cannot provide tenant metadata, so disable the pilot if rolling back.

Only upload/metadata routes and corresponding Rust tests/docs change. No new environment variables, schema migration, client SDK, admin navigation or configuration surface is added. Multipart explicit context is stricter than legacy GraphQL for unbound non-admin users; only administrators may impersonate a selected tenant. Presigned uploads retain existing behavior.


The candidate image preserves the previous runtime filesystem and embedded admin
assets; only the server binary and version stamps differ. Reproduce the binary
stage with the repository Dockerfile's `builder` target. For environments where
the optional Dockerfile frontend registry is unreachable, omitting its syntax
comment from a temporary Dockerfile is sufficient; no Docker daemon or host DNS
configuration change was made. Tests compile the final formatted source using
`cargo test --release -p atomo_server --lib media::tests` and the full
`--test media_http -- --ignored --test-threads=1` against a temporary PostgreSQL DB.

Candidate rollout must verify an existing image and video against their original
bytes after restart. An isolated empty media store cannot prove compatibility with
old storage objects; do not move or rewrite storage keys/ownership during upgrade.


The exact candidate additionally passed two-candidate concurrent conditional
updates (`Promise.all`, identical expected generation/source): exactly one winner,
one empty result, both immutable candidates retained. A same-operation retry
used one no-op conditional update, did not append again, and preserved an
independent reference field edit. These tests ran against a fresh isolated DB.
