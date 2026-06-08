export interface BenchNote {
  id: string;
  title: string;
  body: string;
}
export const schema = {
  models: {
    BenchNote: {
      tableName: "bench_load_notes",
      access: {
        read: "authenticated",
        create: "authenticated",
        update: "authenticated",
        delete: "authenticated",
      },
    },
  },
};
export default schema;
