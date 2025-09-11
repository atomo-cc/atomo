# Performance Tuning

Areas to consider
- Database
  - Index hot paths; avoid N+1 via prefetch or projections.
  - Tune connection pool sizes and timeouts.
- Runtime
  - Enable release builds; use jemalloc (if beneficial) and proper `RUST_LOG`.
  - Profile projector throughput; keep handlers CPU-bound and idempotent.
- GraphQL
  - Limit query depth/complexity; paginate aggressively; cache safe queries.
- Admin UI
  - Use React Query caching; window-virtualize large lists; debounce inputs.

Benchmarks
- Measure P50/P95/P99 for mutation→projection latency.
- Replay rate (ev/min) and read-model rebuild time.
- Memory footprint under sustained write load.

See also
- Vision → Metrics and SLOs: `/vision`
