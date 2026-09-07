# v0.7.0 release TODO

Authorized release scope: storage lifecycle and its verified query, nullable mutation, projection and admin fixes. Follow the repository Release Checklist; do not push directly to main. Update each item immediately after its evidence passes.

- [ ] REL-01 IN_PROGRESS — verify local workspace check/clippy, clean feature diff and publication account; push feature branch, open a reviewable PR and dispatch all five CI jobs on its head.
- [ ] REL-02 TODO (REL-01) — all five feature CI jobs pass; squash-merge feature PR, then dispatch and verify main CI.
- [ ] REL-03 TODO (REL-02) — create release branch; cut changelog, update all nine Rust crates and both public npm packages to 0.7.0, regenerate lockfiles, reconcile released documentation and verify builds/package contents.
- [ ] REL-04 TODO (REL-03) — release PR reviewed and verified, squash-merged; tag the merged main commit and create GitHub release with accurate notes.
- [ ] REL-05 TODO (REL-04) — publish both npm packages; dispatch Docker and documentation workflows from main with explicit version; verify registry versions, image revision and successful workflows.
- [ ] REL-06 TODO (REL-05) — update authorized dependent deployment to the verified image, preserve its state, verify service health and finalize release evidence.

CLI source is unchanged from v0.6.5: optional CLI binary publication is skipped unless release preparation finds a relevant CLI change. crates.io publishing remains disabled by repository policy. Public packages and the server image are still required.

Feature implementation and local test evidence: [TODO.md](TODO.md). This checklist records publication status separately; local feature completion does not imply release completion.
