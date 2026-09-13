# Media tenant fix

- [x] Implement and verify tenant resolver and private metadata route. Release server compilation and four media unit tests passed; HTTP access matrix verification follows.
- [x] Verify real HTTP NULL-admin upload, tenant isolation, legacy compatibility and metadata access. All seven PostgreSQL-backed router tests passed, including the new NULL-admin/Viewer/foreign-tenant matrix.
- [x] Build candidate image and record deployment/rollback evidence. Candidate be7d116c passed restored-backup 20-model-plus-media fingerprint checks, authenticated HTTP tenant upload/metadata, conditional revision append/CAS and SQL-before-byte-unlink retention. Shared deployment remains an operator step.

No schema, configuration, SDK, admin navigation or public byte-serving change is needed. Generic REST behavior is documented in the storage API and changelog.
