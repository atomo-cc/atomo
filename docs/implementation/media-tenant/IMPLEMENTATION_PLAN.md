# Trusted media tenant context

Resolve explicit tenant context for unbound administrators, preserve bound-user isolation and legacy no-header uploads. Add private metadata verification without exposing storage keys or uploader identity. No existing media rows are reassigned. Worker uploads retain no-tenant behavior and cannot select a tenant. Presigned uploads remain outside this bounded multipart change.

Validation uses unit coverage and a real HTTP router against an isolated database, including NULL-bound admin, cross-tenant denial and same-byte dedup isolation. Build a candidate image without deploying a shared service.
