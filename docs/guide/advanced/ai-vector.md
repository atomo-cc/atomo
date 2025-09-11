# AI & Vector Search

This page outlines an embedding-based search pipeline with `pgvector`.

Schema
```sql
CREATE TABLE IF NOT EXISTS post_embeddings (
  post_id UUID PRIMARY KEY,
  embedding vector(1536),
  meta jsonb
);
```

Pipeline
- Chunk & normalize content (strip HTML, split by tokens, limit length).
- Generate embeddings (model choice, batch sizing, retry/backoff).
- Store: upsert into `post_embeddings` with metadata (lang, tags, ts).
- Query: accept a query string → embed → cosine distance search.

Examples
```sql
-- Top 10 most similar posts
SELECT post_id
FROM post_embeddings
ORDER BY embedding <-> $1
LIMIT 10;
```

GraphQL integration
- Expose `similarTo(text: String!, limit: Int)` resolving to the SQL above.
- Combine filters: intersect vector result with domain filters (tenant, tags).

Jobs & backfills
- Use a projector or background job to (re)embed changed entities.
- Track last embedded version to avoid redundant work.

Privacy & safety
- Redact PII before embedding; avoid leaking secrets in prompts.
- Store minimal metadata; encrypt at rest if required by policy.

Cost control
- Batch embeddings; cache by content hash; schedule low-priority backfills.

See also
- Vision → Edge & AI (direction): `/vision`
