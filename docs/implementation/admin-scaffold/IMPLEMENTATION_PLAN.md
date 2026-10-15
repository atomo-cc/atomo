# Admin scaffold ownership (2026-10-07)

## 2026-10-15 continuation

Use a newly owned PostgreSQL container and native server on dedicated loopback
ports. Add safe login lifecycle timing (path/status/duration/failure only),
compare direct and preview requests, then run production smoke without concurrent
builds. Retain existing HTTP/test timeouts and retries=0. Diagnose worker/browser
and preview teardown separately with bounded runs and cleanup of recorded owned
PIDs only. Require consecutive full-suite exit-0 passes before closing the gate.
Also inspect the packaged `/admin/` path separately from Vite preview; do not
claim Docker image verification from a native server test.

The existing smoke will derive its navigation prefix from `E2E_BASE_URL` so the
same assertions can target `/admin/`. Build with `build:server` and serve via
`ATOMO_ADMIN_DIR` on the isolated native server; add that mode to the existing CI
job. This changes test coverage only (no runtime API/config/dependency changes).
Update existing verification docs, architecture, roadmaps and changelog; no new
public page or navigation entry is required.

Dev-mode isolation exposed that using `VITE_API_URL` to choose a proxy backend
also compiles a cross-origin client URL. Reuse the existing test-only
`E2E_API_URL` as the proxy target override, keeping `VITE_API_URL` empty for
same-origin browser requests. Verify with the existing smoke on an isolated
frontend port; do not broaden backend CORS.

### Verified continuation results

The final same-origin dev smoke passed 9/9 (21.64s). Final production preview
passed 9/9 consecutively (21.11s, 22.33s), and native `/admin/` serving passed 9/9
twice (19.06s, 18.47s), including SPA routes, real login and model navigation.
All five processes exited 0 with no outer timeout. Browser and managed-preview
shutdown completed normally. The historical login/shutdown timeout was not
reproduced; these results do not establish its cause or retire another template.

Evidence: `.tmp/e2e-review-20261015/` (`*-exit.json`, matching logs, safe direct vs
proxy login timings). An occupied development port was refused rather than
reused; a subsequent absolute-API dev diagnostic failed CORS and was stopped.
Only the later same-origin runs are accepted. No CORS/HTTP timeout/retry or
dependency/version changes were made. Docs build, workflow YAML, e2e bash syntax
and diff checks passed. The owned native server and labeled disposable PostgreSQL
container/anonymous volume were removed; evidence remains. No remote CI or
Docker-image verification was performed.

## Original 2026-10-07 plan and diagnostic history

Reuse create-app's canonical schema/Compose layout and packaged server admin UI;
do not fork a second SPA or copy obsolete Docker recipes. Accept fullstack-atomo
as a compatibility alias for crm. Keep admin-ui dependencies/version unchanged.

Verify the real scaffolder entry, all template layouts, rejection without writes,
and npm pack content. Extend the existing CI e2e job to build the actual admin
SPA in production mode, then execute the same admin-smoke spec against preview
and its dedicated e2e backend. Browser page errors must fail the smoke.

No Rust/API/config changes: Rust test/build/clippy checklist is not applicable.
No new public docs page: update existing getting-started/architecture/roadmap,
README/CHANGELOG and package README. No nav/API changes required.
No private consumer changes, dependency/version upgrades, pushes or releases.
See root TODO.md for verified progress and evidence.

Verification notes: the first native server bound only IPv6; restart the owned
test process with HOST=127.0.0.1 for isolated IPv4 ports. The first production
run emitted nine success markers but hung in worker shutdown and is NOT accepted
as a completed pass. Add a five-minute global suite timeout and investigate the
shutdown before retiring the old template. No dependencies upgraded to mask it.

The bounded diagnostic run completed with exit 0: 9 passed (3.1m), including a
roughly one-minute browser fixture close delay. Root cause of the native shutdown
delay is not established. Verify the supported new Chromium headless channel and
managed preview separately; do not claim a dependency regression was repaired.

New-headless diagnostic failed at initial navigation and suite/teardown timed out;
it is not accepted. A same-origin rebuild overlapped that diagnostic, so it is
not evidence of a headless engine defect. Revert the channel change. Final
managed-preview validation must wait for build completion and use default Chromium.

The final same-origin managed-preview run reached the actual SPA/backend, but
case 8 failed to sign in: error-context shows Axios `timeout of 10000ms exceeded`
on the login form, not a wrong locator. Do not increase timeouts or retry to claim
green. Investigate transport/backend timing; keep duplicate snapshot retirement
blocked. One earlier absolute-API run passed 9/9; it does not supersede this result.

Final result: 8 passed / 1 failed (6.7m), exit 1, plus production-preview plugin
teardown exceeded 300s. Admin unit suite passed 29/29, scaffold tests passed 9/9,
production build passed with same-origin API configuration. Both docs builds and
workflow YAML parsing passed. No remote CI, package release or dependency upgrade.

Follow-up: instrument login response/failure and proxy timing without request
bodies/headers, compare direct and proxied latency on a fresh disposable backend,
and isolate production suite runs from builds/consumer browser runs. Inspect
preview shutdown lifecycle and bound cleanup of only owned processes. Do not
increase the Axios/test timeout or enable retries. Retirement requires consecutive
final-configuration passes, not selecting the earlier green diagnostic.
