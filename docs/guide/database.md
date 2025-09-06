# Database & Projections

Atomo uses SQL (via `sqlx`) and event sourcing.

- Event Store: append-only stream of domain events.
- Projections: `crates/atomo_projectors` build read models for queries.
- Migrations: manage schema in `migrations/` per service.

## Commands
```bash
# Run migrations (use .env for DATABASE_URL)
atomo migrate

# Generate a migration from schema changes
atomo migrate --generate --name add-contacts
```
