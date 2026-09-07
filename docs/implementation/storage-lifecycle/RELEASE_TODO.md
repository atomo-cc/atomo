# v0.7.0 release TODO

Authorized release scope: storage lifecycle and its verified query, nullable mutation, projection and admin fixes. Follow the repository Release Checklist; do not push directly to main. Update each item immediately after its evidence passes.

- [x] REL-01 DONE — local workspace check and all-target clippy passed (`data-release-check.log`, `data-release-clippy.log`); feature pushed and [PR #78](https://github.com/atomo-cc/atomo/pull/78) opened. Five-job [CI 34145166472](https://github.com/atomo-cc/atomo/actions/runs/34145166472) dispatched on `84c276b`. Existing nine browser tests are discovered with the screenshot configuration; CI retains review screenshots for seven days.
- [ ] REL-02 IN_PROGRESS (REL-01) — all five feature CI jobs pass; squash-merge feature PR, then dispatch and verify main CI.

  First CI run: Rust/PostgreSQL, frontend and lint passed; admin smoke passed 7/9, then failed loading schema and seeding the restarted worker. Evidence retained in run 34145166472 and its screenshot artifact. The expanded suite shares one IP and repeatedly reloads the SPA against the default 100-request window; the isolated e2e server now has an explicit 1,000-request/60-second budget. Added status/path-only response diagnostics and seed HTTP status assertions; the old run did not capture HTTP statuses, so rate exhaustion is a diagnosis to verify, not a claimed observed 429. Production defaults, limiter tests and all nine UI assertions remain unchanged. A complete new CI run is required.
- [ ] REL-03 TODO (REL-02) — create release branch; cut changelog, update all nine Rust crates and both public npm packages to 0.7.0, regenerate lockfiles, reconcile released documentation and verify builds/package contents.
- [ ] REL-04 TODO (REL-03) — release PR reviewed and verified, squash-merged; tag the merged main commit and create GitHub release with accurate notes.
- [ ] REL-05 TODO (REL-04) — publish both npm packages; dispatch Docker and documentation workflows from main with explicit version; verify registry versions, image revision and successful workflows.
- [ ] REL-06 TODO (REL-05) — update authorized dependent deployment to the verified image, preserve its state, verify service health and finalize release evidence.

CLI source is unchanged from v0.6.5: optional CLI binary publication is skipped unless release preparation finds a relevant CLI change. crates.io publishing remains disabled by repository policy. Public packages and the server image are still required.

Feature implementation and local test evidence: [TODO.md](TODO.md). This checklist records publication status separately; local feature completion does not imply release completion.

Publication preflight: GitHub identity is available. `npm whoami` returned 401 on 2026-09-08; account login is required before REL-05 npm publication. Registry currently reports both public SDKs at 0.5.10; all nine Rust crates are 0.6.4 despite the existing v0.6.5 GitHub tag. REL-03 will align package versions to 0.7.0. Never record credentials in release evidence.
