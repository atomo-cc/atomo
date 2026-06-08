-- Seed 200 rows so reads have data from the start.
-- The table itself is auto-created by the Atomo server from schema.ts.
INSERT INTO bench_load_notes (id, title, body)
SELECT gen_random_uuid()::text, 'note-' || g, 'body for note ' || g
FROM generate_series(1, 200) g
ON CONFLICT DO NOTHING;

-- The admin user is auto-seeded by the server via ADMIN_EMAIL / ADMIN_PASSWORD env vars.
