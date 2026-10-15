#!/bin/sh
set -eu
psql -U m1 -d m1 -v ON_ERROR_STOP=1 -v password="$M1_PASSWORD" -v database="${M1_DATABASE:-m1_accept}" <<'SQL'
SELECT 'CREATE ROLE m1_app LOGIN NOSUPERUSER NOBYPASSRLS' WHERE NOT EXISTS (SELECT FROM pg_roles WHERE rolname='m1_app')\gexec
ALTER ROLE m1_app PASSWORD :'password';
ALTER ROLE m1 PASSWORD :'password';
SELECT 'CREATE DATABASE m1_accept OWNER m1_app' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname='m1_accept')\gexec
SELECT 'CREATE DATABASE m1_regression OWNER m1_app' WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname='m1_regression')\gexec
SELECT format('CREATE DATABASE %I OWNER m1_app', :'database') WHERE NOT EXISTS (SELECT FROM pg_database WHERE datname=:'database')\gexec
SQL
