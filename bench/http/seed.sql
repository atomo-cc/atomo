INSERT INTO http_bench_notes (id, title)
SELECT gen_random_uuid()::text, 'note-' || g
FROM generate_series(1, 200) g
ON CONFLICT DO NOTHING;
