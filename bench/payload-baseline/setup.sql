-- Create Payload's bench tables manually (avoids migration conflicts with existing shared tables).
-- Schema matches what Payload 3 + @payloadcms/db-postgres generates.

CREATE TABLE IF NOT EXISTS bench_payload_notes (
  id serial PRIMARY KEY NOT NULL,
  title varchar,
  updated_at timestamp(3) with time zone DEFAULT now() NOT NULL,
  created_at timestamp(3) with time zone DEFAULT now() NOT NULL
);

CREATE TABLE IF NOT EXISTS bench_payload_tags (
  id serial PRIMARY KEY NOT NULL,
  label varchar,
  note_id integer REFERENCES bench_payload_notes(id) ON DELETE SET NULL,
  updated_at timestamp(3) with time zone DEFAULT now() NOT NULL,
  created_at timestamp(3) with time zone DEFAULT now() NOT NULL
);

CREATE INDEX IF NOT EXISTS bench_payload_notes_updated_at_idx ON bench_payload_notes USING btree (updated_at);
CREATE INDEX IF NOT EXISTS bench_payload_notes_created_at_idx ON bench_payload_notes USING btree (created_at);
CREATE INDEX IF NOT EXISTS bench_payload_tags_note_idx ON bench_payload_tags USING btree (note_id);
CREATE INDEX IF NOT EXISTS bench_payload_tags_updated_at_idx ON bench_payload_tags USING btree (updated_at);
CREATE INDEX IF NOT EXISTS bench_payload_tags_created_at_idx ON bench_payload_tags USING btree (created_at);

-- Payload system tables (idempotent — may already exist from other Payload projects)
CREATE TABLE IF NOT EXISTS payload_kv (
  id serial PRIMARY KEY NOT NULL,
  key varchar NOT NULL,
  data jsonb NOT NULL
);

CREATE TABLE IF NOT EXISTS payload_migrations (
  id serial PRIMARY KEY NOT NULL,
  name varchar,
  batch numeric,
  updated_at timestamp(3) with time zone DEFAULT now() NOT NULL,
  created_at timestamp(3) with time zone DEFAULT now() NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
  id serial PRIMARY KEY NOT NULL,
  updated_at timestamp(3) with time zone DEFAULT now() NOT NULL,
  created_at timestamp(3) with time zone DEFAULT now() NOT NULL,
  email varchar NOT NULL,
  reset_password_token varchar,
  reset_password_expiration timestamp(3) with time zone,
  salt varchar,
  hash varchar,
  login_attempts numeric DEFAULT 0,
  lock_until timestamp(3) with time zone
);

CREATE TABLE IF NOT EXISTS users_sessions (
  _order integer NOT NULL,
  _parent_id integer NOT NULL,
  id varchar PRIMARY KEY NOT NULL,
  created_at timestamp(3) with time zone,
  expires_at timestamp(3) with time zone NOT NULL
);

CREATE TABLE IF NOT EXISTS payload_locked_documents (
  id serial PRIMARY KEY NOT NULL,
  global_slug varchar,
  updated_at timestamp(3) with time zone DEFAULT now() NOT NULL,
  created_at timestamp(3) with time zone DEFAULT now() NOT NULL
);

CREATE TABLE IF NOT EXISTS payload_locked_documents_rels (
  id serial PRIMARY KEY NOT NULL,
  "order" integer,
  parent_id integer NOT NULL,
  path varchar NOT NULL,
  bench_payload_notes_id integer,
  bench_payload_tags_id integer,
  users_id integer
);

CREATE TABLE IF NOT EXISTS payload_preferences (
  id serial PRIMARY KEY NOT NULL,
  key varchar,
  value jsonb,
  updated_at timestamp(3) with time zone DEFAULT now() NOT NULL,
  created_at timestamp(3) with time zone DEFAULT now() NOT NULL
);

CREATE TABLE IF NOT EXISTS payload_preferences_rels (
  id serial PRIMARY KEY NOT NULL,
  "order" integer,
  parent_id integer NOT NULL,
  path varchar NOT NULL,
  users_id integer
);
