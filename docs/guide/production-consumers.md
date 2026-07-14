# Building a Production Service

How to structure a real consumer service around Atomo — project layout, schema
discipline, worker patterns, and upgrade habits. Everything here is distilled from
a production consumer that has run Atomo since v0.2.x and filed the feedback that
shaped releases v0.2.2 through v0.6.2; these are the practices that avoided (or
were born from) real incidents, not style opinions.

This page is about *your service's* shape. For hardening the Atomo server itself
(TLS, secrets, metrics), see the
[Production Readiness Checklist](/guide/advanced/production-readiness).

## Project layout

A production consumer is typically a small directory next to your app — Atomo is
the backend; you ship a schema, a thin worker, and compose:

```
backend/
├── schema.ts               # the source of truth — models, access, constraints, ui
├── docker-compose.yml      # db + server (pinned image) + migrate + worker
├── docker-compose.prod.yml
├── migrations/             # ONLY what the schema DSL can't express (see below)
├── worker/                 # thin @atomo-cc/worker-sdk process (external API keys live here)
└── docs/                   # your adoption notes, upgrade log, feedback tracker
```

Two rules make this layout work:

- **The schema is the integrity boundary.** Every constraint that *can* live in
  `schema.ts` should. Each Atomo release has moved more into the schema
  (`@unique`, `@@unique … WHERE`, `select()` value enforcement, `builtins`);
  whenever an upgrade makes something expressible, move it out of raw SQL.
- **The worker is thin.** It holds third-party API keys and does slow external
  work. Business integrity (debits, uniqueness, state transitions) belongs in the
  schema and the server — not in worker code.

## Schema discipline

- **Declare `access` on every model.** A model without rules is open to any
  authenticated caller. Use `"system"` for server-written models (event logs,
  ledgers) — clients get FORBIDDEN, only trusted server paths write. Since
  v0.6.2 the admin UI also reads these rules and hides buttons a role can't use.
- **Use `select()` for constrained fields** (statuses, stages, kinds). Since
  v0.6.2 the declared values are enforced by the validator on every write path
  and render as dropdowns in the admin — free-text drift into a status column
  can't happen.
- **Put an `@unique` idempotency key on every apply-once model.** A credit
  ledger, webhook inbox, or receipt table should carry a unique key (job id,
  receipt id) so a retry can never double-apply. This is the single highest-value
  line in a money schema.
- **Declare `ui.listView` ending with `createdAt`** on append-only models
  (event logs, ledgers) — "when did this happen" is the operator's primary scan
  axis. Metadata-style schemas only (the builder DSL doesn't parse `ui` yet).
- **Extend platform tables with `builtins`, not SQL** (v0.6.0+): extra columns
  and partial-unique anchors on the built-in `users` table (e.g. one platform
  user per external store account) are declarable and applied fail-loud at boot.

## What stays in raw SQL

Keep a `migrations/` directory for the genuinely inexpressible — applied
idempotently by a small compose service after the base tables exist. In a mature
consumer this shrinks to almost nothing; the reference consumer is down to
**stored procedures only** (an atomic check-and-debit primitive). Write every
statement idempotently (`IF NOT EXISTS` / `CREATE OR REPLACE`), because the file
re-runs on every deploy. When an upgrade lets you move something into the schema,
replace the statement with a guarded cleanup — e.g. drop your hand-named index
only once Atomo's replacement exists, so the constraint is never unenforced.

## Worker patterns

Use [`@atomo-cc/worker-sdk`](https://www.npmjs.com/package/@atomo-cc/worker-sdk)
— it owns the `lease → heartbeat → complete/fail` loop; you supply handlers.

- **Handlers must be idempotent.** At-least-once delivery is the contract: a
  crashed worker's lease expires and the job re-runs. Key external side effects
  on the job id.
- **Throw `NonRetryableError` for permanently-bad input** (malformed payload,
  4xx from a provider) so it dead-letters instead of burning retries.
- **Assert the server version on boot.** Set `ATOMO_EXPECTED_VERSION` in your
  worker env and compare it against `GET /version` before starting the loop.
  This one habit catches "the image tag says v0.X but the server isn't" — a
  whole class of silent deploy drift — at startup instead of in production
  behavior.
- **Scope worker tokens narrowly**: mint per-worker tokens (`POST
  /jobs/workers`, admin) with only the queues and `crud:` capabilities the
  worker needs; revoke on decommission.
- **Handle `SIGTERM`** with `worker.stop()` so in-flight jobs complete or
  cleanly return to the queue.

## Money paths

For must-not-double operations (debits, refunds, quota), compose three pieces:

1. a **conditional single-statement `UPDATE`** as the no-overdraw guarantee
   (`… SET balance = balance - $cost WHERE user = $u AND balance >= $cost`),
2. an **idempotent ledger insert** (the `@unique` key above),
3. run both atomically — via a transactional plugin route (a route returning a
   `transaction` array with `expect.minRowsAffected` gates, v0.2.4+) or a stored
   procedure from `migrations/`.

Never split a debit across a worker round trip.

## Operations & upgrades

- **Pin versioned image tags** (`ghcr.io/atomo-cc/atomo-server:v0.6.2`), never
  `:latest`, and keep the pin, the `Dockerfile` build arg, and
  `ATOMO_EXPECTED_VERSION` in lockstep — one grep should find them all.
- **Admin credentials seed once.** `ADMIN_EMAIL`/`ADMIN_PASSWORD` create the
  admin on first boot only; changing the env later is a no-op (the server WARNs).
  Rotate in-app, or opt in with `ADMIN_RESET_PASSWORD=true` for one boot.
- **Smoke-test upgrades on an isolated stack before touching prod.** The
  pattern: throwaway Postgres + the new server image with your real `schema.ts`
  mounted, on unused ports. Verify `GET /version`, boot logs (fail-loud checks
  run at startup), and any migration-sensitive DDL in `pg_indexes` /
  `information_schema` — then bump the prod pin. Minutes of cost; it has caught
  real issues before every adoption.
- **Watch the queue in the admin.** `/admin` → Observability (admin role,
  v0.6.2+) shows job counts by status, a stall warning when the oldest queued
  job ages, and recent failures — the first place to look when background work
  "stopped happening".
- **Keep an upgrade log.** One markdown file recording each pin bump, what the
  release changed for you, and what you verified. When something regresses two
  versions later, this file is the bisect.

## File feedback upstream

The fastest way to get a platform gap fixed is a reproducible report: version,
one-minute repro, what you expected, and — if you can — the exact file/line.
Consumer feedback in that shape has repeatedly shipped as a fix within a version
or two. Track your open items in your own `docs/` so adoption notes and asks
live next to the service that needs them.
