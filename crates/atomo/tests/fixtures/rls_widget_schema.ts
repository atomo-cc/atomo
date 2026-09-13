// Minimal schema for the RLS executor test (tests/rls_executor.rs).
export interface Widget {
  id: string;
  name: string;
}

// parse_schema only registers interfaces declared in schema.models; a bare
// interface is parsed but dropped (with a warning). `Widget: {}` is enough —
// entries with empty metadata are still declared models.
export const schema = {
  models: {
    Widget: {},
  },
};
