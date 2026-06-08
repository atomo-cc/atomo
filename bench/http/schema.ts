// Minimal schema for the HTTP load-test server (a single trivial model is enough to boot Atomo).
export interface Note {
  id: string;
  title: string;
}
export const schema = {
  models: {
    Note: { tableName: "http_bench_notes", access: { read: "public" } },
  },
};
export default schema;
