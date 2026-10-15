# M1 isolated verification

Run from the repository root in a dedicated checkout. All Rust commands run in
WSL Docker. Never point this suite at an application database: serial PostgreSQL
tests create and drop fixture tables, including users.

Create the ignored credential file once, without printing its contents
(PowerShell):

~~~powershell
if (Test-Path tests/m1/.env) { throw 'Preserve the existing local credential file' }
$random = [System.Security.Cryptography.RandomNumberGenerator]::Create()
$bytes = New-Object byte[] 32
$random.GetBytes($bytes)
$password = [BitConverter]::ToString($bytes).Replace('-', '').ToLowerInvariant()
[IO.File]::WriteAllText((Join-Path (Get-Location) 'tests/m1/.env'), "M1_PASSWORD=$password" + [char]10, [Text.UTF8Encoding]::new($false))
$random.Dispose()
Remove-Variable password, bytes
~~~

The Compose project and tool image use fixed random labels. For a separate
checkout, choose a new project name with COMPOSE_PROJECT_NAME in .env before
starting anything; the host scripts export this file. Preserve the existing
project name when continuing a previous run. No host database ports are exposed.
Database, Cargo registry and target volumes belong to that project. Never use
down -v or reuse another project's database. Initialization rotates only this
project's two local database-role passwords; it does not remove previous databases.

~~~powershell
wsl --exec docker compose -f tests/m1/compose.yaml build rust
wsl --exec docker compose -f tests/m1/compose.yaml up -d db
wsl --exec sh tests/m1/define-host.sh targeted
wsl --exec sh tests/m1/define-host.sh full
~~~

Each host-script invocation creates a fresh database name and records it locally.
Avoid simultaneous invocations because database names have one-second precision.
The targeted pass runs runtime-loader, Schema and real PostgreSQL/HTTP security
tests. The full pass runs workspace tests, serial ignored PostgreSQL tests,
server/CLI builds, formatting and strict Clippy with all targets. Each command's
actual exit code goes to local results; any failing command makes the
runner fail. Logs may contain environment-specific details: keep them local.
Keep shell scripts LF. The older final-checks.sh is historical diagnostics and
excludes its historical lint failure from its aggregate; use define-host.sh
for the current mandatory gates.

The schema crate's m1_security.rs tests parser contracts. Runtime loader tests
also exercise builder, legacy and defineModel entry points. The server crate's
m1_security.rs drives production GraphQL, action and Worker routers through
HTTP request/response objects against real PostgreSQL. It covers positive role
and owner cases, denial, rollback, spoofing, tenant mismatch, job delegation and
invalid declarations rejected before migrations. It does not start a separate
listening server or browser. catalog.sh performs read-only fixture inspection.

At source revision 5274717, verification recorded 340 ordinary tests and 107
PostgreSQL tests passing, server/CLI builds, fmt and strict all-target Clippy
passing. Worker SDK 23 tests and SDK/schema type checks passed at the earlier
merge revision 039f2f0; later parser revisions did not rerun those SDK checks.
The Docker images used were a local Rust toolchain image built from the pinned
base in Dockerfile and PostgreSQL 16 Alpine. These results exercise candidate
source in a toolchain container; no candidate runtime image was built, deployed
or published, and no local browser verification is claimed.
