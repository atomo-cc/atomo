#!/bin/sh
# Bootstrap a NEW test database only. Never target an application/shared server.
set -eu
: "${APP_DATABASE_PASSWORD:?random local test password required}"
: "${APP_DATABASE:?new isolated database name required}"
: "${APP_DATABASE_ROLE:?new isolated role name required}"

psql -X -v ON_ERROR_STOP=1 -v role="$APP_DATABASE_ROLE" \
  -v password="$APP_DATABASE_PASSWORD" -v database="$APP_DATABASE" <<'SQL'
SELECT format('CREATE ROLE %I LOGIN NOSUPERUSER NOBYPASSRLS PASSWORD %L', :'role', :'password')\gexec
SELECT format('CREATE DATABASE %I OWNER %I', :'database', :'role')\gexec
SQL

# vector is not a trusted extension: install as bootstrap, then run the entire
# suite as the non-bypass database owner (including extension-dependent tests).
PGDATABASE="$APP_DATABASE" psql -X -v ON_ERROR_STOP=1 \
  -c 'CREATE EXTENSION vector'
PGDATABASE="$APP_DATABASE" PGUSER="$APP_DATABASE_ROLE" \
  PGPASSWORD="$APP_DATABASE_PASSWORD" psql -X -v ON_ERROR_STOP=1 \
  -c 'SELECT current_user, rolsuper, rolbypassrls FROM pg_roles WHERE rolname = current_user'
