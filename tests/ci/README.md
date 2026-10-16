# PostgreSQL CI bootstrap

`init-postgres.sh` is only for a disposable PostgreSQL test service with pgvector
available. It deliberately creates a **new** role and database and fails if either
name already exists. It never drops databases, resets roles or cleans shared data.

Use standard libpq bootstrap connection variables (`PGHOST`, `PGUSER`,
`PGPASSWORD`, `PGDATABASE`). Supply fresh `APP_DATABASE` and `APP_DATABASE_ROLE`
names plus a random `APP_DATABASE_PASSWORD`; keep passwords out of logs and Git.
The bootstrap role creates the non-superuser, non-BYPASSRLS database owner and
preinstalls the untrusted vector extension. The final query reports only the
application role's name and security flags.

The CI Test Suite runs **all** workspace tests, including every serial ignored
PostgreSQL test, using the application role. `ATOMO_ENABLE_RLS=true` enables the
transaction-local tenant binding needed by tests that install policies. A separate
invocation of the existing role-check regression uses the bootstrap role to prove
privileged connections are refused; it does not use the bypass override to run
application queries. No security tests are filtered out of the full suite.

The admin smoke fixture intentionally models global management records. Article
and AuditEvent explicitly declare `tenant: 'global'`; their CRUD permissions stay
unchanged. This does not relax the Required default or assign tenants to existing
users/rows. The same nine-case browser spec covers dev, production preview and
the server's `/admin/` path. Local backend verification instructions live in
[the isolated verification guide](../m1/README.md); toolchain-container results
are distinct from packaged runtime-image verification.
